#[cfg(test)]
mod tests {
    use super::super::objects::{
        ColourMapBody, ContainerBody, DataMaskBody, FillAttributesBody, FontAttributesBody,
        GraphicContextBody, InputListBody, LineAttributesBody, MacroBody, ObjectLabelRefBody,
        ObjectLabelRefEntry, OutputPolygonBody, OutputStringBody, PolygonPoint, SoftKeyMaskBody,
        StringVariableBody, WorkingSetBody, create_colour_map, create_container, create_data_mask,
        create_fill_attributes, create_font_attributes, create_graphic_context, create_input_list,
        create_line_attributes, create_macro, create_number_variable, create_object_label_ref,
        create_output_polygon, create_output_string, create_soft_key_mask, create_string_variable,
        create_working_set,
    };
    use super::*;
    use crate::net::pgn_defs::PGN_ECU_TO_VT;
    use crate::vt_storage::StoredPoolVersion;

    fn ecu_msg(data: Vec<u8>, src: Address) -> Message {
        Message::new(PGN_ECU_TO_VT, data, src)
    }

    fn fixed_command(function: u8) -> Vec<u8> {
        let mut data = [0xFFu8; 8];
        data[0] = function;
        data.to_vec()
    }

    fn valid_pool() -> ObjectPool {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_soft_key_mask(3, &SoftKeyMaskBody::default()))
            .with_object(
                create_input_list(
                    4,
                    &InputListBody {
                        items: vec![ObjectID(5)],
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_string(
                    5,
                    &OutputStringBody {
                        value: b"old".to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(create_font_attributes(6, &FontAttributesBody::default()))
            .with_object(create_line_attributes(7, &LineAttributesBody::default()))
            .with_object(create_fill_attributes(8, &FillAttributesBody::default()).unwrap())
            .with_object(create_output_string(0x10, &OutputStringBody::default()).unwrap())
            .with_object(create_container(0x11, &ContainerBody::default()))
            .with_object(create_output_string(0x50, &OutputStringBody::default()).unwrap())
            .with_object(create_output_string(0x60, &OutputStringBody::default()).unwrap())
            .with_object(create_string_variable(
                0x61,
                &StringVariableBody {
                    length: 3,
                    value: b"ABC".to_vec(),
                },
            ))
            .with_object(create_object_label_ref(
                0x62,
                &ObjectLabelRefBody {
                    labels: vec![ObjectLabelRefEntry {
                        labelled_object: ObjectID(0x60),
                        string_variable: ObjectID(0x61),
                        font_type: 0,
                        graphic_designator: ObjectID::NULL,
                    }],
                },
            ))
            .with_object(
                create_output_polygon(
                    0x65,
                    &OutputPolygonBody {
                        points: vec![
                            PolygonPoint { x: 0, y: 0 },
                            PolygonPoint { x: 10, y: 0 },
                            PolygonPoint { x: 0, y: 10 },
                        ],
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(create_graphic_context(0x70, &GraphicContextBody::default()).unwrap())
            .with_object(
                create_colour_map(
                    0x71,
                    &ColourMapBody {
                        entries: vec![0, 1],
                    },
                )
                .unwrap(),
            )
            .with_object(create_number_variable(0x72, &Default::default()))
            .with_object(create_data_mask(0x20, &DataMaskBody::default()))
            .with_object(create_macro(0x99, &MacroBody::default()))
    }

    fn activate_valid_pool(s: &mut VTServer, src: Address) {
        let _ = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), src));
        let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
        transfer.extend(valid_pool().serialize().unwrap());
        s.handle_ecu_message(&ecu_msg(transfer, src));
        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::END_OF_POOL), src));
        assert_eq!(out[0].data[1], 0x00);
        assert!(s.clients().iter().any(|client| {
            client.client_address == src && client.pool_uploaded && client.pool_activated
        }));
    }

    #[test]
    fn config_builders() {
        let c = VTServerConfig::default()
            .with_screen(800, 600)
            .with_version(6);
        assert_eq!(c.screen_width, 800);
        assert_eq!(c.screen_height, 600);
        assert_eq!(c.vt_version, 6);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn config_validation_rejects_zero_screen_dimensions() {
        let err = VTServerConfig::default()
            .with_screen(0, 600)
            .validate()
            .unwrap_err();
        assert_eq!(err.code, crate::net::ErrorCode::InvalidData);
        assert!(err.message.contains("screen_width"));

        let err = VTServerConfig::default()
            .with_screen(800, 0)
            .validate()
            .unwrap_err();
        assert_eq!(err.code, crate::net::ErrorCode::InvalidData);
        assert!(err.message.contains("screen_height"));
    }

    #[test]
    fn config_validation_rejects_unencodable_vt_versions() {
        for version in [0, 2, 7, u16::MAX] {
            let err = VTServerConfig::default()
                .with_version(version)
                .validate()
                .unwrap_err();
            assert_eq!(err.code, crate::net::ErrorCode::InvalidData);
            assert!(err.message.contains("vt_version"));

            let mut server = VTServer::new(VTServerConfig::default().with_version(version));
            assert!(server.start().is_err());
            assert_eq!(server.state(), VTServerState::Disconnected);
        }
    }

    #[test]
    fn start_transitions_state() {
        let mut s = VTServer::new(VTServerConfig::default());
        assert_eq!(s.state(), VTServerState::Disconnected);
        s.start().unwrap();
        assert_eq!(s.state(), VTServerState::WaitForClientStatus);
    }

    #[test]
    fn get_memory_creates_client_and_replies() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dest, Some(0x42));
        assert_eq!(out[0].data[0], cmd::GET_MEMORY_RESPONSE);
        assert_eq!(s.clients().len(), 1);
        assert_eq!(s.state(), VTServerState::WaitForPoolUpload);
    }

