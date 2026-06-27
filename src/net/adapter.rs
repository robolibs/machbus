//! CAN adapter capability model (ISO 11783-2 physical layer).
//!
//! GAP.md (ISO 11783-2) notes "No adapter capability model beyond the
//! current matrix/playbook". The `can_adapter_matrix.txt` evidence file is
//! planning metadata; this module is the typed, queryable capability model
//! a setup/evidence path can reason about programmatically: what a CAN
//! adapter supports, and whether that is enough to participate on an
//! ISO 11783 network and to collect repository evidence.
//!
//! It is pure data + checks — no driver I/O — so it stays testable and
//! hardware-independent. Real capture evidence (which the user defers)
//! still requires hardware; this models the *requirements* against which
//! such hardware is judged.

use crate::net::can_bus_config::ISO_CAN_BITRATE;

/// What a CAN adapter can do, as advertised by its driver/datasheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdapterCapabilities {
    /// Can be configured to the ISO 11783 bitrate (250 kbit/s).
    pub supports_iso_bitrate: bool,
    /// Supports listen-only / silent mode (required for safe passive
    /// capture without perturbing the bus).
    pub listen_only: bool,
    /// Reports CAN error frames to software (needed for error-handling
    /// evidence).
    pub error_frame_reporting: bool,
    /// Provides hardware receive timestamps (needed for timing evidence).
    pub hardware_timestamping: bool,
    /// Recovers automatically from the bus-off state.
    pub auto_bus_off_recovery: bool,
}

impl AdapterCapabilities {
    /// A capability set advertising everything (e.g. a full-featured
    /// SocketCAN device); handy as an API/test baseline.
    #[must_use]
    pub const fn full() -> Self {
        Self {
            supports_iso_bitrate: true,
            listen_only: true,
            error_frame_reporting: true,
            hardware_timestamping: true,
            auto_bus_off_recovery: true,
        }
    }
}

/// One capability requirement and whether the adapter meets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityCheck {
    pub name: &'static str,
    /// `true` if the requirement is mandatory for ISO 11783 participation.
    pub required: bool,
    pub met: bool,
}

/// Readiness assessment of an adapter against ISO 11783 needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterReadiness {
    pub checks: Vec<CapabilityCheck>,
}

impl AdapterReadiness {
    /// `true` if every *required* capability is met (the adapter can
    /// participate on an ISO 11783 network).
    #[must_use]
    pub fn iso_capable(&self) -> bool {
        self.checks.iter().all(|c| !c.required || c.met)
    }

    /// `true` if every capability — required and recommended — is met (the
    /// adapter is also suitable for full capture evidence).
    #[must_use]
    pub fn evidence_capable(&self) -> bool {
        self.checks.iter().all(|c| c.met)
    }

    /// Names of unmet capabilities (required or recommended).
    #[must_use]
    pub fn missing(&self) -> Vec<&'static str> {
        self.checks
            .iter()
            .filter(|c| !c.met)
            .map(|c| c.name)
            .collect()
    }
}

impl AdapterCapabilities {
    /// Assess this adapter against the ISO 11783 physical-layer needs.
    ///
    /// The ISO bitrate is the one hard requirement to participate; the
    /// rest are recommended and gate *evidence* quality rather than
    /// participation.
    #[must_use]
    pub fn iso11783_readiness(&self) -> AdapterReadiness {
        AdapterReadiness {
            checks: vec![
                CapabilityCheck {
                    name: "iso-bitrate-250k",
                    required: true,
                    met: self.supports_iso_bitrate,
                },
                CapabilityCheck {
                    name: "listen-only",
                    required: false,
                    met: self.listen_only,
                },
                CapabilityCheck {
                    name: "error-frame-reporting",
                    required: false,
                    met: self.error_frame_reporting,
                },
                CapabilityCheck {
                    name: "hardware-timestamping",
                    required: false,
                    met: self.hardware_timestamping,
                },
                CapabilityCheck {
                    name: "auto-bus-off-recovery",
                    required: false,
                    met: self.auto_bus_off_recovery,
                },
            ],
        }
    }
}

/// The ISO 11783 bitrate an adapter must support to participate.
pub const REQUIRED_ADAPTER_BITRATE: u32 = ISO_CAN_BITRATE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_adapter_is_iso_and_evidence_capable() {
        let r = AdapterCapabilities::full().iso11783_readiness();
        assert!(r.iso_capable());
        assert!(r.evidence_capable());
        assert!(r.missing().is_empty());
        assert_eq!(REQUIRED_ADAPTER_BITRATE, 250_000);
    }

    #[test]
    fn bitrate_is_the_only_hard_requirement() {
        // Only the ISO bitrate: participates, but not evidence-grade.
        let caps = AdapterCapabilities {
            supports_iso_bitrate: true,
            ..Default::default()
        };
        let r = caps.iso11783_readiness();
        assert!(
            r.iso_capable(),
            "ISO bitrate alone is enough to participate"
        );
        assert!(!r.evidence_capable(), "missing capture capabilities");
        assert!(r.missing().contains(&"listen-only"));
        assert!(r.missing().contains(&"hardware-timestamping"));
    }

    #[test]
    fn without_iso_bitrate_the_adapter_is_not_capable() {
        let r = AdapterCapabilities::default().iso11783_readiness();
        assert!(!r.iso_capable());
        assert!(r.missing().contains(&"iso-bitrate-250k"));
    }
}
