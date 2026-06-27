//! Property-style fuzz entry points for externally-fed decoders.
//!
//! These are intentionally part of the normal `make test` lane: they are not a
//! replacement for long-running libFuzzer jobs, but they keep the same fuzz
//! surfaces compiled and exercised on arbitrary bytes in every local/CI gate.

use machbus::isobus::fs::{
    CCMMessage, FileServerProperties, FileServerPropertiesV2, FileServerStatus, VolumeStatus,
};
use machbus::isobus::tc::DDOP;
use machbus::isobus::vt::ObjectPool as VTObjectPool;
use machbus::isobus::{AuxNFunction, AuxOFunction, Functionalities, GroupFunctionMsg};
use machbus::j1939::{
    DiagnosticLamps, DiagnosticProtocolId, Dm4Message, Dm7Command, Dm8TestResult, Dm13Signals,
    Dm20Response, Dm21Readiness, Dm22Message, Dm25Request, DmClearAllRequest, DmDtcList, Dtc,
    FreezeFrame, HeartbeatRequest, LanguageData, MaintainPowerData, ProductIdentification,
    Request2Msg, SoftwareIdentification, TransferMsg, shortcut_button,
};
use machbus::net::pgn_defs::{
    PGN_ACKNOWLEDGMENT, PGN_AUX_INPUT_STATUS, PGN_AUX_INPUT_TYPE2, PGN_ETP_CM, PGN_ETP_DT,
    PGN_LANGUAGE_COMMAND, PGN_MAINTAIN_POWER, PGN_NAME_MANAGEMENT, PGN_SHORTCUT_BUTTON, PGN_TP_CM,
    PGN_TP_DT,
};
use machbus::net::{
    BROADCAST_ADDRESS, DataSpan, ExtendedTransportProtocol, FastPacketProtocol, FilterRule, Frame,
    Identifier, Message, Name, NameManagementMsg, NiuNetworkMsg, Priority, TransportProtocol,
    parse_iop_data,
};
use machbus::nmea::SerialGNSS;
use proptest::prelude::*;
use wirebit::can::CanFrame;

fn padded_frame(pgn: u32, data: &[u8], source: u8, destination: u8) -> Frame {
    let mut payload = [0xFFu8; 8];
    let len = data.len().min(8);
    payload[..len].copy_from_slice(&data[..len]);
    Frame::new(
        Identifier::encode(Priority::Default, pgn, source, destination),
        payload,
        len as u8,
    )
}

fn exercise_span(span: DataSpan<'_>, index: usize) {
    let _ = span.as_slice();
    let _ = span.data_ptr();
    let _ = span.size();
    let _ = span.is_empty();
    let _ = span.subspan(index, usize::MAX);
    let _ = span.subspan_from(index);
    let _ = span.at(index);
    let _ = span.get_u8(index);
    let _ = span.get_u16_le(index);
    let _ = span.get_u32_le(index);
    let _ = span.get_u64_le(index);
    let _ = span.get_bit(index, (index & 0x07) as u8);
}

fn exercise_diagnostics(data: &[u8]) {
    let _ = Dtc::decode(data);
    let _ = DiagnosticLamps::decode(data);
    let _ = DmDtcList::decode(data);
    let _ = DmClearAllRequest::decode(data);
    let _ = Dm4Message::decode(data);
    let _ = DiagnosticProtocolId::decode(data);
    let _ = Dm7Command::decode(data);
    let _ = Dm8TestResult::decode(data);
    let _ = Dm13Signals::decode(data);
    let _ = Dm20Response::decode(data);
    let _ = Dm21Readiness::decode(data);
    let _ = Dm22Message::decode(data);
    let _ = Dm25Request::decode(data);
    let _ = ProductIdentification::decode(data);
    let _ = SoftwareIdentification::decode(data);
    let _ = FreezeFrame::decode(data);
}

fn exercise_file_server(data: &[u8]) {
    let _ = FileServerProperties::decode(data);
    let _ = FileServerStatus::decode(data);
    let _ = CCMMessage::decode(data);
    let _ = FileServerPropertiesV2::decode(data);
    let _ = VolumeStatus::decode(data);
}

