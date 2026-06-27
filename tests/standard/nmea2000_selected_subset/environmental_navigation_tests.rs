#[test]
fn nmea2000_water_depth_rejects_reserved_depth_and_offset_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let depths: Rc<RefCell<Vec<WaterDepthData>>> = Rc::new(RefCell::new(Vec::new()));
    let depth_log = depths.clone();
    iface
        .on_depth
        .subscribe(move |depth| depth_log.borrow_mut().push(*depth));

    for reserved_depth in [u32::MAX - 2, u32::MAX - 1] {
        let mut frame = NMEAInterface::build_depth(&WaterDepthData {
            sid: 9,
            depth_m: 3.21,
            offset_m: -0.25,
            range_m: 120.0,
        });
        frame[1..5].copy_from_slice(&reserved_depth.to_le_bytes());
        iface.handle_message(&Message::new(PGN_WATER_DEPTH, frame.to_vec(), 0x24));
    }

    let mut reserved_offset = NMEAInterface::build_depth(&WaterDepthData {
        sid: 9,
        depth_m: 3.21,
        offset_m: -0.25,
        range_m: 120.0,
    });
    reserved_offset[5..7].copy_from_slice(&(i16::MAX - 2).to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_WATER_DEPTH,
        reserved_offset.to_vec(),
        0x24,
    ));

    assert!(depths.borrow().is_empty());

    let valid = NMEAInterface::build_depth(&WaterDepthData {
        sid: 9,
        depth_m: 3.21,
        offset_m: -0.25,
        range_m: 120.0,
    });
    iface.handle_message(&Message::new(PGN_WATER_DEPTH, valid.to_vec(), 0x24));
    assert_eq!(depths.borrow().len(), 1);
    assert!((depths.borrow()[0].depth_m - 3.21).abs() < 0.01);
    assert!((depths.borrow()[0].offset_m + 0.25).abs() < 0.01);
}

#[test]
fn nmea2000_fluid_level_rejects_reserved_numeric_special_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let fluids: Rc<RefCell<Vec<FluidLevelData>>> = Rc::new(RefCell::new(Vec::new()));
    let fluid_log = fluids.clone();
    iface
        .on_fluid_level
        .subscribe(move |fluid| fluid_log.borrow_mut().push(*fluid));

    let valid = NMEAInterface::build_fluid_level(&FluidLevelData {
        instance: 1,
        r#type: FluidType::Fuel,
        level_pct: 55.0,
        capacity_l: 100.0,
    });
    for raw_level in [0xFFFDu16, 0xFFFE] {
        let mut reserved_level = valid;
        reserved_level[1..3].copy_from_slice(&raw_level.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_FLUID_LEVEL,
            reserved_level.to_vec(),
            0x24,
        ));
    }
    for raw_capacity in [0xFFFF_FFFDu32, 0xFFFF_FFFE] {
        let mut reserved_capacity = valid;
        reserved_capacity[3..7].copy_from_slice(&raw_capacity.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_FLUID_LEVEL,
            reserved_capacity.to_vec(),
            0x24,
        ));
    }
    assert!(fluids.borrow().is_empty());

    iface.handle_message(&Message::new(PGN_FLUID_LEVEL, valid.to_vec(), 0x24));
    assert_eq!(fluids.borrow().len(), 1);
    assert_eq!(fluids.borrow()[0].r#type, FluidType::Fuel);
}

#[test]
fn nmea2000_speed_water_rejects_reserved_numeric_special_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let speeds: Rc<RefCell<Vec<SpeedWaterData>>> = Rc::new(RefCell::new(Vec::new()));
    let speed_log = speeds.clone();
    iface
        .on_speed_water
        .subscribe(move |speed| speed_log.borrow_mut().push(*speed));

    let valid = NMEAInterface::build_speed_water(&SpeedWaterData {
        sid: 3,
        water_speed_mps: 2.0,
        ground_speed_mps: 2.1,
        reference: SpeedWaterRefType::PaddleWheel,
    });
    for raw_water in [0xFFFDu16, 0xFFFE] {
        let mut reserved_water = valid;
        reserved_water[1..3].copy_from_slice(&raw_water.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_SPEED_WATER,
            reserved_water.to_vec(),
            0x24,
        ));
    }
    for raw_ground in [0xFFFDu16, 0xFFFE] {
        let mut reserved_ground = valid;
        reserved_ground[3..5].copy_from_slice(&raw_ground.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_SPEED_WATER,
            reserved_ground.to_vec(),
            0x24,
        ));
    }
    assert!(speeds.borrow().is_empty());

    iface.handle_message(&Message::new(PGN_SPEED_WATER, valid.to_vec(), 0x24));
    assert_eq!(speeds.borrow().len(), 1);
    assert_eq!(speeds.borrow()[0].reference, SpeedWaterRefType::PaddleWheel);
}

#[test]
fn nmea2000_humidity_rejects_reserved_and_does_not_emit_negative_percent() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let humidities: Rc<RefCell<Vec<HumidityData>>> = Rc::new(RefCell::new(Vec::new()));
    let humidity_log = humidities.clone();
    iface
        .on_humidity
        .subscribe(move |humidity| humidity_log.borrow_mut().push(*humidity));

    let valid = NMEAInterface::build_humidity(&HumidityData {
        sid: 2,
        instance: 1,
        source: HumiditySource::Outside,
        actual_pct: 55.5,
        set_pct: 60.0,
    });
    for raw_actual in [0xFFFDu16, 0xFFFE] {
        let mut reserved_actual = valid;
        reserved_actual[3..5].copy_from_slice(&raw_actual.to_le_bytes());
        iface.handle_message(&Message::new(PGN_HUMIDITY, reserved_actual.to_vec(), 0x24));
    }
    for raw_set in [0xFFFDu16, 0xFFFE] {
        let mut reserved_set = valid;
        reserved_set[5..7].copy_from_slice(&raw_set.to_le_bytes());
        iface.handle_message(&Message::new(PGN_HUMIDITY, reserved_set.to_vec(), 0x24));
    }
    assert!(
        humidities.borrow().is_empty(),
        "reserved humidity field values must not emit humidity events"
    );

    let mut unavailable = valid;
    unavailable[3..5].copy_from_slice(&0xFFFFu16.to_le_bytes());
    unavailable[5..7].copy_from_slice(&0xFFFFu16.to_le_bytes());
    iface.handle_message(&Message::new(PGN_HUMIDITY, unavailable.to_vec(), 0x24));
    assert_eq!(humidities.borrow().len(), 1);
    assert_eq!(humidities.borrow()[0].source, HumiditySource::Outside);
    assert_eq!(
        humidities.borrow()[0].actual_pct,
        0.0,
        "unavailable actual humidity must not decode as a negative percentage"
    );
    assert_eq!(
        humidities.borrow()[0].set_pct,
        0.0,
        "unavailable set humidity must not decode as a negative percentage"
    );

    iface.handle_message(&Message::new(PGN_HUMIDITY, valid.to_vec(), 0x24));
    assert_eq!(humidities.borrow().len(), 2);
    assert!((humidities.borrow()[1].actual_pct - 55.5).abs() < HUMIDITY_RESOLUTION);
    assert!((humidities.borrow()[1].set_pct - 60.0).abs() < HUMIDITY_RESOLUTION);

    let negative = NMEAInterface::build_humidity(&HumidityData {
        actual_pct: -1.0,
        set_pct: -1.0,
        ..Default::default()
    });
    assert_eq!(
        u16::from_le_bytes([negative[3], negative[4]]),
        0,
        "humidity builder must not wrap negative actual percentages into the unsigned special-value range"
    );
    assert_eq!(
        u16::from_le_bytes([negative[5], negative[6]]),
        0,
        "humidity builder must not wrap negative set percentages into the unsigned special-value range"
    );
}

