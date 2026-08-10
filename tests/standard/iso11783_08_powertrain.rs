use machbus::isobus::implement::Signal;
use machbus::j1939::{
    Aftertreatment1, Aftertreatment2, AmbientConditions, ComponentIdentification, CruiseControl,
    DashDisplay, Eec1, Eec2, Eec3, EngineFluidLp, EngineHours, EngineTemp1, EngineTemp2, Etc1,
    FuelConsumption, FuelEconomy, OverrideControlMode, SpeedAndDistance, TransmissionOilTemp, Tsc1,
    VehicleIdentification, VehiclePosition, Vep1,
};
use machbus::net::message::Message;
use machbus::net::pgn_defs::{
    PGN_AMBIENT_CONDITIONS, PGN_AT1, PGN_AT2, PGN_COMPONENT_ID, PGN_DASH_DISPLAY, PGN_EEC1,
    PGN_EEC2, PGN_EEC3, PGN_EFLP, PGN_ENGINE_HOURS, PGN_ET1, PGN_ET2, PGN_ETC1,
    PGN_FUEL_CONSUMPTION, PGN_FUEL_ECONOMY, PGN_TSC1, PGN_VEHICLE_ID, PGN_VEHICLE_POSITION,
    PGN_VEP1,
};
use machbus::net::{BROADCAST_ADDRESS, NULL_ADDRESS, Priority};

fn assert_close(left: f64, right: f64) {
    assert!(
        (left - right).abs() < 0.000_001,
        "{left} should be close to {right}"
    );
}

/// A `Signal` that must carry a value close to `right`. A parameter reported as
/// error or not-available is a distinct outcome and fails here.
fn assert_signal_close(left: Signal<f64>, right: f64) {
    let value = left
        .value()
        .unwrap_or_else(|| panic!("expected a measurement close to {right}, got {left:?}"));
    assert_close(value, right);
}

#[test]
fn powertrain_eec1_rejects_non_ff_reserved_tail_and_preserves_scaling() {
    let eec1 = Eec1 {
        engine_torque_percent: Signal::Value(12.0),
        driver_demand_percent: Signal::Value(-10.0),
        actual_engine_percent: Signal::Value(20.0),
        engine_speed_rpm: Signal::Value(1_500.0),
        starter_mode: 3,
        source_address: 0x80,
    };
    let encoded = eec1.encode();
    let decoded = Eec1::decode(&encoded).unwrap();

    assert_signal_close(decoded.engine_torque_percent, 12.0);
    assert_signal_close(decoded.driver_demand_percent, -10.0);
    assert_signal_close(decoded.actual_engine_percent, 20.0);
    assert_signal_close(decoded.engine_speed_rpm, 1_500.0);
    assert_eq!(decoded.starter_mode, 3);
    assert_eq!(decoded.source_address, 0x80);

    let mut bad_tail = encoded;
    bad_tail[7] = 0;
    assert_eq!(Eec1::decode(&bad_tail), None);

    let mut bad_starter_reserved = encoded;
    bad_starter_reserved[6] |= 0xF0;
    assert_eq!(Eec1::decode(&bad_starter_reserved), None);
}

#[test]
fn powertrain_eec1_reports_sentinels_as_absent_without_scaling_them() {
    let encoded = Eec1 {
        engine_torque_percent: Signal::Value(0.0),
        driver_demand_percent: Signal::Value(1.0),
        actual_engine_percent: Signal::Value(2.0),
        engine_speed_rpm: Signal::Value(1_500.0),
        starter_mode: 0,
        source_address: 0x80,
    }
    .encode();
    assert!(Eec1::decode(&encoded).is_some());

    // H54 — a sentinel must never be *scaled into a number*, but it must also
    // not cost the rest of the PG: the other parameters in the same frame are
    // still perfectly valid, and a faulted sensor has to stay distinguishable
    // from an unplugged bus (G4).
    for index in [0usize, 1, 2] {
        let mut absent = encoded;
        absent[index] = 0xFF;
        let decoded = Eec1::decode(&absent)
            .unwrap_or_else(|| panic!("byte {index} absent must not drop the frame"));
        let fields = [
            decoded.engine_torque_percent,
            decoded.driver_demand_percent,
            decoded.actual_engine_percent,
        ];
        assert_eq!(
            fields[index],
            Signal::NotAvailable,
            "EEC1 percent byte {index} must report absent, not a scaled value"
        );
        for (other, signal) in fields.iter().enumerate() {
            if other != index {
                assert!(
                    signal.value().is_some(),
                    "byte {other} was reported and must survive"
                );
            }
        }
        assert_signal_close(decoded.engine_speed_rpm, 1_500.0);
    }

    // The error indicator is a distinct outcome from not-available.
    let mut faulted = encoded;
    faulted[0] = 0xFE;
    assert_eq!(
        Eec1::decode(&faulted).unwrap().engine_torque_percent,
        Signal::Error
    );

    let mut absent_speed = encoded;
    absent_speed[3] = 0xFF;
    absent_speed[4] = 0xFF;
    let decoded = Eec1::decode(&absent_speed).expect("an absent speed keeps the frame");
    assert_eq!(decoded.engine_speed_rpm, Signal::NotAvailable);
    assert_signal_close(decoded.driver_demand_percent, 1.0);
}

#[test]
fn powertrain_eec2_rejects_reserved_control_nibble_without_losing_status_edges() {
    // Raw 0xFE is the error indicator, which these fields used to carry as a
    // plain number because they were raw bytes.
    let eec2 = Eec2 {
        accel_pedal_position: Signal::Error,
        engine_load_percent: Signal::Value(250.0),
        accel_pedal_low_idle: 3,
        accel_pedal_kickdown: 3,
        road_speed_limit: Signal::Error,
    };
    let encoded = eec2.encode();
    assert_eq!(encoded[1], 0xFE);
    assert_eq!(Eec2::decode(&encoded), Some(eec2));

    let mut bad_reserved_nibble = encoded;
    bad_reserved_nibble[0] |= 0x10;
    assert_eq!(
        Eec2::decode(&bad_reserved_nibble),
        None,
        "EEC2 must reject the unused high nibble before interpreting control-status bits"
    );

    let mut bad_tail = encoded;
    bad_tail[4] = 0x00;
    assert_eq!(
        Eec2::decode(&bad_tail),
        None,
        "EEC2 must remain an exact fixed-width payload with canonical unused tail bytes"
    );
}

#[test]
fn powertrain_eec2_optional_scalars_reject_reserved_special_values() {
    // Raw 125 at the SPN 91 resolution of 0.4 %/bit is 50 % pedal.
    let eec2 = Eec2 {
        accel_pedal_position: Signal::Value(50.0),
        engine_load_percent: Signal::Value(50.0),
        accel_pedal_low_idle: 1,
        accel_pedal_kickdown: 2,
        road_speed_limit: Signal::Value(80.0),
    };
    let encoded = eec2.encode();
    assert_eq!(encoded[1], 125);
    assert_eq!(Eec2::decode(&encoded), Some(eec2));

    let mut status_values = encoded;
    status_values[1] = 0xFE;
    status_values[3] = 0xFF;
    let decoded = Eec2::decode(&status_values).unwrap();
    assert_eq!(decoded.accel_pedal_position, Signal::Error);
    assert_eq!(decoded.road_speed_limit, Signal::NotAvailable);
    assert_signal_close(decoded.engine_load_percent, 50.0);

    // A reserved raw is reported as absent, never scaled into a number, and
    // never at the cost of the rest of the PG.
    for (index, value) in [(1usize, 0xFB), (1, 0xFD), (3, 0xFB), (3, 0xFD)] {
        let mut reserved = encoded;
        reserved[index] = value;
        let decoded = Eec2::decode(&reserved).unwrap_or_else(|| {
            panic!("reserved raw 0x{value:02X} in byte {index} must not drop the PG")
        });
        let field = if index == 1 {
            decoded.accel_pedal_position
        } else {
            decoded.road_speed_limit
        };
        assert_eq!(field, Signal::NotAvailable);
        assert_signal_close(decoded.engine_load_percent, 50.0);
    }
}

