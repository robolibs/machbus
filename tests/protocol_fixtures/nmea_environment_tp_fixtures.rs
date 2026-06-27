#[test]
fn fixture_nmea_environmental_codecs_and_sentinels_are_stable() {
    let wind_bytes = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "wind_sid7_12_5mps_1rad_true_north",
    );
    let temp_bytes = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "temperature_sid1_outside_293_15k_set295_15k",
    );
    let humidity_bytes = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "humidity_sid2_outside_55_5pct_set60",
    );
    let negative_humidity_bytes = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "humidity_sid2_outside_neg12_5pct_set0",
    );
    let pressure_bytes = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "pressure_sid3_atmospheric_101300pa",
    );
    let negative_pressure_bytes = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "pressure_sid4_water_neg250pa",
    );
    let outside_bytes = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "outside_environmental_sid4_water285_15_air293_15_pressure101300",
    );

    assert_eq!(
        NMEAInterface::build_wind(&WindData {
            sid: 7,
            speed_mps: 12.5,
            direction_rad: 1.0,
            reference: WindReference::TrueNorth,
        }),
        wind_bytes
    );
    assert_eq!(
        NMEAInterface::build_temperature(&TemperatureData {
            sid: 1,
            instance: 0,
            source: TemperatureSource::Outside,
            actual_k: 293.15,
            set_k: 295.15,
        }),
        temp_bytes
    );
    assert_eq!(
        NMEAInterface::build_humidity(&HumidityData {
            sid: 2,
            instance: 1,
            source: HumiditySource::Outside,
            actual_pct: 55.5,
            set_pct: 60.0,
        }),
        humidity_bytes
    );
    assert_eq!(
        NMEAInterface::build_humidity(&HumidityData {
            sid: 2,
            instance: 1,
            source: HumiditySource::Outside,
            actual_pct: -12.5,
            set_pct: 0.0,
        }),
        negative_humidity_bytes
    );
    assert_eq!(
        NMEAInterface::build_pressure(&PressureData {
            sid: 3,
            instance: 2,
            source: PressureSource::Atmospheric,
            pressure_pa: 101_300.0,
        }),
        pressure_bytes
    );
    assert_eq!(
        NMEAInterface::build_pressure(&PressureData {
            sid: 4,
            instance: 0,
            source: PressureSource::Water,
            pressure_pa: -250.0,
        }),
        negative_pressure_bytes
    );
    assert_eq!(
        NMEAInterface::build_outside_environmental(&OutsideEnvironmentalData {
            sid: 4,
            water_temperature_k: 285.15,
            outside_temperature_k: 293.15,
            atmospheric_pressure_pa: 101_300.0,
        }),
        outside_bytes
    );

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let winds: Rc<RefCell<Vec<WindData>>> = Rc::new(RefCell::new(Vec::new()));
    let temperatures: Rc<RefCell<Vec<TemperatureData>>> = Rc::new(RefCell::new(Vec::new()));
    let humidities: Rc<RefCell<Vec<HumidityData>>> = Rc::new(RefCell::new(Vec::new()));
    let pressures: Rc<RefCell<Vec<PressureData>>> = Rc::new(RefCell::new(Vec::new()));
    let outside: Rc<RefCell<Vec<OutsideEnvironmentalData>>> = Rc::new(RefCell::new(Vec::new()));

    let log = winds.clone();
    iface
        .on_wind
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = temperatures.clone();
    iface
        .on_temperature
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = humidities.clone();
    iface
        .on_humidity
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = pressures.clone();
    iface
        .on_pressure
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = outside.clone();
    iface
        .on_outside_environmental
        .subscribe(move |value| log.borrow_mut().push(*value));

    iface.handle_message(&Message::new(PGN_WIND_DATA, wind_bytes.to_vec(), 0x23));
    iface.handle_message(&Message::new(PGN_TEMPERATURE, temp_bytes.to_vec(), 0x23));
    iface.handle_message(&Message::new(PGN_HUMIDITY, humidity_bytes.to_vec(), 0x23));
    iface.handle_message(&Message::new(
        PGN_HUMIDITY,
        negative_humidity_bytes.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(PGN_PRESSURE, pressure_bytes.to_vec(), 0x23));
    iface.handle_message(&Message::new(
        PGN_PRESSURE,
        negative_pressure_bytes.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_OUTSIDE_ENVIRONMENTAL,
        outside_bytes.to_vec(),
        0x23,
    ));

    assert_eq!(winds.borrow().len(), 1);
    assert!((winds.borrow()[0].speed_mps - 12.5).abs() < 0.01);
    assert!((winds.borrow()[0].direction_rad - 1.0).abs() < 0.0001);
    assert_eq!(winds.borrow()[0].reference, WindReference::TrueNorth);

    assert_eq!(temperatures.borrow().len(), 1);
    assert_eq!(temperatures.borrow()[0].source, TemperatureSource::Outside);
    assert!((temperatures.borrow()[0].actual_k - 293.15).abs() < 0.01);
    assert!((temperatures.borrow()[0].set_k - 295.15).abs() < 0.01);

    assert_eq!(humidities.borrow().len(), 2);
    assert_eq!(humidities.borrow()[0].source, HumiditySource::Outside);
    assert!((humidities.borrow()[0].actual_pct - 55.5).abs() < 0.01);
    assert!((humidities.borrow()[0].set_pct - 60.0).abs() < 0.01);
    assert_eq!(humidities.borrow()[1].source, HumiditySource::Outside);
    assert_eq!(
        humidities.borrow()[1].actual_pct,
        0.0,
        "negative outbound humidity must clamp to a non-negative wire value"
    );
    assert!((humidities.borrow()[1].set_pct - 0.0).abs() < 0.01);

    assert_eq!(pressures.borrow().len(), 2);
    assert_eq!(pressures.borrow()[0].source, PressureSource::Atmospheric);
    assert!((pressures.borrow()[0].pressure_pa - 101_300.0).abs() < 0.1);
    assert_eq!(pressures.borrow()[1].source, PressureSource::Water);
    assert!((pressures.borrow()[1].pressure_pa + 250.0).abs() < 0.1);

    assert_eq!(outside.borrow().len(), 1);
    assert!((outside.borrow()[0].water_temperature_k - 285.15).abs() < 0.01);
    assert!((outside.borrow()[0].outside_temperature_k - 293.15).abs() < 0.01);
    assert!((outside.borrow()[0].atmospheric_pressure_pa - 101_300.0).abs() < 0.1);

    iface.handle_message(&Message::new(
        PGN_WIND_DATA,
        parse_named_hex_frame(NMEA_ENVIRONMENTAL_CODECS_HEX, "wind_sid7_unavailable").to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_TEMPERATURE,
        parse_named_hex_frame(
            NMEA_ENVIRONMENTAL_CODECS_HEX,
            "temperature_sid1_outside_unavailable",
        )
        .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_HUMIDITY,
        parse_named_hex_frame(
            NMEA_ENVIRONMENTAL_CODECS_HEX,
            "humidity_sid2_outside_unavailable",
        )
        .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_PRESSURE,
        parse_named_hex_frame(
            NMEA_ENVIRONMENTAL_CODECS_HEX,
            "pressure_sid3_atmospheric_unavailable",
        )
        .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_OUTSIDE_ENVIRONMENTAL,
        parse_named_hex_frame(
            NMEA_ENVIRONMENTAL_CODECS_HEX,
            "outside_environmental_sid4_unavailable",
        )
        .to_vec(),
        0x23,
    ));

    assert_eq!(winds.borrow()[1].speed_mps, 0.0);
    assert_eq!(winds.borrow()[1].direction_rad, 0.0);
    assert_eq!(winds.borrow()[1].reference, WindReference::Unavailable);
    assert_eq!(temperatures.borrow()[1].actual_k, 0.0);
    assert_eq!(temperatures.borrow()[1].set_k, 0.0);
    assert_eq!(humidities.borrow()[2].actual_pct, 0.0);
    assert_eq!(humidities.borrow()[2].set_pct, 0.0);
    assert_eq!(pressures.borrow()[2].pressure_pa, 0.0);
    assert_eq!(outside.borrow()[1].water_temperature_k, 0.0);
    assert_eq!(outside.borrow()[1].outside_temperature_k, 0.0);
    assert_eq!(outside.borrow()[1].atmospheric_pressure_pa, 0.0);

    let counts = (
        winds.borrow().len(),
        temperatures.borrow().len(),
        humidities.borrow().len(),
        pressures.borrow().len(),
        outside.borrow().len(),
    );
    for (pgn, fixture) in [
        (PGN_WIND_DATA, "wind_bad_reserved_tail"),
        (PGN_WIND_DATA, "wind_bad_reserved_reference"),
        (PGN_TEMPERATURE, "temperature_bad_reserved_tail"),
        (PGN_TEMPERATURE, "temperature_bad_unknown_source"),
        (PGN_HUMIDITY, "humidity_bad_reserved_tail"),
        (PGN_HUMIDITY, "humidity_bad_reserved_source"),
        (PGN_PRESSURE, "pressure_bad_reserved_tail"),
        (PGN_PRESSURE, "pressure_bad_unknown_source"),
        (
            PGN_OUTSIDE_ENVIRONMENTAL,
            "outside_environmental_bad_reserved_tail",
        ),
    ] {
        iface.handle_message(&Message::new(
            pgn,
            parse_named_hex_frame(NMEA_ENVIRONMENTAL_CODECS_HEX, fixture).to_vec(),
            0x23,
        ));
    }
    assert_eq!(
        (
            winds.borrow().len(),
            temperatures.borrow().len(),
            humidities.borrow().len(),
            pressures.borrow().len(),
            outside.borrow().len(),
        ),
        counts,
        "environmental bad reserved-tail/reference/source fixtures must be ignored"
    );
}

