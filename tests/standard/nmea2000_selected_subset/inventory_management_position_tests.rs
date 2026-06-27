use machbus::geo::Wgs;
use machbus::net::pgn_defs::{
    PGN_ATTITUDE, PGN_BATTERY_STATUS, PGN_CONFIG_INFO, PGN_ENGINE_PARAMS_RAPID, PGN_FLUID_LEVEL,
    PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_DOPS, PGN_GNSS_POSITION_DATA, PGN_GNSS_POSITION_DELTA,
    PGN_GNSS_POSITION_RAPID, PGN_HEADING_TRACK, PGN_HEARTBEAT_N2K, PGN_HUMIDITY,
    PGN_MAGNETIC_VARIATION, PGN_OUTSIDE_ENVIRONMENTAL, PGN_PRESSURE, PGN_PRODUCT_INFO,
    PGN_RATE_OF_TURN, PGN_RUDDER, PGN_SPEED_WATER, PGN_SYSTEM_TIME, PGN_TEMPERATURE,
    PGN_WATER_DEPTH, PGN_WIND_DATA, PGN_XTE,
};
use machbus::net::{
    BROADCAST_ADDRESS, ErrorCode, FAST_PACKET_MAX_DATA, FastPacketProtocol, Frame, Identifier,
    Message, NULL_ADDRESS, Priority, TP_TIMEOUT_T1_MS,
};
use machbus::nmea::position::GNSSPosition;
use machbus::nmea::{
    AISDTE, AISMode, AISNavStatus, AISRepeat, AISTransceiverInfo, AISUnit, BatteryChemistry,
    BatteryEqSupport, BatteryNominalVoltage, BatteryStatusData, BatteryType, ChargeState,
    ChargerMode, ConverterMode, DCType, DataMode, DelaySource, DistanceCalculationType, EngineData,
    FluidLevelData, FluidType, GNSSDOPData, GNSSDOPMode, GNSSFixType, GNSSSystem,
    HEADING_RESOLUTION, HUMIDITY_RESOLUTION, HeadingReference, HumidityData, HumiditySource,
    MOBBatteryStatus, MOBPositionSource, MOBStatus, N2K_REQUEST_TIMEOUT_MS, N2KConfigInfo,
    N2KHeartbeat, N2KManagement, N2KManagementConfig, N2KProductInfo, NMEA2000_INTERFACE_PGNS,
    NMEA2000_MANAGEMENT_PGNS, NMEA2000_SELECTED_PGNS, NMEAConfig, NMEAInterface,
    NavigationDirection, OnOff, OutsideEnvironmentalData,
    PositionDeltaHighPrecisionRapidUpdateData, PressureData, PressureSource, RangeResidualMode,
    ReferenceStationType, RudderData, RudderDirection, SerialGNSS, SpeedWaterData,
    SpeedWaterRefType, SteeringMode, SystemTimeData, TemperatureData, TemperatureSource,
    TimeSource, TransmissionGear, TurnMode, WaterDepthData, WindData, WindReference, XTEData,
    XTEMode,
};

use std::cell::RefCell;
use std::rc::Rc;

fn standard_position_detail_frame() -> Vec<u8> {
    let mut detail = vec![0xFFu8; 43];
    let lat_raw = (52.0_f64 * 1e16) as i64;
    let lon_raw = (5.0_f64 * 1e16) as i64;
    let alt_raw = (12.5_f64 * 1e6) as i64;
    detail[7..15].copy_from_slice(&lat_raw.to_le_bytes());
    detail[15..23].copy_from_slice(&lon_raw.to_le_bytes());
    detail[23..31].copy_from_slice(&alt_raw.to_le_bytes());
    detail[31] = 0x10;
    detail[33] = 7;
    detail[34..36].copy_from_slice(&100u16.to_le_bytes());
    detail[36..38].copy_from_slice(&150u16.to_le_bytes());
    detail[42] = 0;
    detail
}

fn nmea0183_with_checksum(body: &str) -> String {
    let checksum = body.as_bytes()[1..]
        .iter()
        .fold(0u8, |acc, byte| acc ^ byte);
    format!("{body}*{checksum:02X}\n")
}

#[test]
fn nmea2000_selected_pgn_inventory_is_explicit_partitioned_and_sorted() {
    let expected_interface = [
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
    ];
    assert_eq!(NMEA2000_INTERFACE_PGNS, expected_interface);

    let expected_management = [PGN_HEARTBEAT_N2K, PGN_PRODUCT_INFO, PGN_CONFIG_INFO];
    assert_eq!(NMEA2000_MANAGEMENT_PGNS, expected_management);

    let expected_selected = [
        PGN_SYSTEM_TIME,
        PGN_HEARTBEAT_N2K,
        PGN_PRODUCT_INFO,
        PGN_CONFIG_INFO,
        PGN_RUDDER,
        PGN_HEADING_TRACK,
        PGN_RATE_OF_TURN,
        PGN_ATTITUDE,
        PGN_MAGNETIC_VARIATION,
        PGN_ENGINE_PARAMS_RAPID,
        PGN_FLUID_LEVEL,
        PGN_BATTERY_STATUS,
        PGN_SPEED_WATER,
        PGN_WATER_DEPTH,
        PGN_GNSS_POSITION_RAPID,
        PGN_GNSS_COG_SOG_RAPID,
        PGN_GNSS_POSITION_DELTA,
        PGN_GNSS_POSITION_DATA,
        PGN_XTE,
        PGN_GNSS_DOPS,
        PGN_WIND_DATA,
        PGN_OUTSIDE_ENVIRONMENTAL,
        PGN_TEMPERATURE,
        PGN_HUMIDITY,
        PGN_PRESSURE,
    ];
    assert_eq!(NMEA2000_SELECTED_PGNS, expected_selected);
    assert!(
        NMEA2000_SELECTED_PGNS
            .windows(2)
            .all(|window| window[0] < window[1]),
        "selected PGN inventory must stay sorted and duplicate-free"
    );

    for pgn in NMEA2000_INTERFACE_PGNS {
        assert!(
            NMEA2000_SELECTED_PGNS.contains(&pgn),
            "interface PGN {pgn} must appear in the selected subset"
        );
    }
    for pgn in NMEA2000_MANAGEMENT_PGNS {
        assert!(
            NMEA2000_SELECTED_PGNS.contains(&pgn),
            "management PGN {pgn} must appear in the selected subset"
        );
    }

    let all = NMEAConfig::default().with_all(true);
    assert!(all.listen_rapid_position);
    assert!(all.listen_cog_sog);
    assert!(all.listen_position_delta);
    assert!(all.listen_position_detail);
    assert!(all.listen_gnss_dops);
    assert!(all.listen_system_time);
    assert!(all.listen_heading);
    assert!(all.listen_wind);
    assert!(all.listen_temperature);
    assert!(all.listen_pressure);
    assert!(all.listen_engine);
    assert!(all.listen_battery);

    let navigation = NMEAConfig::default().with_gnss_navigation(true);
    assert!(navigation.listen_position_delta);
    assert!(navigation.listen_position_detail);
    assert!(navigation.listen_system_time);
    assert!(!navigation.listen_wind);
    assert!(!navigation.listen_engine);
    assert!(!navigation.listen_battery);
}