#[test]
fn powertrain_eec3_asymmetry_rejects_reserved_special_values() {
    let eec3 = Eec3 {
        nominal_friction_percent: Signal::Value(25.0),
        desired_operating_speed_rpm: Signal::Value(1_800.0),
        operating_speed_asymmetry: Signal::Value(42.0),
    };
    let encoded = eec3.encode();
    assert_eq!(Eec3::decode(&encoded), Some(eec3));

    let mut status_value = encoded;
    status_value[3] = 0xFE;
    assert_eq!(
        Eec3::decode(&status_value)
            .unwrap()
            .operating_speed_asymmetry,
        Signal::Error
    );

    // A reserved raw is absent, not a number, and not a reason to bin the PG.
    for value in [0xFB, 0xFC, 0xFD] {
        let mut reserved = encoded;
        reserved[3] = value;
        let decoded = Eec3::decode(&reserved).unwrap_or_else(|| {
            panic!("asymmetry raw 0x{value:02X} must not drop the whole PG")
        });
        assert_eq!(
            decoded.operating_speed_asymmetry,
            Signal::NotAvailable,
            "EEC3 asymmetry raw 0x{value:02X} must not become a percentage"
        );
        assert_signal_close(decoded.nominal_friction_percent, 25.0);
    }
}

#[test]
fn powertrain_engine_full_message_helpers_reject_invalid_envelopes() {
    let eec1 = Eec1 {
        engine_torque_percent: Signal::Value(10.0),
        driver_demand_percent: Signal::Value(11.0),
        actual_engine_percent: Signal::Value(12.0),
        engine_speed_rpm: Signal::Value(1_450.0),
        starter_mode: 2,
        source_address: 0x80,
    };
    assert_eq!(
        Eec1::from_message(&Message::new(PGN_EEC1, eec1.encode().to_vec(), 0x80)),
        Some(eec1)
    );

    for msg in [
        Message::new(PGN_ETC1, eec1.encode().to_vec(), 0x80),
        Message::new(PGN_EEC1, eec1.encode().to_vec(), NULL_ADDRESS),
        Message::new(PGN_EEC1, eec1.encode().to_vec(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_EEC1,
            eec1.encode().to_vec(),
            0x80,
            0x42,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            Eec1::from_message(&msg),
            None,
            "EEC1 full-message helper must bind payloads to EEC1 and a usable source and destination envelope"
        );
    }

    let eec2 = Eec2 {
        accel_pedal_position: Signal::Value(50.0),
        engine_load_percent: Signal::Value(25.0),
        accel_pedal_low_idle: 1,
        accel_pedal_kickdown: 2,
        road_speed_limit: Signal::Value(60.0),
    };
    assert_eq!(
        Eec2::from_message(&Message::new(PGN_EEC2, eec2.encode().to_vec(), 0x81)),
        Some(eec2)
    );
    assert_eq!(
        Eec2::from_message(&Message::with_addressing(
            PGN_EEC2,
            eec2.encode().to_vec(),
            0x81,
            0x42,
            Priority::Default,
        )),
        None,
        "EEC2 is a PDU2 broadcast envelope and must reject destination-specific metadata"
    );
    let mut bad_eec2 = eec2.encode();
    bad_eec2[0] |= 0x10;
    assert_eq!(
        Eec2::from_message(&Message::new(PGN_EEC2, bad_eec2.to_vec(), 0x81)),
        None,
        "EEC2 helper must apply the same reserved-nibble gate as payload decode"
    );

    let tsc1 = Tsc1 {
        override_mode: OverrideControlMode::SpeedTorqueLimit,
        requested_speed_rpm: Signal::Value(1_800.0),
        requested_torque_percent: Signal::Value(-5.0),
    };
    assert_eq!(
        Tsc1::from_message(&Message::new(PGN_TSC1, tsc1.encode().to_vec(), 0x82)),
        Some(tsc1)
    );
    assert_eq!(
        Tsc1::from_message(&Message::with_addressing(
            PGN_TSC1,
            tsc1.encode().to_vec(),
            0x82,
            0x42,
            Priority::Default,
        )),
        Some(tsc1),
        "TSC1 is PDU1 (destination-specific): an addressed TSC1 envelope is valid"
    );
    let mut bad_tsc1 = tsc1.encode();
    bad_tsc1[0] |= 0x20;
    assert_eq!(
        Tsc1::from_message(&Message::new(PGN_TSC1, bad_tsc1.to_vec(), 0x82)),
        None,
        "TSC1 helper must reject reserved control bits before command use"
    );
}

#[test]
fn powertrain_remaining_fixed_frame_helpers_reject_invalid_envelopes() {
    macro_rules! assert_fixed_helper {
        ($ty:ty, $pgn:expr, $value:expr) => {{
            let value: $ty = $value;
            let encoded = value.encode();
            assert!(
                <$ty>::from_message(&Message::new($pgn, encoded.to_vec(), 0x80)).is_some(),
                "{} helper must accept its matching PGN and usable source",
                stringify!($ty)
            );

            for msg in [
                Message::new(PGN_EEC1, encoded.to_vec(), 0x80),
                Message::new($pgn, encoded.to_vec(), NULL_ADDRESS),
                Message::new($pgn, encoded.to_vec(), BROADCAST_ADDRESS),
                Message::with_addressing(
                    $pgn,
                    encoded.to_vec(),
                    0x80,
                    0x42,
                    Priority::Default,
                ),
            ] {
                if msg.pgn == $pgn && msg.source == 0x80 && msg.destination == BROADCAST_ADDRESS {
                    continue;
                }
                assert_eq!(
                    <$ty>::from_message(&msg),
                    None,
                    "{} helper must reject wrong PGN invalid source and destination-specific PDU2 metadata",
                    stringify!($ty)
                );
            }

            let mut overlong = encoded.to_vec();
            overlong.push(0xFF);
            assert_eq!(
                <$ty>::from_message(&Message::new($pgn, overlong, 0x80)),
                None,
                "{} helper must reject malformed fixed-frame lengths",
                stringify!($ty)
            );
        }};
    }

    assert_fixed_helper!(
        Eec3,
        PGN_EEC3,
        Eec3 {
            nominal_friction_percent: Signal::Value(25.0),
            desired_operating_speed_rpm: Signal::Value(1_800.0),
            operating_speed_asymmetry: Signal::Value(42.0),
        }
    );
    assert_fixed_helper!(
        EngineTemp1,
        PGN_ET1,
        EngineTemp1 {
            coolant_temp_c: Signal::Value(90.0),
            fuel_temp_c: Signal::Value(50.0),
            oil_temp_c: Signal::Value(100.0),
            turbo_oil_temp_c: Signal::Value(110.0),
            intercooler_temp_c: Signal::Value(60.0),
        }
    );
    assert_fixed_helper!(
        EngineTemp2,
        PGN_ET2,
        EngineTemp2 {
            engine_oil_temp_c: Signal::Value(95.0),
            turbo_oil_temp_c: Signal::Value(105.0),
            engine_intercooler_temp_c: Signal::Value(55.0),
            turbo_1_temp_c: Signal::Value(200.0),
        }
    );
    assert_fixed_helper!(
        EngineFluidLp,
        PGN_EFLP,
        EngineFluidLp {
            oil_pressure_kpa: Signal::Value(400.0),
            coolant_pressure_kpa: Signal::Value(200.0),
            oil_level_percent: Signal::Value(80.0),
            coolant_level_percent: Signal::Value(88.0),
            fuel_delivery_pressure_kpa: Signal::Value(300.0),
            crankcase_pressure_kpa: Signal::Value(0.5),
        }
    );
    assert_fixed_helper!(
        EngineHours,
        PGN_ENGINE_HOURS,
        EngineHours {
            total_hours: Signal::Value(12_345.7),
            total_revolutions: Signal::Value(1_000_000_000.0),
        }
    );
    assert_fixed_helper!(
        FuelEconomy,
        PGN_FUEL_ECONOMY,
        FuelEconomy {
            fuel_rate_lph: Signal::Value(25.0),
            instantaneous_lph: Signal::Value(6.5),
            throttle_position: Signal::Value(80.0),
        }
    );
    assert_fixed_helper!(
        FuelConsumption,
        PGN_FUEL_CONSUMPTION,
        FuelConsumption {
            trip_fuel_l: 250.5,
            total_fuel_l: 12_345.0,
        }
    );
    assert_fixed_helper!(
        Vep1,
        PGN_VEP1,
        Vep1 {
            battery_voltage_v: Signal::Value(12.5),
            alternator_current_a: Signal::Value(20.0),
            charging_system_voltage_v: Signal::Value(14.1),
            key_switch_voltage_v: Signal::Value(12.2),
        }
    );
    assert_fixed_helper!(
        AmbientConditions,
        PGN_AMBIENT_CONDITIONS,
        AmbientConditions {
            barometric_pressure_kpa: Signal::Value(101.0),
            ambient_air_temp_c: Signal::Value(25.0),
            intake_air_temp_c: Signal::Value(30.0),
            road_surface_temp_c: Signal::Value(22.0),
        }
    );
    assert_fixed_helper!(
        DashDisplay,
        PGN_DASH_DISPLAY,
        DashDisplay {
            fuel_level_percent: 200,
            washer_fluid_level: 180,
            fuel_filter_diff_kpa: 50.0,
            oil_filter_diff_kpa: 25.0,
            cargo_ambient_temp_c: 20.0,
        }
    );
    assert_fixed_helper!(
        VehiclePosition,
        PGN_VEHICLE_POSITION,
        VehiclePosition {
            latitude_deg: 52.0,
            longitude_deg: 4.0,
        }
    );
    assert_fixed_helper!(
        Aftertreatment1,
        PGN_AT1,
        Aftertreatment1 {
            def_tank_level: 75.0,
            intake_nox_ppm: 1_500.0,
            outlet_nox_ppm: 50.0,
            intake_nox_reading_status: 1,
            outlet_nox_reading_status: 2,
        }
    );
    assert_fixed_helper!(
        Aftertreatment2,
        PGN_AT2,
        Aftertreatment2 {
            dpf_differential_pressure_kpa: 5.5,
            def_concentration: 32.5,
            dpf_soot_load_percent: 75.0,
            dpf_active_regeneration_status: 2,
            dpf_passive_regeneration_status: 1,
        }
    );
}

#[test]
fn powertrain_identification_payloads_require_exact_printable_star_fields() {
    let component = ComponentIdentification {
        make: "AGRO".into(),
        model: "PT-8".into(),
        serial_number: "SN-0001".into(),
        unit_number: "UNIT-42".into(),
    };
    let component_bytes = component.encode();
    assert_eq!(
        component_bytes.iter().filter(|byte| **byte == b'*').count(),
        4
    );
    assert_eq!(
        ComponentIdentification::decode(&component_bytes),
        Some(component)
    );
    assert_eq!(
        ComponentIdentification::decode(b"AGRO*PT-8*SN-0001*"),
        None,
        "component identification must not accept missing fields"
    );
    assert_eq!(
        ComponentIdentification::decode(b"AGRO*PT-8*SN-0001*UNIT-42*EXTRA*"),
        None,
        "component identification must not accept trailing fields"
    );
    assert_eq!(
        ComponentIdentification::decode(b"AGRO*PT-8*SN-\x1F*UNIT-42*"),
        None,
        "component identification fields must be printable ASCII"
    );

    let vehicle = VehicleIdentification {
        vin: "1HGBH41JXMN109186".into(),
    };
    let vehicle_bytes = vehicle.encode();
    assert_eq!(VehicleIdentification::decode(&vehicle_bytes), Some(vehicle));
    assert_eq!(
        VehicleIdentification::decode(b"1HGBH41JXMN109186"),
        None,
        "vehicle identification must be star terminated"
    );
    assert_eq!(
        VehicleIdentification::decode(b"1HGBH41JXMN109186*TRAILING*"),
        None,
        "vehicle identification must carry exactly one star-delimited field"
    );
    assert_eq!(
        VehicleIdentification::decode(b"1HGBH41JXMN109186\x7F*"),
        None,
        "vehicle identification must reject non-printable bytes"
    );
}

#[test]
fn powertrain_identification_full_message_helpers_reject_invalid_envelopes() {
    let component = ComponentIdentification {
        make: "AGRO".into(),
        model: "PT-8".into(),
        serial_number: "SN-0001".into(),
        unit_number: "UNIT-42".into(),
    };
    assert_eq!(
        ComponentIdentification::from_message(&Message::new(
            PGN_COMPONENT_ID,
            component.encode(),
            0x83,
        )),
        Some(component.clone())
    );

    for msg in [
        Message::new(PGN_VEHICLE_ID, component.encode(), 0x83),
        Message::new(PGN_COMPONENT_ID, component.encode(), NULL_ADDRESS),
        Message::new(PGN_COMPONENT_ID, component.encode(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_COMPONENT_ID,
            component.encode(),
            0x83,
            0x42,
            Priority::Default,
        ),
        Message::new(PGN_COMPONENT_ID, b"AGRO*PT-8*SN-0001*".to_vec(), 0x83),
    ] {
        assert_eq!(
            ComponentIdentification::from_message(&msg),
            None,
            "component identification helper must bind PGN source and exact star-field shape"
        );
    }

    let vehicle = VehicleIdentification {
        vin: "1HGBH41JXMN109186".into(),
    };
    assert_eq!(
        VehicleIdentification::from_message(&Message::new(PGN_VEHICLE_ID, vehicle.encode(), 0x84,)),
        Some(vehicle.clone())
    );

    for msg in [
        Message::new(PGN_COMPONENT_ID, vehicle.encode(), 0x84),
        Message::new(PGN_VEHICLE_ID, vehicle.encode(), NULL_ADDRESS),
        Message::new(PGN_VEHICLE_ID, vehicle.encode(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_VEHICLE_ID,
            vehicle.encode(),
            0x84,
            0x42,
            Priority::Default,
        ),
        Message::new(PGN_VEHICLE_ID, b"1HGBH41JXMN109186".to_vec(), 0x84),
    ] {
        assert_eq!(
            VehicleIdentification::from_message(&msg),
            None,
            "vehicle identification helper must reject invalid envelope or unterminated fields"
        );
    }
}

#[test]
fn powertrain_scalar_decoders_reject_not_available_sentinels_for_non_optional_values() {
    let eec3 = Eec3 {
        nominal_friction_percent: Signal::Value(10.0),
        desired_operating_speed_rpm: Signal::Value(1_250.0),
        operating_speed_asymmetry: Signal::Error,
    };
    let encoded_eec3 = eec3.encode();
    assert_eq!(Eec3::decode(&encoded_eec3), Some(eec3));

    let mut absent_eec3_percent = encoded_eec3;
    absent_eec3_percent[0] = 0xFF;
    let decoded = Eec3::decode(&absent_eec3_percent).expect("the frame still decodes");
    assert_eq!(decoded.nominal_friction_percent, Signal::NotAvailable);
    assert_signal_close(decoded.desired_operating_speed_rpm, 1_250.0);
    let mut absent_eec3_speed = encoded_eec3;
    absent_eec3_speed[1..3].copy_from_slice(&u16::MAX.to_le_bytes());
    let decoded = Eec3::decode(&absent_eec3_speed).expect("the frame still decodes");
    assert_eq!(decoded.desired_operating_speed_rpm, Signal::NotAvailable);
    assert_signal_close(decoded.nominal_friction_percent, 10.0);

    let hours = EngineHours {
        total_hours: Signal::Value(123.0),
        total_revolutions: Signal::Value(456_000.0),
    };
    let encoded_hours = hours.encode();
    assert_eq!(EngineHours::decode(&encoded_hours), Some(hours));
    for range in [0..4, 4..8] {
        let mut absent = encoded_hours;
        absent[range.clone()].copy_from_slice(&u32::MAX.to_le_bytes());
        let decoded = EngineHours::decode(&absent).expect("an absent counter keeps the frame");
        let (reported, missing) = if range.start == 0 {
            (decoded.total_revolutions, decoded.total_hours)
        } else {
            (decoded.total_hours, decoded.total_revolutions)
        };
        assert_eq!(missing, Signal::NotAvailable);
        assert!(reported.value().is_some(), "the other counter must survive");
    }

    let position = VehiclePosition {
        latitude_deg: 52.0,
        longitude_deg: 5.0,
    };
    let encoded_position = position.encode();
    assert_eq!(VehiclePosition::decode(&encoded_position), Some(position));
    for range in [0..4, 4..8] {
        let mut bad = encoded_position;
        bad[range].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(VehiclePosition::decode(&bad), None);
    }

    let fuel = FuelConsumption {
        trip_fuel_l: 100.0,
        total_fuel_l: 2_500.0,
    };
    let encoded_fuel = fuel.encode();
    assert_eq!(FuelConsumption::decode(&encoded_fuel), Some(fuel));
    for range in [0..4, 4..8] {
        let mut bad = encoded_fuel;
        bad[range].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(FuelConsumption::decode(&bad), None);
    }
}

#[test]
fn powertrain_multibyte_error_indicators_are_reported_not_scaled() {
    let eec1 = Eec1 {
        engine_torque_percent: Signal::Value(10.0),
        driver_demand_percent: Signal::Value(20.0),
        actual_engine_percent: Signal::Value(30.0),
        engine_speed_rpm: Signal::Value(1_800.0),
        starter_mode: 1,
        source_address: 0x34,
    };
    let encoded_eec1 = eec1.encode();
    assert!(Eec1::decode(&encoded_eec1).is_some());
    // The indicator is reported as absent rather than scaled — and the rest of
    // the PG survives it (H54/G4).
    let mut absent_engine_speed = encoded_eec1;
    absent_engine_speed[3..5].copy_from_slice(&(u16::MAX - 1).to_le_bytes());
    let decoded = Eec1::decode(&absent_engine_speed).expect("the frame still decodes");
    assert_eq!(decoded.engine_speed_rpm, Signal::NotAvailable);
    assert_signal_close(decoded.engine_torque_percent, 10.0);

    let hours = EngineHours {
        total_hours: Signal::Value(123.0),
        total_revolutions: Signal::Value(456_000.0),
    };
    let encoded_hours = hours.encode();
    assert_eq!(EngineHours::decode(&encoded_hours), Some(hours));
    for (range, name) in [(0..4, "total-hours"), (4..8, "total-revolutions")] {
        let mut bad = encoded_hours;
        bad[range].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
        let decoded = EngineHours::decode(&bad)
            .unwrap_or_else(|| panic!("field {name} must not drop the whole PG"));
        assert!(
            decoded.total_hours == Signal::NotAvailable
                || decoded.total_revolutions == Signal::NotAvailable,
            "engine-hour field {name} must report the multibyte indicator"
        );
    }

    let etc1 = Etc1 {
        current_gear: 5,
        selected_gear: 6,
        output_shaft_speed_rpm: 1_500.0,
        shift_in_progress: 1,
        torque_converter_lockup: 2,
    };
    let encoded_etc1 = etc1.encode();
    assert_eq!(Etc1::decode(&encoded_etc1), Some(etc1));
    let mut bad_etc1_speed = encoded_etc1;
    bad_etc1_speed[1..3].copy_from_slice(&(u16::MAX - 1).to_le_bytes());
    assert_eq!(Etc1::decode(&bad_etc1_speed), None);

    let oil = TransmissionOilTemp { oil_temp_c: 80.0 };
    let encoded_oil = oil.encode();
    assert_eq!(TransmissionOilTemp::decode(&encoded_oil), Some(oil));
    let mut bad_oil = encoded_oil;
    bad_oil[0..2].copy_from_slice(&(u16::MAX - 1).to_le_bytes());
    assert_eq!(TransmissionOilTemp::decode(&bad_oil), None);

    let cruise = CruiseControl {
        wheel_speed_kmh: 12.5,
        cc_active: 1,
        brake_switch: 0,
        clutch_switch: 0,
        park_brake: 1,
        cc_set_speed_kmh: 14.0,
    };
    let encoded_cruise = cruise.encode();
    assert_eq!(CruiseControl::decode(&encoded_cruise), Some(cruise));
    for (range, name) in [(0..2, "wheel-speed"), (3..5, "set-speed")] {
        let mut bad_speed = encoded_cruise;
        bad_speed[range].copy_from_slice(&(u16::MAX - 1).to_le_bytes());
        assert_eq!(
            CruiseControl::decode(&bad_speed),
            None,
            "cruise speed field {name} must reject the multibyte error indicator"
        );
    }

    let measurement = SpeedAndDistance {
        speed_mps: Some(5.0),
        distance_m: Some(1234.5),
        timestamp_us: 0,
    };
    let encoded_measurement = measurement.encode();
    assert!(SpeedAndDistance::decode_measurement_prefix(&encoded_measurement).is_some());
    let mut bad_measurement_speed = encoded_measurement;
    bad_measurement_speed[0..2].copy_from_slice(&(u16::MAX - 1).to_le_bytes());
    assert_eq!(
        SpeedAndDistance::decode_measurement_prefix(&bad_measurement_speed),
        None
    );
    let mut bad_measurement_distance = encoded_measurement;
    bad_measurement_distance[2..6].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
    assert_eq!(
        SpeedAndDistance::decode_measurement_prefix(&bad_measurement_distance),
        None
    );

    let not_available_measurement = [0xFFu8; 8];
    let decoded_not_available =
        SpeedAndDistance::decode_measurement_prefix(&not_available_measurement)
            .expect("not-available sentinels are valid absent optional measurements");
    assert_eq!(decoded_not_available.speed_mps, None);
    assert_eq!(decoded_not_available.distance_m, None);
}

#[test]
fn powertrain_one_byte_scaled_fields_report_special_values_without_scaling() {
    let eec1 = Eec1 {
        engine_torque_percent: Signal::Value(10.0),
        driver_demand_percent: Signal::Value(20.0),
        actual_engine_percent: Signal::Value(30.0),
        engine_speed_rpm: Signal::Value(1_800.0),
        starter_mode: 1,
        source_address: 0x34,
    };
    let encoded_eec1 = eec1.encode();
    assert!(Eec1::decode(&encoded_eec1).is_some());
    for (index, name) in [
        (0usize, "engine-torque"),
        (1, "driver-demand"),
        (2, "actual-engine"),
    ] {
        for (reserved, expected) in [
            (0xFBu8, Signal::NotAvailable),
            (0xFE, Signal::Error),
            (0xFF, Signal::NotAvailable),
        ] {
            let mut special = encoded_eec1;
            special[index] = reserved;
            let decoded = Eec1::decode(&special).unwrap_or_else(|| {
                panic!("EEC1 {name} raw 0x{reserved:02X} must not drop the frame")
            });
            let field = [
                decoded.engine_torque_percent,
                decoded.driver_demand_percent,
                decoded.actual_engine_percent,
            ][index];
            assert_eq!(
                field, expected,
                "EEC1 {name} raw 0x{reserved:02X} must be reported, never scaled"
            );
            // Whatever else the frame carried is untouched.
            assert_signal_close(decoded.engine_speed_rpm, 1_800.0);
        }
    }
    let saturated_eec1 = Eec1 {
        engine_torque_percent: Signal::Value(10_000.0),
        driver_demand_percent: Signal::Value(10_000.0),
        actual_engine_percent: Signal::Value(10_000.0),
        ..eec1
    }
    .encode();
    assert_eq!(&saturated_eec1[0..3], &[250, 250, 250]);
    assert!(Eec1::decode(&saturated_eec1).is_some());

    let eec2 = Eec2 {
        accel_pedal_position: Signal::Error,
        engine_load_percent: Signal::Value(25.0),
        accel_pedal_low_idle: 1,
        accel_pedal_kickdown: 2,
        road_speed_limit: Signal::Error,
    };
    let encoded_eec2 = eec2.encode();
    assert_eq!(Eec2::decode(&encoded_eec2), Some(eec2));
    for (reserved, expected) in [
        (0xFBu8, Signal::NotAvailable),
        (0xFE, Signal::Error),
        (0xFF, Signal::NotAvailable),
    ] {
        let mut special = encoded_eec2;
        special[2] = reserved;
        let decoded = Eec2::decode(&special).unwrap_or_else(|| {
            panic!("engine-load raw 0x{reserved:02X} must not drop the whole PG")
        });
        assert_eq!(
            decoded.engine_load_percent, expected,
            "EEC2 engine-load raw 0x{reserved:02X} must be reported, never scaled"
        );
        assert_eq!(decoded.accel_pedal_low_idle, 1);
    }
    let saturated_eec2 = Eec2 {
        engine_load_percent: Signal::Value(10_000.0),
        ..eec2
    }
    .encode();
    assert_eq!(saturated_eec2[2], 250);
    assert!(Eec2::decode(&saturated_eec2).is_some());

    let eec3 = Eec3 {
        nominal_friction_percent: Signal::Value(12.0),
        desired_operating_speed_rpm: Signal::Value(1_250.0),
        operating_speed_asymmetry: Signal::Error,
    };
    let encoded_eec3 = eec3.encode();
    assert_eq!(Eec3::decode(&encoded_eec3), Some(eec3));
    for (reserved, expected) in [
        (0xFBu8, Signal::NotAvailable),
        (0xFE, Signal::Error),
        (0xFF, Signal::NotAvailable),
    ] {
        let mut special = encoded_eec3;
        special[0] = reserved;
        let decoded = Eec3::decode(&special).expect("the frame still decodes");
        assert_eq!(decoded.nominal_friction_percent, expected);
        assert_signal_close(decoded.desired_operating_speed_rpm, 1_250.0);
    }

    let temp1 = EngineTemp1 {
        coolant_temp_c: Signal::Value(90.0),
        fuel_temp_c: Signal::Value(40.0),
        oil_temp_c: Signal::Value(100.0),
        turbo_oil_temp_c: Signal::Value(110.0),
        intercooler_temp_c: Signal::Value(60.0),
    };
    let encoded_temp1 = temp1.encode();
    assert!(EngineTemp1::decode(&encoded_temp1).is_some());
    for index in [0usize, 1, 6] {
        let mut faulted = encoded_temp1;
        faulted[index] = 0xFE;
        let decoded = EngineTemp1::decode(&faulted)
            .unwrap_or_else(|| panic!("a faulted sensor must not drop the ET1 frame"));
        let field = [
            decoded.coolant_temp_c,
            decoded.fuel_temp_c,
            decoded.intercooler_temp_c,
        ][[0usize, 1, 6].iter().position(|i| *i == index).unwrap()];
        assert_eq!(
            field,
            Signal::Error,
            "EngineTemp1 one-byte field {index} must report the error indicator"
        );
        assert_signal_close(decoded.oil_temp_c, 100.0);
    }

    let tsc1 = Tsc1 {
        override_mode: OverrideControlMode::SpeedTorqueLimit,
        requested_speed_rpm: Signal::Value(2_000.0),
        requested_torque_percent: Signal::Value(33.0),
    };
    let encoded_tsc1 = tsc1.encode();
    assert_eq!(Tsc1::decode(&encoded_tsc1), Some(tsc1));
    // A request that governs speed only leaves torque at the sentinel: that is
    // a command to obey, not a frame to discard.
    for (reserved, expected) in [
        (0xFBu8, Signal::NotAvailable),
        (0xFE, Signal::Error),
        (0xFF, Signal::NotAvailable),
    ] {
        let mut special = encoded_tsc1;
        special[3] = reserved;
        let decoded = Tsc1::decode(&special)
            .unwrap_or_else(|| panic!("torque raw 0x{reserved:02X} must not drop the command"));
        assert_eq!(decoded.requested_torque_percent, expected);
        assert_eq!(decoded.override_mode, OverrideControlMode::SpeedTorqueLimit);
        assert_signal_close(decoded.requested_speed_rpm, 2_000.0);
    }
    let saturated_tsc1 = Tsc1 {
        requested_torque_percent: Signal::Value(10_000.0),
        ..tsc1
    }
    .encode();
    assert_eq!(saturated_tsc1[3], 250);
    assert!(Tsc1::decode(&saturated_tsc1).is_some());

    let vep = Vep1 {
        battery_voltage_v: Signal::Value(12.5),
        alternator_current_a: Signal::Value(50.0),
        charging_system_voltage_v: Signal::Value(14.2),
        key_switch_voltage_v: Signal::Value(12.4),
    };
    let encoded_vep = vep.encode();
    assert!(Vep1::decode(&encoded_vep).is_some());
    for (reserved, expected) in [
        (0xFBu8, Signal::NotAvailable),
        (0xFE, Signal::Error),
        (0xFF, Signal::NotAvailable),
    ] {
        let mut special = encoded_vep;
        special[6] = reserved;
        let decoded = Vep1::decode(&special).unwrap_or_else(|| {
            panic!("alternator raw 0x{reserved:02X} must not drop the whole PG")
        });
        assert_eq!(decoded.alternator_current_a, expected);
        assert_signal_close(decoded.battery_voltage_v, 12.5);
    }
}

#[test]
fn powertrain_remaining_scalar_decoders_reject_not_available_sentinels() {
    let temp1 = EngineTemp1 {
        coolant_temp_c: Signal::Value(90.0),
        fuel_temp_c: Signal::Value(40.0),
        oil_temp_c: Signal::Value(100.0),
        turbo_oil_temp_c: Signal::Value(110.0),
        intercooler_temp_c: Signal::Value(60.0),
    };
    let encoded_temp1 = temp1.encode();
    assert!(EngineTemp1::decode(&encoded_temp1).is_some());
    for index in [0usize, 1, 6] {
        let mut absent = encoded_temp1;
        absent[index] = 0xFF;
        let decoded = EngineTemp1::decode(&absent).expect("an absent sensor keeps the frame");
        assert_signal_close(decoded.oil_temp_c, 100.0);
    }
    for range in [2..4, 4..6] {
        let mut absent = encoded_temp1;
        absent[range].copy_from_slice(&u16::MAX.to_le_bytes());
        let decoded = EngineTemp1::decode(&absent).expect("an absent sensor keeps the frame");
        assert_signal_close(decoded.coolant_temp_c, 90.0);
        assert_signal_close(decoded.intercooler_temp_c, 60.0);
    }
    // Every sensor absent is a valid frame, not a decode failure.
    assert_eq!(
        EngineTemp1::decode(&EngineTemp1::default().encode()),
        Some(EngineTemp1::default())
    );

    let temp2 = EngineTemp2 {
        engine_oil_temp_c: Signal::Value(95.0),
        turbo_oil_temp_c: Signal::Value(105.0),
        engine_intercooler_temp_c: Signal::Value(55.0),
        turbo_1_temp_c: Signal::Value(200.0),
    };
    let encoded_temp2 = temp2.encode();
    assert!(EngineTemp2::decode(&encoded_temp2).is_some());
    for range in [0..2, 2..4, 5..7] {
        let mut absent = encoded_temp2;
        absent[range].copy_from_slice(&u16::MAX.to_le_bytes());
        let decoded = EngineTemp2::decode(&absent).expect("an absent sensor keeps the frame");
        assert_signal_close(decoded.engine_intercooler_temp_c, 55.0);
    }
    let mut absent_intercooler = encoded_temp2;
    absent_intercooler[4] = 0xFF;
    let decoded = EngineTemp2::decode(&absent_intercooler).expect("the frame still decodes");
    assert_eq!(decoded.engine_intercooler_temp_c, Signal::NotAvailable);
    assert_signal_close(decoded.engine_oil_temp_c, 95.0);

    let fluid = EngineFluidLp {
        oil_pressure_kpa: Signal::Value(400.0),
        coolant_pressure_kpa: Signal::Value(200.0),
        oil_level_percent: Signal::Value(80.0),
        coolant_level_percent: Signal::Value(88.0),
        fuel_delivery_pressure_kpa: Signal::Value(300.0),
        crankcase_pressure_kpa: Signal::Value(0.5),
    };
    let encoded_fluid = fluid.encode();
    assert!(EngineFluidLp::decode(&encoded_fluid).is_some());
    for index in [0usize, 1, 2] {
        let mut absent = encoded_fluid;
        absent[index] = 0xFF;
        let decoded = EngineFluidLp::decode(&absent).expect("an absent sensor keeps the frame");
        assert_signal_close(decoded.oil_level_percent, 80.0);
    }
    let mut absent_crankcase = encoded_fluid;
    absent_crankcase[5..7].copy_from_slice(&u16::MAX.to_le_bytes());
    let decoded = EngineFluidLp::decode(&absent_crankcase).expect("the frame still decodes");
    assert_eq!(decoded.crankcase_pressure_kpa, Signal::NotAvailable);
    assert_signal_close(decoded.oil_pressure_kpa, 400.0);

    let economy = FuelEconomy {
        fuel_rate_lph: Signal::Value(25.0),
        instantaneous_lph: Signal::Value(6.5),
        throttle_position: Signal::Value(80.0),
    };
    let encoded_economy = economy.encode();
    assert!(FuelEconomy::decode(&encoded_economy).is_some());
    for range in [0..2, 2..4] {
        let mut absent = encoded_economy;
        absent[range].copy_from_slice(&u16::MAX.to_le_bytes());
        let decoded = FuelEconomy::decode(&absent).expect("an absent rate keeps the frame");
        assert_signal_close(decoded.throttle_position, 80.0);
    }
    let mut absent_throttle = encoded_economy;
    absent_throttle[4] = 0xFF;
    let decoded = FuelEconomy::decode(&absent_throttle).expect("the frame still decodes");
    assert_eq!(decoded.throttle_position, Signal::NotAvailable);
    assert_signal_close(decoded.fuel_rate_lph, 25.0);

    let tsc1 = Tsc1 {
        override_mode: OverrideControlMode::SpeedTorqueLimit,
        requested_speed_rpm: Signal::Value(2_000.0),
        requested_torque_percent: Signal::Value(33.0),
    };
    let encoded_tsc1 = tsc1.encode();
    assert!(Tsc1::decode(&encoded_tsc1).is_some());
    let mut absent_tsc1_speed = encoded_tsc1;
    absent_tsc1_speed[1..3].copy_from_slice(&u16::MAX.to_le_bytes());
    let decoded = Tsc1::decode(&absent_tsc1_speed).expect("a torque-only request is valid");
    assert_eq!(decoded.requested_speed_rpm, Signal::NotAvailable);
    assert_signal_close(decoded.requested_torque_percent, 33.0);
    let mut absent_tsc1_torque = encoded_tsc1;
    absent_tsc1_torque[3] = 0xFF;
    let decoded = Tsc1::decode(&absent_tsc1_torque).expect("a speed-only request is valid");
    assert_eq!(decoded.requested_torque_percent, Signal::NotAvailable);
    assert_signal_close(decoded.requested_speed_rpm, 2_000.0);

    let vep = Vep1 {
        battery_voltage_v: Signal::Value(12.5),
        alternator_current_a: Signal::Value(50.0),
        charging_system_voltage_v: Signal::Value(14.2),
        key_switch_voltage_v: Signal::Value(12.4),
    };
    let encoded_vep = vep.encode();
    assert!(Vep1::decode(&encoded_vep).is_some());
    for range in [0..2, 2..4, 4..6] {
        let mut absent = encoded_vep;
        absent[range].copy_from_slice(&u16::MAX.to_le_bytes());
        let decoded = Vep1::decode(&absent).expect("an absent rail keeps the frame");
        assert_signal_close(decoded.alternator_current_a, 50.0);
    }
    let mut absent_alternator = encoded_vep;
    absent_alternator[6] = 0xFF;
    let decoded = Vep1::decode(&absent_alternator).expect("the frame still decodes");
    assert_eq!(decoded.alternator_current_a, Signal::NotAvailable);
    assert_signal_close(decoded.battery_voltage_v, 12.5);

    let ambient = AmbientConditions {
        barometric_pressure_kpa: Signal::Value(101.0),
        ambient_air_temp_c: Signal::Value(25.0),
        intake_air_temp_c: Signal::Value(30.0),
        road_surface_temp_c: Signal::Value(22.0),
    };
    let encoded_ambient = ambient.encode();
    assert!(AmbientConditions::decode(&encoded_ambient).is_some());
    for index in [0usize, 3] {
        let mut absent = encoded_ambient;
        absent[index] = 0xFF;
        let decoded = AmbientConditions::decode(&absent).expect("an absent sensor keeps the frame");
        assert_signal_close(decoded.ambient_air_temp_c, 25.0);
    }
    for range in [1..3, 4..6] {
        let mut absent = encoded_ambient;
        absent[range].copy_from_slice(&u16::MAX.to_le_bytes());
        let decoded = AmbientConditions::decode(&absent).expect("an absent sensor keeps the frame");
        assert_signal_close(decoded.intake_air_temp_c, 30.0);
    }

    let dash = DashDisplay {
        fuel_level_percent: 200,
        washer_fluid_level: 180,
        fuel_filter_diff_kpa: 50.0,
        oil_filter_diff_kpa: 25.0,
        cargo_ambient_temp_c: 20.0,
    };
    let encoded_dash = dash.encode();
    assert!(DashDisplay::decode(&encoded_dash).is_some());
    for index in [2usize, 3] {
        let mut bad = encoded_dash;
        bad[index] = 0xFF;
        assert_eq!(DashDisplay::decode(&bad), None);
    }
    let mut bad_cargo_temp = encoded_dash;
    bad_cargo_temp[4..6].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(DashDisplay::decode(&bad_cargo_temp), None);

    let aftertreatment1 = Aftertreatment1 {
        def_tank_level: 75.0,
        intake_nox_ppm: 1_500.0,
        outlet_nox_ppm: 50.0,
        intake_nox_reading_status: 1,
        outlet_nox_reading_status: 1,
    };
    let encoded_at1 = aftertreatment1.encode();
    assert!(Aftertreatment1::decode(&encoded_at1).is_some());
    let mut bad_def_level = encoded_at1;
    bad_def_level[0] = 0xFF;
    assert_eq!(Aftertreatment1::decode(&bad_def_level), None);
    for range in [1..3, 3..5] {
        let mut bad = encoded_at1;
        bad[range].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(Aftertreatment1::decode(&bad), None);
    }

    let aftertreatment2 = Aftertreatment2 {
        dpf_differential_pressure_kpa: 5.5,
        def_concentration: 32.5,
        dpf_soot_load_percent: 75.0,
        dpf_active_regeneration_status: 2,
        dpf_passive_regeneration_status: 1,
    };
    let encoded_at2 = aftertreatment2.encode();
    assert!(Aftertreatment2::decode(&encoded_at2).is_some());
    let mut bad_diff = encoded_at2;
    bad_diff[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(Aftertreatment2::decode(&bad_diff), None);
    for index in [2usize, 3] {
        let mut bad = encoded_at2;
        bad[index] = 0xFF;
        assert_eq!(Aftertreatment2::decode(&bad), None);
    }
}

#[test]
fn powertrain_percentage_fields_reject_reserved_special_values_without_losing_defined_statuses() {
    // Raw 250 is the top of the SPN 98/111 range: 100 % at 0.4 %/bit.
    let fluid = EngineFluidLp {
        oil_pressure_kpa: Signal::Value(400.0),
        coolant_pressure_kpa: Signal::Value(200.0),
        oil_level_percent: Signal::Value(100.0),
        coolant_level_percent: Signal::Value(100.0),
        fuel_delivery_pressure_kpa: Signal::Value(300.0),
        crankcase_pressure_kpa: Signal::Value(0.5),
    };
    let encoded_fluid = fluid.encode();
    assert_eq!(encoded_fluid[3], 250);
    assert_signal_close(
        EngineFluidLp::decode(&encoded_fluid)
            .unwrap()
            .oil_level_percent,
        100.0,
    );
    // Not-available and error stay distinguishable from each other.
    let mut special_fluid = encoded_fluid;
    special_fluid[3] = 0xFF;
    special_fluid[4] = 0xFE;
    let decoded = EngineFluidLp::decode(&special_fluid).unwrap();
    assert_eq!(decoded.oil_level_percent, Signal::NotAvailable);
    assert_eq!(decoded.coolant_level_percent, Signal::Error);
    // A reserved raw is reported as absent, never scaled into a percentage.
    for (index, value) in [(3usize, 0xFB), (4, 0xFD)] {
        let mut reserved = encoded_fluid;
        reserved[index] = value;
        let decoded = EngineFluidLp::decode(&reserved).unwrap_or_else(|| {
            panic!("reserved raw 0x{value:02X} in byte {index} must not drop the PG")
        });
        let field = if index == 3 {
            decoded.oil_level_percent
        } else {
            decoded.coolant_level_percent
        };
        assert_eq!(
            field,
            Signal::NotAvailable,
            "reserved raw 0x{value:02X} must not become a percentage"
        );
        assert_signal_close(decoded.oil_pressure_kpa, 400.0);
    }

    let economy = FuelEconomy {
        fuel_rate_lph: Signal::Value(25.0),
        instantaneous_lph: Signal::Value(6.5),
        throttle_position: Signal::Value(100.0),
    };
    let encoded_economy = economy.encode();
    assert!(FuelEconomy::decode(&encoded_economy).is_some());
    let mut status_economy = encoded_economy;
    status_economy[4] = 0xFE;
    assert_eq!(
        FuelEconomy::decode(&status_economy).unwrap().throttle_position,
        Signal::Error
    );
    for value in [0xFB, 0xFD, 0xFF] {
        let mut special = encoded_economy;
        special[4] = value;
        let decoded = FuelEconomy::decode(&special).unwrap_or_else(|| {
            panic!("throttle raw 0x{value:02X} must not drop the whole PG")
        });
        assert_eq!(
            decoded.throttle_position,
            Signal::NotAvailable,
            "throttle raw 0x{value:02X} must not become a percentage"
        );
        assert_signal_close(decoded.fuel_rate_lph, 25.0);
    }

    let dash = DashDisplay {
        fuel_level_percent: 250,
        washer_fluid_level: 250,
        fuel_filter_diff_kpa: 50.0,
        oil_filter_diff_kpa: 25.0,
        cargo_ambient_temp_c: 20.0,
    };
    let encoded_dash = dash.encode();
    assert_eq!(
        DashDisplay::decode(&encoded_dash)
            .unwrap()
            .fuel_level_percent,
        250
    );
    let mut unavailable_dash = encoded_dash;
    unavailable_dash[0] = 0xFF;
    unavailable_dash[1] = 0xFE;
    assert_eq!(
        DashDisplay::decode(&unavailable_dash)
            .unwrap()
            .washer_fluid_level,
        0xFF
    );
    assert_eq!(
        DashDisplay::decode(&unavailable_dash)
            .unwrap()
            .fuel_level_percent,
        0xFE
    );
    for (index, value) in [(0usize, 0xFB), (1, 0xFD)] {
        let mut bad = encoded_dash;
        bad[index] = value;
        assert_eq!(
            DashDisplay::decode(&bad),
            None,
            "dash level percentage byte {index} must reject reserved raw value 0x{value:02X}"
        );
    }

    let aftertreatment1 = Aftertreatment1 {
        def_tank_level: 100.0,
        intake_nox_ppm: 1_500.0,
        outlet_nox_ppm: 50.0,
        intake_nox_reading_status: 1,
        outlet_nox_reading_status: 1,
    };
    let encoded_at1 = aftertreatment1.encode();
    assert!(Aftertreatment1::decode(&encoded_at1).is_some());
    let mut status_at1 = encoded_at1;
    status_at1[0] = 0xFE;
    assert!(Aftertreatment1::decode(&status_at1).is_some());
    for value in [0xFB, 0xFD, 0xFF] {
        let mut bad = encoded_at1;
        bad[0] = value;
        assert_eq!(
            Aftertreatment1::decode(&bad),
            None,
            "aftertreatment DEF tank percentage must reject special raw value 0x{value:02X}"
        );
    }

    let aftertreatment2 = Aftertreatment2 {
        dpf_differential_pressure_kpa: 5.5,
        def_concentration: 100.0,
        dpf_soot_load_percent: 100.0,
        dpf_active_regeneration_status: 2,
        dpf_passive_regeneration_status: 1,
    };
    let encoded_at2 = aftertreatment2.encode();
    assert!(Aftertreatment2::decode(&encoded_at2).is_some());
    let mut status_at2 = encoded_at2;
    status_at2[2] = 0xFE;
    status_at2[3] = 0xFE;
    assert!(Aftertreatment2::decode(&status_at2).is_some());
    for (index, value) in [(2usize, 0xFB), (3, 0xFD), (2, 0xFF)] {
        let mut bad = encoded_at2;
        bad[index] = value;
        assert_eq!(
            Aftertreatment2::decode(&bad),
            None,
            "aftertreatment percentage byte {index} must reject special raw value 0x{value:02X}"
        );
    }
}

#[test]
fn powertrain_aftertreatment1_rejects_reserved_status_bytes_and_ambient_shape() {
    let aftertreatment1 = Aftertreatment1 {
        def_tank_level: 75.0,
        intake_nox_ppm: 1_500.0,
        outlet_nox_ppm: 50.0,
        intake_nox_reading_status: 1,
        outlet_nox_reading_status: 2,
    };
    let encoded_at1 = aftertreatment1.encode();
    let decoded_at1 = Aftertreatment1::decode(&encoded_at1).unwrap();
    assert_eq!(decoded_at1.intake_nox_reading_status, 1);
    assert_eq!(decoded_at1.outlet_nox_reading_status, 2);

    for status in 0..=3 {
        let mut status_edge = encoded_at1;
        status_edge[5] = status;
        status_edge[6] = 3 - status;
        assert!(
            Aftertreatment1::decode(&status_edge).is_some(),
            "AT1 status byte values in the defined status range must remain accepted"
        );
    }
    let mut error_indicator_status = encoded_at1;
    error_indicator_status[5] = 0xFE;
    error_indicator_status[6] = 0xFE;
    assert!(
        Aftertreatment1::decode(&error_indicator_status).is_some(),
        "AT1 status bytes must preserve the defined error-indicator status"
    );

    for (index, value) in [(5usize, 0x04), (6, 0x7F), (5, 0xFD), (6, 0xFF)] {
        let mut bad = encoded_at1;
        bad[index] = value;
        assert_eq!(
            Aftertreatment1::decode(&bad),
            None,
            "AT1 status byte {index} must reject reserved raw value 0x{value:02X}"
        );
    }

    let ambient = AmbientConditions {
        barometric_pressure_kpa: Signal::Value(101.0),
        ambient_air_temp_c: Signal::Value(25.0),
        intake_air_temp_c: Signal::Value(30.0),
        road_surface_temp_c: Signal::Value(22.0),
    };
    let encoded_ambient = ambient.encode();
    assert!(AmbientConditions::decode(&encoded_ambient).is_some());
    assert_eq!(
        Aftertreatment1::decode(&encoded_ambient),
        None,
        "the shared-PGN ambient payload shape must not be accepted as AT1"
    );
    assert_eq!(
        Aftertreatment1::from_message(&Message::new(PGN_AT1, encoded_ambient.to_vec(), 0x80)),
        None,
        "the AT1 full-message helper must reject ambient-shaped shared-PGN payloads"
    );
}

#[test]
fn powertrain_tsc1_round_trips_mode_speed_and_torque() {
    let tsc1 = Tsc1 {
        override_mode: OverrideControlMode::SpeedTorqueLimit,
        requested_speed_rpm: Signal::Value(2_000.0),
        requested_torque_percent: Signal::Value(33.0),
    };
    let decoded = Tsc1::decode(&tsc1.encode()).unwrap();

    assert_eq!(decoded.override_mode, OverrideControlMode::SpeedTorqueLimit);
    assert_signal_close(decoded.requested_speed_rpm, 2_000.0);
    assert_signal_close(decoded.requested_torque_percent, 33.0);
}

#[test]
fn powertrain_tsc1_rejects_reserved_control_bits_before_command_use() {
    for mode in [
        OverrideControlMode::NoOverride,
        OverrideControlMode::SpeedControl,
        OverrideControlMode::TorqueControl,
        OverrideControlMode::SpeedTorqueLimit,
    ] {
        let command = Tsc1 {
            override_mode: mode,
            requested_speed_rpm: Signal::Value(1_250.0),
            requested_torque_percent: Signal::Value(-12.0),
        };
        let encoded = command.encode();
        assert_eq!(Tsc1::decode(&encoded), Some(command));

        for reserved in [0x04, 0x08, 0x20, 0xFC] {
            let mut bad_control = encoded;
            bad_control[0] |= reserved;
            assert_eq!(
                Tsc1::decode(&bad_control),
                None,
                "TSC1 reserved control bits must not be masked into a valid override mode"
            );
        }
    }
}

#[test]
fn powertrain_public_override_control_mode_decoder_rejects_noncanonical_bytes() {
    for (raw, mode) in [
        (0, OverrideControlMode::NoOverride),
        (1, OverrideControlMode::SpeedControl),
        (2, OverrideControlMode::TorqueControl),
        (3, OverrideControlMode::SpeedTorqueLimit),
    ] {
        assert_eq!(OverrideControlMode::try_from_u8(raw), Some(mode));
        assert_eq!(OverrideControlMode::from_u8(raw), mode);
    }

    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(
            OverrideControlMode::try_from_u8(packed_or_reserved),
            None,
            "strict public TSC1 override decoder must reject packed or reserved bits"
        );
    }

    let command = Tsc1 {
        override_mode: OverrideControlMode::SpeedControl,
        requested_speed_rpm: Signal::Value(1_500.0),
        requested_torque_percent: Signal::Value(10.0),
    };
    assert_eq!(
        Tsc1::decode(&command.encode())
            .expect("field-extracted TSC1 payload should still decode")
            .override_mode,
        OverrideControlMode::SpeedControl
    );
}

#[test]
fn powertrain_transmission_decoders_reject_reserved_and_not_available_fields() {
    let etc1 = Etc1 {
        current_gear: 5,
        selected_gear: 6,
        output_shaft_speed_rpm: 1_500.0,
        shift_in_progress: 1,
        torque_converter_lockup: 2,
    };
    let encoded_etc1 = etc1.encode();
    assert_eq!(Etc1::decode(&encoded_etc1), Some(etc1));

    let mut bad_control_reserved = encoded_etc1;
    bad_control_reserved[0] |= 0x10;
    assert_eq!(Etc1::decode(&bad_control_reserved), None);

    let mut bad_tail = encoded_etc1;
    bad_tail[5] = 0x00;
    assert_eq!(Etc1::decode(&bad_tail), None);

    let mut bad_speed_sentinel = encoded_etc1;
    bad_speed_sentinel[1] = 0xFF;
    bad_speed_sentinel[2] = 0xFF;
    assert_eq!(Etc1::decode(&bad_speed_sentinel), None);

    let mut bad_gear_sentinel = encoded_etc1;
    bad_gear_sentinel[3] = 0xFB;
    assert_eq!(Etc1::decode(&bad_gear_sentinel), None);

    let oil = TransmissionOilTemp { oil_temp_c: 80.0 };
    let encoded_oil = oil.encode();
    assert_eq!(TransmissionOilTemp::decode(&encoded_oil), Some(oil));
    let mut bad_oil_tail = encoded_oil;
    bad_oil_tail[2] = 0x00;
    assert_eq!(TransmissionOilTemp::decode(&bad_oil_tail), None);

    let cruise = CruiseControl {
        wheel_speed_kmh: 12.5,
        cc_active: 1,
        brake_switch: 0,
        clutch_switch: 0,
        park_brake: 1,
        cc_set_speed_kmh: 14.0,
    };
    let encoded_cruise = cruise.encode();
    assert_eq!(CruiseControl::decode(&encoded_cruise), Some(cruise));
    let mut bad_cruise_tail = encoded_cruise;
    bad_cruise_tail[5] = 0x00;
    assert_eq!(CruiseControl::decode(&bad_cruise_tail), None);
}

#[test]
fn powertrain_transmission_scalar_decoders_reject_not_available_sentinels() {
    let etc1 = Etc1 {
        current_gear: 5,
        selected_gear: 6,
        output_shaft_speed_rpm: 1_500.0,
        shift_in_progress: 1,
        torque_converter_lockup: 2,
    };
    let encoded_etc1 = etc1.encode();
    assert_eq!(Etc1::decode(&encoded_etc1), Some(etc1));
    let mut bad_etc1_speed = encoded_etc1;
    bad_etc1_speed[1..3].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(Etc1::decode(&bad_etc1_speed), None);
    for index in [3usize, 4] {
        let mut bad_gear = encoded_etc1;
        bad_gear[index] = 0xFB;
        assert_eq!(Etc1::decode(&bad_gear), None);
    }

    let oil = TransmissionOilTemp { oil_temp_c: 80.0 };
    let encoded_oil = oil.encode();
    assert_eq!(TransmissionOilTemp::decode(&encoded_oil), Some(oil));
    let mut bad_oil = encoded_oil;
    bad_oil[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(TransmissionOilTemp::decode(&bad_oil), None);

    let cruise = CruiseControl {
        wheel_speed_kmh: 12.5,
        cc_active: 1,
        brake_switch: 0,
        clutch_switch: 0,
        park_brake: 1,
        cc_set_speed_kmh: 14.0,
    };
    let encoded_cruise = cruise.encode();
    assert_eq!(CruiseControl::decode(&encoded_cruise), Some(cruise));
    for range in [0..2, 3..5] {
        let mut bad_speed = encoded_cruise;
        bad_speed[range].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(CruiseControl::decode(&bad_speed), None);
    }
}
