#![cfg(feature = "embedded")]

extern crate alloc;

use alloc::vec::Vec;

use machbus::geo::Wgs;
use machbus::isobus::fs::{
    FSError, FSFunction, FileClient, FileClientConfig, FileServer, FileServerConfig, OpenFlags,
    VolumeStateV2, VolumeStatus,
};
use machbus::isobus::tc::{
    DDI, DDOP, DeviceElement, DeviceElementType, DeviceObject, ElementNumber, OutstandingRequests,
    PrescriptionController, PrescriptionMap, PrescriptionZone, ProcessDataRateLimiter, TaskSession,
    TaskTotals, TreatmentZoneGrid, point_in_polygon, prescription_rate_process_data_payload,
};
use machbus::isobus::vt::{
    DataMaskBody, GraphicsContextCommand, ObjectLabelState, ObjectPool, ServerRenderEffect,
    StoredPoolVersion as VtStoredPoolVersion, VTClient, VTClientConfig, VTServer, VTServerConfig,
    VTState, WorkingSet, WorkingSetBody, create_data_mask, create_working_set,
};
use machbus::isobus::{
    AuxFunctionState, AuxFunctionType, AuxNFunction, CurvatureCommand, Functionalities,
    GroundBasedSpeedDist, GroupFunctionMsg, GuidanceData, LightState, LightingState,
    MachineSpeedCommandMsg, SCD_LABEL_NONE, SCSequenceState, ScdAction, SequenceRecorder,
    SequenceTanTracker, TimOption, TimOptionSet, TractorFacilities, scd_action,
};
use machbus::j1939::{Fmi, diagnostic::Dtc};
use machbus::net::{
    BROADCAST_ADDRESS, CanTransport, Frame, Identifier, Name, Priority, hash_to_version,
    parse_iop_data, pgn_defs::PGN_REQUEST,
};
use machbus::nmea::{GNSSPosition, NMEAInterface};
use machbus::session::{Session, Transport};
use machbus::time::Instant;
use machbus::vt_storage::StoredPoolVersion;

struct SurfaceTransport {
    rx: Option<(u8, Frame)>,
    tx: Vec<(u8, Frame)>,
}

impl CanTransport for SurfaceTransport {
    type Error = machbus::net::Error;

    fn recv(&mut self) -> Option<(u8, Frame)> {
        self.rx.take()
    }

    fn send(&mut self, port: u8, frame: &Frame) -> machbus::net::Result<()> {
        self.tx.push((port, *frame));
        Ok(())
    }
}

