//! Tractor Implement Management (TIM) as a [`Plugin`]. Wraps the local
//! [`TimAuthority`] guard: guarded PTO/hitch/aux commands refuse to emit unless
//! authority is granted and interlocks are clear, and inbound PTO/hitch/aux
//! status/command PGNs are decoded into [`TimEvent`]s.

use crate::isobus::implement::tractor_commands::{
    HitchCommand, HitchCommandMsg, PtoCommand, PtoCommandMsg,
};
use crate::isobus::tim::{
    AuxValveCommand, HitchState, PtoState, TIM_UPDATE_INTERVAL_MS, TimAuthority, TimCommand,
    TimInterlocks, TimOptionSet, TimValidationError,
};
use crate::net::pgn_defs::{
    PGN_AUX_VALVE_0_7, PGN_AUX_VALVE_8_15, PGN_AUX_VALVE_16_23, PGN_AUX_VALVE_24_31,
    PGN_FRONT_HITCH, PGN_FRONT_HITCH_CMD, PGN_FRONT_PTO, PGN_FRONT_PTO_CMD, PGN_REAR_HITCH,
    PGN_REAR_HITCH_CMD, PGN_REAR_PTO, PGN_REAR_PTO_CMD,
};
use crate::net::{Address, BROADCAST_ADDRESS, Error, Message, Pgn, Priority, Result};
use crate::safety::{SafeModeTrigger, TecuSafeMode};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, Hitch, Pto, TimEvent};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[
    PGN_FRONT_PTO,
    PGN_REAR_PTO,
    PGN_FRONT_HITCH,
    PGN_REAR_HITCH,
    PGN_FRONT_PTO_CMD,
    PGN_REAR_PTO_CMD,
    PGN_FRONT_HITCH_CMD,
    PGN_REAR_HITCH_CMD,
    PGN_AUX_VALVE_0_7,
    PGN_AUX_VALVE_8_15,
    PGN_AUX_VALVE_16_23,
    PGN_AUX_VALVE_24_31,
];

/// TIM plugin (local authority/interlock guard + guarded commands).
pub struct Tim {
    authority: TimAuthority,
    front_pto: Option<PtoState>,
    rear_pto: Option<PtoState>,
    front_hitch: Option<HitchState>,
    rear_hitch: Option<HitchState>,
    last_aux: Option<AuxValveCommand>,
    pending_events: Vec<TimEvent>,
    pending_tx: Vec<(Pgn, Vec<u8>, Address)>,
    /// Previous tick instant, so the authority watchdog can be advanced by the
    /// elapsed time rather than by tick count.
    last_tick: Option<Instant>,
    /// ISO 11783-9 §4.7 safe-mode guard.
    safe_mode: TecuSafeMode,
}

impl Tim {
    /// Create with the given local authority guard.
    #[must_use]
    pub fn new(authority: TimAuthority) -> Self {
        Self {
            authority,
            front_pto: None,
            rear_pto: None,
            front_hitch: None,
            rear_hitch: None,
            last_aux: None,
            pending_events: Vec::new(),
            pending_tx: Vec::new(),
            last_tick: None,
            safe_mode: TecuSafeMode::new(),
        }
    }

    /// Enter ISO 11783-9 §4.7 safe mode. Engage commands are refused until it
    /// is explicitly cleared; stops always pass.
    pub const fn enter_safe_mode(&mut self, trigger: SafeModeTrigger) {
        self.safe_mode.enter(trigger);
    }

    /// Leave safe mode — the operator override of §4.7. Deliberately explicit.
    pub const fn clear_safe_mode(&mut self) {
        self.safe_mode.clear();
    }

    /// Whether safe mode is currently blocking engage commands.
    #[must_use]
    pub const fn is_safe_mode_active(&self) -> bool {
        self.safe_mode.is_active()
    }

    /// Current local authority/interlock guard.
    #[must_use]
    pub fn authority(&self) -> TimAuthority {
        self.authority
    }

