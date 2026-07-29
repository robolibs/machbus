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

use crate::isobus::implement::guidance::{GenericSaeBs02SlotValue, GuidanceMachineInfo};
use crate::isobus::implement::{
    CurvatureCommandStatus, GuidanceSystemCmd, MachineDirection, MachineSpeedCommandMsg,
};
use crate::net::pgn_defs::{
    PGN_GUIDANCE_MACHINE_INFO, PGN_GUIDANCE_SYSTEM_CMD, PGN_MACHINE_SELECTED_SPEED_CMD,
};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, GuidanceEvent};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_GUIDANCE_MACHINE_INFO];

/// A steering ECU broadcasts Machine Info (PGN 0xAC00) every 100 ms. If none has
/// arrived within this window the guidance link is considered dead.
const LINK_TIMEOUT_MS: u32 = 500;

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
    /// Last curvature handed to the controller (1/km); re-sent verbatim whenever
    /// the engage state changes so the new intent reaches the bus immediately.
    commanded_curvature: f64,
    pending: Vec<(Pgn, Vec<u8>)>,
}

impl Guidance {
    /// A guidance controller that commands curvature and listens for machine info.
    /// Starts **disengaged**: call [`engage`](Self::engage) before commands will steer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue an Agricultural Guidance System Command (PGN 0xAD00) carrying the
    /// current commanded curvature and the engage-derived Curvature Command Status.
    fn queue_system_command(&mut self) {
        let cmd = GuidanceSystemCmd {
            commanded_curvature: self.commanded_curvature,
            status: if self.engaged {
                CurvatureCommandStatus::IntendedToSteer
            } else {
                CurvatureCommandStatus::NotIntendedToSteer
            },
        };
        self.pending
            .push((PGN_GUIDANCE_SYSTEM_CMD, cmd.encode().to_vec()));
    }

    /// Request the steering ECU to engage and steer to the commanded curvature.
    ///
    /// Sets the Curvature Command Status to *intended to steer* and immediately
    /// re-queues the last commanded curvature so the intent reaches the bus on the
    /// next tick. The ECU only actually engages if its own machine info reports it
    /// ready (see [`is_steering_ready`](Self::is_steering_ready)).
    pub fn engage(&mut self) {
        self.engaged = true;
        self.queue_system_command();
    }

    /// Stop requesting steering: clears the engage request and commands straight.
    ///
    /// Sends curvature `0.0` with status *not intended to steer*, so a conformant
    /// steering ECU drops back to manual control.
    pub fn disengage(&mut self) {
        self.engaged = false;
        self.commanded_curvature = 0.0;
        self.queue_system_command();
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
    pub fn command_curvature(&mut self, curvature_per_km: f64) {
        self.commanded_curvature = curvature_per_km;
        self.queue_system_command();
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
    /// A near-zero `linear_mps` cannot define a forward path curvature, so it
    /// commands straight (`κ = 0`) while still sending the (near-zero) speed.
    pub fn command_velocity(&mut self, linear_mps: f64, angular_rad_s: f64) {
        // Steering: curvature κ = ω / v, in 1/m → 1/km for the wire.
        let curvature_per_km = if linear_mps.abs() > f64::EPSILON {
            (angular_rad_s / linear_mps) * 1000.0
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
        let speed = MachineSpeedCommandMsg::default()
            .with_speed_mps(linear_mps.abs())
            .with_direction(direction);
        self.pending
            .push((PGN_MACHINE_SELECTED_SPEED_CMD, speed.encode().to_vec()));
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
        matches!(
            self.latest.map(|m| m.steering_system_readiness_state),
            Some(GenericSaeBs02SlotValue::EnabledOnActive)
        )
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
        if msg.pgn == PGN_GUIDANCE_MACHINE_INFO
            && let Some(info) = GuidanceMachineInfo::decode(&msg.data)
        {
            self.latest = Some(info);
            self.last_info_at = Some(ctx.now());
            self.link_alive = true;
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

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        // Decay link liveness if no fresh Machine Info arrived within the window.
        self.link_alive = self
            .last_info_at
            .is_some_and(|t| ctx.now().millis_since(t) < LINK_TIMEOUT_MS);

        for (pgn, data) in self.pending.drain(..) {
            ctx.send(pgn, data, BROADCAST_ADDRESS, Priority::Default);
        }
        None
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
        // v = 2 m/s, ω = 0.04 rad/s → κ = 0.02/m = 20/km = 50 m radius.
        // raw = (20 + 8032) / 0.25 = 32208 = 0x7DD0 → little-endian [D0, 7D].
        s.get_mut::<Guidance>().unwrap().command_velocity(2.0, 0.04);
        s.tick(Instant::ZERO.add_millis(2050));

        let (mut saw_curv, mut saw_speed) = (false, false);
        while let Some((_, frame)) = s.poll_transmit() {
            match frame.id.pgn() {
                PGN_GUIDANCE_SYSTEM_CMD => {
                    saw_curv = true;
                    assert_eq!(&frame.data[0..2], &[0xD0, 0x7D], "curvature κ=20/km");
                }
                PGN_MACHINE_SELECTED_SPEED_CMD => saw_speed = true,
                _ => {}
            }
        }
        assert!(saw_curv, "twist must emit a curvature command (PGN 0xAD00)");
        assert!(saw_speed, "twist must emit a speed command (PGN 0xFD43)");
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

        // Disengaged: a curvature command requests no steering.
        s.get_mut::<Guidance>().unwrap().command_curvature(20.0);
        assert!(!s.get::<Guidance>().unwrap().is_engaged());
        s.tick(now);
        let cmd = last_system_cmd(&mut s);
        assert_eq!(cmd.status, CurvatureCommandStatus::NotIntendedToSteer);
        assert!((cmd.commanded_curvature - 20.0).abs() < 0.25);

        // engage() re-queues the last curvature with the intend-to-steer flag.
        s.get_mut::<Guidance>().unwrap().engage();
        assert!(s.get::<Guidance>().unwrap().is_engaged());
        now = now.add_millis(50);
        s.tick(now);
        let cmd = last_system_cmd(&mut s);
        assert_eq!(cmd.status, CurvatureCommandStatus::IntendedToSteer);
        assert!((cmd.commanded_curvature - 20.0).abs() < 0.25);

        // disengage() drops the request and commands straight.
        s.get_mut::<Guidance>().unwrap().disengage();
        assert!(!s.get::<Guidance>().unwrap().is_engaged());
        now = now.add_millis(50);
        s.tick(now);
        let cmd = last_system_cmd(&mut s);
        assert_eq!(cmd.status, CurvatureCommandStatus::NotIntendedToSteer);
        assert_eq!(cmd.commanded_curvature, 0.0);
    }
}