#[test]
fn nmea2000_heartbeat_rejects_non_canonical_reserved_bits_and_ranges() {
    let heartbeat = N2KHeartbeat {
        update_interval_ms: 60_000,
        sequence_counter: 5,
        controller_class1: 0xAA,
        controller_class2: 0xBB,
    };
    let encoded = heartbeat.encode().unwrap();
    assert_eq!(N2KHeartbeat::decode(&encoded), Some(heartbeat));

    let mut bad_reserved = encoded;
    bad_reserved[2] = 0x05;
    assert_eq!(N2KHeartbeat::decode(&bad_reserved), None);

    assert!(
        N2KHeartbeat {
            sequence_counter: 16,
            ..heartbeat
        }
        .encode()
        .is_err()
    );
    assert!(
        N2KHeartbeat {
            update_interval_ms: 55,
            ..heartbeat
        }
        .encode()
        .is_err()
    );
}

#[test]
fn nmea2000_management_text_fields_reject_overwide_values() {
    let product = N2KProductInfo {
        model_id: "machbus".to_owned(),
        software_version: "0.1.3".to_owned(),
        model_version: "test".to_owned(),
        serial_code: "serial".to_owned(),
        ..N2KProductInfo::default()
    };
    let product_bytes = product.encode().unwrap();
    assert_eq!(N2KProductInfo::decode(&product_bytes).unwrap(), product);

    let overwide = "x".repeat(71);
    assert!(
        N2KConfigInfo {
            installation_desc1: overwide,
            installation_desc2: String::new(),
            manufacturer_info: String::new(),
        }
        .encode()
        .is_err()
    );
}

#[test]
fn nmea2000_public_try_decoders_reject_noncanonical_packed_bytes() {
    assert_eq!(
        HeadingReference::try_from_u8(1),
        Some(HeadingReference::Magnetic)
    );
    assert_eq!(HeadingReference::try_from_u8(0xFC | 1), None);

    assert_eq!(GNSSDOPMode::try_from_u8(3), Some(GNSSDOPMode::Auto));
    assert_eq!(GNSSDOPMode::try_from_u8(0xF8 | 3), None);
    assert_eq!(GNSSSystem::try_from_u8(8), Some(GNSSSystem::Galileo));
    assert_eq!(GNSSSystem::try_from_u8(9), None);
    assert_eq!(
        ReferenceStationType::try_from_u8(15),
        Some(ReferenceStationType::Unavailable)
    );
    assert_eq!(
        ReferenceStationType::try_from_u8(14),
        Some(ReferenceStationType::Error)
    );
    assert_eq!(ReferenceStationType::try_from_u8(2), None);
    assert_eq!(ReferenceStationType::try_from_u8(3), None);

    assert_eq!(FluidType::try_from_u8(1), Some(FluidType::Water));
    assert_eq!(FluidType::try_from_u8(0xF0 | 1), None);

    assert_eq!(
        RudderDirection::try_from_u8(1),
        Some(RudderDirection::Starboard)
    );
    assert_eq!(RudderDirection::try_from_u8(0xF8 | 1), None);
    assert_eq!(
        NavigationDirection::try_from_u8(7),
        Some(NavigationDirection::Unknown)
    );
    assert_eq!(NavigationDirection::try_from_u8(2), None);
    assert_eq!(
        DistanceCalculationType::try_from_u8(1),
        Some(DistanceCalculationType::RhumbLine)
    );
    assert_eq!(DistanceCalculationType::try_from_u8(0xFE | 1), None);

    assert_eq!(XTEMode::try_from_u8(4), Some(XTEMode::Manual));
    assert_eq!(XTEMode::try_from_u8(0xF0 | 4), None);

    assert_eq!(DCType::try_from_u8(4), Some(DCType::WindGenerator));
    assert_eq!(DCType::try_from_u8(5), None);
    assert_eq!(DCType::try_from_u8(0xF0 | 4), None);

    assert_eq!(BatteryType::try_from_u8(2), Some(BatteryType::AGM));
    assert_eq!(BatteryType::try_from_u8(3), None);
    assert_eq!(
        BatteryChemistry::try_from_u8(4),
        Some(BatteryChemistry::NiMh)
    );
    assert_eq!(BatteryChemistry::try_from_u8(5), None);
    assert_eq!(
        BatteryNominalVoltage::try_from_u8(6),
        Some(BatteryNominalVoltage::V48)
    );
    assert_eq!(BatteryNominalVoltage::try_from_u8(7), None);
    assert_eq!(
        BatteryEqSupport::try_from_u8(3),
        Some(BatteryEqSupport::Unavailable)
    );
    assert_eq!(BatteryEqSupport::try_from_u8(0xFC | 1), None);

    assert_eq!(
        TransmissionGear::try_from_u8(2),
        Some(TransmissionGear::Reverse)
    );
    assert_eq!(TransmissionGear::try_from_u8(0xFC | 2), None);

    assert_eq!(
        SteeringMode::try_from_u8(5),
        Some(SteeringMode::TrackControl)
    );
    assert_eq!(SteeringMode::try_from_u8(6), None);
    assert_eq!(TurnMode::try_from_u8(2), Some(TurnMode::RadiusControlled));
    assert_eq!(TurnMode::try_from_u8(3), None);

    assert_eq!(OnOff::try_from_u8(1), Some(OnOff::On));
    assert_eq!(OnOff::try_from_u8(0xFC | 1), None);

    assert_eq!(ChargeState::try_from_u8(9), Some(ChargeState::Fault));
    assert_eq!(ChargeState::try_from_u8(10), None);
    assert_eq!(ChargerMode::try_from_u8(15), Some(ChargerMode::Unavailable));
    assert_eq!(ChargerMode::try_from_u8(4), None);
    assert_eq!(ConverterMode::try_from_u8(11), Some(ConverterMode::PSUMode));
    assert_eq!(ConverterMode::try_from_u8(12), None);
    assert_eq!(
        ConverterMode::try_from_u8(0xFF),
        Some(ConverterMode::NotAvailable)
    );

    assert_eq!(MOBStatus::try_from_u8(2), Some(MOBStatus::TestMode));
    assert_eq!(MOBStatus::try_from_u8(0xFC | 2), None);
    assert_eq!(
        MOBPositionSource::try_from_u8(1),
        Some(MOBPositionSource::ReportedByEmitter)
    );
    assert_eq!(MOBPositionSource::try_from_u8(0xFE | 1), None);
    assert_eq!(
        MOBBatteryStatus::try_from_u8(1),
        Some(MOBBatteryStatus::Low)
    );
    assert_eq!(MOBBatteryStatus::try_from_u8(0xFE | 1), None);

    assert_eq!(AISRepeat::try_from_u8(3), Some(AISRepeat::Final));
    assert_eq!(AISRepeat::try_from_u8(0xFC | 3), None);
    assert_eq!(AISNavStatus::try_from_u8(14), Some(AISNavStatus::AIS_SART));
    assert_eq!(AISNavStatus::try_from_u8(11), None);
    assert_eq!(AISUnit::try_from_u8(0), Some(AISUnit::ClassB_SOTDMA));
    assert_eq!(AISUnit::try_from_u8(0xFE | 1), None);
    assert_eq!(AISMode::try_from_u8(1), Some(AISMode::Assigned));
    assert_eq!(AISMode::try_from_u8(0xFE | 1), None);
    assert_eq!(
        AISTransceiverInfo::try_from_u8(4),
        Some(AISTransceiverInfo::OwnInfoNotBroadcast)
    );
    assert_eq!(AISTransceiverInfo::try_from_u8(5), None);
    assert_eq!(AISDTE::try_from_u8(0), Some(AISDTE::Ready));
    assert_eq!(AISDTE::try_from_u8(0xFE | 1), None);

    assert_eq!(DelaySource::try_from_u8(15), Some(DelaySource::Unavailable));
    assert_eq!(DelaySource::try_from_u8(3), None);
    assert_eq!(DataMode::try_from_u8(4), Some(DataMode::Manual));
    assert_eq!(DataMode::try_from_u8(0xF8 | 4), None);
    assert_eq!(
        RangeResidualMode::try_from_u8(3),
        Some(RangeResidualMode::Unavailable)
    );
    assert_eq!(RangeResidualMode::try_from_u8(0xFC | 3), None);
}

