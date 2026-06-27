#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtc_round_trip() {
        let d = Dtc {
            spn: 0x1_2345,
            fmi: Fmi::AbnormalRateChange,
            occurrence_count: 7,
        };
        let bytes = d.encode();
        let decoded = Dtc::decode(&bytes).unwrap();
        assert_eq!(decoded, d);
    }

    #[test]
    fn dtc_max_spn() {
        // SPN is 19 bits → max value 0x7FFFF.
        let d = Dtc {
            spn: 0x7_FFFF,
            fmi: Fmi::ConditionExists,
            occurrence_count: 0x7F,
        };
        let decoded = Dtc::decode(&d.encode()).unwrap();
        assert_eq!(decoded.spn, 0x7_FFFF);
        assert_eq!(decoded.fmi, Fmi::ConditionExists);
        assert_eq!(decoded.occurrence_count, 0x7F);
    }

    #[test]
    fn spn_encoders_clamp_to_19_bits() {
        assert_eq!(
            Dtc {
                spn: 0x8_0000,
                fmi: Fmi::Erratic,
                occurrence_count: 0xFF,
            }
            .encode(),
            [0xFF, 0xFF, 0xE2, 0x7F]
        );
        assert_eq!(
            Dm7Command {
                spn: 0x8_0000,
                test_id: 5,
            }
            .encode(),
            [0xFF, 0xFF, 0x07, 0x05, 0xFF, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            Dm22Message {
                control: Dm22Control::ClearActive,
                nack_reason: None,
                spn: 0x8_0000,
                fmi: Fmi::Erratic,
            }
            .encode(),
            [0x11, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xE2]
        );
        assert_eq!(
            Dm25Request {
                spn: 0x8_0000,
                fmi: Fmi::Erratic,
                frame_number: 2,
            }
            .encode(),
            [0xFF, 0xFF, 0x07, 0x02, 0x02, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            MonitorPerformanceRatio {
                spn: 0x8_0000,
                numerator: 80,
                denominator: 100,
            }
            .encode(),
            [0xFF, 0xFF, 0x07, 0x50, 0x00, 0x64, 0x00]
        );
        assert_eq!(
            SpnSnapshot {
                spn: 0x8_0000,
                value: 1500,
            }
            .encode(),
            [0xFF, 0xFF, 0x07, 0xDC, 0x05, 0x00, 0x00]
        );
    }

    #[test]
    fn diagnostic_lamps_round_trip() {
        let l = DiagnosticLamps {
            malfunction: LampStatus::On,
            malfunction_flash: LampFlash::FastFlash,
            red_stop: LampStatus::Error,
            red_stop_flash: LampFlash::SlowFlash,
            amber_warning: LampStatus::On,
            amber_warning_flash: LampFlash::Off,
            engine_protect: LampStatus::NotAvailable,
            engine_protect_flash: LampFlash::NotAvailable,
        };
        let bytes = l.encode();
        let decoded = DiagnosticLamps::decode(&bytes).unwrap();
        assert_eq!(decoded, l);
    }

    #[test]
    fn primitive_diagnostic_chunks_reject_prefix_and_partial_payloads() {
        let dtc = Dtc {
            spn: 0x1_2345,
            fmi: Fmi::Erratic,
            occurrence_count: 7,
        }
        .encode();
        assert!(Dtc::decode(&dtc[..3]).is_none());
        assert!(Dtc::decode(&[dtc.as_slice(), &[0xFF]].concat()).is_none());

        let lamps = DiagnosticLamps::default().encode();
        assert!(DiagnosticLamps::decode(&lamps[..1]).is_none());
        assert!(DiagnosticLamps::decode(&[lamps.as_slice(), &[0xFF]].concat()).is_none());

        let ratio = MonitorPerformanceRatio {
            spn: 0x100,
            numerator: 80,
            denominator: 100,
        }
        .encode();
        assert!(MonitorPerformanceRatio::decode(&ratio[..6]).is_none());
        assert!(MonitorPerformanceRatio::decode(&[ratio.as_slice(), &[0xFF]].concat()).is_none());

        let snapshot = SpnSnapshot {
            spn: 0x200,
            value: 0xDEAD_BEEF,
        }
        .encode();
        assert!(SpnSnapshot::decode(&snapshot[..6]).is_none());
        assert!(SpnSnapshot::decode(&[snapshot.as_slice(), &[0xFF]].concat()).is_none());
    }

    #[test]
    fn dm_dtc_list_round_trip_multi() {
        let list = DmDtcList {
            lamps: DiagnosticLamps {
                malfunction: LampStatus::On,
                ..Default::default()
            },
            dtcs: vec![
                Dtc {
                    spn: 100,
                    fmi: Fmi::AboveNormal,
                    occurrence_count: 1,
                },
                Dtc {
                    spn: 200,
                    fmi: Fmi::VoltageHigh,
                    occurrence_count: 5,
                },
            ],
        };
        let bytes = list.encode();
        let decoded = DmDtcList::decode(&bytes).unwrap();
        assert_eq!(decoded.lamps.malfunction, LampStatus::On);
        assert_eq!(decoded.dtcs.len(), 2);
        assert_eq!(decoded.dtcs[0].spn, 100);
        assert_eq!(decoded.dtcs[1].spn, 200);
    }

    #[test]
    fn dm_dtc_list_empty_pads_to_8_bytes() {
        let bytes = DmDtcList::default().encode();
        assert_eq!(bytes.len(), 8);
    }

    #[test]
    fn dm_dtc_list_rejects_short_and_misaligned_payloads() {
        let valid = DmDtcList {
            lamps: DiagnosticLamps::default(),
            dtcs: vec![Dtc {
                spn: 100,
                fmi: Fmi::AboveNormal,
                occurrence_count: 0,
            }],
        }
        .encode();

        assert!(DmDtcList::decode(&valid[..7]).is_none());

        let mut misaligned_multi_packet = DmDtcList {
            lamps: DiagnosticLamps::default(),
            dtcs: vec![
                Dtc {
                    spn: 100,
                    fmi: Fmi::AboveNormal,
                    occurrence_count: 0,
                },
                Dtc {
                    spn: 200,
                    fmi: Fmi::VoltageHigh,
                    occurrence_count: 0,
                },
            ],
        }
        .encode();
        misaligned_multi_packet.push(0xEE);
        assert!(DmDtcList::decode(&misaligned_multi_packet).is_none());
    }

    #[test]
    fn dm_clear_all_request_uses_reserved_ff_payload() {
        let bytes = DmClearAllRequest.encode();
        assert_eq!(bytes, [0xFF; 8]);
        assert_eq!(DmClearAllRequest::decode(&bytes), Some(DmClearAllRequest));
        assert!(DmClearAllRequest::decode(&bytes[..7]).is_none());
        assert!(
            DmClearAllRequest::decode(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF, 0xFF, 0xFF]).is_none()
        );
    }

    #[test]
    fn fixed_size_diagnostic_decoders_reject_short_and_overlong_payloads() {
        let short = [0x00; 7];
        let overlong = [0x00; 9];

        assert!(DmClearAllRequest::decode(&short).is_none());
        assert!(DmClearAllRequest::decode(&overlong).is_none());
        assert!(Dm7Command::decode(&short).is_none());
        assert!(Dm7Command::decode(&overlong).is_none());
        assert!(Dm13Signals::decode(&short).is_none());
        assert!(Dm13Signals::decode(&overlong).is_none());
        assert!(Dm22Message::decode(&short).is_none());
        assert!(Dm22Message::decode(&overlong).is_none());
        assert!(Dm25Request::decode(&short).is_none());
        assert!(Dm25Request::decode(&overlong).is_none());
    }

    #[test]
    fn dm4_round_trip() {
        let m = Dm4Message {
            mil_status: LampStatus::On,
            red_stop_lamp: LampStatus::Off,
            amber_warning: LampStatus::On,
            protect_lamp: LampStatus::Error,
            dtcs: vec![Dtc {
                spn: 42,
                fmi: Fmi::CurrentLow,
                occurrence_count: 3,
            }],
        };
        let bytes = m.encode();
        let decoded = Dm4Message::decode(&bytes).unwrap();
        assert_eq!(decoded.mil_status, LampStatus::On);
        assert_eq!(decoded.dtcs, m.dtcs);
    }

    #[test]
    fn dm7_round_trip() {
        let cmd = Dm7Command {
            spn: 0x1234,
            test_id: 5,
        };
        let decoded = Dm7Command::decode(&cmd.encode()).unwrap();
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn dm8_round_trip() {
        let r = Dm8TestResult {
            spn: 0x5678,
            test_id: 1,
            test_result: 0,
            test_value: 1234,
            test_limit_min: 1000,
            test_limit_max: 1500,
        };
        let decoded = Dm8TestResult::decode(&r.encode()).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn diagnostic_multibyte_records_reject_truncated_or_trailing_payloads() {
        let dm8 = Dm8TestResult {
            spn: 0x5678,
            test_id: 1,
            test_result: 0,
            test_value: 1234,
            test_limit_min: 1000,
            test_limit_max: 1500,
        }
        .encode();
        assert!(Dm8TestResult::decode(&dm8[..10]).is_none());
        assert!(Dm8TestResult::decode(&[dm8.as_slice(), &[0xFF]].concat()).is_none());

        let dm21 = Dm21Readiness {
            distance_with_mil_on_km: 100,
            distance_since_codes_cleared_km: 5_000,
            minutes_with_mil_on: 60,
            time_since_codes_cleared_min: 1_440,
            comprehensive_component: 0xAA,
            fuel_system: 0xBB,
            misfire: 0xCC,
        }
        .encode();
        assert!(Dm21Readiness::decode(&dm21[..10]).is_none());
        assert!(Dm21Readiness::decode(&[dm21.as_slice(), &[0xFF]].concat()).is_none());
    }

    #[test]
    fn dm13_round_trip() {
        let s = Dm13Signals {
            primary_vehicle_network: Dm13Command::SuspendBroadcast,
            sae_j1922_network: Dm13Command::DoNotCare,
            sae_j1587_network: Dm13Command::DoNotCare,
            current_data_link: Dm13Command::DoNotCare,
            suspend_signal: Dm13SuspendSignal::PartialTemporarySuspension,
            suspend_duration_s: 60,
        };
        let decoded = Dm13Signals::decode(&s.encode()).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn dm21_round_trip() {
        let r = Dm21Readiness {
            distance_with_mil_on_km: 100,
            distance_since_codes_cleared_km: 5_000,
            minutes_with_mil_on: 60,
            time_since_codes_cleared_min: 1_440,
            comprehensive_component: 0xAA,
            fuel_system: 0xBB,
            misfire: 0xCC,
        };
        let decoded = Dm21Readiness::decode(&r.encode()).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn dm22_round_trip() {
        let m = Dm22Message {
            control: Dm22Control::ClearActive,
            nack_reason: None,
            spn: 0x1_2345,
            fmi: Fmi::Erratic,
        };
        let decoded = Dm22Message::decode(&m.encode()).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn dm22_unknown_control_byte_returns_none() {
        let bytes = [0xAA, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0xFF];
        assert!(Dm22Message::decode(&bytes).is_none());
    }

    #[test]
    fn dm22_rejects_reserved_padding_bytes() {
        let mut bytes = Dm22Message {
            control: Dm22Control::ClearActive,
            nack_reason: None,
            spn: 0x1_2345,
            fmi: Fmi::Erratic,
        }
        .encode();
        bytes[1] = 0x00;
        assert!(Dm22Message::decode(&bytes).is_none());

        let mut bytes = Dm22Message {
            control: Dm22Control::ClearActive,
            nack_reason: None,
            spn: 0x1_2345,
            fmi: Fmi::Erratic,
        }
        .encode();
        bytes[4] = 0x00;
        assert!(Dm22Message::decode(&bytes).is_none());
    }

    #[test]
    fn dm5_supports_query() {
        let id = DiagnosticProtocolId {
            protocols: DiagProtocol::J1939_73.as_u8() | DiagProtocol::Iso14229_3.as_u8(),
        };
        assert!(id.supports(DiagProtocol::J1939_73));
        assert!(id.supports(DiagProtocol::Iso14229_3));
        assert!(!id.supports(DiagProtocol::Iso14230));
    }

    #[test]
    fn dm5_round_trip_and_rejects_malformed_fixed_size_payloads() {
        let id = DiagnosticProtocolId {
            protocols: DiagProtocol::J1939_73.as_u8() | DiagProtocol::Iso14229_3.as_u8(),
        };
        let alias: Dm5Message = id;
        assert_eq!(alias, id);
        assert_eq!(DiagnosticProtocolId::decode(&id.encode()), Some(id));
        assert!(
            DiagnosticProtocolId::decode(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_none()
        );
        assert!(
            DiagnosticProtocolId::decode(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00])
                .is_none()
        );
        assert!(
            DiagnosticProtocolId::decode(&[0x01, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
                .is_none()
        );
    }

    #[test]
    fn dm9_vehicle_identification_request_round_trips_only_vehicle_id_pgn() {
        let request = Dm9VehicleIdentificationRequest;
        let bytes = request.encode().unwrap();
        assert_eq!(bytes, [0xEC, 0xFE, 0x00]);
        assert_eq!(
            Dm9VehicleIdentificationRequest::decode(&bytes),
            Some(request)
        );
        assert_eq!(
            Dm9VehicleIdentificationRequest::decode(&[
                0xEC, 0xFE, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF
            ]),
            Some(request)
        );
        assert!(Dm9VehicleIdentificationRequest::decode(&[0xDA, 0xFE, 0x00]).is_none());
        assert!(
            Dm9VehicleIdentificationRequest::decode(&[
                0xEC, 0xFE, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF
            ])
            .is_none()
        );
    }

    #[test]
    fn dm10_vehicle_identification_round_trip_and_validates_ascii() {
        let vin = Dm10VehicleIdentification {
            vin: "1HGBH41JXMN109186".into(),
        };
        let bytes = vin.encode().unwrap();
        assert_eq!(bytes, b"1HGBH41JXMN109186*");
        assert_eq!(Dm10VehicleIdentification::decode(&bytes), Some(vin));

        assert!(Dm10VehicleIdentification::decode(b"1HGBH41JXMN109186").is_none());
        assert!(Dm10VehicleIdentification::decode(b"1HGBH41JXMN109186*extra*").is_none());
        assert!(Dm10VehicleIdentification::decode(b"1HGBH41JXMN109186\x80*").is_none());
        assert!(
            Dm10VehicleIdentification {
                vin: "BAD*VIN".into(),
            }
            .encode()
            .is_err()
        );
        assert!(
            Dm10VehicleIdentification {
                vin: "BAD\nVIN".into(),
            }
            .encode()
            .is_err()
        );
    }

    #[test]
    fn product_id_round_trip() {
        let p = ProductIdentification {
            make: "Acme".into(),
            model: "X42".into(),
            serial_number: "SN-1".into(),
        };
        let decoded = ProductIdentification::decode(&p.encode().unwrap()).unwrap();
        assert_eq!(decoded, p);
    }

    #[test]
    fn product_id_encode_rejects_delimiters_and_non_printable_text() {
        let with_delimiter = ProductIdentification {
            make: "Ac*me".into(),
            model: "X42".into(),
            serial_number: "SN-1".into(),
        };
        assert!(with_delimiter.encode().is_err());

        let with_newline = ProductIdentification {
            make: "Acme".into(),
            model: "X42\n".into(),
            serial_number: "SN-1".into(),
        };
        assert!(with_newline.encode().is_err());
    }

    #[test]
    fn product_id_rejects_missing_extra_or_invalid_text_fields() {
        assert!(ProductIdentification::decode(b"Acme*X42*").is_none());
        assert!(ProductIdentification::decode(b"Acme*X42*SN-1*extra*").is_none());
        assert_eq!(
            ProductIdentification::decode(b"Acme*X42*SN-\xFF*"),
            Some(ProductIdentification {
                make: "Acme".into(),
                model: "X42".into(),
                serial_number: "SN-ÿ".into(),
            })
        );
        assert!(ProductIdentification::decode(b"Acme*X42*SN-\x80*").is_none());
    }

    #[test]
    fn software_id_round_trip() {
        let s = SoftwareIdentification {
            versions: vec!["1.0.0".into(), "1.0.1".into(), "BETA".into()],
        };
        let decoded = SoftwareIdentification::decode(&s.encode().unwrap()).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn software_id_encode_rejects_delimiters_and_non_printable_text() {
        assert!(
            SoftwareIdentification {
                versions: vec!["1.0*bad".into()],
            }
            .encode()
            .is_err()
        );
        assert!(
            SoftwareIdentification {
                versions: vec!["1.0\nbad".into()],
            }
            .encode()
            .is_err()
        );
    }

    #[test]
    fn software_id_rejects_missing_final_delimiter_and_invalid_text_fields() {
        assert!(SoftwareIdentification::decode(b"").is_none());
        assert!(SoftwareIdentification::decode(b"1.0.0*1.0.1").is_none());
        assert_eq!(
            SoftwareIdentification::decode(b"1.0.\xFF*"),
            Some(SoftwareIdentification {
                versions: vec!["1.0.ÿ".into()],
            })
        );
        assert!(SoftwareIdentification::decode(b"1.0.\x80*").is_none());
        assert!(SoftwareIdentification::decode(b"\x00extra*").is_none());
    }

    #[test]
    fn monitor_performance_ratio_percentage() {
        let r = MonitorPerformanceRatio {
            spn: 0,
            numerator: 75,
            denominator: 100,
        };
        assert_eq!(r.percentage(), 75);
        assert!(r.meets_threshold(75));
        assert!(!r.meets_threshold(76));

        let zero = MonitorPerformanceRatio {
            spn: 0,
            numerator: 5,
            denominator: 0,
        };
        assert_eq!(zero.percentage(), 0);
    }

    #[test]
    fn dm20_round_trip() {
        let r = Dm20Response {
            ignition_cycles: 10,
            obd_monitoring_conditions_met: 5,
            ratios: vec![
                MonitorPerformanceRatio {
                    spn: 0x100,
                    numerator: 80,
                    denominator: 100,
                },
                MonitorPerformanceRatio {
                    spn: 0x200,
                    numerator: 50,
                    denominator: 60,
                },
            ],
        };
        let decoded = Dm20Response::decode(&r.encode()).unwrap();
        assert_eq!(decoded.ignition_cycles, 10);
        assert_eq!(decoded.ratios.len(), 2);
        assert_eq!(decoded.ratios[0].spn, 0x100);
        assert_eq!(decoded.ratios[1].percentage(), 83);
    }

    #[test]
    fn dm20_rejects_short_and_misaligned_payloads() {
        let empty = Dm20Response::default().encode();
        assert_eq!(empty.len(), 8);
        assert!(Dm20Response::decode(&empty[..7]).is_none());

        let one_ratio = Dm20Response {
            ignition_cycles: 10,
            obd_monitoring_conditions_met: 5,
            ratios: vec![MonitorPerformanceRatio {
                spn: 0x100,
                numerator: 80,
                denominator: 100,
            }],
        }
        .encode();
        assert!(Dm20Response::decode(&one_ratio).is_some());
        assert!(Dm20Response::decode(&one_ratio[..8]).is_none());
        assert!(Dm20Response::decode(&[one_ratio.as_slice(), &[0xFF]].concat()).is_none());
    }

    #[test]
    fn freeze_frame_round_trip() {
        let ff = FreezeFrame {
            dtc: Dtc {
                spn: 0x123,
                fmi: Fmi::VoltageHigh,
                occurrence_count: 2,
            },
            timestamp_ms: 0xCAFE_F00D,
            snapshots: vec![
                SpnSnapshot {
                    spn: 0x100,
                    value: 1500,
                },
                SpnSnapshot {
                    spn: 0x200,
                    value: 0xDEAD_BEEF,
                },
            ],
        };
        let decoded = FreezeFrame::decode(&ff.encode().unwrap()).unwrap();
        assert_eq!(decoded, ff);
    }

    #[test]
    fn dm25_entry_round_trip_and_length_prefix() {
        let ff = FreezeFrame {
            dtc: Dtc {
                spn: 0x123,
                fmi: Fmi::VoltageHigh,
                occurrence_count: 2,
            },
            timestamp_ms: 999, // not part of the DM25 wire entry
            snapshots: vec![
                SpnSnapshot {
                    spn: 0x100,
                    value: 1500,
                },
                SpnSnapshot {
                    spn: 0x200,
                    value: 0xDEAD_BEEF,
                },
            ],
        };
        let bytes = ff.encode_dm25();
        // length byte = 4 (DTC) + 2×7 (snapshots) = 18.
        assert_eq!(bytes[0], 18);
        assert_eq!(bytes.len(), 1 + 18);

        let (decoded, consumed) = FreezeFrame::decode_dm25(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.dtc, ff.dtc);
        assert_eq!(decoded.snapshots, ff.snapshots);
        assert_eq!(decoded.timestamp_ms, 0); // not carried on the wire

        // Two concatenated entries walk correctly.
        let mut two = ff.encode_dm25();
        two.extend_from_slice(&ff.encode_dm25());
        let (_first, n) = FreezeFrame::decode_dm25(&two).unwrap();
        let (_second, m) = FreezeFrame::decode_dm25(&two[n..]).unwrap();
        assert_eq!(n + m, two.len());
        // Truncated / malformed entries are rejected.
        assert!(FreezeFrame::decode_dm25(&[]).is_none());
        assert!(FreezeFrame::decode_dm25(&[18, 0, 0]).is_none());
    }

    #[test]
    fn freeze_frame_rejects_count_mismatches_and_trailing_payloads() {
        let ff = FreezeFrame {
            dtc: Dtc {
                spn: 0x123,
                fmi: Fmi::VoltageHigh,
                occurrence_count: 2,
            },
            timestamp_ms: 0xCAFE_F00D,
            snapshots: vec![SpnSnapshot {
                spn: 0x100,
                value: 1500,
            }],
        };
        let bytes = ff.encode().unwrap();
        assert!(FreezeFrame::decode(&bytes[..bytes.len() - 1]).is_none());
        assert!(FreezeFrame::decode(&[bytes.as_slice(), &[0xFF]].concat()).is_none());

        let mut count_mismatch = bytes.clone();
        count_mismatch[8] = 2;
        assert!(FreezeFrame::decode(&count_mismatch).is_none());
    }

    #[test]
    fn freeze_frame_encode_rejects_overwide_snapshot_count() {
        let frame = FreezeFrame {
            snapshots: vec![SpnSnapshot::default(); 256],
            ..FreezeFrame::default()
        };
        assert!(frame.encode().is_err());
    }

    #[test]
    fn dm25_request_round_trip() {
        let r = Dm25Request {
            spn: 0x1_2345,
            fmi: Fmi::Erratic,
            frame_number: 2,
        };
        let decoded = Dm25Request::decode(&r.encode()).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn fmi_round_trip_all_known_values() {
        for v in 0u8..=31 {
            let f = Fmi::from_u8(v);
            // Reserved values (11..=14 except 11, 12, 13, 14; 19; rest <=19)
            // map to themselves where defined, else to RootCauseUnknown.
            let _ = f.as_u8();
        }
        assert_eq!(Fmi::from_u8(0), Fmi::AboveNormal);
        assert_eq!(Fmi::from_u8(20), Fmi::DataDriftedHigh);
        assert_eq!(Fmi::from_u8(21), Fmi::DataDriftedLow);
        assert_eq!(Fmi::from_u8(31), Fmi::ConditionExists);
        assert_eq!(Fmi::try_from_u8(22), None);
        // Unknown reserved values still map to the conservative default for
        // lossy caller-provided values, but wire decoders use try_from_u8.
        assert_eq!(Fmi::from_u8(22), Fmi::RootCauseUnknown);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_diagnostic_decoders_accept_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=512),
        ) {
            if let Some(dtc) = Dtc::decode(&data) {
                prop_assert_eq!(Dtc::decode(&dtc.encode()), Some(dtc));
            }
            if let Some(lamps) = DiagnosticLamps::decode(&data) {
                prop_assert_eq!(DiagnosticLamps::decode(&lamps.encode()), Some(lamps));
            }
            if let Some(list) = DmDtcList::decode(&data) {
                prop_assert!(list.dtcs.len() <= data.len() / 4);
                let _ = list.encode();
            }
            let _ = DmClearAllRequest::decode(&data).map(DmClearAllRequest::encode);
            let _ = Dm4Message::decode(&data).map(|m| m.encode());
            let _ = Dm7Command::decode(&data).map(|m| m.encode());
            let _ = Dm8TestResult::decode(&data).map(|m| m.encode());
            let _ = Dm13Signals::decode(&data).map(|m| m.encode());
            let _ = Dm21Readiness::decode(&data).map(|m| m.encode());
            let _ = Dm22Message::decode(&data).map(|m| m.encode());
            let _ = DiagnosticProtocolId::decode(&data).map(|m| m.encode());
            let _ = Dm9VehicleIdentificationRequest::decode(&data).map(|m| m.encode());
            let _ = Dm10VehicleIdentification::decode(&data).map(|m| m.encode());
            let _ = ProductIdentification::decode(&data).map(|m| m.encode());
            let _ = SoftwareIdentification::decode(&data).map(|m| m.encode());
            let _ = MonitorPerformanceRatio::decode(&data).map(|m| m.encode());
            if let Some(dm20) = Dm20Response::decode(&data) {
                prop_assert!(dm20.ratios.len() <= data.len() / 7);
                let _ = dm20.encode();
            }
            let _ = SpnSnapshot::decode(&data).map(|m| m.encode());
            if let Some(frame) = FreezeFrame::decode(&data) {
                prop_assert!(frame.snapshots.len() <= data.len() / 7);
                let _ = frame.encode();
            }
            let _ = Dm25Request::decode(&data).map(|m| m.encode());
        }
    }
}