fn request_for_address_claim() -> Frame {
    Frame::new(
        Identifier::encode(Priority::Default, PGN_REQUEST, 0x20, BROADCAST_ADDRESS),
        [0x00, 0xEE, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        8,
    )
}

#[test]
fn embedded_public_surface_imports_and_runs_minimal_loop() -> machbus::net::Result<()> {
    let name = Name::default()
        .with_identity_number(0x55)
        .with_function_code(0x80)
        .with_self_configurable(true);
    let mut session = Session::builder(name, 0x80)
        .fast_packet_pgn(129_029)
        .build()?;
    let mut transport = SurfaceTransport {
        rx: Some((0, request_for_address_claim())),
        tx: Vec::new(),
    };
    let now = Instant::ZERO.add_millis(10);

    session.start()?;
    while let Some((port, frame)) = transport.recv() {
        session.feed(port, &frame, now);
    }
    session.tick(now);
    while let Some((port, frame)) = session.poll_transmit() {
        transport.send(port, &frame)?;
    }

    let gnss = GNSSPosition {
        wgs: Wgs::new(52.0, 5.0, 1.0),
        ..Default::default()
    };
    let _position_payload = NMEAInterface::build_position(&gnss);
    let _dtc = Dtc {
        spn: 123,
        fmi: Fmi::AboveNormal,
        occurrence_count: 1,
    };

    let iop = [0x01, 0x00, 21, 0x0D, 0xF0, 0xFE, 0xCA];
    assert_eq!(parse_iop_data(&iop)?.len(), 1);
    assert!(!hash_to_version(&iop).is_empty());

    let aux = AuxNFunction {
        function_number: 1,
        r#type: AuxFunctionType::Type1,
        state: AuxFunctionState::Variable,
        setpoint: 42,
    };
    assert_eq!(aux.encode().len(), 8);
    assert_eq!(GuidanceData::default().encode().len(), 8);
    assert!(!Functionalities::new().with_min_cf(1).serialize().is_empty());
    assert!(
        GroupFunctionMsg::acknowledge(PGN_REQUEST, Default::default())
            .encode()
            .is_ok()
    );
    assert!(
        TimOptionSet::from_options(&[TimOption::RearPtoEngagementCwIsSupported])
            .contains(TimOption::RearPtoEngagementCwIsSupported)
    );
    assert_eq!(
        GroundBasedSpeedDist::decode(&GroundBasedSpeedDist::default().encode()),
        Some(GroundBasedSpeedDist::default())
    );
    assert_eq!(CurvatureCommand::default().encode().len(), 8);
    assert_eq!(
        LightingState {
            front_work: LightState::On,
            ..Default::default()
        }
        .encode()
        .len(),
        8
    );
    let facilities = TractorFacilities::default().with_class1_all();
    assert_eq!(
        TractorFacilities::decode(&facilities.encode()),
        Some(facilities)
    );
    assert_eq!(MachineSpeedCommandMsg::default().encode().len(), 8);
    let mut recorder = SequenceRecorder::new();
    recorder.start().expect("recorder starts from ready");
    assert_eq!(recorder.record("raise hitch", 100)?, 0);
    assert_eq!(recorder.complete()?.len(), 1);
    assert_eq!(recorder.state(), SCSequenceState::RecordingCompletion);
    assert_eq!(
        scd_action(SCD_LABEL_NONE, [1, 2, 3, 4, 5, 6, 7]),
        ScdAction::Upload
    );
    let mut tan = SequenceTanTracker::new();
    let first_tan = tan.allocate();
    tan.start(first_tan);
    assert_eq!(tan.update(100), Some(first_tan));

    let mut fs_client = FileClient::new(FileClientConfig::default());
    let props_request = fs_client.try_connect_to_server(0x44)?;
    assert_eq!(
        props_request.data[0],
        FSFunction::GetFileServerProperties.as_u8()
    );
    assert!(OpenFlags::Create.bit() != 0);
    assert_eq!(
        FSError::try_from_u8(FSError::EndOfFile.as_u8()),
        Some(FSError::EndOfFile)
    );

    let mut fs_server = FileServer::new(FileServerConfig::default());
    fs_server.add_file("\\TASKDATA.XML", b"<ISO11783_TaskData/>".to_vec(), 0)?;
    let entries = fs_server.list_directory("\\", "*");
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].is_directory());
    assert!(
        VolumeStatus {
            state: VolumeStateV2::Mounted,
            name: "ISOBUS".into(),
            total_bytes: 1024,
            free_bytes: 512,
            removable: false,
        }
        .encode()
        .is_ok()
    );

    let vt_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    let vt_bytes = vt_pool.serialize()?;
    let vt_restored = ObjectPool::deserialize(&vt_bytes)?;
    assert_eq!(vt_restored.size(), 2);
    let mut vt_client = VTClient::new(VTClientConfig::default());
    vt_client.set_object_pool(vt_restored);
    let mut vt_ws = WorkingSet::default();
    vt_ws.set_active_mask(2u16);
    vt_client.set_working_set(vt_ws);
    vt_client.connect()?;
    assert_eq!(vt_client.state(), VTState::WaitForVTStatus);
    let mut vt_server = VTServer::new(VTServerConfig::default());
    vt_server.start()?;
    assert!(vt_server.update(1000).is_some());
    let label = ObjectLabelState {
        string_variable: 4u16.into(),
        font_type: 1,
        graphic_designator: 0u16.into(),
    };
    let effects = alloc::vec![
        ServerRenderEffect::ChangeObjectLabel {
            id: 3u16.into(),
            label,
        },
        ServerRenderEffect::GraphicsContext {
            id: 7u16.into(),
            subcommand: 0x02,
            payload: alloc::vec![1],
        },
    ];
    assert!(matches!(
        effects.as_slice(),
        [
            ServerRenderEffect::ChangeObjectLabel { label: seen_label, .. },
            ServerRenderEffect::GraphicsContext { subcommand: 0x02, .. },
        ] if *seen_label == label
    ));
    let graphics_replay = GraphicsContextCommand {
        object_id: 7u16.into(),
        subcommand: 0x09,
        payload: alloc::vec![1, 0, 1, 0],
    };
    assert_eq!(graphics_replay.payload.len(), 4);
    let mut vt_stored = VtStoredPoolVersion {
        label: "VTP01".into(),
        pool_data: vt_bytes,
        ..Default::default()
    };
    vt_stored.update_metadata_at(5, 0);
    assert!(vt_stored.to_storage_bytes().is_some());

    let mut pool = DDOP::default();
    let device_id = pool.add_device(DeviceObject::default().with_designator("ECU"))?;
    let element_id = pool.add_element(
        DeviceElement::default()
            .with_type(DeviceElementType::Device)
            .with_parent(device_id)
            .with_designator("implement"),
    )?;
    assert_ne!(device_id, element_id);
    assert!(pool.validate().is_ok());

    let mut task = TaskSession::new();
    task.start()?;
    assert!(task.log_value(ElementNumber(1), DDI(0x006C), 42));
    let mut controller = PrescriptionController::new();
    assert_eq!(controller.command(&task, Some(5)).rate, Some(5));

    let mut outstanding = OutstandingRequests::new();
    assert!(outstanding.try_begin(0x80));
    assert!(!outstanding.try_begin(0x80));
    outstanding.complete(0x80);
    assert!(outstanding.is_empty());

    let mut limiter = ProcessDataRateLimiter::new();
    assert!(limiter.allow(1, 0x006C));
    assert!(!limiter.allow(1, 0x006C));
    limiter.tick(100);
    assert!(limiter.allow(1, 0x006C));

    let mut totals = TaskTotals::new();
    totals.accumulate(0x006C, 7);
    assert_eq!(totals.task_total(0x006C), 7);
    assert!(!totals.export_lifetime_totals().is_empty());

    let polygon = [
        Wgs::new(0.0, 0.0, 0.0),
        Wgs::new(0.0, 1.0, 0.0),
        Wgs::new(1.0, 1.0, 0.0),
        Wgs::new(1.0, 0.0, 0.0),
    ];
    assert!(point_in_polygon(Wgs::new(0.5, 0.5, 0.0), &polygon));
    let map = PrescriptionMap {
        structure_label: "rate-map".into(),
        zones: alloc::vec![PrescriptionZone {
            boundary: polygon.to_vec(),
            holes: Vec::new(),
            application_rate: 123,
        }],
    };
    assert_eq!(
        machbus::isobus::tc::point_in_prescription_zone(Wgs::new(0.5, 0.5, 0.0), &map.zones[0]),
        true
    );
    let grid = TreatmentZoneGrid {
        origin: Wgs::new(0.0, 0.0, 0.0),
        cell_lat_deg: 1.0,
        cell_lon_deg: 1.0,
        rows: 1,
        cols: 1,
        cells: alloc::vec![9],
    };
    assert_eq!(grid.zone_at(Wgs::new(0.5, 0.5, 0.0)), Some(9));
    let rate_payload = prescription_rate_process_data_payload(DDI(0x006C), 10)?;
    assert_eq!(rate_payload.len(), 8);

    let mut stored_pool = StoredPoolVersion {
        label: "POOL01".into(),
        pool_data: iop.to_vec(),
        ..Default::default()
    };
    stored_pool.update_metadata_at(6, 0);
    assert!(stored_pool.to_storage_bytes().is_some());

    #[cfg(feature = "embedded")]
    {
        let _ = session.poll_fixed_event::<8>();
        let mut fp = machbus::net::FastPacketProtocol::new();
        let frames = fp.send_fixed::<4>(129_029, &[0x55; 20], 0x80)?;
        assert_eq!(frames.len(), 3);
        let mut fp_rx = machbus::net::FastPacketProtocol::new();
        let mut received = None;
        for frame in frames.iter() {
            received = fp_rx.process_frame_fixed::<32>(frame)?.or(received);
        }
        let received = received.expect("fast packet completes");
        assert_eq!(received.size(), 20);
        assert_eq!(received.data.as_slice(), &[0x55; 20]);
        let mut tp = machbus::net::TransportProtocol::new();
        let bam = tp.send_bam_fixed::<4>(PGN_REQUEST, &[0x33; 20], 0x80, Priority::Default)?;
        assert_eq!(bam.len(), 4);
        assert_eq!(bam[0].pgn(), machbus::net::pgn_defs::PGN_TP_CM);
        assert_eq!(bam[1].pgn(), machbus::net::pgn_defs::PGN_TP_DT);
        assert_eq!(bam[1].payload()[0], 1);
        assert_eq!(bam[3].payload()[0], 3);
        let mut tp_rx = machbus::net::TpRxFixed::<32>::new();
        assert!(tp_rx.process_frame(&bam[0])?.message.is_none());
        assert!(tp_rx.is_active());
        let mut tp_message = None;
        for frame in bam.iter().skip(1) {
            let outcome = tp_rx.process_frame(frame)?;
            if outcome.message.is_some() {
                tp_message = outcome.message;
            }
        }
        let tp_message = tp_message.expect("fixed TP BAM completes");
        assert_eq!(tp_message.pgn, PGN_REQUEST);
        assert_eq!(tp_message.data.as_slice(), &[0x33; 20]);
        assert!(!tp_rx.is_active());
        let mut cmdt = machbus::net::TpCmdtTx::new(PGN_REQUEST, &[0x44; 20], 0x80, 0x90)?;
        let rts = cmdt.rts();
        assert_eq!(rts.pgn(), machbus::net::pgn_defs::PGN_TP_CM);
        cmdt.set_window(1, 2)?;
        let cmdt_frames = cmdt.pending_data_frames_fixed::<2>()?;
        assert_eq!(cmdt_frames.len(), 2);
        assert_eq!(cmdt_frames[0].payload()[0], 1);
        assert_eq!(cmdt_frames[1].payload()[0], 2);
        cmdt.set_window(3, 1)?;
        let last_cmdt_frames = cmdt.pending_data_frames_fixed::<1>()?;
        assert_eq!(last_cmdt_frames[0].payload()[0], 3);
        assert!(cmdt.is_complete());
        let pending = tp.get_pending_data_frames_fixed::<4>()?;
        assert!(pending.is_empty());
        tp.track_session(
            0x80,
            0x90,
            PGN_REQUEST,
            machbus::net::TpSessionState::WaitForCts,
            0,
        );
        assert_eq!(tp.timer_sessions_iter().count(), 1);
        let mut etp = machbus::net::ExtendedTransportProtocol::new();
        let payload = [0xAA; 1792];
        let pgn = 0xFECA;
        let rts = etp.send(pgn, &payload, 0x80, 0x90, 0, Priority::Default)?;
        assert_eq!(rts.len(), 1);
        let cts = Frame::new(
            Identifier::encode(
                Priority::Lowest,
                machbus::net::pgn_defs::PGN_ETP_CM,
                0x90,
                0x80,
            ),
            [
                0x15,
                2,
                1,
                0,
                0,
                (pgn & 0xFF) as u8,
                ((pgn >> 8) & 0xFF) as u8,
                ((pgn >> 16) & 0xFF) as u8,
            ],
            8,
        );
        assert!(etp.process_frame(&cts, 0).is_empty());
        assert_eq!(etp.active_sessions_iter().count(), 1);
        let etp_pending = etp.get_pending_data_frames_fixed::<3>()?;
        assert_eq!(etp_pending.len(), 3);
        let mut etp_cmdt = machbus::net::EtpCmdtTx::new(pgn, &payload, 0x80, 0x90)?;
        let fixed_rts = etp_cmdt.rts();
        assert_eq!(fixed_rts.pgn(), machbus::net::pgn_defs::PGN_ETP_CM);
        assert_eq!(fixed_rts.payload()[0], 0x14);
        etp_cmdt.set_window(1, 2)?;
        let fixed_window = etp_cmdt.pending_data_frames_fixed::<3>()?;
        assert_eq!(fixed_window.len(), 3);
        assert_eq!(fixed_window[0].payload()[0], 0x16);
        assert_eq!(fixed_window[0].payload()[1], 2);
        assert_eq!(fixed_window[1].pgn(), machbus::net::pgn_defs::PGN_ETP_DT);
        assert_eq!(fixed_window[1].payload()[0], 1);
        assert_eq!(fixed_window[2].payload()[0], 2);
        etp_cmdt.set_window(255, 2)?;
        let final_window = etp_cmdt.pending_data_frames_fixed::<3>()?;
        assert_eq!(final_window.len(), 3);
        assert_eq!(final_window[0].payload()[2], 254);
        assert!(etp_cmdt.is_complete());
        let mut etp_tx_for_rx = machbus::net::EtpCmdtTx::new(pgn, &payload, 0x80, 0x90)?;
        let mut etp_rx = machbus::net::EtpRxFixed::<1792>::new();
        let accept = etp_rx.process_frame(&etp_tx_for_rx.rts())?;
        assert!(accept.response.is_some());
        assert!(etp_rx.is_active());
        let mut etp_rx_message = None;
        let mut next_packet = 1u32;
        while etp_rx_message.is_none() {
            let remaining = 256u32 - (next_packet - 1);
            let count = remaining.min(machbus::net::TP_MAX_PACKETS_PER_CTS) as u8;
            etp_tx_for_rx.set_window(next_packet, count)?;
            let window = etp_tx_for_rx.pending_data_frames_fixed::<256>()?;
            for frame in window.iter() {
                let outcome = etp_rx.process_frame(frame)?;
                if outcome.message.is_some() {
                    etp_rx_message = outcome.message;
                }
            }
            next_packet += count as u32;
        }
        let etp_rx_message = etp_rx_message.expect("fixed ETP receiver completes");
        assert_eq!(etp_rx_message.pgn, pgn);
        assert_eq!(etp_rx_message.data.as_slice(), &payload);
        assert!(!etp_rx.is_active());
    }

    let _: &mut dyn Transport<Error = machbus::net::Error> = &mut transport;
    Ok(())
}

#[cfg(feature = "embedded")]
#[test]
fn embedded_fixed_public_surface_imports() -> machbus::net::Result<()> {
    use machbus::fixed::{FixedBytes, FixedMessage};

    let mut q = machbus::fixed::FixedFrameQueue::<2>::new();
    q.push_back((0, request_for_address_claim()))
        .expect("fixed queue has capacity");
    assert_eq!(q.len(), 1);

    let bytes = FixedBytes::<8>::from_slice(request_for_address_claim().payload())
        .expect("request payload fits fixed buffer");
    assert_eq!(bytes.span().get_u16_le(0), 0xEE00);

    let msg = FixedMessage::<8>::from_frame(&request_for_address_claim())
        .expect("single frame payload fits fixed message");
    assert_eq!(msg.size(), 8);
    let frame = msg.to_frame()?;
    assert_eq!(frame.payload(), request_for_address_claim().payload());
    Ok(())
}