#[test]
fn nmea2000_management_product_info_fixed_field_boundaries_are_canonical() {
    let product = N2KProductInfo {
        nmea2000_version: 0x0901,
        product_code: 0x1234,
        model_id: "M".repeat(32),
        software_version: "S".repeat(40),
        model_version: "V".repeat(24),
        serial_code: "N".repeat(32),
        certification_level: 2,
        load_equivalency: 3,
    };
    let product_bytes = product.encode().unwrap();
    assert_eq!(product_bytes.len(), 134);
    assert_eq!(N2KProductInfo::decode(&product_bytes).unwrap(), product);

    for overwide_product in [
        N2KProductInfo {
            model_id: "M".repeat(33),
            ..N2KProductInfo::default()
        },
        N2KProductInfo {
            software_version: "S".repeat(41),
            ..N2KProductInfo::default()
        },
        N2KProductInfo {
            model_version: "V".repeat(25),
            ..N2KProductInfo::default()
        },
        N2KProductInfo {
            serial_code: "N".repeat(33),
            ..N2KProductInfo::default()
        },
    ] {
        assert_eq!(
            overwide_product.encode().unwrap_err().code,
            ErrorCode::InvalidData
        );
    }

    assert_eq!(
        N2KProductInfo {
            model_id: "bad\nmodel".into(),
            ..N2KProductInfo::default()
        }
        .encode()
        .unwrap_err()
        .code,
        ErrorCode::InvalidData
    );
    assert_eq!(
        N2KProductInfo::decode(&product_bytes[..133])
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );

    let mut overlong = product_bytes.clone();
    overlong.push(0xFF);
    assert_eq!(
        N2KProductInfo::decode(&overlong).unwrap_err().code,
        ErrorCode::InvalidData
    );

    let mut non_printable = product_bytes;
    non_printable[4] = 0x1F;
    assert_eq!(
        N2KProductInfo::decode(&non_printable).unwrap_err().code,
        ErrorCode::InvalidData
    );
}

#[test]
fn nmea2000_management_product_info_rejects_hidden_fixed_field_garbage() {
    let product = N2KProductInfo {
        model_id: "M".into(),
        software_version: "S".into(),
        model_version: "V".into(),
        serial_code: "N".into(),
        ..N2KProductInfo::default()
    };
    let product_bytes = product.encode().unwrap();
    assert_eq!(N2KProductInfo::decode(&product_bytes).unwrap(), product);

    let mut hidden_after_ff_padding = product_bytes.clone();
    hidden_after_ff_padding[6] = b'X';
    assert_eq!(
        N2KProductInfo::decode(&hidden_after_ff_padding)
            .unwrap_err()
            .code,
        ErrorCode::InvalidData,
        "fixed ProductInfo strings must not hide printable data after canonical padding"
    );

    let mut hidden_after_zero_terminator = product_bytes;
    hidden_after_zero_terminator[5] = 0x00;
    hidden_after_zero_terminator[6] = b'X';
    assert_eq!(
        N2KProductInfo::decode(&hidden_after_zero_terminator)
            .unwrap_err()
            .code,
        ErrorCode::InvalidData,
        "fixed ProductInfo strings must not hide data after an early terminator"
    );
}

#[test]
fn nmea2000_management_config_info_length_prefixes_are_canonical() {
    let config = N2KConfigInfo {
        installation_desc1: "A".repeat(70),
        installation_desc2: "B".repeat(70),
        manufacturer_info: "C".repeat(70),
    };
    let config_bytes = config.encode().unwrap();
    assert_eq!(N2KConfigInfo::decode(&config_bytes).unwrap(), config);

    let mut overwide_declared = vec![71, 0];
    overwide_declared.extend_from_slice(&[b'A'; 71]);
    overwide_declared.extend_from_slice(&0u16.to_le_bytes());
    overwide_declared.extend_from_slice(&0u16.to_le_bytes());
    assert_eq!(
        N2KConfigInfo::decode(&overwide_declared).unwrap_err().code,
        ErrorCode::InvalidData
    );

    let mut truncated = vec![3, 0, b'A', b'B'];
    truncated.extend_from_slice(&0u16.to_le_bytes());
    truncated.extend_from_slice(&0u16.to_le_bytes());
    assert_eq!(
        N2KConfigInfo::decode(&truncated).unwrap_err().code,
        ErrorCode::InvalidData
    );

    let mut non_printable = N2KConfigInfo {
        installation_desc1: "AB".into(),
        installation_desc2: String::new(),
        manufacturer_info: String::new(),
    }
    .encode()
    .unwrap();
    non_printable[2] = 0x7F;
    assert_eq!(
        N2KConfigInfo::decode(&non_printable).unwrap_err().code,
        ErrorCode::InvalidData
    );

    let mut trailing = N2KConfigInfo::default().encode().unwrap();
    trailing.push(0xFF);
    assert_eq!(
        N2KConfigInfo::decode(&trailing).unwrap_err().code,
        ErrorCode::InvalidData
    );
}

#[test]
fn nmea2000_vessel_heading_rejects_reserved_reference_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let headings: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let heading_log = headings.clone();
    iface
        .on_heading
        .subscribe(move |heading| heading_log.borrow_mut().push(*heading));

    for reserved in [4, 0xFC | HeadingReference::Magnetic.as_u8()] {
        let mut reserved_reference = NMEAInterface::build_heading(1.0, 0.0, 0.0);
        reserved_reference[7] = reserved;
        iface.handle_message(&Message::new(
            PGN_HEADING_TRACK,
            reserved_reference.to_vec(),
            0x24,
        ));
    }
    assert!(headings.borrow().is_empty());
    assert!(iface.latest_position().is_none());

    let valid = NMEAInterface::build_heading(1.0, 0.0, 0.0);
    iface.handle_message(&Message::new(PGN_HEADING_TRACK, valid.to_vec(), 0x24));
    assert_eq!(headings.borrow().len(), 1);
}

