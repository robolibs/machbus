//! Automatic guidance / **autosteer** as a [`Plugin`] (ISO 11783-7).
//!
//! Autosteer in ISOBUS is **curvature-based**: you do not send waypoints or a
//! raw steering angle — you send a desired path **curvature** (1/km, i.e. the
//! inverse of the turn radius) and the tractor's steering ECU closes the loop on
//! the wheels to achieve it. Speed is separate (the tractor owns its speed).
//!
//! This plugin acts as the *guidance controller*:
//! - it broadcasts the **Agricultural Guidance System Command** (PGN 0xAD00),
//!   carrying both the commanded curvature
//!   ([`Guidance::command_curvature`] / [`Guidance::command_radius`]) **and** the
//!   *Curvature Command Status* — the 2-bit "intend to steer" request that the
//!   steering ECU needs in order to engage. Assert it with [`Guidance::engage`]
//!   and clear it with [`Guidance::disengage`]; until you engage, every command
//!   is sent with status *not intended to steer* and the ECU will not autosteer;
//! - it decodes the steering ECU's **Agricultural Guidance Machine Info**
//!   (PGN 0xAC00) into [`Event::Guidance`] and caches the latest
//!   [`GuidanceMachineInfo`] (estimated curvature, steering readiness, limit
//!   status) for fine control via `session.get::<Guidance>()`.
//!
//! Turning a path + GNSS pose into a curvature each cycle (pure-pursuit / Stanley)
//! is the application's job; this plugin moves the resulting command on the wire.

use crate::isobus::implement::guidance::{
    GenericSaeBs02SlotValue, GuidanceMachineInfo, MechanicalLockout, curvature_within_range,
};
use crate::isobus::implement::{
    CurvatureCommandStatus, GuidanceSystemCmd, MachineDirection, MachineSpeedCommandMsg,
};
use crate::j1939::shortcut_button::{ShortcutButtonState, decode_message};
use crate::net::pgn_defs::{
    PGN_GUIDANCE_MACHINE_INFO, PGN_GUIDANCE_SYSTEM_CMD, PGN_MACHINE_SELECTED_SPEED_CMD,
    PGN_SHORTCUT_BUTTON,
};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{
    AutodriveRefusal, BusEvent, Event, GuidanceEvent, SafeStopTrigger, StopLatch,
};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_GUIDANCE_MACHINE_INFO, PGN_SHORTCUT_BUTTON];

/// The PGNs this controller commands the machine with. A refused send on either
/// means the command never reached the bus.
const COMMAND_PGNS: &[Pgn] = &[PGN_GUIDANCE_SYSTEM_CMD, PGN_MACHINE_SELECTED_SPEED_CMD];

/// A steering ECU broadcasts Machine Info (PGN 0xAC00) every 100 ms. Three
/// missed broadcasts is the AEF 023 loss-of-communication threshold.
const LINK_TIMEOUT_MS: u32 = 300;

/// Fastest the controller will put a command on the bus. ISO 11783-7 §5.2.7.2
/// forbids transmitting faster than the parameter group's minimum update
/// period; the guidance pair is a 100 ms group.
const MIN_TX_INTERVAL_MS: u32 = 100;

/// Slowest the controller will go while it has something to say. The command
/// is a heartbeat: a steering ECU that stops hearing it must time out, so the
/// controller keeps re-sending the current setpoint even when nothing changes.
const MAX_TX_INTERVAL_MS: u32 = 2000;

/// Below this ground speed a commanded yaw rate cannot be expressed as a path
/// curvature, so [`Guidance::command_velocity`] commands straight instead.
pub const MIN_CURVATURE_SPEED_MPS: f64 = 0.05;

/// How long an engaged controller re-transmits an unrefreshed setpoint before
/// treating the commanding application as dead. See
/// [`super::autodrive::COMMAND_STALE_MS`].
const COMMAND_STALE_MS: u32 = 300;

/// Automatic-guidance (autosteer) plugin.
#[derive(Default)]
pub struct Guidance {
    latest: Option<GuidanceMachineInfo>,
    /// When the last Machine Info (PGN 0xAC00) was received, for link liveness.
    last_info_at: Option<Instant>,
    /// Cached liveness: fresh Machine Info seen within [`LINK_TIMEOUT_MS`].
    /// Recomputed each tick so it decays when the ECU stops broadcasting.
    link_alive: bool,
    /// Whether the controller is currently requesting the steering ECU to steer
    /// (the *Curvature Command Status* sent on PGN 0xAD00).
    engaged: bool,
    /// Current commanded curvature (1/km). Last value wins: a caller that
    /// commands faster than the cadence replaces the setpoint rather than
    /// queueing another frame.
    commanded_curvature: f64,
    /// Speed setpoint from [`Guidance::command_velocity`], if the caller drives
    /// this plugin with a twist rather than curvature alone.
    speed_setpoint: Option<(f64, MachineDirection)>,
    /// When the setpoint last reached the bus.
    last_tx_at: Option<Instant>,
    /// Setpoint or engage state changed since the last transmission, so the
    /// next one may go out as soon as the minimum interval allows.
    dirty: bool,
    /// Latching record of the first failure that demanded the safe state.
    /// Latching, not momentary: steering must not resume by itself when a
    /// button is released or a link comes back.
    stop: StopLatch,
    /// Bumped by every application command. The setters have no clock, so
    /// freshness is timed in `on_tick` by watching this change.
    command_seq: u64,
    seen_seq: u64,
    seq_changed_at: Option<Instant>,
}