#[test]
fn nmea2000_wind_data_rejects_reserved_numeric_special_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let winds: Rc<RefCell<Vec<WindData>>> = Rc::new(RefCell::new(Vec::new()));
    let wind_log = winds.clone();
    iface
        .on_wind
        .subscribe(move |wind| wind_log.borrow_mut().push(*wind));

    let valid = NMEAInterface::build_wind(&WindData {
        sid: 7,
        speed_mps: 12.5,
        direction_rad: 1.0,
        reference: WindReference::TrueNorth,
    });
    for raw_speed in [0xFFFDu16, 0xFFFE] {
        let mut reserved_speed = valid;
        reserved_speed[1..3].copy_from_slice(&raw_speed.to_le_bytes());
        iface.handle_message(&Message::new(PGN_WIND_DATA, reserved_speed.to_vec(), 0x24));
    }
    for raw_direction in [0xFFFDu16, 0xFFFE] {
        let mut reserved_direction = valid;
        reserved_direction[3..5].copy_from_slice(&raw_direction.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_WIND_DATA,
            reserved_direction.to_vec(),
            0x24,
        ));
    }
    let mut out_of_range_direction = valid;
    out_of_range_direction[3..5].copy_from_slice(&62_832u16.to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_WIND_DATA,
        out_of_range_direction.to_vec(),
        0x24,
    ));
    assert!(winds.borrow().is_empty());

    iface.handle_message(&Message::new(PGN_WIND_DATA, valid.to_vec(), 0x24));
    assert_eq!(winds.borrow().len(), 1);
    assert_eq!(winds.borrow()[0].reference, WindReference::TrueNorth);

    let clamped = NMEAInterface::build_wind(&WindData {
        sid: 7,
        speed_mps: 12.5,
        direction_rad: std::f64::consts::TAU + 1.0,
        reference: WindReference::TrueNorth,
    });
    assert_eq!(
        u16::from_le_bytes([clamped[3], clamped[4]]),
        62_831,
        "wind direction builder must not emit circular-angle values above one revolution"
    );
}

#[test]
fn nmea2000_wind_data_rejects_noncanonical_reference_bytes_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let winds: Rc<RefCell<Vec<WindData>>> = Rc::new(RefCell::new(Vec::new()));
    let wind_log = winds.clone();
    iface
        .on_wind
        .subscribe(move |wind| wind_log.borrow_mut().push(*wind));

    let valid = NMEAInterface::build_wind(&WindData {
        sid: 7,
        speed_mps: 12.5,
        direction_rad: 1.0,
        reference: WindReference::TrueNorth,
    });

    for reference in [0x05, 0x08, 0xFE] {
        let mut noncanonical = valid;
        noncanonical[5] = reference;
        iface.handle_message(&Message::new(PGN_WIND_DATA, noncanonical.to_vec(), 0x24));
    }
    assert!(winds.borrow().is_empty());

    for (reference, expected) in [
        (WindReference::Error.as_u8(), WindReference::Error),
        (0xFF, WindReference::Unavailable),
    ] {
        let mut accepted = valid;
        accepted[5] = reference;
        iface.handle_message(&Message::new(PGN_WIND_DATA, accepted.to_vec(), 0x24));
        assert_eq!(winds.borrow().last().unwrap().reference, expected);
    }

    assert_eq!(winds.borrow().len(), 2);
}

#[test]
fn nmea2000_selected_categorical_fields_reject_reserved_values_before_events() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let events = Rc::new(RefCell::new(0usize));

    macro_rules! count_event {
        ($event:expr) => {{
            let log = events.clone();
            $event.subscribe(move |_| *log.borrow_mut() += 1);
        }};
    }

    count_event!(iface.on_wind);
    count_event!(iface.on_temperature);
    count_event!(iface.on_system_time);
    count_event!(iface.on_magnetic_variation);
    count_event!(iface.on_rudder);
    count_event!(iface.on_fluid_level);
    count_event!(iface.on_speed_water);
    count_event!(iface.on_humidity);

    let mut wind = NMEAInterface::build_wind(&WindData {
        sid: 7,
        speed_mps: 12.5,
        direction_rad: 1.0,
        reference: WindReference::TrueNorth,
    });
    wind[5] = 5;
    iface.handle_message(&Message::new(PGN_WIND_DATA, wind.to_vec(), 0x24));

    let mut temperature = NMEAInterface::build_temperature(&TemperatureData {
        sid: 1,
        instance: 2,
        source: TemperatureSource::Outside,
        actual_k: 293.15,
        set_k: 295.15,
    });
    temperature[2] = 16;
    iface.handle_message(&Message::new(PGN_TEMPERATURE, temperature.to_vec(), 0x24));

    let mut time = NMEAInterface::build_system_time(&SystemTimeData {
        sid: 5,
        source: TimeSource::GPS,
        days_since_epoch: 12_345,
        seconds_since_midnight: 3_600.5,
    });
    time[1] = 6;
    iface.handle_message(&Message::new(PGN_SYSTEM_TIME, time.to_vec(), 0x24));

    let mut variation = NMEAInterface::build_magnetic_variation(0.1, 5);
    variation[1] = 10;
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        variation.to_vec(),
        0x24,
    ));

    let mut rudder = NMEAInterface::build_rudder(&RudderData {
        instance: 2,
        direction: RudderDirection::Port,
        angle_order_rad: -0.1,
        position_rad: 0.25,
    });
    rudder[1] = 0xF8 | 3;
    iface.handle_message(&Message::new(PGN_RUDDER, rudder.to_vec(), 0x24));

    let mut fluid = NMEAInterface::build_fluid_level(&FluidLevelData {
        instance: 1,
        r#type: FluidType::Fuel,
        level_pct: 55.0,
        capacity_l: 100.0,
    });
    fluid[0] = (7 << 4) | 1;
    iface.handle_message(&Message::new(PGN_FLUID_LEVEL, fluid.to_vec(), 0x24));

    let mut speed = NMEAInterface::build_speed_water(&SpeedWaterData {
        sid: 3,
        water_speed_mps: 2.0,
        ground_speed_mps: 2.1,
        reference: SpeedWaterRefType::PaddleWheel,
    });
    speed[5] = 5;
    iface.handle_message(&Message::new(PGN_SPEED_WATER, speed.to_vec(), 0x24));

    let mut humidity = NMEAInterface::build_humidity(&HumidityData {
        sid: 3,
        instance: 1,
        source: HumiditySource::Outside,
        actual_pct: 55.0,
        set_pct: 60.0,
    });
    humidity[2] = 2;
    iface.handle_message(&Message::new(PGN_HUMIDITY, humidity.to_vec(), 0x24));

    assert_eq!(
        *events.borrow(),
        0,
        "reserved categorical values must not emit selected-PGN events"
    );

    iface.handle_message(&Message::new(
        PGN_WIND_DATA,
        NMEAInterface::build_wind(&WindData {
            sid: 7,
            speed_mps: 12.5,
            direction_rad: 1.0,
            reference: WindReference::TrueNorth,
        })
        .to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_TEMPERATURE,
        NMEAInterface::build_temperature(&TemperatureData {
            sid: 1,
            instance: 2,
            source: TemperatureSource::Outside,
            actual_k: 293.15,
            set_k: 295.15,
        })
        .to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_SYSTEM_TIME,
        NMEAInterface::build_system_time(&SystemTimeData {
            sid: 5,
            source: TimeSource::GPS,
            days_since_epoch: 12_345,
            seconds_since_midnight: 3_600.5,
        })
        .to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        NMEAInterface::build_magnetic_variation(0.1, 5).to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_RUDDER,
        NMEAInterface::build_rudder(&RudderData {
            instance: 2,
            direction: RudderDirection::Port,
            angle_order_rad: -0.1,
            position_rad: 0.25,
        })
        .to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_FLUID_LEVEL,
        NMEAInterface::build_fluid_level(&FluidLevelData {
            instance: 1,
            r#type: FluidType::Fuel,
            level_pct: 55.0,
            capacity_l: 100.0,
        })
        .to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_SPEED_WATER,
        NMEAInterface::build_speed_water(&SpeedWaterData {
            sid: 3,
            water_speed_mps: 2.0,
            ground_speed_mps: 2.1,
            reference: SpeedWaterRefType::PaddleWheel,
        })
        .to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_HUMIDITY,
        NMEAInterface::build_humidity(&HumidityData {
            sid: 3,
            instance: 1,
            source: HumiditySource::Outside,
            actual_pct: 55.0,
            set_pct: 60.0,
        })
        .to_vec(),
        0x24,
    ));

    assert_eq!(
        *events.borrow(),
        8,
        "valid categorical values should still reach the selected-PGN event surface"
    );
}

