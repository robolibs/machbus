//! Combined autonomous driving — steering and speed behind one lifecycle.
//!
//! ISOBUS splits autonomy across two unrelated message families: the steering
//! curvature (ISO 11783-7 Guidance System Command, PGN 0xAD00) and the machine
//! speed command (PGN 0xFD43). Driving them as two independent plugins means
//! there is no shared engage lifecycle, no shared safety state, and no single
//! place that can refuse a command — so an application could be steering while
//! its speed authority had been revoked, or keep commanding after the machine
//! had told it to stop.
//!
//! [`AutoDrive`] is that single place. It owns:
//!
//! - **one setpoint** for both axes ([`DriveCommand`]), last-value-wins;
//! - **one cadence**, so commands are a heartbeat (min 100 ms, max 2000 ms) and
//!   never a flood;
//! - **one stop latch** fed by every failure the session can observe — link
//!   timeout, ISB, heartbeat error, bus-off, address-claim loss;
//! - **one set of preconditions**, checked before anything reaches the bus.
//!
//! Turning a path and a GNSS pose into a curvature is still the application's
//! job; [`crate::geo::guidance`] has the geometry for it.

use crate::isobus::implement::Signal;
use crate::isobus::implement::guidance::{
    GenericSaeBs02SlotValue, GuidanceMachineInfo, MechanicalLockout, curvature_within_range,
};
use crate::isobus::implement::{
    CurvatureCommandStatus, GuidanceSystemCmd, MachineDirection, MachineSpeedCommandMsg,
};
use crate::j1939::shortcut_button::decode_message;
use crate::net::pgn_defs::{
    PGN_GUIDANCE_MACHINE_INFO, PGN_GUIDANCE_SYSTEM_CMD, PGN_MACHINE_SELECTED_SPEED_CMD,
    PGN_SHORTCUT_BUTTON,
};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::plugins::gnss::GnssHazards;
use crate::session::plugins::shortcut_button::IsbGuard;
use crate::session::sys::{
    AutodriveEvent, AutodriveRefusal, AutomationStatus, BusEvent, DriveCommand, Event,
    SafeStopTrigger, StopLatch,
};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_GUIDANCE_MACHINE_INFO, PGN_SHORTCUT_BUTTON];

/// The PGNs this controller commands the machine with. A refused send on either
/// means the command never reached the bus.
const COMMAND_PGNS: &[Pgn] = &[PGN_GUIDANCE_SYSTEM_CMD, PGN_MACHINE_SELECTED_SPEED_CMD];

/// Three missed 100 ms Machine Info broadcasts (AEF 023 loss of communication).
pub const LINK_TIMEOUT_MS: u32 = 300;
/// ISO 11783-7 §5.2.7.2 minimum update period for the guidance group.
pub const MIN_TX_INTERVAL_MS: u32 = 100;
/// The command is a heartbeat, so it keeps going out even when unchanged.
pub const MAX_TX_INTERVAL_MS: u32 = 2000;
/// Below this, a yaw rate does not define a forward path curvature.
pub const DEFAULT_MIN_SPEED_MPS: f64 = 0.05;
/// How long an engaged controller will re-transmit an unrefreshed setpoint
/// before treating the commanding application as dead.
///
/// The cadence made the command a heartbeat, which turned a fail-silent path
/// into a fail-active one: an application that stopped commanding used to
/// produce no frames and let the steering ECU time out, and instead kept the
/// machine steering at the last curvature indefinitely. Three command periods,
/// matching [`LINK_TIMEOUT_MS`].
pub const COMMAND_STALE_MS: u32 = 300;

/// Unified autonomous-driving controller. See the [module docs](self).
///
/// The stops this plugin can trip are not listed here: a hand-maintained list
/// has no compiler relationship to the `trip(...)` call sites below, so it went
/// stale silently. `g8_every_trigger_is_reachable` scans this file instead.
pub struct AutoDrive {
    status: AutomationStatus,
    setpoint: DriveCommand,
    latest: Option<GuidanceMachineInfo>,
    last_info_at: Option<Instant>,
    link_alive: bool,
    last_tx_at: Option<Instant>,
    dirty: bool,
    stop: StopLatch,
    min_speed_mps: f64,
    min_tx_ms: u32,
    max_tx_ms: u32,
    /// Bumped by every application command. The setters have no clock, so
    /// freshness is timed in `on_tick` by watching this change.
    command_seq: u64,
    seen_seq: u64,
    seq_changed_at: Option<Instant>,
    stale_ms: u32,
    /// Last Auxiliary Shortcut Button report and when it arrived. The button
    /// was handled only as a latch edge, so its *held* state was unknown:
    /// `clear_stop()` could re-engage while the operator still held stop, and
    /// an ISB transmitter that went silent read as permission rather than as
    /// the loss of stop authority it is.
    isb: IsbGuard,
    /// Live GNSS hazards, so `clear_stop()` cannot re-arm autonomy against a
    /// receiver that is still stale or a fix that still cannot be steered on.
    gnss: GnssHazards,
}