impl Guidance {
    /// A guidance controller that commands curvature and listens for machine info.
    /// Starts **disengaged**: call [`engage`](Self::engage) before commands will steer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The Guidance System Command (PGN 0xAD00) for the current setpoint.
    fn system_command(&self) -> GuidanceSystemCmd {
        GuidanceSystemCmd {
            commanded_curvature: self.commanded_curvature,
            status: if self.engaged {
                CurvatureCommandStatus::IntendedToSteer
            } else {
                CurvatureCommandStatus::NotIntendedToSteer
            },
        }
    }

    /// Force the safe state: straight ahead, not intending to steer. Used on
    /// every failure path so the last command cannot persist.
    fn enter_safe_state(&mut self) {
        self.engaged = false;
        self.commanded_curvature = 0.0;
        self.speed_setpoint = Some((0.0, MachineDirection::Forward));
        self.dirty = true;
    }

    /// Request the steering ECU to engage and steer to the commanded curvature.
    ///
    /// Sets the Curvature Command Status to *intended to steer* and immediately
    /// re-queues the last commanded curvature so the intent reaches the bus on the
    /// next tick. The ECU only actually engages if its own machine info reports it
    /// ready (see [`is_steering_ready`](Self::is_steering_ready)).
    /// # Errors
    /// Returns the first unmet precondition. `engage()` used to set the flag
    /// unconditionally: it checked neither link liveness, nor the mechanical
    /// lockout, nor the operator's engage switch, nor whether a stop was
    /// latched, so an application could request steering from a machine that
    /// had told it not to.
    pub fn engage(&mut self) -> Result<(), AutodriveRefusal> {
        if self.stop.is_latched() {
            return Err(AutodriveRefusal::StopLatched);
        }
        if !self.link_alive {
            return Err(AutodriveRefusal::LinkDown);
        }
        let info = self.latest.ok_or(AutodriveRefusal::LinkDown)?;
        if info.lockout == MechanicalLockout::Active {
            return Err(AutodriveRefusal::MechanicalLockout);
        }
        if info.remote_engage_switch_status == GenericSaeBs02SlotValue::DisabledOffPassive {
            return Err(AutodriveRefusal::OperatorNotEngaged);
        }
        self.engaged = true;
        self.dirty = true;
        self.command_seq = self.command_seq.wrapping_add(1);
        Ok(())
    }

    /// `true` while a stop is latched. [`engage`](Self::engage) refuses until
    /// [`clear_stop`](Self::clear_stop) is called.
    #[must_use]
    pub fn is_stop_latched(&self) -> bool {
        self.stop.is_latched()
    }

    /// Why the controller stopped, if it did.
    #[must_use]
    pub fn stop_reason(&self) -> Option<SafeStopTrigger> {
        self.stop.reason()
    }

    /// Release a latched stop. Deliberately explicit: a fault clearing itself
    /// is not consent to move.
    pub fn clear_stop(&mut self) {
        self.stop.clear();
    }

    /// Trip the safe state from outside — the application wires bus-off,
    /// address-claim loss and heartbeat errors here, since those are observed
    /// at session level rather than on a PGN this plugin subscribes to.
    pub fn request_stop(&mut self, trigger: SafeStopTrigger) -> bool {
        let tripped = self.stop.trip(trigger);
        if tripped {
            self.enter_safe_state();
        }
        tripped
    }

    /// Stop requesting steering: clears the engage request and commands straight.
    ///
    /// Sends curvature `0.0` with status *not intended to steer*, so a conformant
    /// steering ECU drops back to manual control.
    pub fn disengage(&mut self) {
        self.enter_safe_state();
    }

    /// Whether the controller is currently requesting steering (its own intent —
    /// not the ECU's readiness; for that see [`is_steering_ready`](Self::is_steering_ready)).
    #[must_use]
    pub fn is_engaged(&self) -> bool {
        self.engaged
    }

    /// Command the steering system to follow a path **curvature** in 1/km.
    ///
    /// `0.0` = drive straight. Positive and negative follow the ISO 11783-7
    /// wire convention; out-of-range values are clamped by the codec. Queued and
    /// flushed on the next tick as a Guidance System Command (PGN 0xAD00). The
    /// command only steers while the controller is [`engage`](Self::engage)d.
    /// A curvature the codec cannot encode is commanded as **straight**, not
    /// clamped to full lock and not sent as not-available beside an
    /// intent-to-steer. Use [`try_command_curvature`](Self::try_command_curvature)
    /// to see the refusal instead of the safe substitution.
    pub fn command_curvature(&mut self, curvature_per_km: f64) {
        let safe = if curvature_within_range(curvature_per_km) {
            curvature_per_km
        } else {
            0.0
        };
        if self.commanded_curvature != safe {
            self.dirty = true;
        }
        self.commanded_curvature = safe;
        self.command_seq = self.command_seq.wrapping_add(1);
    }

