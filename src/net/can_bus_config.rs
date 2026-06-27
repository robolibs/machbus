//! ISO 11783-2 physical-layer CAN bus configuration and compliance
//! validation.
//!
//! Mirrors the C++ `machbus::net::can_bus_config`. ISO 11783-2
//! mandates 250 kbit/s and an 80 % ± 3 % sample point — anything
//! else is non-compliant and rejected by [`enforce_iso_can_config`].

use alloc::string::{String, ToString};

use super::error::{Error, Result};

// ─── ISO 11783-2 mandated values ───────────────────────────────────────
pub const ISO_CAN_BITRATE: u32 = 250_000;
pub const ISO_SAMPLE_POINT_NOMINAL: f64 = 0.80;
pub const ISO_SAMPLE_POINT_MIN: f64 = 0.77;
pub const ISO_SAMPLE_POINT_MAX: f64 = 0.83;

/// CAN bus physical-layer parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanBusConfig {
    pub bitrate: u32,
    pub sample_point: f64,
    /// Synchronization Jump Width.
    pub sjw: u8,
    pub prop_seg: u8,
    pub phase_seg1: u8,
    pub phase_seg2: u8,
    pub silent_mode: bool,
    pub loopback: bool,
}

impl Default for CanBusConfig {
    fn default() -> Self {
        Self {
            bitrate: ISO_CAN_BITRATE,
            sample_point: ISO_SAMPLE_POINT_NOMINAL,
            sjw: 1,
            prop_seg: 0,
            phase_seg1: 0,
            phase_seg2: 0,
            silent_mode: false,
            loopback: false,
        }
    }
}

impl CanBusConfig {
    #[must_use]
    pub const fn bitrate(mut self, br: u32) -> Self {
        self.bitrate = br;
        self
    }

    #[must_use]
    pub const fn sample_point(mut self, sp: f64) -> Self {
        self.sample_point = sp;
        self
    }

    #[must_use]
    pub const fn sjw(mut self, s: u8) -> Self {
        self.sjw = s;
        self
    }

    #[must_use]
    pub const fn silent(mut self, s: bool) -> Self {
        self.silent_mode = s;
        self
    }

    #[must_use]
    pub const fn loopback(mut self, l: bool) -> Self {
        self.loopback = l;
        self
    }
}

/// Result of ISO 11783-2 compliance validation.
#[derive(Debug, Clone, PartialEq)]
pub struct CanBusValidation {
    pub bitrate_ok: bool,
    pub sample_point_ok: bool,
    pub bit_timing_ok: bool,
    pub physical_mode_ok: bool,
    pub overall_ok: bool,
    pub error_message: String,
}

/// Validate `config` against ISO 11783-2 §6.3 / §6.4.
#[must_use]
pub fn validate_can_bus_config(config: &CanBusConfig) -> CanBusValidation {
    let bitrate_ok = config.bitrate == ISO_CAN_BITRATE;
    let sample_point_ok = config.sample_point.is_finite()
        && config.sample_point >= ISO_SAMPLE_POINT_MIN
        && config.sample_point <= ISO_SAMPLE_POINT_MAX;
    let bit_timing_ok = validate_bit_timing_segments(config);
    let physical_mode_ok = !config.silent_mode && !config.loopback;
    let overall_ok = bitrate_ok && sample_point_ok && bit_timing_ok && physical_mode_ok;

    let error_message = if !bitrate_ok {
        "bitrate must be 250000".to_string()
    } else if !config.sample_point.is_finite() {
        "sample point must be finite".to_string()
    } else if !sample_point_ok {
        "sample point must be 80% +/- 3%".to_string()
    } else if !bit_timing_ok {
        "explicit bit timing segments must be complete and match the sample point window"
            .to_string()
    } else if !physical_mode_ok {
        "silent and loopback modes are not supported for an ISO 11783 physical bus".to_string()
    } else {
        String::new()
    };

    if !overall_ok {
        tracing::warn!(
            target: "machbus.can_bus",
            reason = %error_message,
            "CAN bus config non-compliant",
        );
    }

    CanBusValidation {
        bitrate_ok,
        sample_point_ok,
        bit_timing_ok,
        physical_mode_ok,
        overall_ok,
        error_message,
    }
}