#[test]
fn fixture_nmea_environmental_boundary_vectors_are_stable() {
    let wind_high = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "wind_sid9_clamped_high_error",
    );
    let temperature_high = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "temperature_sid9_shaft_seal_clamped_high",
    );
    let humidity_min_max = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "humidity_sid9_inside_min_max",
    );
    let pressure_high = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "pressure_sid9_oil_clamped_high",
    );
    let pressure_low = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "pressure_sid10_hydraulic_clamped_low",
    );
    let outside_high = parse_named_hex_frame(
        NMEA_ENVIRONMENTAL_CODECS_HEX,
        "outside_environmental_sid9_clamped_high",
    );

    assert_eq!(
        NMEAInterface::build_wind(&WindData {
            sid: 9,
            speed_mps: 1_000_000.0,
            direction_rad: 1_000_000.0,
            reference: WindReference::Error,
        }),
        wind_high
    );
    assert_eq!(
        NMEAInterface::build_temperature(&TemperatureData {
            sid: 9,
            instance: 2,
            source: TemperatureSource::ShaftSeal,
            actual_k: 1_000_000.0,
            set_k: 1_000_000.0,
        }),
        temperature_high
    );
    assert_eq!(
        NMEAInterface::build_humidity(&HumidityData {
            sid: 9,
            instance: 3,
            source: HumiditySource::Inside,
            actual_pct: -200.0,
            set_pct: 200.0,
        }),
        humidity_min_max
    );
    assert_eq!(
        NMEAInterface::build_pressure(&PressureData {
            sid: 9,
            instance: 7,
            source: PressureSource::Oil,
            pressure_pa: 1_000_000_000.0,
        }),
        pressure_high
    );
    assert_eq!(
        NMEAInterface::build_pressure(&PressureData {
            sid: 10,
            instance: 4,
            source: PressureSource::Hydraulic,
            pressure_pa: -1_000_000_000.0,
        }),
        pressure_low
    );
    assert_eq!(
        NMEAInterface::build_outside_environmental(&OutsideEnvironmentalData {
            sid: 9,
            water_temperature_k: 1_000_000.0,
            outside_temperature_k: 1_000_000.0,
            atmospheric_pressure_pa: 1_000_000_000.0,
        }),
        outside_high
    );

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let winds: Rc<RefCell<Vec<WindData>>> = Rc::new(RefCell::new(Vec::new()));
    let temperatures: Rc<RefCell<Vec<TemperatureData>>> = Rc::new(RefCell::new(Vec::new()));
    let humidities: Rc<RefCell<Vec<HumidityData>>> = Rc::new(RefCell::new(Vec::new()));
    let pressures: Rc<RefCell<Vec<PressureData>>> = Rc::new(RefCell::new(Vec::new()));
    let outside: Rc<RefCell<Vec<OutsideEnvironmentalData>>> = Rc::new(RefCell::new(Vec::new()));

    let log = winds.clone();
    iface
        .on_wind
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = temperatures.clone();
    iface
        .on_temperature
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = humidities.clone();
    iface
        .on_humidity
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = pressures.clone();
    iface
        .on_pressure
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = outside.clone();
    iface
        .on_outside_environmental
        .subscribe(move |value| log.borrow_mut().push(*value));

    iface.handle_message(&Message::new(PGN_WIND_DATA, wind_high.to_vec(), 0x23));
    iface.handle_message(&Message::new(
        PGN_TEMPERATURE,
        temperature_high.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(PGN_HUMIDITY, humidity_min_max.to_vec(), 0x23));
    iface.handle_message(&Message::new(PGN_PRESSURE, pressure_high.to_vec(), 0x23));
    iface.handle_message(&Message::new(PGN_PRESSURE, pressure_low.to_vec(), 0x23));
    iface.handle_message(&Message::new(
        PGN_OUTSIDE_ENVIRONMENTAL,
        outside_high.to_vec(),
        0x23,
    ));

    assert_eq!(winds.borrow()[0].reference, WindReference::Error);
    assert!((winds.borrow()[0].speed_mps - 655.32).abs() < 0.01);
    let max_canonical_wind_direction = 62_831.0 * 0.0001;
    assert!((winds.borrow()[0].direction_rad - max_canonical_wind_direction).abs() < 0.0001);

    assert_eq!(
        temperatures.borrow()[0].source,
        TemperatureSource::ShaftSeal
    );
    assert!((temperatures.borrow()[0].actual_k - 655.32).abs() < 0.01);
    assert!((temperatures.borrow()[0].set_k - 655.32).abs() < 0.01);

    assert_eq!(humidities.borrow()[0].source, HumiditySource::Inside);
    assert_eq!(humidities.borrow()[0].actual_pct, 0.0);
    assert!((humidities.borrow()[0].set_pct - 200.0).abs() < 0.001);

    assert_eq!(pressures.borrow()[0].source, PressureSource::Oil);
    assert!((pressures.borrow()[0].pressure_pa - 214_748_364.4).abs() < 0.1);
    assert_eq!(pressures.borrow()[1].source, PressureSource::Hydraulic);
    assert!((pressures.borrow()[1].pressure_pa + 214_748_364.8).abs() < 0.1);

    assert!((outside.borrow()[0].water_temperature_k - 655.32).abs() < 0.01);
    assert!((outside.borrow()[0].outside_temperature_k - 655.32).abs() < 0.01);
    assert!((outside.borrow()[0].atmospheric_pressure_pa - 6_553_200.0).abs() < 0.1);
}

