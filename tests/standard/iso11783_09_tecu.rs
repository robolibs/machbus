use machbus::isobus::implement::{TecuClass, TractorFacilities, TractorFacilitiesRole};
use machbus::isobus::{
    PowerConfig as TecuPowerConfig, TecuClassification, TecuConfig, TecuMaintainPowerRequest,
};
use machbus::j1939::{
    KeySwitchState, MaintainPowerData, MaintainPowerRequest, MaintainPowerRequirement,
    MaintainPowerState, PowerManager, PowerRole, PowerState,
};
use machbus::net::message::Message;
use machbus::net::pgn_defs::{
    PGN_MAINTAIN_POWER, PGN_REQUIRED_TRACTOR_FACILITIES, PGN_TIME_DATE,
    PGN_TRACTOR_FACILITIES_RESPONSE,
};
use machbus::net::{
    BROADCAST_ADDRESS, NULL_ADDRESS, POWER_MAINTAIN_REPEAT_MS, POWER_MAX_EXTENSION_MS,
    POWER_SHUTDOWN_MIN_MS, Priority,
};

#[test]
fn tecu_facility_roles_map_to_distinct_wire_pgns() {
    assert_eq!(
        TractorFacilitiesRole::Required.pgn(),
        PGN_REQUIRED_TRACTOR_FACILITIES
    );
    assert_eq!(
        TractorFacilitiesRole::Response.pgn(),
        PGN_TRACTOR_FACILITIES_RESPONSE
    );
    assert_ne!(
        TractorFacilitiesRole::Required.pgn(),
        TractorFacilitiesRole::Response.pgn()
    );
}

#[test]
fn tecu_classification_and_power_defaults_are_explicit_boundaries() {
    for class in [TecuClass::Class1, TecuClass::Class2, TecuClass::Class3] {
        assert_eq!(TecuClass::try_from_u8(class.as_u8()), Some(class));
        assert_eq!(TecuClass::from_u8(class.as_u8()), Some(class));
    }
    for reserved_class in [0, 4, 5, 0x7F, 0xFF] {
        assert_eq!(TecuClass::try_from_u8(reserved_class), None);
        assert_eq!(TecuClass::from_u8(reserved_class), None);
    }

    let classification = TecuClassification {
        base_class: TecuClass::Class3,
        navigation: true,
        front_mounted: true,
        guidance: true,
        powertrain: true,
        motion_init: true,
        ..Default::default()
    };
    assert_eq!(
        classification.to_string(),
        "Class 3NFGPM",
        "TECU addendum labels must stay in the canonical public order"
    );

    let power = TecuPowerConfig::default();
    assert_eq!(power.shutdown_max_time_ms, 180_000);
    assert_eq!(power.maintain_timeout_ms, 2_000);
    assert_eq!(power.ecu_pwr_current_amps, 15);
    assert_eq!(power.pwr_current_amps, 50);
    assert!(power.is_valid());
    assert!(TecuConfig::default().is_valid());
}