#[test]
fn nmea2000_vessel_heading_rejects_numeric_special_values_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));
    let headings: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let heading_log = headings.clone();
    iface
        .on_heading
        .subscribe(move |heading| heading_log.borrow_mut().push(*heading));

    for heading_raw in [u16::MAX - 2, u16::MAX - 1] {
        let mut frame = NMEAInterface::build_heading(1.0, 0.0, 0.0);
        frame[1..3].copy_from_slice(&heading_raw.to_le_bytes());
        iface.handle_message(&Message::new(PGN_HEADING_TRACK, frame.to_vec(), 0x24));
    }
    let mut out_of_range_heading = NMEAInterface::build_heading(1.0, 0.0, 0.0);
    out_of_range_heading[1..3].copy_from_slice(&62_832u16.to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_HEADING_TRACK,
        out_of_range_heading.to_vec(),
        0x24,
    ));
    for range in [3..5, 5..7] {
        let mut frame = NMEAInterface::build_heading(1.0, 0.0, 0.0);
        frame[range].copy_from_slice(&(i16::MAX - 2).to_le_bytes());
        iface.handle_message(&Message::new(PGN_HEADING_TRACK, frame.to_vec(), 0x24));
    }

    assert!(headings.borrow().is_empty());
    assert!(iface.latest_position().unwrap().heading_rad.is_none());

    let valid = NMEAInterface::build_heading(1.0, 0.0, 0.0);
    iface.handle_message(&Message::new(PGN_HEADING_TRACK, valid.to_vec(), 0x24));
    assert_eq!(headings.borrow().len(), 1);
    assert!(iface.latest_position().unwrap().heading_rad.is_some());

    let clamped = NMEAInterface::build_heading(std::f64::consts::TAU + 1.0, 0.0, 0.0);
    assert_eq!(
        u16::from_le_bytes([clamped[1], clamped[2]]),
        62_831,
        "heading builder must not emit circular-angle values above one revolution"
    );
}

#[test]
fn nmea2000_system_time_rejects_reserved_date_time_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let times: Rc<RefCell<Vec<SystemTimeData>>> = Rc::new(RefCell::new(Vec::new()));
    let time_log = times.clone();
    iface
        .on_system_time
        .subscribe(move |time| time_log.borrow_mut().push(*time));

    let valid = NMEAInterface::build_system_time(&SystemTimeData {
        sid: 5,
        source: TimeSource::GPS,
        days_since_epoch: 12_345,
        seconds_since_midnight: 3_600.5,
    });
    for day_raw in [u16::MAX - 2, u16::MAX - 1] {
        let mut frame = valid;
        frame[2..4].copy_from_slice(&day_raw.to_le_bytes());
        iface.handle_message(&Message::new(PGN_SYSTEM_TIME, frame.to_vec(), 0x24));
    }
    for seconds_raw in [u32::MAX - 2, u32::MAX - 1] {
        let mut frame = valid;
        frame[4..8].copy_from_slice(&seconds_raw.to_le_bytes());
        iface.handle_message(&Message::new(PGN_SYSTEM_TIME, frame.to_vec(), 0x24));
    }

    assert!(times.borrow().is_empty());

    iface.handle_message(&Message::new(PGN_SYSTEM_TIME, valid.to_vec(), 0x24));
    assert_eq!(times.borrow().len(), 1);
    assert_eq!(times.borrow()[0].days_since_epoch, 12_345);
}

#[test]
fn nmea2000_system_time_rejects_out_of_range_time_of_day_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let times: Rc<RefCell<Vec<SystemTimeData>>> = Rc::new(RefCell::new(Vec::new()));
    let time_log = times.clone();
    iface
        .on_system_time
        .subscribe(move |time| time_log.borrow_mut().push(*time));

    let mut valid = NMEAInterface::build_system_time(&SystemTimeData {
        sid: 5,
        source: TimeSource::GPS,
        days_since_epoch: 12_345,
        seconds_since_midnight: 86_401.0,
    });
    valid[4..8].copy_from_slice(&864_010_000u32.to_le_bytes());
    iface.handle_message(&Message::new(PGN_SYSTEM_TIME, valid.to_vec(), 0x24));
    assert_eq!(times.borrow().len(), 1);

    let mut out_of_range = valid;
    out_of_range[4..8].copy_from_slice(&864_010_001u32.to_le_bytes());
    iface.handle_message(&Message::new(PGN_SYSTEM_TIME, out_of_range.to_vec(), 0x24));
    assert_eq!(
        times.borrow().len(),
        1,
        "time-of-day values above the supported range must not emit system-time events"
    );
}

#[test]
fn nmea2000_cog_sog_rejects_reserved_reference_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));
    let cogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let sogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let cog_log = cogs.clone();
    let sog_log = sogs.clone();
    iface
        .on_cog
        .subscribe(move |cog| cog_log.borrow_mut().push(*cog));
    iface
        .on_sog
        .subscribe(move |sog| sog_log.borrow_mut().push(*sog));

    for reserved in [4, 0xFC | HeadingReference::Magnetic.as_u8()] {
        let mut reserved_reference = NMEAInterface::build_cog_sog(1.0, 2.0);
        reserved_reference[1] = reserved;
        iface.handle_message(&Message::new(
            PGN_GNSS_COG_SOG_RAPID,
            reserved_reference.to_vec(),
            0x24,
        ));
    }
    let cached = iface.latest_position().unwrap();
    assert!(cached.cog_rad.is_none());
    assert!(cached.speed_mps.is_none());
    assert!(cogs.borrow().is_empty());
    assert!(sogs.borrow().is_empty());

    let mut magnetic_reference = NMEAInterface::build_cog_sog(1.0, 2.0);
    magnetic_reference[1] = 1;
    iface.handle_message(&Message::new(
        PGN_GNSS_COG_SOG_RAPID,
        magnetic_reference.to_vec(),
        0x24,
    ));
    assert_eq!(cogs.borrow().len(), 1);
    assert_eq!(sogs.borrow().len(), 1);
}

#[test]
fn nmea2000_cog_sog_rejects_reserved_numeric_special_values_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));
    let cogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let sogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let cog_log = cogs.clone();
    let sog_log = sogs.clone();
    iface
        .on_cog
        .subscribe(move |cog| cog_log.borrow_mut().push(*cog));
    iface
        .on_sog
        .subscribe(move |sog| sog_log.borrow_mut().push(*sog));

    let valid = NMEAInterface::build_cog_sog(1.0, 2.0);
    for raw_cog in [0xFFFDu16, 0xFFFE] {
        let mut reserved_cog = valid;
        reserved_cog[2..4].copy_from_slice(&raw_cog.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_GNSS_COG_SOG_RAPID,
            reserved_cog.to_vec(),
            0x24,
        ));
    }
    let mut out_of_range_cog = valid;
    out_of_range_cog[2..4].copy_from_slice(&62_832u16.to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_GNSS_COG_SOG_RAPID,
        out_of_range_cog.to_vec(),
        0x24,
    ));
    for raw_sog in [0xFFFDu16, 0xFFFE] {
        let mut reserved_sog = valid;
        reserved_sog[4..6].copy_from_slice(&raw_sog.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_GNSS_COG_SOG_RAPID,
            reserved_sog.to_vec(),
            0x24,
        ));
    }

    let cached = iface.latest_position().unwrap();
    assert!(cached.cog_rad.is_none());
    assert!(cached.speed_mps.is_none());
    assert!(cogs.borrow().is_empty());
    assert!(sogs.borrow().is_empty());

    iface.handle_message(&Message::new(PGN_GNSS_COG_SOG_RAPID, valid.to_vec(), 0x24));
    let cached = iface.latest_position().unwrap();
    assert!(cached.cog_rad.is_some());
    assert!(cached.speed_mps.is_some());
    assert_eq!(cogs.borrow().len(), 1);
    assert_eq!(sogs.borrow().len(), 1);

    let clamped = NMEAInterface::build_cog_sog(std::f64::consts::TAU + 1.0, 2.0);
    assert_eq!(
        u16::from_le_bytes([clamped[2], clamped[3]]),
        62_831,
        "COG builder must not emit circular-angle values above one revolution"
    );
}

