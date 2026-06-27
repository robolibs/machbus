use machbus::isobus::functionalities::{
    Functionalities, Functionality, FunctionalityData, MinimumControlFunctionOptions,
};
use machbus::j1939::{
    DiagProtocol, DiagnosticLamps, DiagnosticProtocolId, Dm4Message, Dm7Command, Dm8TestResult,
    Dm9VehicleIdentificationRequest, Dm10VehicleIdentification, Dm13Command, Dm13Signals,
    Dm13SuspendSignal, Dm14Command, Dm14PointerType, Dm14Request, Dm15Response, Dm15Status,
    Dm16Transfer, Dm20Response, Dm21Readiness, Dm22Control, Dm22Message, Dm22NackReason,
    Dm25Request, DmClearAllRequest, DmDtcList, Dtc, EcuIdentification, Fmi, FreezeFrame, LampFlash,
    LampStatus, MonitorPerformanceRatio, ProductIdentification, SoftwareIdentification,
    SpnSnapshot,
};
use machbus::net::pgn_defs::{PGN_DM14, PGN_TIME_DATE};
use machbus::net::{BROADCAST_ADDRESS, Message, NULL_ADDRESS, Priority};

#[test]
fn diagnostics_dtc_rejects_reserved_occurrence_count_bit() {
    let dtc = Dtc {
        spn: 0x12345,
        fmi: Fmi::VoltageLow,
        occurrence_count: 7,
    };
    let mut encoded = dtc.encode();
    assert_eq!(Dtc::decode(&encoded), Some(dtc));

    encoded[3] |= 0x80;
    assert_eq!(Dtc::decode(&encoded), None);
}

#[test]
fn diagnostics_fmi_decoders_accept_defined_values_and_reject_reserved_values() {
    for fmi in [
        Fmi::DataDriftedHigh,
        Fmi::DataDriftedLow,
        Fmi::ConditionExists,
    ] {
        let dtc = Dtc {
            spn: 0x12345,
            fmi,
            occurrence_count: 1,
        };
        assert_eq!(Dtc::decode(&dtc.encode()), Some(dtc));
    }

    let mut reserved_dtc = Dtc {
        spn: 0x12345,
        fmi: Fmi::ConditionExists,
        occurrence_count: 1,
    }
    .encode();
    reserved_dtc[2] = (reserved_dtc[2] & 0xE0) | 22;
    assert_eq!(Dtc::decode(&reserved_dtc), None);

    let dm22 = Dm22Message {
        control: Dm22Control::ClearActive,
        nack_reason: None,
        spn: 1234,
        fmi: Fmi::DataDriftedHigh,
    };
    assert_eq!(Dm22Message::decode(&dm22.encode()), Some(dm22));
    let mut reserved_dm22 = dm22.encode();
    reserved_dm22[7] = (reserved_dm22[7] & 0xE0) | 22;
    assert_eq!(Dm22Message::decode(&reserved_dm22), None);

    let dm25 = Dm25Request {
        spn: 0x12345,
        fmi: Fmi::DataDriftedLow,
        frame_number: 0,
    };
    assert_eq!(Dm25Request::decode(&dm25.encode()), Some(dm25));
    let mut reserved_dm25 = dm25.encode();
    reserved_dm25[3] = 22;
    assert_eq!(Dm25Request::decode(&reserved_dm25), None);
}

#[test]
fn diagnostics_lamp_status_round_trips_all_lamp_groups() {
    let lamps = DiagnosticLamps {
        malfunction: LampStatus::On,
        malfunction_flash: LampFlash::FastFlash,
        red_stop: LampStatus::Error,
        red_stop_flash: LampFlash::SlowFlash,
        amber_warning: LampStatus::NotAvailable,
        amber_warning_flash: LampFlash::Off,
        engine_protect: LampStatus::Off,
        engine_protect_flash: LampFlash::NotAvailable,
    };

    assert_eq!(DiagnosticLamps::decode(&lamps.encode()), Some(lamps));
    assert_eq!(DiagnosticLamps::decode(&[0xFF]), None);
}

#[test]
fn diagnostics_public_lamp_decoders_reject_noncanonical_packed_bytes() {
    for (raw, status) in [
        (0, LampStatus::Off),
        (1, LampStatus::On),
        (2, LampStatus::Error),
        (3, LampStatus::NotAvailable),
    ] {
        assert_eq!(LampStatus::try_from_u8(raw), Some(status));
        assert_eq!(LampStatus::from_u8(raw), status);
    }

    for (raw, flash) in [
        (0, LampFlash::SlowFlash),
        (1, LampFlash::FastFlash),
        (2, LampFlash::Off),
        (3, LampFlash::NotAvailable),
    ] {
        assert_eq!(LampFlash::try_from_u8(raw), Some(flash));
        assert_eq!(LampFlash::from_u8(raw), flash);
    }

    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(LampStatus::try_from_u8(packed_or_reserved), None);
        assert_eq!(LampFlash::try_from_u8(packed_or_reserved), None);
    }

    let lamps = DiagnosticLamps {
        malfunction: LampStatus::On,
        red_stop: LampStatus::Error,
        amber_warning: LampStatus::NotAvailable,
        engine_protect: LampStatus::Off,
        malfunction_flash: LampFlash::FastFlash,
        red_stop_flash: LampFlash::SlowFlash,
        amber_warning_flash: LampFlash::Off,
        engine_protect_flash: LampFlash::NotAvailable,
    };
    assert_eq!(DiagnosticLamps::decode(&lamps.encode()), Some(lamps));
}