#[test]
fn fixture_nmea_navigation_power_codecs_and_sentinels_are_stable() {
    let rate_bytes = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "rate_of_turn_sid5_0_1rad_per_s",
    );
    let rudder_bytes = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "rudder_inst2_port_order_neg0_1_position0_25",
    );
    let depth_bytes = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "water_depth_sid9_depth3_21_offset_neg0_25_range120",
    );
    let fluid_bytes = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "fluid_level_inst1_fuel_55pct_100l",
    );
    let battery_bytes = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "battery_inst1_12_34v_neg5_6a",
    );
    let negative_battery_bytes = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "battery_inst2_neg1_23v_0a",
    );
    let speed_water_bytes = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "speed_water_sid3_2mps_2_1mps_paddle",
    );
    let xte_bytes = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "xte_sid4_manual_terminated_1m",
    );

    assert_eq!(
        NMEAInterface::build_rudder(&RudderData {
            instance: 2,
            direction: RudderDirection::Port,
            angle_order_rad: -0.1,
            position_rad: 0.25,
        }),
        rudder_bytes
    );
    assert_eq!(
        NMEAInterface::build_depth(&WaterDepthData {
            sid: 9,
            depth_m: 3.21,
            offset_m: -0.25,
            range_m: 120.0,
        }),
        depth_bytes
    );
    assert_eq!(
        NMEAInterface::build_fluid_level(&FluidLevelData {
            instance: 1,
            r#type: FluidType::Fuel,
            level_pct: 55.0,
            capacity_l: 100.0,
        }),
        fluid_bytes
    );
    assert_eq!(
        NMEAInterface::build_battery_status(&BatteryStatusData {
            instance: 1,
            voltage: 12.34,
            current_a: -5.6,
            ..Default::default()
        }),
        battery_bytes
    );
    assert_eq!(
        NMEAInterface::build_battery_status(&BatteryStatusData {
            instance: 2,
            voltage: -1.23,
            current_a: 0.0,
            ..Default::default()
        }),
        negative_battery_bytes
    );
    assert_eq!(
        NMEAInterface::build_speed_water(&SpeedWaterData {
            sid: 3,
            water_speed_mps: 2.0,
            ground_speed_mps: 2.1,
            reference: SpeedWaterRefType::PaddleWheel,
        }),
        speed_water_bytes
    );
    assert_eq!(
        NMEAInterface::build_xte(&XTEData {
            sid: 4,
            mode: XTEMode::Manual,
            navigation_terminated: true,
            xte_m: 1.0,
        }),
        xte_bytes
    );

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&GNSSPosition {
            wgs: Wgs::new(52.0, 5.0, 0.0),
            ..Default::default()
        })
        .to_vec(),
        0x23,
    ));

    let rudders: Rc<RefCell<Vec<RudderData>>> = Rc::new(RefCell::new(Vec::new()));
    let depths: Rc<RefCell<Vec<WaterDepthData>>> = Rc::new(RefCell::new(Vec::new()));
    let fluids: Rc<RefCell<Vec<FluidLevelData>>> = Rc::new(RefCell::new(Vec::new()));
    let batteries: Rc<RefCell<Vec<BatteryStatusData>>> = Rc::new(RefCell::new(Vec::new()));
    let speed_water: Rc<RefCell<Vec<SpeedWaterData>>> = Rc::new(RefCell::new(Vec::new()));
    let xtes: Rc<RefCell<Vec<XTEData>>> = Rc::new(RefCell::new(Vec::new()));

    let log = rudders.clone();
    iface
        .on_rudder
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = depths.clone();
    iface
        .on_depth
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = fluids.clone();
    iface
        .on_fluid_level
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = batteries.clone();
    iface
        .on_battery
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = speed_water.clone();
    iface
        .on_speed_water
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = xtes.clone();
    iface
        .on_xte
        .subscribe(move |value| log.borrow_mut().push(*value));

    iface.handle_message(&Message::new(PGN_RATE_OF_TURN, rate_bytes.to_vec(), 0x23));
    iface.handle_message(&Message::new(PGN_RUDDER, rudder_bytes.to_vec(), 0x23));
    iface.handle_message(&Message::new(PGN_WATER_DEPTH, depth_bytes.to_vec(), 0x23));
    iface.handle_message(&Message::new(PGN_FLUID_LEVEL, fluid_bytes.to_vec(), 0x23));
    iface.handle_message(&Message::new(
        PGN_BATTERY_STATUS,
        battery_bytes.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_BATTERY_STATUS,
        negative_battery_bytes.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_SPEED_WATER,
        speed_water_bytes.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(PGN_XTE, xte_bytes.to_vec(), 0x23));

    let cached = iface
        .latest_position()
        .expect("rate of turn should update seeded cache");
    assert!((cached.rate_of_turn_rps.unwrap() - 0.1).abs() < 0.000001);

    assert_eq!(rudders.borrow().len(), 1);
    assert_eq!(rudders.borrow()[0].direction, RudderDirection::Port);
    assert!((rudders.borrow()[0].angle_order_rad + 0.1).abs() < 0.0001);
    assert!((rudders.borrow()[0].position_rad - 0.25).abs() < 0.0001);

    assert_eq!(depths.borrow().len(), 1);
    assert!((depths.borrow()[0].depth_m - 3.21).abs() < 0.01);
    assert!((depths.borrow()[0].offset_m + 0.25).abs() < 0.001);
    assert!((depths.borrow()[0].range_m - 120.0).abs() < 0.1);
    assert_eq!(fluids.borrow().len(), 1);
    assert_eq!(fluids.borrow()[0].instance, 1);
    assert_eq!(fluids.borrow()[0].r#type, FluidType::Fuel);
    assert!((fluids.borrow()[0].level_pct - 55.0).abs() < 0.01);
    assert!((fluids.borrow()[0].capacity_l - 100.0).abs() < 0.1);

    assert_eq!(batteries.borrow().len(), 2);
    assert!((batteries.borrow()[0].voltage - 12.34).abs() < 0.01);
    assert!((batteries.borrow()[0].current_a + 5.6).abs() < 0.1);
    assert_eq!(batteries.borrow()[1].voltage, 0.0);
    assert!((batteries.borrow()[1].current_a - 0.0).abs() < 0.1);
    assert_eq!(speed_water.borrow().len(), 1);
    assert!((speed_water.borrow()[0].water_speed_mps - 2.0).abs() < 0.01);
    assert!((speed_water.borrow()[0].ground_speed_mps - 2.1).abs() < 0.01);
    assert_eq!(
        speed_water.borrow()[0].reference,
        SpeedWaterRefType::PaddleWheel
    );
    assert_eq!(xtes.borrow().len(), 1);
    assert_eq!(xtes.borrow()[0].mode, XTEMode::Manual);
    assert!(xtes.borrow()[0].navigation_terminated);
    assert!((xtes.borrow()[0].xte_m - 1.0).abs() < 0.001);

    iface.handle_message(&Message::new(
        PGN_RATE_OF_TURN,
        parse_named_hex_frame(
            NMEA_NAVIGATION_POWER_CODECS_HEX,
            "rate_of_turn_sid5_unavailable",
        )
        .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_RUDDER,
        parse_named_hex_frame(NMEA_NAVIGATION_POWER_CODECS_HEX, "rudder_inst2_unavailable")
            .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_WATER_DEPTH,
        parse_named_hex_frame(
            NMEA_NAVIGATION_POWER_CODECS_HEX,
            "water_depth_sid9_unavailable",
        )
        .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_FLUID_LEVEL,
        parse_named_hex_frame(
            NMEA_NAVIGATION_POWER_CODECS_HEX,
            "fluid_level_inst15_unavailable",
        )
        .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_BATTERY_STATUS,
        parse_named_hex_frame(
            NMEA_NAVIGATION_POWER_CODECS_HEX,
            "battery_inst3_unavailable",
        )
        .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_SPEED_WATER,
        parse_named_hex_frame(
            NMEA_NAVIGATION_POWER_CODECS_HEX,
            "speed_water_sid3_unavailable",
        )
        .to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_XTE,
        parse_named_hex_frame(NMEA_NAVIGATION_POWER_CODECS_HEX, "xte_sid4_unavailable").to_vec(),
        0x23,
    ));

    let cached = iface
        .latest_position()
        .expect("unavailable rate of turn should preserve cache");
    assert!((cached.rate_of_turn_rps.unwrap() - 0.1).abs() < 0.000001);

    assert_eq!(rudders.borrow()[1].direction, RudderDirection::Unavailable);
    assert_eq!(rudders.borrow()[1].angle_order_rad, 0.0);
    assert_eq!(rudders.borrow()[1].position_rad, 0.0);
    assert_eq!(depths.borrow()[1].depth_m, 0.0);
    assert_eq!(depths.borrow()[1].offset_m, 0.0);
    assert_eq!(depths.borrow()[1].range_m, 0.0);
    assert_eq!(fluids.borrow()[1].instance, 15);
    assert_eq!(fluids.borrow()[1].r#type, FluidType::Unavailable);
    assert_eq!(fluids.borrow()[1].level_pct, 0.0);
    assert_eq!(fluids.borrow()[1].capacity_l, 0.0);
    assert_eq!(batteries.borrow()[2].voltage, 0.0);
    assert_eq!(batteries.borrow()[2].current_a, 0.0);
    assert_eq!(speed_water.borrow()[1].water_speed_mps, 0.0);
    assert_eq!(speed_water.borrow()[1].ground_speed_mps, 0.0);
    assert_eq!(
        speed_water.borrow()[1].reference,
        SpeedWaterRefType::Unavailable
    );
    assert_eq!(xtes.borrow()[1].mode, XTEMode::Autonomous);
    assert!(!xtes.borrow()[1].navigation_terminated);
    assert_eq!(xtes.borrow()[1].xte_m, 0.0);

    let depth_count = depths.borrow().len();
    let battery_count = batteries.borrow().len();
    iface.handle_message(&Message::new(
        PGN_WATER_DEPTH,
        parse_named_hex_bytes(NMEA_NAVIGATION_POWER_CODECS_HEX, "water_depth_sid9_short7"),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_BATTERY_STATUS,
        parse_named_hex_bytes(NMEA_NAVIGATION_POWER_CODECS_HEX, "battery_inst1_overlong9"),
        0x23,
    ));
    assert_eq!(
        depths.borrow().len(),
        depth_count,
        "short Water Depth payload must be ignored instead of prefix-decoded"
    );
    assert_eq!(
        batteries.borrow().len(),
        battery_count,
        "overlong Battery Status payload must be ignored instead of prefix-decoded"
    );

    let counts = (
        rudders.borrow().len(),
        depths.borrow().len(),
        fluids.borrow().len(),
        batteries.borrow().len(),
        speed_water.borrow().len(),
        xtes.borrow().len(),
    );
    for (pgn, fixture) in [
        (PGN_RATE_OF_TURN, "rate_of_turn_bad_reserved_tail"),
        (PGN_RUDDER, "rudder_bad_reserved_control"),
        (PGN_RUDDER, "rudder_bad_reserved_direction"),
        (PGN_RUDDER, "rudder_bad_reserved_tail"),
        (PGN_FLUID_LEVEL, "fluid_level_bad_reserved_type"),
        (PGN_BATTERY_STATUS, "battery_bad_reserved_tail"),
        (PGN_SPEED_WATER, "speed_water_bad_reserved_reference"),
        (PGN_XTE, "xte_bad_reserved_mode"),
    ] {
        iface.handle_message(&Message::new(
            pgn,
            parse_named_hex_frame(NMEA_NAVIGATION_POWER_CODECS_HEX, fixture).to_vec(),
            0x23,
        ));
    }
    assert_eq!(
        (
            rudders.borrow().len(),
            depths.borrow().len(),
            fluids.borrow().len(),
            batteries.borrow().len(),
            speed_water.borrow().len(),
            xtes.borrow().len(),
        ),
        counts,
        "navigation/power bad reserved-control/tail/type/reference/mode fixtures must be ignored"
    );
    let cached = iface
        .latest_position()
        .expect("bad rate-of-turn tail should preserve cache");
    assert!((cached.rate_of_turn_rps.unwrap() - 0.1).abs() < 0.000001);
}

#[test]
fn fixture_nmea_navigation_power_boundary_vectors_are_stable() {
    let rate_high = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "rate_of_turn_sid6_upper_non_special",
    );
    let rate_low = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "rate_of_turn_sid7_lower_raw",
    );
    let rudder_boundary = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "rudder_inst9_starboard_max_min",
    );
    let depth_boundary = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "water_depth_sid8_clamped_high_min_offset",
    );
    let battery_boundary = parse_named_hex_frame(
        NMEA_NAVIGATION_POWER_CODECS_HEX,
        "battery_inst4_max_voltage_min_current",
    );

    assert_eq!(
        NMEAInterface::build_rudder(&RudderData {
            instance: 9,
            direction: RudderDirection::Starboard,
            angle_order_rad: 100.0,
            position_rad: -100.0,
        }),
        rudder_boundary
    );
    assert_eq!(
        NMEAInterface::build_depth(&WaterDepthData {
            sid: 8,
            depth_m: 100_000_000.0,
            offset_m: -100.0,
            range_m: 100_000.0,
        }),
        depth_boundary
    );
    assert_eq!(
        NMEAInterface::build_battery_status(&BatteryStatusData {
            instance: 4,
            voltage: 1_000.0,
            current_a: -10_000.0,
            ..Default::default()
        }),
        battery_boundary
    );

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&GNSSPosition {
            wgs: Wgs::new(52.0, 5.0, 0.0),
            ..Default::default()
        })
        .to_vec(),
        0x23,
    ));

    let rudders: Rc<RefCell<Vec<RudderData>>> = Rc::new(RefCell::new(Vec::new()));
    let depths: Rc<RefCell<Vec<WaterDepthData>>> = Rc::new(RefCell::new(Vec::new()));
    let batteries: Rc<RefCell<Vec<BatteryStatusData>>> = Rc::new(RefCell::new(Vec::new()));

    let log = rudders.clone();
    iface
        .on_rudder
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = depths.clone();
    iface
        .on_depth
        .subscribe(move |value| log.borrow_mut().push(*value));
    let log = batteries.clone();
    iface
        .on_battery
        .subscribe(move |value| log.borrow_mut().push(*value));

    iface.handle_message(&Message::new(PGN_RATE_OF_TURN, rate_high.to_vec(), 0x23));
    let cached = iface
        .latest_position()
        .expect("upper rate of turn should update seeded cache");
    assert!((cached.rate_of_turn_rps.unwrap() - 67.108_863_875).abs() < 0.000_001);

    iface.handle_message(&Message::new(PGN_RATE_OF_TURN, rate_low.to_vec(), 0x23));
    let cached = iface
        .latest_position()
        .expect("lower rate of turn should update seeded cache");
    assert!((cached.rate_of_turn_rps.unwrap() + 67.108_864).abs() < 0.000_001);

    iface.handle_message(&Message::new(PGN_RUDDER, rudder_boundary.to_vec(), 0x23));
    iface.handle_message(&Message::new(
        PGN_WATER_DEPTH,
        depth_boundary.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_BATTERY_STATUS,
        battery_boundary.to_vec(),
        0x23,
    ));

    assert_eq!(rudders.borrow()[0].direction, RudderDirection::Starboard);
    assert!((rudders.borrow()[0].angle_order_rad - 3.2764).abs() < 0.0001);
    assert!((rudders.borrow()[0].position_rad + 3.2768).abs() < 0.0001);

    assert!((depths.borrow()[0].depth_m - 42_949_672.92).abs() < 0.01);
    assert!((depths.borrow()[0].offset_m + 32.768).abs() < 0.001);
    assert!((depths.borrow()[0].range_m - 2_520.0).abs() < 0.1);

    assert!((batteries.borrow()[0].voltage - 655.32).abs() < 0.01);
    assert!((batteries.borrow()[0].current_a + 3_276.8).abs() < 0.1);
}