#[test]
fn nmea2000_position_detail_rejects_reserved_fix_method_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut detail = standard_position_detail_frame();
    detail[31] = 0x90;

    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, detail.clone(), 0x24));
    assert!(positions.borrow().is_empty());
    assert!(iface.latest_position().is_none());

    detail[31] = 0x10;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, detail, 0x24));
    assert_eq!(positions.borrow().len(), 1);
    assert!(iface.latest_position().is_some());
}

#[test]
fn nmea2000_position_detail_accepts_defined_manual_and_simulated_fix_methods() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let fixes: Rc<RefCell<Vec<GNSSFixType>>> = Rc::new(RefCell::new(Vec::new()));
    let fix_log = fixes.clone();
    iface.on_position.subscribe(move |pos| {
        fix_log.borrow_mut().push(pos.fix_type);
    });

    let cases = [
        (0x70, GNSSFixType::ManualInput),
        (0x80, GNSSFixType::SimulateMode),
    ];
    for (type_byte, expected_fix) in cases {
        let mut detail = standard_position_detail_frame();
        detail[31] = type_byte;
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, detail, 0x24));
        assert_eq!(iface.latest_position().unwrap().fix_type, expected_fix);
    }

    assert_eq!(
        fixes.borrow().as_slice(),
        &[GNSSFixType::ManualInput, GNSSFixType::SimulateMode]
    );
}

#[test]
fn nmea2000_position_detail_rejects_reserved_gnss_system_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<GNSSSystem>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface.on_position.subscribe(move |pos| {
        position_log.borrow_mut().push(pos.gnss_system);
    });

    let mut valid = standard_position_detail_frame();
    valid[31] = (1 << 4) | GNSSSystem::GPS_SBAS_GLO.as_u8();
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().as_slice(), &[GNSSSystem::GPS_SBAS_GLO]);
    assert_eq!(
        iface.latest_position().unwrap().gnss_system,
        GNSSSystem::GPS_SBAS_GLO
    );

    for defined_system in [
        GNSSSystem::Chayka,
        GNSSSystem::Integrated,
        GNSSSystem::Surveyed,
        GNSSSystem::Galileo,
    ] {
        let mut defined = standard_position_detail_frame();
        defined[31] = (1 << 4) | defined_system.as_u8();
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, defined, 0x44));
        assert_eq!(iface.latest_position().unwrap().gnss_system, defined_system);
    }

    let mut reserved_system = valid;
    reserved_system[31] = (1 << 4) | 0x09;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved_system, 0x44));
    assert_eq!(
        positions.borrow().len(),
        5,
        "reserved GNSS system nibble must not emit a second position event"
    );
    assert_eq!(
        iface.latest_position().unwrap().gnss_system,
        GNSSSystem::Galileo,
        "reserved GNSS system nibble must not replace the cached GNSS fix"
    );
}

#[test]
fn nmea2000_position_detail_rejects_reserved_reference_station_count_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[42] = 0xFF;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);

    valid[42] = 0;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 2);

    let cached = iface.latest_position().unwrap();
    for count in [0xFD, 0xFE] {
        let mut reserved_count = valid.clone();
        reserved_count[42] = count;
        reserved_count.resize(43 + usize::from(count) * 4, 0);
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved_count, 0x44));
    }

    assert_eq!(
        positions.borrow().len(),
        2,
        "reserved reference-station counts must not emit additional position events"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "reserved reference-station counts must not replace cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_reserved_reference_station_type_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[42] = 1;
    valid.extend_from_slice(&[0x01, 0x00, 0x20, 0x00]);
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);

    for station_type in [
        ReferenceStationType::None.as_u8(),
        ReferenceStationType::Error.as_u8(),
        ReferenceStationType::Unavailable.as_u8(),
    ] {
        let mut defined_type = valid.clone();
        defined_type[43] = station_type;
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, defined_type, 0x44));
    }
    assert_eq!(
        positions.borrow().len(),
        4,
        "all defined reference-station type bytes must be accepted"
    );

    let cached = iface.latest_position().unwrap();
    assert!((cached.wgs.latitude - 52.0).abs() < 1e-12);
    assert!((cached.wgs.longitude - 5.0).abs() < 1e-12);

    let mut reserved_type = valid;
    reserved_type[43] = 0x02;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved_type, 0x44));
    assert_eq!(
        positions.borrow().len(),
        4,
        "reserved reference-station type must not emit another position event"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "reserved reference-station type must not replace the cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_reference_station_reserved_age_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[42] = 1;
    valid.extend_from_slice(&[0x01, 0x00, 0x20, 0x00]);
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);
    let cached = iface.latest_position().unwrap();

    for reserved_age in [0xFFFDu16, 0xFFFE] {
        let mut reserved = valid.clone();
        reserved[45..47].copy_from_slice(&reserved_age.to_le_bytes());
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved, 0x44));
    }

    assert_eq!(
        positions.borrow().len(),
        1,
        "reserved reference-station correction age values must not emit another position event"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "reserved reference-station correction age values must not replace the cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_noncanonical_integrity_reserved_bits_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[32] = 0xFD;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);

    let cached = iface.latest_position().unwrap();
    let mut noncanonical = valid;
    noncanonical[32] = 0x03;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, noncanonical, 0x44));
    assert_eq!(
        positions.borrow().len(),
        1,
        "integrity byte with noncanonical reserved bits must not emit a second position event"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "integrity byte with noncanonical reserved bits must not replace cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_geoidal_special_values_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[38..42].copy_from_slice(&100i32.to_le_bytes());
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);

    let cached = iface.latest_position().unwrap();
    let mut reserved = valid;
    reserved[38..42].copy_from_slice(&(i32::MAX - 2).to_le_bytes());
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved, 0x44));
    assert_eq!(
        positions.borrow().len(),
        1,
        "reserved geoidal field must not emit a second position event"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "reserved geoidal field must not replace cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_reserved_date_time_values_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[1..3].copy_from_slice(&12_345u16.to_le_bytes());
    valid[3..7].copy_from_slice(&36_005_000u32.to_le_bytes());
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);

    let cached = iface.latest_position().unwrap();
    for day_raw in [u16::MAX - 2, u16::MAX - 1] {
        let mut reserved_day = valid.clone();
        reserved_day[1..3].copy_from_slice(&day_raw.to_le_bytes());
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved_day, 0x44));
    }
    for time_raw in [u32::MAX - 2, u32::MAX - 1] {
        let mut reserved_time = valid.clone();
        reserved_time[3..7].copy_from_slice(&time_raw.to_le_bytes());
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved_time, 0x44));
    }

    assert_eq!(
        positions.borrow().len(),
        1,
        "reserved date/time values must not emit additional position events"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "reserved date/time values must not replace cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_out_of_range_time_of_day_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[1..3].copy_from_slice(&12_345u16.to_le_bytes());
    valid[3..7].copy_from_slice(&864_010_000u32.to_le_bytes());
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);

    let cached = iface.latest_position().unwrap();
    let mut out_of_range = valid;
    out_of_range[3..7].copy_from_slice(&864_010_001u32.to_le_bytes());
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, out_of_range, 0x44));

    assert_eq!(
        positions.borrow().len(),
        1,
        "out-of-range time-of-day values must not emit additional position events"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "out-of-range time-of-day values must not replace cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_out_of_range_coordinates_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[7..15].copy_from_slice(&900_000_000_000_000_000i64.to_le_bytes());
    valid[15..23].copy_from_slice(&1_800_000_000_000_000_000i64.to_le_bytes());
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);

    let cached = iface.latest_position().unwrap();
    for (range, raw) in [
        (7..15, 900_000_000_000_000_001i64),
        (7..15, -900_000_000_000_000_001i64),
        (15..23, 1_800_000_000_000_000_001i64),
        (15..23, -1_800_000_000_000_000_001i64),
    ] {
        let mut out_of_range = valid.clone();
        out_of_range[range].copy_from_slice(&raw.to_le_bytes());
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, out_of_range, 0x44));
    }

    assert_eq!(
        positions.borrow().len(),
        1,
        "out-of-range coordinates must not emit additional position events"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "out-of-range coordinates must not replace cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_reserved_satellite_count_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = standard_position_detail_frame();
    valid[33] = 0xFF;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 1);
    assert_eq!(iface.latest_position().unwrap().satellites_used, 0);

    valid[33] = 7;
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, valid.clone(), 0x44));
    assert_eq!(positions.borrow().len(), 2);
    assert_eq!(iface.latest_position().unwrap().satellites_used, 7);

    let cached = iface.latest_position().unwrap();
    for count in [0xFD, 0xFE] {
        let mut reserved_count = valid.clone();
        reserved_count[33] = count;
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved_count, 0x44));
    }

    assert_eq!(
        positions.borrow().len(),
        2,
        "reserved satellite counts must not emit additional position events"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "reserved satellite counts must not replace cached GNSS position"
    );
}