#[test]
fn nmea2000_selected_sequence_ids_reject_reserved_values_before_events_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));
    let cached = iface.latest_position().unwrap();
    let events = Rc::new(RefCell::new(0usize));

    macro_rules! count_event {
        ($event:expr) => {{
            let log = events.clone();
            $event.subscribe(move |_| *log.borrow_mut() += 1);
        }};
    }

    count_event!(iface.on_position);
    count_event!(iface.on_cog);
    count_event!(iface.on_sog);
    count_event!(iface.on_position_delta);
    count_event!(iface.on_attitude);
    count_event!(iface.on_wind);
    count_event!(iface.on_temperature);
    count_event!(iface.on_depth);
    count_event!(iface.on_heading);
    count_event!(iface.on_system_time);
    count_event!(iface.on_gnss_dops);
    count_event!(iface.on_magnetic_variation);
    count_event!(iface.on_speed_water);
    count_event!(iface.on_xte);
    count_event!(iface.on_humidity);
    count_event!(iface.on_pressure);
    count_event!(iface.on_outside_environmental);

    let sid_frames: Vec<(u32, Vec<u8>)> = vec![
        (
            PGN_GNSS_COG_SOG_RAPID,
            NMEAInterface::build_cog_sog(1.0, 2.0).to_vec(),
        ),
        (
            PGN_GNSS_POSITION_DELTA,
            NMEAInterface::build_position_delta(&PositionDeltaHighPrecisionRapidUpdateData {
                sid: 1,
                time_delta_s: 0.25,
                latitude_delta_deg: 0.000_001,
                longitude_delta_deg: -0.000_001,
            })
            .to_vec(),
        ),
        (PGN_ATTITUDE, {
            let mut data = [0xFFu8; 8];
            data[0] = 1;
            data[1..3].copy_from_slice(&10i16.to_le_bytes());
            data[3..5].copy_from_slice(&20i16.to_le_bytes());
            data[5..7].copy_from_slice(&30i16.to_le_bytes());
            data.to_vec()
        }),
        (PGN_RATE_OF_TURN, {
            let mut data = [0xFFu8; 8];
            data[0] = 1;
            data[1..5].copy_from_slice(&100i32.to_le_bytes());
            data.to_vec()
        }),
        (
            PGN_WIND_DATA,
            NMEAInterface::build_wind(&WindData {
                sid: 2,
                speed_mps: 4.0,
                direction_rad: 1.0,
                reference: WindReference::Apparent,
            })
            .to_vec(),
        ),
        (
            PGN_TEMPERATURE,
            NMEAInterface::build_temperature(&TemperatureData {
                sid: 3,
                instance: 2,
                source: TemperatureSource::Outside,
                actual_k: 293.15,
                set_k: 295.15,
            })
            .to_vec(),
        ),
        (
            PGN_WATER_DEPTH,
            NMEAInterface::build_depth(&WaterDepthData {
                sid: 4,
                depth_m: 3.21,
                offset_m: -0.25,
                range_m: 120.0,
            })
            .to_vec(),
        ),
        (PGN_HEADING_TRACK, {
            let mut data = NMEAInterface::build_heading(1.0, 0.0, 0.0);
            data[0] = 5;
            data.to_vec()
        }),
        (
            PGN_SYSTEM_TIME,
            NMEAInterface::build_system_time(&SystemTimeData {
                sid: 6,
                source: TimeSource::GPS,
                days_since_epoch: 12_345,
                seconds_since_midnight: 3_600.5,
            })
            .to_vec(),
        ),
        (PGN_GNSS_DOPS, {
            let mut data = [0xFFu8; 8];
            data[0] = 7;
            data[1] = 0x1B;
            data[2..4].copy_from_slice(&100u16.to_le_bytes());
            data[4..6].copy_from_slice(&150u16.to_le_bytes());
            data[6..8].copy_from_slice(&200u16.to_le_bytes());
            data.to_vec()
        }),
        (PGN_MAGNETIC_VARIATION, {
            let mut data = NMEAInterface::build_magnetic_variation(0.1, 5);
            data[0] = 8;
            data.to_vec()
        }),
        (
            PGN_SPEED_WATER,
            NMEAInterface::build_speed_water(&SpeedWaterData {
                sid: 9,
                water_speed_mps: 2.0,
                ground_speed_mps: 2.1,
                reference: SpeedWaterRefType::PaddleWheel,
            })
            .to_vec(),
        ),
        (
            PGN_XTE,
            NMEAInterface::build_xte(&XTEData {
                sid: 10,
                mode: XTEMode::Manual,
                navigation_terminated: true,
                xte_m: -1.25,
            })
            .to_vec(),
        ),
        (
            PGN_HUMIDITY,
            NMEAInterface::build_humidity(&HumidityData {
                sid: 11,
                instance: 1,
                source: HumiditySource::Outside,
                actual_pct: 55.0,
                set_pct: 60.0,
            })
            .to_vec(),
        ),
        (
            PGN_PRESSURE,
            NMEAInterface::build_pressure(&PressureData {
                sid: 12,
                instance: 1,
                source: PressureSource::Atmospheric,
                pressure_pa: 101_300.0,
            })
            .to_vec(),
        ),
        (
            PGN_OUTSIDE_ENVIRONMENTAL,
            NMEAInterface::build_outside_environmental(&OutsideEnvironmentalData {
                sid: 13,
                water_temperature_k: 285.0,
                outside_temperature_k: 291.0,
                atmospheric_pressure_pa: 101_000.0,
            })
            .to_vec(),
        ),
        (PGN_GNSS_POSITION_DATA, standard_position_detail_frame()),
    ];

    for reserved_sid in [0xFD, 0xFE] {
        for (pgn, frame) in &sid_frames {
            let mut reserved = frame.clone();
            reserved[0] = reserved_sid;
            iface.handle_message(&Message::new(*pgn, reserved, 0x24));
        }
    }

    assert_eq!(
        *events.borrow(),
        0,
        "reserved sequence identifiers must not emit selected-PGN events"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "reserved sequence identifiers must not mutate the GNSS cache"
    );
}