impl Default for AutoDrive {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoDrive {
    /// A controller that starts disarmed, disengaged and commanding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: AutomationStatus::NotReady,
            setpoint: DriveCommand::default(),
            latest: None,
            last_info_at: None,
            link_alive: false,
            last_tx_at: None,
            dirty: false,
            stop: StopLatch::new(),
            min_speed_mps: DEFAULT_MIN_SPEED_MPS,
            min_tx_ms: MIN_TX_INTERVAL_MS,
            max_tx_ms: MAX_TX_INTERVAL_MS,
            command_seq: 0,
            seen_seq: 0,
            seq_changed_at: None,
            stale_ms: COMMAND_STALE_MS,
            isb: IsbGuard::new(),
            gnss: GnssHazards::default(),
        }
    }

    /// Override how long an unrefreshed setpoint is re-transmitted before the
    /// controller stops. Zero disables the watchdog, which is only appropriate
    /// when something else guarantees liveness.
    #[must_use]
    pub const fn with_command_stale_ms(mut self, ms: u32) -> Self {
        self.stale_ms = ms;
        self
    }

    /// Override the transmit cadence. The minimum is a conformance limit, so
    /// raising it is safe and lowering it is not — values below
    /// [`MIN_TX_INTERVAL_MS`] are clamped.
    #[must_use]
    pub fn with_cadence(mut self, min_ms: u32, max_ms: u32) -> Self {
        self.min_tx_ms = min_ms.max(MIN_TX_INTERVAL_MS);
        self.max_tx_ms = max_ms.max(self.min_tx_ms);
        self
    }

    /// Override the speed below which curvature is not commanded.
    #[must_use]
    pub fn with_min_speed(mut self, mps: f64) -> Self {
        self.min_speed_mps = mps.abs();
        self
    }

    /// Current automation status.
    #[must_use]
    pub const fn status(&self) -> AutomationStatus {
        self.status
    }

    /// `true` when actively commanding the machine.
    #[must_use]
    pub const fn is_engaged(&self) -> bool {
        self.status.is_active()
    }

    /// `true` when a steering ECU is broadcasting within the link timeout.
    #[must_use]
    pub const fn is_link_alive(&self) -> bool {
        self.link_alive
    }

    /// Why the controller stopped, if it did.
    #[must_use]
    pub const fn stop_reason(&self) -> Option<SafeStopTrigger> {
        self.stop.reason()
    }

    /// The steering ECU's last report and its age, or `None` if none arrived.
    #[must_use]
    pub fn machine_info(&self, now: Instant) -> Option<(GuidanceMachineInfo, u32)> {
        let info = self.latest?;
        let age = self.last_info_at.map_or(u32::MAX, |t| now.millis_since(t));
        Some((info, age))
    }

    /// Move to *ready to enable*: the machine is answering and nothing is
    /// blocking, but no setpoint is being commanded yet.
    ///
    /// # Errors
    /// The first unmet precondition.
    pub fn arm(&mut self) -> Result<(), AutodriveRefusal> {
        self.check_preconditions()?;
        self.set_status(AutomationStatus::ReadyToEnable);
        Ok(())
    }

    /// Begin commanding. The setpoint reaches the bus on the next tick.
    ///
    /// # Errors
    /// The first unmet precondition — a latched stop, a dead link, a mechanical
    /// lockout, or an inactive operator engage switch.
    pub fn engage(&mut self) -> Result<(), AutodriveRefusal> {
        self.check_preconditions()?;
        self.set_status(AutomationStatus::ActiveNotLimited);
        self.dirty = true;
        self.command_seq = self.command_seq.wrapping_add(1);
        Ok(())
    }

    /// Stop commanding and fall back to the safe state. Idempotent and
    /// infallible: a disengage must never be refused.
    pub fn disengage(&mut self, reason: SafeStopTrigger) {
        self.stop.trip(reason);
        self.enter_safe_state();
    }

    /// Replace the setpoint. Last value wins — commanding faster than the
    /// cadence updates the target rather than queueing another frame.
    ///
    /// # Errors
    /// Refuses while stopped, disengaged, or below the minimum speed for a
    /// curvature command.
    pub fn command(&mut self, cmd: DriveCommand) -> Result<(), AutodriveRefusal> {
        if self.stop.is_latched() {
            return Err(AutodriveRefusal::StopLatched);
        }
        if !self.status.is_active() {
            return Err(AutodriveRefusal::StatusNotActive);
        }
        // The codec clamps out-of-range curvature to the SLOT limit, which is
        // full lock. Refusing here keeps the wire encoder from being the only
        // range check on a steering command (G7).
        if let Some(curvature) = cmd.curvature_km_inv
            && !curvature_within_range(curvature)
        {
            return Err(AutodriveRefusal::CurvatureOutOfRange);
        }
        if let Some(speed) = cmd.speed_mps
            && !speed.is_finite()
        {
            return Err(AutodriveRefusal::SpeedNotFinite);
        }
        if let (Some(speed), Some(curvature)) = (cmd.speed_mps, cmd.curvature_km_inv)
            && curvature != 0.0
            && speed.abs() <= self.min_speed_mps
        {
            return Err(AutodriveRefusal::SpeedBelowMinimum);
        }

        if self.setpoint != cmd {
            self.dirty = true;
        }
        self.setpoint = cmd;
        self.command_seq = self.command_seq.wrapping_add(1);
        Ok(())
    }

    /// Release a latched stop. Deliberately explicit — the fault clearing is
    /// not by itself consent to move.
    ///
    /// Refused while the operator is still asserting stop on the Auxiliary
    /// Shortcut Button: clearing there produced a window of commanded motion
    /// against a stop that was being held down.
    pub fn clear_stop(&mut self) -> Result<(), AutodriveRefusal> {
        // G7 — the refusal has to be reportable. Clearing while the operator is
        // still on the shortcut button, or while a GNSS hazard is live, was a
        // silent no-op: an HMI showed the fault cleared and re-enabled Engage
        // with the latch still set.
        if self.isb.is_asserted() || self.gnss.is_live() {
            return Err(AutodriveRefusal::StopConditionLive);
        }
        self.stop.clear();
        if self.status == AutomationStatus::Fault {
            self.status = AutomationStatus::NotReady;
        }
        Ok(())
    }

    /// `true` while the operator is commanding stop on the Auxiliary Shortcut
    /// Button, or a previously-seen ISB source has gone silent.
    #[must_use]
    pub const fn is_isb_stop_asserted(&self) -> bool {
        self.isb.is_asserted()
    }

    fn check_preconditions(&self) -> Result<(), AutodriveRefusal> {
        if self.stop.is_latched() || self.isb.is_asserted() || self.gnss.is_live() {
            return Err(AutodriveRefusal::StopLatched);
        }
        if !self.link_alive {
            return Err(AutodriveRefusal::LinkDown);
        }
        let info = self.latest.ok_or(AutodriveRefusal::LinkDown)?;
        self.machine_violation(info).map_or(Ok(()), Err)
    }

    /// The machine-reported conditions that forbid steering. Split out so they
    /// can be re-checked on every Machine Info broadcast: checking them once at
    /// engage meant the operator could drop the engage switch or assert the
    /// mechanical lockout mid-drive and this node kept asking for the wheel.
    const fn machine_violation(&self, info: GuidanceMachineInfo) -> Option<AutodriveRefusal> {
        if matches!(info.lockout, MechanicalLockout::Active) {
            return Some(AutodriveRefusal::MechanicalLockout);
        }
        if matches!(
            info.remote_engage_switch_status,
            GenericSaeBs02SlotValue::DisabledOffPassive
        ) {
            return Some(AutodriveRefusal::OperatorNotEngaged);
        }
        None
    }

    fn set_status(&mut self, status: AutomationStatus) {
        if self.status != status {
            self.status = status;
            self.dirty = true;
        }
    }

    fn enter_safe_state(&mut self) {
        self.status = AutomationStatus::Fault;
        self.setpoint = DriveCommand::halt();
        self.dirty = true;
    }

    fn system_command(&self) -> GuidanceSystemCmd {
        GuidanceSystemCmd {
            commanded_curvature: self
                .setpoint
                .curvature_km_inv
                .map_or(Signal::NotAvailable, Signal::Value),
            status: if self.status.is_active() {
                CurvatureCommandStatus::IntendedToSteer
            } else {
                CurvatureCommandStatus::NotIntendedToSteer
            },
        }
    }
}