#[test]
fn fixture_nmea_management_config_info_and_heartbeat_are_stable() {
    let config = N2KConfigInfo {
        installation_desc1: "Bridge".to_string(),
        installation_desc2: "Cabin".to_string(),
        manufacturer_info: "AcmeCo".to_string(),
    };
    let config_bytes = parse_hex_bytes(N2K_CONFIG_INFO_BRIDGE_CABIN_ACMECO_HEX.trim());
    assert_eq!(config.encode().unwrap(), config_bytes);
    assert_eq!(N2KConfigInfo::decode(&config_bytes).unwrap(), config);

    let max70 = "A".repeat(70);
    let max_config = N2KConfigInfo {
        installation_desc1: max70.clone(),
        installation_desc2: String::new(),
        manufacturer_info: String::new(),
    };
    let max70_bytes = parse_hex_bytes(N2K_CONFIG_INFO_MAX70_DESC1_HEX.trim());
    assert_eq!(max_config.encode().unwrap(), max70_bytes);
    assert_eq!(N2KConfigInfo::decode(&max70_bytes).unwrap(), max_config);

    let overlong_encode_config = N2KConfigInfo {
        installation_desc1: format!("{max70}B"),
        installation_desc2: String::new(),
        manufacturer_info: String::new(),
    };
    assert_eq!(
        overlong_encode_config.encode().unwrap_err().code,
        ErrorCode::InvalidData,
        "Config Info encode rejects strings above the 70-byte NMEA 2000 field limit"
    );
    let overlong_config_bytes = parse_hex_bytes(N2K_CONFIG_INFO_OVERLONG_DESC1_HEX.trim());
    assert!(
        N2KConfigInfo::decode(&overlong_config_bytes).is_err(),
        "Config Info decode rejects declared string lengths above 70 bytes"
    );
    let nonprintable_config_bytes = parse_named_hex_bytes(
        N2K_MANAGEMENT_MALFORMED_HEX,
        "config_info_nonprintable_manufacturer",
    );
    assert!(
        N2KConfigInfo::decode(&nonprintable_config_bytes).is_err(),
        "Config Info decode rejects non-printable length-prefixed strings"
    );
    assert!(
        N2KConfigInfo::decode(&parse_named_hex_bytes(
            N2K_MANAGEMENT_MALFORMED_HEX,
            "config_info_truncated_desc1",
        ))
        .is_err(),
        "Config Info decode rejects declared lengths beyond available bytes"
    );

    let hb = N2KHeartbeat {
        update_interval_ms: 60_000,
        sequence_counter: 5,
        controller_class1: 0xAA,
        controller_class2: 0xBB,
    };
    let hb_bytes = parse_hex_bytes(N2K_HEARTBEAT_60000MS_SEQ5_CLASSES_AABB_HEX.trim());
    assert_eq!(hb.encode().unwrap().as_slice(), hb_bytes.as_slice());
    assert_eq!(N2KHeartbeat::decode(&hb_bytes).unwrap(), hb);
    assert!(
        N2KHeartbeat::decode(&parse_hex_bytes(N2K_HEARTBEAT_BAD_RESERVED_BITS_HEX.trim()))
            .is_none(),
        "Heartbeat sequence byte must keep the high reserved nibble set"
    );
    assert!(
        N2KHeartbeat::decode(&parse_hex_bytes(N2K_HEARTBEAT_BAD_TAIL_HEX.trim())).is_none(),
        "Heartbeat bytes 5..7 must remain reserved 0xFF padding"
    );

    let mut management = N2KManagement::default();
    let configs: Rc<RefCell<Vec<(N2KConfigInfo, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let heartbeats: Rc<RefCell<Vec<(N2KHeartbeat, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let configs_log = configs.clone();
    let heartbeats_log = heartbeats.clone();
    management
        .on_config_info_received
        .subscribe(move |info| configs_log.borrow_mut().push(info.clone()));
    management
        .on_heartbeat_received
        .subscribe(move |info| heartbeats_log.borrow_mut().push(*info));

    management.handle_message(&Message::new(PGN_CONFIG_INFO, config_bytes, 0x23));
    management.handle_message(&Message::new(PGN_CONFIG_INFO, overlong_config_bytes, 0x26));
    management.handle_message(&Message::new(
        PGN_CONFIG_INFO,
        nonprintable_config_bytes,
        0x27,
    ));
    management.handle_message(&Message::new(PGN_HEARTBEAT_N2K, hb_bytes, 0x24));
    management.handle_message(&Message::new(
        PGN_HEARTBEAT_N2K,
        parse_hex_bytes(N2K_HEARTBEAT_BAD_TAIL_HEX.trim()),
        0x25,
    ));

    assert_eq!(configs.borrow().as_slice(), &[(config, 0x23)]);
    assert_eq!(heartbeats.borrow().as_slice(), &[(hb, 0x24)]);
}

#[test]
fn fixture_nmea_config_info_fast_packet_reaches_management_layer() {
    let config = N2KConfigInfo {
        installation_desc1: "Bridge".to_string(),
        installation_desc2: "Cabin".to_string(),
        manufacturer_info: "AcmeCo".to_string(),
    };
    let config_bytes = parse_hex_bytes(N2K_CONFIG_INFO_BRIDGE_CABIN_ACMECO_HEX.trim());
    assert_eq!(config.encode().unwrap(), config_bytes);
    assert_eq!(config_bytes.len(), 23);

    let expected_frames = [
        parse_named_hex_frame(N2K_CONFIG_INFO_FAST_PACKET_HEX, "frame0"),
        parse_named_hex_frame(N2K_CONFIG_INFO_FAST_PACKET_HEX, "frame1"),
        parse_named_hex_frame(N2K_CONFIG_INFO_FAST_PACKET_HEX, "frame2"),
        parse_named_hex_frame(N2K_CONFIG_INFO_FAST_PACKET_HEX, "frame3"),
    ];

    let mut tx = FastPacketProtocol::new();
    let frames = tx
        .send(PGN_CONFIG_INFO, &config_bytes, 0x23)
        .expect("Config Info payload should use NMEA fast packet");
    assert_eq!(frames.len(), expected_frames.len());
    for (frame, expected) in frames.iter().zip(expected_frames) {
        assert_eq!(frame.pgn(), PGN_CONFIG_INFO);
        assert_eq!(frame.source(), 0x23);
        assert_eq!(frame.destination(), BROADCAST_ADDRESS);
        assert_eq!(frame.data, expected);
    }

    let mut rx = FastPacketProtocol::new();
    let mut completed = None;
    for payload in expected_frames {
        let id = Identifier::encode(Priority::Default, PGN_CONFIG_INFO, 0x23, BROADCAST_ADDRESS);
        completed = rx.process_frame(&Frame::new(id, payload, 8));
    }
    let msg = completed.expect("last fast-packet frame should complete Config Info");
    assert_eq!(msg.pgn, PGN_CONFIG_INFO);
    assert_eq!(msg.source, 0x23);
    assert_eq!(msg.data, config_bytes);

    let mut management = N2KManagement::default();
    let configs: Rc<RefCell<Vec<(N2KConfigInfo, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let configs_log = configs.clone();
    management
        .on_config_info_received
        .subscribe(move |info| configs_log.borrow_mut().push(info.clone()));
    management.handle_message(&msg);
    assert_eq!(configs.borrow().as_slice(), &[(config, 0x23)]);
}

#[test]
fn fixture_nmea_fast_packet_bad_counter_drops_reassembly_session() {
    let pgn = 0x1F805;
    let id = Identifier::encode(Priority::Default, pgn, 0x23, 0xFF);
    let frame0 = Frame::new(id, *FP_10B_FRAME0, 8);
    let bad_counter = Frame::new(id, *FP_10B_BAD_COUNTER, 8);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&frame0).is_none());
    assert_eq!(rx.rx_session_count(), 1);

    assert!(rx.process_frame(&bad_counter).is_none());
    assert_eq!(rx.rx_session_count(), 0);
}

#[test]
fn fixture_nmea_fast_packet_timeout_drops_reassembly_session() {
    let pgn = 0x1F805;
    let id = Identifier::encode(Priority::Default, pgn, 0x23, 0xFF);
    let frame0 = Frame::new(id, *FP_10B_FRAME0, 8);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&frame0).is_none());
    assert_eq!(rx.rx_session_count(), 1);

    rx.update(TP_TIMEOUT_T1_MS + 1);
    assert_eq!(rx.rx_session_count(), 0);
}