#[test]
fn diagnostics_identification_readiness_and_driver_info_shapes_are_canonical() {
    let dtc = Dtc {
        spn: 0x12345,
        fmi: Fmi::VoltageHigh,
        occurrence_count: 2,
    };
    let driver_info = Dm4Message {
        mil_status: LampStatus::On,
        red_stop_lamp: LampStatus::Off,
        amber_warning: LampStatus::Error,
        protect_lamp: LampStatus::NotAvailable,
        dtcs: vec![dtc],
    };
    let mut driver_bytes = driver_info.encode();
    assert_eq!(
        driver_bytes.len(),
        8,
        "single-frame DM4 payloads must use a complete canonical frame"
    );
    assert_eq!(driver_bytes[1], 0xFF);
    assert_eq!(Dm4Message::decode(&driver_bytes), Some(driver_info));

    driver_bytes[1] = 0x00;
    assert_eq!(Dm4Message::decode(&driver_bytes), None);

    assert_eq!(
        Dm4Message::decode(&driver_bytes[..6]),
        None,
        "DM4 driver-information payloads must not accept unpadded short single frames"
    );

    let no_driver_dtcs = Dm4Message::default();
    let no_driver_dtc_bytes = no_driver_dtcs.encode();
    assert_eq!(no_driver_dtc_bytes.len(), 8);
    assert!(no_driver_dtc_bytes[2..].iter().all(|byte| *byte == 0xFF));
    assert_eq!(
        Dm4Message::decode(&no_driver_dtc_bytes),
        Some(no_driver_dtcs)
    );

    let mut misaligned = Dm4Message {
        dtcs: vec![dtc],
        ..Default::default()
    }
    .encode();
    misaligned.push(0xFF);
    assert_eq!(Dm4Message::decode(&misaligned), None);

    let mut bad_dtc_reserved = Dm4Message {
        dtcs: vec![dtc],
        ..Default::default()
    }
    .encode();
    bad_dtc_reserved[5] |= 0x80;
    assert_eq!(Dm4Message::decode(&bad_dtc_reserved), None);

    let mut hidden_driver_info_tail = Dm4Message {
        dtcs: vec![dtc],
        ..Default::default()
    }
    .encode();
    hidden_driver_info_tail[6] = 0x00;
    assert_eq!(
        Dm4Message::decode(&hidden_driver_info_tail),
        None,
        "DM4 single-frame padding after one DTC must remain canonical"
    );

    let protocols = DiagnosticProtocolId {
        protocols: DiagProtocol::J1939_73.as_u8() | DiagProtocol::Iso14229_3.as_u8(),
    };
    let protocol_bytes = protocols.encode();
    assert!(protocols.supports(DiagProtocol::J1939_73));
    assert!(protocols.supports(DiagProtocol::Iso14229_3));
    assert!(!protocols.supports(DiagProtocol::Iso14230));
    assert_eq!(
        DiagnosticProtocolId::decode(&protocol_bytes),
        Some(protocols)
    );

    let mut bad_protocol_tail = protocol_bytes;
    bad_protocol_tail[7] = 0x00;
    assert_eq!(DiagnosticProtocolId::decode(&bad_protocol_tail), None);
    assert_eq!(DiagnosticProtocolId::decode(&protocol_bytes[..7]), None);

    let dm9_payload = Dm9VehicleIdentificationRequest.encode().unwrap();
    assert_eq!(dm9_payload, [0xEC, 0xFE, 0x00]);
    assert_eq!(
        Dm9VehicleIdentificationRequest::decode(&dm9_payload),
        Some(Dm9VehicleIdentificationRequest)
    );
    assert_eq!(
        Dm9VehicleIdentificationRequest::decode(&[0x00, 0x00, 0x00]),
        None
    );

    let vehicle_id = Dm10VehicleIdentification {
        vin: "1HGBH41JXMN109186".into(),
    };
    let vehicle_bytes = vehicle_id.encode().unwrap();
    assert_eq!(vehicle_bytes.last().copied(), Some(b'*'));
    assert_eq!(
        Dm10VehicleIdentification::decode(&vehicle_bytes),
        Some(vehicle_id.clone())
    );
    assert_eq!(
        Dm10VehicleIdentification::decode(b"1HGBH41JXMN109186"),
        None
    );
    assert_eq!(
        Dm10VehicleIdentification::decode(b"1HGBH41JXMN109186*TRAILING*"),
        None
    );
    assert_eq!(Dm10VehicleIdentification::decode(b"VIN\x1F*"), None);
    assert!(
        Dm10VehicleIdentification {
            vin: "BAD*VIN".into(),
        }
        .encode()
        .is_err()
    );

    let readiness = Dm21Readiness {
        distance_with_mil_on_km: 12,
        distance_since_codes_cleared_km: 34,
        minutes_with_mil_on: 56,
        time_since_codes_cleared_min: 78,
        comprehensive_component: 0b0000_0011,
        fuel_system: 0b0000_1100,
        misfire: 0b0011_0000,
    };
    let readiness_bytes = readiness.encode();
    assert_eq!(readiness_bytes.len(), 11);
    assert_eq!(Dm21Readiness::decode(&readiness_bytes), Some(readiness));
    assert_eq!(Dm21Readiness::decode(&readiness_bytes[..10]), None);
    let mut overlong_readiness = readiness_bytes;
    overlong_readiness.push(0xFF);
    assert_eq!(Dm21Readiness::decode(&overlong_readiness), None);
}

#[test]
fn diagnostics_protocol_id_rejects_unknown_bits_and_noncanonical_padding() {
    let supported = DiagnosticProtocolId {
        protocols: DiagProtocol::J1939_73.as_u8()
            | DiagProtocol::Iso14230.as_u8()
            | DiagProtocol::Iso14229_3.as_u8(),
    };
    let supported_bytes = supported.encode();
    assert_eq!(
        DiagnosticProtocolId::decode(&supported_bytes),
        Some(supported)
    );
    assert!(supported.supports(DiagProtocol::J1939_73));
    assert!(supported.supports(DiagProtocol::Iso14230));
    assert!(supported.supports(DiagProtocol::Iso14229_3));

    for unknown_bits in [0x08, 0x10, 0x80, 0xF8] {
        let mut bad_protocols = supported_bytes;
        bad_protocols[0] |= unknown_bits;
        assert_eq!(
            DiagnosticProtocolId::decode(&bad_protocols),
            None,
            "unknown diagnostic protocol bits must not be promoted into capabilities"
        );
    }

    let mut bad_tail = supported_bytes;
    bad_tail[1] = 0x00;
    assert_eq!(DiagnosticProtocolId::decode(&bad_tail), None);
    assert_eq!(DiagnosticProtocolId::decode(&supported_bytes[..7]), None);
}