#[test]
fn tecu_classification_gates_facility_advertisement_by_class_and_addendum() {
    let class1 = TecuClassification::default();
    assert!(class1.allows_facilities(&TractorFacilities {
        rear_hitch_position: true,
        rear_pto_speed: true,
        wheel_based_speed: true,
        ..Default::default()
    }));
    assert!(!class1.allows_facilities(&TractorFacilities {
        ground_based_distance: true,
        ..Default::default()
    }));
    assert!(!class1.allows_facilities(&TractorFacilities {
        rear_hitch_command: true,
        ..Default::default()
    }));

    let class2_n = TecuClassification {
        base_class: TecuClass::Class2,
        navigation: true,
        version: 1,
        ..Default::default()
    };
    assert!(class2_n.allows_facilities(&TractorFacilities {
        ground_based_distance: true,
        wheel_based_distance: true,
        navigation: true,
        ..Default::default()
    }));
    assert!(!class2_n.allows_facilities(&TractorFacilities {
        guidance: true,
        ..Default::default()
    }));
    assert!(!class2_n.allows_facilities(&TractorFacilities {
        front_hitch_position: true,
        ..Default::default()
    }));

    let class3_v2_full = TecuClassification {
        base_class: TecuClass::Class3,
        navigation: true,
        front_mounted: true,
        guidance: true,
        powertrain: true,
        version: 2,
        ..Default::default()
    };
    assert!(class3_v2_full.allows_facilities(&TractorFacilities {
        rear_hitch_command: true,
        aux_valve_command: true,
        front_pto_command: true,
        navigation: true,
        guidance: true,
        machine_selected_speed: true,
        machine_selected_speed_command: true,
        rear_hitch_limit_status: true,
        front_pto_exit_code: true,
        ..Default::default()
    }));

    let class3_v1_front_powertrain = TecuClassification {
        base_class: TecuClass::Class3,
        front_mounted: true,
        version: 1,
        ..Default::default()
    };
    assert!(
        !class3_v1_front_powertrain.allows_facilities(&TractorFacilities {
            rear_hitch_limit_status: true,
            ..Default::default()
        })
    );
    assert!(
        !class3_v1_front_powertrain.allows_facilities(&TractorFacilities {
            machine_selected_speed: true,
            ..Default::default()
        })
    );
}

#[test]
fn tecu_classification_advertisable_facility_matrix_is_closed_under_gates() {
    let class1 = TecuClassification::default();
    let class1_facilities = class1
        .advertisable_facilities()
        .expect("valid Class 1 profile should produce an advertisement");
    assert_eq!(
        class1_facilities,
        TractorFacilities::default().with_class1_all()
    );
    assert!(class1.allows_facilities(&class1_facilities));
    let mut class1_overclaim = class1_facilities;
    class1_overclaim.ground_based_distance = true;
    assert!(
        !class1.allows_facilities(&class1_overclaim),
        "Class 1 generated matrix must be closed before Class 2 distance bits"
    );

    let class2_nf = TecuClassification {
        base_class: TecuClass::Class2,
        navigation: true,
        front_mounted: true,
        version: 1,
        ..Default::default()
    };
    let class2_nf_facilities = class2_nf
        .advertisable_facilities()
        .expect("valid Class 2NF profile should produce an advertisement");
    assert!(class2_nf.allows_facilities(&class2_nf_facilities));
    assert!(class2_nf_facilities.ground_based_distance);
    assert!(class2_nf_facilities.wheel_based_direction);
    assert!(class2_nf_facilities.navigation);
    assert!(class2_nf_facilities.front_hitch_position);
    assert!(class2_nf_facilities.front_pto_engagement);
    assert!(!class2_nf_facilities.rear_hitch_command);
    assert!(!class2_nf_facilities.front_hitch_command);
    assert!(!class2_nf_facilities.guidance);
    assert!(!class2_nf_facilities.machine_selected_speed);
    let mut class2_overclaim = class2_nf_facilities;
    class2_overclaim.rear_pto_command = true;
    assert!(
        !class2_nf.allows_facilities(&class2_overclaim),
        "Class 2 generated matrix must not silently permit command bits"
    );

    let class3_v2_full = TecuClassification {
        base_class: TecuClass::Class3,
        navigation: true,
        front_mounted: true,
        guidance: true,
        powertrain: true,
        motion_init: true,
        version: 2,
        instance: 2,
    };
    let class3_v2_facilities = class3_v2_full
        .advertisable_facilities()
        .expect("valid Class 3 v2 profile should produce an advertisement");
    assert!(class3_v2_full.allows_facilities(&class3_v2_facilities));
    assert!(class3_v2_facilities.rear_hitch_command);
    assert!(class3_v2_facilities.aux_valve_command);
    assert!(class3_v2_facilities.front_hitch_command);
    assert!(class3_v2_facilities.front_pto_command);
    assert!(class3_v2_facilities.navigation);
    assert!(class3_v2_facilities.guidance);
    assert!(class3_v2_facilities.machine_selected_speed);
    assert!(class3_v2_facilities.machine_selected_speed_command);
    assert!(class3_v2_facilities.rear_hitch_limit_status);
    assert!(class3_v2_facilities.aux_valve_exit_code);
    assert!(class3_v2_facilities.front_hitch_limit_status);
    assert!(class3_v2_facilities.front_pto_exit_code);

    let invalid_v1_guidance = TecuClassification {
        guidance: true,
        version: 1,
        ..Default::default()
    };
    assert_eq!(
        invalid_v1_guidance.advertisable_facilities(),
        None,
        "invalid local profiles must not be converted into a wire advertisement"
    );
}