#[test]
fn fixture_nmea_fast_packet_first_frame_restart_replaces_session() {
    let pgn = 0x1F805;
    let id = Identifier::encode(Priority::Default, pgn, 0x23, 0xFF);
    let old_frame0 = Frame::new(id, *FP_10B_FRAME0, 8);
    let restart_frame0 = Frame::new(id, *FP_9B_FRAME0, 8);
    let restart_frame1 = Frame::new(id, *FP_9B_FRAME1, 8);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&old_frame0).is_none());
    assert_eq!(rx.rx_session_count(), 1);

    assert!(rx.process_frame(&restart_frame0).is_none());
    assert_eq!(rx.rx_session_count(), 1);
    let msg = rx
        .process_frame(&restart_frame1)
        .expect("restart stream completes");

    assert_eq!(msg.data, (0u8..9).collect::<Vec<_>>());
    assert_eq!(rx.rx_session_count(), 0);
}

#[test]
fn fixture_nmea_fast_packet_two_sources_interleave() {
    let pgn = 0x1F805;
    let src_a_id = Identifier::encode(Priority::Default, pgn, 0x23, 0xFF);
    let src_b_id = Identifier::encode(Priority::Default, pgn, 0x24, 0xFF);
    let a0 = Frame::new(src_a_id, *FP_10B_FRAME0, 8);
    let a1 = Frame::new(src_a_id, *FP_10B_FRAME1, 8);
    let b0 = Frame::new(src_b_id, *FP_10B_FRAME0, 8);
    let b1 = Frame::new(src_b_id, *FP_10B_FRAME1, 8);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&a0).is_none());
    assert!(rx.process_frame(&b0).is_none());
    assert_eq!(rx.rx_session_count(), 2);

    let msg_b = rx.process_frame(&b1).expect("source B completes");
    assert_eq!(msg_b.source, 0x24);
    assert_eq!(msg_b.data, (0u8..10).collect::<Vec<_>>());
    assert_eq!(rx.rx_session_count(), 1);

    let msg_a = rx.process_frame(&a1).expect("source A completes");
    assert_eq!(msg_a.source, 0x23);
    assert_eq!(msg_a.data, (0u8..10).collect::<Vec<_>>());
    assert_eq!(rx.rx_session_count(), 0);
}

