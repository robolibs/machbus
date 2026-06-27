use std::cell::RefCell;
use std::rc::Rc;

use machbus::isobus::functionalities::{Functionalities, Functionality};
use machbus::isobus::implement::{
    CurvatureCommandStatus, GenericSaeBs02SlotValue, GroundBasedSpeedDist, GuidanceLimitStatus,
    GuidanceMachineInfo, GuidanceSystemCmd, MachineDirection, MachineSelectedSpeedFull,
    MechanicalLockout, RequestResetCommandStatus, SpeedSource, WheelBasedSpeedDist,
};
use machbus::isobus::tc::{
    DDI, DDOP, DDOPHelpers, DataDictionary, DeviceElement, DeviceElementType, DeviceObject,
    DeviceProcessData, DeviceProperty, DeviceValuePresentation, ObjectID,
    ProcessDataAcknowledgeErrorCodes, TC_STATUS_INTERVAL_MS, TCClientCapabilities,
    TCClientTaskStatus, TCServerConfig, TaskControllerClient, TaskControllerServer, TriggerMethod,
    ddi,
};
use machbus::isobus::vt::{
    ObjectID as VTObjectID, ObjectPool, VTClient, VTClientConfig, VTState, WorkingSetBody,
    cmd as vt_cmd, create_working_set,
};
use machbus::j1939::heartbeat::hb_seq;
use machbus::j1939::shortcut_button::{self, ShortcutButtonMessage, ShortcutButtonState};
use machbus::j1939::speed_distance::SpeedAndDistance;
use machbus::j1939::{
    AreaUnit, DateFormat, DecimalSymbol, DiagnosticLamps, DistanceUnit, Dm13Command, Dm13Signals,
    Dm13SuspendSignal, Dm22Control, Dm22Message, Dm22NackReason, Dtc, EcuIdentification, Fmi,
    ForceUnit, HeartbeatRequest, LampFlash, LampStatus, LanguageData, MaintainPowerData,
    MaintainPowerRequirement, MaintainPowerState, MassUnit, PGN_HEARTBEAT_REQUEST, PressureUnit,
    ProductIdentification, SoftwareIdentification, TemperatureUnit, TimeDate, TimeFormat,
    UnitSystem, VolumeUnit,
};
use machbus::net::pgn_defs::{
    PGN_ADDRESS_CLAIMED, PGN_COMMANDED_ADDRESS, PGN_ECU_TO_TC, PGN_ECU_TO_VT,
    PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_POSITION_DATA, PGN_GNSS_POSITION_DELTA,
    PGN_GNSS_POSITION_RAPID, PGN_GROUND_BASED_SPEED_DIST, PGN_GUIDANCE_MACHINE_INFO,
    PGN_GUIDANCE_SYSTEM_CMD, PGN_HEADING_TRACK, PGN_HEARTBEAT, PGN_MACHINE_SPEED, PGN_RATE_OF_TURN,
    PGN_REQUEST, PGN_SHORTCUT_BUTTON, PGN_TC_TO_ECU, PGN_TIME_DATE, PGN_TP_CM, PGN_TP_DT,
    PGN_VT_TO_ECU, PGN_WHEEL_BASED_SPEED_DIST, PGN_WORKING_SET_MASTER,
};
use machbus::net::{
    AddressClaimer, BROADCAST_ADDRESS, ClaimState, FastPacketProtocol, Frame, Identifier,
    InternalCf, IsoNet, MAX_ADDRESS, Message, NULL_ADDRESS, Name, NameFilter, NameFilterField,
    NetworkConfig, Priority, TransportProtocol,
};
use machbus::nmea::{
    COG_RESOLUTION, HEADING_RESOLUTION, LAT_LON_RESOLUTION, NMEAConfig, NMEAInterface,
    POSITION_DELTA_RESOLUTION, POSITION_DELTA_TIME_RESOLUTION,
    PositionDeltaHighPrecisionRapidUpdateData, ROT_RESOLUTION, SPEED_RESOLUTION,
};
use wirebit::ShmLink;
use wirebit::topology::Topology;

// ─── NAME bit-packing compat ──────────────────────────────────────────
//
// AgIsoStack `test/can_name_tests.cpp::NAMEProperties` builds a NAME
// from these specific 9 fields and asserts the full 64-bit raw value
// equals `10881826125818888196`. Verifying we produce the same value
// proves our ISO 11783-5 NAME bit layout matches byte-for-byte.

#[test]
fn name_field_layout_matches_agisostack_magic_constant() {
    let name = Name::default()
        .with_self_configurable(true) // = "arbitrary_address_capable" in AgIsoStack
        .with_industry_group(1)
        .with_device_class(2)
        .with_function_code(3)
        .with_identity_number(4)
        .with_ecu_instance(5)
        .with_function_instance(6)
        .with_device_class_instance(7)
        .with_manufacturer_code(8);
    // AgIsoStack: EXPECT_EQ(TestDeviceNAME.get_full_name(), 10881826125818888196U);
    assert_eq!(name.raw, 10_881_826_125_818_888_196);
}

#[test]
fn name_equality_via_raw_constructor() {
    // AgIsoStack `NAMEEquals`: NAME(10376445291390828545U) == NAME(...).
    let a = Name {
        raw: 10_376_445_291_390_828_545,
    };
    let b = Name {
        raw: 10_376_445_291_390_828_545,
    };
    assert_eq!(a, b);
}

// ─── NAME field truncation (out-of-range setters) ─────────────────────
//
// AgIsoStack `NAMEPropertiesOutOfRange` asserts that setting a value
// wider than the destination field silently drops the high bits. Our
// `with_*` builders mask, so an over-range value should *not* round
// back to the original.

#[test]
fn name_setters_silently_mask_out_of_range_values() {
    // industry_group is 3 bits → max 7. 8 must NOT round-trip.
    let n = Name::default().with_industry_group(8);
    assert_ne!(n.industry_group(), 8);
    // device_class is 7 bits → max 127. 128 must NOT round-trip.
    let n = Name::default().with_device_class(128);
    assert_ne!(n.device_class(), 128);
    // identity_number is 21 bits → max 0x1F_FFFF.
    let n = Name::default().with_identity_number(0x20_0000);
    assert_ne!(n.identity_number(), 0x20_0000);
    // ecu_instance is 3 bits.
    let n = Name::default().with_ecu_instance(8);
    assert_ne!(n.ecu_instance(), 8);
    // function_instance is 5 bits.
    let n = Name::default().with_function_instance(32);
    assert_ne!(n.function_instance(), 32);
    // manufacturer_code is 11 bits.
    let n = Name::default().with_manufacturer_code(2048);
    assert_ne!(n.manufacturer_code(), 2048);
    // device_class_instance is 4 bits.
    let n = Name::default().with_device_class_instance(16);
    assert_ne!(n.device_class_instance(), 16);
}

// ─── Address Claim saturated-network behavior ─────────────────────────
//
// AgIsoStack `address_claim_tests.cpp::CannotClaim` pre-populates every
// claimable source address with lower-priority NAMEs, then creates a
// self-configurable internal CF with a higher NAME. The reference stack emits
// Cannot Claim Address from source 0xFE and leaves that CF without a valid
// address.