#[test]
fn tecu_maintain_power_request_expiry_is_source_scoped_and_saturating() {
    let request = TecuMaintainPowerRequest {
        requester: 0x80,
        ecu_pwr: true,
        pwr: false,
        timestamp_ms: 1_000,
    };

    assert!(!request.is_expired(500, 2_000));
    assert!(!request.is_expired(3_000, 2_000));
    assert!(request.is_expired(3_001, 2_000));
    assert_eq!(request.requester, 0x80);
    assert!(request.ecu_pwr);
    assert!(!request.pwr);
}

#[test]
fn tecu_facility_matrix_round_trips_class_front_guidance_and_powertrain_bits() {
    let facilities = TractorFacilities {
        rear_hitch_position: true,
        rear_pto_speed: true,
        wheel_based_speed: true,
        ground_based_distance: true,
        rear_hitch_command: true,
        aux_valve_command: true,
        front_hitch_position: true,
        front_pto_command: true,
        navigation: true,
        guidance: true,
        machine_selected_speed: true,
        machine_selected_speed_command: true,
        rear_hitch_limit_status: true,
        front_pto_exit_code: true,
        ..TractorFacilities::default()
    };

    let encoded = facilities.encode();
    assert_eq!(encoded[5..], [0xFF, 0xFF, 0xFF]);
    assert_eq!(encoded[4] & 0xC0, 0xC0);
    assert_eq!(TractorFacilities::decode(&encoded), Some(facilities));
}

#[test]
fn tecu_facility_payload_requires_complete_fixed_frame_and_reserved_bits() {
    let facilities = TractorFacilities {
        rear_hitch_position: true,
        rear_pto_command: true,
        aux_valve_exit_code: true,
        front_pto_exit_code: true,
        ..TractorFacilities::default()
    };
    let encoded = facilities.encode();
    assert_eq!(TractorFacilities::decode(&encoded), Some(facilities));

    assert_eq!(TractorFacilities::decode(&encoded[..4]), None);
    assert_eq!(TractorFacilities::decode(&encoded[..7]), None);

    let mut bad_byte4_reserved = encoded;
    bad_byte4_reserved[4] &= 0x3F;
    assert_eq!(TractorFacilities::decode(&bad_byte4_reserved), None);

    for index in [5usize, 6, 7] {
        let mut bad_tail = encoded;
        bad_tail[index] = 0x00;
        assert_eq!(TractorFacilities::decode(&bad_tail), None);
    }
}

