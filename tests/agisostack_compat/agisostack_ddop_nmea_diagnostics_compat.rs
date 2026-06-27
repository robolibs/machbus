#[test]
fn tc_ddop_helper_flat_no_functions_matches_agisostack_example() {
    assert_eq!(
        DDOPHelpers::extract_geometry(&DDOP::default())
            .sections
            .len(),
        0
    );

    let device_only = DDOP::default().with_element(
        DeviceElement::default()
            .with_id(1)
            .with_type(DeviceElementType::Device)
            .with_designator("Device"),
    );
    assert_eq!(
        DDOPHelpers::extract_geometry(&device_only).sections.len(),
        0
    );

    let ddop = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(0)
                .with_designator("TEST")
                .with_software_version("123")
                .with_serial_number("1234567")
                .with_structure_label([1, 2, 3, 4, 5, 6, 7]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(4)
                .with_type(DeviceElementType::Section)
                .with_number(0)
                .with_parent(1)
                .with_designator("Section1")
                .with_children([6, 7, 8, 9]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(5)
                .with_type(DeviceElementType::Section)
                .with_number(1)
                .with_parent(1)
                .with_designator("Section2")
                .with_children([6, 7, 8, 10]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(45)
                .with_type(DeviceElementType::Bin)
                .with_number(2)
                .with_parent(1)
                .with_designator("Product")
                .with_children([46, 47, 48, 49]),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(6)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_X)
                .with_value(2000)
                .with_designator("Xoffset"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(7)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Y)
                .with_value(3000)
                .with_designator("yoffset"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(8)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Z)
                .with_value(4000)
                .with_designator("zoffset"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(9)
                .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                .with_value(5000)
                .with_designator("width1"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(10)
                .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                .with_value(6000)
                .with_designator("width2"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(46)
                .with_ddi(ddi::SETPOINT_MASS_PER_AREA_APPLICATION_RATE)
                .with_value(7000)
                .with_designator("Rate Setpoint"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(47)
                .with_ddi(ddi::DEFAULT_MASS_PER_AREA_APPLICATION_RATE)
                .with_value(8000)
                .with_designator("Rate Default"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(48)
                .with_ddi(ddi::MAXIMUM_VOLUME_PER_MASS_APPLICATION_RATE)
                .with_value(9000)
                .with_designator("Rate Max"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(49)
                .with_ddi(ddi::MINIMUM_VOLUME_PER_MASS_APPLICATION_RATE)
                .with_value(0)
                .with_designator("Rate Min"),
        );

    let geometry = DDOPHelpers::extract_geometry(&ddop);
    assert_eq!(DDOPHelpers::section_count(&ddop), 2);
    assert_eq!(geometry.sections.len(), 2);
    assert_eq!(geometry.sections[0].element_id, ObjectID(4));
    assert_eq!(geometry.sections[0].number.raw(), 0);
    assert_eq!(geometry.sections[0].width_mm, 5000);
    assert_eq!(geometry.sections[0].offset_x_mm, 2000);
    assert_eq!(geometry.sections[0].offset_y_mm, 3000);
    assert_eq!(geometry.sections[1].element_id, ObjectID(5));
    assert_eq!(geometry.sections[1].number.raw(), 1);
    assert_eq!(geometry.sections[1].width_mm, 6000);
    assert_eq!(geometry.sections[1].offset_x_mm, 2000);
    assert_eq!(geometry.sections[1].offset_y_mm, 3000);
    assert_eq!(geometry.total_width_mm, 11000);

    let rates = DDOPHelpers::extract_rates(&ddop);
    assert_eq!(rates.len(), 4);
    assert_eq!(rates[0].process_data_id, ObjectID(46));
    assert_eq!(
        rates[0].ddi,
        DDI(ddi::SETPOINT_MASS_PER_AREA_APPLICATION_RATE)
    );
    assert_eq!(rates[0].value, Some(7000));
    assert!(!rates[0].editable);
    assert_eq!(rates[1].process_data_id, ObjectID(47));
    assert_eq!(
        rates[1].ddi,
        DDI(ddi::DEFAULT_MASS_PER_AREA_APPLICATION_RATE)
    );
    assert_eq!(rates[1].value, Some(8000));
    assert!(!rates[1].editable);
    assert_eq!(rates[2].process_data_id, ObjectID(48));
    assert_eq!(
        rates[2].ddi,
        DDI(ddi::MAXIMUM_VOLUME_PER_MASS_APPLICATION_RATE)
    );
    assert_eq!(rates[2].value, Some(9000));
    assert!(!rates[2].editable);
    assert_eq!(rates[3].process_data_id, ObjectID(49));
    assert_eq!(
        rates[3].ddi,
        DDI(ddi::MINIMUM_VOLUME_PER_MASS_APPLICATION_RATE)
    );
    assert_eq!(rates[3].value, Some(0));
    assert!(!rates[3].editable);
}

// AgIsoStack `tc_server_tests.cpp::DDOPHelper_SubBooms` builds a main boom
// with two function-typed sub-booms, one section under each sub-boom, and a
// bin-hosted rate under SubBoom1. Rust exposes this as `sub_booms` on the
// extracted geometry.

#[test]
fn tc_ddop_helper_sub_booms_match_agisostack_example() {
    let ddop = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(0)
                .with_designator("TEST")
                .with_software_version("123")
                .with_serial_number("1234567")
                .with_structure_label([1, 2, 3, 4, 5, 6, 7]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(1)
                .with_type(DeviceElementType::Device)
                .with_number(0)
                .with_parent(0)
                .with_designator("Device"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(11)
                .with_type(DeviceElementType::Function)
                .with_number(0)
                .with_parent(1)
                .with_designator("MainBoom"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Function)
                .with_number(0)
                .with_parent(11)
                .with_designator("SubBoom1")
                .with_children([12, 13, 40]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(3)
                .with_type(DeviceElementType::Function)
                .with_number(0)
                .with_parent(11)
                .with_designator("SubBoom2"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(4)
                .with_type(DeviceElementType::Section)
                .with_number(0)
                .with_parent(2)
                .with_designator("Section1")
                .with_children([6, 7, 8, 9]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(5)
                .with_type(DeviceElementType::Section)
                .with_number(0)
                .with_parent(3)
                .with_designator("Section2")
                .with_children([14, 7, 8, 10]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(40)
                .with_type(DeviceElementType::Bin)
                .with_number(0)
                .with_parent(2)
                .with_designator("SubBoomProduct")
                .with_children([41]),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(6)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_X)
                .with_value(2000)
                .with_designator("Xoffset"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(7)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Y)
                .with_value(3000)
                .with_designator("yoffset"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(8)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Z)
                .with_value(4000)
                .with_designator("zoffset"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(9)
                .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                .with_value(5000)
                .with_designator("width1"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(10)
                .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                .with_value(6000)
                .with_designator("width2"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(12)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Z)
                .with_value(7000)
                .with_designator("SBzoffset"),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(13)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_X)
                .with_designator("SBxoffset"),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(14)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_X)
                .with_designator("secTestDPD"),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(41)
                .with_ddi(ddi::ACTUAL_APPLICATION_RATE_OF_PHOSPHOR)
                .with_designator("SBRate"),
        );

    let geometry = DDOPHelpers::extract_geometry(&ddop);
    assert!(geometry.sections.is_empty());
    assert_eq!(geometry.sub_booms.len(), 2);
    assert_eq!(geometry.sub_booms[0].sections.len(), 1);
    assert_eq!(geometry.sub_booms[1].sections.len(), 1);
    assert_eq!(geometry.sub_booms[0].rates.len(), 1);
    assert_eq!(geometry.sub_booms[0].offset_x, None);
    assert_eq!(geometry.sub_booms[0].offset_y, None);
    assert_eq!(geometry.sub_booms[0].offset_z, Some(7000));

    let section1 = &geometry.sub_booms[0].sections[0];
    assert_eq!(section1.width, Some(5000));
    assert_eq!(section1.offset_x, Some(2000));
    assert_eq!(section1.offset_y, Some(3000));
    assert_eq!(section1.offset_z, Some(4000));

    let section2 = &geometry.sub_booms[1].sections[0];
    assert_eq!(section2.width, Some(6000));
    assert_eq!(section2.offset_x, None);
    assert_eq!(section2.offset_y, Some(3000));
    assert_eq!(section2.offset_z, Some(4000));

    let rate = &geometry.sub_booms[0].rates[0];
    assert_eq!(rate.process_data_id, ObjectID(41));
    assert_eq!(rate.ddi, DDI(ddi::ACTUAL_APPLICATION_RATE_OF_PHOSPHOR));
    assert_eq!(rate.value, None);
    assert!(rate.editable);
}

// ─── ISO 11783-11 DDI database lookup facade ─────────────────────────
//
// AgIsoStack `isobus_data_dictionary_tests.cpp::DDI_Lookups` checks two
// concrete DDIs plus the invalid-DDI sentinel returned by
// `DataDictionary::get_entry`.

#[test]
fn ddi_database_entries_match_agisostack_lookup_examples() {
    let actual_net_weight = DataDictionary::get_entry(229);
    assert_eq!(actual_net_weight.ddi, 229);
    assert_eq!(actual_net_weight.name, "Actual Net Weight");
    assert_eq!(actual_net_weight.resolution, 1.0);
    assert_eq!(actual_net_weight.unit_symbol, "g");
    assert_eq!(actual_net_weight.unit_description, "Mass large");
    assert!((actual_net_weight.display_range.0 - f64::from(i32::MIN)).abs() < 0.001);
    assert!((actual_net_weight.display_range.1 - f64::from(i32::MAX)).abs() < 0.001);

    let default_crop_grade_length = DataDictionary::get_entry(40962);
    assert_eq!(default_crop_grade_length.ddi, 40962);
    assert_eq!(default_crop_grade_length.name, "Default Crop Grade Length");
    assert!((default_crop_grade_length.resolution - 0.001).abs() < 0.001);
    assert_eq!(default_crop_grade_length.unit_symbol, "mm");
    assert_eq!(default_crop_grade_length.unit_description, "Length");
    assert!((default_crop_grade_length.display_range.0 - 0.0).abs() < 0.001);
    assert!((default_crop_grade_length.display_range.1 - f64::from(i32::MAX)).abs() < 0.001);

    let invalid = DataDictionary::get_entry(1957);
    assert_eq!(invalid.ddi, 65535);
    assert_eq!(invalid.name, "Unknown");
    assert_eq!(invalid.resolution, 0.0);
    assert_eq!(invalid.unit_symbol, "Unknown");
    assert_eq!(invalid.unit_description, "Unknown");
    assert_eq!(invalid.display_range, (0.0, 0.0));
}

// ─── ISO 11783-12 Control Function Functionalities layout ────────────
//
// AgIsoStack `cf_functionalities_tests.cpp::CFFunctionalitiesTest` checks
// that PGN 0xFC8E payloads start with fixed byte 0xFF, then a functionality
// count, then per-functionality `(id, generation, option-byte-count, options)`
// blocks. The Rust encoder additionally canonicalizes zero option-byte tails
// before padding single-frame payloads with 0xFF.

#[test]
fn control_functionalities_layout_matches_agisostack_message_examples() {
    let mut cf = Functionalities::new();
    assert_eq!(
        cf.serialize(),
        [0xFF, 1, 0, 1, 0, 0xFF, 0xFF, 0xFF],
        "Minimum CF only, no options"
    );

    cf.set_functionality_supported(Functionality::UniversalTerminalWorkingSet, 1, true);
    assert_eq!(
        cf.serialize(),
        [0xFF, 2, 0, 1, 0, 2, 1, 0],
        "Minimum CF plus UT Working Set"
    );

    cf.set_functionality_supported(Functionality::AuxNFunctions, 1, true);
    assert_eq!(
        cf.serialize(),
        [0xFF, 3, 0, 1, 0, 2, 1, 0, 6, 1, 0],
        "Minimum CF plus UT Working Set plus AUX-N functions"
    );

    cf.set_functionality_supported(Functionality::TaskControllerSectionControlClient, 1, true);
    cf.tc_sc_client_booms = 1;
    cf.tc_sc_client_sections = 255;
    let sc_client = cf.serialize();
    assert_eq!(
        sc_client,
        [0xFF, 4, 0, 1, 0, 2, 1, 0, 6, 1, 0, 12, 1, 2, 1, 255],
        "Minimum CF plus UT Working Set plus AUX-N functions plus TC-SC client"
    );
    assert_eq!(Functionalities::decode(&sc_client).unwrap().len(), 4);
}

// ─── NMEA2000 Fast Packet wrapping ───────────────────────────────────
//
// AgIsoStack `nmea2000_message_tests.cpp` verifies that Datum PGN 0x1F814
// and GNSS Position Data PGN 0x1F805 are carried as Fast Packet streams with
// byte 0 = sequence/counter, byte 1 = total length on the first frame, six
// bytes in the first frame, then seven bytes per continuation frame.

#[test]
fn nmea2000_fast_packet_wrapping_matches_agisostack_interface_examples() {
    let mut tx = FastPacketProtocol::new();
    let datum_payload: Vec<u8> = (0..20).collect();
    let datum_frames = tx.send(0x1F814, &datum_payload, 0x52).unwrap();
    assert_eq!(datum_frames.len(), 3);
    assert_eq!(datum_frames[0].id.raw, 0x19F8_1452);
    assert_eq!(datum_frames[0].data, [0x00, 0x14, 0, 1, 2, 3, 4, 5]);
    assert_eq!(datum_frames[1].data, [0x01, 6, 7, 8, 9, 10, 11, 12]);
    assert_eq!(datum_frames[2].data, [0x02, 13, 14, 15, 16, 17, 18, 19]);

    let mut rx = FastPacketProtocol::new();
    let mut completed = None;
    for frame in &datum_frames {
        completed = rx.process_frame(frame).or(completed);
    }
    let completed = completed.expect("datum fast-packet stream completes");
    assert_eq!(completed.pgn, 0x1F814);
    assert_eq!(completed.source, 0x52);
    assert_eq!(completed.data, datum_payload);

    let mut tx = FastPacketProtocol::new();
    let gnss_payload: Vec<u8> = (0..47).collect();
    let gnss_frames = tx
        .send(PGN_GNSS_POSITION_DATA, &gnss_payload, 0x52)
        .unwrap();
    assert_eq!(gnss_frames.len(), 7);
    assert_eq!(gnss_frames[0].id.raw, 0x19F8_0552);
    assert_eq!(gnss_frames[0].data, [0x00, 0x2F, 0, 1, 2, 3, 4, 5]);
    assert_eq!(gnss_frames[1].data, [0x01, 6, 7, 8, 9, 10, 11, 12]);
    assert_eq!(gnss_frames[2].data, [0x02, 13, 14, 15, 16, 17, 18, 19]);
    assert_eq!(gnss_frames[6].data, [0x06, 41, 42, 43, 44, 45, 46, 0xFF]);

    let mut rx = FastPacketProtocol::new();
    let mut completed = None;
    for frame in &gnss_frames {
        completed = rx.process_frame(frame).or(completed);
    }
    let completed = completed.expect("GNSS fast-packet stream completes");
    assert_eq!(completed.pgn, PGN_GNSS_POSITION_DATA);
    assert_eq!(completed.source, 0x52);
    assert_eq!(completed.data, gnss_payload);
}

// ─── NMEA2000 navigation raw scaling/layout ──────────────────────────
//
// AgIsoStack `nmea2000_message_tests.cpp` exercises the same raw values below:
// PositionRapidUpdate latitude=1000 longitude=2000, COG/SOG raw 50/75,
// PositionDeltaHighPrecisionRapidUpdate raw time/lat/lon 7/-5000/-9000,
// RateOfTurn raw 100 with SID 200, and VesselHeading raw heading/deviation/
// variation 1/2/-3 with SID 4.

#[test]
fn nmea2000_navigation_layout_matches_agisostack_raw_examples() {
    let mut iface = NMEAInterface::new(NMEAConfig::default().with_gnss_navigation(true));

    let position = [0xE8, 0x03, 0, 0, 0xD0, 0x07, 0, 0];
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        position.to_vec(),
        0x23,
    ));
    let cached = iface
        .latest_position()
        .expect("position rapid updates cache");
    assert!((cached.wgs.latitude - 1000.0 * LAT_LON_RESOLUTION).abs() < 1e-12);
    assert!((cached.wgs.longitude - 2000.0 * LAT_LON_RESOLUTION).abs() < 1e-12);

    let cog_sog = [9, 1, 50, 0, 75, 0, 0xFF, 0xFF];
    iface.handle_message(&Message::new(
        PGN_GNSS_COG_SOG_RAPID,
        cog_sog.to_vec(),
        0x23,
    ));
    let cached = iface.latest_position().unwrap();
    assert!((cached.cog_rad.unwrap() - 50.0 * COG_RESOLUTION).abs() < 1e-12);
    assert!((cached.speed_mps.unwrap() - 75.0 * SPEED_RESOLUTION).abs() < 1e-12);

    let deltas = Rc::new(RefCell::new(Vec::new()));
    let deltas_sink = deltas.clone();
    iface
        .on_position_delta
        .subscribe(move |delta| deltas_sink.borrow_mut().push(*delta));

    let delta = PositionDeltaHighPrecisionRapidUpdateData {
        sid: 49,
        time_delta_s: 7.0 * POSITION_DELTA_TIME_RESOLUTION,
        latitude_delta_deg: -5000.0 * POSITION_DELTA_RESOLUTION,
        longitude_delta_deg: -9000.0 * POSITION_DELTA_RESOLUTION,
    };
    assert_eq!(
        NMEAInterface::build_position_delta(&delta),
        [49, 7, 0x78, 0xEC, 0xFF, 0xD8, 0xDC, 0xFF]
    );
    let before_delta = iface.latest_position().unwrap();
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_DELTA,
        vec![49, 7, 0x78, 0xEC, 0xFF, 0xD8, 0xDC, 0xFF],
        0x23,
    ));
    let delta_events = deltas.borrow();
    assert_eq!(delta_events.len(), 1);
    assert_eq!(delta_events[0].sid, delta.sid);
    assert!((delta_events[0].time_delta_s - delta.time_delta_s).abs() < 1e-12);
    assert!((delta_events[0].latitude_delta_deg - delta.latitude_delta_deg).abs() < 1e-12);
    assert!((delta_events[0].longitude_delta_deg - delta.longitude_delta_deg).abs() < 1e-12);
    drop(delta_events);
    let after_delta = iface.latest_position().unwrap();
    assert!(
        (after_delta.wgs.latitude - (before_delta.wgs.latitude + delta.latitude_delta_deg)).abs()
            < 1e-12
    );
    assert!(
        (after_delta.wgs.longitude - (before_delta.wgs.longitude + delta.longitude_delta_deg))
            .abs()
            < 1e-12
    );

    let rate = [200, 100, 0, 0, 0, 0xFF, 0xFF, 0xFF];
    iface.handle_message(&Message::new(PGN_RATE_OF_TURN, rate.to_vec(), 0x23));
    let cached = iface.latest_position().unwrap();
    assert!((cached.rate_of_turn_rps.unwrap() - 100.0 * ROT_RESOLUTION).abs() < 1e-12);
    // AgIsoStack's test uses a broad 0.0005 tolerance for the public
    // engineering getter; the raw byte layout is the interoperability anchor.
    assert!((cached.rate_of_turn_rps.unwrap() - 100.0 * ((1.0 / 32.0) * 10E-6)).abs() < 0.0005);

    let heading = [4, 1, 0, 2, 0, 0xFD, 0xFF, 0];
    iface.handle_message(&Message::new(PGN_HEADING_TRACK, heading.to_vec(), 0x23));
    let cached = iface.latest_position().unwrap();
    assert!((cached.heading_rad.unwrap() - HEADING_RESOLUTION).abs() < 1e-12);
}

// ─── ISO 11783-7 heartbeat INIT sentinel ──────────────────────────────
//
// AgIsoStack `heartbeat_tests.cpp::HeartBeat` asserts the first byte
// of the first heartbeat after `request_heartbeat` is `251`. ISO
// 11783-7 §8.3 calls this the "initial value" sentinel; ours is at
// `hb_seq::INIT`.

#[test]
fn heartbeat_init_sentinel_matches_iso_spec() {
    // AgIsoStack: EXPECT_EQ(testFrame.data[0], 251);
    assert_eq!(hb_seq::INIT, 251);
}

#[test]
fn heartbeat_sender_emits_init_then_zero() {
    // AgIsoStack `HeartBeat` test: first emission is INIT, the next
    // (after the 100 ms cadence) is 0.
    use machbus::j1939::HeartbeatSender;
    let mut s = HeartbeatSender::default();
    let first = s.next_sequence();
    let second = s.next_sequence();
    assert_eq!(first, hb_seq::INIT);
    assert_eq!(second, 0);
}

#[test]
fn heartbeat_request_frame_matches_agisostack_reference_bytes() {
    // AgIsoStack `heartbeat_tests.cpp::HeartBeat` sends a request from
    // source 0x41 to partner 0xF4 as raw identifier 0x18CCF441:
    // requested PGN 61668 (0xF0E4), interval 100 ms, FF reserved tail.
    let request = HeartbeatRequest::for_heartbeat(100);
    let payload = request.encode().unwrap();
    assert_eq!(payload, [0xE4, 0xF0, 0x00, 0x64, 0x00, 0xFF, 0xFF, 0xFF]);
    assert_eq!(HeartbeatRequest::decode(&payload), Some(request));

    let request_frame = Frame::from_message(
        Priority::Default,
        PGN_HEARTBEAT_REQUEST,
        0x41,
        0xF4,
        &payload,
    );
    assert_eq!(request_frame.id.raw, 0x18CC_F441);

    let mut sender = machbus::j1939::HeartbeatSender::default();
    let first_heartbeat = [sender.next_sequence()];
    let first_frame = Frame::from_message(
        Priority::Normal,
        PGN_HEARTBEAT,
        0x41,
        0xFF,
        &first_heartbeat,
    );
    assert_eq!(first_frame.id.raw, 0x0CF0_E441);
    assert_eq!(first_frame.payload(), &[hb_seq::INIT]);
}

// ─── Time/Date PGN 0xFEE6 wire decode ─────────────────────────────────
//
// AgIsoStack `time_date_tests.cpp::ReceivingMessages` sends the
// 8-byte payload below and asserts the decoded fields. Our `TimeDate`
// must produce the same field values; otherwise we'd disagree with
// any TECU broadcasting time on the wire.

#[test]
fn time_date_decodes_agisostack_reference_payload() {
    let payload = [0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78];
    let msg = Message::new(PGN_TIME_DATE, payload.to_vec(), 0x47);
    let td = TimeDate::decode(&msg).expect("decode succeeds");

    // AgIsoStack expectations:
    assert_eq!(td.year, Some(2023));
    assert_eq!(td.month, Some(8));
    assert_eq!(td.day, Some(7));
    assert_eq!(td.hours, Some(22));
    assert_eq!(td.minutes, Some(49));
    assert_eq!(td.seconds, Some(41));
    // Local hour offset −5 (Eastern Standard Time), minute offset 0.
    assert_eq!(td.utc_offset_hours, Some(-5));
    assert_eq!(td.utc_offset_min, Some(0));
}

#[test]
fn time_date_round_trips_through_encode_decode() {
    // Build the same payload from fields, encode, decode → same fields.
    let original = TimeDate {
        seconds: Some(41),
        minutes: Some(49),
        hours: Some(22),
        day: Some(7),
        month: Some(8),
        year: Some(2023),
        utc_offset_min: Some(0),
        utc_offset_hours: Some(-5),
        timestamp_us: 0,
    };
    let bytes = original.encode();
    // AgIsoStack reference bytes:
    assert_eq!(bytes, [0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78]);
    let msg = Message::new(PGN_TIME_DATE, bytes.to_vec(), 0x47);
    let decoded = TimeDate::decode(&msg).unwrap();
    assert_eq!(decoded.year, original.year);
    assert_eq!(decoded.month, original.month);
    assert_eq!(decoded.day, original.day);
    assert_eq!(decoded.hours, original.hours);
    assert_eq!(decoded.minutes, original.minutes);
    assert_eq!(decoded.seconds, original.seconds);
    assert_eq!(decoded.utc_offset_hours, original.utc_offset_hours);
    assert_eq!(decoded.utc_offset_min, original.utc_offset_min);
}

// ─── J1939-73 DTC byte packing ────────────────────────────────────────
//
// AgIsoStack `test/diagnostic_protocol_tests.cpp` uses these DTCs while
// validating DM1/DM2 emission:
//   SPN 1234 / FMI ConditionExists
//   SPN 567 / FMI DataErratic
//   SPN 8910 / FMI BadIntelligentDevice
// The byte assertions below mirror the upstream expected SPN/FMI bytes.

#[test]
fn diagnostic_dtc_packing_matches_agisostack_reference_dtcs() {
    let dtc1 = Dtc {
        spn: 1234,
        fmi: Fmi::ConditionExists,
        occurrence_count: 0,
    };
    assert_eq!(dtc1.encode(), [0xD2, 0x04, 31, 0x00]);
    assert_eq!(Dtc::decode(&dtc1.encode()), Some(dtc1));

    let dtc2 = Dtc {
        spn: 567,
        fmi: Fmi::Erratic,
        occurrence_count: 0,
    };
    assert_eq!(dtc2.encode(), [0x37, 0x02, 2, 0x00]);
    assert_eq!(Dtc::decode(&dtc2.encode()), Some(dtc2));

    let dtc3 = Dtc {
        spn: 8910,
        fmi: Fmi::BadDevice,
        occurrence_count: 0,
    };
    assert_eq!(dtc3.encode(), [0xCE, 0x22, 12, 0x00]);
    assert_eq!(Dtc::decode(&dtc3.encode()), Some(dtc3));
}

#[test]
fn diagnostic_lamp_bytes_cover_agisostack_dm1_examples() {
    let all_off = DiagnosticLamps::default();
    assert_eq!(all_off.encode(), [0x00, 0xAA]);

    let amber_slow_flash = DiagnosticLamps {
        amber_warning: LampStatus::On,
        amber_warning_flash: LampFlash::SlowFlash,
        ..DiagnosticLamps::default()
    };
    assert_eq!(amber_slow_flash.encode(), [0x10, 0x8A]);

    let red_stop_solid = DiagnosticLamps {
        red_stop: LampStatus::On,
        ..DiagnosticLamps::default()
    };
    assert_eq!(red_stop_solid.encode(), [0x04, 0xAA]);
}

#[test]
fn diagnostic_dm22_layout_matches_agisostack_clear_examples() {
    // AgIsoStack `diagnostic_protocol_tests.cpp::MessageEncoding` sends DM22
    // requests/responses with the control byte in byte 0, the NACK reason in
    // byte 1 only for negative responses, reserved bytes 2..4, and the target
    // DTC SPN/FMI in bytes 5..7.
    let active_request = Dm22Message {
        control: Dm22Control::ClearActive,
        nack_reason: None,
        spn: 1234,
        fmi: Fmi::ConditionExists,
    };
    assert_eq!(
        active_request.encode(),
        [0x11, 0xFF, 0xFF, 0xFF, 0xFF, 0xD2, 0x04, 31]
    );
    assert_eq!(
        Dm22Message::decode(&active_request.encode()),
        Some(active_request)
    );

    let active_ack = Dm22Message {
        control: Dm22Control::AckClearActive,
        nack_reason: None,
        spn: 1234,
        fmi: Fmi::ConditionExists,
    };
    assert_eq!(
        active_ack.encode(),
        [0x12, 0xFF, 0xFF, 0xFF, 0xFF, 0xD2, 0x04, 31]
    );

    let active_nack = Dm22Message {
        control: Dm22Control::NackClearActive,
        nack_reason: Some(Dm22NackReason::DtcNoLongerActive),
        spn: 1234,
        fmi: Fmi::ConditionExists,
    };
    assert_eq!(
        active_nack.encode(),
        [0x13, 0x04, 0xFF, 0xFF, 0xFF, 0xD2, 0x04, 31]
    );

    let previous_request = Dm22Message {
        control: Dm22Control::ClearPreviouslyActive,
        nack_reason: None,
        spn: 1234,
        fmi: Fmi::ConditionExists,
    };
    assert_eq!(
        previous_request.encode(),
        [0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xD2, 0x04, 31]
    );

    let previous_ack = Dm22Message {
        control: Dm22Control::AckClearPreviouslyActive,
        nack_reason: None,
        spn: 1234,
        fmi: Fmi::ConditionExists,
    };
    assert_eq!(
        previous_ack.encode(),
        [0x02, 0xFF, 0xFF, 0xFF, 0xFF, 0xD2, 0x04, 31]
    );

    let previous_nack = Dm22Message {
        control: Dm22Control::NackClearPreviouslyActive,
        nack_reason: Some(Dm22NackReason::DtcNoLongerPrevious),
        spn: 1234,
        fmi: Fmi::ConditionExists,
    };
    assert_eq!(
        previous_nack.encode(),
        [0x03, 0x03, 0xFF, 0xFF, 0xFF, 0xD2, 0x04, 31]
    );
    assert_eq!(
        Dm22Message::decode(&previous_nack.encode()),
        Some(previous_nack)
    );

    let mut invalid_non_nack_reason = active_request.encode();
    invalid_non_nack_reason[1] = Dm22NackReason::UnknownDtc.as_u8();
    assert!(Dm22Message::decode(&invalid_non_nack_reason).is_none());

    let mut invalid_reserved = active_request.encode();
    invalid_reserved[4] = 0;
    assert!(Dm22Message::decode(&invalid_reserved).is_none());
}

#[test]
fn diagnostic_dm13_layout_matches_agisostack_broadcast_control_examples() {
    // AgIsoStack `diagnostic_protocol_tests.cpp::MessageEncoding` uses these
    // DM13 byte layouts for local suspend announcements and for received
    // stop/start commands on J1939 Network 1 and the current data link.
    let announce = Dm13Signals {
        suspend_duration_s: 5,
        ..Dm13Signals::default()
    };
    assert_eq!(
        announce.encode(),
        [0xFF, 0xFF, 0xFF, 0xFF, 0x05, 0x00, 0xFF, 0xFF]
    );
    assert_eq!(Dm13Signals::decode(&announce.encode()), Some(announce));

    let primary_stop = Dm13Signals {
        primary_vehicle_network: Dm13Command::SuspendBroadcast,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::DoNotCare,
        suspend_signal: Dm13SuspendSignal::PartialTemporarySuspension,
        suspend_duration_s: 10,
    };
    assert_eq!(
        primary_stop.encode(),
        [0xFC, 0xFF, 0xFF, 0x03, 0x0A, 0x00, 0xFF, 0xFF]
    );

    let primary_resume = Dm13Signals {
        primary_vehicle_network: Dm13Command::ResumeBroadcast,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::DoNotCare,
        suspend_signal: Dm13SuspendSignal::IndefiniteSuspension,
        suspend_duration_s: 0xFFFF,
    };
    assert_eq!(
        primary_resume.encode(),
        [0xFD, 0xFF, 0xFF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]
    );

    let current_link_stop = Dm13Signals {
        primary_vehicle_network: Dm13Command::DoNotCare,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::SuspendBroadcast,
        suspend_signal: Dm13SuspendSignal::IndefiniteSuspension,
        suspend_duration_s: 10,
    };
    assert_eq!(
        current_link_stop.encode(),
        [0x3F, 0xFF, 0xFF, 0x00, 0x0A, 0x00, 0xFF, 0xFF]
    );

    let current_link_resume = Dm13Signals {
        primary_vehicle_network: Dm13Command::DoNotCare,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::ResumeBroadcast,
        suspend_signal: Dm13SuspendSignal::IndefiniteSuspension,
        suspend_duration_s: 0xFFFF,
    };
    assert_eq!(
        current_link_resume.encode(),
        [0x7F, 0xFF, 0xFF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]
    );
}

#[test]
fn diagnostic_identification_strings_match_agisostack_examples() {
    // AgIsoStack `diagnostic_protocol_tests.cpp::MessageEncoding` builds
    // Product Identification as product-code*, brand*, model*.
    let product = ProductIdentification {
        make: "1234567890ABC".into(),
        model: "Open-Agriculture".into(),
        serial_number: "AgIsoStack++".into(),
    };
    let product_payload = b"1234567890ABC*Open-Agriculture*AgIsoStack++*";
    assert_eq!(product.encode().unwrap(), product_payload);
    assert_eq!(
        ProductIdentification::decode(product_payload),
        Some(product)
    );

    // The same AgIsoStack test sends Software Identification as plain
    // star-delimited version strings; there is no leading count byte.
    let software = SoftwareIdentification {
        versions: vec!["Unit Test 1.0.0".into(), "Another version x.x.x.x".into()],
    };
    let software_payload = b"Unit Test 1.0.0*Another version x.x.x.x*";
    assert_eq!(software.encode().unwrap(), software_payload);
    assert_eq!(
        SoftwareIdentification::decode(software_payload),
        Some(software)
    );
    assert!(SoftwareIdentification::decode(b"\x02Unit Test 1.0.0*").is_none());
}

#[test]
fn ecu_identification_strings_match_agisostack_iso_and_j1939_examples() {
    // AgIsoStack `diagnostic_protocol_tests.cpp::MessageEncoding` configures
    // six ECU Identification fields in ISO mode and omits HardwareID in J1939
    // mode. Both forms are plain star-delimited strings.
    let ecu = EcuIdentification {
        ecu_part_number: "1234".into(),
        ecu_serial_number: "9876".into(),
        ecu_location: "The Internet".into(),
        ecu_type: "AgISOStack".into(),
        ecu_manufacturer: "None".into(),
        ecu_hardware_id: Some("Some Hardware ID".into()),
    };
    let iso_payload = b"1234*9876*The Internet*AgISOStack*None*Some Hardware ID*";
    assert_eq!(ecu.encode_iso11783().unwrap(), iso_payload);
    assert_eq!(EcuIdentification::decode(iso_payload), Some(ecu.clone()));

    let j1939_payload = b"1234*9876*The Internet*AgISOStack*None*";
    assert_eq!(ecu.encode_j1939().unwrap(), j1939_payload);
    assert_eq!(
        EcuIdentification::decode(j1939_payload),
        Some(EcuIdentification {
            ecu_hardware_id: None,
            ..ecu
        })
    );
}