    /// Request authority over a set of TIM options.
    ///
    /// # Errors
    /// Propagates authority-guard validation errors.
    pub fn request_authority(&mut self, requested: TimOptionSet) -> Result<()> {
        self.authority.request(requested).map_err(tim_error)?;
        self.push_state();
        Ok(())
    }

    /// Grant the requested authority if interlocks are clear.
    ///
    /// # Errors
    /// Propagates authority-guard validation errors.
    pub fn grant_authority(&mut self) -> Result<()> {
        self.authority.grant().map_err(tim_error)?;
        self.push_state();
        Ok(())
    }

    /// Deny the current authority request.
    pub fn deny_authority(&mut self) {
        self.authority.deny();
        self.push_state();
    }

    /// Revoke granted authority.
    pub fn revoke_authority(&mut self) {
        self.authority.revoke();
        self.push_state();
    }

    /// Update local TIM safety interlocks.
    pub fn set_interlocks(&mut self, interlocks: TimInterlocks) {
        let before = self.authority.state();
        self.authority.set_interlocks(interlocks);
        if self.authority.state() != before {
            self.push_state();
        }
    }

    /// Send a guarded hitch-position command (broadcast).
    ///
    /// # Errors
    /// Returns an error (and emits `CommandBlocked`) if authority/interlocks
    /// disallow the command.
    pub fn command_hitch_position(
        &mut self,
        hitch: Hitch,
        target_position: u16,
        rate: u8,
    ) -> Result<()> {
        let guard = match hitch {
            Hitch::Front => TimCommand::FrontHitchPosition,
            Hitch::Rear => TimCommand::RearHitchPosition,
        };
        let msg = HitchCommandMsg {
            command: HitchCommand::Position,
            target_position,
            rate,
        };
        self.guarded(guard, hitch.cmd_pgn(), msg.encode().to_vec())
    }

    /// Send a guarded PTO engagement command.
    ///
    /// # Errors
    /// As [`Self::command_hitch_position`].
    pub fn command_pto_engage(&mut self, pto: Pto, cw_direction: bool) -> Result<()> {
        let guard = match (pto, cw_direction) {
            (Pto::Front, false) => TimCommand::FrontPtoEngageCcw,
            (Pto::Front, true) => TimCommand::FrontPtoEngageCw,
            (Pto::Rear, false) => TimCommand::RearPtoEngageCcw,
            (Pto::Rear, true) => TimCommand::RearPtoEngageCw,
        };
        let msg = PtoCommandMsg {
            command: PtoCommand::Engage,
            target_speed_rpm: 0xFFFF,
            ramp_rate: 0xFF,
        };
        self.guarded(guard, pto.cmd_pgn(), msg.encode().to_vec())
    }

    /// Send a guarded PTO disengagement command.
    ///
    /// # Errors
    /// As [`Self::command_hitch_position`].
    pub fn command_pto_disengage(&mut self, pto: Pto) -> Result<()> {
        let guard = match pto {
            Pto::Front => TimCommand::FrontPtoDisengage,
            Pto::Rear => TimCommand::RearPtoDisengage,
        };
        let msg = PtoCommandMsg {
            command: PtoCommand::Disengage,
            target_speed_rpm: 0xFFFF,
            ramp_rate: 0xFF,
        };
        self.guarded(guard, pto.cmd_pgn(), msg.encode().to_vec())
    }

    /// Broadcast a PTO status frame (no authority guard).
    pub fn broadcast_pto_status(&mut self, pto: Pto, status: PtoState) {
        self.pending_tx.push((
            pto_status_pgn(pto),
            status.encode().to_vec(),
            BROADCAST_ADDRESS,
        ));
    }