#[test]
fn diagnostics_protocol_id_public_protocol_decoder_rejects_noncanonical_bytes() {
    for (raw, protocol) in [
        (0x00, DiagProtocol::None),
        (0x01, DiagProtocol::J1939_73),
        (0x02, DiagProtocol::Iso14230),
        (0x04, DiagProtocol::Iso14229_3),
    ] {
        assert_eq!(DiagProtocol::try_from_u8(raw), Some(protocol));
    }
    for raw in [0x03, 0x05, 0x08, 0x10, 0x7F, 0x80, 0xFF] {
        assert_eq!(
            DiagProtocol::try_from_u8(raw),
            None,
            "single diagnostic protocol decoder must reject combined or unknown bit sets"
        );
    }
}

#[test]
fn diagnostics_protocol_id_encoder_masks_reserved_bits_before_wire_use() {
    let supported_bits = DiagProtocol::J1939_73.as_u8()
        | DiagProtocol::Iso14230.as_u8()
        | DiagProtocol::Iso14229_3.as_u8();
    assert_eq!(DiagnosticProtocolId::known_protocol_bits(), supported_bits);

    let local_capabilities = DiagnosticProtocolId { protocols: 0xFF };
    let encoded = local_capabilities.encode();
    assert_eq!(
        encoded[0], supported_bits,
        "outbound diagnostic protocol identification must not advertise reserved bits"
    );
    assert!(
        encoded[1..].iter().all(|byte| *byte == 0xFF),
        "outbound diagnostic protocol identification must keep fixed-frame padding canonical"
    );
    assert_eq!(
        DiagnosticProtocolId::decode(&encoded),
        Some(DiagnosticProtocolId {
            protocols: supported_bits,
        })
    );
}

#[test]
fn diagnostics_functionalities_use_canonical_generation_and_option_counts() {
    let default = Functionalities::new();
    assert_eq!(
        default.serialize(),
        [0xFF, 1, 0, 1, 0, 0xFF, 0xFF, 0xFF],
        "zero option bytes must be omitted for a no-option Minimum CF advertisement"
    );
    assert_eq!(
        Functionalities::decode(&default.serialize()).unwrap(),
        vec![FunctionalityData {
            functionality: Functionality::MinimumControlFunction,
            generation: 1,
            option_bytes: vec![],
        }]
    );

    let mut heartbeat = Functionalities::new();
    heartbeat.set_minimum_control_function_option_state(
        MinimumControlFunctionOptions::SupportOfHeartbeatProducer,
        true,
    );
    assert_eq!(
        heartbeat.serialize(),
        [0xFF, 1, 0, 1, 1, 0x04, 0xFF, 0xFF],
        "non-zero option bytes are counted and preserved"
    );

    let mut aux = Functionalities::new();
    aux.set_functionality_supported(Functionality::AuxNInputs, 1, true);
    aux.aux_n_inputs_options = 0x0001;
    let aux_bytes = aux.serialize();
    assert_eq!(
        &aux_bytes[5..],
        &[Functionality::AuxNInputs.as_u8(), 1, 1, 0x01],
        "multi-byte option groups must omit trailing zero option bytes"
    );

    assert!(
        Functionalities::decode(&[0xFF, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF]).is_err(),
        "functionality generation zero is outside the advertised generation range"
    );
    assert!(
        Functionalities::decode(&[0xFF, 1, 0, 1, 1, 0x00, 0xFF, 0xFF]).is_err(),
        "explicit trailing zero option bytes must be rejected as non-canonical"
    );
    assert!(
        Functionalities::decode(&[0xFF, 1, Functionality::AuxNInputs.as_u8(), 1, 2, 1, 0]).is_err(),
        "multi-byte option groups must not retain trailing zero option bytes"
    );
    assert!(
        Functionalities::decode(&[0xFF, 1, Functionality::FileServer.as_u8(), 1, 1, 1]).is_err(),
        "functionality rows with no option support must advertise zero option bytes"
    );
}

#[test]
fn diagnostics_functionality_public_decoder_rejects_noncanonical_bytes() {
    let valid = [
        Functionality::MinimumControlFunction,
        Functionality::UniversalTerminalServer,
        Functionality::UniversalTerminalWorkingSet,
        Functionality::AuxOInputs,
        Functionality::AuxOFunctions,
        Functionality::AuxNInputs,
        Functionality::AuxNFunctions,
        Functionality::TaskControllerBasicServer,
        Functionality::TaskControllerBasicClient,
        Functionality::TaskControllerGeoServer,
        Functionality::TaskControllerGeoClient,
        Functionality::TaskControllerSectionControlServer,
        Functionality::TaskControllerSectionControlClient,
        Functionality::BasicTractorEcuServer,
        Functionality::BasicTractorEcuImplementClient,
        Functionality::TractorImplementManagementServer,
        Functionality::TractorImplementManagementClient,
        Functionality::FileServer,
        Functionality::FileServerClient,
    ];
    for functionality in valid {
        assert_eq!(
            Functionality::try_from_u8(functionality.as_u8()),
            Some(functionality)
        );
        assert_eq!(
            Functionality::from_u8(functionality.as_u8()),
            Some(functionality)
        );
    }
    for raw in [19, 20, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(Functionality::try_from_u8(raw), None);
        assert_eq!(Functionality::from_u8(raw), None);
    }
}