fn validate_bit_timing_segments(config: &CanBusConfig) -> bool {
    if config.sjw == 0 {
        return false;
    }

    let segments = [config.prop_seg, config.phase_seg1, config.phase_seg2];
    let explicit_segments = segments.iter().any(|&segment| segment != 0);
    if !explicit_segments {
        return true;
    }
    if segments.contains(&0) || config.sjw > config.phase_seg2 {
        return false;
    }

    let total_time_quanta = u16::from(config.prop_seg)
        + u16::from(config.phase_seg1)
        + u16::from(config.phase_seg2)
        + 1;
    if total_time_quanta <= 1 {
        return false;
    }

    let sample_time_quanta = u16::from(config.prop_seg) + u16::from(config.phase_seg1) + 1;
    let derived_sample_point = f64::from(sample_time_quanta) / f64::from(total_time_quanta);
    (ISO_SAMPLE_POINT_MIN..=ISO_SAMPLE_POINT_MAX).contains(&derived_sample_point)
        && (derived_sample_point - config.sample_point).abs() <= 0.001
}

/// Reject non-compliant configs with an [`Error::invalid_state`].
pub fn enforce_iso_can_config(config: &CanBusConfig) -> Result<()> {
    let v = validate_can_bus_config(config);
    if !v.overall_ok {
        return Err(Error::invalid_state(v.error_message));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_compliant() {
        let cfg = CanBusConfig::default();
        let v = validate_can_bus_config(&cfg);
        assert!(v.overall_ok);
        assert!(enforce_iso_can_config(&cfg).is_ok());
    }

    #[test]
    fn wrong_bitrate_rejected() {
        let cfg = CanBusConfig::default().bitrate(500_000);
        let v = validate_can_bus_config(&cfg);
        assert!(!v.bitrate_ok);
        assert!(!v.overall_ok);
        assert!(enforce_iso_can_config(&cfg).is_err());
    }

    #[test]
    fn sample_point_just_inside_window() {
        let lower = CanBusConfig::default().sample_point(0.77);
        let upper = CanBusConfig::default().sample_point(0.83);
        assert!(validate_can_bus_config(&lower).sample_point_ok);
        assert!(validate_can_bus_config(&upper).sample_point_ok);
    }

    #[test]
    fn sample_point_just_outside_window_rejected() {
        let too_low = CanBusConfig::default().sample_point(0.76);
        let too_high = CanBusConfig::default().sample_point(0.84);
        assert!(!validate_can_bus_config(&too_low).sample_point_ok);
        assert!(!validate_can_bus_config(&too_high).sample_point_ok);
    }

    #[test]
    fn fluent_setters_chain() {
        let cfg = CanBusConfig::default()
            .bitrate(250_000)
            .sample_point(0.80)
            .sjw(2)
            .silent(true)
            .loopback(true);
        assert_eq!(cfg.sjw, 2);
        assert!(cfg.silent_mode);
        assert!(cfg.loopback);
    }

    #[test]
    fn silent_and_loopback_modes_are_not_compliant_physical_bus_configs() {
        for cfg in [
            CanBusConfig::default().silent(true),
            CanBusConfig::default().loopback(true),
            CanBusConfig::default().silent(true).loopback(true),
        ] {
            let validation = validate_can_bus_config(&cfg);
            assert!(!validation.physical_mode_ok);
            assert!(!validation.overall_ok);
            assert!(validation.error_message.contains("silent and loopback"));
            assert!(enforce_iso_can_config(&cfg).is_err());
        }
    }

    #[test]
    fn explicit_bit_timing_segments_must_be_complete_and_consistent() {
        let compliant = CanBusConfig {
            prop_seg: 7,
            phase_seg1: 8,
            phase_seg2: 4,
            ..CanBusConfig::default()
        };
        assert!(validate_can_bus_config(&compliant).bit_timing_ok);
        assert!(enforce_iso_can_config(&compliant).is_ok());

        for cfg in [
            CanBusConfig {
                prop_seg: 7,
                phase_seg1: 0,
                phase_seg2: 4,
                ..CanBusConfig::default()
            },
            CanBusConfig {
                sjw: 0,
                ..CanBusConfig::default()
            },
            CanBusConfig {
                sjw: 5,
                prop_seg: 7,
                phase_seg1: 8,
                phase_seg2: 4,
                ..CanBusConfig::default()
            },
            CanBusConfig {
                prop_seg: 1,
                phase_seg1: 1,
                phase_seg2: 8,
                ..CanBusConfig::default()
            },
        ] {
            let validation = validate_can_bus_config(&cfg);
            assert!(!validation.bit_timing_ok);
            assert!(!validation.overall_ok);
            assert!(validation.error_message.contains("bit timing"));
            assert!(enforce_iso_can_config(&cfg).is_err());
        }
    }
}