#[test]
fn nmea2000_position_detail_rejects_signed_special_values_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let signed_i64_reserved = i64::MAX - 2;
    for range in [7..15, 15..23, 23..31] {
        let mut reserved = standard_position_detail_frame();
        reserved[range].copy_from_slice(&signed_i64_reserved.to_le_bytes());
        iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, reserved, 0x24));
    }

    assert!(positions.borrow().is_empty());
    assert!(iface.latest_position().is_none());

    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_DATA,
        standard_position_detail_frame(),
        0x24,
    ));
    assert_eq!(positions.borrow().len(), 1);
    let cached = iface.latest_position().unwrap();
    assert!(cached.altitude_m.is_some());
    assert_eq!(cached.hdop, Some(1.0));
    assert_eq!(cached.pdop, Some(1.5));
}

#[test]
fn nmea2000_gnss_dop_fields_reject_reserved_and_do_not_emit_negative_values() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_gnss_navigation(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));
    let dops: Rc<RefCell<Vec<GNSSDOPData>>> = Rc::new(RefCell::new(Vec::new()));
    let positions: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
    let dops_log = dops.clone();
    let positions_log = positions.clone();
    iface
        .on_gnss_dops
        .subscribe(move |dop| dops_log.borrow_mut().push(*dop));
    iface
        .on_position
        .subscribe(move |pos| positions_log.borrow_mut().push(*pos));

    let mut valid_dops = [0xFFu8; 8];
    valid_dops[0] = 7;
    valid_dops[1] = 0x1B;
    valid_dops[2..4].copy_from_slice(&100u16.to_le_bytes());
    valid_dops[4..6].copy_from_slice(&150u16.to_le_bytes());
    valid_dops[6..8].copy_from_slice(&200u16.to_le_bytes());
    for range in [2..4, 4..6, 6..8] {
        for reserved in [0xFFFDu16, 0xFFFE] {
            let mut frame = valid_dops;
            frame[range.clone()].copy_from_slice(&reserved.to_le_bytes());
            iface.handle_message(&Message::new(PGN_GNSS_DOPS, frame.to_vec(), 0x24));
        }
    }
    assert!(dops.borrow().is_empty());
    assert!(iface.latest_position().unwrap().hdop.is_none());
    assert!(iface.latest_position().unwrap().vdop.is_none());

    let mut unavailable_dops = valid_dops;
    unavailable_dops[2..4].copy_from_slice(&0xFFFFu16.to_le_bytes());
    unavailable_dops[4..6].copy_from_slice(&0xFFFFu16.to_le_bytes());
    unavailable_dops[6..8].copy_from_slice(&0xFFFFu16.to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_GNSS_DOPS,
        unavailable_dops.to_vec(),
        0x24,
    ));
    assert_eq!(dops.borrow().len(), 1);
    assert_eq!(
        dops.borrow()[0],
        GNSSDOPData {
            sid: 7,
            desired_mode: GNSSDOPMode::Auto,
            actual_mode: GNSSDOPMode::Auto,
            ..Default::default()
        }
    );
    assert!(iface.latest_position().unwrap().hdop.is_none());
    assert!(iface.latest_position().unwrap().vdop.is_none());

    let mut position_reserved = standard_position_detail_frame();
    position_reserved[34..36].copy_from_slice(&0xFFFDu16.to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_DATA,
        position_reserved,
        0x24,
    ));
    assert!(
        positions.borrow().is_empty(),
        "reserved position-detail DOP fields must not emit a GNSS position"
    );

    let mut position_unavailable = standard_position_detail_frame();
    position_unavailable[34..36].copy_from_slice(&0xFFFFu16.to_le_bytes());
    position_unavailable[36..38].copy_from_slice(&0xFFFFu16.to_le_bytes());
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_DATA,
        position_unavailable,
        0x24,
    ));
    assert_eq!(positions.borrow().len(), 1);
    let position = positions.borrow()[0];
    assert_eq!(position.hdop, None);
    assert_eq!(position.pdop, None);

    iface.handle_message(&Message::new(PGN_GNSS_DOPS, valid_dops.to_vec(), 0x24));
    assert_eq!(dops.borrow().len(), 2);
    assert_eq!(iface.latest_position().unwrap().hdop, Some(1.0));
    assert_eq!(iface.latest_position().unwrap().vdop, Some(1.5));
}

#[test]
fn nmea2000_position_rapid_rejects_reserved_coordinate_values_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let valid = NMEAInterface::build_position(&Default::default());
    for range in [0..4, 4..8] {
        let mut reserved = valid;
        reserved[range].copy_from_slice(&((i32::MAX - 2) as u32).to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_GNSS_POSITION_RAPID,
            reserved.to_vec(),
            0x24,
        ));
    }
    assert!(positions.borrow().is_empty());
    assert!(iface.latest_position().is_none());

    iface.handle_message(&Message::new(PGN_GNSS_POSITION_RAPID, valid.to_vec(), 0x24));
    assert_eq!(positions.borrow().len(), 1);
    assert!(iface.latest_position().is_some());
}