impl Plugin for AutoDrive {
    fn name(&self) -> &'static str {
        "autodrive"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn transmits(&self) -> &'static [Pgn] {
        COMMAND_PGNS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if msg.pgn == PGN_SHORTCUT_BUTTON
            && let Some(decoded) = decode_message(msg)
        {
            if self.isb.observe(decoded.state, ctx.now())
                && self.stop.trip(SafeStopTrigger::IsbStop)
            {
                self.enter_safe_state();
                ctx.emit(Event::Autodrive(AutodriveEvent::SafeStop {
                    trigger: SafeStopTrigger::IsbStop,
                }));
            }
            return;
        }

        if msg.pgn == PGN_GUIDANCE_MACHINE_INFO
            && let Some(info) = GuidanceMachineInfo::decode(&msg.data)
        {
            self.latest = Some(info);
            self.last_info_at = Some(ctx.now());
            self.link_alive = true;

            // Preconditions are a continuing contract, not an entry check: the
            // operator dropping the engage switch or asserting the mechanical
            // lockout mid-drive must stop this node asking for the wheel.
            if self.status.is_active()
                && let Some(refusal) = self.machine_violation(info)
            {
                let trigger = match refusal {
                    AutodriveRefusal::MechanicalLockout => SafeStopTrigger::IsbStop,
                    _ => SafeStopTrigger::OperatorOverride,
                };
                if self.stop.trip(trigger) {
                    self.enter_safe_state();
                    ctx.emit(Event::Autodrive(AutodriveEvent::SafeStop { trigger }));
                }
                return;
            }

            // The machine's own limit status is the anti-windup signal an outer
            // loop needs, so mirror it into the automation status rather than
            // discarding it as the guidance plugin used to.
            if self.status.is_active() {
                use crate::isobus::implement::guidance::GuidanceLimitStatus as Limit;
                match info.guidance_limit_status {
                    // The operator is limiting or has taken control, and a
                    // non-recoverable fault is a fault. Both are the machine
                    // telling this node to stop asking for the wheel. Status 1
                    // used to fall through to "not limited", so an operator
                    // intervention read as normal operation.
                    Limit::OperatorLimitedControlled | Limit::NonRecoverableFault => {
                        let trigger = SafeStopTrigger::OperatorOverride;
                        if self.stop.trip(trigger) {
                            self.enter_safe_state();
                            ctx.emit(Event::Autodrive(AutodriveEvent::SafeStop { trigger }));
                        }
                    }
                    // Anti-windup information for an outer loop.
                    Limit::LimitedHigh | Limit::LimitedLow | Limit::NotLimited => {
                        let next = match info.guidance_limit_status {
                            Limit::LimitedHigh => AutomationStatus::ActiveLimitedHigh,
                            Limit::LimitedLow => AutomationStatus::ActiveLimitedLow,
                            _ => AutomationStatus::ActiveNotLimited,
                        };
                        if next != self.status {
                            self.status = next;
                            ctx.emit(Event::Autodrive(AutodriveEvent::StateChanged {
                                status: next,
                            }));
                        }
                    }
                    // Reserved and not-available say nothing about the limit, so
                    // claiming "not limited" would be inventing a reading.
                    Limit::Reserved1 | Limit::Reserved2 | Limit::NotAvailable => {}
                }
            }
        }
    }

    /// React to a session-observed fault. Without this the stop latch was fed
    /// only by what arrives on the two PGNs above, so bus-off, a lost address
    /// claim, a heartbeat fault and a refused command could all be detected by
    /// the session and never stop the machine.
    fn on_event(&mut self, event: &Event, ctx: &mut PluginCtx<'_>) {
        self.gnss.observe(event);
        let trigger = SafeStopTrigger::from_event(event).or_else(|| match event {
            Event::Bus(BusEvent::SendFailed { pgn, .. }) if COMMAND_PGNS.contains(pgn) => {
                Some(SafeStopTrigger::SendFailed(*pgn))
            }
            _ => None,
        });
        let Some(trigger) = trigger else {
            return;
        };
        if self.stop.trip(trigger) {
            self.enter_safe_state();
            ctx.emit(Event::Autodrive(AutodriveEvent::SafeStop { trigger }));
        }
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();

        let alive = self
            .last_info_at
            .is_some_and(|t| now.millis_since(t) < LINK_TIMEOUT_MS);
        if self.link_alive && !alive {
            self.stop.trip(SafeStopTrigger::GuidanceLinkTimeout);
            self.enter_safe_state();
            ctx.emit(Event::Autodrive(AutodriveEvent::SafeStop {
                trigger: SafeStopTrigger::GuidanceLinkTimeout,
            }));
        }
        self.link_alive = alive;

        if self.isb.tick(now) && self.stop.trip(SafeStopTrigger::IsbStop) {
            self.enter_safe_state();
            ctx.emit(Event::Autodrive(AutodriveEvent::SafeStop {
                trigger: SafeStopTrigger::IsbStop,
            }));
        }

        // A live application refreshes its setpoint; one that has died does not.
        // Without this the cadence re-transmits "intended to steer" at the last
        // curvature forever, which is unintended motion with no bound in time.
        if self.command_seq != self.seen_seq {
            self.seen_seq = self.command_seq;
            self.seq_changed_at = Some(now);
        }
        if self.stale_ms > 0
            && self.status.is_active()
            && self
                .seq_changed_at
                .is_some_and(|t| now.millis_since(t) >= self.stale_ms)
            && self.stop.trip(SafeStopTrigger::CommandStale)
        {
            self.enter_safe_state();
            ctx.emit(Event::Autodrive(AutodriveEvent::SafeStop {
                trigger: SafeStopTrigger::CommandStale,
            }));
        }

        // Nothing queued before the claim completes reaches the bus, and the
        // cadence used to advance anyway — which both hid the refusal and
        // delayed the first real command by up to `max_tx_ms`.
        if !ctx.is_claimed() {
            return Some(now.add_millis(u64::from(self.min_tx_ms)));
        }

        let due = match self.last_tx_at {
            None => true,
            Some(last) => {
                let since = now.millis_since(last);
                if self.status.is_active() {
                    // While steering, the command *is* the heartbeat the ECU
                    // times out on — see the guidance plugin for the same rule.
                    since >= self.min_tx_ms
                } else {
                    since >= self.max_tx_ms || (self.dirty && since >= self.min_tx_ms)
                }
            }
        };

        if due {
            ctx.send(
                PGN_GUIDANCE_SYSTEM_CMD,
                self.system_command().encode().to_vec(),
                BROADCAST_ADDRESS,
                Priority::Normal,
            );
            if let Some(speed_mps) = self.setpoint.speed_mps {
                let direction = if speed_mps < 0.0 {
                    MachineDirection::Reverse
                } else {
                    MachineDirection::Forward
                };
                let speed = MachineSpeedCommandMsg::default()
                    .with_speed_mps(speed_mps.abs())
                    .with_direction(direction);
                ctx.send(
                    PGN_MACHINE_SELECTED_SPEED_CMD,
                    speed.encode().to_vec(),
                    BROADCAST_ADDRESS,
                    Priority::Normal,
                );
            }
            self.last_tx_at = Some(now);
            self.dirty = false;
        }

        Some(
            self.last_tx_at
                .map_or(now, |last| last.add_millis(u64::from(self.min_tx_ms))),
        )
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::j1939::shortcut_button::ShortcutButtonState;
    use crate::net::{Frame, Identifier, Name};
    use crate::session::Session;

    /// Real captured Machine Info: lockout not active, engage switch not
    /// actively disabled, limit status not-available.
    const MACHINE_INFO: [u8; 8] = [0x64, 0x7D, 0x3C, 0xFF, 0xC0, 0xFF, 0xFF, 0xFF];

    fn node() -> Session {
        let name = Name::default()
            .with_identity_number(0x99)
            .with_function_code(0x80)
            .with_self_configurable(true);
        let mut s = Session::builder(name, 0x80)
            .plug(AutoDrive::new())
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

    fn feed_info(s: &mut Session, limit_status: u8, at: Instant) {
        let mut data = MACHINE_INFO;
        data[3] = (data[3] & 0x1F) | (limit_status << 5);
        let frame = Frame::new(
            Identifier::encode(
                Priority::Default,
                PGN_GUIDANCE_MACHINE_INFO,
                0xF0,
                BROADCAST_ADDRESS,
            ),
            data,
            8,
        );
        s.feed(0, &frame, at);
    }

    fn last_command(s: &mut Session) -> Option<GuidanceSystemCmd> {
        let mut cmd = None;
        while let Some((_, frame)) = s.poll_transmit() {
            if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                cmd = GuidanceSystemCmd::decode(&frame.data);
            }
        }
        cmd
    }

    /// A3.6 — the full lifecycle in one place. No test previously exercised
    /// guidance and speed together, or the engage → steer → drop-out sequence
    /// end to end.
    /// C3 — the GNSS antenna cable is cut mid-field. `plugins::gnss` emits
    /// `PositionStale` **once**, AutoDrive latches and halts, and the operator
    /// presses the UI "clear stop". Clearing there re-armed autonomy with no
    /// fix, and because the trigger is edge-emitted it could never fire again:
    /// one clear permanently disarmed the GNSS safety net with no indication.
    ///
    /// ISO 11783-9 §4.7.2 — "The implement shall not start unexpectedly."
    #[test]
    fn clear_stop_is_refused_while_a_gnss_hazard_is_live() {
        use crate::session::sys::GnssEvent;

        let mut auto = AutoDrive::new();
        let mut hazards = GnssHazards::default();

        // The cable is cut: one PositionStale, and nothing after it.
        let stale = Event::Gnss(GnssEvent::PositionStale {
            silent_for_ms: 1_500,
        });
        assert!(hazards.observe(&stale), "a stale receiver is a live hazard");
        assert!(auto.stop.trip(SafeStopTrigger::PositionStale));
        assert_eq!(auto.stop_reason(), Some(SafeStopTrigger::PositionStale));

        // The operator clears. The hazard has not gone away, so neither does
        // the latch — and no second PositionStale is ever coming.
        //
        // NOTE this only exercises `clear_stop` reading the field. That the
        // field is *populated* by `on_event` is asserted through `Session` by
        // `clear_stop_is_refused_through_the_session_while_the_receiver_is_stale`
        // (G9); without that companion, deleting `self.gnss.observe(event)`
        // kept this test green and silently reverted C3.
        auto.gnss = hazards;
        assert_eq!(auto.clear_stop(), Err(AutodriveRefusal::StopConditionLive));
        assert_eq!(
            auto.stop_reason(),
            Some(SafeStopTrigger::PositionStale),
            "clearing must not re-arm against a receiver that is still stale"
        );

        // A position arriving is what actually resolves it.
        let recovered = Event::Gnss(GnssEvent::Position(Default::default()));
        assert!(!hazards.observe(&recovered));
    }

    /// The same rule for a fix that cannot be steered on, which recovers via
    /// `FixRestored` rather than by a position arriving.
    #[test]
    fn a_degraded_fix_stays_live_until_it_is_restored() {
        use crate::nmea::GNSSFixType;
        use crate::session::sys::GnssEvent;

        let mut hazards = GnssHazards::default();
        assert!(hazards.observe(&Event::Gnss(GnssEvent::FixDegraded {
            fix_type: GNSSFixType::NoFix,
        })));
        // A position still arrives while the fix is unusable, so it must not by
        // itself clear the hazard.
        assert!(hazards.observe(&Event::Gnss(GnssEvent::Position(Default::default()))));
        assert!(!hazards.observe(&Event::Gnss(GnssEvent::FixRestored {
            fix_type: GNSSFixType::RTKFixed,
        })));
    }

    #[test]
    fn arm_engage_steer_then_lose_the_link() {
        let mut s = node();
        let base = 20_000u64;

        // Nothing known about the machine yet: arming is refused.
        assert_eq!(
            s.get_mut::<AutoDrive>().unwrap().arm(),
            Err(AutodriveRefusal::LinkDown)
        );

        feed_info(&mut s, 7, Instant::from_millis(base));
        s.get_mut::<AutoDrive>().unwrap().arm().unwrap();
        assert_eq!(
            s.get::<AutoDrive>().unwrap().status(),
            AutomationStatus::ReadyToEnable
        );

        // Commanding before engaging is refused, not silently accepted.
        assert_eq!(
            s.get_mut::<AutoDrive>()
                .unwrap()
                .command(DriveCommand::steer(20.0)),
            Err(AutodriveRefusal::StatusNotActive)
        );

        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();
        s.get_mut::<AutoDrive>()
            .unwrap()
            .command(DriveCommand {
                speed_mps: Some(2.0),
                curvature_km_inv: Some(20.0),
            })
            .unwrap();
        assert!(s.get::<AutoDrive>().unwrap().is_engaged());

        s.tick(Instant::from_millis(base + 100));
        let mut saw_speed = false;
        let mut steer = None;
        while let Some((_, frame)) = s.poll_transmit() {
            match frame.id.pgn() {
                PGN_GUIDANCE_SYSTEM_CMD => steer = GuidanceSystemCmd::decode(&frame.data),
                PGN_MACHINE_SELECTED_SPEED_CMD => saw_speed = true,
                _ => {}
            }
        }
        let steer = steer.expect("a steering command reaches the bus");
        assert_eq!(steer.status, CurvatureCommandStatus::IntendedToSteer);
        assert!((steer.commanded_curvature.value().unwrap() - 20.0).abs() < 0.25);
        assert!(saw_speed, "both axes travel together");

        // The steering ECU goes silent: one safe state, both axes zeroed.
        while s.poll_event().is_some() {}
        s.tick(Instant::from_millis(
            base + 100 + u64::from(LINK_TIMEOUT_MS) + 10,
        ));

        assert!(!s.get::<AutoDrive>().unwrap().is_engaged());
        assert_eq!(
            s.get::<AutoDrive>().unwrap().stop_reason(),
            Some(SafeStopTrigger::GuidanceLinkTimeout)
        );
        let stopped = std::iter::from_fn(|| s.poll_event()).any(|e| {
            matches!(
                e,
                Event::Autodrive(AutodriveEvent::SafeStop {
                    trigger: SafeStopTrigger::GuidanceLinkTimeout
                })
            )
        });
        assert!(stopped, "the safe stop is reported");

        let cmd = last_command(&mut s).expect("the safe state reaches the bus");
        assert_eq!(cmd.status, CurvatureCommandStatus::NotIntendedToSteer);
        assert_eq!(cmd.commanded_curvature, Signal::Value(0.0));

        // Recovery is explicit: a returning link does not re-engage by itself.
        feed_info(&mut s, 7, Instant::from_millis(base + 600));
        assert_eq!(
            s.get_mut::<AutoDrive>().unwrap().engage(),
            Err(AutodriveRefusal::StopLatched)
        );
        s.get_mut::<AutoDrive>().unwrap().clear_stop().unwrap();
        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();
        assert!(s.get::<AutoDrive>().unwrap().is_engaged());
    }

    /// The machine's limit status is the anti-windup signal an outer loop
    /// needs. It must reach the application as a distinct state, and a
    /// non-recoverable fault must stop the machine rather than read as a limit.
    #[test]
    fn limit_status_surfaces_and_a_fault_stops() {
        let mut s = node();
        let base = 20_000u64;
        feed_info(&mut s, 7, Instant::from_millis(base));
        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();
        while s.poll_event().is_some() {}

        // Limited high while still actively steering.
        feed_info(&mut s, 2, Instant::from_millis(base + 50));
        assert_eq!(
            s.get::<AutoDrive>().unwrap().status(),
            AutomationStatus::ActiveLimitedHigh
        );
        assert!(s.get::<AutoDrive>().unwrap().status().is_active());
        assert!(s.get::<AutoDrive>().unwrap().status().is_limited());

        // Non-recoverable fault is not a limit: it stops.
        feed_info(&mut s, 6, Instant::from_millis(base + 100));
        assert!(!s.get::<AutoDrive>().unwrap().is_engaged());
        assert!(s.get::<AutoDrive>().unwrap().stop_reason().is_some());
    }

    /// A curvature at a standstill is meaningless, and dividing anyway is how
    /// odometry noise became a full-lock command.
    #[test]
    fn curvature_below_the_speed_floor_is_refused() {
        let mut s = node();
        let base = 20_000u64;
        feed_info(&mut s, 7, Instant::from_millis(base));
        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();

        assert_eq!(
            s.get_mut::<AutoDrive>().unwrap().command(DriveCommand {
                speed_mps: Some(1e-6),
                curvature_km_inv: Some(8000.0),
            }),
            Err(AutodriveRefusal::SpeedBelowMinimum)
        );

        // Straight ahead at a standstill is fine — it commands nothing.
        s.get_mut::<AutoDrive>()
            .unwrap()
            .command(DriveCommand::halt())
            .unwrap();
    }

    fn feed_isb(s: &mut Session, state: ShortcutButtonState, at: Instant) {
        let frame = Frame::new(
            Identifier::encode(
                Priority::Default,
                PGN_SHORTCUT_BUTTON,
                0x26,
                BROADCAST_ADDRESS,
            ),
            // Bytes 1-6 reserved (FF), byte 7 transition count, byte 8 state in
            // bits 1-2 with the rest reserved.
            [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xFC | state as u8],
            8,
        );
        s.feed(0, &frame, at);
    }

    /// C13 — `clear_stop()` cleared a stop the operator was still asserting,
    /// giving a window of commanded motion against a held-down button.
    #[test]
    fn clear_stop_is_refused_while_the_button_is_held() {
        let mut s = node();
        let mut now = Instant::from_millis(5_000);
        feed_info(&mut s, 0, now);
        s.tick(now);
        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();

        feed_isb(&mut s, ShortcutButtonState::StopImplementOperations, now);
        assert_eq!(
            s.get::<AutoDrive>().unwrap().stop_reason(),
            Some(SafeStopTrigger::IsbStop)
        );

        // Still held: clearing must not take, and engage must stay refused.
        assert_eq!(
            s.get_mut::<AutoDrive>().unwrap().clear_stop(),
            Err(AutodriveRefusal::StopConditionLive)
        );
        assert!(s.get::<AutoDrive>().unwrap().is_isb_stop_asserted());
        assert_eq!(
            s.get::<AutoDrive>().unwrap().stop_reason(),
            Some(SafeStopTrigger::IsbStop),
            "a stop being actively asserted must not be clearable"
        );
        assert_eq!(
            s.get_mut::<AutoDrive>().unwrap().engage(),
            Err(AutodriveRefusal::StopLatched)
        );

        // Released: now the explicit clear is honoured.
        now = now.add_millis(50);
        feed_isb(
            &mut s,
            ShortcutButtonState::PermitAllImplementsToOperate,
            now,
        );
        s.get_mut::<AutoDrive>().unwrap().clear_stop().unwrap();
        assert!(!s.get::<AutoDrive>().unwrap().is_isb_stop_asserted());
        assert_eq!(s.get::<AutoDrive>().unwrap().stop_reason(), None);
    }

    /// C36 — cutting the ISB cable left the machine steering with no stop
    /// authority. Seen once then lost is a stop, not permission.
    #[test]
    fn losing_the_isb_transmitter_stops_the_machine() {
        let mut s = node();
        let mut now = Instant::from_millis(5_000);
        feed_info(&mut s, 0, now);
        s.tick(now);
        feed_isb(
            &mut s,
            ShortcutButtonState::PermitAllImplementsToOperate,
            now,
        );
        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();
        assert!(s.get::<AutoDrive>().unwrap().is_engaged());

        // The ISB node disappears; Machine Info keeps arriving so the guidance
        // link watchdog cannot be what trips.
        for _ in 0..8 {
            now = now.add_millis(50);
            feed_info(&mut s, 0, now);
            s.tick(now);
        }

        assert_eq!(
            s.get::<AutoDrive>().unwrap().stop_reason(),
            Some(SafeStopTrigger::IsbStop),
            "a silent ISB source is a loss of stop authority"
        );
        assert!(!s.get::<AutoDrive>().unwrap().is_engaged());
    }

    /// C11 — preconditions were checked once at engage, so the operator
    /// dropping the engage switch mid-drive did not stop this node asking for
    /// the wheel.
    #[test]
    fn dropping_the_engage_switch_mid_drive_disengages() {
        let mut s = node();
        let now = Instant::from_millis(5_000);
        feed_info(&mut s, 0, now);
        s.tick(now);
        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();
        assert!(s.get::<AutoDrive>().unwrap().is_engaged());

        // Byte 5 bits 6..7 carry the remote engage switch; 0b00 is
        // disabled/off/passive — the operator letting go.
        let mut data = MACHINE_INFO;
        data[4] &= 0x3F;
        let frame = Frame::new(
            Identifier::encode(
                Priority::Default,
                PGN_GUIDANCE_MACHINE_INFO,
                0xF0,
                BROADCAST_ADDRESS,
            ),
            data,
            8,
        );
        s.feed(0, &frame, now.add_millis(50));

        assert!(
            !s.get::<AutoDrive>().unwrap().is_engaged(),
            "the operator's engage switch dropping must disengage"
        );
        assert_eq!(
            s.get::<AutoDrive>().unwrap().stop_reason(),
            Some(SafeStopTrigger::OperatorOverride)
        );
    }

    /// H26 — guidance limit status 1 is `OperatorLimitedControlled`: the
    /// operator is limiting or has taken control. It used to fall through to
    /// the catch-all and be mirrored as `ActiveNotLimited`, so an operator
    /// intervention read as normal operation and the controller kept steering.
    #[test]
    fn an_operator_limited_status_stops_the_controller() {
        for limit_status in [1u8, 6] {
            let mut s = node();
            let now = Instant::from_millis(5_000);
            feed_info(&mut s, 0, now);
            s.tick(now);
            s.get_mut::<AutoDrive>().unwrap().engage().unwrap();
            assert!(s.get::<AutoDrive>().unwrap().is_engaged());

            feed_info(&mut s, limit_status, now.add_millis(50));
            assert!(
                !s.get::<AutoDrive>().unwrap().is_engaged(),
                "limit status {limit_status} must stop the controller"
            );
            assert_eq!(
                s.get::<AutoDrive>().unwrap().stop_reason(),
                Some(SafeStopTrigger::OperatorOverride)
            );
        }
    }

    /// A reserved or not-available limit status says nothing about the limit,
    /// so it must not be reported as "not limited" either.
    #[test]
    fn an_unknown_limit_status_leaves_the_state_alone() {
        let mut s = node();
        let now = Instant::from_millis(5_000);
        feed_info(&mut s, 2, now);
        s.tick(now);
        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();
        feed_info(&mut s, 2, now.add_millis(50));
        assert_eq!(
            s.get::<AutoDrive>().unwrap().status(),
            AutomationStatus::ActiveLimitedHigh
        );

        // 7 = not available: the previous reading stands.
        feed_info(&mut s, 7, now.add_millis(100));
        assert_eq!(
            s.get::<AutoDrive>().unwrap().status(),
            AutomationStatus::ActiveLimitedHigh,
            "an unknown limit must not be reported as not-limited"
        );
    }

    /// P2.2 — the codec clamps an out-of-range curvature to the SLOT limit,
    /// which is full lock. A transient numerical excursion must be refused,
    /// not silently turned into a maximum-curvature steering command at speed.
    #[test]
    fn an_unencodable_curvature_is_refused_rather_than_clamped() {
        let mut s = node();
        let now = Instant::from_millis(5_000);
        feed_info(&mut s, 0, now);
        s.tick(now);
        s.get_mut::<AutoDrive>().unwrap().engage().unwrap();

        for bad in [1.0e9, -1.0e9, f64::INFINITY, f64::NAN] {
            assert_eq!(
                s.get_mut::<AutoDrive>().unwrap().command(DriveCommand {
                    curvature_km_inv: Some(bad),
                    speed_mps: Some(2.0),
                }),
                Err(AutodriveRefusal::CurvatureOutOfRange),
                "{bad} must be refused"
            );
        }

        // A non-finite speed is refused on the same principle.
        assert_eq!(
            s.get_mut::<AutoDrive>().unwrap().command(DriveCommand {
                curvature_km_inv: Some(10.0),
                speed_mps: Some(f64::NAN),
            }),
            Err(AutodriveRefusal::SpeedNotFinite)
        );

        // The extremes of the encodable range are still accepted.
        for good in [0.0, 8031.75, -8032.0] {
            assert!(
                s.get_mut::<AutoDrive>()
                    .unwrap()
                    .command(DriveCommand {
                        curvature_km_inv: Some(good),
                        speed_mps: Some(2.0),
                    })
                    .is_ok(),
                "{good} is encodable and must be accepted"
            );
        }
    }
}