#[test]
fn nmea2000_selected_builders_do_not_emit_reserved_sequence_ids_or_pressure_source() {
    let position_delta =
        NMEAInterface::build_position_delta(&PositionDeltaHighPrecisionRapidUpdateData {
            sid: 0xFD,
            time_delta_s: 0.25,
            latitude_delta_deg: 0.000_001,
            longitude_delta_deg: -0.000_001,
        });
    let wind = NMEAInterface::build_wind(&WindData {
        sid: 0xFE,
        speed_mps: 4.0,
        direction_rad: 1.0,
        reference: WindReference::Apparent,
    });
    let temperature = NMEAInterface::build_temperature(&TemperatureData {
        sid: 0xFD,
        instance: 2,
        source: TemperatureSource::Outside,
        actual_k: 293.15,
        set_k: 295.15,
    });
    let depth = NMEAInterface::build_depth(&WaterDepthData {
        sid: 0xFE,
        depth_m: 3.21,
        offset_m: -0.25,
        range_m: 120.0,
    });
    let system_time = NMEAInterface::build_system_time(&SystemTimeData {
        sid: 0xFD,
        source: TimeSource::GPS,
        days_since_epoch: 12_345,
        seconds_since_midnight: 3_600.5,
    });
    let speed_water = NMEAInterface::build_speed_water(&SpeedWaterData {
        sid: 0xFE,
        water_speed_mps: 2.0,
        ground_speed_mps: 2.1,
        reference: SpeedWaterRefType::PaddleWheel,
    });
    let xte = NMEAInterface::build_xte(&XTEData {
        sid: 0xFD,
        mode: XTEMode::Manual,
        navigation_terminated: true,
        xte_m: -1.25,
    });
    let humidity = NMEAInterface::build_humidity(&HumidityData {
        sid: 0xFE,
        instance: 1,
        source: HumiditySource::Outside,
        actual_pct: 55.0,
        set_pct: 60.0,
    });
    let pressure = NMEAInterface::build_pressure(&PressureData {
        sid: 0xFD,
        instance: 1,
        source: PressureSource::Reserved,
        pressure_pa: 101_300.0,
    });
    let outside = NMEAInterface::build_outside_environmental(&OutsideEnvironmentalData {
        sid: 0xFE,
        water_temperature_k: 285.0,
        outside_temperature_k: 291.0,
        atmospheric_pressure_pa: 101_000.0,
    });

    for frame in [
        position_delta,
        wind,
        temperature,
        depth,
        system_time,
        speed_water,
        xte,
        humidity,
        pressure,
        outside,
    ] {
        assert_eq!(
            frame[0], 0xFF,
            "selected-PGN builders must encode invalid SIDs as unavailable not reserved"
        );
    }
    assert_eq!(
        pressure[2],
        PressureSource::Unavailable.as_u8(),
        "pressure builder must not emit the reserved source byte"
    );
    let reserved_system_time_date = NMEAInterface::build_system_time(&SystemTimeData {
        sid: 0xFC,
        source: TimeSource::GPS,
        days_since_epoch: 0xFFFD,
        seconds_since_midnight: 86_402.0,
    });
    assert_eq!(
        u16::from_le_bytes([reserved_system_time_date[2], reserved_system_time_date[3]]),
        0xFFFF,
        "system-time builder must not emit reserved date bytes"
    );
    assert_eq!(
        u32::from_le_bytes([
            reserved_system_time_date[4],
            reserved_system_time_date[5],
            reserved_system_time_date[6],
            reserved_system_time_date[7],
        ]),
        864_010_000,
        "system-time builder must clamp finite time-of-day above the supported range"
    );
    let reserved_magnetic_age = NMEAInterface::build_magnetic_variation(0.1, 0xFFFE);
    assert_eq!(
        u16::from_le_bytes([reserved_magnetic_age[2], reserved_magnetic_age[3]]),
        0xFFFF,
        "magnetic-variation builder must not emit reserved age bytes"
    );

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let pressures: Rc<RefCell<Vec<PressureData>>> = Rc::new(RefCell::new(Vec::new()));
    let pressure_log = pressures.clone();
    iface
        .on_pressure
        .subscribe(move |pressure| pressure_log.borrow_mut().push(*pressure));
    iface.handle_message(&Message::new(PGN_PRESSURE, pressure.to_vec(), 0x24));
    assert_eq!(pressures.borrow().len(), 1);
    assert_eq!(pressures.borrow()[0].source, PressureSource::Unavailable);

    let canonical = NMEAInterface::build_humidity(&HumidityData {
        sid: 0xFC,
        instance: 1,
        source: HumiditySource::Outside,
        actual_pct: 55.0,
        set_pct: 60.0,
    });
    assert_eq!(canonical[0], 0xFC);
}

#[test]
fn nmea2000_selected_u8_scalar_builders_encode_nonfinite_as_unavailable() {
    let position_delta =
        NMEAInterface::build_position_delta(&PositionDeltaHighPrecisionRapidUpdateData {
            sid: 3,
            time_delta_s: f64::NAN,
            latitude_delta_deg: 0.000_001,
            longitude_delta_deg: -0.000_001,
        });
    assert_eq!(
        position_delta[1], 0xFF,
        "non-finite position-delta time must not be clamped to a valid upper-edge byte"
    );

    let finite_position_delta =
        NMEAInterface::build_position_delta(&PositionDeltaHighPrecisionRapidUpdateData {
            sid: 3,
            time_delta_s: f64::MAX,
            latitude_delta_deg: 0.000_001,
            longitude_delta_deg: -0.000_001,
        });
    assert_eq!(
        finite_position_delta[1], 0xFC,
        "finite oversized position-delta time should clamp to the highest valid byte"
    );

    let unavailable_depth = NMEAInterface::build_depth(&WaterDepthData {
        sid: 9,
        depth_m: 3.21,
        offset_m: -0.25,
        range_m: f64::INFINITY,
    });
    assert_eq!(
        unavailable_depth[7], 0xFF,
        "non-finite water-depth range must encode as unavailable"
    );

    let upper_edge_depth = NMEAInterface::build_depth(&WaterDepthData {
        sid: 9,
        depth_m: 3.21,
        offset_m: -0.25,
        range_m: f64::MAX,
    });
    assert_eq!(
        upper_edge_depth[7], 0xFC,
        "finite oversized water-depth range should clamp to the highest valid byte"
    );

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let depths: Rc<RefCell<Vec<WaterDepthData>>> = Rc::new(RefCell::new(Vec::new()));
    let deltas: Rc<RefCell<Vec<PositionDeltaHighPrecisionRapidUpdateData>>> =
        Rc::new(RefCell::new(Vec::new()));
    let depth_log = depths.clone();
    iface
        .on_depth
        .subscribe(move |depth| depth_log.borrow_mut().push(*depth));
    let delta_log = deltas.clone();
    iface
        .on_position_delta
        .subscribe(move |delta| delta_log.borrow_mut().push(*delta));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));

    iface.handle_message(&Message::new(
        PGN_WATER_DEPTH,
        unavailable_depth.to_vec(),
        0x24,
    ));
    assert_eq!(depths.borrow().len(), 1);
    assert_eq!(depths.borrow()[0].range_m, 0.0);

    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_DELTA,
        position_delta.to_vec(),
        0x24,
    ));
    assert!(
        deltas.borrow().is_empty(),
        "position-delta unavailable time must not be promoted to a valid max-time delta"
    );
}

#[test]
fn nmea2000_magnetic_variation_rejects_reserved_age_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let variations: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let variation_log = variations.clone();
    iface
        .on_magnetic_variation
        .subscribe(move |variation| variation_log.borrow_mut().push(*variation));

    let valid = NMEAInterface::build_magnetic_variation(0.1, 5);
    for reserved_age in [0xFFFDu16, 0xFFFE] {
        let mut reserved = valid;
        reserved[2..4].copy_from_slice(&reserved_age.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_MAGNETIC_VARIATION,
            reserved.to_vec(),
            0x24,
        ));
    }
    assert!(
        variations.borrow().is_empty(),
        "reserved magnetic-variation age values must not emit events"
    );

    iface.handle_message(&Message::new(PGN_MAGNETIC_VARIATION, valid.to_vec(), 0x24));
    assert_eq!(variations.borrow().len(), 1);
}