    /// Broadcast a hitch status frame.
    ///
    /// # Errors
    /// Propagates hitch-status encode errors.
    pub fn broadcast_hitch_status(&mut self, hitch: Hitch, status: HitchState) -> Result<()> {
        let bytes = status.try_encode().map_err(tim_error)?;
        self.pending_tx
            .push((hitch_status_pgn(hitch), bytes.to_vec(), BROADCAST_ADDRESS));
        Ok(())
    }

    /// Most recent front PTO status observed.
    #[must_use]
    pub fn last_front_pto_status(&self) -> Option<PtoState> {
        self.front_pto
    }

    /// Most recent rear hitch status observed.
    #[must_use]
    pub fn last_rear_hitch_status(&self) -> Option<HitchState> {
        self.rear_hitch
    }

    /// Most recent aux-valve command observed.
    #[must_use]
    pub fn last_aux_valve(&self) -> Option<AuxValveCommand> {
        self.last_aux
    }

    fn guarded(&mut self, guard: TimCommand, pgn: Pgn, bytes: Vec<u8>) -> Result<()> {
        // ISO 11783-9 §4.7: no unexpected start, but always allow a stop.
        // `TecuSafeMode` implemented exactly this and was consulted nowhere —
        // the third of three safety abstractions the crate had that could not
        // actually stop anything.
        if !self.safe_mode.allows(guard.safe_mode_kind()) {
            let error = TimValidationError::SafeModeActive;
            self.pending_events.push(TimEvent::CommandBlocked {
                command: guard,
                error,
            });
            return Err(tim_error(error));
        }
        match self.authority.ensure_command(guard) {
            Ok(()) => {
                self.pending_tx.push((pgn, bytes, BROADCAST_ADDRESS));
                Ok(())
            }
            Err(error) => {
                self.pending_events.push(TimEvent::CommandBlocked {
                    command: guard,
                    error,
                });
                Err(tim_error(error))
            }
        }
    }

    fn push_state(&mut self) {
        self.pending_events
            .push(TimEvent::AuthorityStateChanged(self.authority.state()));
    }

    fn drain_pending(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.pending_events.drain(..) {
            ctx.emit(Event::Tim(event));
        }
        for (pgn, data, dst) in self.pending_tx.drain(..) {
            ctx.send(pgn, data, dst, Priority::Default);
        }
    }
}