#[test]
fn fixture_nmea_fast_packet_same_source_different_seq_interleaves() {
    let pgn = 0x1F805;
    let id = Identifier::encode(Priority::Default, pgn, 0x23, 0xFF);
    let seq0_frame0 = Frame::new(id, *FP_10B_FRAME0, 8);
    let seq0_frame1 = Frame::new(id, *FP_10B_FRAME1, 8);
    let seq1_frame0 = Frame::new(id, *FP_10B_SEQ1_FRAME0, 8);
    let seq1_frame1 = Frame::new(id, *FP_10B_SEQ1_FRAME1, 8);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&seq0_frame0).is_none());
    assert!(rx.process_frame(&seq1_frame0).is_none());
    assert_eq!(rx.rx_session_count(), 2);

    let msg0 = rx.process_frame(&seq0_frame1).expect("seq 0 completes");
    assert_eq!(msg0.data, (0u8..10).collect::<Vec<_>>());
    assert_eq!(rx.rx_session_count(), 1);

    let msg1 = rx.process_frame(&seq1_frame1).expect("seq 1 completes");
    assert_eq!(msg1.data, (100u8..110).collect::<Vec<_>>());
    assert_eq!(rx.rx_session_count(), 0);
}

#[test]
fn fixture_nmea_fast_packet_malformed_first_lengths_are_dropped() {
    let pgn = 0x1F805;
    let id = Identifier::encode(Priority::Default, pgn, 0x23, 0xFF);
    let too_small = Frame::new(id, *FP_8B_MALFORMED_FIRST, 8);
    let too_large = Frame::new(id, *FP_224B_MALFORMED_FIRST, 8);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&too_small).is_none());
    assert_eq!(rx.rx_session_count(), 0);

    assert!(rx.process_frame(&too_large).is_none());
    assert_eq!(rx.rx_session_count(), 0);
}

#[test]
fn fixture_tp_bam_control_and_first_dt_frame_are_stable() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::new();
    let initial = tx
        .send(
            PGN_PROPRIETARY_A,
            &payload,
            0x80,
            BROADCAST_ADDRESS,
            0,
            Priority::Default,
        )
        .expect("BAM send must start");

    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].pgn(), PGN_TP_CM);
    assert_eq!(initial[0].source(), 0x80);
    assert_eq!(initial[0].destination(), BROADCAST_ADDRESS);
    assert_eq!(initial[0].data, *TP_BAM_20B_PGN_EF00);

    let dt = tx.update(TP_BAM_INTER_PACKET_MS);
    assert_eq!(dt.len(), 1);
    assert_eq!(dt[0].pgn(), PGN_TP_DT);
    assert_eq!(dt[0].source(), 0x80);
    assert_eq!(dt[0].destination(), BROADCAST_ADDRESS);
    assert_eq!(dt[0].data, *TP_DT_SEQ1_20B_PAYLOAD);
}

#[test]
fn fixture_tp_rejects_invalid_endpoint_addresses_before_session_mutation() {
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let rts = Frame::new(
            Identifier::encode(Priority::Default, PGN_TP_CM, source, 0x20),
            *TP_RTS_20B_PGN_EF00,
            8,
        );
        let mut rx = TransportProtocol::new();

        assert!(rx.process_frame(&rts, 0).is_empty());
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    for destination in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let rts = Frame::new(
            Identifier::encode(Priority::Default, PGN_TP_CM, 0x10, destination),
            *TP_RTS_20B_PGN_EF00,
            8,
        );
        let mut rx = TransportProtocol::new();

        assert!(rx.process_frame(&rts, 0).is_empty());
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
    }
}

#[test]
fn fixture_tp_bam_update_respects_exact_inter_packet_cadence() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::new();
    tx.send(
        PGN_PROPRIETARY_A,
        &payload,
        0x80,
        BROADCAST_ADDRESS,
        0,
        Priority::Default,
    )
    .expect("BAM send must start");

    assert!(
        tx.update(TP_BAM_INTER_PACKET_MS - 1).is_empty(),
        "BAM DT must not be emitted before the 50 ms inter-packet gap"
    );

    let seq1 = tx.update(1);
    assert_eq!(seq1.len(), 1);
    assert_eq!(seq1[0].data, *TP_DT_SEQ1_20B_PAYLOAD);

    assert!(
        tx.update(TP_BAM_INTER_PACKET_MS - 1).is_empty(),
        "second BAM DT must wait for a fresh 50 ms gap"
    );

    let seq2 = tx.update(1);
    assert_eq!(seq2.len(), 1);
    assert_eq!(seq2[0].data, *TP_DT_SEQ2_20B_PAYLOAD);

    let seq3 = tx.update(TP_BAM_INTER_PACKET_MS);
    assert_eq!(seq3.len(), 1);
    assert_eq!(seq3[0].data, *TP_DT_SEQ3_20B_PAYLOAD);
    assert!(
        tx.active_sessions().is_empty(),
        "BAM TX session must close after the last DT frame"
    );
    assert!(tx.update(TP_BAM_INTER_PACKET_MS).is_empty());
}

#[test]
fn fixture_tp_bam_missing_packet_drops_session_without_abort_frame() {
    let bam_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, BROADCAST_ADDRESS);
    let bam = Frame::new(bam_id, *TP_BAM_20B_PGN_EF00, 8);
    let dt_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, BROADCAST_ADDRESS);
    let dt2 = Frame::new(dt_id, *TP_DT_SEQ2_20B_PAYLOAD, 8);

    let mut rx = TransportProtocol::new();
    assert!(rx.process_frame(&bam, 0).is_empty());
    assert_eq!(rx.active_sessions().len(), 1);

    let responses = rx.process_frame(&dt2, 0);
    assert!(
        responses.is_empty(),
        "BAM receivers must not emit TP.CM aborts"
    );
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_rts_cts_and_first_dt_frame_are_stable() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::new();
    let mut rx = TransportProtocol::new();

    let rts = tx
        .send(
            PGN_PROPRIETARY_A,
            &payload,
            0x80,
            0x90,
            0,
            Priority::Default,
        )
        .expect("CMDT send must start");
    assert_eq!(rts.len(), 1);
    assert_eq!(rts[0].pgn(), PGN_TP_CM);
    assert_eq!(rts[0].source(), 0x80);
    assert_eq!(rts[0].destination(), 0x90);
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);

    let cts = rx.process_frame(&rts[0], 0);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].pgn(), PGN_TP_CM);
    assert_eq!(cts[0].source(), 0x90);
    assert_eq!(cts[0].destination(), 0x80);
    assert_eq!(cts[0].data, *TP_CTS_20B_PGN_EF00);

    assert!(tx.process_frame(&cts[0], 0).is_empty());
    let dt = tx.get_pending_data_frames();
    assert_eq!(dt.len(), 3);
    assert_eq!(dt[0].pgn(), PGN_TP_DT);
    assert_eq!(dt[0].source(), 0x80);
    assert_eq!(dt[0].destination(), 0x90);
    assert_eq!(dt[0].data, *TP_DT_SEQ1_20B_PAYLOAD);
}

#[test]
fn fixture_tp_cmdt_cts_hold_then_resume_uses_golden_frames() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::new();

    let rts = tx
        .send(
            PGN_PROPRIETARY_A,
            &payload,
            0x80,
            0x90,
            0,
            Priority::Default,
        )
        .expect("CMDT send must start");
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);

    let cts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x90, 0x80);
    let hold = Frame::new(cts_id, *TP_CTS_HOLD_PGN_EF00, 8);
    assert!(tx.process_frame(&hold, 0).is_empty());
    assert_eq!(tx.active_sessions().len(), 1);
    assert!(tx.get_pending_data_frames().is_empty());

    let resume = Frame::new(cts_id, *TP_CTS_20B_PGN_EF00, 8);
    assert!(tx.process_frame(&resume, 0).is_empty());
    let dt = tx.get_pending_data_frames();
    assert_eq!(dt.len(), 3);
    assert_eq!(dt[0].data, *TP_DT_SEQ1_20B_PAYLOAD);
    assert_eq!(dt[1].data, *TP_DT_SEQ2_20B_PAYLOAD);
    assert_eq!(dt[2].data, *TP_DT_SEQ3_20B_PAYLOAD);
}