#[test]
fn tecu_maintain_power_requires_canonical_fixed_frame_and_valid_source() {
    let request = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::Active,
        implement_ready_to_work_state: MaintainPowerState::Active,
        implement_transport_state: MaintainPowerState::Active,
        maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        timestamp_us: 0,
    };
    let encoded = request.encode();
    assert_eq!(
        MaintainPowerData::decode(&encoded)
            .unwrap()
            .maintain_ecu_power,
        MaintainPowerRequirement::RequirementFor2SecondsMore
    );

    let mut bad_low_reserved = encoded;
    bad_low_reserved[0] &= 0xFE;
    assert_eq!(MaintainPowerData::decode(&bad_low_reserved), None);

    let mut bad_tail = encoded;
    bad_tail[2] = 0x00;
    assert_eq!(MaintainPowerData::decode(&bad_tail), None);

    let mut valid_helper = Message::new(PGN_MAINTAIN_POWER, encoded.to_vec(), 0x80);
    valid_helper.timestamp_us = 123;
    assert_eq!(
        MaintainPowerData::from_message(&valid_helper)
            .expect("valid maintain-power message should decode")
            .timestamp_us,
        123
    );
    for msg in [
        Message::new(PGN_TIME_DATE, encoded.to_vec(), 0x80),
        Message::new(PGN_MAINTAIN_POWER, encoded.to_vec(), NULL_ADDRESS),
        Message::new(PGN_MAINTAIN_POWER, encoded.to_vec(), BROADCAST_ADDRESS),
    ] {
        assert_eq!(
            MaintainPowerData::from_message(&msg),
            None,
            "Maintain Power full-message helper must bind PGN and source before decoding"
        );
    }

    let mut tecu = PowerManager::new(PowerRole::Tecu);
    tecu.key_off();
    tecu.update(1_000);
    let null_request = Message::new(PGN_MAINTAIN_POWER, encoded.to_vec(), NULL_ADDRESS);
    let broadcast_request = Message::new(PGN_MAINTAIN_POWER, encoded.to_vec(), BROADCAST_ADDRESS);
    let destination_specific_request = Message::with_addressing(
        PGN_MAINTAIN_POWER,
        encoded.to_vec(),
        0x80,
        0x42,
        Priority::Default,
    );
    tecu.handle_message(&null_request);
    tecu.handle_message(&broadcast_request);
    tecu.handle_message(&destination_specific_request);
    tecu.update(1_000);
    assert_eq!(tecu.state(), PowerState::PowerOff);

    let mut tecu = PowerManager::new(PowerRole::Tecu);
    tecu.key_off();
    tecu.update(1_000);
    let valid_request = Message::new(PGN_MAINTAIN_POWER, encoded.to_vec(), 0x80);
    tecu.handle_message(&valid_request);
    tecu.update(1_000);
    assert_eq!(tecu.state(), PowerState::Maintaining);
}

#[test]
fn tecu_maintain_power_public_decoders_reject_noncanonical_packed_bytes() {
    for (raw, state) in [
        (0, MaintainPowerState::Inactive),
        (1, MaintainPowerState::Active),
        (2, MaintainPowerState::Error),
        (3, MaintainPowerState::NotAvailable),
    ] {
        assert_eq!(MaintainPowerState::try_from_u8(raw), Some(state));
        assert_eq!(MaintainPowerState::from_u8(raw), state);
    }

    for (raw, requirement) in [
        (0, MaintainPowerRequirement::NoFurtherRequirement),
        (1, MaintainPowerRequirement::RequirementFor2SecondsMore),
        (2, MaintainPowerRequirement::Error),
        (3, MaintainPowerRequirement::DontCare),
    ] {
        assert_eq!(
            MaintainPowerRequirement::try_from_u8(raw),
            Some(requirement)
        );
        assert_eq!(MaintainPowerRequirement::from_u8(raw), requirement);
    }

    for (raw, key) in [
        (0, KeySwitchState::Off),
        (1, KeySwitchState::NotOff),
        (2, KeySwitchState::Error),
        (3, KeySwitchState::NotAvailable),
    ] {
        assert_eq!(KeySwitchState::try_from_u8(raw), Some(key));
        assert_eq!(KeySwitchState::from_u8(raw), key);
    }

    for (raw, request) in [
        (0, MaintainPowerRequest::NoRequest),
        (1, MaintainPowerRequest::EcuRequest),
        (2, MaintainPowerRequest::Error),
        (3, MaintainPowerRequest::NotAvailable),
    ] {
        assert_eq!(MaintainPowerRequest::try_from_u8(raw), Some(request));
        assert_eq!(MaintainPowerRequest::from_u8(raw), request);
    }

    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(MaintainPowerState::try_from_u8(packed_or_reserved), None);
        assert_eq!(
            MaintainPowerRequirement::try_from_u8(packed_or_reserved),
            None
        );
        assert_eq!(KeySwitchState::try_from_u8(packed_or_reserved), None);
        assert_eq!(MaintainPowerRequest::try_from_u8(packed_or_reserved), None);
    }

    let frame = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::NotAvailable,
        implement_ready_to_work_state: MaintainPowerState::Inactive,
        implement_transport_state: MaintainPowerState::Error,
        maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        maintain_ecu_power: MaintainPowerRequirement::DontCare,
        timestamp_us: 0,
    };
    assert_eq!(MaintainPowerData::decode(&frame.encode()), Some(frame));
}