#[test]
fn nmea2000_temperature_rejects_reserved_numeric_special_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let temperatures: Rc<RefCell<Vec<TemperatureData>>> = Rc::new(RefCell::new(Vec::new()));
    let temperature_log = temperatures.clone();
    iface
        .on_temperature
        .subscribe(move |temperature| temperature_log.borrow_mut().push(*temperature));

    let valid = NMEAInterface::build_temperature(&TemperatureData {
        sid: 1,
        instance: 2,
        source: TemperatureSource::Outside,
        actual_k: 293.15,
        set_k: 295.15,
    });
    for raw_actual in [0xFFFDu16, 0xFFFE] {
        let mut reserved_actual = valid;
        reserved_actual[3..5].copy_from_slice(&raw_actual.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_TEMPERATURE,
            reserved_actual.to_vec(),
            0x24,
        ));
    }
    for raw_set in [0xFFFDu16, 0xFFFE] {
        let mut reserved_set = valid;
        reserved_set[5..7].copy_from_slice(&raw_set.to_le_bytes());
        iface.handle_message(&Message::new(PGN_TEMPERATURE, reserved_set.to_vec(), 0x24));
    }
    assert!(temperatures.borrow().is_empty());

    iface.handle_message(&Message::new(PGN_TEMPERATURE, valid.to_vec(), 0x24));
    assert_eq!(temperatures.borrow().len(), 1);
    assert_eq!(temperatures.borrow()[0].source, TemperatureSource::Outside);
}

#[test]
fn nmea2000_engine_rapid_rejects_reserved_numeric_special_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let engines: Rc<RefCell<Vec<EngineData>>> = Rc::new(RefCell::new(Vec::new()));
    let engine_log = engines.clone();
    iface
        .on_engine
        .subscribe(move |engine| engine_log.borrow_mut().push(*engine));

    let valid = NMEAInterface::build_engine_params(&EngineData {
        instance: 1,
        rpm: 1_500.0,
        boost_pressure_pa: 125_000.0,
        tilt_trim: 12,
    });
    for raw_rpm in [0xFFFDu16, 0xFFFE] {
        let mut reserved_rpm = valid;
        reserved_rpm[1..3].copy_from_slice(&raw_rpm.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_ENGINE_PARAMS_RAPID,
            reserved_rpm.to_vec(),
            0x24,
        ));
    }
    for raw_boost in [0xFFFDu16, 0xFFFE] {
        let mut reserved_boost = valid;
        reserved_boost[3..5].copy_from_slice(&raw_boost.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_ENGINE_PARAMS_RAPID,
            reserved_boost.to_vec(),
            0x24,
        ));
    }
    for raw_tilt_trim in [0x7D, 0x7E, 0x7F, 0x80, 0x9B] {
        let mut reserved_tilt_trim = valid;
        reserved_tilt_trim[5] = raw_tilt_trim;
        iface.handle_message(&Message::new(
            PGN_ENGINE_PARAMS_RAPID,
            reserved_tilt_trim.to_vec(),
            0x24,
        ));
    }
    assert!(engines.borrow().is_empty());

    let high_tilt = NMEAInterface::build_engine_params(&EngineData {
        tilt_trim: i8::MAX,
        ..EngineData::default()
    });
    assert_eq!(
        high_tilt[5], 100,
        "engine tilt/trim builder must not emit positive special-value bytes"
    );

    let low_tilt = NMEAInterface::build_engine_params(&EngineData {
        tilt_trim: i8::MIN,
        ..EngineData::default()
    });
    assert_eq!(
        low_tilt[5],
        (-100i8).to_le_bytes()[0],
        "engine tilt/trim builder must clamp below the supported signed range"
    );

    iface.handle_message(&Message::new(PGN_ENGINE_PARAMS_RAPID, valid.to_vec(), 0x24));
    assert_eq!(engines.borrow().len(), 1);
    assert_eq!(engines.borrow()[0].instance, 1);
    assert_eq!(engines.borrow()[0].tilt_trim, 12);
}

#[test]
fn nmea2000_outside_environmental_rejects_reserved_numeric_special_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let environments: Rc<RefCell<Vec<OutsideEnvironmentalData>>> =
        Rc::new(RefCell::new(Vec::new()));
    let environment_log = environments.clone();
    iface
        .on_outside_environmental
        .subscribe(move |environment| environment_log.borrow_mut().push(*environment));

    let valid = NMEAInterface::build_outside_environmental(&OutsideEnvironmentalData {
        sid: 8,
        water_temperature_k: 286.15,
        outside_temperature_k: 294.15,
        atmospheric_pressure_pa: 101_300.0,
    });
    for range in [1..3, 3..5, 5..7] {
        for raw_value in [0xFFFDu16, 0xFFFE] {
            let mut reserved_value = valid;
            reserved_value[range.clone()].copy_from_slice(&raw_value.to_le_bytes());
            iface.handle_message(&Message::new(
                PGN_OUTSIDE_ENVIRONMENTAL,
                reserved_value.to_vec(),
                0x24,
            ));
        }
    }
    assert!(environments.borrow().is_empty());

    iface.handle_message(&Message::new(
        PGN_OUTSIDE_ENVIRONMENTAL,
        valid.to_vec(),
        0x24,
    ));
    assert_eq!(environments.borrow().len(), 1);
    assert_eq!(environments.borrow()[0].sid, 8);
}

#[test]
fn nmea2000_signed_numeric_fields_reject_reserved_special_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let batteries: Rc<RefCell<Vec<BatteryStatusData>>> = Rc::new(RefCell::new(Vec::new()));
    let pressures: Rc<RefCell<Vec<PressureData>>> = Rc::new(RefCell::new(Vec::new()));
    let xtes: Rc<RefCell<Vec<XTEData>>> = Rc::new(RefCell::new(Vec::new()));
    let battery_log = batteries.clone();
    let pressure_log = pressures.clone();
    let xte_log = xtes.clone();
    iface
        .on_battery
        .subscribe(move |battery| battery_log.borrow_mut().push(*battery));
    iface
        .on_pressure
        .subscribe(move |pressure| pressure_log.borrow_mut().push(*pressure));
    iface
        .on_xte
        .subscribe(move |xte| xte_log.borrow_mut().push(*xte));

    let reserved_i16 = i16::MAX - 2;
    let battery = NMEAInterface::build_battery_status(&BatteryStatusData {
        instance: 2,
        voltage: 12.4,
        current_a: -3.2,
        ..Default::default()
    });
    let mut reserved_battery_current = battery;
    reserved_battery_current[3..5].copy_from_slice(&(reserved_i16 as u16).to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_BATTERY_STATUS,
        reserved_battery_current.to_vec(),
        0x24,
    ));
    assert!(batteries.borrow().is_empty());

    let pressure = NMEAInterface::build_pressure(&PressureData {
        sid: 4,
        instance: 1,
        source: PressureSource::Atmospheric,
        pressure_pa: 101_300.0,
    });
    let mut reserved_pressure = pressure;
    reserved_pressure[3..7].copy_from_slice(&((i32::MAX - 2) as u32).to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_PRESSURE,
        reserved_pressure.to_vec(),
        0x24,
    ));
    assert!(pressures.borrow().is_empty());

    let xte = NMEAInterface::build_xte(&XTEData {
        sid: 5,
        mode: XTEMode::Autonomous,
        navigation_terminated: false,
        xte_m: -1.25,
    });
    let mut reserved_xte = xte;
    reserved_xte[2..6].copy_from_slice(&((i32::MAX - 2) as u32).to_le_bytes());
    iface.handle_message(&Message::new(PGN_XTE, reserved_xte.to_vec(), 0x24));
    assert!(xtes.borrow().is_empty());

    iface.handle_message(&Message::new(PGN_BATTERY_STATUS, battery.to_vec(), 0x24));
    iface.handle_message(&Message::new(PGN_PRESSURE, pressure.to_vec(), 0x24));
    iface.handle_message(&Message::new(PGN_XTE, xte.to_vec(), 0x24));
    assert_eq!(batteries.borrow().len(), 1);
    assert_eq!(pressures.borrow().len(), 1);
    assert_eq!(xtes.borrow().len(), 1);
}