#[test]
fn fixture_tp_cmdt_duplicate_cts_retransmits_same_golden_window() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::new();

    let rts = tx
        .send(
            PGN_PROPRIETARY_A,
            &payload,
            0x80,
            0x90,
            0,
            Priority::Default,
        )
        .expect("CMDT send must start");
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);

    let cts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x90, 0x80);
    let cts = Frame::new(cts_id, *TP_CTS_20B_PGN_EF00, 8);
    assert!(tx.process_frame(&cts, 0).is_empty());
    let first_window = tx.get_pending_data_frames();
    assert_eq!(first_window.len(), 3);
    assert_eq!(first_window[0].data, *TP_DT_SEQ1_20B_PAYLOAD);
    assert_eq!(first_window[1].data, *TP_DT_SEQ2_20B_PAYLOAD);
    assert_eq!(first_window[2].data, *TP_DT_SEQ3_20B_PAYLOAD);
    assert_eq!(
        tx.active_sessions()[0].state,
        SessionState::WaitingForEndOfMsg
    );

    // If the receiver retries CTS because the first DT burst or its EoMA
    // acknowledgement path was lost, the sender rewinds to the requested
    // packet and emits the same golden DT window instead of dropping the CTS.
    assert!(tx.process_frame(&cts, 0).is_empty());
    let retransmitted = tx.get_pending_data_frames();
    assert_eq!(retransmitted.len(), 3);
    assert_eq!(retransmitted[0].data, *TP_DT_SEQ1_20B_PAYLOAD);
    assert_eq!(retransmitted[1].data, *TP_DT_SEQ2_20B_PAYLOAD);
    assert_eq!(retransmitted[2].data, *TP_DT_SEQ3_20B_PAYLOAD);
    assert_eq!(tx.stats().dropped_frames, 0);
}

#[test]
fn fixture_tp_cmdt_cts_while_sending_aborts_with_connection_mode_error() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::new();

    let rts = tx
        .send(
            PGN_PROPRIETARY_A,
            &payload,
            0x80,
            0x90,
            0,
            Priority::Default,
        )
        .expect("CMDT send must start");
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);

    let cts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x90, 0x80);
    let cts = Frame::new(cts_id, *TP_CTS_20B_PGN_EF00, 8);
    assert!(tx.process_frame(&cts, 0).is_empty());
    assert_eq!(tx.active_sessions()[0].state, SessionState::SendingData);
    assert!(tx.process_frame(&cts, 0).is_empty());
    assert_eq!(tx.active_sessions()[0].state, SessionState::SendingData);

    let mut conflicting_cts = cts;
    conflicting_cts.data[2] = 2;
    let abort = tx.process_frame(&conflicting_cts, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x80);
    assert_eq!(abort[0].destination(), 0x90);
    assert_eq!(abort[0].data[0], 0xFF);
    assert_eq!(
        abort[0].data[1],
        machbus::net::TransportAbortReason::ConnectionModeError.as_u8()
    );
    assert_eq!(&abort[0].data[5..=7], &[0x00, 0xEF, 0x00]);
    assert!(tx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_duplicate_cts_retransmit_cap_aborts() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::with_max_retransmits(1);

    let rts = tx
        .send(
            PGN_PROPRIETARY_A,
            &payload,
            0x80,
            0x90,
            0,
            Priority::Default,
        )
        .expect("CMDT send must start");
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);

    let cts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x90, 0x80);
    let cts = Frame::new(cts_id, *TP_CTS_20B_PGN_EF00, 8);
    assert!(tx.process_frame(&cts, 0).is_empty());
    assert_eq!(tx.get_pending_data_frames().len(), 3);

    assert!(tx.process_frame(&cts, 0).is_empty());
    assert_eq!(tx.get_pending_data_frames().len(), 3);

    let abort = tx.process_frame(&cts, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x80);
    assert_eq!(abort[0].destination(), 0x90);
    assert_eq!(abort[0].data[0], 0xFF);
    assert_eq!(
        abort[0].data[1],
        machbus::net::TransportAbortReason::MaxRetransmitsExceeded.as_u8()
    );
    assert_eq!(&abort[0].data[5..=7], &[0x00, 0xEF, 0x00]);
    assert!(tx.active_sessions().is_empty());
    assert_eq!(tx.stats().aborts_sent, 1);
    assert_eq!(tx.stats().dropped_sessions, 1);
}

#[test]
fn fixture_tp_auxiliary_timer_keepalive_and_timeout_frames_are_stable() {
    let mut paused = TransportProtocol::new();
    paused.track_session(0x80, 0x90, PGN_PROPRIETARY_A, TpSessionState::WaitForCts, 0);
    paused.set_receiver_paused(0x80, 0x90, PGN_PROPRIETARY_A, 0);
    assert!(paused.update_sessions(TP_T_HOLD_MS - 1).is_empty());
    let keepalive = paused.update_sessions(1);
    assert_eq!(keepalive.len(), 1);
    assert_eq!(keepalive[0].pgn(), PGN_TP_CM);
    assert_eq!(keepalive[0].source(), 0x90);
    assert_eq!(keepalive[0].destination(), 0x80);
    assert_eq!(keepalive[0].data, *TP_CTS_HOLD_PGN_EF00);

    let mut timed_out = TransportProtocol::new();
    timed_out.track_session(0x80, 0x90, PGN_PROPRIETARY_A, TpSessionState::WaitForCts, 0);
    let abort = timed_out.update_sessions(TP_TIMEOUT_T3_MS);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x80);
    assert_eq!(abort[0].destination(), 0x90);
    assert_eq!(abort[0].data, *TP_ABORT_TIMEOUT_PGN_EF00);
    assert_eq!(timed_out.stats().timeouts, 1);
    assert_eq!(timed_out.stats().dropped_sessions, 1);
    assert_eq!(timed_out.stats().aborts_sent, 1);
}

