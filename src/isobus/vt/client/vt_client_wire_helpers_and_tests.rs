fn object_pool_transfer_settle_ms(transfer_len: usize, timeout_ms: u32) -> u32 {
    let packet_count = transfer_len.saturating_add(6) / 7;
    let delay = u32::try_from(packet_count)
        .unwrap_or(u32::MAX)
        .saturating_add(2)
        .saturating_mul(VT_OBJECT_POOL_TRANSFER_SETTLE_PER_PACKET_MS)
        .max(VT_OBJECT_POOL_TRANSFER_SETTLE_MIN_MS);
    if timeout_ms == 0 {
        delay
    } else {
        delay.min(timeout_ms.saturating_sub(1))
    }
}

fn serialize_pool_for_vt_transfer(pool: &ObjectPool) -> Result<(Vec<u8>, u32)> {
    let serialized = pool.serialize()?;
    if serialized.is_empty() {
        return Err(Error::invalid_state("object pool serializes to empty"));
    }
    let size = u32::try_from(serialized.len()).map_err(|_| {
        Error::invalid_data("VT object pool serialized length exceeds u32 GetMemory field")
    })?;
    Ok((serialized, size))
}

#[inline]
fn u16_le(buf: &[u8]) -> u16 {
    (buf[0] as u16) | ((buf[1] as u16) << 8)
}

#[inline]
fn u32_le(buf: &[u8]) -> u32 {
    (buf[0] as u32) | ((buf[1] as u32) << 8) | ((buf[2] as u32) << 16) | ((buf[3] as u32) << 24)
}

fn classic_version_list_response_is_canonical(data: &[u8]) -> bool {
    const LABEL_SIZE: usize = 7;
    if data.len() < 2 {
        return false;
    }
    version_list_response_is_canonical(data, 2, data[1] as usize, LABEL_SIZE)
}

fn extended_version_list_response_is_canonical(data: &[u8]) -> bool {
    if data.len() < 3 {
        return false;
    }
    version_list_response_is_canonical(data, 3, data[2] as usize, cmd::EXTENDED_VERSION_LABEL_SIZE)
}

fn version_list_response_is_canonical(
    data: &[u8],
    labels_offset: usize,
    label_count: usize,
    label_size: usize,
) -> bool {
    let Some(label_bytes) = label_count.checked_mul(label_size) else {
        return false;
    };
    let Some(required_len) = labels_offset.checked_add(label_bytes) else {
        return false;
    };

    if required_len < 8 {
        if data.len() != 8 || data[required_len..].iter().any(|&b| b != 0xFF) {
            return false;
        }
    } else {
        if data.len() != required_len {
            return false;
        }
    }

    let label_name = if label_size == cmd::CLASSIC_VERSION_LABEL_SIZE {
        "classic VT version label"
    } else {
        "extended VT version label"
    };
    data[labels_offset..required_len]
        .chunks_exact(label_size)
        .all(|label| decode_padded_version_label(label, label_size, label_name).is_some())
}

fn decode_padded_version_label(field: &[u8], max_len: usize, name: &str) -> Option<String> {
    if field.len() != max_len {
        return None;
    }
    let label_len = field
        .iter()
        .position(|&byte| byte == b' ' || byte == 0)
        .unwrap_or(field.len());
    if field[label_len..]
        .iter()
        .any(|&byte| byte != b' ' && byte != 0)
    {
        return None;
    }
    let label = core::str::from_utf8(&field[..label_len]).ok()?;
    validate_version_label(label, max_len, name).ok()?;
    Some(label.to_owned())
}

fn version_operation_response_is_canonical(data: &[u8]) -> bool {
    data.len() == 8 && data[3..].iter().all(|&byte| byte == 0xFF)
}

#[cfg(test)]
mod tests {
    use super::super::objects::{
        DataMaskBody, WorkingSetBody, create_data_mask, create_working_set,
    };
    use super::super::server::VTServer;
    use super::*;
    use crate::net::pgn_defs::{PGN_LANGUAGE_COMMAND, PGN_VT_TO_ECU};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn vt_msg(data: Vec<u8>, src: Address) -> Message {
        Message::new(PGN_VT_TO_ECU, data, src)
    }

    fn fixed_response(function: u8, status: u8) -> Vec<u8> {
        let mut data = [0xFFu8; 8];
        data[0] = function;
        data[1] = status;
        data.to_vec()
    }

    fn end_of_pool_response(status: u8, pool_error_bitmask: u8) -> Vec<u8> {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::END_OF_POOL;
        data[1] = status;
        data[6] = pool_error_bitmask;
        data.to_vec()
    }

    fn dummy_pool() -> ObjectPool {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
    }

    fn unserializable_pool() -> ObjectPool {
        let too_many_children: Vec<u16> = (0..=u16::MAX).collect();
        ObjectPool::default().with_object(
            create_working_set(1, &WorkingSetBody::default()).with_children(too_many_children),
        )
    }

    #[test]
    fn connect_requires_pool() {
        let mut c = VTClient::new(VTClientConfig::default());
        assert!(c.connect().is_err());
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();
        assert_eq!(c.state(), VTState::WaitForVTStatus);
    }