    #[test]
    fn server_answers_hardware_softkey_and_font_capability_queries() {
        let cfg = VTServerConfig {
            screen_width: 800,
            screen_height: 600,
            ..VTServerConfig::default()
        };
        let mut s = VTServer::new(cfg);
        s.start().unwrap();

        // Get Hardware: graphic type, hw features, X/Y pixels = screen size.
        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_HARDWARE), 0x42));
        assert_eq!(out[0].data[0], cmd::GET_HARDWARE);
        assert_eq!(out[0].data[2], 2); // 256-colour default
        assert_eq!(u16::from_le_bytes([out[0].data[4], out[0].data[5]]), 800);
        assert_eq!(u16::from_le_bytes([out[0].data[6], out[0].data[7]]), 600);

        // Get Number Of Soft Keys: X/Y dots + virtual/physical counts.
        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_NUMBER_SOFTKEYS), 0x42));
        assert_eq!(out[0].data[0], cmd::GET_NUMBER_SOFTKEYS);
        assert_eq!(out[0].data[4], 60); // X dots
        assert_eq!(out[0].data[6], 6); // virtual soft keys

        // Get Text Font Data: small/large/style bitfields.
        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_TEXT_FONT_DATA), 0x42));
        assert_eq!(out[0].data[0], cmd::GET_TEXT_FONT_DATA);
        assert_eq!(out[0].data[5], 0xFF);
        assert_eq!(out[0].data[7], 0xFF);

        for command in [
            cmd::GET_HARDWARE,
            cmd::GET_NUMBER_SOFTKEYS,
            cmd::GET_TEXT_FONT_DATA,
            cmd::GET_WINDOW_MASK_DATA,
        ] {
            let mut request = fixed_command(command);
            request[1] = 0x00;
            assert!(
                s.handle_ecu_message(&ecu_msg(request, 0x42)).is_empty(),
                "parameterless VT technical-data request 0x{command:02X} must reserve bytes 1..=7 as 0xFF"
            );
        }
    }

    #[test]
    fn get_versions_response_never_wraps_one_byte_count() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        activate_valid_pool(&mut s, 0x42);
        for i in 0..=MAX_STORED_VERSIONS {
            s.clients[0].stored_versions.push(StoredPoolVersion {
                label: format!("V{:06}", i % MAX_STORED_VERSIONS),
                ..Default::default()
            });
        }

        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_VERSIONS), 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], cmd::GET_VERSIONS_RESPONSE);
        assert_eq!(out[0].data[1], u8::MAX);
        assert_eq!(out[0].data.len(), 2 + MAX_STORED_VERSIONS * 7);
    }

    #[test]
    fn pool_transfer_and_end_of_pool_connects() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        // Get memory: prepares client.
        let _ = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), 0x42));

        let pool = valid_pool();
        let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
        transfer.extend(pool.serialize().unwrap());
        s.handle_ecu_message(&ecu_msg(transfer, 0x42));
        // E5 — the transfer is staged; C.2.2 b)1) allows any number of sessions
        // before End of Object Pool, so nothing is live until that arrives.
        assert!(!s.clients()[0].pool_uploaded);

        // End of pool ⇒ no errors and the working set becomes active.
        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::END_OF_POOL), 0x42));
        assert_eq!(out[0].data[1], 0x00);
        assert_eq!(out[0].data[6], 0x00);
        assert!(s.clients()[0].pool_uploaded);
        assert_eq!(s.clients()[0].pool.size(), valid_pool().size());
        assert_eq!(s.state(), VTServerState::Connected);
        assert_eq!(s.active_working_set(), 0x42);
    }

    #[test]
    fn malformed_pool_transfer_does_not_create_or_overwrite_client_state() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();

        s.handle_ecu_message(&ecu_msg(vec![cmd::OBJECT_POOL_TRANSFER], 0x42));
        assert!(
            s.clients().is_empty(),
            "empty upload must not create client"
        );

        s.handle_ecu_message(&ecu_msg(
            vec![cmd::OBJECT_POOL_TRANSFER, 0x01, 0x00, 0xFE, 0x00, 0x00],
            0x42,
        ));
        assert!(
            s.clients().is_empty(),
            "unknown object type must not create client"
        );

        // E5 — C.2.2 b)1): transfers are staged and only End of Object Pool
        // closes the upload, so the pool is not live until then.
        let _ = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), 0x42));
        let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
        transfer.extend(valid_pool().serialize().unwrap());
        s.handle_ecu_message(&ecu_msg(transfer, 0x42));
        assert!(
            !s.clients()[0].pool_uploaded,
            "a transfer is staged, not applied"
        );
        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::END_OF_POOL), 0x42));
        assert_eq!(out[0].data[1], 0x00);
        let valid_pool_size = valid_pool().size();
        assert_eq!(s.clients()[0].pool.size(), valid_pool_size);
        assert!(s.clients()[0].pool_uploaded);

        // A malformed replacement upload must not damage the live pool.
        let _ = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), 0x42));
        s.handle_ecu_message(&ecu_msg(
            vec![cmd::OBJECT_POOL_TRANSFER, 0x01, 0x00, 0x00],
            0x42,
        ));
        assert_eq!(
            s.clients()[0].pool.size(),
            valid_pool_size,
            "a staged transfer must not touch the live pool"
        );
    }

    #[test]
    fn end_of_pool_rejects_semantically_invalid_uploaded_pool() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        let _ = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), 0x42));

        let invalid = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]));
        let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
        transfer.extend(invalid.serialize().unwrap());
        s.handle_ecu_message(&ecu_msg(transfer, 0x42));
        assert!(!s.clients()[0].pool_uploaded);
        assert!(s.clients()[0].pool.is_empty());

        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::END_OF_POOL), 0x42));
        assert_eq!(out[0].data[1], 0x01);
        // E8 — C.2.5: bit 1 says the pool references an object it does not
        // contain, bit 3 that the VT deleted it. This used to report bit 1
        // alone, and never actually deleted the rejected pool.
        assert_eq!(out[0].data[6], 0x02 | 0x08);
        assert_eq!(s.state(), VTServerState::WaitForPoolUpload);
        assert_eq!(s.active_working_set(), NULL_ADDRESS);
        assert!(!s.clients()[0].pool_activated);
    }

    #[test]
    fn end_of_pool_without_upload_errors() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        let _ = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), 0x42));
        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::END_OF_POOL), 0x42));
        assert_eq!(out[0].data[1], 0x01);
        // E8 — nothing was transferred: "any other error" (bit 2), plus bit 3
        // for the deletion. Reporting a missing object reference sent the
        // Working Set hunting for a bad reference that does not exist.
        assert_eq!(out[0].data[6], 0x04 | 0x08);
    }

    #[test]
    fn fixed_size_ecu_commands_reject_prefix_payloads() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        assert!(
            s.handle_ecu_message(&ecu_msg(vec![cmd::GET_MEMORY], 0x42))
                .is_empty()
        );
        assert!(s.clients().is_empty());

        let mut overlong = fixed_command(cmd::END_OF_POOL);
        overlong.push(0x00);
        assert!(s.handle_ecu_message(&ecu_msg(overlong, 0x42)).is_empty());
    }

    #[test]
    fn vt_status_emitted_at_cadence() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        assert!(s.update(VT_STATUS_INTERVAL_MS - 1).is_none());
        let bytes = s.update(2).unwrap();
        assert_eq!(bytes[0], cmd::VT_STATUS);
    }

    /// E3 — Annex G.2: bytes 3-4 are the visible Data/Alarm Mask Object ID and
    /// bytes 5-6 the visible Soft Key Mask, "or FFFF16 if no Working Set owns
    /// the VT". These four bytes carried the working set's *source address*, so
    /// a VT with the active set at SA 0x26 announced visible-mask Object ID
    /// 0x2600 — not an object in any pool.
    /// E2 — ISO 11783-6 §4.6.9: the Working Set Maintenance message is how the
    /// VT knows a Working Set is still there. It was unhandled, so it fell
    /// through to the catch-all: the VT answered every one with Unsupported VT
    /// Function, once per second forever, and never detected a working set
    /// going away.
    #[test]
    fn working_set_maintenance_is_consumed_and_watchdogged() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();

        let addr = 0x42;
        let mut maintenance = [0xFFu8; 8];
        maintenance[0] = cmd::WORKING_SET_MAINTENANCE;
        maintenance[1] = 0x01; // Initiating, reserved bits clear (G.3 byte 2).
        maintenance[2] = 5; // Compliant with VT version 5 (G.3 byte 3).

        let out = s.handle_ecu_message(&ecu_msg(maintenance.to_vec(), addr));
        assert!(
            out.is_empty(),
            "G.3 defines no response; the VT must not NACK it"
        );
        assert_eq!(
            s.clients()
                .iter()
                .find(|c| c.client_address == addr)
                .and_then(|c| c.declared_version),
            Some(5),
            "the working set's declared version is recorded"
        );

        // Cadence keeps it alive.
        let mut later = maintenance;
        later[1] = 0x00;
        for _ in 0..5 {
            s.update(1000);
            assert!(s.handle_ecu_message(&ecu_msg(later.to_vec(), addr)).is_empty());
        }
        assert!(s.clients().iter().any(|c| c.client_address == addr));

        // Three seconds of silence is an unexpected shutdown.
        s.update(WORKING_SET_MAINTENANCE_TIMEOUT_MS);
        assert!(
            !s.clients().iter().any(|c| c.client_address == addr),
            "a silent Working Set Master must be dropped"
        );
    }

    /// §4.6.9: a *second* Initiating means the master restarted underneath us.
    #[test]
    fn a_second_initiating_maintenance_drops_the_working_set() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();

        let addr = 0x42;
        let mut initiating = [0xFFu8; 8];
        initiating[0] = cmd::WORKING_SET_MAINTENANCE;
        initiating[1] = 0x01;
        initiating[2] = 5;

        s.handle_ecu_message(&ecu_msg(initiating.to_vec(), addr));
        assert!(s.clients().iter().any(|c| c.client_address == addr));

        s.handle_ecu_message(&ecu_msg(initiating.to_vec(), addr));
        assert!(
            !s.clients().iter().any(|c| c.client_address == addr),
            "a repeated Initiating bit is an unexpected shutdown"
        );
    }

    /// E5 — C.2.2 b)1): "The Working Set Master can send several single packet,
    /// TP or ETP sessions or a combination of any of these to transfer the
    /// entire pool. This can be required depending on the size of buffers
    /// designed into the Working Set Master. Any number of sessions can be sent
    /// before the End of Object Pool message is sent."
    ///
    /// Each session used to be deserialized on arrival and to *replace* the
    /// pool, so only the last one survived: any ECU whose transmit buffer is
    /// smaller than its pool — the case this clause exists for — silently lost
    /// everything before it, and then failed End of Object Pool.
    #[test]
    fn a_pool_split_across_sessions_is_assembled_before_activation() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        let _ = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), 0x42));

        let bytes = valid_pool().serialize().unwrap();
        assert!(bytes.len() > 8, "the fixture pool needs splitting to matter");
        let split = bytes.len() / 3;

        // Three sessions, none of which is a whole pool on its own.
        for chunk in [&bytes[..split], &bytes[split..split * 2], &bytes[split * 2..]] {
            let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
            transfer.extend_from_slice(chunk);
            assert!(
                s.handle_ecu_message(&ecu_msg(transfer, 0x42)).is_empty(),
                "a transfer session has no Annex F response"
            );
        }

        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::END_OF_POOL), 0x42));
        assert_eq!(
            out[0].data[1], 0x00,
            "a pool sent in three sessions must assemble into one"
        );
        assert_eq!(s.clients()[0].pool.size(), valid_pool().size());
        assert!(s.clients()[0].pool_activated);
    }

    /// C.2.6: "Only those objects that need to be changed are transmitted" —
    /// a runtime update replaces the objects it carries and leaves the rest.
    #[test]
    fn a_runtime_pool_update_merges_rather_than_replacing() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        activate_valid_pool(&mut s, 0x42);
        let original = s.clients()[0].pool.size();

        // Re-open the upload window and send a single changed object.
        let _ = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::GET_MEMORY), 0x42));
        let update = ObjectPool::default().with_object(
            create_output_string(
                0x10,
                &OutputStringBody {
                    value: b"new".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
        transfer.extend(update.serialize().unwrap());
        s.handle_ecu_message(&ecu_msg(transfer, 0x42));

        let out = s.handle_ecu_message(&ecu_msg(fixed_command(cmd::END_OF_POOL), 0x42));
        assert_eq!(out[0].data[1], 0x00, "a runtime update is accepted");
        assert_eq!(
            s.clients()[0].pool.size(),
            original,
            "a runtime update must not discard the objects it does not carry"
        );
    }

    /// The hand-maintained list above is only as complete as whoever last
    /// edited it: `cmd::EXECUTE_EXTENDED_MACRO` was dispatched through
    /// `annex_f_response` for a command `VtResponseShape::for_command` had no
    /// arm for, so the macro ran and the Working Set was never answered. Scan
    /// the dispatcher instead of trusting the list.
    #[test]
    fn every_dispatched_annex_f_command_has_a_response_shape() {
        let dispatcher = include_str!("server_lifecycle_queries_upload.rs");
        let shapes = include_str!("server_types_and_config.rs");
        let table = shapes
            .split("fn for_command")
            .nth(1)
            .expect("for_command exists");

        let mut dispatched: Vec<&str> = Vec::new();
        for (i, line) in dispatcher.lines().enumerate() {
            let Some(rest) = line.trim().strip_prefix("cmd::") else {
                continue;
            };
            let Some(name) = rest.split(|c: char| !c.is_ascii_uppercase() && c != '_').next() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            // A block arm whose body answers via annex_f_response. Anchoring on
            // `=> {` keeps single-expression arms such as `cmd::END_OF_POOL =>
            // self.handle_end_of_pool(msg)` out of the scan.
            if !line.trim_end().ends_with("=> {") {
                continue;
            }
            let answers = dispatcher
                .lines()
                .skip(i + 1)
                .take(3)
                .any(|l| l.contains("self.annex_f_response(msg, outcome)"));
            if answers && !dispatched.contains(&name) {
                dispatched.push(name);
            }
        }
        assert!(
            dispatched.len() > 20,
            "the scan found only {dispatched:?}; it stopped matching the dispatcher"
        );
        for name in dispatched {
            assert!(
                table.contains(&format!("cmd::{name}")),
                "cmd::{name} is answered through annex_f_response but has no \
                 VtResponseShape arm, so the VT carries the command out and \
                 sends nothing (Annex F.1)"
            );
        }
    }

    /// E1 — Annex F.1: "The VT shall respond to these commands even if no object
    /// pool of the originating Working Set is loaded. The originator shall wait
    /// for a response before sending another command. Unless stated otherwise,
    /// another command can be sent if a response is not received within 1,5 s."
    ///
    /// Every mutating command used to return no frame at all, so a conformant
    /// Working Set blocked 1,5 s per command and retried three times before
    /// declaring the VT unresponsive. A burst of Change Numeric Value updates
    /// advanced at roughly one command per 1,5 s instead of one per CAN frame.
    #[test]
    fn every_annex_f_command_is_answered_even_with_no_pool_loaded() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();

        // Deliberately no pool: F.1 says the VT answers anyway.
        for function in [
            cmd::HIDE_SHOW,
            cmd::ENABLE_DISABLE,
            cmd::CONTROL_AUDIO_SIGNAL,
            cmd::SET_AUDIO_VOLUME,
            cmd::CHANGE_CHILD_LOCATION,
            cmd::CHANGE_CHILD_POSITION,
            cmd::CHANGE_SIZE,
            cmd::CHANGE_BACKGROUND_COLOUR,
            cmd::CHANGE_NUMERIC_VALUE,
            cmd::CHANGE_END_POINT,
            cmd::CHANGE_FONT_ATTRIBUTES,
            cmd::CHANGE_LINE_ATTRIBUTES,
            cmd::CHANGE_FILL_ATTRIBUTES,
            cmd::CHANGE_ACTIVE_MASK,
            cmd::CHANGE_SOFT_KEY_MASK,
            cmd::CHANGE_ATTRIBUTE,
            cmd::CHANGE_PRIORITY,
            cmd::CHANGE_LIST_ITEM,
            cmd::DELETE_OBJECT_POOL,
            cmd::CHANGE_POLYGON_POINT,
            cmd::CHANGE_POLYGON_SCALE,
            cmd::SELECT_COLOUR_MAP,
            cmd::LOCK_UNLOCK_MASK,
            cmd::EXECUTE_MACRO,
            cmd::EXECUTE_EXTENDED_MACRO,
        ] {
            let mut data = [0xFFu8; 8];
            data[0] = function;
            // A real, non-NULL Object ID: 0xFFFF means "no object" on several
            // of these commands, which is a legitimate no-pool operation and
            // would not exercise the missing-object path at all.
            data[1..3].copy_from_slice(&0x1234u16.to_le_bytes());
            let out = s.handle_ecu_message(&ecu_msg(data.to_vec(), 0x42));

            assert_eq!(
                out.len(),
                1,
                "function {function:#04X} must be answered (Annex F.1)"
            );
            let reply = &out[0].data;
            assert_eq!(reply.len(), 8);
            assert_eq!(
                reply[0], function,
                "F.1: the response echoes the command's function code"
            );

            // Commands that address an object cannot have succeeded with no
            // pool loaded. Three do not address one: the two audio commands,
            // and Delete Object Pool, which is idempotent — deleting nothing is
            // not an error. F.1 only requires that they are *answered*.
            let shape = VtResponseShape::for_command(function)
                .expect("this command defines an Annex F response");
            let addresses_an_object = !matches!(
                function,
                cmd::CONTROL_AUDIO_SIGNAL | cmd::SET_AUDIO_VOLUME | cmd::DELETE_OBJECT_POOL
            );
            if addresses_an_object {
                assert_ne!(
                    reply[shape.echo + 1],
                    0,
                    "function {function:#04X} cannot have succeeded with no pool"
                );
            }
        }
    }

    #[test]
    fn vt_status_carries_the_visible_mask_object_ids() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();

        // Nothing owns the VT yet.
        let idle = s.update(VT_STATUS_INTERVAL_MS).unwrap();
        assert_eq!(&idle[2..6], &[0xFF, 0xFF, 0xFF, 0xFF]);

        // With an active working set the real Object IDs appear, little-endian.
        let addr = 0x26;
        s.ensure_client(addr);
        if let Some(c) = s.clients.iter_mut().find(|c| c.client_address == addr) {
            c.pool_activated = true;
            c.object_state.active_data_mask = ObjectID(0x1234);
            c.object_state.active_soft_key_mask = ObjectID(0x5678);
        }
        s.set_active_working_set(addr);

        let active = s.update(VT_STATUS_INTERVAL_MS).unwrap();
        assert_eq!(active[1], addr, "byte 2 is where the address belongs");
        assert_eq!(u16_le(&active[2..]), 0x1234);
        assert_eq!(u16_le(&active[4..]), 0x5678);
    }

    #[test]
    fn build_helpers_layout() {
        let bytes =
            VTServer::build_button_activation(KeyActivationCode::Pressed, 0xCAFE, 0xBEEF, 7);
        assert_eq!(bytes[0], cmd::BUTTON_ACTIVATION);
        assert_eq!(bytes[1], 1);
        assert_eq!(u16_le(&bytes[2..]), 0xCAFE);
        assert_eq!(u16_le(&bytes[4..]), 0xBEEF);
        assert_eq!(bytes[6], 7);

        let bytes = VTServer::build_change_numeric_value(0x1234, 0xDEADBEEF);
        assert_eq!(bytes[0], cmd::NUMERIC_VALUE_CHANGE);
        assert_eq!(u16_le(&bytes[1..]), 0x1234);
        assert_eq!(u32_le(&bytes[4..]), 0xDEADBEEF);

        let bytes = VTServer::build_change_string_value(0x05, "hé").unwrap();
        assert_eq!(bytes[0], cmd::STRING_VALUE_CHANGE);
        assert_eq!(u16_le(&bytes[1..]), 0x05);
        assert_eq!(u16_le(&bytes[3..]), 3);
        assert_eq!(&bytes[5..], "hé".as_bytes());

        let too_long = "x".repeat(VT_STRING_VALUE_MAX_LEN + 1);
        assert!(VTServer::build_change_string_value(0x05, &too_long).is_err());

        let bytes = VTServer::build_unsupported_function(cmd::CHANGE_POLYGON_POINT);
        assert_eq!(
            bytes,
            [
                cmd::UNSUPPORTED_VT_FUNCTION,
                cmd::CHANGE_POLYGON_POINT,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );
    }

    #[test]
    fn unknown_ecu_command_returns_unsupported_function_message() {
        const UNKNOWN_FUNCTION: u8 = 0xEF;
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        let out = s.handle_ecu_message(&ecu_msg(vec![UNKNOWN_FUNCTION], 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dest, Some(0x42));
        assert_eq!(
            out[0].data,
            vec![
                cmd::UNSUPPORTED_VT_FUNCTION,
                UNKNOWN_FUNCTION,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );
    }

    #[test]
    fn ecu_change_commands_update_server_working_set_state() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        activate_valid_pool(&mut s, 0x42);

        // E1 — Annex F.1: "The originator shall wait for a response before
        // sending another command." This used to assert `out.is_empty()`, i.e.
        // that these commands produce no frame at all, which is the defect: a
        // conformant Working Set then blocked 1,5 s per command.
        let send = |server: &mut VTServer, data: Vec<u8>| {
            let function = data[0];
            let out = server.handle_ecu_message(&ecu_msg(data.clone(), 0x42));
            let shape = VtResponseShape::for_command(function)
                .expect("every command here has an Annex F response");
            assert_eq!(out.len(), 1, "function {function:#04X} must be answered");
            let reply = &out[0].data;
            assert_eq!(reply.len(), 8);
            assert_eq!(reply[0], function, "F.1: the response echoes the function");
            for i in 1..=shape.echo {
                assert_eq!(
                    reply[i], data[i],
                    "F.1: byte {} must be echoed from the command",
                    i + 1
                );
            }
            assert_eq!(
                reply[shape.echo + 1],
                0,
                "command {function:#04X} reported an error"
            );
        };

        // A command the VT refuses still gets an Annex F response — that is the
        // whole point of F.1 — but with the error byte set.
        let send_rejected = |server: &mut VTServer, data: Vec<u8>| {
            let function = data[0];
            let out = server.handle_ecu_message(&ecu_msg(data, 0x42));
            let shape = VtResponseShape::for_command(function).expect("has a response");
            assert_eq!(out.len(), 1, "a refusal must still be answered");
            assert_ne!(
                out[0].data[shape.echo + 1],
                0,
                "command {function:#04X} was refused and must say so"
            );
        };

        send(
            &mut s,
            vec![cmd::HIDE_SHOW, 0x11, 0x00, 0x01, 0xFF, 0xFF, 0xFF, 0xFF],
        );
        send(
            &mut s,
            vec![
                cmd::ENABLE_DISABLE,
                0x04,
                0x00,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::CHANGE_ACTIVE_MASK,
                0x01,
                0x00,
                0x02,
                0x00,
                0xFF,
                0xFF,
                0xFF,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::CHANGE_SOFT_KEY_MASK,
                0x01,
                0x02,
                0x00,
                0x03,
                0x00,
                0xFF,
                0xFF,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::CHANGE_ATTRIBUTE,
                0x10,
                0x00,
                0x07,
                0x02,
                0x00,
                0x00,
                0x00,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::CHANGE_LIST_ITEM,
                0x04,
                0x00,
                0x00,
                0x05,
                0x00,
                0xFF,
                0xFF,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::CHANGE_CHILD_LOCATION,
                0x01,
                0x00,
                0x02,
                0x00,
                12,
                13,
                0xFF,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::CHANGE_CHILD_POSITION,
                0x01,
                0x00,
                0x02,
                0x00,
                0x34,
                0x12,
                0x78,
                0x56,
            ],
        );
        send(
            &mut s,
            vec![cmd::CHANGE_SIZE, 0x50, 0x00, 0x80, 0x02, 0xE0, 0x01, 0xFF],
        );
        send(
            &mut s,
            vec![
                cmd::CHANGE_BACKGROUND_COLOUR,
                0x50,
                0x00,
                0x09,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::CHANGE_OBJECT_LABEL,
                0x60,
                0x00,
                0x61,
                0x00,
                0x02,
                0xFF,
                0xFF,
            ],
        );
        let graphics_response = s.handle_ecu_message(&ecu_msg(
            vec![
                cmd::GRAPHICS_CONTEXT,
                0x70,
                0x00,
                0x0D,
                0x01,
                0x02,
                b'o',
                b'k',
            ],
            0x42,
        ));
        assert_eq!(graphics_response.len(), 1);
        assert_eq!(
            graphics_response[0].data,
            vec![cmd::GRAPHICS_CONTEXT, 0x70, 0x00, 0x0D, 0x00, 0xFF, 0xFF, 0xFF],
            "Graphics Context commands get the standard F.57 success response"
        );
        send(
            &mut s,
            vec![
                cmd::CONTROL_AUDIO_SIGNAL,
                2,
                0x34,
                0x12,
                0x78,
                0x56,
                0xBC,
                0x9A,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::SET_AUDIO_VOLUME,
                80,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
        );
        send(
            &mut s,
            vec![
                cmd::LOCK_UNLOCK_MASK,
                0x01,
                0x20,
                0x00,
                0xE8,
                0x03,
                0xFF,
                0xFF,
            ],
        );
        send(
            &mut s,
            vec![cmd::EXECUTE_MACRO, 0x99, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        );

        let state = &s.clients()[0].object_state;
        assert_eq!(state.visibility.get(&ObjectID(0x11)), Some(&true));
        assert_eq!(state.enable_state.get(&ObjectID(0x04)), Some(&false));
        assert_eq!(state.active_data_mask, ObjectID(0x02));
        assert_eq!(state.active_soft_key_mask, ObjectID(0x03));
        assert_eq!(state.attributes.get(&(ObjectID(0x10), 7)), Some(&2));
        assert_eq!(
            state.list_items.get(&(ObjectID(0x04), 0)),
            Some(&ObjectID(0x05))
        );
        assert_eq!(
            state.child_locations.get(&(ObjectID(0x01), ObjectID(0x02))),
            Some(&(12, 13))
        );
        assert_eq!(
            state.child_positions.get(&(ObjectID(0x01), ObjectID(0x02))),
            Some(&(0x1234, 0x5678))
        );
        assert_eq!(state.sizes.get(&ObjectID(0x50)), Some(&(640, 480)));
        assert_eq!(state.background_colours.get(&ObjectID(0x50)), Some(&9));
        assert_eq!(
            state.object_labels.get(&ObjectID(0x60)).copied(),
            Some(ObjectLabelState {
                string_variable: ObjectID(0x61),
                font_type: 2,
                graphic_designator: ObjectID::NULL
            })
        );
        assert_eq!(state.graphics_contexts.len(), 1);
        assert_eq!(state.graphics_contexts[0].object_id, ObjectID(0x70));
        assert_eq!(
            state.graphics_contexts[0].payload,
            vec![0x01, 0x02, b'o', b'k']
        );
        assert_eq!(state.audio_volume_percent, Some(80));
        assert_eq!(
            state.audio_signal,
            Some(AudioSignalState {
                activations: 2,
                frequency_hz: 0x1234,
                duration_ms: 0x5678,
                off_time_ms: 0x9ABC,
            })
        );
        assert_eq!(
            state.mask_locks.get(&ObjectID(0x20)),
            Some(&MaskLockState {
                locked: true,
                timeout_ms: 1000,
            })
        );
        assert_eq!(state.executed_macros, vec![ObjectID(0x99)]);

        // 0x02 is not a canonical Hide/Show value (F.2: 0 = Hide, 1 = Show).
        send_rejected(
            &mut s,
            vec![cmd::HIDE_SHOW, 0x11, 0x00, 0x02, 0xFF, 0xFF, 0xFF, 0xFF],
        );
        assert_eq!(
            s.clients()[0].object_state.visibility.get(&ObjectID(0x11)),
            Some(&true),
            "malformed canonical-bool command must not mutate cached VT state"
        );
    }

    /// C3 — F.35 gives an invalid Working Set bit 0 and an invalid mask bit 1.
    /// The table had the two swapped and folded the invalid-mask case into "any
    /// other error", so bit 0 was never set for any input and a Working Set
    /// whose runtime pool update removed the mask could not tell that the mask
    /// ID was the problem — it retried instead of re-uploading, and the alarm
    /// never came up.
    #[test]
    fn change_active_mask_separates_a_bad_working_set_from_a_bad_mask() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        activate_valid_pool(&mut s, 0x42);

        let error_byte = |server: &mut VTServer, ws: u16, mask: u16| {
            let [ws_lo, ws_hi] = ws.to_le_bytes();
            let [mask_lo, mask_hi] = mask.to_le_bytes();
            let out = server.handle_ecu_message(&ecu_msg(
                vec![
                    cmd::CHANGE_ACTIVE_MASK,
                    ws_lo,
                    ws_hi,
                    mask_lo,
                    mask_hi,
                    0xFF,
                    0xFF,
                    0xFF,
                ],
                0x42,
            ));
            assert_eq!(out.len(), 1, "F.1: the command must be answered");
            out[0].data[3]
        };

        assert_eq!(error_byte(&mut s, 0x0001, 0x0002), 0x00, "the valid pair");
        assert_eq!(
            error_byte(&mut s, 0x7FFF, 0x0002),
            0x01,
            "F.35 bit 0: invalid Working Set Object ID"
        );
        assert_eq!(
            error_byte(&mut s, 0x0001, 0x7FFE),
            0x02,
            "F.35 bit 1: invalid Data/Alarm Mask Object ID"
        );
    }

    #[test]
    fn unsupported_function_message_does_not_loop() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        let out = s.handle_ecu_message(&ecu_msg(
            vec![cmd::UNSUPPORTED_VT_FUNCTION, cmd::GRAPHICS_CONTEXT],
            0x42,
        ));
        assert!(out.is_empty());
    }

    #[test]
    fn ecu_numeric_change_fires_event() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        activate_valid_pool(&mut s, 0x42);
        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<(ObjectID, u32)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        s.on_numeric_value_change
            .subscribe(move |&v| lc.borrow_mut().push(v));
        let mut data = vec![cmd::CHANGE_NUMERIC_VALUE, 0x72, 0x00, 0xFF];
        data.extend(0x12345678u32.to_le_bytes());
        s.handle_ecu_message(&ecu_msg(data, 0x42));
        assert_eq!(*log.borrow(), vec![(ObjectID(0x72), 0x12345678u32)]);

        let mut bad_reserved = vec![cmd::CHANGE_NUMERIC_VALUE, 0x72, 0x00, 0x00];
        bad_reserved.extend(0x87654321u32.to_le_bytes());
        s.handle_ecu_message(&ecu_msg(bad_reserved, 0x42));
        assert_eq!(
            *log.borrow(),
            vec![(ObjectID(0x72), 0x12345678u32)],
            "bad reserved byte must be ignored"
        );
    }

    #[test]
    fn ecu_string_change_rejects_truncated_declared_length() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        activate_valid_pool(&mut s, 0x42);
        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<(ObjectID, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        s.on_string_value_change
            .subscribe(move |v| lc.borrow_mut().push(v.clone()));
        s.handle_ecu_message(&ecu_msg(
            vec![cmd::CHANGE_STRING_VALUE, 0x05, 0x00, 0x03, 0x00, b'h'],
            0x42,
        ));
        assert!(log.borrow().is_empty());

        s.handle_ecu_message(&ecu_msg(
            vec![
                cmd::CHANGE_STRING_VALUE,
                0x05,
                0x00,
                0x02,
                0x00,
                b'h',
                b'i',
                0x00,
            ],
            0x42,
        ));
        assert!(log.borrow().is_empty(), "bad trailing padding is ignored");
    }

    #[test]
    fn ecu_string_change_preserves_utf8_and_rejects_invalid_utf8() {
        let mut s = VTServer::new(VTServerConfig::default());
        s.start().unwrap();
        activate_valid_pool(&mut s, 0x42);
        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<(ObjectID, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        s.on_string_value_change
            .subscribe(move |v| lc.borrow_mut().push(v.clone()));

        let payload = "hé".as_bytes();
        let mut data = vec![cmd::CHANGE_STRING_VALUE, 0x05, 0x00];
        data.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        data.extend_from_slice(payload);
        s.handle_ecu_message(&ecu_msg(data, 0x42));
        assert_eq!(*log.borrow(), vec![(ObjectID(5), "hé".to_owned())]);

        s.handle_ecu_message(&ecu_msg(
            vec![
                cmd::CHANGE_STRING_VALUE,
                0x05,
                0x00,
                0x02,
                0x00,
                0xC3,
                0x28,
                0xFF,
            ],
            0x42,
        ));
        assert_eq!(
            *log.borrow(),
            vec![(ObjectID(5), "hé".to_owned())],
            "invalid UTF-8 string payload must be ignored"
        );
    }
}