#[test]
fn diagnostics_functionalities_reject_zero_count_and_preserve_minimum_cf() {
    assert!(
        Functionalities::decode(&[0xFF, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_err(),
        "a functionality advertisement must contain at least one functionality"
    );

    let mut model = Functionalities::new();
    model.set_functionality_supported(Functionality::MinimumControlFunction, 1, false);
    assert!(
        model.is_functionality_supported(Functionality::MinimumControlFunction),
        "Minimum CF support must not be removed from a conforming advertisement"
    );
    assert_eq!(
        model.serialize(),
        [0xFF, 1, 0, 1, 0, 0xFF, 0xFF, 0xFF],
        "removing Minimum CF must not create a zero-count payload"
    );

    model.set_functionality_supported(Functionality::UniversalTerminalServer, 0, true);
    assert_eq!(
        model.functionality_generation(Functionality::UniversalTerminalServer),
        1,
        "generation zero must be canonicalized before transmit"
    );
    assert!(
        Functionalities::decode(&model.serialize()).is_ok(),
        "locally serialized functionality advertisements must stay decodable"
    );
}

#[test]
fn diagnostics_product_and_software_identification_are_strict_star_fields() {
    let product = ProductIdentification {
        make: "AGRO".into(),
        model: "DX-10".into(),
        serial_number: "SN-1234".into(),
    };
    let product_bytes = product.encode().unwrap();
    assert_eq!(
        product_bytes.iter().filter(|byte| **byte == b'*').count(),
        3
    );
    assert_eq!(
        ProductIdentification::decode(&product_bytes),
        Some(product.clone())
    );
    assert_eq!(
        ProductIdentification::decode(b"AGRO*DX-10*"),
        None,
        "product identification must not accept missing fields"
    );
    assert_eq!(
        ProductIdentification::decode(b"AGRO*DX-10*SN-1234*EXTRA*"),
        None,
        "product identification must not accept trailing fields"
    );
    assert_eq!(
        ProductIdentification::decode(b"AGRO*DX-\x1F*SN-1234*"),
        None,
        "product identification must reject control bytes"
    );
    assert!(
        ProductIdentification {
            make: "AG*RO".into(),
            ..product
        }
        .encode()
        .is_err()
    );

    let software = SoftwareIdentification {
        versions: vec!["boot-1.0".into(), "app-2.1".into(), "cfg-3".into()],
    };
    let software_bytes = software.encode().unwrap();
    assert_eq!(
        software_bytes.iter().filter(|byte| **byte == b'*').count(),
        3
    );
    assert_eq!(
        SoftwareIdentification::decode(&software_bytes),
        Some(software.clone())
    );
    assert_eq!(
        SoftwareIdentification::decode(b"boot-1.0*app-2.1"),
        None,
        "software identification must be terminated at every version field"
    );
    assert_eq!(
        SoftwareIdentification::decode(b""),
        None,
        "software identification must not silently accept an absent field list"
    );
    assert_eq!(
        SoftwareIdentification::decode(b"boot-1.0*\x7F*"),
        None,
        "software identification must reject non-printable version bytes"
    );
    assert!(
        SoftwareIdentification {
            versions: vec!["bad*version".into()],
        }
        .encode()
        .is_err()
    );
}

#[test]
fn diagnostics_identification_text_fields_use_single_byte_latin1_and_reserved_delimiters() {
    let product = ProductIdentification {
        make: "AGRÉ".into(),
        model: "DX-10".into(),
        serial_number: "SN-ÿ".into(),
    };
    let product_bytes = product.encode().unwrap();
    assert!(
        product_bytes.contains(&0xC9),
        "U+00C9 must be encoded as one diagnostic text byte"
    );
    assert!(
        product_bytes.contains(&0xFF),
        "U+00FF must be encoded as one diagnostic text byte"
    );
    assert!(!product_bytes.windows(2).any(|pair| pair == [0xC3, 0x89]));
    assert_eq!(
        ProductIdentification::decode(&product_bytes),
        Some(product.clone())
    );
    assert_eq!(
        ProductIdentification::decode(b"AGR\xC9*DX-10*SN-\xFF*"),
        Some(product)
    );
    assert_eq!(
        ProductIdentification::decode(b"AGR\x80*DX-10*SN-1*"),
        None,
        "undefined single-byte text values must be rejected before field use"
    );

    let software = SoftwareIdentification {
        versions: vec!["boot-1.0".into(), "app-\u{00A0}".into()],
    };
    let software_bytes = software.encode().unwrap();
    assert!(software_bytes.contains(&0xA0));
    assert_eq!(
        SoftwareIdentification::decode(&software_bytes),
        Some(software)
    );

    let ecu = EcuIdentification {
        ecu_part_number: "PN-É".into(),
        ecu_serial_number: "SN-1".into(),
        ecu_location: "CAB-ÿ".into(),
        ecu_type: "TECU".into(),
        ecu_manufacturer: "AGRO".into(),
        ecu_hardware_id: Some("HW-\u{00A0}".into()),
    };
    let ecu_bytes = ecu.encode_iso11783().unwrap();
    assert!(ecu_bytes.contains(&0xC9));
    assert!(ecu_bytes.contains(&0xFF));
    assert!(ecu_bytes.contains(&0xA0));
    assert_eq!(EcuIdentification::decode(&ecu_bytes), Some(ecu));
    assert_eq!(
        EcuIdentification::decode(b"PN*SN*CAB*TECU*AGRO*HW#1*"),
        None,
        "hardware identifiers must reject reserved delimiter candidates"
    );
    assert!(
        EcuIdentification {
            ecu_hardware_id: Some("HW#1".into()),
            ..EcuIdentification::default()
        }
        .encode_iso11783()
        .is_err()
    );
}

#[test]
fn diagnostics_dm25_freeze_frame_request_shape_is_canonical() {
    let latest = Dm25Request {
        spn: 0x7_FFFF,
        fmi: Fmi::ConditionExists,
        frame_number: 0,
    };
    let latest_bytes = latest.encode();
    assert_eq!(Dm25Request::decode(&latest_bytes), Some(latest));

    let older = Dm25Request {
        frame_number: 0xFE,
        ..latest
    };
    assert_eq!(Dm25Request::decode(&older.encode()), Some(older));

    let mut reserved_spn_bits = latest_bytes;
    reserved_spn_bits[2] |= 0xF8;
    assert_eq!(Dm25Request::decode(&reserved_spn_bits), None);

    let mut reserved_fmi_bits = latest_bytes;
    reserved_fmi_bits[3] |= 0xE0;
    assert_eq!(Dm25Request::decode(&reserved_fmi_bits), None);

    let mut undefined_fmi = latest_bytes;
    undefined_fmi[3] = 22;
    assert_eq!(Dm25Request::decode(&undefined_fmi), None);

    let mut bad_tail = latest_bytes;
    bad_tail[5] = 0x00;
    assert_eq!(Dm25Request::decode(&bad_tail), None);
    assert_eq!(Dm25Request::decode(&latest_bytes[..7]), None);

    let mut overlong = latest_bytes.to_vec();
    overlong.push(0xFF);
    assert_eq!(Dm25Request::decode(&overlong), None);
}

#[test]
fn diagnostics_dtc_lists_reject_prefix_compatible_garbage_and_bad_padding() {
    let lamps = DiagnosticLamps {
        malfunction: LampStatus::On,
        red_stop: LampStatus::Off,
        amber_warning: LampStatus::Off,
        engine_protect: LampStatus::Off,
        ..DiagnosticLamps::default()
    };
    let dtc = Dtc {
        spn: 100,
        fmi: Fmi::AboveNormal,
        occurrence_count: 2,
    };
    let list = DmDtcList {
        lamps,
        dtcs: vec![dtc],
    };
    let encoded = list.encode();
    assert_eq!(DmDtcList::decode(&encoded), Some(list));

    let mut hidden_tail = DmDtcList {
        lamps,
        dtcs: Vec::new(),
    }
    .encode();
    hidden_tail[7] = 0x00;
    assert_eq!(DmDtcList::decode(&hidden_tail), None);

    let mut noncanonical_empty_dtc_placeholder = DmDtcList {
        lamps,
        dtcs: Vec::new(),
    }
    .encode();
    noncanonical_empty_dtc_placeholder[5] = 1;
    assert_eq!(
        DmDtcList::decode(&noncanonical_empty_dtc_placeholder),
        None,
        "single-frame DTC-list empty placeholder must not hide a nonzero occurrence count"
    );

    let mut misaligned_multi_packet = encoded.clone();
    misaligned_multi_packet.push(0xFF);
    assert_eq!(DmDtcList::decode(&misaligned_multi_packet), None);

    let mut bad_dtc_reserved_bit = encoded;
    bad_dtc_reserved_bit[5] |= 0x80;
    assert_eq!(DmDtcList::decode(&bad_dtc_reserved_bit), None);
}

#[test]
fn diagnostics_dtc_lists_allow_empty_placeholder_only_as_single_frame_empty_list() {
    let lamps = DiagnosticLamps::default();
    let dtc = Dtc {
        spn: 0x12345,
        fmi: Fmi::VoltageLow,
        occurrence_count: 1,
    };

    let empty = DmDtcList {
        lamps,
        dtcs: Vec::new(),
    }
    .encode();
    assert_eq!(
        DmDtcList::decode(&empty),
        Some(DmDtcList {
            lamps,
            dtcs: Vec::new(),
        })
    );

    let mut placeholder_before_real = Vec::new();
    placeholder_before_real.extend_from_slice(&lamps.encode());
    placeholder_before_real.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    placeholder_before_real.extend_from_slice(&dtc.encode());
    assert_eq!(
        DmDtcList::decode(&placeholder_before_real),
        None,
        "the empty DTC-list placeholder must not be mixed into a multi-DTC payload"
    );

    let mut real_before_placeholder = Vec::new();
    real_before_placeholder.extend_from_slice(&lamps.encode());
    real_before_placeholder.extend_from_slice(&dtc.encode());
    real_before_placeholder.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    assert_eq!(
        DmDtcList::decode(&real_before_placeholder),
        None,
        "the empty DTC-list placeholder must not appear after real DTC records"
    );
}

#[test]
fn diagnostics_clear_all_requests_accept_only_canonical_reserved_payload() {
    let bytes = DmClearAllRequest.encode();
    assert_eq!(bytes, [0xFF; 8]);
    assert_eq!(DmClearAllRequest::decode(&bytes), Some(DmClearAllRequest));
    assert_eq!(DmClearAllRequest::decode(&bytes[..7]), None);

    let mut non_canonical = bytes;
    non_canonical[3] = 0x00;
    assert_eq!(DmClearAllRequest::decode(&non_canonical), None);
}

#[test]
fn diagnostics_dm7_and_dm14_reject_reserved_fields_before_service_workflow_use() {
    let command = Dm7Command {
        spn: 0x12_345,
        test_id: 0x42,
    };
    let mut dm7 = command.encode();
    assert_eq!(Dm7Command::decode(&dm7), Some(command));
    dm7[2] |= 0x80;
    assert_eq!(Dm7Command::decode(&dm7), None);

    let request = Dm14Request {
        command: Dm14Command::Write,
        pointer_type: Dm14PointerType::DirectVirtual,
        address: 0x12_3456,
        length: 8,
        key: 0xAA,
    };
    let encoded = request.encode().unwrap();
    assert_eq!(Dm14Request::decode(&encoded), Some(request));
    assert_eq!(
        Dm14Request::from_message(&Message::new(PGN_DM14, encoded.to_vec(), 0xA5)),
        Some(request)
    );
    assert_eq!(
        Dm14Request::from_message(&Message::new(PGN_TIME_DATE, encoded.to_vec(), 0xA5)),
        None
    );
    assert_eq!(
        Dm14Request::from_message(&Message::new(PGN_DM14, encoded.to_vec(), NULL_ADDRESS)),
        None
    );
    assert_eq!(
        Dm14Request::from_message(&Message::new(PGN_DM14, encoded.to_vec(), BROADCAST_ADDRESS,)),
        None
    );
    assert_eq!(
        Dm14Request::from_message(&Message::with_addressing(
            PGN_DM14,
            encoded.to_vec(),
            0xA5,
            NULL_ADDRESS,
            Priority::Default,
        )),
        None,
        "DM14 full-message helper must reject null destination metadata before service workflow use"
    );

    let mut reserved_control = encoded;
    reserved_control[0] |= 0b0000_1000;
    assert_eq!(Dm14Request::decode(&reserved_control), None);

    let mut reserved_command = encoded;
    reserved_command[0] = 0b0000_0110;
    assert_eq!(Dm14Request::decode(&reserved_command), None);

    let mut reserved_tail = encoded;
    reserved_tail[7] = 0x00;
    assert_eq!(Dm14Request::decode(&reserved_tail), None);

    assert!(
        Dm14Request {
            address: 0x0100_0000,
            ..request
        }
        .encode()
        .is_err()
    );
}

#[test]
fn diagnostics_memory_access_commands_and_dm16_lengths_are_explicit() {
    let commands = [
        Dm14Command::Read,
        Dm14Command::Write,
        Dm14Command::StatusRequest,
        Dm14Command::Erase,
        Dm14Command::BootLoad,
        Dm14Command::EdcpGeneration,
    ];
    let pointer_types = [
        Dm14PointerType::DirectPhysical,
        Dm14PointerType::DirectVirtual,
        Dm14PointerType::Indirect,
        Dm14PointerType::NotAvailable,
    ];

    for (command_index, command) in commands.into_iter().enumerate() {
        for pointer_type in pointer_types {
            let request = Dm14Request {
                command,
                pointer_type,
                address: 0xFF_FFFF - command_index as u32,
                length: 0xFFFF - command_index as u16,
                key: 0xA0 | command.as_u8(),
            };
            let encoded = request.encode().unwrap();
            assert_eq!(
                encoded[0],
                command.as_u8() | (pointer_type.as_u8() << 4),
                "DM14 control byte must carry only the command and pointer-type fields"
            );
            assert_eq!(encoded[7], 0xFF);
            assert_eq!(Dm14Request::decode(&encoded), Some(request));
        }
    }

    assert_eq!(Dm14Command::try_from_u8(6), None);
    assert_eq!(Dm14Command::try_from_u8(7), None);

    let exact_single_frame = Dm16Transfer {
        num_bytes: 7,
        data: vec![1, 2, 3, 4, 5, 6, 7],
    };
    let encoded_single_frame = exact_single_frame.encode().unwrap();
    assert_eq!(encoded_single_frame, [7, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(
        Dm16Transfer::decode(&encoded_single_frame),
        Some(exact_single_frame)
    );

    assert!(
        Dm16Transfer {
            num_bytes: 8,
            data: vec![0; 8],
        }
        .encode()
        .is_err()
    );
    assert!(
        Dm16Transfer {
            num_bytes: 4,
            data: vec![0xAA, 0xBB, 0xCC],
        }
        .encode()
        .is_err()
    );

    assert_eq!(
        Dm16Transfer::decode(&[9, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        Some(Dm16Transfer {
            num_bytes: 9,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
        })
    );
    assert_eq!(
        Dm16Transfer::decode(&[9, 1, 2, 3, 4, 5, 6, 7, 8]),
        None,
        "reassembled DM16 payload length must match the declared transfer length"
    );
}

#[test]
fn diagnostics_dm14_command_public_decoder_rejects_noncanonical_bytes() {
    for (raw, command) in [
        (0, Dm14Command::Read),
        (1, Dm14Command::Write),
        (2, Dm14Command::StatusRequest),
        (3, Dm14Command::Erase),
        (4, Dm14Command::BootLoad),
        (5, Dm14Command::EdcpGeneration),
    ] {
        assert_eq!(Dm14Command::try_from_u8(raw), Some(command));
        assert_eq!(Dm14Command::from_u8(raw), command);
    }
    for raw in [6, 7, 8, 0x10, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(
            Dm14Command::try_from_u8(raw),
            None,
            "DM14 command public strict decoder must reject reserved or packed bytes"
        );
    }
}

#[test]
fn diagnostics_dm15_status_public_decoder_rejects_noncanonical_bytes() {
    for (raw, status) in [
        (0, Dm15Status::Proceed),
        (1, Dm15Status::Busy),
        (2, Dm15Status::Completed),
        (3, Dm15Status::Error),
        (4, Dm15Status::EdcpFault),
    ] {
        assert_eq!(Dm15Status::try_from_u8(raw), Some(status));
        assert_eq!(Dm15Status::from_u8(raw), status);
    }
    for raw in [5, 6, 7, 8, 0x10, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(
            Dm15Status::try_from_u8(raw),
            None,
            "DM15 status public strict decoder must reject reserved or packed bytes"
        );
    }
}

#[test]
fn diagnostics_dm8_test_result_rejects_reserved_spn_octet_bits() {
    let result = Dm8TestResult {
        spn: 0x7_FFFF,
        test_id: 0x42,
        test_result: 1,
        test_value: 0x1234,
        test_limit_min: 0x0102,
        test_limit_max: 0xFEFE,
    };
    let mut encoded = result.encode();
    assert_eq!(Dm8TestResult::decode(&encoded), Some(result));

    encoded[2] |= 0xF8;
    assert_eq!(Dm8TestResult::decode(&encoded), None);
}

#[test]
fn diagnostics_dm15_memory_response_rejects_reserved_status_bits_and_values() {
    let response = Dm15Response {
        status: Dm15Status::EdcpFault,
        length: 0x1234,
        address: 0xFF_FFFF,
        edcp_extension: 0xAA,
        seed: 0x55,
    };
    let mut encoded = response.encode().unwrap();
    assert_eq!(Dm15Response::decode(&encoded), Some(response));

    encoded[0] |= 0xF8;
    assert_eq!(Dm15Response::decode(&encoded), None);

    let mut reserved_status = response.encode().unwrap();
    reserved_status[0] = 0x05;
    assert_eq!(Dm15Response::decode(&reserved_status), None);

    assert!(
        Dm15Response {
            address: 0x0100_0000,
            ..response
        }
        .encode()
        .is_err()
    );
}

#[test]
fn diagnostics_dm13_broadcast_control_rejects_reserved_payload_bytes() {
    let suspend = Dm13Signals {
        primary_vehicle_network: Dm13Command::DoNotCare,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::SuspendBroadcast,
        suspend_signal: Dm13SuspendSignal::TemporarySuspension,
        suspend_duration_s: 5,
    };
    let encoded = suspend.encode();
    assert_eq!(Dm13Signals::decode(&encoded), Some(suspend));

    for index in [1usize, 2, 6, 7] {
        let mut bad_tail = encoded;
        bad_tail[index] = 0x00;
        assert_eq!(Dm13Signals::decode(&bad_tail), None);
    }

    let mut reserved_signal = encoded;
    reserved_signal[3] = 5;
    assert_eq!(Dm13Signals::decode(&reserved_signal), None);
}

#[test]
fn diagnostics_dm13_suspend_signal_rejects_high_nibble_slop() {
    let suspend = Dm13Signals {
        primary_vehicle_network: Dm13Command::SuspendBroadcast,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::DoNotCare,
        suspend_signal: Dm13SuspendSignal::TemporarySuspension,
        suspend_duration_s: 30,
    };
    let encoded = suspend.encode();
    assert_eq!(Dm13Signals::decode(&encoded), Some(suspend));

    for high_nibble in [0x10, 0x70, 0xF0] {
        let mut noncanonical_signal = encoded;
        noncanonical_signal[3] |= high_nibble;
        assert_eq!(
            Dm13Signals::decode(&noncanonical_signal),
            None,
            "DM13 suspend-signal byte must not hide high-nibble data"
        );
    }

    let not_available = Dm13Signals::default().encode();
    assert_eq!(
        Dm13Signals::decode(&not_available),
        Some(Dm13Signals::default()),
        "the all-ones not-available signal remains the canonical exception"
    );
}

#[test]
fn diagnostics_dm13_rejects_undefined_network_commands_before_workflow_use() {
    let suspend = Dm13Signals {
        primary_vehicle_network: Dm13Command::DoNotCare,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::SuspendBroadcast,
        suspend_signal: Dm13SuspendSignal::TemporarySuspension,
        suspend_duration_s: 5,
    };
    let encoded = suspend.encode();
    assert_eq!(Dm13Signals::decode(&encoded), Some(suspend));

    for shift in [0, 2, 4, 6] {
        let mut undefined_command = encoded;
        undefined_command[0] &= !(0x03 << shift);
        undefined_command[0] |= 0x02 << shift;
        assert_eq!(
            Dm13Signals::decode(&undefined_command),
            None,
            "undefined two-bit DM13 command values must be rejected before diagnostic workflow use"
        );
    }
}

#[test]
fn diagnostics_dm13_public_command_decoder_rejects_noncanonical_packed_bytes() {
    assert_eq!(
        Dm13Command::try_from_u8(0),
        Some(Dm13Command::SuspendBroadcast)
    );
    assert_eq!(
        Dm13Command::try_from_u8(1),
        Some(Dm13Command::ResumeBroadcast)
    );
    assert_eq!(Dm13Command::try_from_u8(2), None);
    assert_eq!(Dm13Command::try_from_u8(3), Some(Dm13Command::DoNotCare));
    assert_eq!(Dm13Command::try_from_u8(0xFC | 1), None);

    let suspend = Dm13Signals {
        primary_vehicle_network: Dm13Command::SuspendBroadcast,
        sae_j1922_network: Dm13Command::ResumeBroadcast,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::DoNotCare,
        suspend_signal: Dm13SuspendSignal::TemporarySuspension,
        suspend_duration_s: 5,
    };
    assert_eq!(Dm13Signals::decode(&suspend.encode()), Some(suspend));
}

#[test]
fn diagnostics_public_service_field_decoders_reject_noncanonical_packed_bytes() {
    for (raw, signal) in [
        (0, Dm13SuspendSignal::IndefiniteSuspension),
        (1, Dm13SuspendSignal::PartialIndefiniteSuspension),
        (2, Dm13SuspendSignal::TemporarySuspension),
        (3, Dm13SuspendSignal::PartialTemporarySuspension),
        (4, Dm13SuspendSignal::Resuming),
        (15, Dm13SuspendSignal::NotAvailable),
        (0xFF, Dm13SuspendSignal::NotAvailable),
    ] {
        assert_eq!(Dm13SuspendSignal::try_from_u8(raw), Some(signal));
        assert_eq!(Dm13SuspendSignal::from_u8(raw), Some(signal));
    }
    for packed_or_reserved in [0x05, 0x10, 0x14, 0x20, 0xF2, 0xFE] {
        assert_eq!(
            Dm13SuspendSignal::try_from_u8(packed_or_reserved),
            None,
            "strict public DM13 suspend-signal decoder must reject packed or reserved bytes"
        );
        assert_eq!(
            Dm13SuspendSignal::from_u8(packed_or_reserved),
            None,
            "strict public DM13 suspend-signal decoder must reject packed or reserved bytes"
        );
    }

    for (raw, control) in [
        (0x01, Dm22Control::ClearPreviouslyActive),
        (0x02, Dm22Control::AckClearPreviouslyActive),
        (0x03, Dm22Control::NackClearPreviouslyActive),
        (0x11, Dm22Control::ClearActive),
        (0x12, Dm22Control::AckClearActive),
        (0x13, Dm22Control::NackClearActive),
    ] {
        assert_eq!(Dm22Control::try_from_u8(raw), Some(control));
        assert_eq!(Dm22Control::from_u8(raw), Some(control));
    }
    for raw in [0x00, 0x04, 0x10, 0x14, 0x20, 0xFF] {
        assert_eq!(Dm22Control::try_from_u8(raw), None);
        assert_eq!(Dm22Control::from_u8(raw), None);
    }

    for (raw, reason) in [
        (0x00, Dm22NackReason::GeneralNack),
        (0x01, Dm22NackReason::AccessDenied),
        (0x02, Dm22NackReason::UnknownDtc),
        (0x03, Dm22NackReason::DtcNoLongerPrevious),
        (0x04, Dm22NackReason::DtcNoLongerActive),
    ] {
        assert_eq!(Dm22NackReason::try_from_u8(raw), Some(reason));
        assert_eq!(Dm22NackReason::from_u8(raw), Some(reason));
    }
    for raw in [0x05, 0x10, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(Dm22NackReason::try_from_u8(raw), None);
        assert_eq!(Dm22NackReason::from_u8(raw), None);
    }

    for (raw, pointer) in [
        (0, Dm14PointerType::DirectPhysical),
        (1, Dm14PointerType::DirectVirtual),
        (2, Dm14PointerType::Indirect),
        (3, Dm14PointerType::NotAvailable),
    ] {
        assert_eq!(Dm14PointerType::try_from_u8(raw), Some(pointer));
        assert_eq!(Dm14PointerType::from_u8(raw), pointer);
    }
    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(
            Dm14PointerType::try_from_u8(packed_or_reserved),
            None,
            "strict public DM14 pointer decoder must reject packed bytes"
        );
    }

    let request = Dm14Request {
        command: Dm14Command::Read,
        pointer_type: Dm14PointerType::NotAvailable,
        address: 0x12_3456,
        length: 8,
        key: 0xFF,
    };
    assert_eq!(
        Dm14Request::decode(&request.encode().unwrap()),
        Some(request)
    );
}

#[test]
fn diagnostics_dm22_clear_messages_distinguish_request_ack_and_nack_shapes() {
    let clear_active = Dm22Message {
        control: Dm22Control::ClearActive,
        nack_reason: None,
        spn: 1234,
        fmi: Fmi::AboveNormal,
    };
    let encoded_clear = clear_active.encode();
    assert_eq!(Dm22Message::decode(&encoded_clear), Some(clear_active));

    let mut non_nack_with_reason = encoded_clear;
    non_nack_with_reason[1] = Dm22NackReason::UnknownDtc.as_u8();
    assert_eq!(Dm22Message::decode(&non_nack_with_reason), None);

    let nack = Dm22Message {
        control: Dm22Control::NackClearActive,
        nack_reason: Some(Dm22NackReason::DtcNoLongerActive),
        spn: 1234,
        fmi: Fmi::AboveNormal,
    };
    let encoded_nack = nack.encode();
    assert_eq!(Dm22Message::decode(&encoded_nack), Some(nack));

    let mut unknown_nack_reason = encoded_nack;
    unknown_nack_reason[1] = 0x7F;
    assert_eq!(Dm22Message::decode(&unknown_nack_reason), None);

    let mut reserved_middle = encoded_clear;
    reserved_middle[2] = 0x00;
    assert_eq!(Dm22Message::decode(&reserved_middle), None);

    let mut unknown_control = encoded_clear;
    unknown_control[0] = 0x99;
    assert_eq!(Dm22Message::decode(&unknown_control), None);
}

#[test]
fn diagnostics_dm16_and_ecu_identification_reject_malformed_variable_payloads() {
    let single = Dm16Transfer {
        num_bytes: 3,
        data: vec![0xAA, 0xBB, 0xCC],
    };
    let encoded = single.encode().unwrap();
    assert_eq!(Dm16Transfer::decode(&encoded), Some(single));

    let mut hidden_tail = encoded;
    hidden_tail[4] = 0x00;
    assert_eq!(Dm16Transfer::decode(&hidden_tail), None);

    assert_eq!(
        Dm16Transfer::decode(&[8, 1, 2, 3, 4, 5, 6, 7]),
        None,
        "declared multi-frame length must match the reassembled payload length"
    );
    assert_eq!(
        Dm16Transfer::decode(&[8, 1, 2, 3, 4, 5, 6, 7, 8]),
        Some(Dm16Transfer {
            num_bytes: 8,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        })
    );

    let identification = EcuIdentification {
        ecu_part_number: "PN-1".into(),
        ecu_serial_number: "SN-1".into(),
        ecu_location: "CAB".into(),
        ecu_type: "TECU".into(),
        ecu_manufacturer: "AGRO".into(),
        ecu_hardware_id: Some("HW-1".into()),
    };
    let iso_bytes = identification.encode_iso11783().unwrap();
    assert_eq!(iso_bytes.iter().filter(|byte| **byte == b'*').count(), 6);
    assert_eq!(
        EcuIdentification::decode(&iso_bytes),
        Some(identification.clone())
    );
    assert_eq!(
        EcuIdentification::decode(b"PN*SN*CAB*TECU*AGRO*HW*EXTRA*"),
        None
    );
    assert_eq!(EcuIdentification::decode(b"PN*SN*CAB*TECU*AGRO\x1F*"), None);

    assert!(
        EcuIdentification {
            ecu_part_number: "BAD*PN".into(),
            ..identification
        }
        .encode_iso11783()
        .is_err()
    );
}

#[test]
fn diagnostics_dm20_monitor_ratios_require_complete_groups_and_reserved_spn_bits() {
    let ratio = MonitorPerformanceRatio {
        spn: 0x1_2345,
        numerator: 25,
        denominator: 100,
    };
    let response = Dm20Response {
        ignition_cycles: 3,
        obd_monitoring_conditions_met: 4,
        ratios: vec![ratio],
    };
    let encoded = response.encode();
    assert_eq!(Dm20Response::decode(&encoded), Some(response));

    let empty = Dm20Response::default().encode();
    assert_eq!(Dm20Response::decode(&empty), Some(Dm20Response::default()));
    let mut hidden_single_frame_ratio = empty;
    hidden_single_frame_ratio[2] = 0x00;
    assert_eq!(
        Dm20Response::decode(&hidden_single_frame_ratio),
        None,
        "single-frame DM20 without complete ratio groups must keep the unused bytes canonical"
    );

    let mut reserved_spn_bits = encoded.clone();
    reserved_spn_bits[4] |= 0xF8;
    assert_eq!(
        Dm20Response::decode(&reserved_spn_bits),
        None,
        "DM20 monitor-ratio SPN fields must reject reserved high bits"
    );

    let mut partial_ratio = encoded;
    partial_ratio.push(0xFF);
    assert_eq!(
        Dm20Response::decode(&partial_ratio),
        None,
        "multi-packet DM20 payloads must contain only complete ratio records"
    );
}

#[test]
fn diagnostics_freeze_frame_rejects_count_mismatch_and_reserved_snapshot_bits() {
    let frame = FreezeFrame {
        dtc: Dtc {
            spn: 0x1234,
            fmi: Fmi::AboveNormal,
            occurrence_count: 1,
        },
        timestamp_ms: 42,
        snapshots: vec![SpnSnapshot {
            spn: 0x2345,
            value: 0xDEAD_BEEF,
        }],
    };
    let encoded = frame.encode().unwrap();
    assert_eq!(FreezeFrame::decode(&encoded), Some(frame));

    let mut reserved_snapshot_bits = encoded.clone();
    reserved_snapshot_bits[11] |= 0xF8;
    assert_eq!(
        FreezeFrame::decode(&reserved_snapshot_bits),
        None,
        "freeze-frame SPN snapshots must reject reserved high bits before use"
    );

    let mut count_too_high = encoded.clone();
    count_too_high[8] = 2;
    assert_eq!(
        FreezeFrame::decode(&count_too_high),
        None,
        "freeze-frame snapshot count must match the reassembled payload length"
    );

    let mut trailing_snapshot_tail = encoded;
    trailing_snapshot_tail.push(0xFF);
    assert_eq!(
        FreezeFrame::decode(&trailing_snapshot_tail),
        None,
        "freeze-frame payloads must not accept trailing bytes after declared snapshots"
    );
}