#[test]
fn address_claim_cannot_claim_when_all_addresses_are_observed_occupied() {
    let local_name = Name::default()
        .with_self_configurable(true)
        .with_industry_group(1)
        .with_device_class(6)
        .with_function_code(0x80)
        .with_identity_number(65_534)
        .with_ecu_instance(1)
        .with_function_instance(2)
        .with_device_class_instance(0)
        .with_manufacturer_code(1407);
    let mut local_cf = InternalCf::new(local_name, 0, 0x80);
    let mut claimer = AddressClaimer::new(0);
    let _ = claimer.start(&mut local_cf);

    for addr in 0..=MAX_ADDRESS {
        if addr == local_cf.preferred_address() {
            continue;
        }
        let lower_name = Name::default()
            .with_self_configurable(true)
            .with_industry_group(0)
            .with_device_class(0)
            .with_function_code(0)
            .with_identity_number(addr as u32)
            .with_manufacturer_code(1);
        assert!(lower_name < local_name);
        assert!(
            claimer
                .handle_claim(&mut local_cf, addr, lower_name)
                .is_empty(),
            "claims for other addresses are only learned as occupied"
        );
    }

    let preferred_winner = Name::default()
        .with_self_configurable(true)
        .with_industry_group(0)
        .with_device_class(0)
        .with_function_code(0)
        .with_identity_number(1)
        .with_manufacturer_code(1);
    let preferred_address = local_cf.preferred_address();
    let cannot_claim = claimer.handle_claim(&mut local_cf, preferred_address, preferred_winner);

    assert_eq!(local_cf.claim_state(), ClaimState::Failed);
    assert_eq!(local_cf.address(), NULL_ADDRESS);
    assert_eq!(cannot_claim.len(), 1);
    assert_eq!(cannot_claim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(cannot_claim[0].source(), NULL_ADDRESS);
    assert_eq!(cannot_claim[0].id.raw, 0x18EE_FFFE);
    assert_eq!(cannot_claim[0].payload(), &local_name.to_bytes());
}

#[test]
fn address_claim_partnered_filtering_matches_agisostack_partnered_claim() {
    // AgIsoStack NAME::Function enum values used by
    // `address_claim_tests.cpp::AddressClaim_PartneredClaim`.
    const CAB_CLIMATE_CONTROL: u8 = 21;
    const SEAT_CONTROL: u8 = 40;

    let cab_climate = Name::default()
        .with_self_configurable(true)
        .with_industry_group(1)
        .with_device_class(0)
        .with_function_code(CAB_CLIMATE_CONTROL)
        .with_identity_number(1)
        .with_manufacturer_code(69);
    let seat_control = Name::default()
        .with_self_configurable(true)
        .with_industry_group(1)
        .with_device_class(0)
        .with_function_code(SEAT_CONTROL)
        .with_identity_number(2)
        .with_manufacturer_code(69);

    let mut topo = Topology::new();
    let first = topo.add_node("first");
    let second = topo.add_node("second");
    topo.can_bus("bus0").members(&[first, second]);
    let mut built = topo.build().unwrap();
    let bus = built.can_bus_mut("bus0").unwrap();
    let first_ep = bus.take_endpoint("first").unwrap();
    let second_ep = bus.take_endpoint("second").unwrap();

    let mut first_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    let mut second_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    first_net.set_endpoint(0, first_ep);
    second_net.set_endpoint(0, second_ep);

    let first_internal = first_net.create_internal(cab_climate, 0, 0x1C).unwrap();
    let second_internal = second_net.create_internal(seat_control, 0, 0x2A).unwrap();
    let first_partner_for_second = first_net
        .create_partner(
            0,
            vec![NameFilter::new(
                NameFilterField::FunctionCode,
                SEAT_CONTROL as u32,
            )],
        )
        .unwrap();
    let second_partner_for_first = second_net
        .create_partner(
            0,
            vec![NameFilter::new(
                NameFilterField::FunctionCode,
                CAB_CLIMATE_CONTROL as u32,
            )],
        )
        .unwrap();

    first_net.start_address_claiming().unwrap();
    second_net.start_address_claiming().unwrap();
    for _ in 0..30 {
        first_net.update(50);
        second_net.update(50);
        built.pump_all().unwrap();
        first_net.update(0);
        second_net.update(0);
        built.pump_all().unwrap();
        if first_net.internal_cf(first_internal).unwrap().claim_state() == ClaimState::Claimed
            && second_net
                .internal_cf(second_internal)
                .unwrap()
                .claim_state()
                == ClaimState::Claimed
            && first_net
                .partner_cf(first_partner_for_second)
                .unwrap()
                .cf()
                .is_online()
            && second_net
                .partner_cf(second_partner_for_first)
                .unwrap()
                .cf()
                .is_online()
        {
            break;
        }
    }

    assert_eq!(
        first_net.internal_cf(first_internal).unwrap().claim_state(),
        ClaimState::Claimed
    );
    assert_eq!(
        second_net
            .internal_cf(second_internal)
            .unwrap()
            .claim_state(),
        ClaimState::Claimed
    );
    assert_eq!(
        first_net
            .partner_cf(first_partner_for_second)
            .unwrap()
            .address(),
        0x2A
    );
    assert_eq!(
        first_net
            .partner_cf(first_partner_for_second)
            .unwrap()
            .name(),
        seat_control
    );
    assert_eq!(
        second_net
            .partner_cf(second_partner_for_first)
            .unwrap()
            .address(),
        0x1C
    );
    assert_eq!(
        second_net
            .partner_cf(second_partner_for_first)
            .unwrap()
            .name(),
        cab_climate
    );
}

#[test]
fn commanded_address_bam_matches_agisostack_core_network_example() {
    // AgIsoStack `core_network_management_tests.cpp::CommandedAddress`
    // delivers PGN 0xFED8 over a short TP.BAM: 9 bytes total, two DT packets,
    // payload = target NAME (little endian) plus new source address 0x04.
    let mut topo = Topology::new();
    let internal = topo.add_node("internal");
    let external = topo.add_node("external");
    topo.can_bus("bus0").members(&[internal, external]);
    let mut built = topo.build().unwrap();
    let bus = built.can_bus_mut("bus0").unwrap();
    let internal_ep = bus.take_endpoint("internal").unwrap();
    let external_ep = bus.take_endpoint("external").unwrap();

    let internal_name = Name::default()
        .with_self_configurable(true)
        .with_industry_group(1)
        .with_device_class(7)
        .with_function_code(0x80)
        .with_identity_number(0x343)
        .with_manufacturer_code(69);
    let external_name = Name::default()
        .with_self_configurable(true)
        .with_industry_group(1)
        .with_device_class(7)
        .with_function_code(0x81)
        .with_identity_number(0xF8)
        .with_manufacturer_code(69);

    let mut internal_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    let mut external_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    internal_net.set_endpoint(0, internal_ep);
    external_net.set_endpoint(0, external_ep);

    let internal_handle = internal_net
        .create_internal(internal_name, 0, 0x43)
        .unwrap();
    let external_handle = external_net
        .create_internal(external_name, 0, 0xF8)
        .unwrap();
    let external_partner_for_internal = external_net
        .create_partner(
            0,
            vec![NameFilter::new(
                NameFilterField::IdentityNumber,
                internal_name.identity_number(),
            )],
        )
        .unwrap();

    internal_net.start_address_claiming().unwrap();
    external_net.start_address_claiming().unwrap();
    for _ in 0..30 {
        internal_net.update(50);
        external_net.update(50);
        built.pump_all().unwrap();
        internal_net.update(0);
        external_net.update(0);
        built.pump_all().unwrap();
        if internal_net
            .internal_cf(internal_handle)
            .unwrap()
            .claim_state()
            == ClaimState::Claimed
            && external_net
                .internal_cf(external_handle)
                .unwrap()
                .claim_state()
                == ClaimState::Claimed
        {
            break;
        }
    }
    assert_eq!(
        internal_net.internal_cf(internal_handle).unwrap().address(),
        0x43
    );

    let mut commanded_address_payload = internal_name.to_bytes().to_vec();
    commanded_address_payload.push(0x04);
    external_net
        .send(
            PGN_COMMANDED_ADDRESS,
            &commanded_address_payload,
            external_handle,
            BROADCAST_ADDRESS,
            Priority::Lowest,
        )
        .unwrap();

    for _ in 0..30 {
        internal_net.update(50);
        external_net.update(50);
        built.pump_all().unwrap();
        internal_net.update(0);
        external_net.update(0);
        built.pump_all().unwrap();
        if internal_net.internal_cf(internal_handle).unwrap().address() == 0x04
            && external_net
                .partner_cf(external_partner_for_internal)
                .unwrap()
                .address()
                == 0x04
        {
            break;
        }
    }

    let internal_cf = internal_net.internal_cf(internal_handle).unwrap();
    assert_eq!(internal_cf.claim_state(), ClaimState::Claimed);
    assert!(internal_cf.cf().is_online());
    assert_eq!(internal_cf.address(), 0x04);
    assert_eq!(
        external_net
            .partner_cf(external_partner_for_internal)
            .unwrap()
            .address(),
        0x04
    );
}

#[test]
fn partner_control_function_invalidates_after_unanswered_address_claim_request() {
    // AgIsoStack `core_network_management_tests.cpp::InvalidatingControlFunctions`
    // makes a partner CF online from an Address Claimed frame, then sends a
    // global request for Address Claimed. If that partner does not answer
    // within the reference wait window, it becomes address-invalid/offline.
    let mut topo = Topology::new();
    let observer = topo.add_node("observer");
    let partner_node = topo.add_node("partner");
    let requester = topo.add_node("requester");
    topo.can_bus("bus0")
        .members(&[observer, partner_node, requester]);
    let mut built = topo.build().unwrap();
    let bus = built.can_bus_mut("bus0").unwrap();
    let observer_ep = bus.take_endpoint("observer").unwrap();
    let partner_ep = bus.take_endpoint("partner").unwrap();
    let requester_ep = bus.take_endpoint("requester").unwrap();

    let observer_name = Name::default()
        .with_self_configurable(true)
        .with_function_code(0x82)
        .with_identity_number(0x40)
        .with_manufacturer_code(69);
    let partner_name = Name::default()
        .with_self_configurable(true)
        .with_function_code(0x79)
        .with_identity_number(0x79)
        .with_manufacturer_code(69);
    let requester_name = Name::default()
        .with_self_configurable(true)
        .with_function_code(0x83)
        .with_identity_number(0x41)
        .with_manufacturer_code(69);

    let mut observer_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    let mut partner_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    let mut requester_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    observer_net.set_endpoint(0, observer_ep);
    partner_net.set_endpoint(0, partner_ep);
    requester_net.set_endpoint(0, requester_ep);

    let observer_handle = observer_net
        .create_internal(observer_name, 0, 0x40)
        .unwrap();
    let partner_handle = partner_net.create_internal(partner_name, 0, 0x79).unwrap();
    let requester_handle = requester_net
        .create_internal(requester_name, 0, 0x41)
        .unwrap();
    let observed_partner = observer_net
        .create_partner(
            0,
            vec![NameFilter::new(
                NameFilterField::IdentityNumber,
                partner_name.identity_number(),
            )],
        )
        .unwrap();

    observer_net.start_address_claiming().unwrap();
    partner_net.start_address_claiming().unwrap();
    requester_net.start_address_claiming().unwrap();
    for _ in 0..30 {
        observer_net.update(50);
        partner_net.update(50);
        requester_net.update(50);
        built.pump_all().unwrap();
        observer_net.update(0);
        partner_net.update(0);
        requester_net.update(0);
        built.pump_all().unwrap();
        if observer_net
            .internal_cf(observer_handle)
            .unwrap()
            .claim_state()
            == ClaimState::Claimed
            && partner_net
                .internal_cf(partner_handle)
                .unwrap()
                .claim_state()
                == ClaimState::Claimed
            && requester_net
                .internal_cf(requester_handle)
                .unwrap()
                .claim_state()
                == ClaimState::Claimed
            && observer_net
                .partner_cf(observed_partner)
                .unwrap()
                .cf()
                .is_online()
        {
            break;
        }
    }
    assert_eq!(
        observer_net.partner_cf(observed_partner).unwrap().address(),
        0x79
    );

    let mut request_address_claim = [0xFFu8; 8];
    request_address_claim[0] = (PGN_ADDRESS_CLAIMED & 0xFF) as u8;
    request_address_claim[1] = ((PGN_ADDRESS_CLAIMED >> 8) & 0xFF) as u8;
    request_address_claim[2] = ((PGN_ADDRESS_CLAIMED >> 16) & 0xFF) as u8;
    requester_net
        .send(
            PGN_REQUEST,
            &request_address_claim,
            requester_handle,
            BROADCAST_ADDRESS,
            Priority::Default,
        )
        .unwrap();

    // Deliver the request to the observer, but intentionally stop polling the
    // partner node so it does not answer the request-for-address-claim.
    built.pump_all().unwrap();
    observer_net.update(0);
    requester_net.update(0);
    built.pump_all().unwrap();

    assert!(
        observer_net
            .partner_cf(observed_partner)
            .unwrap()
            .cf()
            .is_online(),
        "the request starts a validation window, it does not invalidate immediately",
    );

    observer_net.update(2_000);
    assert_eq!(
        observer_net.partner_cf(observed_partner).unwrap().address(),
        NULL_ADDRESS
    );
    assert!(
        !observer_net
            .partner_cf(observed_partner)
            .unwrap()
            .cf()
            .is_online()
    );
}

// ─── Identifier construction round-trip ───────────────────────────────
//
// AgIsoStack `identifier_tests.cpp::RawIdentifierConstuction` builds
// an Extended ID with PGN 0xEF00, dest 0x1C, source 0x80, priority
// "Default 6" and reads the fields back unchanged.

#[test]
fn identifier_round_trip_pdu1() {
    // Priority::Default = 6 in our enum ⇄ AgIsoStack `PriorityDefault6`.
    let id = Identifier::encode(Priority::Default, 0xEF00, 0x80, 0x1C);
    assert_eq!(id.priority(), Priority::Default);
    assert_eq!(id.pgn(), 0xEF00);
    assert_eq!(id.source(), 0x80);
    assert_eq!(id.destination(), 0x1C);
    // PDU1 (PF < 0xF0): destination is part of the ID, not part of the
    // PGN — re-extracting it from the raw 29-bit value must round-trip.
    assert!(!id.is_broadcast());
}

#[test]
fn identifier_request_pgn_routes_to_destination_byte() {
    // PGN_REQUEST (0xEA00) is PDU1. Encoding with dest 0x42 must place
    // 0x42 in the PS byte of the raw 29-bit identifier.
    let id = Identifier::encode(Priority::High, PGN_REQUEST, 0x80, 0x42);
    assert_eq!(id.destination(), 0x42);
    assert_eq!(id.source(), 0x80);
    assert_eq!(id.priority(), Priority::High);
}

// ─── CANMessage defensive data accessors ──────────────────────────────
//
// AgIsoStack `test/can_message_tests.cpp::DataCorrectnessTest` feeds the
// byte sequence 01 02 03 04 05 06 07 08 through the CANMessage accessors and
// asserts the little-endian multi-byte and custom-bit extraction results below.

#[test]
fn message_data_accessors_match_agisostack_data_correctness_subset() {
    let mut msg = Message::new(0xE100, vec![1, 2, 3, 4, 5, 6, 7, 8], 0xAA);
    msg.timestamp_us = 1_000_000;

    assert_eq!(msg.timestamp_us, 1_000_000);
    assert_eq!(msg.get_u16_le(0), 513);
    assert_eq!(msg.get_u32_le(0), 67_305_985);
    assert_eq!(msg.get_u64_le(0), 578_437_695_752_307_201);
    assert_eq!(msg.get_bits(8, 16), 770);
    assert_eq!(msg.get_bits(14, 3), 4);
    assert_eq!(msg.get_bits(63, 255), 0);
    assert_eq!(msg.get_bits(65_748_321, 1), 0);
}

#[test]
fn can_frame_wrapper_matches_agisostack_can_message_identity_subset() {
    let frame = Frame::from_message_at(
        Priority::Default,
        0xE100,
        0xAA,
        0x55,
        &[1, 2, 3, 4, 5, 6, 7, 8],
        1_000_000,
    );
    assert_eq!(frame.id.raw, 0x18E1_55AA);
    assert_eq!(frame.pgn(), 0xE100);
    assert_eq!(frame.source(), 0xAA);
    assert_eq!(frame.destination(), 0x55);
    assert_eq!(frame.priority(), Priority::Default);
    assert_eq!(frame.length, 8);
    assert_eq!(frame.payload(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(frame.timestamp_us, 1_000_000);
    assert!(!frame.is_broadcast());

    let can_frame = frame.to_can_frame();
    assert!(can_frame.is_extended());
    assert_eq!(can_frame.id(), 0x18E1_55AA);
    let restored = Frame::from_can_frame(&can_frame).expect("extended data frame converts back");
    assert_eq!(restored.id, frame.id);
    assert_eq!(restored.length, frame.length);
    assert_eq!(restored.payload(), frame.payload());
}

// ─── Transport Protocol TP.BAM / TP.CMDT byte layout ───────────────────
//
// AgIsoStack `transport_protocol_tests.cpp::BroadcastMessageSending` and
// `DestinationSpecificMessageSending` pin the TP.CM and TP.DT bytes for
// a 17-byte broadcast PGN 0xFEEC and a 23-byte destination-specific PGN
// 0xFEEB. These tests keep our public TP engine aligned with those frames.

#[test]
fn transport_protocol_sending_matches_agisostack_examples() {
    let broadcast_data: Vec<u8> = (1..=17).collect();
    let mut bam = TransportProtocol::new();
    let first = bam
        .send(0xFEEC, &broadcast_data, 0x01, 0xFF, 0, Priority::Lowest)
        .expect("BAM send must start");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].pgn(), PGN_TP_CM);
    assert_eq!(first[0].destination(), 0xFF);
    assert_eq!(first[0].data, [0x20, 17, 0, 3, 0xFF, 0xEC, 0xFE, 0x00]);

    for expected in [
        [1, 1, 2, 3, 4, 5, 6, 7],
        [2, 8, 9, 10, 11, 12, 13, 14],
        [3, 15, 16, 17, 0xFF, 0xFF, 0xFF, 0xFF],
    ] {
        let out = bam.update(50);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pgn(), PGN_TP_DT);
        assert_eq!(out[0].destination(), 0xFF);
        assert_eq!(out[0].data, expected);
    }
    assert!(bam.active_sessions().is_empty());

    let destination_data: Vec<u8> = (1..=23).collect();
    let mut cmdt = TransportProtocol::with_advertised_packets_per_cts(1);
    assert_eq!(cmdt.advertised_packets_per_cts(), 1);
    let rts = cmdt
        .send(0xFEEB, &destination_data, 0x01, 0x02, 0, Priority::Lowest)
        .expect("CMDT send must start");
    assert_eq!(rts.len(), 1);
    assert_eq!(rts[0].pgn(), PGN_TP_CM);
    assert_eq!(rts[0].destination(), 0x02);
    assert_eq!(rts[0].data, [0x10, 23, 0, 4, 1, 0xEB, 0xFE, 0x00]);

    let cts_1 = Frame::new(
        Identifier::encode(Priority::Lowest, PGN_TP_CM, 0x02, 0x01),
        [0x11, 2, 1, 0xFF, 0xFF, 0xEB, 0xFE, 0x00],
        8,
    );
    assert!(cmdt.process_frame(&cts_1, 0).is_empty());
    let first_window = cmdt.get_pending_data_frames();
    assert_eq!(first_window.len(), 2);
    assert_eq!(first_window[0].data, [1, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(first_window[1].data, [2, 8, 9, 10, 11, 12, 13, 14]);

    let cts_2 = Frame::new(
        Identifier::encode(Priority::Lowest, PGN_TP_CM, 0x02, 0x01),
        [0x11, 2, 3, 0xFF, 0xFF, 0xEB, 0xFE, 0x00],
        8,
    );
    assert!(cmdt.process_frame(&cts_2, 0).is_empty());
    let second_window = cmdt.get_pending_data_frames();
    assert_eq!(second_window.len(), 2);
    assert_eq!(second_window[0].data, [3, 15, 16, 17, 18, 19, 20, 21]);
    assert_eq!(
        second_window[1].data,
        [4, 22, 23, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
}

// ─── ISO 11783-6 VT client command construction ───────────────────────
//
// AgIsoStack `vt_client_tests.cpp::MessageConstruction` pins ECU-to-VT
// command queue output for ChangeActiveMask, HideShow, EnableDisable, and a
// GraphicsContext DrawText command. The Rust client is pump-style rather than
// CAN-manager-coupled, so this test compares the same command payload bytes
// and the PDU1 raw identifier that a caller emits when routing the outbound
// command from ECU 0x37 to VT 0x26 at priority 5.

fn agisostack_vt_dummy_pool() -> ObjectPool {
    use machbus::isobus::vt::{DataMaskBody, create_data_mask};

    ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
}

fn vt_message(data: Vec<u8>, src: u8) -> Message {
    Message::new(PGN_VT_TO_ECU, data, src)
}

fn connect_agisostack_vt_client() -> VTClient {
    let mut client = VTClient::new(VTClientConfig::default());
    client.set_object_pool(agisostack_vt_dummy_pool());
    client
        .connect()
        .expect("VT client connect starts with pool");

    let mut vt_status = vec![vt_cmd::VT_STATUS];
    vt_status.resize(8, 0xFF);
    vt_status[6] = 4;
    client.handle_vt_message(&vt_message(vt_status, 0x26));
    let _ = client.update(1);
    let _ = client.update(1);

    let mut memory_response = [0xFFu8; 8];
    memory_response[0] = vt_cmd::GET_MEMORY_RESPONSE;
    memory_response[1] = 0x00;
    client.handle_vt_message(&vt_message(memory_response.to_vec(), 0x26));
    let _ = client.update(1);
    let _ = client.update(1_000);

    let mut end_response = [0xFFu8; 8];
    end_response[0] = vt_cmd::END_OF_POOL;
    end_response[1] = 0x00;
    end_response[6] = 0x00;
    client.handle_vt_message(&vt_message(end_response.to_vec(), 0x26));
    assert_eq!(client.state(), VTState::Connected);
    client
}

fn assert_agisostack_ecu_to_vt_command(
    out_pgn: u32,
    out_dest: Option<u8>,
    data: &[u8],
    expected: &[u8],
) {
    assert_eq!(out_pgn, PGN_ECU_TO_VT);
    assert_eq!(out_dest, Some(0x26));
    assert_eq!(data, expected);
    assert_eq!(
        Identifier::encode(Priority::Low, out_pgn, 0x37, out_dest.unwrap()).raw,
        0x14E7_2637
    );
}

#[test]
fn vt_client_message_construction_matches_agisostack_examples() {
    let client = connect_agisostack_vt_client();

    let active_mask = client
        .change_active_mask(VTObjectID::new(123), VTObjectID::new(456))
        .unwrap();
    assert_agisostack_ecu_to_vt_command(
        active_mask.pgn,
        active_mask.dest,
        &active_mask.data,
        &[0xAD, 123, 0, 0xC8, 0x01, 0xFF, 0xFF, 0xFF],
    );

    let hide = client.hide_show(1234, false).unwrap();
    assert_agisostack_ecu_to_vt_command(
        hide.pgn,
        hide.dest,
        &hide.data,
        &[0xA0, 0xD2, 0x04, 0x00, 0xFF, 0xFF, 0xFF, 0xFF],
    );

    let disable = client.enable_disable(1234, false).unwrap();
    assert_agisostack_ecu_to_vt_command(
        disable.pgn,
        disable.dest,
        &disable.data,
        &[0xA1, 0xD2, 0x04, 0x00, 0xFF, 0xFF, 0xFF, 0xFF],
    );

    let draw_text = client
        .graphics_context_draw_text(VTObjectID::new(123), true, "a")
        .unwrap();
    assert_agisostack_ecu_to_vt_command(
        draw_text.pgn,
        draw_text.dest,
        &draw_text.data,
        &[0xB8, 123, 0, 0x0D, 0x01, 0x01, b'a', 0xFF],
    );

    assert!(
        client
            .graphics_context_draw_text(VTObjectID::new(123), true, "")
            .is_err()
    );
}

// ─── ISO 11783-10 TC client command byte layout ───────────────────────
//
// AgIsoStack `tc_client_tests.cpp::MessageEncoding` pins the outbound
// TC-client payloads for Working Set Master, version/label/delete
// requests, client status, capabilities response, PDACK, value command,
// and TC-identification request.

#[test]
fn task_controller_client_message_encoding_matches_agisostack_examples() {
    let wsm = TaskControllerClient::build_working_set_master(1);
    assert_eq!(wsm, [1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    let wsm_frame =
        Frame::from_message(Priority::Default, PGN_WORKING_SET_MASTER, 0x84, 0xFF, &wsm);
    assert_eq!(wsm_frame.pgn(), PGN_WORKING_SET_MASTER);
    assert_eq!(wsm_frame.payload(), &wsm);

    let version_request = TaskControllerClient::build_version_request();
    assert_eq!(
        version_request,
        [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    let version_frame = Frame::from_message(
        Priority::Default,
        PGN_ECU_TO_TC,
        0x84,
        0xF7,
        &version_request,
    );
    assert_eq!(version_frame.pgn(), PGN_ECU_TO_TC);
    assert_eq!(version_frame.source(), 0x84);
    assert_eq!(version_frame.destination(), 0xF7);

    assert_eq!(
        TaskControllerClient::build_status(TCClientTaskStatus::Idle, 0, 0),
        [0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00]
    );

    assert_eq!(
        TaskControllerClient::build_request_version_response(TCClientCapabilities::default()),
        [0x10, 0x04, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
    let all_options = TCClientCapabilities::default()
        .with_options(0x1F)
        .with_booms(1)
        .with_sections(2)
        .with_channels(3);
    assert_eq!(
        TaskControllerClient::build_request_version_response(all_options),
        [0x10, 0x04, 0xFF, 0x1F, 0x00, 0x01, 0x02, 0x03]
    );

    assert_eq!(
        TaskControllerClient::build_request_structure_label(),
        [0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        TaskControllerClient::build_request_localization_label(),
        [0x21, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        TaskControllerClient::build_delete_object_pool(),
        [0xA1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );

    assert_eq!(
        TaskControllerClient::build_process_data_ack(
            47u16,
            29u16,
            ProcessDataAcknowledgeErrorCodes::NoError
        )
        .unwrap(),
        [0xFD, 0x02, 0x1D, 0x00, 0x00, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        TaskControllerClient::build_value_command(1234u16, 567u16, 8910).unwrap(),
        [0x23, 0x4D, 0x37, 0x02, 0xCE, 0x22, 0x00, 0x00]
    );
    assert!(TaskControllerClient::build_value_command(0x1000u16, 567u16, 8910).is_err());

    assert_eq!(
        TaskControllerClient::build_task_controller_identification_request(),
        [0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
}

#[test]
fn task_controller_server_version_response_matches_agisostack_example() {
    // AgIsoStack `tc_server_tests.cpp::MessageEncoding` configures a TC with
    // version 4, options 0x15, 4 booms, and 16 channels, then responds to the
    // client version request on PGN 0xCB00. The original fixture uses 0xFF for
    // sections; machbus rejects that unadvertisable topology at start(), so the
    // positive compatibility case uses the highest advertisable section count.
    let mut server = TaskControllerServer::new(
        TCServerConfig::default()
            .with_version(4)
            .with_options(0x15)
            .with_booms(4)
            .with_sections(0xFE)
            .with_channels(16),
    );
    server.start().unwrap();

    let out = server.handle_client_message(&Message::new(
        PGN_ECU_TO_TC,
        TaskControllerClient::build_version_request().to_vec(),
        0x88,
    ));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].dest, Some(0x88));
    assert_eq!(
        out[0].data,
        [0x10, 0x04, 0xFF, 0x15, 0x00, 0x04, 0xFE, 0x10]
    );

    assert_eq!(out[0].pgn, PGN_TC_TO_ECU);
    let frame = Frame::from_message(Priority::Low, PGN_TC_TO_ECU, 0x87, 0x88, &out[0].data);
    assert_eq!(frame.id.raw, 0x14CB_8887);
}

#[test]
fn task_controller_server_busy_status_matches_agisostack_examples() {
    // AgIsoStack `tc_server_tests.cpp::B6CommandBusyStateTracking` and the two
    // command-specific variants assert that TC Status byte 4 carries the
    // BusyExecutingACommand bit while bytes 5 and 6 expose the command source
    // address and the current B6 command group byte.
    let mut server = TaskControllerServer::new(
        TCServerConfig::default()
            .with_number(7)
            .with_version(4)
            .with_options(0x11)
            .with_booms(4)
            .with_sections(0xFE)
            .with_channels(16),
    );
    server.start().unwrap();

    server.set_command_busy(false);
    let idle = server.update(TC_STATUS_INTERVAL_MS).unwrap();
    assert_eq!(idle[0], 0xFE);
    assert_eq!(idle[4] & 0x08, 0x00);
    assert_eq!(idle[5], 0x00);
    assert_eq!(idle[6], 0x00);

    server.set_command_busy_for(0x88, 0x60);
    let object_pool_transfer_busy = server.update(TC_STATUS_INTERVAL_MS).unwrap();
    assert_eq!(object_pool_transfer_busy[0], 0xFE);
    assert_ne!(object_pool_transfer_busy[4] & 0x08, 0x00);
    assert_eq!(object_pool_transfer_busy[5], 0x88);
    assert_eq!(object_pool_transfer_busy[6], 0x60);

    server.set_command_busy(false);
    let cleared = server.update(TC_STATUS_INTERVAL_MS).unwrap();
    assert_eq!(cleared[4] & 0x08, 0x00);
    assert_eq!(cleared[5], 0x00);
    assert_eq!(cleared[6], 0x00);

    server.set_command_busy_for(0x77, 0x80);
    let activate_deactivate_busy = server.update(TC_STATUS_INTERVAL_MS).unwrap();
    assert_ne!(activate_deactivate_busy[4] & 0x08, 0x00);
    assert_eq!(activate_deactivate_busy[5], 0x77);
    assert_eq!(activate_deactivate_busy[6], 0x80);
}

#[test]
fn task_controller_server_client_version_tracking_matches_agisostack_example() {
    // AgIsoStack `tc_server_tests.cpp::ClientVersionTracking` registers a
    // client from Working Set Master, sends a version request, reports version
    // 0 before any response, then stores successive client version responses.
    let mut server = TaskControllerServer::new(
        TCServerConfig::default()
            .with_version(4)
            .with_options(0x15)
            .with_booms(4)
            .with_sections(0xFE)
            .with_channels(16),
    );
    server.start().unwrap();

    let version_events = Rc::new(RefCell::new(Vec::new()));
    let version_events_clone = version_events.clone();
    server
        .on_client_version_received
        .subscribe(move |event| version_events_clone.borrow_mut().push(*event));

    let out = server.handle_working_set_master(&Message::new(
        PGN_WORKING_SET_MASTER,
        TaskControllerClient::build_working_set_master(1).to_vec(),
        0x91,
    ));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].pgn, PGN_TC_TO_ECU);
    assert_eq!(out[0].dest, Some(0x91));
    assert_eq!(
        out[0].data,
        TaskControllerClient::build_version_request().to_vec()
    );
    assert_eq!(server.get_client_version(0x91), 0);

    let out = server.handle_client_message(&Message::new(
        PGN_ECU_TO_TC,
        vec![0x10, 0x04, 0xFF, 0x1F, 0x00, 0x01, 0x20, 0x10],
        0x91,
    ));
    assert!(out.is_empty());
    assert_eq!(server.get_client_version(0x91), 4);
    assert_eq!(*version_events.borrow(), vec![(0x91, 4)]);

    let request = server.request_client_version(0x91).unwrap();
    assert_eq!(request.pgn, PGN_TC_TO_ECU);
    assert_eq!(request.dest, Some(0x91));
    assert_eq!(
        request.data,
        TaskControllerClient::build_version_request().to_vec()
    );

    let out = server.handle_client_message(&Message::new(
        PGN_ECU_TO_TC,
        vec![0x10, 0x03, 0xFF, 0x1F, 0x00, 0x01, 0x20, 0x10],
        0x91,
    ));
    assert!(out.is_empty());
    assert_eq!(server.get_client_version(0x91), 3);
    assert_eq!(*version_events.borrow(), vec![(0x91, 4), (0x91, 3)]);

    let client = server
        .clients()
        .iter()
        .find(|client| client.address == 0x91)
        .unwrap();
    assert_eq!(client.tc_options, 0x1F);
    assert_eq!(client.tc_booms, 0x01);
    assert_eq!(client.tc_sections, 0x20);
    assert_eq!(client.tc_channels, 0x10);
}

#[test]
fn task_controller_server_label_responses_match_agisostack_examples() {
    // AgIsoStack `tc_server_tests.cpp::MessageEncoding` answers structure and
    // localization label requests with all-FF bytes when no label is available,
    // then echoes the configured seven-byte labels.
    let mut server = TaskControllerServer::new(
        TCServerConfig::default()
            .with_version(4)
            .with_options(0x15)
            .with_booms(4)
            .with_sections(0xFE)
            .with_channels(16),
    );
    server.start().unwrap();

    let out = server.handle_client_message(&Message::new(
        PGN_ECU_TO_TC,
        TaskControllerClient::build_request_structure_label().to_vec(),
        0x88,
    ));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].dest, Some(0x88));
    assert_eq!(
        out[0].data,
        [0x11, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    let frame = Frame::from_message(Priority::Low, PGN_TC_TO_ECU, 0x87, 0x88, &out[0].data);
    assert_eq!(frame.id.raw, 0x14CB_8887);

    server.set_structure_label([1, 2, 3, 4, 5, 6, 7]);
    let out = server.handle_client_message(&Message::new(
        PGN_ECU_TO_TC,
        TaskControllerClient::build_request_structure_label().to_vec(),
        0x88,
    ));
    assert_eq!(out[0].data, [0x11, 1, 2, 3, 4, 5, 6, 7]);

    let out = server.handle_client_message(&Message::new(
        PGN_ECU_TO_TC,
        TaskControllerClient::build_request_localization_label().to_vec(),
        0x88,
    ));
    assert_eq!(
        out[0].data,
        [0x31, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );

    server.set_localization_label([1, 2, 3, 4, 5, 6, 7]);
    let out = server.handle_client_message(&Message::new(
        PGN_ECU_TO_TC,
        TaskControllerClient::build_request_localization_label().to_vec(),
        0x88,
    ));
    assert_eq!(out[0].data, [0x31, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn task_controller_server_process_data_commands_match_agisostack_examples() {
    // AgIsoStack `tc_server_tests.cpp::MessageEncoding` checks the same
    // TC→ECU process-data command payloads for a partnered client at address
    // 0x88. These builders expose the byte-exact payloads without needing a
    // live CAN plugin.
    let request_value = TaskControllerServer::build_request_value(456u16, 1234u16).unwrap();
    assert_eq!(
        request_value,
        [0x82, 0x1C, 0xD2, 0x04, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        Frame::from_message(Priority::Low, PGN_TC_TO_ECU, 0x87, 0x88, &request_value)
            .id
            .raw,
        0x14CB_8887
    );

    assert_eq!(
        TaskControllerServer::build_time_interval_measurement_command(99u16, 6u16, 1000).unwrap(),
        [0x34, 0x06, 0x06, 0x00, 0xE8, 0x03, 0x00, 0x00]
    );
    assert_eq!(
        TaskControllerServer::build_distance_interval_measurement_command(999u16, 654u16, 65534)
            .unwrap(),
        [0x75, 0x3E, 0x8E, 0x02, 0xFE, 0xFF, 0x00, 0x00]
    );
    assert_eq!(
        TaskControllerServer::build_minimum_threshold_measurement_command(
            0u16,
            445u16,
            0x00FF_FFFF
        )
        .unwrap(),
        [0x06, 0x00, 0xBD, 0x01, 0xFF, 0xFF, 0xFF, 0x00]
    );
    assert_eq!(
        TaskControllerServer::build_maximum_threshold_measurement_command(
            0u16,
            445u16,
            0xFFFF_FFFF
        )
        .unwrap(),
        [0x07, 0x00, 0xBD, 0x01, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        TaskControllerServer::build_change_threshold_measurement_command(0u16, 14u16, 1).unwrap(),
        [0x08, 0x00, 0x0E, 0x00, 0x01, 0x00, 0x00, 0x00]
    );

    let set_value_ack =
        TaskControllerServer::build_set_value_and_acknowledge(0u16, 14u16, 600).unwrap();
    assert_eq!(
        set_value_ack,
        [0x0A, 0x00, 0x0E, 0x00, 0x58, 0x02, 0x00, 0x00]
    );
    assert_eq!(
        Frame::from_message(Priority::Normal, PGN_TC_TO_ECU, 0x87, 0x88, &set_value_ack)
            .id
            .raw,
        0x0CCB_8887
    );

    assert_eq!(
        TaskControllerServer::build_set_value(0u16, 2455u16, 800).unwrap(),
        [0x03, 0x00, 0x97, 0x09, 0x20, 0x03, 0x00, 0x00]
    );
}

// ─── Language Command wire layout ─────────────────────────────────────
//
// AgIsoStack `language_command_interface_tests.cpp` asserts that Language
// Command transmit payloads use:
//   byte 2: decimal bits 6..7, time bits 4..5, low nibble 0xF
//   byte 3: date format
//   byte 4: mass/volume/area/distance packed in two-bit fields
//   byte 5: generic/force/pressure/temperature packed in two-bit fields
//   byte 6..7: country code

#[test]
fn language_command_layout_matches_agisostack_transmit_packing() {
    let en_us = LanguageData {
        language_code: *b"en",
        decimal: DecimalSymbol::Comma,
        time_format: TimeFormat::TwelveHour,
        date_format: DateFormat::YyyyMmDd,
        distance: DistanceUnit::Imperial,
        area: AreaUnit::Imperial,
        volume: VolumeUnit::Us,
        mass: MassUnit::Us,
        temperature: TemperatureUnit::Imperial,
        pressure: PressureUnit::Imperial,
        force: ForceUnit::Imperial,
        generic: UnitSystem::Us,
        country_code: *b"US",
    };
    let bytes = en_us.encode();
    assert_eq!(bytes, [b'e', b'n', 0x1F, 0x04, 0x5A, 0x55, b'U', b'S']);
    assert_eq!(
        LanguageData::decode(&Message::new(0xFE0F, bytes.to_vec(), 0x49)),
        Some(en_us)
    );
}

#[test]
fn language_command_decodes_agisostack_message_content_samples() {
    let sample = [b'e', b'n', 0x0F, 0x04, 0x5A, 0x04, b'U', b'S'];
    let decoded = LanguageData::decode(&Message::new(0xFE0F, sample.to_vec(), 0x80)).unwrap();
    assert_eq!(decoded.language_code, *b"en");
    assert_eq!(decoded.decimal, DecimalSymbol::Comma);
    assert_eq!(decoded.time_format, TimeFormat::TwentyFourHour);
    assert_eq!(decoded.date_format, DateFormat::YyyyMmDd);
    assert_eq!(decoded.distance, DistanceUnit::Imperial);
    assert_eq!(decoded.area, AreaUnit::Imperial);
    assert_eq!(decoded.volume, VolumeUnit::Us);
    assert_eq!(decoded.mass, MassUnit::Us);
    assert_eq!(decoded.temperature, TemperatureUnit::Metric);
    assert_eq!(decoded.pressure, PressureUnit::Metric);
    assert_eq!(decoded.force, ForceUnit::Imperial);
    assert_eq!(decoded.generic, UnitSystem::Metric);
    assert_eq!(decoded.country_code, *b"US");
}

// ─── Shortcut Button / ISB status layout ─────────────────────────────
//
// AgIsoStack `isb_tests.cpp` uses PGN 0xFD02 with bytes 0..=5 set to 0xFF,
// byte 6 as transition counter, and byte 7 as the stop/permit state. Its
// receiver accepts the low two state bits from incoming frames, while its
// transmitter emits the high six reserved bits as ones.

#[test]
fn shortcut_button_layout_matches_agisostack_isb_examples() {
    assert_eq!(
        Identifier::encode(Priority::Default, PGN_SHORTCUT_BUTTON, 0x74, 0xFF).raw,
        0x18FD_0274
    );

    assert_eq!(
        shortcut_button::encode(ShortcutButtonState::StopImplementOperations),
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xFC]
    );
    assert_eq!(
        shortcut_button::encode_with_transition_count(
            ShortcutButtonState::PermitAllImplementsToOperate,
            9,
        ),
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x09, 0xFD]
    );

    let stop_rx = Message::new(
        PGN_SHORTCUT_BUTTON,
        vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00],
        0x74,
    );
    assert_eq!(
        shortcut_button::decode_message(&stop_rx),
        Some(ShortcutButtonMessage {
            state: ShortcutButtonState::StopImplementOperations,
            transition_count: 0,
        })
    );

    let permit_rx = Message::new(
        PGN_SHORTCUT_BUTTON,
        vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x09, 0x01],
        0x74,
    );
    assert_eq!(
        shortcut_button::decode_message(&permit_rx),
        Some(ShortcutButtonMessage {
            state: ShortcutButtonState::PermitAllImplementsToOperate,
            transition_count: 9,
        })
    );
}

// ─── Speed/distance PGN wire layout ───────────────────────────────────
//
// AgIsoStack `speed_distance_message_tests.cpp` uses raw mm/s and mm
// values below while checking Machine Selected Speed, Wheel Based Speed,
// and Ground Based Speed frame layouts.

#[test]
fn speed_distance_layout_matches_agisostack_speed_message_examples() {
    let machine_selected = MachineSelectedSpeedFull {
        speed_mps: 1.0,
        distance_m: 123.456,
        direction: MachineDirection::Forward,
        source: SpeedSource::NavigationBased,
        limit_status: 3,
        exit_code: 15,
    };
    assert_eq!(
        Identifier::encode(Priority::Normal, PGN_MACHINE_SPEED, 0x45, 0xFF).raw,
        0x0CF0_2245
    );
    assert_eq!(
        machine_selected.encode(),
        [0xE8, 0x03, 0x40, 0xE2, 0x01, 0x00, 15, 0x39]
    );

    let wheel = WheelBasedSpeedDist {
        speed_mps: 9.876,
        distance_m: 5.0,
        direction: MachineDirection::Reverse,
        max_power_time_min: 3,
        key_switch_state: 1,
        implement_start_stop_operations_state: 1,
        operator_direction_reversed_state: 0,
    };
    assert_eq!(
        Identifier::encode(Priority::Normal, PGN_WHEEL_BASED_SPEED_DIST, 0x45, 0xFF).raw,
        0x0CFE_4845
    );
    assert_eq!(
        wheel.encode(),
        [0x94, 0x26, 0x88, 0x13, 0x00, 0x00, 3, 0x14]
    );

    let ground = GroundBasedSpeedDist {
        speed_mps: 9.999,
        distance_m: 80.0,
        direction: MachineDirection::Forward,
    };
    assert_eq!(
        Identifier::encode(Priority::Normal, PGN_GROUND_BASED_SPEED_DIST, 0x45, 0xFF).raw,
        0x0CFE_4945
    );
    assert_eq!(
        ground.encode(),
        [0x0F, 0x27, 0x80, 0x38, 0x01, 0x00, 0xFF, 0x01]
    );
}

#[test]
fn j1939_speed_distance_measurement_prefix_decodes_agisostack_speed_examples() {
    let machine_selected = Message::new(
        PGN_MACHINE_SPEED,
        vec![0xE8, 0x03, 0x40, 0xE2, 0x01, 0x00, 15, 0x39],
        0x45,
    );
    let decoded = SpeedAndDistance::from_message(&machine_selected).unwrap();
    assert_eq!(decoded.speed_mps, Some(1.0));
    assert!((decoded.distance_m.unwrap() - 123.456).abs() < 1e-12);

    let wheel = Message::new(
        PGN_WHEEL_BASED_SPEED_DIST,
        vec![0x94, 0x26, 0x88, 0x13, 0x00, 0x00, 3, 0x14],
        0x45,
    );
    let decoded = SpeedAndDistance::from_message(&wheel).unwrap();
    assert!((decoded.speed_mps.unwrap() - 9.876).abs() < 1e-12);
    assert_eq!(decoded.distance_m, Some(5.0));

    let ground = Message::new(
        PGN_GROUND_BASED_SPEED_DIST,
        vec![0x0F, 0x27, 0x80, 0x38, 0x01, 0x00, 0xFF, 0x01],
        0x45,
    );
    let decoded = SpeedAndDistance::from_message(&ground).unwrap();
    assert!((decoded.speed_mps.unwrap() - 9.999).abs() < 1e-12);
    assert_eq!(decoded.distance_m, Some(80.0));
}

// ─── Agricultural guidance wire layout ────────────────────────────────
//
// AgIsoStack `guidance_tests.cpp` sends Agricultural Guidance Machine Info
// on PGN 0xAC00 and Guidance System Command on PGN 0xAD00 at priority 3.
// Its payload checks pin the same 0.25 km^-1/bit, -8032 km^-1 offset,
// byte-2 status packing, byte-3 guidance limit bits, byte-4 exit/engage
// bits, and all-ones tail padding.

#[test]
fn agricultural_guidance_layout_matches_agisostack_examples() {
    assert_eq!(
        Identifier::encode(Priority::Normal, PGN_GUIDANCE_MACHINE_INFO, 0x44, 0xFF).raw,
        0x0CAC_FF44
    );
    assert_eq!(
        Identifier::encode(Priority::Normal, PGN_GUIDANCE_SYSTEM_CMD, 0x46, 0xFF).raw,
        0x0CAD_FF46
    );

    let machine = GuidanceMachineInfo {
        estimated_curvature: 10.0,
        lockout: MechanicalLockout::NotActive,
        steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
        steering_input_position_status: GenericSaeBs02SlotValue::DisabledOffPassive,
        request_reset_status: RequestResetCommandStatus::ResetNotRequired,
        guidance_limit_status: GuidanceLimitStatus::LimitedLow,
        guidance_system_command_exit_reason_code: 27,
        remote_engage_switch_status: GenericSaeBs02SlotValue::EnabledOnActive,
    };
    let machine_bytes = machine.encode();
    assert_eq!(
        machine_bytes,
        [0xA8, 0x7D, 0x04, 0x60, 0x5B, 0xFF, 0xFF, 0xFF]
    );
    let decoded_machine = GuidanceMachineInfo::decode(&machine_bytes).unwrap();
    assert!((decoded_machine.estimated_curvature - 10.0).abs() < 0.25);
    assert_eq!(
        decoded_machine.guidance_limit_status,
        GuidanceLimitStatus::LimitedLow
    );
    assert_eq!(
        decoded_machine.steering_input_position_status,
        GenericSaeBs02SlotValue::DisabledOffPassive
    );
    assert_eq!(
        decoded_machine.steering_system_readiness_state,
        GenericSaeBs02SlotValue::EnabledOnActive
    );
    assert_eq!(
        decoded_machine.remote_engage_switch_status,
        GenericSaeBs02SlotValue::EnabledOnActive
    );
    assert_eq!(decoded_machine.lockout, MechanicalLockout::NotActive);
    assert_eq!(
        decoded_machine.request_reset_status,
        RequestResetCommandStatus::ResetNotRequired
    );

    let command = GuidanceSystemCmd {
        commanded_curvature: -43.4,
        status: CurvatureCommandStatus::IntendedToSteer,
    };
    let command_bytes = command.encode();
    assert_eq!(
        command_bytes,
        [0xD2, 0x7C, 0xFD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    let decoded_command = GuidanceSystemCmd::decode(&command_bytes).unwrap();
    assert!((decoded_command.commanded_curvature - -43.5).abs() < 0.25);
    assert_eq!(
        decoded_command.status,
        CurvatureCommandStatus::IntendedToSteer
    );
}

#[test]
fn agricultural_guidance_listen_only_examples_decode_agisostack_payloads() {
    let command_payload = [0xF9, 0x7E, 0xFD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let command = GuidanceSystemCmd::decode(&command_payload).unwrap();
    assert!((command.commanded_curvature - 94.25).abs() < 0.25);
    assert_eq!(command.status, CurvatureCommandStatus::IntendedToSteer);

    let machine_payload = [0xC1, 0x7C, 0x55, 0xE0, 0x64, 0xFF, 0xFF, 0xFF];
    let machine = GuidanceMachineInfo::decode(&machine_payload).unwrap();
    assert!((machine.estimated_curvature - -47.75).abs() < 0.25);
    assert_eq!(
        machine.guidance_limit_status,
        GuidanceLimitStatus::NotAvailable
    );
    assert_eq!(
        machine.steering_input_position_status,
        GenericSaeBs02SlotValue::EnabledOnActive
    );
    assert_eq!(
        machine.steering_system_readiness_state,
        GenericSaeBs02SlotValue::EnabledOnActive
    );
    assert_eq!(
        machine.remote_engage_switch_status,
        GenericSaeBs02SlotValue::EnabledOnActive
    );
    assert_eq!(machine.lockout, MechanicalLockout::Active);
    assert_eq!(
        machine.request_reset_status,
        RequestResetCommandStatus::ResetRequired
    );
    assert_eq!(machine.guidance_system_command_exit_reason_code, 36);
}

// ─── Maintain Power wire layout ───────────────────────────────────────
//
// AgIsoStack `maintain_power_tests.cpp::MessageParsing/MessageEncoding`
// uses 0x5F 0x55 FF.. for all active states plus two-second PWR/ECU_PWR
// requests, and 0x0F 0x00 FF.. for all inactive/no-request states.

#[test]
fn maintain_power_layout_matches_agisostack_message_examples() {
    let active_request = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::Active,
        implement_ready_to_work_state: MaintainPowerState::Active,
        implement_transport_state: MaintainPowerState::Active,
        maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        timestamp_us: 0,
    };
    assert_eq!(
        active_request.encode(),
        [0x5F, 0x55, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        MaintainPowerData::decode(&active_request.encode()),
        Some(active_request)
    );

    let inactive = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Inactive,
        implement_park_state: MaintainPowerState::Inactive,
        implement_ready_to_work_state: MaintainPowerState::Inactive,
        implement_transport_state: MaintainPowerState::Inactive,
        maintain_actuator_power: MaintainPowerRequirement::NoFurtherRequirement,
        maintain_ecu_power: MaintainPowerRequirement::NoFurtherRequirement,
        timestamp_us: 0,
    };
    assert_eq!(
        inactive.encode(),
        [0x0F, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        MaintainPowerData::decode(&inactive.encode()),
        Some(inactive)
    );
}

// ─── ISO 11783-10 DDOP object pool subset ───────────────────────────
//
// AgIsoStack `ddop_tests.cpp::CreateSprayerDDOP` builds a sprayer object pool,
// serializes/deserializes it, then checks selected object IDs and fields:
// DVC 0, DET 1/4, DPT 14/15, DVP 90, and DPD 85. The Rust DDOP stays stricter
// on text length/encoding, but these selected object semantics and binary
// round trip must stay aligned.

#[test]
fn tc_ddop_sprayer_subset_matches_agisostack_create_sprayer_expectations() {
    let mut ddop = DDOP::default();
    ddop.add_device(
        DeviceObject::default()
            .with_id(0)
            .with_designator("AgIsoStack++ UnitTest")
            .with_software_version("1.0.0")
            .with_serial_number("123")
            .with_structure_label(*b"I++1.0 ")
            .with_localization_label([b'e', b'n', 0x0F, 0x04, 0x5A, 0x04, b'U']),
    )
    .unwrap();
    ddop.add_element(
        DeviceElement::default()
            .with_id(1)
            .with_type(DeviceElementType::Device)
            .with_number(1)
            .with_parent(0)
            .with_designator("Sprayer"),
    )
    .unwrap();
    ddop.add_element(
        DeviceElement::default()
            .with_id(4)
            .with_type(DeviceElementType::Connector)
            .with_number(4)
            .with_parent(1)
            .with_designator("Connector"),
    )
    .unwrap();
    ddop.add_property(
        DeviceProperty::default()
            .with_id(14)
            .with_ddi(134)
            .with_value(0)
            .with_presentation(88)
            .with_designator("Offset X"),
    )
    .unwrap();
    ddop.add_property(
        DeviceProperty::default()
            .with_id(15)
            .with_ddi(135)
            .with_value(0)
            .with_presentation(88)
            .with_designator("Offset Y"),
    )
    .unwrap();
    ddop.add_value_presentation(
        DeviceValuePresentation::default()
            .with_id(88)
            .with_unit("mm")
            .with_offset(0)
            .with_scale(1.0)
            .with_decimals(0),
    )
    .unwrap();
    ddop.add_value_presentation(
        DeviceValuePresentation::default()
            .with_id(90)
            .with_unit("L")
            .with_offset(0)
            .with_scale(0.001)
            .with_decimals(0),
    )
    .unwrap();
    ddop.add_process_data(
        DeviceProcessData::default()
            .with_id(85)
            .with_ddi(72)
            .with_trigger(TriggerMethod::TimeInterval)
            .with_presentation(90)
            .with_designator("Tank Volume"),
    )
    .unwrap();
    ddop.validate().unwrap();

    let binary = ddop.serialize().unwrap();
    let restored = DDOP::deserialize(&binary).unwrap();
    restored.validate().unwrap();

    let device = restored
        .devices()
        .iter()
        .find(|obj| obj.id == ObjectID(0))
        .expect("DVC object 0");
    assert_eq!(device.designator, "AgIsoStack++ UnitTest");
    assert_eq!(device.serial_number, "123");
    assert_eq!(device.structure_label, *b"I++1.0 ");
    assert_eq!(
        device.localization_label,
        [b'e', b'n', 0x0F, 0x04, 0x5A, 0x04, b'U']
    );

    let sprayer = restored
        .elements()
        .iter()
        .find(|obj| obj.id == ObjectID(1))
        .expect("DET object 1");
    assert_eq!(sprayer.designator, "Sprayer");
    assert_eq!(sprayer.number.raw(), 1);
    assert_eq!(sprayer.parent_id, ObjectID(0));
    assert!(sprayer.child_objects.is_empty());

    let connector = restored
        .elements()
        .iter()
        .find(|obj| obj.id == ObjectID(4))
        .expect("DET object 4");
    assert_eq!(connector.designator, "Connector");
    assert_eq!(connector.number.raw(), 4);
    assert_eq!(connector.parent_id, ObjectID(1));
    assert!(connector.child_objects.is_empty());

    let offset_x = restored
        .properties()
        .iter()
        .find(|obj| obj.id == ObjectID(14))
        .expect("DPT object 14");
    assert_eq!(offset_x.designator, "Offset X");
    assert_eq!(offset_x.ddi, DDI(134));
    assert_eq!(offset_x.presentation_object_id, ObjectID(88));

    let offset_y = restored
        .properties()
        .iter()
        .find(|obj| obj.id == ObjectID(15))
        .expect("DPT object 15");
    assert_eq!(offset_y.designator, "Offset Y");
    assert_eq!(offset_y.ddi, DDI(135));
    assert_eq!(offset_y.presentation_object_id, ObjectID(88));

    let volume = restored
        .value_presentations()
        .iter()
        .find(|obj| obj.id == ObjectID(90))
        .expect("DVP object 90");
    assert_eq!(volume.unit_designator, "L");
    assert_eq!(volume.decimal_digits, 0);
    assert!((volume.scale - 0.001).abs() < 0.001);

    let tank_volume = restored
        .process_data()
        .iter()
        .find(|obj| obj.id == ObjectID(85))
        .expect("DPD object 85");
    assert_eq!(tank_volume.designator, "Tank Volume");
    assert_eq!(tank_volume.ddi, DDI(72));
    assert_eq!(
        tank_volume.trigger_methods,
        TriggerMethod::TimeInterval.as_u8()
    );
}

// AgIsoStack `tc_server_tests.cpp::DDOPHelper_NoFunctions` builds a flat
// no-function DDOP with two sections and property-backed rate defaults. Our
// helper API exposes a lighter flat geometry/rate view, but the section
// offsets, widths, and fixed non-editable rate values must match that example.