#[test]
fn nmea2000_battery_status_voltage_is_unsigned_and_current_remains_signed() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let batteries: Rc<RefCell<Vec<BatteryStatusData>>> = Rc::new(RefCell::new(Vec::new()));
    let battery_log = batteries.clone();
    iface
        .on_battery
        .subscribe(move |battery| battery_log.borrow_mut().push(*battery));

    let valid = NMEAInterface::build_battery_status(&BatteryStatusData {
        instance: 2,
        voltage: 12.4,
        current_a: -3.2,
        ..Default::default()
    });

    for reserved_voltage in [0xFFFDu16, 0xFFFE] {
        let mut reserved = valid;
        reserved[1..3].copy_from_slice(&reserved_voltage.to_le_bytes());
        iface.handle_message(&Message::new(PGN_BATTERY_STATUS, reserved.to_vec(), 0x24));
    }
    assert!(
        batteries.borrow().is_empty(),
        "reserved unsigned voltage raw values must not emit battery events"
    );

    let mut unavailable_voltage = valid;
    unavailable_voltage[1..3].copy_from_slice(&0xFFFFu16.to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_BATTERY_STATUS,
        unavailable_voltage.to_vec(),
        0x24,
    ));
    assert_eq!(batteries.borrow().len(), 1);
    assert_eq!(batteries.borrow()[0].voltage, 0.0);
    assert!((batteries.borrow()[0].current_a + 3.2).abs() < 0.1);

    iface.handle_message(&Message::new(PGN_BATTERY_STATUS, valid.to_vec(), 0x24));
    assert_eq!(batteries.borrow().len(), 2);
    assert!((batteries.borrow()[1].voltage - 12.4).abs() < 0.01);
    assert!((batteries.borrow()[1].current_a + 3.2).abs() < 0.1);

    let negative_voltage = NMEAInterface::build_battery_status(&BatteryStatusData {
        instance: 3,
        voltage: -1.23,
        current_a: 0.0,
        ..Default::default()
    });
    assert_eq!(
        &negative_voltage[1..3],
        &0u16.to_le_bytes(),
        "builder must clamp negative unsigned voltage to zero instead of wrapping"
    );
    iface.handle_message(&Message::new(
        PGN_BATTERY_STATUS,
        negative_voltage.to_vec(),
        0x24,
    ));
    assert_eq!(batteries.borrow().len(), 3);
    assert_eq!(batteries.borrow()[2].voltage, 0.0);
    assert_eq!(batteries.borrow()[2].current_a, 0.0);
}

#[test]
fn nmea2000_pressure_rejects_reserved_source_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let pressures: Rc<RefCell<Vec<PressureData>>> = Rc::new(RefCell::new(Vec::new()));
    let pressure_log = pressures.clone();
    iface
        .on_pressure
        .subscribe(move |pressure| pressure_log.borrow_mut().push(*pressure));

    let valid = NMEAInterface::build_pressure(&PressureData {
        sid: 4,
        instance: 1,
        source: PressureSource::Atmospheric,
        pressure_pa: 101_300.0,
    });
    let mut reserved_source = valid;
    reserved_source[2] = PressureSource::Reserved.as_u8();
    iface.handle_message(&Message::new(PGN_PRESSURE, reserved_source.to_vec(), 0x24));
    assert!(
        pressures.borrow().is_empty(),
        "reserved pressure source must not emit an event"
    );

    iface.handle_message(&Message::new(PGN_PRESSURE, valid.to_vec(), 0x24));
    assert_eq!(pressures.borrow().len(), 1);
    assert_eq!(pressures.borrow()[0].source, PressureSource::Atmospheric);
}

#[test]
fn nmea2000_xte_mode_byte_rejects_reserved_bits_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let xtes: Rc<RefCell<Vec<XTEData>>> = Rc::new(RefCell::new(Vec::new()));
    let xte_log = xtes.clone();
    iface
        .on_xte
        .subscribe(move |xte| xte_log.borrow_mut().push(*xte));

    let valid = NMEAInterface::build_xte(&XTEData {
        sid: 5,
        mode: XTEMode::Manual,
        navigation_terminated: true,
        xte_m: -1.25,
    });
    assert_eq!(valid[1] & 0x40, 0x40);
    assert_eq!(valid[1] & 0x0F, XTEMode::Manual.as_u8());

    for reserved in [0x10, 0x20, 0x80, 0xB0] {
        let mut reserved_mode_byte = valid;
        reserved_mode_byte[1] |= reserved;
        iface.handle_message(&Message::new(PGN_XTE, reserved_mode_byte.to_vec(), 0x24));
    }
    assert!(
        xtes.borrow().is_empty(),
        "reserved XTE mode-byte bits must not emit an event"
    );

    let mut reserved_mode = valid;
    reserved_mode[1] = 0x40 | 0x05;
    iface.handle_message(&Message::new(PGN_XTE, reserved_mode.to_vec(), 0x24));
    assert!(
        xtes.borrow().is_empty(),
        "reserved XTE mode values must not emit an event"
    );

    iface.handle_message(&Message::new(PGN_XTE, valid.to_vec(), 0x24));
    assert_eq!(xtes.borrow().len(), 1);
    assert_eq!(xtes.borrow()[0].mode, XTEMode::Manual);
    assert!(xtes.borrow()[0].navigation_terminated);
}

#[test]
fn nmea2000_selected_pgns_ignore_invalid_sources_before_events_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let events = Rc::new(RefCell::new(0usize));

    macro_rules! count_event {
        ($event:expr) => {{
            let log = events.clone();
            $event.subscribe(move |_| *log.borrow_mut() += 1);
        }};
    }

    count_event!(iface.on_position);
    count_event!(iface.on_cog);
    count_event!(iface.on_sog);
    count_event!(iface.on_position_delta);
    count_event!(iface.on_attitude);
    count_event!(iface.on_wind);
    count_event!(iface.on_temperature);
    count_event!(iface.on_engine);
    count_event!(iface.on_depth);
    count_event!(iface.on_heading);
    count_event!(iface.on_system_time);
    count_event!(iface.on_gnss_dops);
    count_event!(iface.on_magnetic_variation);
    count_event!(iface.on_rudder);
    count_event!(iface.on_fluid_level);
    count_event!(iface.on_battery);
    count_event!(iface.on_speed_water);
    count_event!(iface.on_xte);
    count_event!(iface.on_humidity);
    count_event!(iface.on_pressure);
    count_event!(iface.on_outside_environmental);

    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        for pgn in [
            PGN_GNSS_POSITION_RAPID,
            PGN_GNSS_COG_SOG_RAPID,
            PGN_GNSS_POSITION_DELTA,
            PGN_ATTITUDE,
            PGN_RATE_OF_TURN,
            PGN_GNSS_POSITION_DATA,
            PGN_GNSS_DOPS,
            PGN_MAGNETIC_VARIATION,
            PGN_WIND_DATA,
            PGN_TEMPERATURE,
            PGN_HUMIDITY,
            PGN_PRESSURE,
            PGN_OUTSIDE_ENVIRONMENTAL,
            PGN_ENGINE_PARAMS_RAPID,
            PGN_FLUID_LEVEL,
            PGN_BATTERY_STATUS,
            PGN_WATER_DEPTH,
            PGN_SPEED_WATER,
            PGN_XTE,
            PGN_RUDDER,
            PGN_SYSTEM_TIME,
            PGN_HEADING_TRACK,
        ] {
            iface.handle_message(&Message::new(pgn, vec![0xAA, 0x55, 0x00], source));
        }
    }

    assert_eq!(
        *events.borrow(),
        0,
        "invalid NMEA 2000 sources must be rejected before dispatching selected PGNs"
    );
    assert!(
        iface.latest_position().is_none(),
        "invalid sources must not seed the GNSS cache"
    );

    let valid_position = NMEAInterface::build_position(&Default::default());
    iface.handle_message(&Message::with_addressing(
        PGN_GNSS_POSITION_RAPID,
        valid_position.to_vec(),
        0x24,
        0x42,
        Priority::Default,
    ));

    assert_eq!(
        *events.borrow(),
        0,
        "PDU2 destination-specific selected NMEA 2000 metadata must be rejected before dispatch"
    );
    assert!(
        iface.latest_position().is_none(),
        "destination-specific metadata on PDU2 selected PGNs must not seed the GNSS cache"
    );

    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        valid_position.to_vec(),
        0x24,
    ));
    assert_eq!(
        *events.borrow(),
        1,
        "a valid broadcast-style selected NMEA 2000 envelope must still dispatch"
    );
    assert!(iface.latest_position().is_some());
}