    /// [`command_curvature`](Self::command_curvature) that reports an
    /// unencodable value rather than substituting straight ahead (G7).
    ///
    /// # Errors
    /// [`AutodriveRefusal::CurvatureOutOfRange`] when the value is non-finite
    /// or outside the SLOT, where the codec would otherwise clamp it to full
    /// lock.
    pub fn try_command_curvature(
        &mut self,
        curvature_per_km: f64,
    ) -> Result<(), AutodriveRefusal> {
        if !curvature_within_range(curvature_per_km) {
            return Err(AutodriveRefusal::CurvatureOutOfRange);
        }
        self.command_curvature(curvature_per_km);
        Ok(())
    }

    /// Command a turn of the given **radius in metres** (a convenience over
    /// [`command_curvature`](Self::command_curvature); curvature = 1000 / radius).
    /// A zero or non-finite radius commands straight ahead.
    pub fn command_radius(&mut self, radius_m: f64) {
        let curvature = if radius_m.is_finite() && radius_m.abs() > f64::EPSILON {
            1000.0 / radius_m
        } else {
            0.0
        };
        self.command_curvature(curvature);
    }

    /// Command straight-ahead (zero curvature).
    pub fn command_straight(&mut self) {
        self.command_curvature(0.0);
    }

    /// Command with a **robotics-style twist**: linear velocity `linear_mps`
    /// (m/s, forward positive) and angular/yaw velocity `angular_rad_s`
    /// (rad/s, left positive) — the `(v, ω)` interface from mobile robotics.
    ///
    /// Autosteer is curvature-based, and curvature is exactly `κ = ω / v`, so
    /// this sends **two** messages: the steering curvature on the Guidance
    /// System Command (PGN 0xAD00) **and** the target speed on the Machine
    /// Selected Speed Command (PGN 0xFD43). Reverse is encoded via the speed
    /// command's direction; the curvature sign follows the ISO 11783-7 wire
    /// convention (flip `angular_rad_s` if your platform's sign differs).
    ///
    /// A `linear_mps` below [`MIN_CURVATURE_SPEED_MPS`] cannot define a forward
    /// path curvature, so it commands straight (`κ = 0`) while still sending
    /// the (near-zero) speed.
    ///
    /// The guard is a *physical* threshold, not `f64::EPSILON`: at 1e-16 m/s a
    /// micrometre-per-second odometry residue yields a curvature in the
    /// billions, which the codec clamps to ±8031.75 km⁻¹ — a 12 cm turn radius
    /// — and transmits as a perfectly valid maximum-curvature command.
    pub fn command_velocity(&mut self, linear_mps: f64, angular_rad_s: f64) {
        // Steering: curvature κ = ω / v, in 1/m → 1/km for the wire. The twist
        // is left-positive (robotics); AEF 023 D.7.2.1 makes the wire SLOT
        // right-positive, so the sign is flipped at this boundary.
        let curvature_per_km = if linear_mps.abs() > MIN_CURVATURE_SPEED_MPS {
            -(angular_rad_s / linear_mps) * 1000.0
        } else {
            0.0
        };
        self.command_curvature(curvature_per_km);

        // Speed: Machine Selected Speed Command (magnitude + direction).
        let direction = if linear_mps < 0.0 {
            MachineDirection::Reverse
        } else {
            MachineDirection::Forward
        };
        let setpoint = Some((linear_mps.abs(), direction));
        if self.speed_setpoint != setpoint {
            self.dirty = true;
        }
        self.speed_setpoint = setpoint;
    }

    /// The most recent machine info from the steering ECU, if any has arrived.
    #[must_use]
    pub fn latest_machine_info(&self) -> Option<GuidanceMachineInfo> {
        self.latest
    }

    /// The steering system's last estimated curvature (1/km), if known.
    #[must_use]
    pub fn estimated_curvature(&self) -> Option<f64> {
        self.latest.map(|m| m.estimated_curvature)
    }

    /// Whether the steering system last reported it is ready/engaged to steer.
    ///
    /// This is the ECU's *self-reported* readiness slot. Note that some machines
    /// never populate it (they leave it `NotAvailable`) even while genuinely
    /// steering — use [`is_link_alive`](Self::is_link_alive) to tell whether
    /// guidance data is actually flowing.
    #[must_use]
    pub fn is_steering_ready(&self) -> bool {
        // Gated on liveness: `latest` is never cleared, so without this an
        // application polling `if is_steering_ready() { engage() }` would see
        // the last EnabledOnActive forever after the ECU was unplugged.
        self.link_alive
            && matches!(
                self.latest.map(|m| m.steering_system_readiness_state),
                Some(GenericSaeBs02SlotValue::EnabledOnActive)
            )
    }

    /// Age of the cached Machine Info in milliseconds, or `None` if none has
    /// arrived. Pair with [`latest_machine_info`](Self::latest_machine_info)
    /// when displaying the raw record, which is deliberately not expired.
    #[must_use]
    pub fn machine_info_age_ms(&self, now: Instant) -> Option<u32> {
        self.last_info_at.map(|t| now.millis_since(t))
    }