#[test]
fn fixture_tp_cmdt_full_stream_reassembles_and_emits_golden_eoma() {
    let rts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let rts = Frame::new(rts_id, *TP_RTS_20B_PGN_EF00, 8);
    let dt_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, 0x90);
    let dt1 = Frame::new(dt_id, *TP_DT_SEQ1_20B_PAYLOAD, 8);
    let dt2 = Frame::new(dt_id, *TP_DT_SEQ2_20B_PAYLOAD, 8);
    let dt3 = Frame::new(dt_id, *TP_DT_SEQ3_20B_PAYLOAD, 8);

    let mut rx = TransportProtocol::new();
    let cts = rx.process_frame(&rts, 0);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].data, *TP_CTS_20B_PGN_EF00);

    assert!(rx.process_frame(&dt1, 0).is_empty());
    assert!(rx.process_frame(&dt2, 0).is_empty());
    let eoma = rx.process_frame(&dt3, 0);

    assert_eq!(eoma.len(), 1);
    assert_eq!(eoma[0].pgn(), PGN_TP_CM);
    assert_eq!(eoma[0].source(), 0x90);
    assert_eq!(eoma[0].destination(), 0x80);
    assert_eq!(eoma[0].data, *TP_EOMA_20B_PGN_EF00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_out_of_order_dt_aborts_with_golden_abort() {
    let rts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let rts = Frame::new(rts_id, *TP_RTS_20B_PGN_EF00, 8);
    let bad_dt_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, 0x90);
    let bad_dt = Frame::new(bad_dt_id, *TP_DT_SEQ2_20B_PAYLOAD, 8);

    let mut rx = TransportProtocol::new();
    let cts = rx.process_frame(&rts, 0);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].data, *TP_CTS_20B_PGN_EF00);

    let abort = rx.process_frame(&bad_dt, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x90);
    assert_eq!(abort[0].destination(), 0x80);
    assert_eq!(abort[0].data, *TP_ABORT_BAD_SEQUENCE_PGN_EF00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_duplicate_dt_aborts_with_golden_abort() {
    let rts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let rts = Frame::new(rts_id, *TP_RTS_20B_PGN_EF00, 8);
    let dt_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, 0x90);
    let dt = Frame::new(dt_id, *TP_DT_SEQ1_20B_PAYLOAD, 8);

    let mut rx = TransportProtocol::new();
    assert_eq!(rx.process_frame(&rts, 0).len(), 1);
    assert!(rx.process_frame(&dt, 0).is_empty());

    let abort = rx.process_frame(&dt, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x90);
    assert_eq!(abort[0].destination(), 0x80);
    assert_eq!(abort[0].data, *TP_ABORT_DUPLICATE_SEQUENCE_PGN_EF00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_wrong_peer_dt_is_ignored_without_dropping_session() {
    let rts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let rts = Frame::new(rts_id, *TP_RTS_20B_PGN_EF00, 8);
    let wrong_src_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x81, 0x90);
    let wrong_dst_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, 0x91);
    let good_dt_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, 0x90);
    let wrong_src = Frame::new(wrong_src_id, *TP_DT_SEQ1_20B_PAYLOAD, 8);
    let wrong_dst = Frame::new(wrong_dst_id, *TP_DT_SEQ1_20B_PAYLOAD, 8);
    let dt1 = Frame::new(good_dt_id, *TP_DT_SEQ1_20B_PAYLOAD, 8);
    let dt2 = Frame::new(good_dt_id, *TP_DT_SEQ2_20B_PAYLOAD, 8);
    let dt3 = Frame::new(good_dt_id, *TP_DT_SEQ3_20B_PAYLOAD, 8);

    let mut rx = TransportProtocol::new();
    assert_eq!(rx.process_frame(&rts, 0).len(), 1);

    assert!(rx.process_frame(&wrong_src, 0).is_empty());
    assert_eq!(rx.active_sessions().len(), 1);
    assert!(rx.process_frame(&wrong_dst, 0).is_empty());
    assert_eq!(rx.active_sessions().len(), 1);

    assert!(rx.process_frame(&dt1, 0).is_empty());
    assert!(rx.process_frame(&dt2, 0).is_empty());
    let eoma = rx.process_frame(&dt3, 0);
    assert_eq!(eoma.len(), 1);
    assert_eq!(eoma[0].data, *TP_EOMA_20B_PGN_EF00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_two_peer_sessions_interleave_to_eoma() {
    let rts_80_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let rts_81_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x81, 0x90);
    let rts_80 = Frame::new(rts_80_id, *TP_RTS_20B_PGN_EF00, 8);
    let rts_81 = Frame::new(rts_81_id, *TP_RTS_20B_PGN_EF00, 8);
    let dt_80_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, 0x90);
    let dt_81_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x81, 0x90);
    let dt_80_1 = Frame::new(dt_80_id, *TP_DT_SEQ1_20B_PAYLOAD, 8);
    let dt_80_2 = Frame::new(dt_80_id, *TP_DT_SEQ2_20B_PAYLOAD, 8);
    let dt_80_3 = Frame::new(dt_80_id, *TP_DT_SEQ3_20B_PAYLOAD, 8);
    let dt_81_1 = Frame::new(dt_81_id, *TP_DT_SEQ1_20B_PAYLOAD, 8);
    let dt_81_2 = Frame::new(dt_81_id, *TP_DT_SEQ2_20B_PAYLOAD, 8);
    let dt_81_3 = Frame::new(dt_81_id, *TP_DT_SEQ3_20B_PAYLOAD, 8);

    let mut rx = TransportProtocol::new();
    let cts_80 = rx.process_frame(&rts_80, 0);
    let cts_81 = rx.process_frame(&rts_81, 0);
    assert_eq!(cts_80.len(), 1);
    assert_eq!(cts_80[0].source(), 0x90);
    assert_eq!(cts_80[0].destination(), 0x80);
    assert_eq!(cts_80[0].data, *TP_CTS_20B_PGN_EF00);
    assert_eq!(cts_81.len(), 1);
    assert_eq!(cts_81[0].source(), 0x90);
    assert_eq!(cts_81[0].destination(), 0x81);
    assert_eq!(cts_81[0].data, *TP_CTS_20B_PGN_EF00);

    assert!(rx.process_frame(&dt_80_1, 0).is_empty());
    assert!(rx.process_frame(&dt_81_1, 0).is_empty());
    assert!(rx.process_frame(&dt_80_2, 0).is_empty());
    assert!(rx.process_frame(&dt_81_2, 0).is_empty());

    let eoma_80 = rx.process_frame(&dt_80_3, 0);
    assert_eq!(eoma_80.len(), 1);
    assert_eq!(eoma_80[0].source(), 0x90);
    assert_eq!(eoma_80[0].destination(), 0x80);
    assert_eq!(eoma_80[0].data, *TP_EOMA_20B_PGN_EF00);

    let eoma_81 = rx.process_frame(&dt_81_3, 0);
    assert_eq!(eoma_81.len(), 1);
    assert_eq!(eoma_81[0].source(), 0x90);
    assert_eq!(eoma_81[0].destination(), 0x81);
    assert_eq!(eoma_81[0].data, *TP_EOMA_20B_PGN_EF00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_timeout_emits_golden_abort() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::new();

    let rts = tx
        .send(
            PGN_PROPRIETARY_A,
            &payload,
            0x80,
            0x90,
            0,
            Priority::Default,
        )
        .expect("CMDT send must start");
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);

    let abort = tx.update(TP_TIMEOUT_T3_MS + 1);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x80);
    assert_eq!(abort[0].destination(), 0x90);
    assert_eq!(abort[0].data, *TP_ABORT_TIMEOUT_PGN_EF00);
    assert!(tx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_missing_eoma_timeout_emits_golden_abort() {
    let payload = incrementing_payload(20);
    let mut tx = TransportProtocol::new();

    let rts = tx
        .send(
            PGN_PROPRIETARY_A,
            &payload,
            0x80,
            0x90,
            0,
            Priority::Default,
        )
        .expect("CMDT send must start");
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);

    let cts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x90, 0x80);
    let cts = Frame::new(cts_id, *TP_CTS_20B_PGN_EF00, 8);
    assert!(tx.process_frame(&cts, 0).is_empty());

    let dt = tx.get_pending_data_frames();
    assert_eq!(dt.len(), 3);
    assert_eq!(dt[0].data, *TP_DT_SEQ1_20B_PAYLOAD);
    assert_eq!(dt[1].data, *TP_DT_SEQ2_20B_PAYLOAD);
    assert_eq!(dt[2].data, *TP_DT_SEQ3_20B_PAYLOAD);

    let abort = tx.update(TP_TIMEOUT_T3_MS + 1);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x80);
    assert_eq!(abort[0].destination(), 0x90);
    assert_eq!(abort[0].data, *TP_ABORT_TIMEOUT_PGN_EF00);
    assert!(tx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_receive_timeout_emits_golden_abort() {
    let rts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let rts = Frame::new(rts_id, *TP_RTS_20B_PGN_EF00, 8);

    let mut rx = TransportProtocol::new();
    let cts = rx.process_frame(&rts, 0);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].data, *TP_CTS_20B_PGN_EF00);

    let abort = rx.update(TP_TIMEOUT_T1_MS + 1);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x90);
    assert_eq!(abort[0].destination(), 0x80);
    assert_eq!(abort[0].data, *TP_ABORT_TIMEOUT_PGN_EF00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_cmdt_receive_cap_aborts_with_golden_no_resources() {
    let rts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let rts = Frame::new(rts_id, *TP_RTS_20B_PGN_EF00, 8);

    let mut rx = TransportProtocol::with_max_receive_bytes(19);
    let abort = rx.process_frame(&rts, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x90);
    assert_eq!(abort[0].destination(), 0x80);
    assert_eq!(abort[0].data, *TP_ABORT_NO_RESOURCES_PGN_EF00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_tp_malformed_rts_aborts_with_golden_abort() {
    let rts_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let malformed = Frame::new(rts_id, *TP_RTS_MALFORMED_1786B_255PKTS_PGN_EF00, 8);

    let mut rx = TransportProtocol::new();
    let abort = rx.process_frame(&malformed, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_TP_CM);
    assert_eq!(abort[0].source(), 0x90);
    assert_eq!(abort[0].destination(), 0x80);
    assert_eq!(abort[0].data, *TP_ABORT_UNEXPECTED_SIZE_PGN_EF00);
    assert!(rx.active_sessions().is_empty());
}