#[test]
fn nmea2000_selected_pgn_config_gates_ignore_disabled_groups_without_cache_or_events() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(false));
    let events = Rc::new(RefCell::new(0usize));

    macro_rules! count_event {
        ($event:expr) => {{
            let log = events.clone();
            $event.subscribe(move |_| *log.borrow_mut() += 1);
        }};
    }

    count_event!(iface.on_position);
    count_event!(iface.on_cog);
    count_event!(iface.on_sog);
    count_event!(iface.on_position_delta);
    count_event!(iface.on_attitude);
    count_event!(iface.on_wind);
    count_event!(iface.on_temperature);
    count_event!(iface.on_engine);
    count_event!(iface.on_depth);
    count_event!(iface.on_heading);
    count_event!(iface.on_system_time);
    count_event!(iface.on_gnss_dops);
    count_event!(iface.on_magnetic_variation);
    count_event!(iface.on_rudder);
    count_event!(iface.on_fluid_level);
    count_event!(iface.on_battery);
    count_event!(iface.on_speed_water);
    count_event!(iface.on_xte);
    count_event!(iface.on_humidity);
    count_event!(iface.on_pressure);
    count_event!(iface.on_outside_environmental);

    let valid_selected_frames: [(u32, Vec<u8>); 22] = [
        (
            PGN_GNSS_POSITION_RAPID,
            NMEAInterface::build_position(&Default::default()).to_vec(),
        ),
        (
            PGN_GNSS_COG_SOG_RAPID,
            NMEAInterface::build_cog_sog(1.0, 2.0).to_vec(),
        ),
        (
            PGN_GNSS_POSITION_DELTA,
            NMEAInterface::build_position_delta(&PositionDeltaHighPrecisionRapidUpdateData {
                sid: 1,
                time_delta_s: 0.25,
                latitude_delta_deg: 0.000_001,
                longitude_delta_deg: -0.000_001,
            })
            .to_vec(),
        ),
        (PGN_ATTITUDE, {
            let mut data = [0xFFu8; 8];
            data[1..3].copy_from_slice(&10i16.to_le_bytes());
            data[3..5].copy_from_slice(&20i16.to_le_bytes());
            data[5..7].copy_from_slice(&30i16.to_le_bytes());
            data.to_vec()
        }),
        (PGN_RATE_OF_TURN, {
            let mut data = [0xFFu8; 8];
            data[1..5].copy_from_slice(&100i32.to_le_bytes());
            data.to_vec()
        }),
        (PGN_GNSS_POSITION_DATA, standard_position_detail_frame()),
        (PGN_GNSS_DOPS, {
            let mut data = [0xFFu8; 8];
            data[1] = 0x1B;
            data[2..4].copy_from_slice(&100u16.to_le_bytes());
            data[4..6].copy_from_slice(&150u16.to_le_bytes());
            data[6..8].copy_from_slice(&200u16.to_le_bytes());
            data.to_vec()
        }),
        (
            PGN_MAGNETIC_VARIATION,
            NMEAInterface::build_magnetic_variation(0.1, 5).to_vec(),
        ),
        (
            PGN_WIND_DATA,
            NMEAInterface::build_wind(&WindData {
                sid: 2,
                speed_mps: 4.0,
                direction_rad: 1.0,
                reference: WindReference::Apparent,
            })
            .to_vec(),
        ),
        (
            PGN_TEMPERATURE,
            NMEAInterface::build_temperature(&TemperatureData {
                sid: 3,
                instance: 1,
                source: TemperatureSource::Outside,
                actual_k: 293.15,
                set_k: 294.15,
            })
            .to_vec(),
        ),
        (
            PGN_HUMIDITY,
            NMEAInterface::build_humidity(&HumidityData {
                sid: 4,
                instance: 1,
                source: HumiditySource::Outside,
                actual_pct: 50.0,
                set_pct: 55.0,
            })
            .to_vec(),
        ),
        (
            PGN_PRESSURE,
            NMEAInterface::build_pressure(&PressureData {
                sid: 5,
                instance: 1,
                source: PressureSource::Atmospheric,
                pressure_pa: 101_325.0,
            })
            .to_vec(),
        ),
        (
            PGN_OUTSIDE_ENVIRONMENTAL,
            NMEAInterface::build_outside_environmental(&OutsideEnvironmentalData {
                sid: 6,
                water_temperature_k: 290.0,
                outside_temperature_k: 291.0,
                atmospheric_pressure_pa: 101_000.0,
            })
            .to_vec(),
        ),
        (
            PGN_ENGINE_PARAMS_RAPID,
            NMEAInterface::build_engine_params(&EngineData {
                instance: 1,
                rpm: 1200.0,
                boost_pressure_pa: 5000.0,
                tilt_trim: 10,
            })
            .to_vec(),
        ),
        (
            PGN_FLUID_LEVEL,
            NMEAInterface::build_fluid_level(&FluidLevelData {
                instance: 1,
                r#type: FluidType::Fuel,
                level_pct: 25.0,
                capacity_l: 100.0,
            })
            .to_vec(),
        ),
        (
            PGN_BATTERY_STATUS,
            NMEAInterface::build_battery_status(&BatteryStatusData {
                instance: 1,
                voltage: 12.5,
                current_a: 3.0,
                ..Default::default()
            })
            .to_vec(),
        ),
        (
            PGN_WATER_DEPTH,
            NMEAInterface::build_depth(&WaterDepthData {
                sid: 7,
                depth_m: 3.5,
                offset_m: -0.2,
                range_m: 10.0,
            })
            .to_vec(),
        ),
        (
            PGN_SPEED_WATER,
            NMEAInterface::build_speed_water(&SpeedWaterData {
                sid: 8,
                water_speed_mps: 2.2,
                ground_speed_mps: 2.5,
                reference: SpeedWaterRefType::PaddleWheel,
            })
            .to_vec(),
        ),
        (
            PGN_XTE,
            NMEAInterface::build_xte(&XTEData {
                sid: 9,
                mode: XTEMode::Manual,
                navigation_terminated: false,
                xte_m: -1.0,
            })
            .to_vec(),
        ),
        (
            PGN_RUDDER,
            NMEAInterface::build_rudder(&RudderData {
                instance: 1,
                direction: RudderDirection::Port,
                angle_order_rad: -0.2,
                position_rad: 0.1,
            })
            .to_vec(),
        ),
        (
            PGN_SYSTEM_TIME,
            NMEAInterface::build_system_time(&SystemTimeData {
                sid: 10,
                source: TimeSource::GPS,
                days_since_epoch: 20_000,
                seconds_since_midnight: 12_345.0,
            })
            .to_vec(),
        ),
        (
            PGN_HEADING_TRACK,
            NMEAInterface::build_heading(1.0, 0.1, -0.1).to_vec(),
        ),
    ];

    for (pgn, data) in valid_selected_frames {
        iface.handle_message(&Message::new(pgn, data, 0x24));
    }

    assert_eq!(
        *events.borrow(),
        0,
        "disabled selected-PGN listen gates must ignore otherwise valid frames"
    );
    assert!(
        iface.latest_position().is_none(),
        "disabled GNSS gates must not seed or mutate the selected-PGN position cache"
    );
}