    /// Whether a steering ECU is currently broadcasting Machine Info (PGN 0xAC00)
    /// — i.e. the guidance link is live (fresh frame within [`LINK_TIMEOUT_MS`]).
    ///
    /// This reflects that guidance data is flowing regardless of what the ECU's
    /// readiness slot reports, so it stays true for machines that stream valid
    /// machine info without ever asserting `EnabledOnActive`.
    #[must_use]
    pub fn is_link_alive(&self) -> bool {
        self.link_alive
    }

    /// The steering system's last self-reported readiness slot, if any Machine
    /// Info has arrived — for displaying the raw state verbatim.
    #[must_use]
    pub fn steering_readiness_state(&self) -> Option<GenericSaeBs02SlotValue> {
        self.latest.map(|m| m.steering_system_readiness_state)
    }
}

impl Plugin for Guidance {
    fn name(&self) -> &'static str {
        "guidance"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        // Auxiliary Shortcut Button: the operator's stop-all command. This was
        // decoded elsewhere in the crate and acted on nowhere, so pressing it
        // left autosteer commanding exactly as before.
        if msg.pgn == PGN_SHORTCUT_BUTTON
            && let Some(decoded) = decode_message(msg)
            && decoded.state == ShortcutButtonState::StopImplementOperations
        {
            let was_engaged = self.engaged;
            if self.stop.trip(SafeStopTrigger::IsbStop) {
                self.enter_safe_state();
                ctx.emit(Event::Guidance(GuidanceEvent::StopRequested {
                    was_engaged,
                }));
            }
            return;
        }