impl Plugin for Tim {
    fn name(&self) -> &'static str {
        "tim"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        let pgn = msg.pgn;
        if !msg.has_usable_envelope_for_pgn(pgn) {
            return;
        }
        // Any well-formed frame from a subscribed TIM PGN is evidence the peer
        // is still there. `TimAuthority::keepalive` existed with no caller
        // anywhere in the crate, so the AEF communication watchdog revoked
        // every grant 300 ms after it was made: guarded hitch and PTO commands
        // stopped reaching the bus while the implement held its last commanded
        // position, with nothing to say why.
        self.authority.keepalive();
        match pgn {
            PGN_FRONT_PTO | PGN_REAR_PTO => {
                if let Some(state) = PtoState::decode(msg) {
                    let pto = if pgn == PGN_FRONT_PTO {
                        Pto::Front
                    } else {
                        Pto::Rear
                    };
                    if pto == Pto::Front {
                        self.front_pto = Some(state);
                    } else {
                        self.rear_pto = Some(state);
                    }
                    ctx.emit(Event::Tim(TimEvent::PtoStatus {
                        pto,
                        source: msg.source,
                        state,
                    }));
                }
            }
            PGN_FRONT_HITCH | PGN_REAR_HITCH => {
                if let Some(state) = HitchState::decode(msg) {
                    let hitch = if pgn == PGN_FRONT_HITCH {
                        Hitch::Front
                    } else {
                        Hitch::Rear
                    };
                    if hitch == Hitch::Front {
                        self.front_hitch = Some(state);
                    } else {
                        self.rear_hitch = Some(state);
                    }
                    ctx.emit(Event::Tim(TimEvent::HitchStatus {
                        hitch,
                        source: msg.source,
                        state,
                    }));
                }
            }
            PGN_FRONT_PTO_CMD | PGN_REAR_PTO_CMD => {
                if let Some(decoded) = PtoCommandMsg::decode(&msg.data) {
                    let pto = if pgn == PGN_FRONT_PTO_CMD {
                        Pto::Front
                    } else {
                        Pto::Rear
                    };
                    ctx.emit(Event::Tim(TimEvent::PtoCommand {
                        pto,
                        source: msg.source,
                        msg: decoded,
                    }));
                }
            }
            PGN_FRONT_HITCH_CMD | PGN_REAR_HITCH_CMD => {
                if let Some(decoded) = HitchCommandMsg::decode(&msg.data) {
                    let hitch = if pgn == PGN_FRONT_HITCH_CMD {
                        Hitch::Front
                    } else {
                        Hitch::Rear
                    };
                    ctx.emit(Event::Tim(TimEvent::HitchCommand {
                        hitch,
                        source: msg.source,
                        msg: decoded,
                    }));
                }
            }
            PGN_AUX_VALVE_0_7 | PGN_AUX_VALVE_8_15 | PGN_AUX_VALVE_16_23 | PGN_AUX_VALVE_24_31 => {
                if let Some(command) = AuxValveCommand::decode(msg) {
                    self.last_aux = Some(command);
                    ctx.emit(Event::Tim(TimEvent::AuxValveCommand {
                        source: msg.source,
                        command,
                    }));
                }
            }
            _ => {}
        }
        self.drain_pending(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();

        // Advance the loss-of-communication watchdog. `TimAuthority` has always
        // had one, with a documented AEF safe-stop revoke — but nothing called
        // `tick`, so a granted authority survived indefinitely after the peer
        // stopped sending keepalives.
        let elapsed = crate::time::advance_millis(&mut self.last_tick, now);
        if elapsed > 0 && self.authority.tick(elapsed) {
            self.pending_events
                .push(TimEvent::AuthorityStateChanged(self.authority.state()));
        }

        self.drain_pending(ctx);
        Some(now.add_millis(u64::from(TIM_UPDATE_INTERVAL_MS)))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

const fn pto_status_pgn(pto: Pto) -> Pgn {
    match pto {
        Pto::Front => PGN_FRONT_PTO,
        Pto::Rear => PGN_REAR_PTO,
    }
}

const fn hitch_status_pgn(hitch: Hitch) -> Pgn {
    match hitch {
        Hitch::Front => PGN_FRONT_HITCH,
        Hitch::Rear => PGN_REAR_HITCH,
    }
}

fn tim_error(err: TimValidationError) -> Error {
    Error::invalid_state(format!(
        "TIM authority/interlock guard rejected command: {err:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::tim::{TimAuthorityState, TimOption};
    use crate::net::{BROADCAST_ADDRESS, Frame, Identifier, Name, Priority};
    use crate::session::Session;

    fn node() -> Session {
        let name = Name::default()
            .with_identity_number(0x77)
            .with_function_code(0x80)
            .with_self_configurable(true);
        let mut s = Session::builder(name, 0x80)
            .plug(Tim::new(TimAuthority::new(TimOptionSet::from_options(&[
                TimOption::FrontPtoEngagementCwIsSupported,
                TimOption::FrontPtoDisengagementIsSupported,
            ]))))
            .build()
            .unwrap();
        s.start().unwrap();
        let mut now = Instant::ZERO;
        for _ in 0..40 {
            now = now.add_millis(100);
            s.tick(now);
            while s.poll_transmit().is_some() {}
            if s.is_claimed() {
                break;
            }
        }
        s
    }

    fn feed_pto_status(s: &mut Session, at: Instant) {
        let frame = Frame::new(
            Identifier::encode(Priority::Default, PGN_FRONT_PTO, 0xF0, BROADCAST_ADDRESS),
            [0xFFu8; 8],
            8,
        );
        s.feed(0, &frame, at);
    }

    /// H59 — `TecuSafeMode` implemented the ISO 11783-9 §4.7 obligations ("no
    /// unexpected start", "must allow stop") in full and was consulted by
    /// nothing outside its own unit test. Wiring it into the guarded-command
    /// path is what makes it able to stop anything.
    #[test]
    fn safe_mode_blocks_engagement_but_never_a_stop() {
        let mut s = node();
        let now = Instant::from_millis(10_000);
        s.tick(now);
        feed_pto_status(&mut s, now);
        s.get_mut::<Tim>()
            .unwrap()
            .set_interlocks(TimInterlocks::all_clear());
        s.get_mut::<Tim>()
            .unwrap()
            .request_authority(TimOptionSet::from_options(&[
                TimOption::FrontPtoEngagementCwIsSupported,
                TimOption::FrontPtoDisengagementIsSupported,
            ]))
            .unwrap();
        s.get_mut::<Tim>().unwrap().grant_authority().unwrap();

        // Engaging is allowed while safe mode is clear.
        assert!(
            s.get_mut::<Tim>()
                .unwrap()
                .command_pto_engage(Pto::Front, true)
                .is_ok()
        );

        s.get_mut::<Tim>()
            .unwrap()
            .enter_safe_mode(SafeModeTrigger::TecuCommLoss);
        assert!(s.get::<Tim>().unwrap().is_safe_mode_active());

        assert!(
            s.get_mut::<Tim>()
                .unwrap()
                .command_pto_engage(Pto::Front, true)
                .is_err(),
            "safe mode must refuse an unexpected start"
        );
        assert!(
            s.get_mut::<Tim>()
                .unwrap()
                .command_pto_disengage(Pto::Front)
                .is_ok(),
            "a stop is never refused, whatever the safe-mode state"
        );

        // The exit is explicit, per the operator-override obligation.
        s.get_mut::<Tim>().unwrap().clear_safe_mode();
        assert!(
            s.get_mut::<Tim>()
                .unwrap()
                .command_pto_engage(Pto::Front, true)
                .is_ok()
        );
    }

    /// C25 — `TimAuthority::keepalive` had no caller anywhere in the crate, so
    /// the AEF communication watchdog revoked every grant 300 ms after it was
    /// made. Guarded hitch and PTO commands stopped reaching the bus while the
    /// implement held its last commanded position, with nothing to say why.
    #[test]
    fn a_talking_peer_keeps_the_tim_authority_alive() {
        let mut s = node();
        let mut now = Instant::from_millis(10_000);
        // Sync the plugin's tick baseline before granting, so the watchdog is
        // not carrying the idle gap left by the address-claim phase.
        s.tick(now);
        feed_pto_status(&mut s, now);

        s.get_mut::<Tim>()
            .unwrap()
            .set_interlocks(TimInterlocks::all_clear());
        s.get_mut::<Tim>()
            .unwrap()
            .request_authority(TimOptionSet::from_options(&[
                TimOption::FrontPtoEngagementCwIsSupported,
            ]))
            .unwrap();
        s.get_mut::<Tim>().unwrap().grant_authority().unwrap();
        assert_eq!(
            s.get::<Tim>().unwrap().authority().state(),
            TimAuthorityState::Granted
        );

        // The peer keeps broadcasting well past the 300 ms window.
        for _ in 0..20 {
            now = now.add_millis(100);
            feed_pto_status(&mut s, now);
            s.tick(now);
        }
        assert_eq!(
            s.get::<Tim>().unwrap().authority().state(),
            TimAuthorityState::Granted,
            "a peer that is still talking must not have its authority revoked"
        );

        // It goes quiet: the watchdog must then revoke.
        for _ in 0..8 {
            now = now.add_millis(100);
            s.tick(now);
        }
        assert_ne!(
            s.get::<Tim>().unwrap().authority().state(),
            TimAuthorityState::Granted,
            "loss of communication still revokes the grant"
        );
    }
}