#[test]
fn tecu_maintain_power_payload_field_extraction_uses_strict_decoders() {
    let raw = [0x9F, 0x67, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let decoded = MaintainPowerData::decode(&raw).unwrap();

    assert_eq!(decoded.implement_in_work_state, MaintainPowerState::Active);
    assert_eq!(decoded.implement_park_state, MaintainPowerState::Error);
    assert_eq!(
        decoded.implement_ready_to_work_state,
        MaintainPowerState::NotAvailable
    );
    assert_eq!(
        decoded.implement_transport_state,
        MaintainPowerState::Active
    );
    assert_eq!(
        decoded.maintain_actuator_power,
        MaintainPowerRequirement::Error
    );
    assert_eq!(
        decoded.maintain_ecu_power,
        MaintainPowerRequirement::RequirementFor2SecondsMore
    );

    assert_eq!(MaintainPowerData::decode(&decoded.encode()), Some(decoded));
}

#[test]
fn tecu_maintain_power_shutdown_window_requires_fresh_requests() {
    let request = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::Active,
        implement_ready_to_work_state: MaintainPowerState::Active,
        implement_transport_state: MaintainPowerState::Active,
        maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        timestamp_us: 0,
    };
    let request_message = Message::new(PGN_MAINTAIN_POWER, request.encode().to_vec(), 0x80);

    let mut tecu = PowerManager::new(PowerRole::Tecu);
    tecu.key_off();
    tecu.update(POWER_SHUTDOWN_MIN_MS / 2);
    tecu.handle_message(&request_message);
    tecu.update(POWER_SHUTDOWN_MIN_MS / 2);
    assert_eq!(
        tecu.state(),
        PowerState::Maintaining,
        "a fresh maintain request during the shutdown hold must enter maintained power"
    );

    tecu.update(POWER_MAINTAIN_REPEAT_MS - 1);
    assert_eq!(
        tecu.state(),
        PowerState::Maintaining,
        "maintained power must remain active until the request freshness window expires"
    );
    tecu.update(1);
    assert_eq!(
        tecu.state(),
        PowerState::PowerOff,
        "stale maintain requests must not extend TECU power indefinitely"
    );

    let mut cf = PowerManager::new(PowerRole::Cf);
    assert!(cf.update(POWER_MAINTAIN_REPEAT_MS).is_empty());
    cf.request_power(true);
    assert_eq!(
        cf.update(0).len(),
        1,
        "a CF power request should be broadcast on the next update"
    );
    assert!(
        cf.update(POWER_MAINTAIN_REPEAT_MS - 1).is_empty(),
        "CF maintain requests must obey the repeat cadence"
    );
    assert_eq!(cf.update(1).len(), 1);
    cf.request_power(false);
    assert!(
        cf.update(POWER_MAINTAIN_REPEAT_MS).is_empty(),
        "clearing the CF request must stop further maintain-power broadcasts"
    );
}