        if msg.pgn == PGN_GUIDANCE_MACHINE_INFO
            && let Some(info) = GuidanceMachineInfo::decode(&msg.data)
        {
            let was_dead = !self.link_alive && self.last_info_at.is_some();
            self.latest = Some(info);
            self.last_info_at = Some(ctx.now());
            self.link_alive = true;
            if was_dead {
                ctx.emit(Event::Guidance(GuidanceEvent::LinkRestored {
                    source: msg.source,
                }));
            }
            ctx.emit(Event::Guidance(GuidanceEvent::MachineInfo {
                source: msg.source,
                estimated_curvature: info.estimated_curvature,
                steering_ready: matches!(
                    info.steering_system_readiness_state,
                    GenericSaeBs02SlotValue::EnabledOnActive
                ),
                limit_status: info.guidance_limit_status.as_u8(),
            }));
        }
    }

    /// React to a session-observed fault. `request_stop` used to be the
    /// application's job to wire up, which meant an application that did not
    /// know to call it kept steering through bus-off, a lost address claim and
    /// a heartbeat error alike.
    fn on_event(&mut self, event: &Event, ctx: &mut PluginCtx<'_>) {
        let trigger = SafeStopTrigger::from_event(event).or_else(|| match event {
            Event::Bus(BusEvent::SendFailed { pgn, .. }) if COMMAND_PGNS.contains(pgn) => {
                Some(SafeStopTrigger::SendFailed(*pgn))
            }
            _ => None,
        });
        let Some(trigger) = trigger else {
            return;
        };
        let was_engaged = self.engaged;
        if self.request_stop(trigger) {
            ctx.emit(Event::Guidance(GuidanceEvent::StopRequested {
                was_engaged,
            }));
        }
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();

        // Loss of the steering ECU forces the safe state. Previously this flag
        // was recomputed every tick and never read, so an engaged controller
        // kept streaming "intended to steer" at the last curvature forever
        // after the ECU fell silent.
        let alive = self
            .last_info_at
            .is_some_and(|t| now.millis_since(t) < LINK_TIMEOUT_MS);
        if self.link_alive && !alive {
            let silent_for_ms = self.last_info_at.map_or(0, |t| now.millis_since(t));
            let was_engaged = self.engaged;
            self.stop.trip(SafeStopTrigger::GuidanceLinkTimeout);
            self.enter_safe_state();
            ctx.emit(Event::Guidance(GuidanceEvent::LinkLost {
                silent_for_ms,
                was_engaged,
            }));
        }
        self.link_alive = alive;

        // A live application refreshes its setpoint; one that has died does
        // not. Without this the cadence re-transmits "intended to steer" at the
        // last curvature forever — unintended motion with no bound in time.
        if self.command_seq != self.seen_seq {
            self.seen_seq = self.command_seq;
            self.seq_changed_at = Some(now);
        }
        if self.engaged
            && self
                .seq_changed_at
                .is_some_and(|t| now.millis_since(t) >= COMMAND_STALE_MS)
            && self.stop.trip(SafeStopTrigger::CommandStale)
        {
            let was_engaged = self.engaged;
            self.enter_safe_state();
            ctx.emit(Event::Guidance(GuidanceEvent::StopRequested {
                was_engaged,
            }));
        }

        // Nothing queued before the claim completes reaches the bus, and the
        // cadence used to advance anyway — which both hid the refusal and
        // delayed the first real command by up to MAX_TX_INTERVAL_MS.
        if !ctx.is_claimed() {
            return Some(now.add_millis(u64::from(MIN_TX_INTERVAL_MS)));
        }

        // Last value wins on a bounded cadence: at most one frame per PGN per
        // MIN_TX_INTERVAL_MS, and at least one per MAX_TX_INTERVAL_MS so the
        // command keeps behaving as the heartbeat a steering ECU expects.
        let due = match self.last_tx_at {
            None => true,
            Some(last) => {
                let since = now.millis_since(last);
                since >= MAX_TX_INTERVAL_MS || (self.dirty && since >= MIN_TX_INTERVAL_MS)
            }
        };

        if due {
            ctx.send(
                PGN_GUIDANCE_SYSTEM_CMD,
                self.system_command().encode().to_vec(),
                BROADCAST_ADDRESS,
                Priority::Normal,
            );
            if let Some((speed_mps, direction)) = self.speed_setpoint {
                let speed = MachineSpeedCommandMsg::default()
                    .with_speed_mps(speed_mps)
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
                .map_or(now, |last| last.add_millis(u64::from(MIN_TX_INTERVAL_MS))),
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
    use crate::net::Name;
    use crate::session::Session;
    use crate::time::Instant;

    /// A real captured Machine Info frame: lockout not active, engage switch
    /// not-available (i.e. not actively disabled). Feeding one is now a
    /// precondition of `engage()`.
    const CAPTURED_MACHINE_INFO: [u8; 8] = [0x64, 0x7D, 0x3C, 0xFF, 0xC0, 0xFF, 0xFF, 0xFF];

    fn feed_machine_info(s: &mut Session, at: Instant) {
        use crate::net::{Frame, Identifier};
        let frame = Frame::new(
            Identifier::encode(
                Priority::Default,
                PGN_GUIDANCE_MACHINE_INFO,
                0xF0,
                BROADCAST_ADDRESS,
            ),
            CAPTURED_MACHINE_INFO,
            8,
        );
        s.feed(0, &frame, at);
    }

    fn claimed_session() -> Session {
        let name = Name::default()
            .with_identity_number(0x123)
            .with_function_code(0x80)
            .with_self_configurable(true);
        let mut s = Session::builder(name, 0x80)
            .plug(Guidance::new())
            .build()
            .unwrap();
        s.start().unwrap();
        let mut now = Instant::ZERO;
        for _ in 0..40 {
            now = now.add_millis(50);
            s.tick(now);
            while s.poll_transmit().is_some() {}
            if s.is_claimed() {
                break;
            }
        }
        s
    }

    #[test]
    fn link_liveness_tracks_machine_info_arrival_and_decay() {
        use crate::net::pgn_defs::PGN_GUIDANCE_MACHINE_INFO;
        use crate::net::{BROADCAST_ADDRESS, Frame, Identifier, Priority};

        let mut s = claimed_session();

        // Before any Machine Info: link dead, readiness unknown.
        assert!(!s.get::<Guidance>().unwrap().is_link_alive());
        assert!(
            s.get::<Guidance>()
                .unwrap()
                .steering_readiness_state()
                .is_none()
        );

        // Feed a real captured GMS frame (PGN 0xAC00) — this machine reports
        // readiness = NotAvailable even though it streams valid machine info.
        let frame = Frame::new(
            Identifier::encode(
                Priority::Default,
                PGN_GUIDANCE_MACHINE_INFO,
                0xF0,
                BROADCAST_ADDRESS,
            ),
            [0x64, 0x7D, 0x3C, 0xFF, 0xC0, 0xFF, 0xFF, 0xFF],
            8,
        );
        s.feed(0, &frame, Instant::from_millis(10_000));
        s.tick(Instant::from_millis(10_050));

        let g = s.get::<Guidance>().unwrap();
        assert!(g.is_link_alive(), "fresh Machine Info ⇒ link live");
        assert_eq!(
            g.steering_readiness_state(),
            Some(GenericSaeBs02SlotValue::NotAvailableTakeNoAction)
        );
        assert!(
            !g.is_steering_ready(),
            "this machine never asserts EnabledOnActive"
        );
        assert!(g.estimated_curvature().is_some(), "est curvature is known");

        // No further frames past the timeout window ⇒ link decays to dead, but
        // the last readiness/curvature stays cached for display.
        s.tick(Instant::from_millis(10_000 + LINK_TIMEOUT_MS as u64 + 10));
        let g = s.get::<Guidance>().unwrap();
        assert!(!g.is_link_alive(), "stale link ⇒ dead");
        assert!(g.steering_readiness_state().is_some());
    }

    #[test]
    fn command_velocity_emits_curvature_and_speed() {
        let mut s = claimed_session();
        // v = 2 m/s, ω = +0.04 rad/s is a **left** turn of 50 m radius. AEF 023
        // D.7.2.1 makes the wire right-positive, so κ = -20/km and
        // raw = (-20 + 8032) / 0.25 = 32048 = 0x7D30 → little-endian [30, 7D].
        s.get_mut::<Guidance>().unwrap().command_velocity(2.0, 0.04);
        s.tick(Instant::ZERO.add_millis(2050));

        let (mut saw_curv, mut saw_speed) = (false, false);
        while let Some((_, frame)) = s.poll_transmit() {
            match frame.id.pgn() {
                PGN_GUIDANCE_SYSTEM_CMD => {
                    saw_curv = true;
                    assert_eq!(
                        &frame.data[0..2],
                        &[0x30, 0x7D],
                        "a left turn is negative curvature on the wire"
                    );
                }
                PGN_MACHINE_SELECTED_SPEED_CMD => saw_speed = true,
                _ => {}
            }
        }
        assert!(saw_curv, "twist must emit a curvature command (PGN 0xAD00)");
        assert!(saw_speed, "twist must emit a speed command (PGN 0xFD43)");
    }

    /// The mirror image of the above, so the sign is pinned from both sides:
    /// turning to the driver's right must encode above the straight-ahead raw.
    #[test]
    fn turning_right_encodes_positive_curvature() {
        let mut s = claimed_session();
        // ω = -0.04 rad/s (clockwise) is a right turn → κ = +20/km,
        // raw = (20 + 8032) / 0.25 = 32208 = 0x7DD0 → little-endian [D0, 7D].
        s.get_mut::<Guidance>()
            .unwrap()
            .command_velocity(2.0, -0.04);
        s.tick(Instant::ZERO.add_millis(2050));

        let mut raw = None;
        while let Some((_, frame)) = s.poll_transmit() {
            if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                raw = Some(u16::from_le_bytes([frame.data[0], frame.data[1]]));
            }
        }
        let raw = raw.expect("a curvature command reaches the bus");
        assert_eq!(raw, 0x7DD0);
        assert!(
            raw > 0x7D80,
            "right of straight-ahead (0x7D80) must be the larger raw"
        );
    }

    /// The Guidance System Command (PGN 0xAD00) must carry the Curvature Command
    /// Status in byte 2 bits 0..1: `NotIntendedToSteer` (0) while disengaged,
    /// `IntendedToSteer` (1) after `engage()`. Bits 2..7 are reserved (sent as 1).
    #[test]
    fn engage_sets_curvature_command_status_on_the_wire() {
        use crate::isobus::implement::{CurvatureCommandStatus, GuidanceSystemCmd};

        fn last_system_cmd(s: &mut Session) -> GuidanceSystemCmd {
            let mut cmd = None;
            while let Some((_, frame)) = s.poll_transmit() {
                if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                    cmd = GuidanceSystemCmd::decode(&frame.data);
                }
            }
            cmd.expect("a Guidance System Command was transmitted")
        }

        let mut s = claimed_session();
        let mut now = Instant::ZERO.add_millis(2050);
        feed_machine_info(&mut s, now);

        // Disengaged: a curvature command requests no steering.
        s.get_mut::<Guidance>().unwrap().command_curvature(20.0);
        assert!(!s.get::<Guidance>().unwrap().is_engaged());
        s.tick(now);
        let cmd = last_system_cmd(&mut s);
        assert_eq!(cmd.status, CurvatureCommandStatus::NotIntendedToSteer);
        assert!((cmd.commanded_curvature - 20.0).abs() < 0.25);

        // engage() re-sends the last curvature with the intend-to-steer flag,
        // no sooner than the minimum transmit interval.
        s.get_mut::<Guidance>().unwrap().engage().unwrap();
        assert!(s.get::<Guidance>().unwrap().is_engaged());
        now = now.add_millis(u64::from(MIN_TX_INTERVAL_MS));
        feed_machine_info(&mut s, now);
        s.tick(now);
        let cmd = last_system_cmd(&mut s);
        assert_eq!(cmd.status, CurvatureCommandStatus::IntendedToSteer);
        assert!((cmd.commanded_curvature - 20.0).abs() < 0.25);

        // disengage() drops the request and commands straight.
        s.get_mut::<Guidance>().unwrap().disengage();
        assert!(!s.get::<Guidance>().unwrap().is_engaged());
        now = now.add_millis(u64::from(MIN_TX_INTERVAL_MS));
        s.tick(now);
        let cmd = last_system_cmd(&mut s);
        assert_eq!(cmd.status, CurvatureCommandStatus::NotIntendedToSteer);
        assert_eq!(cmd.commanded_curvature, 0.0);
    }

    /// S1.8 — the command is a heartbeat, not a one-shot. The plugin used to
    /// transmit only what the application queued, so an application that
    /// commanded once and stopped left the steering ECU holding a stale
    /// "intended to steer" forever while PGN 0xAD00 vanished from the bus.
    #[test]
    fn command_is_resent_without_further_application_calls() {
        let mut s = claimed_session();
        let mut now = Instant::ZERO.add_millis(2050);
        s.get_mut::<Guidance>().unwrap().command_curvature(20.0);

        let mut sent = 0usize;
        for _ in 0..60 {
            now = now.add_millis(50);
            s.tick(now);
            while let Some((_, frame)) = s.poll_transmit() {
                if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                    sent += 1;
                }
            }
        }

        assert!(
            sent >= 2,
            "the setpoint must keep reaching the bus with no further calls (saw {sent})"
        );
    }

    /// P1.4 — the other half of the heartbeat. Re-sending an unrefreshed
    /// setpoint forever is what turned a fail-silent path into a fail-active
    /// one: before the cadence existed, an application that died produced no
    /// frames and the steering ECU timed out. An *engaged* controller must stop
    /// on its own when its commanding application goes quiet.
    #[test]
    fn a_dead_application_stops_the_machine_instead_of_steering_forever() {
        let mut s = claimed_session();
        let mut now = Instant::ZERO.add_millis(2050);
        feed_machine_info(&mut s, now);
        s.get_mut::<Guidance>().unwrap().command_curvature(20.0);
        s.get_mut::<Guidance>().unwrap().engage().unwrap();
        assert!(s.get::<Guidance>().unwrap().is_engaged());

        // The application stops calling. Machine Info keeps arriving, so the
        // link watchdog stays satisfied and cannot be what trips.
        let mut last_status = None;
        for _ in 0..20 {
            now = now.add_millis(50);
            feed_machine_info(&mut s, now);
            s.tick(now);
            while let Some((_, frame)) = s.poll_transmit() {
                if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD
                    && let Some(cmd) = GuidanceSystemCmd::decode(&frame.data)
                {
                    last_status = Some(cmd.status);
                }
            }
        }

        assert_eq!(
            s.get::<Guidance>().unwrap().stop_reason(),
            Some(SafeStopTrigger::CommandStale),
            "a setpoint nobody is refreshing must stop the machine"
        );
        assert!(!s.get::<Guidance>().unwrap().is_engaged());
        assert_eq!(
            last_status,
            Some(CurvatureCommandStatus::NotIntendedToSteer),
            "the safe state must reach the bus, not just the local flag"
        );
    }

    /// The watchdog must not fire while the application is doing its job.
    #[test]
    fn a_refreshed_setpoint_keeps_steering() {
        let mut s = claimed_session();
        let mut now = Instant::ZERO.add_millis(2050);
        feed_machine_info(&mut s, now);
        s.get_mut::<Guidance>().unwrap().command_curvature(20.0);
        s.get_mut::<Guidance>().unwrap().engage().unwrap();

        for _ in 0..20 {
            now = now.add_millis(50);
            feed_machine_info(&mut s, now);
            s.get_mut::<Guidance>().unwrap().command_curvature(20.0);
            s.tick(now);
            while s.poll_transmit().is_some() {}
        }

        assert_eq!(s.get::<Guidance>().unwrap().stop_reason(), None);
        assert!(
            s.get::<Guidance>().unwrap().is_engaged(),
            "re-commanding the same value is still proof of life"
        );
    }

    /// S1.8 — and it must not flood. The committed capture shows ~314 Hz on
    /// PGN 0xAD00 against the TECU's 100 ms broadcasts, because every
    /// application call pushed another frame onto an unbounded queue.
    #[test]
    fn rapid_commands_are_rate_limited_to_the_minimum_interval() {
        let mut s = claimed_session();
        let mut now = Instant::ZERO.add_millis(2050);

        // One second of wall time, commanded every 3 ms as the drive tool does.
        let mut sent = 0usize;
        for i in 0..333 {
            s.get_mut::<Guidance>()
                .unwrap()
                .command_curvature(f64::from(i % 17));
            now = now.add_millis(3);
            s.tick(now);
            while let Some((_, frame)) = s.poll_transmit() {
                if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                    sent += 1;
                }
            }
        }

        assert!(
            sent <= 11,
            "~1 s at a 100 ms floor is at most ~10 frames, saw {sent}"
        );
    }

    /// S1.8 — safety commands are control traffic. ISO 11783-3 Table D.1 puts
    /// them at priority 3; the capture shows every real TECU frame as 0x0C…
    /// while machbus emitted 0x18… (priority 6).
    #[test]
    fn commands_are_sent_at_control_priority() {
        let mut s = claimed_session();
        s.get_mut::<Guidance>().unwrap().command_curvature(20.0);
        s.tick(Instant::ZERO.add_millis(2050));

        let mut checked = false;
        while let Some((_, frame)) = s.poll_transmit() {
            if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                assert_eq!(frame.id.priority(), Priority::Normal, "control priority 3");
                checked = true;
            }
        }
        assert!(checked, "a guidance command was transmitted");
    }

    /// S1.2 — losing the steering ECU must force the safe state. The liveness
    /// flag existed but had no consumer anywhere in the crate.
    #[test]
    fn losing_the_link_disengages_and_commands_straight() {
        use crate::isobus::implement::{CurvatureCommandStatus, GuidanceSystemCmd};
        let mut s = claimed_session();
        let base = 10_000u64;

        feed_machine_info(&mut s, Instant::from_millis(base));
        s.get_mut::<Guidance>().unwrap().command_curvature(20.0);
        s.get_mut::<Guidance>().unwrap().engage().unwrap();
        s.tick(Instant::from_millis(base + 100));
        assert!(s.get::<Guidance>().unwrap().is_engaged());
        while s.poll_transmit().is_some() {}
        while s.poll_event().is_some() {}

        // Let the link go quiet past the timeout.
        s.tick(Instant::from_millis(
            base + 100 + u64::from(LINK_TIMEOUT_MS) + 10,
        ));

        assert!(
            !s.get::<Guidance>().unwrap().is_engaged(),
            "a dead steering link must clear the engage request"
        );
        let lost = std::iter::from_fn(|| s.poll_event()).any(|e| {
            matches!(
                e,
                Event::Guidance(GuidanceEvent::LinkLost { was_engaged, .. }) if was_engaged
            )
        });
        assert!(lost, "the loss must be reported, not just acted on");

        let mut cmd = None;
        while let Some((_, frame)) = s.poll_transmit() {
            if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                cmd = GuidanceSystemCmd::decode(&frame.data);
            }
        }
        let cmd = cmd.expect("the safe state must reach the bus");
        assert_eq!(cmd.status, CurvatureCommandStatus::NotIntendedToSteer);
        assert_eq!(cmd.commanded_curvature, 0.0);
    }

    /// S1.4 — the ISB is the operator's stop-all button. It was decoded and
    /// cached, and no plugin, session path or example ever acted on it: an
    /// engaged autosteer kept commanding straight through a press.
    #[test]
    fn isb_stop_latches_and_blocks_re_engagement() {
        use crate::isobus::implement::{CurvatureCommandStatus, GuidanceSystemCmd};
        use crate::j1939::shortcut_button::{ShortcutButtonState, encode_with_transition_count};
        use crate::net::pgn_defs::PGN_SHORTCUT_BUTTON;
        use crate::net::{Frame, Identifier};

        let mut s = claimed_session();
        let base = 10_000u64;
        feed_machine_info(&mut s, Instant::from_millis(base));
        s.get_mut::<Guidance>().unwrap().command_curvature(20.0);
        s.get_mut::<Guidance>().unwrap().engage().unwrap();
        s.tick(Instant::from_millis(base));
        assert!(s.get::<Guidance>().unwrap().is_engaged());
        while s.poll_transmit().is_some() {}
        while s.poll_event().is_some() {}

        let stop = Frame::new(
            Identifier::encode(
                Priority::BelowNormal,
                PGN_SHORTCUT_BUTTON,
                0x30,
                BROADCAST_ADDRESS,
            ),
            encode_with_transition_count(ShortcutButtonState::StopImplementOperations, 3),
            8,
        );
        s.feed(0, &stop, Instant::from_millis(base + 10));

        let g = s.get::<Guidance>().unwrap();
        assert!(!g.is_engaged(), "an ISB stop must disengage");
        assert!(g.is_stop_latched());

        let reported = std::iter::from_fn(|| s.poll_event()).any(|e| {
            matches!(
                e,
                Event::Guidance(GuidanceEvent::StopRequested { was_engaged }) if was_engaged
            )
        });
        assert!(reported, "the stop must be reported");

        // Re-engaging is refused while latched — releasing the button must not
        // by itself put the machine back under automation.
        assert_eq!(
            s.get_mut::<Guidance>().unwrap().engage(),
            Err(AutodriveRefusal::StopLatched)
        );
        assert!(!s.get::<Guidance>().unwrap().is_engaged());

        s.tick(Instant::from_millis(
            base + u64::from(MIN_TX_INTERVAL_MS) + 20,
        ));
        let mut cmd = None;
        while let Some((_, frame)) = s.poll_transmit() {
            if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                cmd = GuidanceSystemCmd::decode(&frame.data);
            }
        }
        let cmd = cmd.expect("the safe state reaches the bus");
        assert_eq!(cmd.status, CurvatureCommandStatus::NotIntendedToSteer);
        assert_eq!(cmd.commanded_curvature, 0.0);

        // Explicitly clearing the latch restores control. The machine info must
        // be stamped after the last tick: a backwards clock is itself a fault
        // now, and would re-latch the stop before engage() is reached.
        s.get_mut::<Guidance>().unwrap().clear_stop();
        feed_machine_info(
            &mut s,
            Instant::from_millis(base + u64::from(MIN_TX_INTERVAL_MS) + 40),
        );
        s.get_mut::<Guidance>().unwrap().engage().unwrap();
        assert!(s.get::<Guidance>().unwrap().is_engaged());
    }

    /// W10 — `f64::EPSILON` is not a speed. A micrometre-per-second odometry
    /// residue used to yield a curvature in the billions, clamped to a 12 cm
    /// turn radius and transmitted as a valid maximum-curvature command.
    #[test]
    fn near_zero_speed_commands_straight_not_full_lock() {
        let mut s = claimed_session();
        s.get_mut::<Guidance>()
            .unwrap()
            .command_velocity(1e-6, 0.04);
        s.tick(Instant::ZERO.add_millis(2050));

        let mut curvature = None;
        while let Some((_, frame)) = s.poll_transmit() {
            if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
                curvature = crate::isobus::implement::GuidanceSystemCmd::decode(&frame.data)
                    .map(|c| c.commanded_curvature);
            }
        }
        assert_eq!(
            curvature,
            Some(0.0),
            "a speed below the physical threshold must command straight"
        );
    }
}