fn exercise_utility_protocols(data: &[u8], source: u8) {
    let group_msg = Message::new(PGN_ACKNOWLEDGMENT, data.to_vec(), source);
    let language_msg = Message::new(PGN_LANGUAGE_COMMAND, data.to_vec(), source);
    let shortcut_msg = Message::new(PGN_SHORTCUT_BUTTON, data.to_vec(), source);
    let maintain_msg = Message::new(PGN_MAINTAIN_POWER, data.to_vec(), source);
    let aux_o_msg = Message::new(PGN_AUX_INPUT_STATUS, data.to_vec(), source);
    let aux_n_msg = Message::new(PGN_AUX_INPUT_TYPE2, data.to_vec(), source);
    let name_mgmt_msg = Message::new(PGN_NAME_MANAGEMENT, data.to_vec(), source);

    if let Some(decoded) = GroupFunctionMsg::decode(&group_msg.data)
        && let Ok(encoded) = decoded.encode()
    {
        let _ = GroupFunctionMsg::decode(&encoded);
    }
    let _ = Functionalities::decode(data);

    if let Some(decoded) = Request2Msg::decode(data)
        && let Ok(encoded) = decoded.encode()
    {
        let _ = Request2Msg::decode(&encoded);
    }
    if let Some(decoded) = TransferMsg::decode(data)
        && let Ok(encoded) = decoded.encode()
    {
        let _ = TransferMsg::decode(&encoded);
    }

    if let Some(decoded) = NameManagementMsg::decode(&name_mgmt_msg.data) {
        let _ = NameManagementMsg::decode(&decoded.encode());
    }
    if let Some(decoded) = LanguageData::decode(&language_msg) {
        let _ = LanguageData::decode(&Message::new(
            PGN_LANGUAGE_COMMAND,
            decoded.encode().to_vec(),
            source,
        ));
    }
    let _ = shortcut_button::decode_message(&shortcut_msg);
    let _ = shortcut_button::decode(&shortcut_msg);
    let _ = HeartbeatRequest::decode(data);
    if let Some(decoded) = MaintainPowerData::from_message(&maintain_msg) {
        let _ = MaintainPowerData::decode(&decoded.encode());
    }
    if let Some(decoded) = AuxOFunction::decode(&aux_o_msg) {
        let _ = AuxOFunction::decode(&Message::new(
            PGN_AUX_INPUT_STATUS,
            decoded.encode().to_vec(),
            source,
        ));
    }
    if let Some(decoded) = AuxNFunction::decode(&aux_n_msg) {
        let _ = AuxNFunction::decode(&Message::new(
            PGN_AUX_INPUT_TYPE2,
            decoded.encode().to_vec(),
            source,
        ));
    }

    let _ = FilterRule::decode(data);
    let _ = NiuNetworkMsg::decode(data);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 96,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    #[test]
    fn proptest_external_decoder_fuzz_surfaces_are_bounded(
        data in proptest::collection::vec(any::<u8>(), 0..=1024),
        raw_id in any::<u32>(),
        source in 0u8..=0xFD,
        destination in 0u8..=0xFF,
        index in any::<usize>(),
    ) {
        let span = DataSpan::new(&data);
        exercise_span(span, index);

        let identifier = Identifier::from_raw(raw_id);
        let _ = identifier.raw;
        let _ = identifier.pgn();
        let _ = identifier.source();
        let _ = identifier.destination();
        let _ = Name::from_bytes(&data);

        let can = CanFrame::make_ext(raw_id, &data);
        let _ = Frame::from_can_frame(&can);
        let standard = CanFrame::make_std(raw_id, &data);
        prop_assert!(Frame::from_can_frame(&standard).is_none());

        let tp_cm = padded_frame(PGN_TP_CM, &data, source, destination);
        let tp_dt = padded_frame(PGN_TP_DT, &data, source, destination);
        let etp_cm = padded_frame(PGN_ETP_CM, &data, source, destination);
        let etp_dt = padded_frame(PGN_ETP_DT, &data, source, destination);
        let fp = padded_frame(0x1F805, &data, source, BROADCAST_ADDRESS);

        let mut tp = TransportProtocol::with_max_sessions(2);
        let mut etp = ExtendedTransportProtocol::with_max_sessions(2);
        let mut fast_packet = FastPacketProtocol::with_max_rx_sessions(2);

        let _ = tp.process_frame(&tp_cm, 0);
        let _ = tp.process_frame(&tp_dt, 0);
        let _ = etp.process_frame(&etp_cm, 0);
        let _ = etp.process_frame(&etp_dt, 0);
        let completed = fast_packet.process_frame(&fp);

        let _ = tp.max_sessions();
        let _ = etp.max_sessions();
        prop_assert!(fast_packet.rx_session_count() <= fast_packet.max_rx_sessions());
        if let Some(message) = completed {
            prop_assert!(message.data.len() <= 223);
        }

        let _ = VTObjectPool::deserialize(&data);
        let _ = DDOP::deserialize(&data);
        exercise_file_server(&data);
        exercise_diagnostics(&data);
        exercise_utility_protocols(&data, source);
        let _ = parse_iop_data(&data);

        let mut serial = SerialGNSS::new();
        serial.feed_bytes(&data);
        if let Some(position) = serial.latest_position() {
            prop_assert!(position.wgs.latitude.is_finite());
            prop_assert!(position.wgs.longitude.is_finite());
            prop_assert!(position.wgs.altitude.is_finite());
        }
    }
}