#[test]
fn tecu_maintain_power_accepts_either_power_rail_request_before_shutdown_expiry() {
    let mut actuator_only = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::Active,
        implement_ready_to_work_state: MaintainPowerState::Active,
        implement_transport_state: MaintainPowerState::Active,
        maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        maintain_ecu_power: MaintainPowerRequirement::NoFurtherRequirement,
        timestamp_us: 0,
    };

    for (label, request) in [("actuator rail", actuator_only), {
        actuator_only.maintain_actuator_power = MaintainPowerRequirement::NoFurtherRequirement;
        actuator_only.maintain_ecu_power = MaintainPowerRequirement::RequirementFor2SecondsMore;
        ("ECU rail", actuator_only)
    }] {
        let mut tecu = PowerManager::new(PowerRole::Tecu);
        tecu.key_off();
        tecu.update(POWER_SHUTDOWN_MIN_MS / 2);
        tecu.handle_message(&Message::new(
            PGN_MAINTAIN_POWER,
            request.encode().to_vec(),
            0x80,
        ));
        tecu.update(POWER_SHUTDOWN_MIN_MS / 2);
        assert_eq!(
            tecu.state(),
            PowerState::Maintaining,
            "{label} maintain request must be enough to extend the shutdown window"
        );
    }

    let no_extension = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::Active,
        implement_ready_to_work_state: MaintainPowerState::Active,
        implement_transport_state: MaintainPowerState::Active,
        maintain_actuator_power: MaintainPowerRequirement::NoFurtherRequirement,
        maintain_ecu_power: MaintainPowerRequirement::NoFurtherRequirement,
        timestamp_us: 0,
    };
    let mut tecu = PowerManager::new(PowerRole::Tecu);
    tecu.key_off();
    tecu.update(POWER_SHUTDOWN_MIN_MS / 2);
    tecu.handle_message(&Message::new(
        PGN_MAINTAIN_POWER,
        no_extension.encode().to_vec(),
        0x80,
    ));
    tecu.update(POWER_SHUTDOWN_MIN_MS / 2);
    assert_eq!(
        tecu.state(),
        PowerState::PowerOff,
        "a canonical maintain-power frame without either rail request must not extend shutdown"
    );
}

#[test]
fn tecu_maintain_power_maximum_extension_limit_overrides_fresh_requests() {
    let request = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::Active,
        implement_ready_to_work_state: MaintainPowerState::Active,
        implement_transport_state: MaintainPowerState::Active,
        maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        timestamp_us: 0,
    };
    let request_message = Message::new(PGN_MAINTAIN_POWER, request.encode().to_vec(), 0x80);

    let mut tecu = PowerManager::new(PowerRole::Tecu);
    tecu.key_off();
    let mut elapsed_ms = 0;
    while elapsed_ms < POWER_MAX_EXTENSION_MS {
        tecu.handle_message(&request_message);
        tecu.update(1_000);
        elapsed_ms += 1_000;
        if tecu.state() == PowerState::PowerOff {
            break;
        }
    }

    assert_eq!(
        tecu.state(),
        PowerState::PowerOff,
        "fresh Maintain Power requests must not extend TECU power beyond the maximum extension window"
    );
}

#[test]
fn tecu_maintain_power_ignores_wrong_pgn_before_shutdown_state_extension() {
    let request = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::Active,
        implement_ready_to_work_state: MaintainPowerState::Active,
        implement_transport_state: MaintainPowerState::Active,
        maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        timestamp_us: 0,
    }
    .encode();

    let mut tecu = PowerManager::new(PowerRole::Tecu);
    tecu.key_off();
    tecu.update(1_000);
    tecu.handle_message(&Message::new(
        PGN_TRACTOR_FACILITIES_RESPONSE,
        request.to_vec(),
        0x80,
    ));
    tecu.update(1_000);

    assert_eq!(tecu.state(), PowerState::PowerOff);
}
