use machbus::net::{
    CanBusConfig, ISO_CAN_BITRATE, ISO_SAMPLE_POINT_MAX, ISO_SAMPLE_POINT_MIN,
    enforce_iso_can_config, validate_can_bus_config,
};
#[test]
fn physical_layer_accepts_only_the_supported_iso_bus_profile() {
    let default = CanBusConfig::default();
    assert_eq!(default.bitrate, ISO_CAN_BITRATE);
    assert!(validate_can_bus_config(&default).overall_ok);
    assert!(enforce_iso_can_config(&default).is_ok());

    for sample_point in [ISO_SAMPLE_POINT_MIN, 0.80, ISO_SAMPLE_POINT_MAX] {
        let config = CanBusConfig::default().sample_point(sample_point);
        assert!(
            validate_can_bus_config(&config).sample_point_ok,
            "sample point {sample_point} should be accepted by the configured supported profile"
        );
    }
}

#[test]
fn physical_layer_rejects_unsupported_bitrate_before_runtime_use() {
    let config = CanBusConfig::default().bitrate(500_000);
    let validation = validate_can_bus_config(&config);
    assert!(!validation.bitrate_ok);
    assert!(!validation.overall_ok);

    let err = enforce_iso_can_config(&config).expect_err("unsupported bitrate must fail");
    assert_eq!(err.code, machbus::net::ErrorCode::InvalidState);
    assert!(err.message.contains("250000"));
}

#[test]
fn physical_layer_rejects_sample_points_outside_supported_window_before_runtime_use() {
    for sample_point in [
        ISO_SAMPLE_POINT_MIN - 0.000_001,
        ISO_SAMPLE_POINT_MAX + 0.000_001,
        0.0,
        1.0,
    ] {
        let config = CanBusConfig::default().sample_point(sample_point);
        let validation = validate_can_bus_config(&config);

        assert!(!validation.sample_point_ok);
        assert!(!validation.overall_ok);

        let err = enforce_iso_can_config(&config)
            .expect_err("out-of-window sample point must not satisfy a physical-bus claim");
        assert_eq!(err.code, machbus::net::ErrorCode::InvalidState);
        assert!(err.message.contains("sample point"));
    }
}

#[test]
fn physical_layer_rejects_local_only_modes_for_compliant_bus_claims() {
    for config in [
        CanBusConfig::default().silent(true),
        CanBusConfig::default().loopback(true),
        CanBusConfig::default().silent(true).loopback(true),
    ] {
        let validation = validate_can_bus_config(&config);
        assert!(!validation.physical_mode_ok);
        assert!(!validation.overall_ok);

        let err = enforce_iso_can_config(&config)
            .expect_err("local-only interface modes must not satisfy a physical-bus claim");
        assert_eq!(err.code, machbus::net::ErrorCode::InvalidState);
        assert!(err.message.contains("silent and loopback"));
    }
}

#[test]
fn physical_layer_rejects_non_finite_sample_points_before_compliance_claims() {
    for sample_point in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let config = CanBusConfig::default().sample_point(sample_point);
        let validation = validate_can_bus_config(&config);
        assert!(!validation.sample_point_ok);
        assert!(!validation.overall_ok);

        let err = enforce_iso_can_config(&config)
            .expect_err("non-finite sample point must not satisfy a physical-bus claim");
        assert_eq!(err.code, machbus::net::ErrorCode::InvalidState);
        assert!(err.message.contains("sample point"));
    }
}

#[test]
fn physical_layer_rejects_inconsistent_explicit_bit_timing_segments() {
    let explicit_good = CanBusConfig {
        prop_seg: 7,
        phase_seg1: 8,
        phase_seg2: 4,
        ..CanBusConfig::default()
    };
    let validation = validate_can_bus_config(&explicit_good);
    assert!(validation.sample_point_ok);
    assert!(validation.bit_timing_ok);
    assert!(validation.overall_ok);
    assert!(enforce_iso_can_config(&explicit_good).is_ok());

    for config in [
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
        let validation = validate_can_bus_config(&config);
        assert!(
            !validation.bit_timing_ok,
            "incomplete, zero-SJW, SJW-over-phase2, or sample-point-mismatched timing must fail"
        );
        assert!(!validation.overall_ok);

        let err = enforce_iso_can_config(&config)
            .expect_err("invalid explicit bit timing must not satisfy a physical-bus claim");
        assert_eq!(err.code, machbus::net::ErrorCode::InvalidState);
        assert!(err.message.contains("bit timing"));
    }
}