#[test]
fn nmea2000_position_rapid_rejects_out_of_range_coordinates_before_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<_>>> = Rc::new(RefCell::new(Vec::new()));
    let position_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| position_log.borrow_mut().push(*pos));

    let mut valid = NMEAInterface::build_position(&Default::default());
    valid[0..4].copy_from_slice(&900_000_000i32.to_le_bytes());
    valid[4..8].copy_from_slice(&1_800_000_000i32.to_le_bytes());
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_RAPID, valid.to_vec(), 0x24));
    assert_eq!(positions.borrow().len(), 1);

    let cached = iface.latest_position().unwrap();
    for (range, raw) in [
        (0..4, 900_000_001i32),
        (0..4, -900_000_001i32),
        (4..8, 1_800_000_001i32),
        (4..8, -1_800_000_001i32),
    ] {
        let mut out_of_range = valid;
        out_of_range[range].copy_from_slice(&raw.to_le_bytes());
        iface.handle_message(&Message::new(
            PGN_GNSS_POSITION_RAPID,
            out_of_range.to_vec(),
            0x24,
        ));
    }

    assert_eq!(
        positions.borrow().len(),
        1,
        "out-of-range rapid coordinates must not emit additional position events"
    );
    assert_eq!(
        iface.latest_position().unwrap(),
        cached,
        "out-of-range rapid coordinates must not replace cached GNSS position"
    );

    let clamped = NMEAInterface::build_position(&GNSSPosition {
        wgs: Wgs::new(91.0, -181.0, 0.0),
        ..Default::default()
    });
    assert_eq!(
        i32::from_le_bytes(clamped[0..4].try_into().unwrap()),
        900_000_000,
        "rapid-position builder must not emit latitude raw values outside the supported range"
    );
    assert_eq!(
        i32::from_le_bytes(clamped[4..8].try_into().unwrap()),
        -1_800_000_000,
        "rapid-position builder must not emit longitude raw values outside the supported range"
    );
}

#[test]
fn nmea2000_position_delta_rejects_signed_reserved_values_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_gnss_navigation(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));
    let before = iface.latest_position().unwrap();
    let deltas: Rc<RefCell<Vec<PositionDeltaHighPrecisionRapidUpdateData>>> =
        Rc::new(RefCell::new(Vec::new()));
    let delta_log = deltas.clone();
    iface
        .on_position_delta
        .subscribe(move |delta| delta_log.borrow_mut().push(*delta));

    let valid = NMEAInterface::build_position_delta(&PositionDeltaHighPrecisionRapidUpdateData {
        sid: 3,
        time_delta_s: 0.25,
        latitude_delta_deg: 0.000_010,
        longitude_delta_deg: -0.000_020,
    });
    for range in [2..5, 5..8] {
        let mut reserved_delta = valid;
        reserved_delta[range].copy_from_slice(&[0xFD, 0xFF, 0x7F]);
        iface.handle_message(&Message::new(
            PGN_GNSS_POSITION_DELTA,
            reserved_delta.to_vec(),
            0x24,
        ));
    }
    for reserved_time in [0xFD, 0xFE, 0xFF] {
        let mut reserved_delta = valid;
        reserved_delta[1] = reserved_time;
        iface.handle_message(&Message::new(
            PGN_GNSS_POSITION_DELTA,
            reserved_delta.to_vec(),
            0x24,
        ));
    }

    assert!(
        deltas.borrow().is_empty(),
        "reserved signed 24-bit delta values must not emit position-delta events"
    );
    assert_eq!(
        iface.latest_position().unwrap().wgs,
        before.wgs,
        "reserved deltas must not mutate the cached position"
    );

    let oversized =
        NMEAInterface::build_position_delta(&PositionDeltaHighPrecisionRapidUpdateData {
            sid: 4,
            time_delta_s: f64::MAX,
            latitude_delta_deg: f64::MAX,
            longitude_delta_deg: f64::MAX,
        });
    assert_ne!(
        oversized[1], 0xFD,
        "builder must not encode finite oversized delta time as a reserved special value"
    );
    assert_ne!(
        &oversized[2..5],
        &[0xFD, 0xFF, 0x7F],
        "builder must not encode finite oversized deltas as reserved special values"
    );
    assert_ne!(
        &oversized[5..8],
        &[0xFD, 0xFF, 0x7F],
        "builder must not encode finite oversized deltas as reserved special values"
    );

    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DELTA, valid.to_vec(), 0x24));
    assert_eq!(deltas.borrow().len(), 1);
    assert_ne!(iface.latest_position().unwrap().wgs, before.wgs);
}

#[test]
fn nmea2000_gnss_dops_mode_byte_rejects_reserved_values_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_gnss_navigation(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));
    let dops: Rc<RefCell<Vec<GNSSDOPData>>> = Rc::new(RefCell::new(Vec::new()));
    let dops_log = dops.clone();
    iface
        .on_gnss_dops
        .subscribe(move |dop| dops_log.borrow_mut().push(*dop));

    let mut valid = [0xFFu8; 8];
    valid[0] = 7;
    valid[1] = 0x1B;
    valid[2..4].copy_from_slice(&100u16.to_le_bytes());
    valid[4..6].copy_from_slice(&150u16.to_le_bytes());
    valid[6..8].copy_from_slice(&200u16.to_le_bytes());

    for reserved_mode_byte in [
        valid[1] | 0x40,
        valid[1] | 0x80,
        (valid[1] & !0x07) | 0x04,
        (valid[1] & !0x07) | 0x05,
        (valid[1] & !0x38) | (0x04 << 3),
        (valid[1] & !0x38) | (0x05 << 3),
    ] {
        let mut reserved = valid;
        reserved[1] = reserved_mode_byte;
        iface.handle_message(&Message::new(PGN_GNSS_DOPS, reserved.to_vec(), 0x24));
    }

    assert!(
        dops.borrow().is_empty(),
        "reserved DOP mode bits and values must not emit a DOP event"
    );
    let cached = iface.latest_position().unwrap();
    assert!(cached.hdop.is_none());
    assert!(cached.vdop.is_none());

    iface.handle_message(&Message::new(PGN_GNSS_DOPS, valid.to_vec(), 0x24));
    assert_eq!(dops.borrow().len(), 1);
    assert_eq!(iface.latest_position().unwrap().hdop, Some(1.0));
    assert_eq!(iface.latest_position().unwrap().vdop, Some(1.5));
}

#[test]
fn nmea2000_attitude_rejects_unavailable_axes_before_event_or_cache_update() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_gnss_navigation(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));
    let attitudes: Rc<RefCell<Vec<(f64, f64, f64)>>> = Rc::new(RefCell::new(Vec::new()));
    let attitude_log = attitudes.clone();
    iface
        .on_attitude
        .subscribe(move |attitude| attitude_log.borrow_mut().push(*attitude));

    let mut unavailable_axis = [0xFFu8; 8];
    unavailable_axis[1..3].copy_from_slice(&i16::MAX.to_le_bytes());
    unavailable_axis[3..5].copy_from_slice(&100i16.to_le_bytes());
    unavailable_axis[5..7].copy_from_slice(&(-100i16).to_le_bytes());
    iface.handle_message(&Message::new(PGN_ATTITUDE, unavailable_axis.to_vec(), 0x24));
    let cached = iface.latest_position().unwrap();
    assert!(cached.heading_rad.is_none());
    assert!(cached.pitch_rad.is_none());
    assert!(cached.roll_rad.is_none());
    assert!(attitudes.borrow().is_empty());

    let mut valid = [0xFFu8; 8];
    valid[1..3].copy_from_slice(&100i16.to_le_bytes());
    valid[3..5].copy_from_slice(&(-50i16).to_le_bytes());
    valid[5..7].copy_from_slice(&25i16.to_le_bytes());
    iface.handle_message(&Message::new(PGN_ATTITUDE, valid.to_vec(), 0x24));
    assert_eq!(attitudes.borrow().len(), 1);
    let cached = iface.latest_position().unwrap();
    assert!((cached.heading_rad.unwrap() - 100.0 * HEADING_RESOLUTION).abs() < 1e-12);
    assert!((cached.pitch_rad.unwrap() + 50.0 * HEADING_RESOLUTION).abs() < 1e-12);
    assert!((cached.roll_rad.unwrap() - 25.0 * HEADING_RESOLUTION).abs() < 1e-12);
}

