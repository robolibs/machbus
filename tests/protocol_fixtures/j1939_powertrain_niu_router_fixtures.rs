#[test]
fn fixture_j1939_engine_powertrain_default_and_sentinel_vectors_are_stable() {
    let default_vectors = [
        ("eec1_default", Eec1::default().encode()),
        ("eec2_default", Eec2::default().encode()),
        ("eec3_default", Eec3::default().encode()),
        ("engine_temp1_default", EngineTemp1::default().encode()),
        ("engine_temp2_default", EngineTemp2::default().encode()),
        ("engine_fluid_lp_default", EngineFluidLp::default().encode()),
        ("engine_hours_default", EngineHours::default().encode()),
        ("fuel_economy_default", FuelEconomy::default().encode()),
        ("tsc1_default", Tsc1::default().encode()),
        ("vep1_default", Vep1::default().encode()),
        ("ambient_default", AmbientConditions::default().encode()),
        ("dash_display_default", DashDisplay::default().encode()),
        (
            "vehicle_position_default",
            VehiclePosition::default().encode(),
        ),
        (
            "fuel_consumption_default",
            FuelConsumption::default().encode(),
        ),
        (
            "aftertreatment1_default",
            Aftertreatment1::default().encode(),
        ),
        (
            "aftertreatment2_default",
            Aftertreatment2::default().encode(),
        ),
        ("etc1_default", Etc1::default().encode()),
        (
            "transmission_oil_default",
            TransmissionOilTemp::default().encode(),
        ),
        ("cruise_control_default", CruiseControl::default().encode()),
    ];
    for (name, expected) in default_vectors {
        assert_eq!(
            expected,
            parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, name),
            "{name} default fixture drifted"
        );
    }

    let engine_hours_min =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "engine_hours_raw_min");
    assert_eq!(
        EngineHours {
            total_hours: 0.0,
            total_revolutions: 0.0,
        }
        .encode(),
        engine_hours_min
    );
    let decoded = EngineHours::decode(&engine_hours_min).unwrap();
    assert_eq!(decoded.total_hours, 0.0);
    assert_eq!(decoded.total_revolutions, 0.0);

    let engine_hours_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_hours_upper_edge",
    );
    assert_eq!(
        EngineHours {
            total_hours: 214_748_364.65,
            total_revolutions: 4_294_967_293_000.0,
        }
        .encode(),
        engine_hours_upper
    );
    let decoded = EngineHours::decode(&engine_hours_upper).unwrap();
    assert!((decoded.total_hours - 214_748_364.65).abs() < 0.001);
    assert_eq!(decoded.total_revolutions, 4_294_967_293_000.0);

    let position_min = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "vehicle_position_raw_min",
    );
    assert_eq!(
        VehiclePosition {
            latitude_deg: -210.0,
            longitude_deg: -210.0,
        }
        .encode(),
        position_min
    );
    let decoded = VehiclePosition::decode(&position_min).unwrap();
    assert_eq!(decoded.latitude_deg, -210.0);
    assert_eq!(decoded.longitude_deg, -210.0);

    let position_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "vehicle_position_upper_edge",
    );
    assert_eq!(
        VehiclePosition {
            latitude_deg: 219.496_729_3,
            longitude_deg: 219.496_729_3,
        }
        .encode(),
        position_upper
    );
    let decoded = VehiclePosition::decode(&position_upper).unwrap();
    assert!((decoded.latitude_deg - 219.496_729_3).abs() < 1e-9);
    assert!((decoded.longitude_deg - 219.496_729_3).abs() < 1e-9);

    let fuel_consumption_min = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "fuel_consumption_raw_min",
    );
    assert_eq!(
        FuelConsumption {
            trip_fuel_l: 0.0,
            total_fuel_l: 0.0,
        }
        .encode(),
        fuel_consumption_min
    );
    let decoded = FuelConsumption::decode(&fuel_consumption_min).unwrap();
    assert_eq!(decoded.trip_fuel_l, 0.0);
    assert_eq!(decoded.total_fuel_l, 0.0);

    let fuel_consumption_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "fuel_consumption_upper_edge",
    );
    assert_eq!(
        FuelConsumption {
            trip_fuel_l: 2_147_483_646.5,
            total_fuel_l: 2_147_483_646.5,
        }
        .encode(),
        fuel_consumption_upper
    );
    let decoded = FuelConsumption::decode(&fuel_consumption_upper).unwrap();
    assert_eq!(decoded.trip_fuel_l, 2_147_483_646.5);
    assert_eq!(decoded.total_fuel_l, 2_147_483_646.5);

    let etc1_min = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "etc1_min_gears_zero");
    assert_eq!(
        Etc1 {
            current_gear: -125,
            selected_gear: -125,
            output_shaft_speed_rpm: 0.0,
            shift_in_progress: 0,
            torque_converter_lockup: 0,
        }
        .encode(),
        etc1_min
    );
    let decoded = Etc1::decode(&etc1_min).unwrap();
    assert_eq!(decoded.current_gear, -125);
    assert_eq!(decoded.selected_gear, -125);
    assert_eq!(decoded.output_shaft_speed_rpm, 0.0);

    let etc1_upper = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "etc1_upper_edge");
    assert_eq!(
        Etc1 {
            current_gear: 125,
            selected_gear: 125,
            output_shaft_speed_rpm: 8191.625,
            shift_in_progress: 2,
            torque_converter_lockup: 3,
        }
        .encode(),
        etc1_upper
    );
    let decoded = Etc1::decode(&etc1_upper).unwrap();
    assert_eq!(decoded.current_gear, 125);
    assert_eq!(decoded.selected_gear, 125);
    assert_eq!(decoded.output_shaft_speed_rpm, 8191.625);
    assert_eq!(decoded.shift_in_progress, 2);
    assert_eq!(decoded.torque_converter_lockup, 3);

    let oil_min = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "transmission_oil_raw_min",
    );
    assert_eq!(TransmissionOilTemp { oil_temp_c: -273.0 }.encode(), oil_min);
    assert_eq!(
        TransmissionOilTemp::decode(&oil_min).unwrap().oil_temp_c,
        -273.0
    );

    let oil_high = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "transmission_oil_raw_high",
    );
    assert_eq!(
        TransmissionOilTemp {
            oil_temp_c: 1774.90625,
        }
        .encode(),
        oil_high
    );
    assert_eq!(
        TransmissionOilTemp::decode(&oil_high).unwrap().oil_temp_c,
        1774.90625
    );

    let cruise_status =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "cruise_all_status_na");
    assert_eq!(
        CruiseControl {
            wheel_speed_kmh: 0.0,
            cc_active: 3,
            brake_switch: 3,
            clutch_switch: 3,
            park_brake: 3,
            cc_set_speed_kmh: 0.0,
        }
        .encode(),
        cruise_status
    );
    let decoded = CruiseControl::decode(&cruise_status).unwrap();
    assert_eq!(decoded.cc_active, 3);
    assert_eq!(decoded.brake_switch, 3);
    assert_eq!(decoded.clutch_switch, 3);
    assert_eq!(decoded.park_brake, 3);

    let cruise_upper =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "cruise_upper_speed");
    assert_eq!(
        CruiseControl {
            wheel_speed_kmh: 255.98828125,
            cc_active: 0,
            brake_switch: 0,
            clutch_switch: 0,
            park_brake: 0,
            cc_set_speed_kmh: 255.98828125,
        }
        .encode(),
        cruise_upper
    );
    let decoded = CruiseControl::decode(&cruise_upper).unwrap();
    assert_eq!(decoded.wheel_speed_kmh, 255.98828125);
    assert_eq!(decoded.cc_set_speed_kmh, 255.98828125);

    let speed_zero =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "speed_distance_zero");
    assert_eq!(
        SpeedAndDistance {
            speed_mps: Some(0.0),
            distance_m: Some(0.0),
            timestamp_us: 0,
        }
        .encode(),
        speed_zero
    );
    let decoded = SpeedAndDistance::decode(&speed_zero).unwrap();
    assert_eq!(decoded.speed_mps, Some(0.0));
    assert_eq!(decoded.distance_m, Some(0.0));

    let speed_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "speed_distance_upper_edge",
    );
    assert_eq!(
        SpeedAndDistance {
            speed_mps: Some(1_000.0),
            distance_m: Some(10_000_000.0),
            timestamp_us: 0,
        }
        .encode(),
        speed_upper
    );
    let decoded = SpeedAndDistance::decode(&speed_upper).unwrap();
    assert!((decoded.speed_mps.unwrap() - 65.533).abs() < 0.001);
    assert!((decoded.distance_m.unwrap() - 4_294_967.293).abs() < 0.002);

    let speed_na_distance = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "speed_distance_speed_na_distance_42m",
    );
    let decoded = SpeedAndDistance::decode(&speed_na_distance).unwrap();
    assert_eq!(decoded.speed_mps, None);
    assert_eq!(decoded.distance_m, Some(42.0));

    let speed_distance_na = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "speed_distance_speed_7_5_distance_na",
    );
    let decoded = SpeedAndDistance::decode(&speed_distance_na).unwrap();
    assert_eq!(decoded.speed_mps, Some(7.5));
    assert_eq!(decoded.distance_m, None);

    assert_eq!(
        Etc1 {
            current_gear: -128,
            selected_gear: 127,
            output_shaft_speed_rpm: 99_999.0,
            shift_in_progress: 2,
            torque_converter_lockup: 3,
        }
        .encode(),
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "etc1_clamped_inputs")
    );
    assert_eq!(
        TransmissionOilTemp {
            oil_temp_c: 10_000.0,
        }
        .encode(),
        parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "transmission_oil_clamped_high",
        )
    );
    assert_eq!(
        TransmissionOilTemp {
            oil_temp_c: f64::NAN
        }
        .encode(),
        parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "transmission_oil_nan_zero"
        )
    );
    assert_eq!(
        CruiseControl {
            wheel_speed_kmh: 10_000.0,
            cc_active: 0,
            brake_switch: 0,
            clutch_switch: 0,
            park_brake: 0,
            cc_set_speed_kmh: 10_000.0,
        }
        .encode(),
        parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "cruise_clamped_high_speeds",
        )
    );
    let clamped_speed_distance = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "speed_distance_clamped_high",
    );
    assert_eq!(
        SpeedAndDistance {
            speed_mps: Some(1_000.0),
            distance_m: Some(10_000_000.0),
            timestamp_us: 0,
        }
        .encode(),
        clamped_speed_distance
    );
    let decoded = SpeedAndDistance::decode(&clamped_speed_distance).unwrap();
    assert!(decoded.speed_mps.is_some());
    assert!(decoded.distance_m.is_some());
    assert_eq!(
        SpeedAndDistance {
            speed_mps: Some(f64::NAN),
            distance_m: Some(f64::INFINITY),
            timestamp_us: 0,
        }
        .encode(),
        parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "speed_distance_nonfinite_zero",
        )
    );

    let clamped_engine_vectors = [
        (
            "eec1_clamped_inputs",
            Eec1 {
                engine_torque_percent: 99_999.0,
                driver_demand_percent: 99_999.0,
                actual_engine_percent: 99_999.0,
                engine_speed_rpm: 99_999.0,
                starter_mode: 0x0F,
                source_address: 0xEE,
            }
            .encode(),
        ),
        (
            "eec2_clamped_inputs",
            Eec2 {
                accel_pedal_position: 0xFE,
                engine_load_percent: 99_999.0,
                accel_pedal_low_idle: 3,
                accel_pedal_kickdown: 3,
                road_speed_limit: 0xFE,
            }
            .encode(),
        ),
        (
            "eec3_clamped_inputs",
            Eec3 {
                nominal_friction_percent: 99_999.0,
                desired_operating_speed_rpm: 99_999.0,
                operating_speed_asymmetry: 0xFE,
            }
            .encode(),
        ),
        (
            "engine_temp1_clamped_inputs",
            EngineTemp1 {
                coolant_temp_c: 99_999.0,
                fuel_temp_c: 99_999.0,
                oil_temp_c: 99_999.0,
                turbo_oil_temp_c: 99_999.0,
                intercooler_temp_c: 99_999.0,
            }
            .encode(),
        ),
        (
            "engine_temp2_clamped_inputs",
            EngineTemp2 {
                engine_oil_temp_c: 99_999.0,
                turbo_oil_temp_c: 99_999.0,
                engine_intercooler_temp_c: 99_999.0,
                turbo_1_temp_c: 99_999.0,
            }
            .encode(),
        ),
        (
            "engine_fluid_lp_clamped_inputs",
            EngineFluidLp {
                oil_pressure_kpa: 99_999.0,
                coolant_pressure_kpa: 99_999.0,
                oil_level_percent: 0xFE,
                coolant_level_percent: 0xFE,
                fuel_delivery_pressure_kpa: 99_999.0,
                crankcase_pressure_kpa: 99_999.0,
            }
            .encode(),
        ),
        (
            "engine_hours_clamped_inputs",
            EngineHours {
                total_hours: 999_999_999_999.0,
                total_revolutions: 999_999_999_999_999.0,
            }
            .encode(),
        ),
        (
            "fuel_economy_clamped_inputs",
            FuelEconomy {
                fuel_rate_lph: 99_999.0,
                instantaneous_lph: 99_999.0,
                throttle_position: 99_999.0,
            }
            .encode(),
        ),
        (
            "tsc1_clamped_inputs",
            Tsc1 {
                override_mode: OverrideControlMode::SpeedTorqueLimit,
                requested_speed_rpm: 99_999.0,
                requested_torque_percent: 99_999.0,
            }
            .encode(),
        ),
        (
            "vep1_clamped_inputs",
            Vep1 {
                battery_voltage_v: 99_999.0,
                alternator_current_a: 99_999.0,
                charging_system_voltage_v: 99_999.0,
                key_switch_voltage_v: 99_999.0,
            }
            .encode(),
        ),
        (
            "ambient_clamped_inputs",
            AmbientConditions {
                barometric_pressure_kpa: 99_999.0,
                ambient_air_temp_c: 99_999.0,
                intake_air_temp_c: 99_999.0,
                road_surface_temp_c: 99_999.0,
            }
            .encode(),
        ),
        (
            "dash_display_clamped_inputs",
            DashDisplay {
                fuel_level_percent: 0xFE,
                washer_fluid_level: 0xFE,
                fuel_filter_diff_kpa: 99_999.0,
                oil_filter_diff_kpa: 99_999.0,
                cargo_ambient_temp_c: 99_999.0,
            }
            .encode(),
        ),
        (
            "vehicle_position_clamped_inputs",
            VehiclePosition {
                latitude_deg: 1_000.0,
                longitude_deg: 1_000.0,
            }
            .encode(),
        ),
        (
            "fuel_consumption_clamped_inputs",
            FuelConsumption {
                trip_fuel_l: 9_999_999_999.0,
                total_fuel_l: 9_999_999_999.0,
            }
            .encode(),
        ),
        (
            "aftertreatment1_clamped_inputs",
            Aftertreatment1 {
                def_tank_level: 99_999.0,
                intake_nox_ppm: 99_999.0,
                outlet_nox_ppm: 99_999.0,
                intake_nox_reading_status: 0xFE,
                outlet_nox_reading_status: 0xFE,
            }
            .encode(),
        ),
        (
            "aftertreatment2_clamped_inputs",
            Aftertreatment2 {
                dpf_differential_pressure_kpa: 99_999.0,
                def_concentration: 99_999.0,
                dpf_soot_load_percent: 99_999.0,
                dpf_active_regeneration_status: 0xFE,
                dpf_passive_regeneration_status: 0xFE,
            }
            .encode(),
        ),
    ];
    for (name, expected) in clamped_engine_vectors {
        assert_eq!(
            expected,
            parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, name),
            "{name} fixture drifted"
        );
    }

    let eec1_min = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "eec1_raw_min");
    assert_eq!(
        Eec1 {
            engine_torque_percent: -125.0,
            driver_demand_percent: -125.0,
            actual_engine_percent: -125.0,
            engine_speed_rpm: 0.0,
            starter_mode: 0,
            source_address: 0,
        }
        .encode(),
        eec1_min
    );
    let decoded = Eec1::decode(&eec1_min).unwrap();
    assert_eq!(decoded.engine_torque_percent, -125.0);
    assert_eq!(decoded.engine_speed_rpm, 0.0);

    let eec1_upper = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "eec1_upper_edge");
    assert_eq!(
        Eec1 {
            engine_torque_percent: 125.0,
            driver_demand_percent: 125.0,
            actual_engine_percent: 125.0,
            engine_speed_rpm: 8191.625,
            starter_mode: 0x0F,
            source_address: 0xFE,
        }
        .encode(),
        eec1_upper
    );
    let decoded = Eec1::decode(&eec1_upper).unwrap();
    assert_eq!(decoded.engine_torque_percent, 125.0);
    assert_eq!(decoded.driver_demand_percent, 125.0);
    assert_eq!(decoded.actual_engine_percent, 125.0);
    assert_eq!(decoded.engine_speed_rpm, 8191.625);
    assert_eq!(decoded.starter_mode, 0x0F);

    let eec2_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "eec2_zero_status_upper_values",
    );
    assert_eq!(
        Eec2 {
            accel_pedal_position: 0xFE,
            engine_load_percent: 250.0,
            accel_pedal_low_idle: 0,
            accel_pedal_kickdown: 0,
            road_speed_limit: 0xFE,
        }
        .encode(),
        eec2_upper
    );
    let decoded = Eec2::decode(&eec2_upper).unwrap();
    assert_eq!(decoded.accel_pedal_position, 0xFE);
    assert_eq!(decoded.engine_load_percent, 250.0);

    let eec2_error = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "eec2_error_status_zero_values",
    );
    assert_eq!(
        Eec2 {
            accel_pedal_position: 0,
            engine_load_percent: 0.0,
            accel_pedal_low_idle: 3,
            accel_pedal_kickdown: 3,
            road_speed_limit: 0,
        }
        .encode(),
        eec2_error
    );
    let decoded = Eec2::decode(&eec2_error).unwrap();
    assert_eq!(decoded.accel_pedal_low_idle, 3);
    assert_eq!(decoded.accel_pedal_kickdown, 3);

    let eec3_min = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "eec3_raw_min");
    assert_eq!(
        Eec3 {
            nominal_friction_percent: -125.0,
            desired_operating_speed_rpm: 0.0,
            operating_speed_asymmetry: 0,
        }
        .encode(),
        eec3_min
    );
    assert_eq!(
        Eec3::decode(&eec3_min).unwrap().nominal_friction_percent,
        -125.0
    );

    let eec3_upper = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "eec3_upper_edge");
    assert_eq!(
        Eec3 {
            nominal_friction_percent: 125.0,
            desired_operating_speed_rpm: 8191.625,
            operating_speed_asymmetry: 0xFE,
        }
        .encode(),
        eec3_upper
    );
    let decoded = Eec3::decode(&eec3_upper).unwrap();
    assert_eq!(decoded.desired_operating_speed_rpm, 8191.625);
    assert_eq!(decoded.operating_speed_asymmetry, 0xFE);

    let temp1_min =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "engine_temp1_raw_min");
    assert_eq!(
        EngineTemp1 {
            coolant_temp_c: -40.0,
            fuel_temp_c: -40.0,
            oil_temp_c: -273.0,
            turbo_oil_temp_c: -273.0,
            intercooler_temp_c: -40.0,
        }
        .encode(),
        temp1_min
    );
    assert_eq!(EngineTemp1::decode(&temp1_min).unwrap().oil_temp_c, -273.0);

    let temp1_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_temp1_upper_edge",
    );
    assert_eq!(
        EngineTemp1 {
            coolant_temp_c: 210.0,
            fuel_temp_c: 210.0,
            oil_temp_c: 1774.90625,
            turbo_oil_temp_c: 1774.90625,
            intercooler_temp_c: 210.0,
        }
        .encode(),
        temp1_upper
    );
    let decoded = EngineTemp1::decode(&temp1_upper).unwrap();
    assert_eq!(decoded.coolant_temp_c, 210.0);
    assert_eq!(decoded.oil_temp_c, 1774.90625);

    let temp2_min =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "engine_temp2_raw_min");
    assert_eq!(
        EngineTemp2 {
            engine_oil_temp_c: -273.0,
            turbo_oil_temp_c: -273.0,
            engine_intercooler_temp_c: -40.0,
            turbo_1_temp_c: -273.0,
        }
        .encode(),
        temp2_min
    );
    assert_eq!(
        EngineTemp2::decode(&temp2_min).unwrap().engine_oil_temp_c,
        -273.0
    );

    let temp2_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_temp2_upper_edge",
    );
    assert_eq!(
        EngineTemp2 {
            engine_oil_temp_c: 1774.90625,
            turbo_oil_temp_c: 1774.90625,
            engine_intercooler_temp_c: 210.0,
            turbo_1_temp_c: 1774.90625,
        }
        .encode(),
        temp2_upper
    );
    let decoded = EngineTemp2::decode(&temp2_upper).unwrap();
    assert_eq!(decoded.engine_intercooler_temp_c, 210.0);
    assert_eq!(decoded.turbo_1_temp_c, 1774.90625);

    let fluid_min = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_fluid_lp_raw_min",
    );
    assert_eq!(
        EngineFluidLp {
            fuel_delivery_pressure_kpa: 0.0,
            oil_pressure_kpa: 0.0,
            coolant_pressure_kpa: 0.0,
            oil_level_percent: 0,
            coolant_level_percent: 0,
            crankcase_pressure_kpa: -250.0,
        }
        .encode(),
        fluid_min
    );
    assert_eq!(
        EngineFluidLp::decode(&fluid_min)
            .unwrap()
            .crankcase_pressure_kpa,
        -250.0
    );

    let fluid_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_fluid_lp_upper_edge",
    );
    assert_eq!(
        EngineFluidLp {
            fuel_delivery_pressure_kpa: 1000.0,
            oil_pressure_kpa: 1000.0,
            coolant_pressure_kpa: 500.0,
            oil_level_percent: 0xFE,
            coolant_level_percent: 0xFE,
            crankcase_pressure_kpa: 3026.65,
        }
        .encode(),
        fluid_upper
    );
    let decoded = EngineFluidLp::decode(&fluid_upper).unwrap();
    assert_eq!(decoded.fuel_delivery_pressure_kpa, 1000.0);
    assert!((decoded.crankcase_pressure_kpa - 3026.65).abs() < 1e-9);

    let tsc1_min = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "tsc1_no_override_raw_min",
    );
    assert_eq!(
        Tsc1 {
            override_mode: OverrideControlMode::NoOverride,
            requested_speed_rpm: 0.0,
            requested_torque_percent: -125.0,
        }
        .encode(),
        tsc1_min
    );
    assert_eq!(
        Tsc1::decode(&tsc1_min).unwrap().requested_torque_percent,
        -125.0
    );

    let tsc1_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "tsc1_speed_torque_upper",
    );
    assert_eq!(
        Tsc1 {
            override_mode: OverrideControlMode::SpeedTorqueLimit,
            requested_speed_rpm: 8191.625,
            requested_torque_percent: 125.0,
        }
        .encode(),
        tsc1_upper
    );
    let decoded = Tsc1::decode(&tsc1_upper).unwrap();
    assert_eq!(decoded.override_mode, OverrideControlMode::SpeedTorqueLimit);
    assert_eq!(decoded.requested_speed_rpm, 8191.625);

    let vep1_min = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "vep1_raw_min");
    assert_eq!(
        Vep1 {
            battery_voltage_v: 0.0,
            charging_system_voltage_v: 0.0,
            key_switch_voltage_v: 0.0,
            alternator_current_a: -125.0,
        }
        .encode(),
        vep1_min
    );
    assert_eq!(
        Vep1::decode(&vep1_min).unwrap().alternator_current_a,
        -125.0
    );

    let vep1_upper = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "vep1_upper_edge");
    assert_eq!(
        Vep1 {
            battery_voltage_v: 3276.65,
            charging_system_voltage_v: 3276.65,
            key_switch_voltage_v: 3276.65,
            alternator_current_a: 125.0,
        }
        .encode(),
        vep1_upper
    );
    let decoded = Vep1::decode(&vep1_upper).unwrap();
    assert!((decoded.battery_voltage_v - 3276.65).abs() < 1e-9);
    assert_eq!(decoded.alternator_current_a, 125.0);

    let ambient_min = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "ambient_raw_min");
    assert_eq!(
        AmbientConditions {
            barometric_pressure_kpa: 0.0,
            ambient_air_temp_c: -273.0,
            intake_air_temp_c: -40.0,
            road_surface_temp_c: -273.0,
        }
        .encode(),
        ambient_min
    );
    assert_eq!(
        AmbientConditions::decode(&ambient_min)
            .unwrap()
            .ambient_air_temp_c,
        -273.0
    );

    let ambient_upper =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "ambient_upper_edge");
    assert_eq!(
        AmbientConditions {
            barometric_pressure_kpa: 125.0,
            ambient_air_temp_c: 1774.90625,
            intake_air_temp_c: 210.0,
            road_surface_temp_c: 1774.90625,
        }
        .encode(),
        ambient_upper
    );
    let decoded = AmbientConditions::decode(&ambient_upper).unwrap();
    assert_eq!(decoded.barometric_pressure_kpa, 125.0);
    assert_eq!(decoded.road_surface_temp_c, 1774.90625);

    let dash_min =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "dash_display_raw_min");
    assert_eq!(
        DashDisplay {
            washer_fluid_level: 0,
            fuel_level_percent: 0,
            fuel_filter_diff_kpa: 0.0,
            oil_filter_diff_kpa: 0.0,
            cargo_ambient_temp_c: -273.0,
        }
        .encode(),
        dash_min
    );
    assert_eq!(
        DashDisplay::decode(&dash_min).unwrap().cargo_ambient_temp_c,
        -273.0
    );

    let dash_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "dash_display_upper_edge",
    );
    assert_eq!(
        DashDisplay {
            washer_fluid_level: 0xFE,
            fuel_level_percent: 0xFE,
            fuel_filter_diff_kpa: 500.0,
            oil_filter_diff_kpa: 125.0,
            cargo_ambient_temp_c: 1774.90625,
        }
        .encode(),
        dash_upper
    );
    let decoded = DashDisplay::decode(&dash_upper).unwrap();
    assert_eq!(decoded.fuel_filter_diff_kpa, 500.0);
    assert_eq!(decoded.cargo_ambient_temp_c, 1774.90625);

    let fuel_economy_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "fuel_economy_upper_edge",
    );
    assert_eq!(
        FuelEconomy {
            fuel_rate_lph: 3276.65,
            instantaneous_lph: 127.994140625,
            throttle_position: 100.0,
        }
        .encode(),
        fuel_economy_upper
    );
    let decoded = FuelEconomy::decode(&fuel_economy_upper).unwrap();
    assert!((decoded.fuel_rate_lph - 3276.65).abs() < 1e-9);
    assert!((decoded.instantaneous_lph - 127.994140625).abs() < 1e-12);

    let at1_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "aftertreatment1_upper_edge",
    );
    assert_eq!(
        Aftertreatment1 {
            def_tank_level: 100.0,
            intake_nox_ppm: 3276.65,
            outlet_nox_ppm: 3276.65,
            intake_nox_reading_status: 0xFE,
            outlet_nox_reading_status: 0xFE,
        }
        .encode(),
        at1_upper
    );
    let decoded = Aftertreatment1::decode(&at1_upper).unwrap();
    assert!((decoded.def_tank_level - 100.0).abs() < 1e-9);
    assert_eq!(decoded.outlet_nox_reading_status, 0xFE);

    let at2_upper = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "aftertreatment2_upper_edge",
    );
    assert_eq!(
        Aftertreatment2 {
            dpf_differential_pressure_kpa: 6553.3,
            def_concentration: 100.0,
            dpf_soot_load_percent: 100.0,
            dpf_active_regeneration_status: 0xFE,
            dpf_passive_regeneration_status: 0xFE,
        }
        .encode(),
        at2_upper
    );
    let decoded = Aftertreatment2::decode(&at2_upper).unwrap();
    assert!((decoded.dpf_differential_pressure_kpa - 6553.3).abs() < 1e-9);
    assert_eq!(decoded.dpf_passive_regeneration_status, 0xFE);

    let short = parse_named_hex_bytes(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "malformed_fixed8_short7",
    );
    let overlong = parse_named_hex_bytes(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "malformed_fixed8_overlong9",
    );
    for malformed in [&short, &overlong] {
        assert!(Eec1::decode(malformed).is_none());
        assert!(Eec2::decode(malformed).is_none());
        assert!(Eec3::decode(malformed).is_none());
        assert!(EngineTemp1::decode(malformed).is_none());
        assert!(EngineTemp2::decode(malformed).is_none());
        assert!(EngineFluidLp::decode(malformed).is_none());
        assert!(EngineHours::decode(malformed).is_none());
        assert!(FuelEconomy::decode(malformed).is_none());
        assert!(Tsc1::decode(malformed).is_none());
        assert!(Vep1::decode(malformed).is_none());
        assert!(AmbientConditions::decode(malformed).is_none());
        assert!(DashDisplay::decode(malformed).is_none());
        assert!(VehiclePosition::decode(malformed).is_none());
        assert!(FuelConsumption::decode(malformed).is_none());
        assert!(Aftertreatment1::decode(malformed).is_none());
        assert!(Aftertreatment2::decode(malformed).is_none());
        assert!(Etc1::decode(malformed).is_none());
        assert!(TransmissionOilTemp::decode(malformed).is_none());
        assert!(CruiseControl::decode(malformed).is_none());
        assert!(SpeedAndDistance::decode(malformed).is_none());
    }

    assert!(
        Eec1::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_eec1_bad_padding",
        ))
        .is_none()
    );
    assert!(
        Eec2::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_eec2_bad_padding",
        ))
        .is_none()
    );
    assert!(
        Eec3::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_eec3_bad_padding",
        ))
        .is_none()
    );
    assert!(
        EngineTemp1::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_engine_temp1_bad_padding",
        ))
        .is_none()
    );
    assert!(
        EngineTemp2::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_engine_temp2_bad_padding",
        ))
        .is_none()
    );
    assert!(
        EngineFluidLp::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_engine_fluid_lp_bad_padding",
        ))
        .is_none()
    );
    assert!(
        FuelEconomy::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_fuel_economy_bad_padding",
        ))
        .is_none()
    );
    assert!(
        Tsc1::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_tsc1_bad_padding",
        ))
        .is_none()
    );
    assert!(
        Vep1::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_vep1_bad_padding",
        ))
        .is_none()
    );
    assert!(
        AmbientConditions::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_ambient_bad_padding",
        ))
        .is_none()
    );
    assert!(
        DashDisplay::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_dash_display_bad_padding",
        ))
        .is_none()
    );
    assert!(
        Aftertreatment1::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_aftertreatment1_bad_padding",
        ))
        .is_none()
    );
    assert!(
        Aftertreatment2::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_aftertreatment2_bad_padding",
        ))
        .is_none()
    );

    for malformed in [
        "malformed_etc1_reserved_bits",
        "malformed_etc1_bad_padding",
        "malformed_etc1_speed_na",
        "malformed_etc1_current_gear_na",
        "malformed_etc1_selected_gear_na",
    ] {
        assert!(
            Etc1::decode(&parse_named_hex_frame(
                J1939_ENGINE_POWERTRAIN_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }
    assert!(
        TransmissionOilTemp::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_transmission_oil_bad_padding",
        ))
        .is_none()
    );
    assert!(
        TransmissionOilTemp::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_transmission_oil_na",
        ))
        .is_none()
    );
    assert!(
        CruiseControl::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_cruise_control_bad_padding",
        ))
        .is_none()
    );
    for malformed in [
        "malformed_cruise_wheel_speed_na",
        "malformed_cruise_set_speed_na",
    ] {
        assert!(
            CruiseControl::decode(&parse_named_hex_frame(
                J1939_ENGINE_POWERTRAIN_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }
    assert!(
        SpeedAndDistance::decode(&parse_named_hex_frame(
            J1939_ENGINE_POWERTRAIN_CODECS_HEX,
            "malformed_speed_distance_bad_padding",
        ))
        .is_none()
    );
}

#[test]
fn fixture_isobus_niu_control_and_policy_snapshots_are_stable() {
    let add_filter = parse_named_hex_frame(ISOBUS_NIU_CONTROL_HEX, "add_filter_pgn_ef00_port1");
    let delete_filter =
        parse_named_hex_frame(ISOBUS_NIU_CONTROL_HEX, "delete_filter_pgn_ef00_port1");
    let set_block_all =
        parse_named_hex_frame(ISOBUS_NIU_CONTROL_HEX, "set_filter_mode_block_all_port1");
    let port_stats = parse_named_hex_frame(
        ISOBUS_NIU_CONTROL_HEX,
        "port_stats_forwarded1234_blockedabcd_port2",
    );

    let expected_add = NiuNetworkMsg {
        function: NiuFunction::AddFilterEntry,
        port_number: 1,
        filter_pgn: PGN_PROPRIETARY_A,
        ..Default::default()
    };
    assert_eq!(expected_add.encode().unwrap(), add_filter);
    assert_eq!(NiuNetworkMsg::decode(&add_filter), Some(expected_add));

    let add_frame = Frame::from_message(
        Priority::Default,
        PGN_NIU_NETWORK_MSG,
        0x80,
        0x90,
        &add_filter,
    );
    assert_eq!(
        add_frame.id.raw,
        parse_hex_u64(parse_named_text_value(
            ISOBUS_NIU_CONTROL_HEX,
            "niu_add_filter_dest90_src80_raw_id",
        )) as u32
    );
    assert_eq!(add_frame.pgn(), PGN_NIU_NETWORK_MSG);
    assert_eq!(add_frame.source(), 0x80);
    assert_eq!(add_frame.destination(), 0x90);

    let expected_delete = NiuNetworkMsg {
        function: NiuFunction::DeleteFilterEntry,
        port_number: 1,
        filter_pgn: PGN_PROPRIETARY_A,
        ..Default::default()
    };
    assert_eq!(expected_delete.encode().unwrap(), delete_filter);
    assert_eq!(NiuNetworkMsg::decode(&delete_filter), Some(expected_delete));

    let expected_set_block_all = NiuNetworkMsg {
        function: NiuFunction::SetFilterMode,
        port_number: 1,
        filter_mode: NiuFilterMode::BlockAll,
        ..Default::default()
    };
    assert_eq!(expected_set_block_all.encode().unwrap(), set_block_all);
    assert_eq!(
        NiuNetworkMsg::decode(&set_block_all),
        Some(expected_set_block_all)
    );

    let expected_stats = NiuNetworkMsg {
        function: NiuFunction::PortStatsResponse,
        port_number: 2,
        msgs_forwarded: 0x1234,
        msgs_blocked: 0xABCD,
        ..Default::default()
    };
    assert_eq!(expected_stats.encode().unwrap(), port_stats);
    assert_eq!(NiuNetworkMsg::decode(&port_stats), Some(expected_stats));

    for malformed in [
        "malformed_niu_short_add_filter",
        "malformed_niu_add_filter_bad_pgn_high_bits",
        "malformed_niu_add_filter_bad_tail",
        "malformed_niu_set_filter_mode_reserved_mode",
        "malformed_niu_set_filter_mode_bad_tail",
        "malformed_niu_port_stats_bad_tail",
        "malformed_niu_unknown_function",
    ] {
        assert!(
            NiuNetworkMsg::try_decode(&parse_named_hex_bytes(ISOBUS_NIU_CONTROL_HEX, malformed))
                .is_none(),
            "{malformed} must reject"
        );
    }

    let filter_rule = parse_named_hex_bytes(
        ISOBUS_NIU_CONTROL_HEX,
        "filter_rule_monitor_pgn_ef00_bidirectional_persistent_250ms",
    );
    let rule = FilterRule::new(PGN_PROPRIETARY_A, ForwardPolicy::Monitor, true)
        .persistent(true)
        .with_max_frequency_ms(250);
    assert_eq!(rule.encode().unwrap(), filter_rule);

    let decoded_rule = FilterRule::decode(&filter_rule).expect("NIU filter fixture decodes");
    assert_eq!(decoded_rule.pgn, PGN_PROPRIETARY_A);
    assert_eq!(decoded_rule.policy, ForwardPolicy::Monitor);
    assert!(decoded_rule.bidirectional);
    assert!(decoded_rule.persistent);
    assert_eq!(decoded_rule.max_frequency_ms, 250);
    assert_eq!(decoded_rule.source_name, None);
    assert_eq!(decoded_rule.destination_name, None);
    assert_eq!(decoded_rule.last_forward_time_ms, None);

    for malformed in [
        "filter_rule_short21",
        "filter_rule_overlong23",
        "filter_rule_bad_pgn_high_bits",
        "filter_rule_bad_policy_bits",
        "filter_rule_bad_reserved_flag_bits",
        "filter_rule_absent_source_not_ff",
        "filter_rule_absent_dest_not_ff",
    ] {
        assert!(
            FilterRule::decode(&parse_named_hex_bytes(ISOBUS_NIU_CONTROL_HEX, malformed)).is_err(),
            "{malformed} must be rejected"
        );
    }

    let mut niu = Niu::new(NiuConfig::default().mode(NiuFilterMode::BlockAll));
    niu.add_filter(decoded_rule);
    niu.start().unwrap();
    let snapshot_before = niu.filter_snapshot();
    assert_eq!(snapshot_before.len(), 1);
    assert_eq!(snapshot_before[0].pgn, PGN_PROPRIETARY_A);
    assert_eq!(snapshot_before[0].policy, ForwardPolicy::Monitor);
    assert!(snapshot_before[0].bidirectional);
    assert!(snapshot_before[0].persistent);
    assert_eq!(snapshot_before[0].max_frequency_ms, 250);
    let policy_before = niu.policy_snapshot();
    assert_eq!(policy_before.name, "NIU");
    assert_eq!(policy_before.filter_mode, NiuFilterMode::BlockAll);
    assert!(policy_before.forward_global_by_default);
    assert!(policy_before.forward_specific_by_default);
    assert_eq!(policy_before.loop_guard_window_ms, 250);
    assert_eq!(policy_before.persistence_file, None);
    assert_eq!(policy_before.filters, snapshot_before);

    let frame = Frame::from_message(
        Priority::Default,
        PGN_PROPRIETARY_A,
        0x80,
        0x42,
        &[1, 2, 3, 4, 5, 6, 7, 8],
    );
    assert!(niu.process_frame(frame, Side::Tractor, 0).is_some());
    assert!(
        niu.process_frame(frame, Side::Tractor, 100).is_none(),
        "the second matching frame is inside the 250 ms rate window"
    );
    assert!(niu.process_frame(frame, Side::Tractor, 250).is_some());
    assert_eq!(niu.forwarded(), 2);
    assert_eq!(niu.blocked(), 1);
    assert_eq!(
        niu.filter_snapshot(),
        snapshot_before,
        "runtime rate-limiter timestamps must not leak into policy snapshots"
    );
    assert_eq!(
        niu.policy_snapshot(),
        policy_before,
        "policy snapshots must also omit counters, rate timestamps, and loop-guard state"
    );

    let mut control_niu = Niu::new(NiuConfig::default());
    control_niu.start().unwrap();
    let captured = Rc::new(RefCell::new(Vec::new()));
    let captured_events = captured.clone();
    control_niu
        .on_niu_message
        .subscribe(move |event| captured_events.borrow_mut().push(*event));
    for malformed in [
        "malformed_niu_short_add_filter",
        "malformed_niu_add_filter_bad_pgn_high_bits",
        "malformed_niu_add_filter_bad_tail",
        "malformed_niu_set_filter_mode_reserved_mode",
        "malformed_niu_set_filter_mode_bad_tail",
        "malformed_niu_unknown_function",
    ] {
        control_niu.handle_niu_message(&Message::with_addressing(
            PGN_NIU_NETWORK_MSG,
            parse_named_hex_bytes(ISOBUS_NIU_CONTROL_HEX, malformed),
            0x21,
            0x80,
            Priority::Default,
        ));
    }
    assert!(control_niu.filters().is_empty());
    assert_eq!(control_niu.filter_mode(), NiuFilterMode::PassAll);
    assert!(captured.borrow().is_empty());
    let set_mode_msg = Message::with_addressing(
        PGN_NIU_NETWORK_MSG,
        set_block_all.to_vec(),
        0x21,
        0x80,
        Priority::Default,
    );
    control_niu.handle_niu_message(&set_mode_msg);
    assert_eq!(control_niu.filter_mode(), NiuFilterMode::BlockAll);
    assert_eq!(
        captured.borrow().as_slice(),
        &[(expected_set_block_all, 0x21)]
    );
    let unmatched = Frame::from_message(
        Priority::Default,
        PGN_HEARTBEAT,
        0x80,
        BROADCAST_ADDRESS,
        &[0xFF; 8],
    );
    assert!(
        control_niu
            .process_frame(unmatched, Side::Tractor, 0)
            .is_none()
    );

    let mut router = Router::new(
        NiuConfig::default()
            .name("fixture-router")
            .mode(NiuFilterMode::BlockAll)
            .global_default(false)
            .specific_default(false)
            .loop_guard_window_ms(600)
            .persistence("fixture-router.rules"),
    );
    let tractor_cf = Name::default().with_identity_number(0x100);
    let implement_cf = Name::default().with_identity_number(0x200);
    router.niu_mut().allow_pgn(PGN_PROPRIETARY_A, true);
    router.add_translation(implement_cf, 0x42, 0x52).unwrap();
    router.add_translation(tractor_cf, 0x10, 0x20).unwrap();
    let router_policy = router.policy_snapshot();
    assert_eq!(router_policy.niu.name, "fixture-router");
    assert_eq!(router_policy.niu.filter_mode, NiuFilterMode::BlockAll);
    assert!(!router_policy.niu.forward_global_by_default);
    assert!(!router_policy.niu.forward_specific_by_default);
    assert_eq!(router_policy.niu.loop_guard_window_ms, 600);
    assert_eq!(
        router_policy.niu.persistence_file.as_deref(),
        Some("fixture-router.rules")
    );
    assert_eq!(router_policy.niu.filters, router_policy.filters);
    assert_eq!(router_policy.filters.len(), 1);
    assert_eq!(router_policy.filters[0].pgn, PGN_PROPRIETARY_A);
    assert_eq!(router_policy.translations.len(), 2);
    assert_eq!(router_policy.translations[0].name, tractor_cf);
    assert_eq!(router_policy.translations[1].name, implement_cf);
}

#[test]
fn fixture_isobus_niu_router_translates_tp_etp_session_frames() {
    let raw_id =
        |name: &str| parse_hex_u64(parse_named_text_value(ISOBUS_NIU_CONTROL_HEX, name)) as u32;

    let mut router = Router::new(NiuConfig::default());
    router.niu_mut().start().unwrap();
    let tractor_cf = Name::default().with_identity_number(0x100);
    let implement_cf = Name::default().with_identity_number(0x200);
    router.add_translation(tractor_cf, 0x10, 0x20).unwrap();
    router.add_translation(implement_cf, 0x42, 0x52).unwrap();

    let tp_rts = [0x10, 20, 0, 3, 0xFF, 0x00, 0xEF, 0x00];
    let tp_cm = Frame::from_message(Priority::Default, PGN_TP_CM, 0x10, 0x42, &tp_rts);
    assert_eq!(tp_cm.id.raw, raw_id("router_tp_cm_src10_dst42_raw_id"));
    let translated_tp_cm = router
        .process_frame(tp_cm, Side::Tractor, 0)
        .expect("destination-specific TP.CM forwards when both peers translate");
    assert_eq!(
        translated_tp_cm.id.raw,
        raw_id("router_tp_cm_translated_src20_dst52_raw_id")
    );
    assert_eq!(translated_tp_cm.pgn(), PGN_TP_CM);
    assert_eq!(translated_tp_cm.source(), 0x20);
    assert_eq!(translated_tp_cm.destination(), 0x52);
    assert_eq!(translated_tp_cm.payload(), tp_rts.as_slice());

    let tp_bam = [0x20, 20, 0, 3, 0xFF, 0x00, 0xEF, 0x00];
    let tp_bam_frame = Frame::from_message(
        Priority::Default,
        PGN_TP_CM,
        0x10,
        BROADCAST_ADDRESS,
        &tp_bam,
    );
    assert_eq!(
        tp_bam_frame.id.raw,
        raw_id("router_tp_bam_src10_global_raw_id")
    );
    let translated_bam = router
        .process_frame(tp_bam_frame, Side::Tractor, 10)
        .expect("broadcast TP.BAM forwards with translated source");
    assert_eq!(
        translated_bam.id.raw,
        raw_id("router_tp_bam_translated_src20_global_raw_id")
    );
    assert_eq!(translated_bam.source(), 0x20);
    assert_eq!(translated_bam.destination(), BROADCAST_ADDRESS);
    assert_eq!(translated_bam.payload(), tp_bam.as_slice());

    let etp_dt_payload = [1, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0x11, 0x22];
    let etp_dt = Frame::from_message(Priority::Default, PGN_ETP_DT, 0x52, 0x20, &etp_dt_payload);
    assert_eq!(
        etp_dt.id.raw,
        raw_id("router_etp_dt_impl_src52_dst20_raw_id")
    );
    let translated_etp_dt = router
        .process_frame(etp_dt, Side::Implement, 20)
        .expect("ETP.DT translates in the reverse direction");
    assert_eq!(
        translated_etp_dt.id.raw,
        raw_id("router_etp_dt_translated_src42_dst10_raw_id")
    );
    assert_eq!(translated_etp_dt.pgn(), PGN_ETP_DT);
    assert_eq!(translated_etp_dt.source(), 0x42);
    assert_eq!(translated_etp_dt.destination(), 0x10);
    assert_eq!(translated_etp_dt.payload(), etp_dt_payload.as_slice());

    let unknown_dest_tp_dt = Frame::from_message(
        Priority::Default,
        PGN_TP_DT,
        0x10,
        0x43,
        &[1, 2, 3, 4, 5, 6, 7, 8],
    );
    assert!(
        router
            .process_frame(unknown_dest_tp_dt, Side::Tractor, 30)
            .is_none(),
        "destination-specific TP.DT with no destination translation must be blocked"
    );
    assert_eq!(router.niu().forwarded(), 4);
    assert_eq!(router.niu().blocked(), 1);
}

#[test]
fn fixture_isobus_niu_router_translates_address_claim_flows() {
    let raw_id =
        |name: &str| parse_hex_u64(parse_named_text_value(ISOBUS_NIU_CONTROL_HEX, name)) as u32;

    let mut router = Router::new(NiuConfig::default());
    router.niu_mut().start().unwrap();
    let tractor_cf = Name::default().with_identity_number(0x100);
    let implement_cf = Name::default().with_identity_number(0x200);
    router.add_translation(tractor_cf, 0x10, 0x20).unwrap();
    router.add_translation(implement_cf, 0x42, 0x52).unwrap();

    let tractor_claim = Frame::from_message(
        Priority::Default,
        PGN_ADDRESS_CLAIMED,
        0x10,
        BROADCAST_ADDRESS,
        &tractor_cf.to_bytes(),
    );
    assert_eq!(
        tractor_claim.id.raw,
        raw_id("router_address_claim_src10_raw_id")
    );
    let translated_claim = router
        .process_frame(tractor_claim, Side::Tractor, 0)
        .expect("matching tractor-side address claim forwards");
    assert_eq!(
        translated_claim.id.raw,
        raw_id("router_address_claim_translated_src20_raw_id")
    );
    assert_eq!(translated_claim.pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(translated_claim.source(), 0x20);
    assert_eq!(translated_claim.destination(), BROADCAST_ADDRESS);
    assert_eq!(translated_claim.payload(), tractor_cf.to_bytes().as_slice());

    let implement_claim = Frame::from_message(
        Priority::Default,
        PGN_ADDRESS_CLAIMED,
        0x52,
        BROADCAST_ADDRESS,
        &implement_cf.to_bytes(),
    );
    assert_eq!(
        implement_claim.id.raw,
        raw_id("router_address_claim_impl_src52_raw_id")
    );
    let translated_implement_claim = router
        .process_frame(implement_claim, Side::Implement, 1)
        .expect("matching implement-side address claim forwards");
    assert_eq!(
        translated_implement_claim.id.raw,
        raw_id("router_address_claim_translated_impl_src42_raw_id")
    );
    assert_eq!(translated_implement_claim.source(), 0x42);
    assert_eq!(
        translated_implement_claim.payload(),
        implement_cf.to_bytes().as_slice()
    );

    let request_payload = encode_request(PGN_ADDRESS_CLAIMED).unwrap();
    let global_request = Frame::from_message(
        Priority::Default,
        PGN_REQUEST,
        0x10,
        BROADCAST_ADDRESS,
        &request_payload,
    );
    assert_eq!(
        global_request.id.raw,
        raw_id("router_request_address_claim_src10_global_raw_id")
    );
    let translated_global_request = router
        .process_frame(global_request, Side::Tractor, 2)
        .expect("global address-claim request forwards with translated source");
    assert_eq!(
        translated_global_request.id.raw,
        raw_id("router_request_address_claim_translated_src20_global_raw_id")
    );
    assert_eq!(translated_global_request.source(), 0x20);
    assert_eq!(translated_global_request.destination(), BROADCAST_ADDRESS);
    assert_eq!(
        translated_global_request.payload(),
        request_payload.as_slice()
    );

    let specific_request =
        Frame::from_message(Priority::Default, PGN_REQUEST, 0x10, 0x42, &request_payload);
    assert_eq!(
        specific_request.id.raw,
        raw_id("router_request_address_claim_src10_dst42_raw_id")
    );
    let translated_specific_request = router
        .process_frame(specific_request, Side::Tractor, 3)
        .expect("specific address-claim request forwards when both peers translate");
    assert_eq!(
        translated_specific_request.id.raw,
        raw_id("router_request_address_claim_translated_src20_dst52_raw_id")
    );
    assert_eq!(translated_specific_request.source(), 0x20);
    assert_eq!(translated_specific_request.destination(), 0x52);
    assert_eq!(
        translated_specific_request.payload(),
        request_payload.as_slice()
    );

    let cannot_claim = Frame::from_message(
        Priority::Default,
        PGN_ADDRESS_CLAIMED,
        NULL_ADDRESS,
        BROADCAST_ADDRESS,
        &tractor_cf.to_bytes(),
    );
    assert_eq!(
        cannot_claim.id.raw,
        raw_id("router_cannot_claim_srcfe_raw_id")
    );
    let forwarded_cannot_claim = router
        .process_frame(cannot_claim, Side::Tractor, 4)
        .expect("Cannot Claim Address source 0xFE is forwarded without translation");
    assert_eq!(
        forwarded_cannot_claim.id.raw,
        raw_id("router_cannot_claim_srcfe_raw_id")
    );
    assert_eq!(forwarded_cannot_claim.source(), NULL_ADDRESS);
    assert_eq!(
        forwarded_cannot_claim.payload(),
        tractor_cf.to_bytes().as_slice()
    );

    let spoofed_claim = Frame::from_message(
        Priority::Default,
        PGN_ADDRESS_CLAIMED,
        0x10,
        BROADCAST_ADDRESS,
        &Name::default().with_identity_number(0x999).to_bytes(),
    );
    assert!(
        router
            .process_frame(spoofed_claim, Side::Tractor, 5)
            .is_none(),
        "a known translated address must not claim a different NAME"
    );

    let moved_known_name = Frame::from_message(
        Priority::Default,
        PGN_ADDRESS_CLAIMED,
        0x11,
        BROADCAST_ADDRESS,
        &tractor_cf.to_bytes(),
    );
    assert!(
        router
            .process_frame(moved_known_name, Side::Tractor, 6)
            .is_none(),
        "a known translated NAME must not claim from an unexpected address"
    );
    assert_eq!(router.niu().blocked(), 2);
}