    #[test]
    fn connect_rejects_unserializable_pool_before_get_memory() {
        let mut c = VTClient::new(VTClientConfig::default());
        c.set_object_pool(unserializable_pool());

        assert!(c.connect().is_err());
        assert_eq!(c.state(), VTState::Disconnected);
    }

    #[test]
    fn update_does_not_emit_zero_size_get_memory_for_unserializable_pool() {
        let mut c = VTClient::new(VTClientConfig::default());
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();

        let mut data = vec![cmd::VT_STATUS];
        data.resize(8, 0xFF);
        data[6] = 4;
        c.handle_vt_message(&vt_msg(data, 0x80));
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(c.state(), VTState::SendGetMemory);

        c.set_object_pool(unserializable_pool());
        let out = c.update(1);
        assert!(out.is_empty());
        assert_eq!(c.state(), VTState::Disconnected);
    }

    #[test]
    fn vt_status_drives_to_send_ws_master() {
        let mut c = VTClient::new(VTClientConfig::default());
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();
        // VT_STATUS payload: [func, active_ws, ..., version=4]
        let mut data = vec![cmd::VT_STATUS, 0xFF, 0, 0, 0, 0, 4u8, 0xFF];
        data.resize(8, 0xFF);
        c.handle_vt_message(&vt_msg(data, 0x80));
        assert_eq!(c.state(), VTState::SendWorkingSetMaster);
        assert_eq!(c.vt_address(), 0x80);
        assert_eq!(c.vt_version_value(), 4);
    }