#[test]
fn nmea2000_signed_navigation_fields_reject_reserved_special_values_before_events_or_cache_update()
{
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));

    let attitudes: Rc<RefCell<Vec<(f64, f64, f64)>>> = Rc::new(RefCell::new(Vec::new()));
    let variations: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let rudders: Rc<RefCell<Vec<RudderData>>> = Rc::new(RefCell::new(Vec::new()));
    let attitude_log = attitudes.clone();
    let variation_log = variations.clone();
    let rudder_log = rudders.clone();
    iface
        .on_attitude
        .subscribe(move |attitude| attitude_log.borrow_mut().push(*attitude));
    iface
        .on_magnetic_variation
        .subscribe(move |variation| variation_log.borrow_mut().push(*variation));
    iface
        .on_rudder
        .subscribe(move |rudder| rudder_log.borrow_mut().push(*rudder));

    let reserved_i16 = i16::MAX - 2;
    let mut attitude = [0xFFu8; 8];
    attitude[1..3].copy_from_slice(&reserved_i16.to_le_bytes());
    attitude[3..5].copy_from_slice(&10i16.to_le_bytes());
    attitude[5..7].copy_from_slice(&20i16.to_le_bytes());
    iface.handle_message(&Message::new(PGN_ATTITUDE, attitude.to_vec(), 0x24));

    let mut variation = NMEAInterface::build_magnetic_variation(0.1, 5);
    variation[4..6].copy_from_slice(&reserved_i16.to_le_bytes());
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
    rudder[2..4].copy_from_slice(&reserved_i16.to_le_bytes());
    iface.handle_message(&Message::new(PGN_RUDDER, rudder.to_vec(), 0x24));

    let mut rot = [0xFFu8; 8];
    rot[1..5].copy_from_slice(&((i32::MAX - 2) as u32).to_le_bytes());
    iface.handle_message(&Message::new(PGN_RATE_OF_TURN, rot.to_vec(), 0x24));

    let cached = iface.latest_position().unwrap();
    assert!(cached.heading_rad.is_none());
    assert!(cached.pitch_rad.is_none());
    assert!(cached.roll_rad.is_none());
    assert!(cached.rate_of_turn_rps.is_none());
    assert!(attitudes.borrow().is_empty());
    assert!(variations.borrow().is_empty());
    assert!(rudders.borrow().is_empty());

    let mut valid_attitude = [0xFFu8; 8];
    valid_attitude[1..3].copy_from_slice(&10i16.to_le_bytes());
    valid_attitude[3..5].copy_from_slice(&20i16.to_le_bytes());
    valid_attitude[5..7].copy_from_slice(&30i16.to_le_bytes());
    iface.handle_message(&Message::new(PGN_ATTITUDE, valid_attitude.to_vec(), 0x24));
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
    rot[1..5].copy_from_slice(&100i32.to_le_bytes());
    iface.handle_message(&Message::new(PGN_RATE_OF_TURN, rot.to_vec(), 0x24));

    assert_eq!(attitudes.borrow().len(), 1);
    assert_eq!(variations.borrow().len(), 1);
    assert_eq!(rudders.borrow().len(), 1);
    assert!(iface.latest_position().unwrap().rate_of_turn_rps.is_some());
}

#[test]
fn nmea2000_navigation_auxiliary_frames_reject_short_or_noncanonical_tail_before_mutation() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        NMEAInterface::build_position(&Default::default()).to_vec(),
        0x24,
    ));

    let variations: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let rudders: Rc<RefCell<Vec<RudderData>>> = Rc::new(RefCell::new(Vec::new()));
    let variation_log = variations.clone();
    let rudder_log = rudders.clone();
    iface
        .on_magnetic_variation
        .subscribe(move |variation| variation_log.borrow_mut().push(*variation));
    iface
        .on_rudder
        .subscribe(move |rudder| rudder_log.borrow_mut().push(*rudder));

    let mut rate_of_turn = [0xFFu8; 8];
    rate_of_turn[1..5].copy_from_slice(&100i32.to_le_bytes());
    let mut bad_rate_tail = rate_of_turn;
    bad_rate_tail[5] = 0x00;
    iface.handle_message(&Message::new(
        PGN_RATE_OF_TURN,
        bad_rate_tail.to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_RATE_OF_TURN,
        rate_of_turn[..7].to_vec(),
        0x24,
    ));
    assert!(
        iface.latest_position().unwrap().rate_of_turn_rps.is_none(),
        "short or non-canonical Rate of Turn frames must not update the cached navigation state"
    );

    let magnetic_variation = NMEAInterface::build_magnetic_variation(0.1, 5);
    let mut bad_variation_tail = magnetic_variation;
    bad_variation_tail[6] = 0x00;
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        bad_variation_tail.to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        magnetic_variation[..7].to_vec(),
        0x24,
    ));
    assert!(
        variations.borrow().is_empty(),
        "short or non-canonical Magnetic Variation frames must not emit events"
    );

    let rudder = NMEAInterface::build_rudder(&RudderData {
        instance: 2,
        direction: RudderDirection::Port,
        angle_order_rad: -0.1,
        position_rad: 0.25,
    });
    let mut bad_rudder_tail = rudder;
    bad_rudder_tail[6] = 0x00;
    iface.handle_message(&Message::new(PGN_RUDDER, bad_rudder_tail.to_vec(), 0x24));
    iface.handle_message(&Message::new(PGN_RUDDER, rudder[..7].to_vec(), 0x24));
    assert!(
        rudders.borrow().is_empty(),
        "short or non-canonical Rudder frames must not emit events"
    );

    iface.handle_message(&Message::new(PGN_RATE_OF_TURN, rate_of_turn.to_vec(), 0x24));
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        magnetic_variation.to_vec(),
        0x24,
    ));
    iface.handle_message(&Message::new(PGN_RUDDER, rudder.to_vec(), 0x24));

    assert!(iface.latest_position().unwrap().rate_of_turn_rps.is_some());
    assert_eq!(variations.borrow().len(), 1);
    assert_eq!(rudders.borrow().len(), 1);
}

#[test]
fn nmea2000_water_depth_rejects_reserved_range_values_before_event() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let depths: Rc<RefCell<Vec<WaterDepthData>>> = Rc::new(RefCell::new(Vec::new()));
    let depth_log = depths.clone();
    iface
        .on_depth
        .subscribe(move |depth| depth_log.borrow_mut().push(*depth));

    for reserved in [0xFD, 0xFE] {
        let mut reserved_range = NMEAInterface::build_depth(&WaterDepthData {
            sid: 9,
            depth_m: 3.21,
            offset_m: -0.25,
            range_m: 120.0,
        });
        reserved_range[7] = reserved;
        iface.handle_message(&Message::new(
            PGN_WATER_DEPTH,
            reserved_range.to_vec(),
            0x24,
        ));
    }
    assert!(depths.borrow().is_empty());

    let valid_upper_edge = NMEAInterface::build_depth(&WaterDepthData {
        sid: 9,
        depth_m: 3.21,
        offset_m: -0.25,
        range_m: 2_520.0,
    });
    assert_eq!(valid_upper_edge[7], 0xFC);
    iface.handle_message(&Message::new(
        PGN_WATER_DEPTH,
        valid_upper_edge.to_vec(),
        0x24,
    ));
    assert_eq!(depths.borrow().len(), 1);
    assert!((depths.borrow()[0].range_m - 2_520.0).abs() < 0.1);
}

