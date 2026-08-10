//! Types for the combined autonomous-driving surface.
//!
//! ISOBUS splits autonomy across two unrelated message families: steering is a
//! path **curvature** (ISO 11783-7 Guidance System Command, PGN 0xAD00) and
//! speed is a separate command (PGN 0xFD43), with the AEF TIM layer gating both
//! when a TIM peer is present. Nothing in the stack tied them together — they
//! were two plugins with no shared lifecycle, no shared safety state and no way
//! to refuse a command that the machine had not authorised.
//!
//! These are the shared vocabulary: the AEF automation states, a single command
//! covering both axes, and the reasons a request can be refused.

/// AEF 023 D.2.2 Table 45 — the 4-bit automation status SLOT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AutomationStatus {
    /// The function is not offered at all.
    Unavailable = 0x0,
    /// Offered, but its preconditions are not met.
    NotReady = 0x1,
    /// Preconditions met; the client may ask to enable it.
    ReadyToEnable = 0x2,
    /// Enabled, awaiting a setpoint.
    Enabled = 0x3,
    /// Requested and waiting on operator acknowledgement.
    Pending = 0x4,
    /// Actively controlling, tracking the setpoint.
    ActiveNotLimited = 0x5,
    /// Actively controlling, saturated at an upper limit.
    ActiveLimitedHigh = 0x6,
    /// Actively controlling, saturated at a lower limit.
    ActiveLimitedLow = 0x7,
    /// Non-recoverable fault.
    Fault = 0xD,
    /// Recoverable error.
    Error = 0xE,
    #[default]
    NotAvailable = 0xF,
}

impl AutomationStatus {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x0F {
            0x0 => Self::Unavailable,
            0x1 => Self::NotReady,
            0x2 => Self::ReadyToEnable,
            0x3 => Self::Enabled,
            0x4 => Self::Pending,
            0x5 => Self::ActiveNotLimited,
            0x6 => Self::ActiveLimitedHigh,
            0x7 => Self::ActiveLimitedLow,
            0xD => Self::Fault,
            0xE => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x0..=0x7 | 0xD | 0xE | 0xF => Some(Self::from_u8(v)),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// `true` while the function is actually controlling the machine, at a
    /// limit or not.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::ActiveNotLimited | Self::ActiveLimitedHigh | Self::ActiveLimitedLow
        )
    }

    /// `true` when the function is saturated. This is the anti-windup signal an
    /// outer control loop needs, and it must be distinguishable from a fault.
    #[must_use]
    pub const fn is_limited(self) -> bool {
        matches!(self, Self::ActiveLimitedHigh | Self::ActiveLimitedLow)
    }
}

/// A setpoint for both axes. `None` leaves that axis alone, so a caller that
/// only steers does not have to invent a speed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DriveCommand {
    /// Metres per second, forward positive.
    pub speed_mps: Option<f64>,
    /// Path curvature in km⁻¹ — the ISO 11783-7 SLOT unit, i.e. the inverse of
    /// the turn radius in kilometres.
    pub curvature_km_inv: Option<f64>,
}

impl DriveCommand {
    /// Steer only, leaving speed under whoever already owns it.
    #[must_use]
    pub const fn steer(curvature_km_inv: f64) -> Self {
        Self {
            speed_mps: None,
            curvature_km_inv: Some(curvature_km_inv),
        }
    }

    /// Drive straight at a speed.
    #[must_use]
    pub const fn drive(speed_mps: f64) -> Self {
        Self {
            speed_mps: Some(speed_mps),
            curvature_km_inv: Some(0.0),
        }
    }

    /// Stop and straighten — the setpoint every failure path falls back to.
    #[must_use]
    pub const fn halt() -> Self {
        Self {
            speed_mps: Some(0.0),
            curvature_km_inv: Some(0.0),
        }
    }
}

/// Lifecycle events from the combined autonomous-driving controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutodriveEvent {
    /// The automation status changed (including into and out of a limit).
    StateChanged { status: AutomationStatus },
    /// The controller began commanding the machine.
    Engaged,
    /// The controller stopped commanding, without a fault.
    Disengaged,
    /// A request was refused before anything reached the bus.
    Refused { refusal: AutodriveRefusal },
    /// The controller fell back to the safe state.
    SafeStop {
        trigger: crate::session::sys::SafeStopTrigger,
    },
}

/// Why an arm/engage/command request was refused.
///
/// Refusals are values rather than silent no-ops: an autonomy client that asks
/// to steer and is ignored cannot tell the difference between "commanded" and
/// "declined", which is exactly how a stale *intended to steer* used to survive
/// on the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutodriveRefusal {
    /// No TIM authority has been granted for this function.
    NoAuthority,
    /// The tractor never advertised the facility being commanded.
    FacilityNotAdvertised,
    /// The steering ECU is not broadcasting, so its state is unknown.
    LinkDown,
    /// The machine reports its steering is mechanically locked out.
    MechanicalLockout,
    /// The operator's engage switch is not active.
    OperatorNotEngaged,
    /// Below the speed at which a yaw rate defines a path curvature.
    SpeedBelowMinimum,
    /// A safe stop is latched and has not been explicitly cleared.
    StopLatched,
    /// The control function has no claimed address, so nothing would reach the
    /// bus.
    NotClaimed,
    /// The function's automation status is not one that accepts setpoints.
    StatusNotActive,
}

impl AutodriveRefusal {
    /// A short, stable identifier for logs and bindings.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoAuthority => "no_authority",
            Self::FacilityNotAdvertised => "facility_not_advertised",
            Self::LinkDown => "link_down",
            Self::MechanicalLockout => "mechanical_lockout",
            Self::OperatorNotEngaged => "operator_not_engaged",
            Self::SpeedBelowMinimum => "speed_below_minimum",
            Self::StopLatched => "stop_latched",
            Self::NotClaimed => "not_claimed",
            Self::StatusNotActive => "status_not_active",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automation_status_round_trips_and_rejects_reserved() {
        for raw in [0x0u8, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0xD, 0xE, 0xF] {
            let status = AutomationStatus::try_from_u8(raw).expect("assigned value");
            assert_eq!(status.as_u8(), raw);
            assert_eq!(AutomationStatus::from_u8(raw), status);
        }
        // 0x8..=0xC are unassigned in Table 45.
        for reserved in 0x8u8..=0xC {
            assert_eq!(AutomationStatus::try_from_u8(reserved), None);
        }
    }

    #[test]
    fn limited_is_distinguishable_from_faulted() {
        assert!(AutomationStatus::ActiveLimitedHigh.is_active());
        assert!(AutomationStatus::ActiveLimitedHigh.is_limited());
        assert!(!AutomationStatus::Fault.is_active());
        assert!(!AutomationStatus::Fault.is_limited());
        assert!(AutomationStatus::ActiveNotLimited.is_active());
        assert!(!AutomationStatus::ActiveNotLimited.is_limited());
    }

    #[test]
    fn halt_commands_both_axes() {
        let halt = DriveCommand::halt();
        assert_eq!(halt.speed_mps, Some(0.0));
        assert_eq!(halt.curvature_km_inv, Some(0.0));
        // Steering alone must not silently command a speed.
        assert_eq!(DriveCommand::steer(5.0).speed_mps, None);
    }
}