    #[test]
    fn full_connect_flow_emits_expected_outbounds() {
        let mut c = VTClient::new(VTClientConfig::default());
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();
        // 1. VT status arrives ⇒ state = SendWorkingSetMaster.
        let mut data = vec![cmd::VT_STATUS];
        data.resize(8, 0xFF);
        data[6] = 4;
        c.handle_vt_message(&vt_msg(data, 0x80));
        // 2. update() emits Working Set Master frame, transitions to
        //    SendGetMemory.
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pgn, PGN_WORKING_SET_MASTER);
        assert_eq!(c.state(), VTState::SendGetMemory);
        // 3. update() emits GET_MEMORY, transitions to WaitForMemory.
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pgn, PGN_ECU_TO_VT);
        assert_eq!(out[0].data[0], cmd::GET_MEMORY);
        assert_eq!(c.state(), VTState::WaitForMemory);
        // 4. GET_MEMORY_RESPONSE with success ⇒ state = UploadPool.
        c.handle_vt_message(&vt_msg(
            fixed_response(cmd::GET_MEMORY_RESPONSE, 0x00),
            0x80,
        ));
        assert_eq!(c.state(), VTState::UploadPool);
        // 5. update() emits Object Pool Transfer, then waits long enough for
        //    TP-backed uploads to drain before EndOfObjectPool.
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], cmd::OBJECT_POOL_TRANSFER);
        assert_eq!(c.state(), VTState::WaitForPoolStore);
        let out = c.update(1_000);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], cmd::END_OF_POOL);
        assert_eq!(c.state(), VTState::WaitForEndOfPool);
        // 6. END_OF_POOL response with success ⇒ Connected.
        c.handle_vt_message(&vt_msg(end_of_pool_response(0x00, 0x00), 0x80));
        assert_eq!(c.state(), VTState::Connected);
    }

    #[test]
    fn end_of_pool_response_checks_error_bitmask() {
        let mut c = VTClient::new(VTClientConfig::default());
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();
        let mut data = vec![cmd::VT_STATUS];
        data.resize(8, 0xFF);
        c.handle_vt_message(&vt_msg(data, 0x80));
        let _ = c.update(1);
        let _ = c.update(1);
        c.handle_vt_message(&vt_msg(
            fixed_response(cmd::GET_MEMORY_RESPONSE, 0x00),
            0x80,
        ));
        let _ = c.update(1);
        let _ = c.update(1_000);
        assert_eq!(c.state(), VTState::WaitForEndOfPool);

        c.handle_vt_message(&vt_msg(
            vec![cmd::END_OF_POOL, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x02, 0xFF],
            0x80,
        ));
        assert_eq!(c.state(), VTState::Disconnected);

        c.connect().unwrap();
        let mut data = vec![cmd::VT_STATUS];
        data.resize(8, 0xFF);
        c.handle_vt_message(&vt_msg(data, 0x80));
        let _ = c.update(1);
        let _ = c.update(1);
        c.handle_vt_message(&vt_msg(
            fixed_response(cmd::GET_MEMORY_RESPONSE, 0x00),
            0x80,
        ));
        let _ = c.update(1);
        let _ = c.update(1_000);
        c.handle_vt_message(&vt_msg(
            vec![cmd::END_OF_POOL, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xFF],
            0x80,
        ));
        assert_eq!(c.state(), VTState::Connected);
    }

    #[test]
    fn timeout_disconnects_when_waiting_for_vt_status() {
        let mut c = VTClient::new(VTClientConfig::default().with_timeout(100));
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();
        c.update(50);
        assert_eq!(c.state(), VTState::WaitForVTStatus);
        c.update(60);
        assert_eq!(c.state(), VTState::Disconnected);
    }

    #[test]
    fn commands_error_when_not_connected() {
        let c = VTClient::new(VTClientConfig::default());
        assert!(c.hide_show(1, true).is_err());
        assert!(c.change_numeric_value(1, 0).is_err());
        assert!(c.change_string_value(1, "x").is_err());
    }

    fn force_connected(c: &mut VTClient) {
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();
        let mut data = vec![cmd::VT_STATUS];
        data.resize(8, 0xFF);
        data[6] = 4;
        c.handle_vt_message(&vt_msg(data, 0x80));
        // Drive through the state machine.
        let _ = c.update(1);
        let _ = c.update(1);
        c.handle_vt_message(&vt_msg(
            fixed_response(cmd::GET_MEMORY_RESPONSE, 0x00),
            0x80,
        ));
        let _ = c.update(1);
        let _ = c.update(1_000);
        c.handle_vt_message(&vt_msg(end_of_pool_response(0x00, 0x00), 0x80));
        assert_eq!(c.state(), VTState::Connected);
    }

    #[test]
    fn change_numeric_value_layout() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);
        let out = c.change_numeric_value(0x1234, 0xDEADBEEF).unwrap();
        assert_eq!(out.pgn, PGN_ECU_TO_VT);
        assert_eq!(out.dest, Some(0x80));
        assert_eq!(out.data[0], cmd::CHANGE_NUMERIC_VALUE);
        assert_eq!(u16_le(&out.data[1..]), 0x1234);
        assert_eq!(u32_le(&out.data[4..]), 0xDEADBEEF);
    }

    #[test]
    fn inbound_numeric_value_change_uses_reserved_byte_layout() {
        let mut c = VTClient::new(VTClientConfig::default());
        let log: Rc<RefCell<Vec<(ObjectID, u32)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_numeric_value_change
            .subscribe(move |&v| lc.borrow_mut().push(v));

        let mut data = vec![cmd::NUMERIC_VALUE_CHANGE, 0x34, 0x12, 0xFF];
        data.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        c.handle_vt_message(&vt_msg(data, 0x80));

        assert_eq!(*log.borrow(), vec![(ObjectID(0x1234), 0xDEAD_BEEF)]);

        let mut bad_reserved = vec![cmd::NUMERIC_VALUE_CHANGE, 0x34, 0x12, 0x00];
        bad_reserved.extend_from_slice(&0x8765_4321u32.to_le_bytes());
        c.handle_vt_message(&vt_msg(bad_reserved, 0x80));
        assert_eq!(
            *log.borrow(),
            vec![(ObjectID(0x1234), 0xDEAD_BEEF)],
            "bad reserved byte must be ignored"
        );
    }

    #[test]
    fn inbound_string_value_change_rejects_truncated_declared_length() {
        let mut c = VTClient::new(VTClientConfig::default());
        let log: Rc<RefCell<Vec<(ObjectID, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_string_value_change
            .subscribe(move |v| lc.borrow_mut().push(v.clone()));

        c.handle_vt_message(&vt_msg(
            vec![cmd::STRING_VALUE_CHANGE, 0x05, 0x00, 0x03, 0x00, b'h'],
            0x80,
        ));

        assert!(log.borrow().is_empty());

        c.handle_vt_message(&vt_msg(
            vec![
                cmd::STRING_VALUE_CHANGE,
                0x05,
                0x00,
                0x02,
                0x00,
                b'h',
                b'i',
                0x00,
            ],
            0x80,
        ));
        assert!(
            log.borrow().is_empty(),
            "bad trailing string padding must be ignored"
        );
    }

    #[test]
    fn inbound_string_value_change_preserves_utf8_and_rejects_invalid_utf8() {
        let mut c = VTClient::new(VTClientConfig::default());
        let log: Rc<RefCell<Vec<(ObjectID, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_string_value_change
            .subscribe(move |v| lc.borrow_mut().push(v.clone()));

        let payload = "hé".as_bytes();
        let mut data = vec![cmd::STRING_VALUE_CHANGE, 0x05, 0x00];
        data.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        data.extend_from_slice(payload);
        c.handle_vt_message(&vt_msg(data, 0x80));
        assert_eq!(*log.borrow(), vec![(ObjectID(5), "hé".to_owned())]);

        c.handle_vt_message(&vt_msg(
            vec![
                cmd::STRING_VALUE_CHANGE,
                0x05,
                0x00,
                0x02,
                0x00,
                0xC3,
                0x28,
                0xFF,
            ],
            0x80,
        ));
        assert_eq!(
            *log.borrow(),
            vec![(ObjectID(5), "hé".to_owned())],
            "invalid UTF-8 string payload must be ignored"
        );
    }

    #[test]
    fn change_string_value_pads_to_eight_bytes() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);
        let out = c.change_string_value(7, "é").unwrap();
        assert!(out.data.len() >= 8);
        assert_eq!(out.data[0], cmd::CHANGE_STRING_VALUE);
        assert_eq!(u16_le(&out.data[1..]), 7);
        assert_eq!(u16_le(&out.data[3..]), 2);
        assert_eq!(&out.data[5..7], "é".as_bytes());
        assert_eq!(out.data[7], 0xFF);
    }

    #[test]
    fn change_string_value_rejects_lengths_that_cannot_encode() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);
        let too_long = "x".repeat(VT_STRING_VALUE_MAX_LEN + 1);
        assert!(c.change_string_value(7, &too_long).is_err());
    }

    #[test]
    fn attribute_change_builders_match_agisostack_layout() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);

        let out = c
            .change_font_attributes(0x1234.into(), 5, 6, 1, 0x0A)
            .unwrap();
        assert_eq!(
            out.data,
            vec![cmd::CHANGE_FONT_ATTRIBUTES, 0x34, 0x12, 5, 6, 1, 0x0A, 0xFF]
        );

        let out = c
            .change_line_attributes(0x1234.into(), 5, 3, 0x00C0)
            .unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_LINE_ATTRIBUTES,
                0x34,
                0x12,
                5,
                3,
                0xC0,
                0x00,
                0xFF
            ]
        );

        let out = c
            .change_fill_attributes(0x1234.into(), 2, 7, 0xBEEF.into())
            .unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_FILL_ATTRIBUTES,
                0x34,
                0x12,
                2,
                7,
                0xEF,
                0xBE,
                0xFF
            ]
        );

        let out = c.change_end_point(0x1234.into(), 300, 200, 1).unwrap();
        assert_eq!(
            out.data,
            vec![cmd::CHANGE_END_POINT, 0x34, 0x12, 0x2C, 0x01, 0xC8, 0x00, 1]
        );

        let out = c.change_priority(0x1234.into(), 2).unwrap();
        assert_eq!(
            out.data,
            vec![cmd::CHANGE_PRIORITY, 0x34, 0x12, 2, 0xFF, 0xFF, 0xFF, 0xFF]
        );

        let out = c.change_polygon_point(0x1234.into(), 3, 300, 200).unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_POLYGON_POINT,
                0x34,
                0x12,
                3,
                0x2C,
                0x01,
                0xC8,
                0x00
            ]
        );

        let out = c.change_polygon_scale(0x1234.into(), 300, 200).unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_POLYGON_SCALE,
                0x34,
                0x12,
                0x2C,
                0x01,
                0xC8,
                0x00,
                0xFF
            ]
        );

        let out = c
            .change_object_label(0x1234.into(), 0xBEEF.into(), 1, 0xCAFE.into())
            .unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_OBJECT_LABEL,
                0x34,
                0x12,
                0xEF,
                0xBE,
                1,
                0xFE,
                0xCA
            ]
        );

        let out = c.select_colour_map(0x1234.into()).unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::SELECT_COLOUR_MAP,
                0x34,
                0x12,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );
        let out = c.select_colour_palette(0x5678.into()).unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::SELECT_COLOUR_MAP,
                0x78,
                0x56,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ],
            "Select Colour Palette uses the same standard function code as Select Colour Map"
        );

        // All require an active connection.
        let disconnected = VTClient::new(VTClientConfig::default());
        assert!(disconnected.change_priority(0x1234.into(), 2).is_err());
        assert!(disconnected.select_colour_map(0x1234.into()).is_err());
        assert!(disconnected.select_colour_palette(0x1234.into()).is_err());
    }

    #[test]
    fn soft_key_button_detailed_events_carry_parent_and_key_number() {
        use std::cell::RefCell;
        use std::rc::Rc;
        type DetailedLog = Rc<RefCell<Vec<(ObjectID, ObjectID, u8, ActivationCode)>>>;
        let mut c = VTClient::new(VTClientConfig::default());
        let sk: DetailedLog = Rc::new(RefCell::new(Vec::new()));
        let bt: DetailedLog = Rc::new(RefCell::new(Vec::new()));
        let skc = sk.clone();
        let btc = bt.clone();
        c.on_soft_key_detailed
            .subscribe(move |&v| skc.borrow_mut().push(v));
        c.on_button_detailed
            .subscribe(move |&v| btc.borrow_mut().push(v));

        // Server builder encodes (code, object 0xCAFE, parent 0xBEEF, key# 7).
        c.handle_vt_message(&vt_msg(
            VTServer::build_soft_key_activation(ActivationCode::Held, 0xCAFE, 0xBEEF, 7).to_vec(),
            0x80,
        ));
        c.handle_vt_message(&vt_msg(
            VTServer::build_button_activation(ActivationCode::Pressed, 0xCAFE, 0xBEEF, 7).to_vec(),
            0x80,
        ));

        assert_eq!(
            *sk.borrow(),
            vec![(ObjectID(0xCAFE), ObjectID(0xBEEF), 7, ActivationCode::Held)]
        );
        assert_eq!(
            *bt.borrow(),
            vec![(
                ObjectID(0xCAFE),
                ObjectID(0xBEEF),
                7,
                ActivationCode::Pressed
            )]
        );
    }

    #[test]
    fn vt_technical_data_request_builders_match_layout() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);

        let out = c.get_attribute_value(0x1234.into(), 9).unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::GET_ATTRIBUTE_VALUE,
                0x34,
                0x12,
                9,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );

        // Parameterless technical-data requests: [code][FF×7].
        for (out, code) in [
            (c.identify_vt().unwrap(), cmd::IDENTIFY_VT),
            (
                c.get_number_of_soft_keys().unwrap(),
                cmd::GET_NUMBER_SOFTKEYS,
            ),
            (c.get_text_font_data().unwrap(), cmd::GET_TEXT_FONT_DATA),
            (c.get_hardware().unwrap(), cmd::GET_HARDWARE),
            (c.get_window_mask_data().unwrap(), cmd::GET_WINDOW_MASK_DATA),
            (
                c.get_supported_objects().unwrap(),
                cmd::GET_SUPPORTED_OBJECTS,
            ),
        ] {
            assert_eq!(
                out.data,
                vec![code, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
            );
        }

        let out = c.get_supported_widechars().unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::GET_SUPPORTED_WIDECHARS,
                0x00,
                0x00,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ],
            "default WideChar request must be a valid code-plane-0 full-range query"
        );
        let out = c.get_supported_widechars_range(3, 0x0041, 0x00FF).unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::GET_SUPPORTED_WIDECHARS,
                0x03,
                0x41,
                0x00,
                0xFF,
                0x00,
                0xFF,
                0xFF
            ],
            "range WideChar request must encode code plane plus first/last query fields"
        );

        let disconnected = VTClient::new(VTClientConfig::default());
        assert!(disconnected.identify_vt().is_err());
        assert!(disconnected.get_supported_widechars().is_err());
        assert!(
            disconnected
                .get_supported_widechars_range(0, 0, u16::MAX)
                .is_err()
        );
    }

    #[test]
    fn vt_command_layouts_match_protocol_codes() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);

        let out = c.select_input_object(0x1234, 0x02).unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::SELECT_INPUT_OBJECT_COMMAND,
                0x34,
                0x12,
                0x02,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );

        let out = c.esc_input().unwrap();
        assert_eq!(
            out.data,
            vec![cmd::ESC_INPUT, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );

        let out = c.vt_esc_response(0x1234).unwrap();
        assert_eq!(
            out.data,
            vec![cmd::VT_ESC, 0x34, 0x12, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );

        let out = c.vt_esc_response_with_error(0x1234, 0x10).unwrap();
        assert_eq!(
            out.data,
            vec![cmd::VT_ESC, 0x34, 0x12, 0x10, 0xFF, 0xFF, 0xFF, 0xFF]
        );

        let out = c
            .vt_esc_response_with_transfer_sequence_number(0x1234, 0x0A)
            .unwrap();
        assert_eq!(
            out.data,
            vec![cmd::VT_ESC, 0x34, 0x12, 0xFF, 0xFF, 0xFF, 0xFF, 0xAF]
        );
        let out = c
            .change_soft_key_mask(ObjectID::new(0x1234), ObjectID::new(0x5678))
            .unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_SOFT_KEY_MASK,
                0x01,
                0x34,
                0x12,
                0x78,
                0x56,
                0xFF,
                0xFF
            ],
            "client helper must encode standard Mask Type 1 for Data Mask"
        );
        let out = c
            .change_alarm_soft_key_mask(ObjectID::new(0x1234), ObjectID::new(0x5678))
            .unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_SOFT_KEY_MASK,
                0x02,
                0x34,
                0x12,
                0x78,
                0x56,
                0xFF,
                0xFF
            ],
            "client helper must encode standard Mask Type 2 for Alarm Mask"
        );
        let out = c
            .vt_esc_response_with_error_and_transfer_sequence_number(0x1234, 0x10, 0x0A)
            .unwrap();
        assert_eq!(
            out.data,
            vec![cmd::VT_ESC, 0x34, 0x12, 0x10, 0xFF, 0xFF, 0xFF, 0xAF]
        );
        assert!(
            c.vt_esc_response_with_transfer_sequence_number(0x1234, 0x10)
                .is_err()
        );
        assert!(
            c.vt_esc_response_with_error_and_transfer_sequence_number(0x1234, 0x10, 0x10)
                .is_err()
        );

        let out = c
            .change_child_location(0x1000.into(), 0x2000.into(), 7, 9)
            .unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_CHILD_LOCATION,
                0x00,
                0x10,
                0x00,
                0x20,
                7,
                9,
                0xFF
            ]
        );

        let out = c
            .change_child_position(0x1000.into(), 0x2000.into(), 300, 400)
            .unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::CHANGE_CHILD_POSITION,
                0x00,
                0x10,
                0x00,
                0x20,
                0x2C,
                0x01,
                0x90,
                0x01
            ]
        );

        let out = c.set_audio_volume(80).unwrap();
        assert_eq!(
            out.data,
            vec![
                cmd::SET_AUDIO_VOLUME,
                80,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );
        assert!(c.set_audio_volume(101).is_err());
    }

    #[test]
    fn store_load_version_classic() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);
        let out = c.store_version("V1").unwrap();
        assert_eq!(out.data[0], cmd::STORE_VERSION);
        // 7-byte label, space-padded.
        assert_eq!(&out.data[1..8], b"V1     ");

        let out = c.load_version("V1").unwrap();
        assert_eq!(out.data[0], cmd::LOAD_VERSION);
        assert_eq!(c.state(), VTState::WaitForEndOfPool);
    }

    #[test]
    fn extended_version_label_is_32_bytes() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);
        let out = c.send_extended_store_version("MyLabelABC").unwrap();
        assert_eq!(out.data[0], cmd::EXTENDED_STORE_VERSION);
        assert_eq!(out.data[1], cmd::EXTENDED_VERSION_SUBFUNCTION);
        assert_eq!(out.data.len(), 2 + cmd::EXTENDED_VERSION_LABEL_SIZE);
        assert_eq!(&out.data[2..12], b"MyLabelABC");
        assert_eq!(c.extended_version_label(), "MyLabelABC");
    }

    #[test]
    fn version_list_responses_reject_count_length_mismatches() {
        let mut c = VTClient::new(VTClientConfig::default());
        let classic_log: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let classic_seen = classic_log.clone();
        c.on_versions_received
            .subscribe(move |labels| classic_seen.borrow_mut().push(labels.clone()));

        c.handle_vt_message(&vt_msg(
            vec![
                cmd::GET_VERSIONS_RESPONSE,
                0,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        assert_eq!(*classic_log.borrow(), vec![Vec::<String>::new()]);

        c.handle_vt_message(&vt_msg(
            vec![
                cmd::GET_VERSIONS_RESPONSE,
                0,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        c.handle_vt_message(&vt_msg(
            vec![
                cmd::GET_VERSIONS_RESPONSE,
                1,
                b'V',
                b'1',
                b' ',
                b' ',
                b' ',
                b' ',
            ],
            0x80,
        ));
        assert_eq!(
            classic_log.borrow().len(),
            1,
            "classic count/padding mismatches must be ignored"
        );

        c.handle_vt_message(&vt_msg(
            vec![
                cmd::GET_VERSIONS_RESPONSE,
                1,
                b'V',
                b'1',
                b' ',
                b' ',
                b' ',
                b' ',
                b' ',
            ],
            0x80,
        ));
        assert_eq!(classic_log.borrow()[1], vec!["V1".to_string()]);

        let mut extended = VTClient::new(VTClientConfig::default());
        let extended_log: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let extended_seen = extended_log.clone();
        extended
            .on_extended_versions_received
            .subscribe(move |labels| extended_seen.borrow_mut().push(labels.clone()));

        extended.handle_vt_message(&vt_msg(
            vec![
                cmd::EXTENDED_GET_VERSIONS,
                cmd::EXTENDED_VERSION_SUBFUNCTION,
            ],
            0x80,
        ));
        extended.handle_vt_message(&vt_msg(
            vec![
                cmd::EXTENDED_GET_VERSIONS,
                cmd::EXTENDED_VERSION_SUBFUNCTION,
                0,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        extended.handle_vt_message(&vt_msg(
            vec![
                cmd::EXTENDED_GET_VERSIONS,
                cmd::EXTENDED_VERSION_SUBFUNCTION,
                1,
                b'V',
                b'1',
                b' ',
                b' ',
                b' ',
            ],
            0x80,
        ));
        assert!(extended_log.borrow().is_empty());
        assert!(!extended.vt_supports_extended_versions());

        extended.handle_vt_message(&vt_msg(
            vec![
                cmd::EXTENDED_GET_VERSIONS,
                cmd::EXTENDED_VERSION_SUBFUNCTION,
                0,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        assert_eq!(*extended_log.borrow(), vec![Vec::<String>::new()]);
        assert!(extended.vt_supports_extended_versions());

        let mut one_label = vec![
            cmd::EXTENDED_GET_VERSIONS,
            cmd::EXTENDED_VERSION_SUBFUNCTION,
            1,
        ];
        one_label.extend_from_slice(b"V1");
        one_label.resize(3 + cmd::EXTENDED_VERSION_LABEL_SIZE, b' ');
        extended.handle_vt_message(&vt_msg(one_label, 0x80));
        assert_eq!(extended_log.borrow()[1], vec!["V1".to_string()]);
    }

    #[test]
    fn extended_store_load_responses_reject_short_prefixes() {
        let mut store = VTClient::new(VTClientConfig::default());
        force_connected(&mut store);
        let store_log: Rc<RefCell<Vec<(bool, u8)>>> = Rc::new(RefCell::new(Vec::new()));
        let store_seen = store_log.clone();
        store
            .on_extended_store_response
            .subscribe(move |&response| store_seen.borrow_mut().push(response));

        store.handle_vt_message(&vt_msg(vec![cmd::EXTENDED_STORE_VERSION, 0x00], 0x80));
        assert!(store_log.borrow().is_empty());

        store.handle_vt_message(&vt_msg(
            vec![
                cmd::EXTENDED_STORE_VERSION,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        assert_eq!(*store_log.borrow(), vec![(true, 0xFF)]);

        let mut load = VTClient::new(VTClientConfig::default());
        force_connected(&mut load);
        load.send_extended_load_version("V1").unwrap();
        assert_eq!(load.state(), VTState::WaitForEndOfPool);
        load.handle_vt_message(&vt_msg(vec![cmd::EXTENDED_LOAD_VERSION, 0x00], 0x80));
        assert_eq!(load.state(), VTState::WaitForEndOfPool);

        load.handle_vt_message(&vt_msg(
            vec![
                cmd::EXTENDED_LOAD_VERSION,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        assert_eq!(load.state(), VTState::Connected);
    }

    #[test]
    fn unsupported_function_message_is_cached_and_emitted() {
        let mut c = VTClient::new(VTClientConfig::default());
        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_unsupported_function
            .subscribe(move |&v| lc.borrow_mut().push(v));

        c.handle_vt_message(&vt_msg(
            vec![
                cmd::UNSUPPORTED_VT_FUNCTION,
                cmd::CHANGE_CHILD_POSITION,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        c.handle_vt_message(&vt_msg(
            vec![cmd::UNSUPPORTED_VT_FUNCTION, cmd::CHANGE_CHILD_POSITION],
            0x80,
        ));

        assert_eq!(c.unsupported_functions(), &[cmd::CHANGE_CHILD_POSITION]);
        assert_eq!(*log.borrow(), vec![cmd::CHANGE_CHILD_POSITION]);
    }

    #[test]
    fn activation_events_parse_server_layout_and_reject_malformed_payloads() {
        let mut c = VTClient::new(VTClientConfig::default());
        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<(ObjectID, ActivationCode)>>> = Rc::new(RefCell::new(Vec::new()));
        let button_log: Rc<RefCell<Vec<(ObjectID, ActivationCode)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        let bc = button_log.clone();
        c.on_soft_key.subscribe(move |&v| lc.borrow_mut().push(v));
        c.on_button.subscribe(move |&v| bc.borrow_mut().push(v));
        c.handle_vt_message(&vt_msg(
            VTServer::build_soft_key_activation(ActivationCode::Held, 0xCAFE, 0xBEEF, 7).to_vec(),
            0x80,
        ));
        c.handle_vt_message(&vt_msg(
            VTServer::build_button_activation(ActivationCode::Pressed, 0xCAFE, 0xBEEF, 7).to_vec(),
            0x80,
        ));
        c.handle_vt_message(&vt_msg(vec![cmd::SOFT_KEY_ACTIVATION, 0x05, 0, 1], 0x80));
        let mut bad_code =
            VTServer::build_soft_key_activation(ActivationCode::Held, 0xCAFE, 0xBEEF, 7);
        bad_code[1] = 0x05;
        c.handle_vt_message(&vt_msg(bad_code.to_vec(), 0x80));
        let mut bad_tail =
            VTServer::build_soft_key_activation(ActivationCode::Held, 0xCAFE, 0xBEEF, 7);
        bad_tail[7] = 0x00;
        c.handle_vt_message(&vt_msg(bad_tail.to_vec(), 0x80));
        let mut bad_button_code =
            VTServer::build_button_activation(ActivationCode::Pressed, 0xCAFE, 0xBEEF, 7);
        bad_button_code[1] = 0x05;
        c.handle_vt_message(&vt_msg(bad_button_code.to_vec(), 0x80));
        let mut bad_button_tail =
            VTServer::build_button_activation(ActivationCode::Pressed, 0xCAFE, 0xBEEF, 7);
        bad_button_tail[7] = 0x00;
        c.handle_vt_message(&vt_msg(bad_button_tail.to_vec(), 0x80));
        assert_eq!(
            *log.borrow(),
            vec![(ObjectID(0xCAFE), ActivationCode::Held)]
        );
        assert_eq!(
            *button_log.borrow(),
            vec![(ObjectID(0xCAFE), ActivationCode::Pressed)]
        );
    }

    #[test]
    fn pointing_and_select_input_object_events_parse_server_layout() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let mut c = VTClient::new(VTClientConfig::default());
        let points: Rc<RefCell<Vec<(u16, u16, ActivationCode)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let selects: Rc<RefCell<Vec<(ObjectID, bool, bool)>>> = Rc::new(RefCell::new(Vec::new()));
        let select_responses: Rc<RefCell<Vec<(ObjectID, u8, u8)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let pc = points.clone();
        let sc = selects.clone();
        let src = select_responses.clone();
        c.on_pointing_event
            .subscribe(move |&v| pc.borrow_mut().push(v));
        c.on_select_input_object
            .subscribe(move |&v| sc.borrow_mut().push(v));
        c.on_select_input_object_response
            .subscribe(move |&v| src.borrow_mut().push(v));

        // Pointing event at X=300 (0x012C), Y=200 (0x00C8), Pressed.
        c.handle_vt_message(&vt_msg(
            vec![
                cmd::POINTING_EVENT,
                0x2C,
                0x01,
                0xC8,
                0x00,
                0x01,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        // Select input object 0xCAFE, selected + open-for-input.
        c.handle_vt_message(&vt_msg(
            vec![
                cmd::SELECT_INPUT_OBJECT,
                0xFE,
                0xCA,
                0x01,
                0x01,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        // Select Input Object response 0xBEEF opened for edit without error.
        c.handle_vt_message(&vt_msg(
            vec![
                cmd::SELECT_INPUT_OBJECT_COMMAND,
                0xEF,
                0xBE,
                0x02,
                0x00,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        // Malformed (short) payloads are ignored.
        c.handle_vt_message(&vt_msg(vec![cmd::POINTING_EVENT, 0x2C, 0x01], 0x80));

        assert_eq!(*points.borrow(), vec![(300, 200, ActivationCode::Pressed)]);
        assert_eq!(*selects.borrow(), vec![(ObjectID(0xCAFE), true, true)]);
        assert_eq!(
            *select_responses.borrow(),
            vec![(ObjectID(0xBEEF), 0x02, 0x00)]
        );
    }

    #[test]
    fn vt_esc_event_parses_aborted_input_and_error_code() {
        type VtEscDetailedLog = Rc<RefCell<Vec<(ObjectID, u8, Option<u8>)>>>;

        let mut c = VTClient::new(VTClientConfig::default());
        let esc_events: Rc<RefCell<Vec<(ObjectID, u8)>>> = Rc::new(RefCell::new(Vec::new()));
        let detailed: VtEscDetailedLog = Rc::new(RefCell::new(Vec::new()));
        let log = esc_events.clone();
        c.on_vt_esc
            .subscribe(move |&value| log.borrow_mut().push(value));
        let detailed_log = detailed.clone();
        c.on_vt_esc_detailed
            .subscribe(move |&value| detailed_log.borrow_mut().push(value));

        c.handle_vt_message(&vt_msg(
            vec![cmd::VT_ESC, 0xFE, 0xCA, 0x10, 0xFF, 0xFF, 0xFF, 0xFF],
            0x80,
        ));
        c.handle_vt_message(&vt_msg(
            vec![cmd::VT_ESC, 0xFE, 0xCA, 0x00, 0xFF, 0xFF, 0xFF, 0xAF],
            0x80,
        ));
        // Malformed reserved tail is ignored.
        c.handle_vt_message(&vt_msg(
            vec![cmd::VT_ESC, 0xFE, 0xCA, 0x10, 0x00, 0xFF, 0xFF, 0xFF],
            0x80,
        ));
        // Malformed VT v6 byte with non-reserved lower nibble is ignored.
        c.handle_vt_message(&vt_msg(
            vec![cmd::VT_ESC, 0xFE, 0xCA, 0x10, 0xFF, 0xFF, 0xFF, 0xA0],
            0x80,
        ));

        assert_eq!(
            *esc_events.borrow(),
            vec![(ObjectID(0xCAFE), 0x10), (ObjectID(0xCAFE), 0x00)]
        );
        assert_eq!(
            *detailed.borrow(),
            vec![
                (ObjectID(0xCAFE), 0x10, None),
                (ObjectID(0xCAFE), 0x00, Some(0x0A)),
            ]
        );
    }

    #[test]
    fn language_command_triggers_reload_when_connected() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);
        // English client; receive a German Language Command.
        c.handle_language_command(&Message::new(
            PGN_LANGUAGE_COMMAND,
            b"de\0\0\0\0\0\0".to_vec(),
            0x80,
        ));
        assert_eq!(c.vt_language(), LanguageCode { code: *b"de" });
        assert_eq!(c.state(), VTState::ReloadPool);
        // ReloadPool ticks back through SendGetMemory → WaitForMemory.
        let _ = c.update(1);
        assert_eq!(c.state(), VTState::SendGetMemory);
    }

    #[test]
    fn auto_reload_can_be_disabled() {
        let mut c = VTClient::new(VTClientConfig::default());
        force_connected(&mut c);
        c.set_auto_reload_on_language_change(false);
        c.handle_language_command(&Message::new(
            PGN_LANGUAGE_COMMAND,
            b"de\0\0\0\0\0\0".to_vec(),
            0x80,
        ));
        // Language updated but no reload.
        assert_eq!(c.vt_language(), LanguageCode { code: *b"de" });
        assert_eq!(c.state(), VTState::Connected);

        c.handle_language_command(&Message::new(PGN_LANGUAGE_COMMAND, b"fr".to_vec(), 0x80));
        assert_eq!(
            c.vt_language(),
            LanguageCode { code: *b"de" },
            "short Language Command payloads must be ignored"
        );
    }

    #[test]
    fn macro_register_dedups_by_id() {
        let mut c = VTClient::new(VTClientConfig::default());
        c.register_macro(VTMacro {
            macro_id: ObjectID(1),
            commands: vec![vec![0x01]],
        });
        c.register_macro(VTMacro {
            macro_id: ObjectID(1),
            commands: vec![vec![0x02]],
        });
        assert_eq!(c.macros().len(), 1);
        assert_eq!(c.get_macro(ObjectID(1)).unwrap().commands, vec![vec![0x02]]);
    }

    #[test]
    fn vt_status_active_ws_detection() {
        let mut c = VTClient::new(VTClientConfig::default());
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();
        c.set_self_address(0x42);
        // VT_STATUS active WS = 0x42 (us).
        let mut data = vec![cmd::VT_STATUS, 0x42];
        data.resize(8, 0xFF);
        data[6] = 4;
        c.handle_vt_message(&vt_msg(data, 0x80));
        assert!(c.is_active_ws());
        // VT_STATUS active WS changes to someone else.
        let mut data = vec![cmd::VT_STATUS, 0x10];
        data.resize(8, 0xFF);
        data[6] = 4;
        c.handle_vt_message(&vt_msg(data, 0x80));
        assert!(!c.is_active_ws());
    }
}