#[test]
fn nmea2000_fast_packet_accepts_selected_boundary_payloads_and_rejects_wrong_size_class() {
    let pgn = PGN_PRODUCT_INFO;
    let source = 0x23;

    let mut tx = FastPacketProtocol::new();
    assert_eq!(
        tx.send(pgn, &[0xAA; 8], source).unwrap_err().code,
        ErrorCode::InvalidState,
        "8-byte payloads must stay classic CAN, not Fast Packet"
    );

    let nine = [0x11; 9];
    let nine_frames = tx.send(pgn, &nine, source).unwrap();
    assert_eq!(nine_frames.len(), 2);
    assert_eq!(nine_frames[0].data[1], 9);
    assert_eq!(nine_frames[1].data[0] & 0x1F, 1);
    assert_eq!(&nine_frames[1].data[1..4], &[0x11; 3]);
    assert_eq!(&nine_frames[1].data[4..], &[0xFF; 4]);

    let max_payload: Vec<u8> = (0..FAST_PACKET_MAX_DATA)
        .map(|value| (value & 0xFF) as u8)
        .collect();
    let max_frames = tx.send(pgn, &max_payload, source).unwrap();
    assert_eq!(max_frames.len(), 32);
    assert_eq!(max_frames[0].data[1], FAST_PACKET_MAX_DATA as u8);
    assert_eq!(max_frames.last().unwrap().data[0] & 0x1F, 31);

    let mut rx = FastPacketProtocol::new();
    let mut completed = None;
    for frame in &max_frames {
        completed = rx.process_frame(frame).or(completed);
    }
    let completed = completed.expect("maximum selected Fast Packet payload reassembles");
    assert_eq!(completed.pgn, pgn);
    assert_eq!(completed.source, source);
    assert_eq!(completed.data, max_payload);

    assert_eq!(
        tx.send(pgn, &[0x55; FAST_PACKET_MAX_DATA as usize + 1], source)
            .unwrap_err()
            .code,
        ErrorCode::BufferOverflow
    );
}

#[test]
fn nmea2000_management_heartbeat_sequence_wrap_and_invalid_interval_are_state_safe() {
    let mut manager =
        N2KManagement::new(N2KManagementConfig::default().with_heartbeat_interval(1000));

    for expected in (0u8..16).chain(0u8..2) {
        let out = manager.send_heartbeat().unwrap();
        assert_eq!(out.pgn, PGN_HEARTBEAT_N2K);
        assert_eq!(out.dest, None);
        let heartbeat = N2KHeartbeat::decode(&out.data).unwrap();
        assert_eq!(
            heartbeat.sequence_counter, expected,
            "NMEA 2000 heartbeat sequence counter must wrap in the low four bits"
        );
    }
    assert_eq!(manager.heartbeat_sequence(), 2);

    let mut invalid =
        N2KManagement::new(N2KManagementConfig::default().with_heartbeat_interval(51));
    assert_eq!(invalid.heartbeat_sequence(), 0);
    assert_eq!(
        invalid.send_heartbeat().unwrap_err().code,
        ErrorCode::InvalidData
    );
    assert_eq!(
        invalid.heartbeat_sequence(),
        0,
        "unencodable heartbeat intervals must not advance the local sequence counter"
    );
    assert_eq!(invalid.update(51).unwrap_err().code, ErrorCode::InvalidData);
    assert_eq!(
        invalid.heartbeat_sequence(),
        0,
        "failed periodic heartbeat generation must also leave the sequence counter unchanged"
    );
}

#[test]
fn nmea2000_fast_packet_transmit_sequence_counter_wraps_three_bit_field() {
    let pgn = PGN_PRODUCT_INFO;
    let source = 0x23;
    let payload = [0x66; 9];
    let mut tx = FastPacketProtocol::new();

    for transfer_index in 0..10u8 {
        let frames = tx.send(pgn, &payload, source).unwrap();
        assert_eq!(frames.len(), 2);

        let expected_sequence_bits = (transfer_index & 0x07) << 5;
        assert_eq!(frames[0].data[0], expected_sequence_bits);
        assert_eq!(frames[1].data[0], expected_sequence_bits | 0x01);
        assert_eq!(frames[0].data[1], payload.len() as u8);
    }
}

#[test]
fn nmea2000_fast_packet_reassembles_interleaved_sources_and_sequences_independently() {
    let pgn = PGN_PRODUCT_INFO;
    let first_payload = [0x11; 13];
    let second_payload = [0x22; 13];
    let peer_payload = [0x33; 13];

    let mut same_source_tx = FastPacketProtocol::new();
    let first_same_source = same_source_tx.send(pgn, &first_payload, 0x23).unwrap();
    let second_same_source = same_source_tx.send(pgn, &second_payload, 0x23).unwrap();
    let mut peer_tx = FastPacketProtocol::new();
    let peer_frames = peer_tx.send(pgn, &peer_payload, 0x24).unwrap();

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&first_same_source[0]).is_none());
    assert!(rx.process_frame(&second_same_source[0]).is_none());
    assert!(rx.process_frame(&peer_frames[0]).is_none());
    assert_eq!(rx.rx_session_count(), 3);

    let first = rx
        .process_frame(&first_same_source[1])
        .expect("first same-source sequence should complete independently");
    assert_eq!(first.source, 0x23);
    assert_eq!(first.data, first_payload);
    assert_eq!(rx.rx_session_count(), 2);

    let peer = rx
        .process_frame(&peer_frames[1])
        .expect("peer source sequence should complete independently");
    assert_eq!(peer.source, 0x24);
    assert_eq!(peer.data, peer_payload);
    assert_eq!(rx.rx_session_count(), 1);

    let second = rx
        .process_frame(&second_same_source[1])
        .expect("second same-source sequence should complete independently");
    assert_eq!(second.source, 0x23);
    assert_eq!(second.data, second_payload);
    assert_eq!(rx.rx_session_count(), 0);
    assert_eq!(rx.stats().dropped_frames, 0);
    assert_eq!(rx.stats().dropped_sessions, 0);
}

#[test]
fn nmea2000_fast_packet_rejects_malformed_receive_sequences_without_partial_delivery() {
    let pgn = PGN_PRODUCT_INFO;
    let source = 0x23;
    let id = Identifier::encode(Priority::Default, pgn, source, BROADCAST_ADDRESS);

    for total_bytes in [8, FAST_PACKET_MAX_DATA as u8 + 1] {
        let malformed_first = Frame::new(
            id,
            [0x00, total_bytes, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF],
            8,
        );
        let mut rx = FastPacketProtocol::new();
        assert!(
            rx.process_frame(&malformed_first).is_none(),
            "invalid first-frame size must not produce a message"
        );
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    let mut tx = FastPacketProtocol::new();
    let payload: Vec<u8> = (0..20).collect();
    let frames = tx.send(pgn, &payload, source).unwrap();
    assert_eq!(frames.len(), 3);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&frames[0]).is_none());
    assert_eq!(rx.rx_session_count(), 1);
    assert!(
        rx.process_frame(&frames[2]).is_none(),
        "skipping the expected frame counter must discard the in-flight session"
    );
    assert_eq!(rx.rx_session_count(), 0);
    assert_eq!(rx.stats().dropped_frames, 1);
    assert_eq!(rx.stats().dropped_sessions, 1);

    assert!(
        rx.process_frame(&frames[1]).is_none(),
        "late frame from the discarded session must not be delivered"
    );
    assert_eq!(rx.rx_session_count(), 0);
    assert_eq!(rx.stats().dropped_frames, 2);
}

