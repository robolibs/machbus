#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
    use alloc::vec;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn nmea_msg(pgn: u32, data: Vec<u8>) -> Message {
        Message::new(pgn, data, 0x10)
    }

    #[test]
    fn position_rapid_round_trip() {
        let pos = GNSSPosition {
            wgs: Wgs::new(52.0, 5.0, 0.0),
            ..Default::default()
        };
        let bytes = NMEAInterface::build_position(&pos);
        let mut iface = NMEAInterface::default();
        iface.handle_message(&nmea_msg(PGN_GNSS_POSITION_RAPID, bytes.to_vec()));
        let p = iface.latest_position().unwrap();
        assert!((p.wgs.latitude - 52.0).abs() < 1e-6);
        assert!((p.wgs.longitude - 5.0).abs() < 1e-6);
    }

    #[test]
    fn inbound_messages_reject_invalid_source_addresses_before_events_or_cache() {
        for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let pos = GNSSPosition {
                wgs: Wgs::new(52.0, 5.0, 0.0),
                ..Default::default()
            };
            let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
            let emitted: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
            let seen = emitted.clone();
            iface
                .on_position
                .subscribe(move |_| *seen.borrow_mut() += 1);

            let mut msg = nmea_msg(
                PGN_GNSS_POSITION_RAPID,
                NMEAInterface::build_position(&pos).to_vec(),
            );
            msg.source = bad_source;
            iface.handle_message(&msg);

            assert!(
                iface.latest_position().is_none(),
                "NMEA source 0x{bad_source:02X} must not update the position cache"
            );
            assert_eq!(
                *emitted.borrow(),
                0,
                "NMEA source 0x{bad_source:02X} must not emit a position event"
            );
        }
    }

    #[test]
    fn position_detail_unavailable_coordinates_do_not_overwrite_cache() {
        let pos = GNSSPosition {
            wgs: Wgs::new(52.0, 5.0, 0.0),
            ..Default::default()
        };
        let mut iface = NMEAInterface::default();
        iface.handle_message(&nmea_msg(
            PGN_GNSS_POSITION_RAPID,
            NMEAInterface::build_position(&pos).to_vec(),
        ));

        let emitted: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
        let seen = emitted.clone();
        iface
            .on_position
            .subscribe(move |_| *seen.borrow_mut() += 1);

        let mut detail = vec![0xFFu8; 43];
        detail[7..15].copy_from_slice(&i64::MAX.to_le_bytes());
        detail[15..23].copy_from_slice(&i64::MAX.to_le_bytes());
        detail[23..31].copy_from_slice(&i64::MAX.to_le_bytes());
        detail[33] = 0xFF;
        iface.handle_message(&nmea_msg(PGN_GNSS_POSITION_DATA, detail));

        let cached = iface.latest_position().unwrap();
        assert!((cached.wgs.latitude - 52.0).abs() < 1e-6);
        assert!((cached.wgs.longitude - 5.0).abs() < 1e-6);
        assert_eq!(*emitted.borrow(), 0);
    }

    #[test]
    fn position_detail_rejects_reference_station_count_mismatches() {
        let mut iface = NMEAInterface::default();
        let emitted: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = emitted.clone();
        iface
            .on_position
            .subscribe(move |pos| seen.borrow_mut().push(*pos));

        let mut detail = vec![0xFFu8; 43];
        let lat_raw = (52.0_f64 * 1e16) as i64;
        let lon_raw = (5.0_f64 * 1e16) as i64;
        detail[7..15].copy_from_slice(&lat_raw.to_le_bytes());
        detail[15..23].copy_from_slice(&lon_raw.to_le_bytes());
        detail[23..31].copy_from_slice(&i64::MAX.to_le_bytes());
        detail[31] = (GNSSFixType::GNSSFix.as_u8() << 4) | GNSSSystem::GPS.as_u8();

        let mut zero_refs_with_tail = detail.clone();
        zero_refs_with_tail[42] = 0;
        zero_refs_with_tail.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        iface.handle_message(&nmea_msg(PGN_GNSS_POSITION_DATA, zero_refs_with_tail));
        assert!(emitted.borrow().is_empty());

        let mut one_ref_truncated = detail.clone();
        one_ref_truncated[42] = 1;
        iface.handle_message(&nmea_msg(PGN_GNSS_POSITION_DATA, one_ref_truncated));
        assert!(emitted.borrow().is_empty());

        let mut one_ref_complete = detail;
        one_ref_complete[42] = 1;
        one_ref_complete.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        iface.handle_message(&nmea_msg(PGN_GNSS_POSITION_DATA, one_ref_complete));
        assert_eq!(emitted.borrow().len(), 1);
        assert!((emitted.borrow()[0].wgs.latitude - 52.0).abs() < 1e-12);
        assert!((emitted.borrow()[0].wgs.longitude - 5.0).abs() < 1e-12);
    }

    #[test]
    fn system_time_rejects_unknown_source() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        let emitted: Rc<RefCell<Vec<SystemTimeData>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = emitted.clone();
        iface
            .on_system_time
            .subscribe(move |time| seen.borrow_mut().push(*time));

        let valid = NMEAInterface::build_system_time(&SystemTimeData {
            sid: 0x2A,
            source: TimeSource::GLONASS,
            days_since_epoch: 12_345,
            seconds_since_midnight: 3_600.5,
        });
        iface.handle_message(&nmea_msg(PGN_SYSTEM_TIME, valid.to_vec()));
        assert_eq!(emitted.borrow().len(), 1);
        assert_eq!(emitted.borrow()[0].source, TimeSource::GLONASS);

        let mut bad = valid;
        bad[1] = 0x06;
        iface.handle_message(&nmea_msg(PGN_SYSTEM_TIME, bad.to_vec()));
        assert_eq!(emitted.borrow().len(), 1);
    }

    #[test]
    fn local_time_offset_decodes_canboat_layout() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        let emitted: Rc<RefCell<Vec<LocalTimeOffsetData>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = emitted.clone();
        iface
            .on_local_time_offset
            .subscribe(move |v| seen.borrow_mut().push(*v));

        // date=12345 days, time=3600.5 s (×10000 = 36_005_000), offset=+120 min.
        let mut data = [0u8; 8];
        data[0..2].copy_from_slice(&12_345u16.to_le_bytes());
        data[2..6].copy_from_slice(&36_005_000u32.to_le_bytes());
        data[6..8].copy_from_slice(&120i16.to_le_bytes());
        iface.handle_message(&nmea_msg(PGN_LOCAL_TIME_OFFSET, data.to_vec()));

        assert_eq!(emitted.borrow().len(), 1);
        let v = emitted.borrow()[0];
        assert_eq!(v.days_since_epoch, 12_345);
        assert!((v.seconds_since_midnight - 3600.5).abs() < 1e-6);
        assert_eq!(v.local_offset_minutes, 120);
    }

    #[test]
    fn navigation_data_decodes_canboat_layout() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        let emitted: Rc<RefCell<Vec<NavigationData>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = emitted.clone();
        iface
            .on_navigation_data
            .subscribe(move |v| seen.borrow_mut().push(*v));

        let mut data = vec![0x2A]; // sid
        data.extend_from_slice(&123_456u32.to_le_bytes()); // distance 1234.56 m
        data.push(0x00); // flags: bearing-ref True, calc GreatCircle
        data.extend_from_slice(&0u32.to_le_bytes()); // eta time
        data.extend_from_slice(&0u16.to_le_bytes()); // eta date
        data.extend_from_slice(&10000u16.to_le_bytes()); // bearing orig→dest 1.0 rad
        data.extend_from_slice(&5000u16.to_le_bytes()); // bearing pos→dest 0.5 rad
        data.extend_from_slice(&1u32.to_le_bytes()); // origin wp
        data.extend_from_slice(&2u32.to_le_bytes()); // dest wp
        data.extend_from_slice(&520_000_000i32.to_le_bytes()); // dest lat 52.0°
        data.extend_from_slice(&50_000_000i32.to_le_bytes()); // dest lon 5.0°
        data.extend_from_slice(&250i16.to_le_bytes()); // closing vel 2.5 m/s
        iface.handle_message(&nmea_msg(PGN_NAVIGATION_DATA, data));

        assert_eq!(emitted.borrow().len(), 1);
        let v = emitted.borrow()[0];
        assert!((v.distance_to_wp_m - 1234.56).abs() < 1e-3);
        assert!((v.bearing_origin_to_dest_rad - 1.0).abs() < 1e-4);
        assert_eq!(v.dest_wp_number, 2);
        assert!((v.dest_latitude - 52.0).abs() < 1e-6);
        assert!((v.dest_longitude - 5.0).abs() < 1e-6);
        assert!((v.wp_closing_velocity_mps - 2.5).abs() < 1e-6);
    }

    #[test]
    fn gnss_sats_in_view_decodes_canboat_repeating_set() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        let emitted: Rc<RefCell<Vec<GnssSatsInViewData>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = emitted.clone();
        iface
            .on_gnss_sats_in_view
            .subscribe(move |v| seen.borrow_mut().push(v.clone()));

        // Header: sid=0x2A, mode byte, satsInView=2.
        let mut data = vec![0x2A, 0x00, 2];
        // Per-satellite 12-byte records.
        let sat = |prn: u8, elev: i16, azim: u16, snr: i16| {
            let mut s = vec![prn];
            s.extend_from_slice(&elev.to_le_bytes());
            s.extend_from_slice(&azim.to_le_bytes());
            s.extend_from_slice(&snr.to_le_bytes());
            s.extend_from_slice(&0i32.to_le_bytes()); // range residuals
            s.push(0x02); // status
            s
        };
        data.extend(sat(5, 5000, 12000, 4500)); // 0.5 rad, 1.2 rad, 45.0 dB
        data.extend(sat(12, 1000, 30000, 3000));
        iface.handle_message(&nmea_msg(PGN_GNSS_SATELLITES_IN_VIEW, data));

        assert_eq!(emitted.borrow().len(), 1);
        let v = &emitted.borrow()[0];
        assert_eq!(v.sid, 0x2A);
        assert_eq!(v.sats_in_view, 2);
        assert_eq!(v.satellites.len(), 2);
        assert_eq!(v.satellites[0].prn, 5);
        assert!((v.satellites[0].elevation_rad - 0.5).abs() < 1e-4);
        assert!((v.satellites[0].snr_db - 45.0).abs() < 1e-6);
        assert_eq!(v.satellites[1].prn, 12);
    }

    #[test]
    fn cog_sog_updates_position_cache() {
        let mut iface = NMEAInterface::default();
        // First seed a position.
        let pos = GNSSPosition {
            wgs: Wgs::new(52.0, 5.0, 0.0),
            ..Default::default()
        };
        iface.handle_message(&nmea_msg(
            PGN_GNSS_POSITION_RAPID,
            NMEAInterface::build_position(&pos).to_vec(),
        ));
        // ~90°, 5.5 m/s.
        let bytes = NMEAInterface::build_cog_sog(std::f64::consts::FRAC_PI_2, 5.5);
        iface.handle_message(&nmea_msg(PGN_GNSS_COG_SOG_RAPID, bytes.to_vec()));
        let p = iface.latest_position().unwrap();
        assert!((p.cog_rad.unwrap() - std::f64::consts::FRAC_PI_2).abs() < 0.001);
        assert!((p.speed_mps.unwrap() - 5.5).abs() < 0.01);
    }

    #[test]
    fn wind_decode_emits_event() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        let log: Rc<RefCell<Vec<WindData>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        iface.on_wind.subscribe(move |w| lc.borrow_mut().push(*w));
        let wind = WindData {
            sid: 7,
            speed_mps: 12.5,
            direction_rad: 1.0,
            reference: WindReference::TrueNorth,
        };
        let bytes = NMEAInterface::build_wind(&wind);
        iface.handle_message(&nmea_msg(PGN_WIND_DATA, bytes.to_vec()));
        assert_eq!(log.borrow().len(), 1);
        assert!((log.borrow()[0].speed_mps - 12.5).abs() < 0.01);
        assert_eq!(log.borrow()[0].reference, WindReference::TrueNorth);
    }

    #[test]
    fn temperature_round_trip() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        let log: Rc<RefCell<Vec<TemperatureData>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        iface
            .on_temperature
            .subscribe(move |t| lc.borrow_mut().push(*t));
        let temp = TemperatureData {
            sid: 1,
            instance: 0,
            source: TemperatureSource::Outside,
            actual_k: 293.15,
            set_k: 0.0,
        };
        let bytes = NMEAInterface::build_temperature(&temp);
        iface.handle_message(&nmea_msg(PGN_TEMPERATURE, bytes.to_vec()));
        assert!((log.borrow()[0].actual_k - 293.15).abs() < 0.05);
        assert_eq!(log.borrow()[0].source, TemperatureSource::Outside);
    }

    #[test]
    fn config_default_listens_to_position_only_categories() {
        let cfg = NMEAConfig::default();
        assert!(cfg.listen_rapid_position);
        assert!(cfg.listen_cog_sog);
        assert!(!cfg.listen_wind);
        assert!(!cfg.listen_temperature);
    }

    #[test]
    fn config_gnss_navigation_profile_covers_stack_events_without_other_groups() {
        let cfg = NMEAConfig::default().with_gnss_navigation(true);
        assert!(cfg.listen_rapid_position);
        assert!(cfg.listen_cog_sog);
        assert!(cfg.listen_attitude);
        assert!(cfg.listen_rate_of_turn);
        assert!(cfg.listen_position_detail);
        assert!(cfg.listen_gnss_dops);
        assert!(cfg.listen_magnetic_variation);
        assert!(cfg.listen_system_time);
        assert!(cfg.listen_heading);
        assert!(!cfg.listen_wind);
        assert!(!cfg.listen_temperature);
        assert!(!cfg.listen_humidity);
        assert!(!cfg.listen_pressure);
        assert!(!cfg.listen_engine);
        assert!(!cfg.listen_fluid_level);
        assert!(!cfg.listen_battery);
        assert!(!cfg.listen_depth);
        assert!(!cfg.listen_speed_water);
        assert!(!cfg.listen_xte);
        assert!(!cfg.listen_rudder);
    }

    #[test]
    fn disabled_pgn_is_dropped() {
        // Default NMEAConfig does NOT listen to wind.
        let mut iface = NMEAInterface::default();
        let log: Rc<RefCell<i32>> = Rc::new(RefCell::new(0));
        let lc = log.clone();
        iface.on_wind.subscribe(move |_| *lc.borrow_mut() += 1);
        let bytes = NMEAInterface::build_wind(&WindData::default());
        iface.handle_message(&nmea_msg(PGN_WIND_DATA, bytes.to_vec()));
        assert_eq!(*log.borrow(), 0);
    }

    #[test]
    fn engine_decode_handles_unavailable_sentinels() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        let log: Rc<RefCell<Vec<EngineData>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        iface.on_engine.subscribe(move |e| lc.borrow_mut().push(*e));
        // All-FF payload → only instance from byte 0 reaches us.
        let mut data = [0xFFu8; 8];
        data[0] = 3;
        iface.handle_message(&nmea_msg(PGN_ENGINE_PARAMS_RAPID, data.to_vec()));
        assert_eq!(log.borrow()[0].instance, 3);
        assert_eq!(log.borrow()[0].rpm, 0.0);
    }

    #[test]
    fn classic_single_frame_handlers_reject_bad_reserved_tails() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        iface.handle_message(&nmea_msg(
            PGN_GNSS_POSITION_RAPID,
            NMEAInterface::build_position(&GNSSPosition {
                wgs: Wgs::new(52.0, 5.0, 0.0),
                ..Default::default()
            })
            .to_vec(),
        ));

        let events: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
        macro_rules! count_event {
            ($event:expr) => {{
                let log = events.clone();
                $event.subscribe(move |_| *log.borrow_mut() += 1);
            }};
        }

        count_event!(iface.on_cog);
        count_event!(iface.on_sog);
        count_event!(iface.on_attitude);
        count_event!(iface.on_wind);
        count_event!(iface.on_temperature);
        count_event!(iface.on_engine);
        count_event!(iface.on_magnetic_variation);
        count_event!(iface.on_gnss_dops);
        count_event!(iface.on_rudder);
        count_event!(iface.on_fluid_level);
        count_event!(iface.on_battery);
        count_event!(iface.on_speed_water);
        count_event!(iface.on_xte);
        count_event!(iface.on_humidity);
        count_event!(iface.on_pressure);
        count_event!(iface.on_outside_environmental);

        let mut cog_sog = NMEAInterface::build_cog_sog(1.0, 2.0);
        cog_sog[6] = 0x00;
        iface.handle_message(&nmea_msg(PGN_GNSS_COG_SOG_RAPID, cog_sog.to_vec()));

        let mut attitude = [0xFF, 0, 0, 0, 0, 0, 0, 0x00];
        attitude[6] = 0xFF;
        iface.handle_message(&nmea_msg(PGN_ATTITUDE, attitude.to_vec()));

        iface.handle_message(&nmea_msg(
            PGN_RATE_OF_TURN,
            vec![0x05, 0x00, 0xD4, 0x30, 0x00, 0x00, 0xFF, 0xFF],
        ));
        iface.handle_message(&nmea_msg(
            PGN_GNSS_DOPS,
            vec![0x2A, 0xD3, 0x55, 0x00, 0x6E, 0x00, 0x32, 0x00],
        ));
        iface.handle_message(&nmea_msg(
            PGN_GNSS_DOPS,
            vec![0x2A, 0x14, 0x55, 0x00, 0x6E, 0x00, 0x32, 0x00],
        ));

        let mut wind = NMEAInterface::build_wind(&WindData {
            sid: 7,
            speed_mps: 12.5,
            direction_rad: 1.0,
            reference: WindReference::TrueNorth,
        });
        wind[6] = 0x00;
        iface.handle_message(&nmea_msg(PGN_WIND_DATA, wind.to_vec()));
        let mut wind_bad_reference = NMEAInterface::build_wind(&WindData {
            sid: 7,
            speed_mps: 12.5,
            direction_rad: 1.0,
            reference: WindReference::TrueNorth,
        });
        wind_bad_reference[5] = 0x05;
        iface.handle_message(&nmea_msg(PGN_WIND_DATA, wind_bad_reference.to_vec()));

        let mut temperature = NMEAInterface::build_temperature(&TemperatureData {
            sid: 1,
            instance: 0,
            source: TemperatureSource::Outside,
            actual_k: 293.15,
            set_k: 295.15,
        });
        temperature[7] = 0x00;
        iface.handle_message(&nmea_msg(PGN_TEMPERATURE, temperature.to_vec()));
        let mut temperature_bad_source = NMEAInterface::build_temperature(&TemperatureData {
            sid: 1,
            instance: 0,
            source: TemperatureSource::Outside,
            actual_k: 293.15,
            set_k: 295.15,
        });
        temperature_bad_source[2] = 0x10;
        iface.handle_message(&nmea_msg(PGN_TEMPERATURE, temperature_bad_source.to_vec()));

        let mut engine = NMEAInterface::build_engine_params(&EngineData {
            instance: 1,
            rpm: 1200.0,
            boost_pressure_pa: 10_000.0,
            tilt_trim: 0,
        });
        engine[6] = 0x00;
        iface.handle_message(&nmea_msg(PGN_ENGINE_PARAMS_RAPID, engine.to_vec()));

        let mut variation = NMEAInterface::build_magnetic_variation(0.1, 5);
        variation[6] = 0x00;
        iface.handle_message(&nmea_msg(PGN_MAGNETIC_VARIATION, variation.to_vec()));

        let mut rudder = NMEAInterface::build_rudder(&RudderData {
            instance: 2,
            direction: RudderDirection::Port,
            angle_order_rad: -0.1,
            position_rad: 0.25,
        });
        rudder[6] = 0x00;
        iface.handle_message(&nmea_msg(PGN_RUDDER, rudder.to_vec()));
        let mut rudder_bad_control = NMEAInterface::build_rudder(&RudderData {
            instance: 2,
            direction: RudderDirection::Port,
            angle_order_rad: -0.1,
            position_rad: 0.25,
        });
        rudder_bad_control[1] &= 0x07;
        iface.handle_message(&nmea_msg(PGN_RUDDER, rudder_bad_control.to_vec()));

        let mut fluid = NMEAInterface::build_fluid_level(&FluidLevelData {
            instance: 1,
            r#type: FluidType::Fuel,
            level_pct: 55.0,
            capacity_l: 100.0,
        });
        fluid[7] = 0x00;
        iface.handle_message(&nmea_msg(PGN_FLUID_LEVEL, fluid.to_vec()));
        let mut fluid_bad_type = NMEAInterface::build_fluid_level(&FluidLevelData {
            instance: 1,
            r#type: FluidType::Fuel,
            level_pct: 55.0,
            capacity_l: 100.0,
        });
        fluid_bad_type[0] = 0x71;
        iface.handle_message(&nmea_msg(PGN_FLUID_LEVEL, fluid_bad_type.to_vec()));

        let mut battery = NMEAInterface::build_battery_status(&BatteryStatusData {
            instance: 1,
            voltage: 12.34,
            current_a: -5.6,
            ..Default::default()
        });
        battery[5] = 0x00;
        iface.handle_message(&nmea_msg(PGN_BATTERY_STATUS, battery.to_vec()));

        let mut speed = NMEAInterface::build_speed_water(&SpeedWaterData {
            sid: 3,
            water_speed_mps: 2.0,
            ground_speed_mps: 2.1,
            reference: SpeedWaterRefType::PaddleWheel,
        });
        speed[6] = 0x00;
        iface.handle_message(&nmea_msg(PGN_SPEED_WATER, speed.to_vec()));
        let mut speed_bad_reference = NMEAInterface::build_speed_water(&SpeedWaterData {
            sid: 3,
            water_speed_mps: 2.0,
            ground_speed_mps: 2.1,
            reference: SpeedWaterRefType::PaddleWheel,
        });
        speed_bad_reference[5] = 0x05;
        iface.handle_message(&nmea_msg(PGN_SPEED_WATER, speed_bad_reference.to_vec()));

        let mut xte = NMEAInterface::build_xte(&XTEData {
            sid: 4,
            mode: XTEMode::Autonomous,
            navigation_terminated: false,
            xte_m: 1.0,
        });
        xte[6] = 0x00;
        iface.handle_message(&nmea_msg(PGN_XTE, xte.to_vec()));
        let mut xte_bad_mode = NMEAInterface::build_xte(&XTEData {
            sid: 4,
            mode: XTEMode::Autonomous,
            navigation_terminated: false,
            xte_m: 1.0,
        });
        xte_bad_mode[1] = 0x05;
        iface.handle_message(&nmea_msg(PGN_XTE, xte_bad_mode.to_vec()));

        let mut humidity = NMEAInterface::build_humidity(&HumidityData {
            sid: 2,
            instance: 1,
            source: HumiditySource::Outside,
            actual_pct: 55.5,
            set_pct: 60.0,
        });
        humidity[7] = 0x00;
        iface.handle_message(&nmea_msg(PGN_HUMIDITY, humidity.to_vec()));
        let mut humidity_bad_source = NMEAInterface::build_humidity(&HumidityData {
            sid: 2,
            instance: 1,
            source: HumiditySource::Outside,
            actual_pct: 55.5,
            set_pct: 60.0,
        });
        humidity_bad_source[2] = 0x02;
        iface.handle_message(&nmea_msg(PGN_HUMIDITY, humidity_bad_source.to_vec()));

        let mut pressure = NMEAInterface::build_pressure(&PressureData {
            sid: 3,
            instance: 2,
            source: PressureSource::Atmospheric,
            pressure_pa: 101_300.0,
        });
        pressure[7] = 0x00;
        iface.handle_message(&nmea_msg(PGN_PRESSURE, pressure.to_vec()));
        let mut pressure_bad_source = NMEAInterface::build_pressure(&PressureData {
            sid: 3,
            instance: 2,
            source: PressureSource::Atmospheric,
            pressure_pa: 101_300.0,
        });
        pressure_bad_source[2] = 0x09;
        iface.handle_message(&nmea_msg(PGN_PRESSURE, pressure_bad_source.to_vec()));

        let mut outside = NMEAInterface::build_outside_environmental(&OutsideEnvironmentalData {
            sid: 4,
            water_temperature_k: 285.15,
            outside_temperature_k: 293.15,
            atmospheric_pressure_pa: 101_300.0,
        });
        outside[7] = 0x00;
        iface.handle_message(&nmea_msg(PGN_OUTSIDE_ENVIRONMENTAL, outside.to_vec()));

        assert_eq!(*events.borrow(), 0);
        assert!(
            iface
                .latest_position()
                .and_then(|p| p.rate_of_turn_rps)
                .is_none()
        );
        assert!(iface.latest_position().and_then(|p| p.hdop).is_none());
    }

    #[test]
    fn classic_single_frame_handlers_reject_short_and_overlong_payloads() {
        let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
        let events: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));

        macro_rules! count_event {
            ($event:expr) => {{
                let log = events.clone();
                $event.subscribe(move |_| *log.borrow_mut() += 1);
            }};
        }

        count_event!(iface.on_position);
        count_event!(iface.on_cog);
        count_event!(iface.on_sog);
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

        for pgn in [
            PGN_GNSS_POSITION_RAPID,
            PGN_GNSS_COG_SOG_RAPID,
            PGN_ATTITUDE,
            PGN_RATE_OF_TURN,
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
            iface.handle_message(&nmea_msg(pgn, vec![0x00; 7]));
            iface.handle_message(&nmea_msg(pgn, vec![0x00; 9]));
        }

        assert_eq!(*events.borrow(), 0);
        assert!(iface.latest_position().is_none());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_nmea2000_interface_accepts_or_rejects_arbitrary_payloads_without_panics(
            pgn_index in 0usize..NMEA2000_INTERFACE_PGNS.len(),
            data in proptest::collection::vec(any::<u8>(), 0..=128),
            source in any::<u8>(),
            timestamp_us in any::<u64>(),
        ) {
            let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
            let mut msg = Message::new(NMEA2000_INTERFACE_PGNS[pgn_index], data, source);
            msg.timestamp_us = timestamp_us;

            iface.handle_message(&msg);

            if let Some(pos) = iface.latest_position() {
                prop_assert!(pos.wgs.latitude.is_finite());
                prop_assert!(pos.wgs.longitude.is_finite());
                prop_assert!(pos.wgs.altitude.is_finite());
            }
        }
    }
}
