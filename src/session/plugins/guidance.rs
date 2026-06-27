//! Automatic guidance / **autosteer** as a [`Plugin`] (ISO 11783-7).
//!
//! Autosteer in ISOBUS is **curvature-based**: you do not send waypoints or a
//! raw steering angle — you send a desired path **curvature** (1/km, i.e. the
//! inverse of the turn radius) and the tractor's steering ECU closes the loop on
//! the wheels to achieve it. Speed is separate (the tractor owns its speed).
//!
//! This plugin acts as the *guidance controller*:
//! - it broadcasts the **Guidance System Command** (PGN 0xAD00) carrying the
//!   commanded curvature ([`Guidance::command_curvature`] / [`Guidance::command_radius`]);
//! - it decodes the steering ECU's **Agricultural Guidance Machine Info**
//!   (PGN 0xAC00) into [`Event::Guidance`] and caches the latest
//!   [`GuidanceMachineInfo`] (estimated curvature, steering readiness, limit
//!   status) for fine control via `session.get::<Guidance>()`.
//!
//! Turning a path + GNSS pose into a curvature each cycle (pure-pursuit / Stanley)
//! is the application's job; this plugin moves the resulting command on the wire.

use crate::isobus::implement::guidance::{
    GenericSaeBs02SlotValue, GuidanceMachineInfo, GuidanceSystemStatus, SteeringReadiness,
};
use crate::isobus::implement::{MachineDirection, MachineSpeedCommandMsg};
use crate::net::pgn_defs::{
    PGN_GUIDANCE_MACHINE_INFO, PGN_GUIDANCE_SYSTEM_CMD, PGN_MACHINE_SELECTED_SPEED_CMD,
};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, GuidanceEvent};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_GUIDANCE_MACHINE_INFO];

/// Automatic-guidance (autosteer) plugin.
#[derive(Default)]
pub struct Guidance {
    latest: Option<GuidanceMachineInfo>,
    pending: Vec<(Pgn, Vec<u8>)>,
}

impl Guidance {
    /// A guidance controller that commands curvature and listens for machine info.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Command the steering system to follow a path **curvature** in 1/km.
    ///
    /// `0.0` = drive straight. Positive and negative follow the ISO 11783-7
    /// wire convention; out-of-range values are clamped by the codec. Queued and
    /// flushed on the next tick as a Guidance System Command (PGN 0xAD00).
    pub fn command_curvature(&mut self, curvature_per_km: f64) {
        let cmd = GuidanceSystemStatus {
            estimated_curvature: curvature_per_km,
            readiness: SteeringReadiness::FullyReady,
            integrity_level: 0,
        };
        self.pending
            .push((PGN_GUIDANCE_SYSTEM_CMD, cmd.encode().to_vec()));
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
    #[must_use]
    pub fn is_steering_ready(&self) -> bool {
        matches!(
            self.latest.map(|m| m.steering_system_readiness_state),
            Some(GenericSaeBs02SlotValue::EnabledOnActive)
        )
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
}
