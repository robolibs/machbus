use machbus::isobus::sc::{
    SC_MAX_SEQUENCE_STEP_ID, SC_MSG_CODE_CLIENT, SC_MSG_CODE_MASTER, SCClient, SCClientConfig,
    SCClientFuncError, SCClientState, SCMaster, SCMasterConfig, SCMasterState, SCSequenceState,
    SCState, SequenceStep,
};
use machbus::net::ErrorCode;
use machbus::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use machbus::net::message::Message;
use machbus::net::pgn_defs::{PGN_SC_CLIENT_STATUS, PGN_SC_MASTER_STATUS};

const SC_SEQUENCE_NUMBER_NOT_AVAILABLE: u8 = 0xFF;
fn sc_step(step_id: u16) -> SequenceStep {
    SequenceStep {
        step_id,
        description: format!("standard-derived SC step {step_id}"),
        duration_ms: 0,
        completed: false,
    }
}

fn master_status(source: u8, bytes: [u8; 8]) -> Message {
    Message::new(PGN_SC_MASTER_STATUS, bytes.to_vec(), source)
}

fn client_status(source: u8, bytes: [u8; 8]) -> Message {
    Message::new(PGN_SC_CLIENT_STATUS, bytes.to_vec(), source)
}

fn master_ready() -> [u8; 8] {
    [
        SC_MSG_CODE_MASTER,
        SCMasterState::Active.as_u8(),
        SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
        SCSequenceState::Ready.as_u8(),
        0,
        0xFF,
        0xFF,
        0xFF,
    ]
}

fn master_playback(step_id: u8) -> [u8; 8] {
    [
        SC_MSG_CODE_MASTER,
        SCMasterState::Active.as_u8(),
        step_id,
        SCSequenceState::PlayBack.as_u8(),
        0,
        0xFF,
        0xFF,
        0xFF,
    ]
}

fn master_abort(step_id: u8) -> [u8; 8] {
    [
        SC_MSG_CODE_MASTER,
        SCMasterState::Active.as_u8(),
        step_id,
        SCSequenceState::Abort.as_u8(),
        0,
        0xFF,
        0xFF,
        0xFF,
    ]
}

fn client_ready() -> [u8; 8] {
    [
        SC_MSG_CODE_CLIENT,
        SCClientState::Enabled.as_u8(),
        SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
        SCSequenceState::Ready.as_u8(),
        0,
        0xFF,
        0xFF,
        0xFF,
    ]
}

fn client_playback(step_id: u8) -> [u8; 8] {
    [
        SC_MSG_CODE_CLIENT,
        SCClientState::Enabled.as_u8(),
        step_id,
        SCSequenceState::PlayBack.as_u8(),
        0,
        0xFF,
        0xFF,
        0xFF,
    ]
}

fn client_initialization() -> [u8; 8] {
    [
        SC_MSG_CODE_CLIENT,
        SCClientState::Initialization.as_u8(),
        SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
        SCSequenceState::Ready.as_u8(),
        0,
        0xFF,
        0xFF,
        0xFF,
    ]
}

fn master_inactive() -> [u8; 8] {
    // G4 — F.2 byte 4: "FF16 When byte 2 is set to inactive". This vector used
    // Reserved (0x00), which is what the crate transmitted and therefore what
    // it accepted; a conformant inactive SCM was rejected.
    [
        SC_MSG_CODE_MASTER,
        SCMasterState::Inactive.as_u8(),
        SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
        SCSequenceState::NotApplicable.as_u8(),
        0,
        0xFF,
        0xFF,
        0xFF,
    ]
}

fn client_disabled() -> [u8; 8] {
    // G4 — F.3 bytes 4 and 5: "FF16 When byte 2 is set to disabled".
    [
        SC_MSG_CODE_CLIENT,
        SCClientState::Disabled.as_u8(),
        SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
        SCSequenceState::NotApplicable.as_u8(),
        SCClientFuncError::NotApplicable.as_u8(),
        0xFF,
        0xFF,
        0xFF,
    ]
}

#[test]
fn sequence_control_wire_constants_keep_status_roles_separate() {
    assert_ne!(SC_MSG_CODE_MASTER, SC_MSG_CODE_CLIENT);
    assert_eq!(SC_MAX_SEQUENCE_STEP_ID, 0x31);
}

#[test]
fn sequence_control_public_state_enums_do_not_promote_reserved_values() {
    assert_eq!(SCMasterState::from_u8(1), SCMasterState::Active);
    assert_eq!(SCMasterState::from_u8(0xFF), SCMasterState::Inactive);

    assert_eq!(SCClientState::from_u8(1), SCClientState::Enabled);
    assert_eq!(SCClientState::from_u8(0xFF), SCClientState::Disabled);

    assert_eq!(SCSequenceState::from_u8(4), SCSequenceState::PlayBack);
    // G4 — 0xFF is the inactive/disabled sentinel, not a reserved value.
    assert_eq!(
        SCSequenceState::from_u8(0xFF),
        SCSequenceState::NotApplicable
    );
}

#[test]
fn sequence_control_public_status_decoders_reject_noncanonical_bytes() {
    for (raw, state) in [
        (0, SCMasterState::Inactive),
        (1, SCMasterState::Active),
        (2, SCMasterState::Initialization),
    ] {
        assert_eq!(SCMasterState::try_from_u8(raw), Some(state));
        assert_eq!(SCMasterState::from_u8(raw), state);
    }

    for (raw, state) in [
        (0, SCClientState::Disabled),
        (1, SCClientState::Enabled),
        (2, SCClientState::Initialization),
    ] {
        assert_eq!(SCClientState::try_from_u8(raw), Some(state));
        assert_eq!(SCClientState::from_u8(raw), state);
    }

    for (raw, state) in [
        (0, SCSequenceState::Reserved),
        (1, SCSequenceState::Ready),
        (2, SCSequenceState::Recording),
        (3, SCSequenceState::RecordingCompletion),
        (4, SCSequenceState::PlayBack),
        (5, SCSequenceState::Abort),
    ] {
        assert_eq!(SCSequenceState::try_from_u8(raw), Some(state));
        assert_eq!(SCSequenceState::from_u8(raw), state);
    }

    for (raw, error) in [
        (0, SCClientFuncError::NoErrors),
        (1, SCClientFuncError::NoChange),
        (2, SCClientFuncError::Changed),
        (3, SCClientFuncError::NeedsConfirm),
    ] {
        assert_eq!(SCClientFuncError::try_from_u8(raw), Some(error));
        assert_eq!(SCClientFuncError::from_u8(raw), error);
    }

    for reserved in [6, 7, 0x08, 0x10, 0x40, 0xFE] {
        assert_eq!(SCSequenceState::try_from_u8(reserved), None);
        assert_eq!(
            SCSequenceState::from_u8(reserved),
            SCSequenceState::Reserved,
            "legacy lossy decoder may only be used after strict wire validation"
        );
    }

    for reserved in [3, 4, 5, 0x08, 0x10, 0x40, 0xFF] {
        assert_eq!(SCMasterState::try_from_u8(reserved), None);
        assert_eq!(SCClientState::try_from_u8(reserved), None);
        assert_eq!(
            SCMasterState::from_u8(reserved),
            SCMasterState::Inactive,
            "legacy lossy decoder may only be used after strict wire validation"
        );
        assert_eq!(
            SCClientState::from_u8(reserved),
            SCClientState::Disabled,
            "legacy lossy decoder may only be used after strict wire validation"
        );
    }

    // 0xFF is excluded: F.3 byte 5 assigns it to the disabled state.
    for reserved in [4, 5, 6, 0x08, 0x10, 0x40, 0xFE] {
        assert_eq!(SCClientFuncError::try_from_u8(reserved), None);
        assert_eq!(
            SCClientFuncError::from_u8(reserved),
            SCClientFuncError::NoErrors,
            "legacy lossy decoder may only be used after strict wire validation"
        );
    }
}

#[test]
fn sequence_control_master_client_lifecycle_preserves_standard_status_bytes() {
    let master_cfg = SCMasterConfig::default()
        .with_status_interval(100)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(1_000_000);
    let mut master = SCMaster::new(master_cfg);
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();

    assert_eq!(master.update(100).unwrap(), master_ready());

    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    let ready_reply = client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap()
        .expect("Ready status must elicit a client status when spacing allows");
    assert_eq!(ready_reply, client_ready());
    assert!(client.is(SCState::Ready));

    master
        .try_handle_client_status(&client_status(0x20, ready_reply))
        .unwrap();
    assert!(master.is(SCState::Active));
    assert_eq!(master.update(100).unwrap(), master_playback(7));

    let active_reply = client
        .try_handle_master_status(&master_status(0x10, master_playback(7)))
        .unwrap()
        .expect("PlayBack status must elicit a client acknowledgement");
    assert_eq!(active_reply, client_playback(7));
    assert!(client.is(SCState::Active));

    master
        .try_handle_client_status(&client_status(0x20, active_reply))
        .unwrap();
    master.step_completed(7).unwrap();
    assert!(master.is(SCState::Complete));
    assert!(
        master.update(100).is_none(),
        "completed sequences must not keep emitting active status frames"
    );
}

#[test]
fn sequence_control_pause_resume_and_abort_are_protocol_visible() {
    let master_cfg = SCMasterConfig::default()
        .with_status_interval(100)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(1_000_000);
    let mut master = SCMaster::new(master_cfg);
    master.add_step(sc_step(8)).unwrap();
    master.add_step(sc_step(9)).unwrap();
    master.start().unwrap();
    assert_eq!(master.update(100).unwrap(), master_ready());

    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    let ready_reply = client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap()
        .unwrap();
    master
        .try_handle_client_status(&client_status(0x20, ready_reply))
        .unwrap();
    assert_eq!(master.update(100).unwrap(), master_playback(8));
    let active_reply = client
        .try_handle_master_status(&master_status(0x10, master_playback(8)))
        .unwrap()
        .unwrap();
    assert_eq!(active_reply, client_playback(8));
    master
        .try_handle_client_status(&client_status(0x20, active_reply))
        .unwrap();

    master.pause().unwrap();
    assert_eq!(
        master.update(100).unwrap(),
        master_ready(),
        "pause is represented as the ready/not-applicable sequence status"
    );
    let pause_reply = client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap()
        .unwrap();
    assert_eq!(pause_reply, client_ready());
    assert!(client.is(SCState::Paused));

    master.resume().unwrap();
    assert_eq!(master.update(100).unwrap(), master_playback(8));
    let resume_reply = client
        .try_handle_master_status(&master_status(0x10, master_playback(8)))
        .unwrap()
        .unwrap();
    assert_eq!(resume_reply, client_playback(8));
    assert!(client.is(SCState::Active));

    master.abort().unwrap();
    assert_eq!(master.update(0).unwrap(), master_abort(8));
    let abort_reply = client
        .try_handle_master_status(&master_status(0x10, master_abort(8)))
        .unwrap()
        .unwrap();
    assert_eq!(
        abort_reply,
        [
            SC_MSG_CODE_CLIENT,
            SCClientState::Enabled.as_u8(),
            8,
            SCSequenceState::Abort.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ]
    );
    assert!(client.is(SCState::Error));
}

#[test]
fn sequence_control_client_busy_spacing_and_timeout_are_protocol_visible() {
    let mut client = SCClient::new(
        SCClientConfig::default()
            .with_min_spacing(100)
            .with_busy_timeout(250),
    );

    let ready_reply = client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap()
        .expect("first Ready reply is not throttled");
    assert_eq!(ready_reply, client_ready());
    assert!(client.is(SCState::Ready));

    assert!(
        client
            .try_handle_master_status(&master_status(0x10, master_playback(7)))
            .unwrap()
            .is_none(),
        "active acknowledgement is deferred by the configured minimum spacing"
    );
    assert!(client.is(SCState::Active));
    assert!(client.update(99).is_none());
    assert_eq!(
        client.update(1).unwrap(),
        client_playback(7),
        "deferred active acknowledgement is emitted once the spacing boundary is reached"
    );

    assert!(
        client.set_busy(true).is_none(),
        "busy transition is also paced by the status minimum spacing"
    );
    assert!(client.is_busy());
    assert!(client.update(99).is_none());
    assert_eq!(
        client.update(1).unwrap(),
        client_playback(7),
        "the client must not invent non-standard busy bits in the function-error byte"
    );
    assert!(client.is(SCState::Active));

    assert!(
        client.update(149).is_none(),
        "busy-pause timeout must not fire before the configured boundary"
    );
    assert!(client.is(SCState::Active));
    assert_eq!(
        client.update(1).unwrap(),
        [
            SC_MSG_CODE_CLIENT,
            SCClientState::Enabled.as_u8(),
            7,
            SCSequenceState::Abort.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        "busy-pause timeout must be visible as an abort status for the active step"
    );
    assert!(client.is(SCState::Error));
}

/// B8 — ISO 11783-14 F.3: the SCClientStatus is "sent immediately on state
/// change of any of the bytes 2 to 5, once per second during the 'Ready' state
/// or if SCC is disabled, and 5 messages per second during the active
/// 'Recording', 'Recording Completion', 'Play Back' or 'Abort'" states.
///
/// The client used to emit only on change, so a conformant SCM applying its
/// 600 ms F.3 timeout declared the client dead as soon as nothing changed —
/// which is most of a play back. `SCMaster` has always run this cadence.
/// G1 — ISO 11783-14 F.3: "A timeout of 600 ms for the SCClientStatus message
/// shall be applied in the 'Recording', 'Play Back' or 'Abort' state."
///
/// The master ran that timer only until `client_ack_received` latched, so once
/// the required clients had acknowledged a step it stopped watching them: a
/// client that then lost power, dropped off the bus, or hung left the system in
/// Play Back with its function still commanded (§4.5.2, §3.10).
#[test]
fn sequence_control_master_halts_when_an_acknowledged_client_goes_silent() {
    let mut master = SCMaster::new(SCMasterConfig::default());
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();

    master
        .try_handle_client_status(&client_status(0x20, client_ready()))
        .unwrap();
    master
        .try_handle_client_status(&client_status(0x20, client_playback(7)))
        .unwrap();
    assert!(master.is(SCState::Active), "the step is acknowledged");

    // While the client keeps reporting, Play Back continues.
    for _ in 0..5 {
        master.update(500);
        master
            .try_handle_client_status(&client_status(0x20, client_playback(7)))
            .unwrap();
        assert!(master.is(SCState::Active));
    }

    // The moment it stops, the sequence halts.
    let halted = master
        .update(600)
        .expect("a silent client must produce an abort status");
    assert!(master.is(SCState::Error));
    assert_eq!(halted[3], SCSequenceState::Abort.as_u8());
}

/// G2 — ISO 11783-14 F.2: "A timeout of 600 ms for the SCMasterStatus message
/// shall be applied in the 'Recording', 'Recording Completion', 'Play Back' or
/// 'Abort' state. A timeout of 3 s shall be applied in the 'Ready' state."
///
/// The client had no inbound timer at all — `time_since_last_status_ms`
/// measures its own transmit spacing — so a master that stopped transmitting
/// left it Active with `current_step_id` set, still commanding its function.
#[test]
fn sequence_control_client_halts_when_the_master_goes_silent() {
    let mut client = SCClient::new(SCClientConfig::default());
    client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap();
    client
        .try_handle_master_status(&master_status(0x10, master_playback(7)))
        .unwrap();
    assert!(client.is(SCState::Active));

    // A live master keeps it there.
    for _ in 0..5 {
        client.update(500);
        client
            .try_handle_master_status(&master_status(0x10, master_playback(7)))
            .unwrap();
        assert!(client.is(SCState::Active));
    }

    // Silence past the F.2 window drops it out of Play Back.
    client.update(600);
    assert!(
        client.is(SCState::Error),
        "a silent SCM must not leave the client driving the last commanded step"
    );
}

#[test]
fn sequence_control_client_emits_the_periodic_status_cadence() {
    let mut client = SCClient::new(SCClientConfig::default());
    // A fresh client is deliberately un-throttled so it announces itself at
    // once; take that first status out of the way.
    assert!(client.update(0).is_some(), "a new client announces itself");

    // Ready / disabled: once per second.
    assert!(
        client.update(999).is_none(),
        "no status is due before the 1 s idle cadence"
    );
    assert!(
        client.update(1).is_some(),
        "an idle client must still announce itself once per second"
    );

    // Drive it into Play Back and check the 5 Hz cadence.
    client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap();
    client
        .try_handle_master_status(&master_status(0x10, master_playback(7)))
        .unwrap();
    assert!(client.is(SCState::Active));
    // Drain whatever the transitions themselves queued.
    while client.update(100).is_some() {}
    // Re-arm the F.2 reception watchdog: the drain above advanced time too.
    client
        .try_handle_master_status(&master_status(0x10, master_playback(7)))
        .unwrap();

    // F.3 gives 5 messages per second in Play Back, i.e. one per 200 ms. Tick
    // inside the 600 ms F.2 master-status window so the G2 reception watchdog
    // is not what ends the run.
    let mut emitted = 0;
    for _ in 0..5 {
        if client.update(100).is_some() {
            emitted += 1;
        }
    }
    // Two or three depending on where the drain above left the cadence phase;
    // what matters is that it keeps transmitting without any state change.
    assert!(
        (2..=3).contains(&emitted),
        "Play Back is paced at 200 ms, not one message per state change: {emitted}"
    );
    assert!(client.is(SCState::Active));

    // G2 — and once the master stops talking, the client leaves Play Back
    // rather than continuing to drive the last commanded step (F.2, §3.10).
    client.update(600);
    assert!(
        client.is(SCState::Error),
        "a silent SCM must not leave the client commanding its function"
    );
}

#[test]
fn sequence_control_client_inactive_master_status_resets_local_session_state() {
    let mut client = SCClient::new(
        SCClientConfig::default()
            .with_min_spacing(0)
            .with_busy_timeout(250),
    );

    let ready_reply = client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap()
        .expect("Ready must be acknowledged");
    assert_eq!(ready_reply, client_ready());
    let active_reply = client
        .try_handle_master_status(&master_status(0x10, master_playback(7)))
        .unwrap()
        .expect("PlayBack must be acknowledged");
    assert_eq!(active_reply, client_playback(7));
    assert!(client.is(SCState::Active));

    assert_eq!(
        client.set_busy(true).unwrap(),
        client_playback(7),
        "local busy changes remain represented by a normal client status"
    );
    assert!(client.is_busy());
    // B8 — the client now emits the F.3 cadence (5/s while in Play Back), so
    // this tick produces a *status*, not an abort. What must not have happened
    // is the busy timeout firing.
    assert_eq!(
        client.update(200),
        Some(client_playback(7)),
        "the periodic status must keep flowing and the busy timeout must not fire"
    );
    assert!(client.is(SCState::Active));

    let disabled_reply = client
        .try_handle_master_status(&master_status(0x10, master_inactive()))
        .unwrap()
        .expect("inactive master status must be acknowledged when spacing allows");
    assert_eq!(
        disabled_reply,
        client_disabled(),
        "an inactive master must make the client report disabled with no active step"
    );
    assert!(client.is(SCState::Idle));
    assert!(
        !client.is_busy(),
        "leaving the active sequence must clear local busy state before the next sequence"
    );

    let next_ready_reply = client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap()
        .expect("a new Ready sequence must be accepted after the inactive reset");
    assert_eq!(next_ready_reply, client_ready());
    let next_active_reply = client
        .try_handle_master_status(&master_status(0x10, master_playback(8)))
        .unwrap()
        .expect("the next PlayBack step must use the new sequence number");
    assert_eq!(next_active_reply, client_playback(8));
    assert!(client.is(SCState::Active));
    assert!(
        client.update(50).is_none(),
        "busy timing from the previous sequence must not leak into the new active step"
    );
    assert!(client.is(SCState::Active));
}

#[test]
fn sequence_control_master_busy_flags_are_low_bit_status_only() {
    let master_cfg = SCMasterConfig::default()
        .with_status_interval(100)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(1_000_000);
    let mut master = SCMaster::new(master_cfg);
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    master.set_busy_nv_memory(true);
    master.set_busy_parsing_scd(true);

    let ready_with_busy = master.update(100).unwrap();
    assert_eq!(ready_with_busy[0], SC_MSG_CODE_MASTER);
    assert_eq!(ready_with_busy[2], SC_SEQUENCE_NUMBER_NOT_AVAILABLE);
    assert_eq!(ready_with_busy[3], SCSequenceState::Ready.as_u8());
    assert_eq!(
        ready_with_busy[4] & !0x03,
        0,
        "master status busy information is bounded to the defined low bits"
    );
    assert_eq!(
        ready_with_busy[4] & 0x03,
        0x03,
        "both defined master busy conditions must be protocol-visible"
    );
    assert_eq!(&ready_with_busy[5..], &[0xFF, 0xFF, 0xFF]);

    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    let ready_reply = client
        .try_handle_master_status(&master_status(0x10, ready_with_busy))
        .unwrap()
        .expect("defined master busy bits must not block Ready acknowledgement");
    assert_eq!(ready_reply, client_ready());
    assert!(client.is(SCState::Ready));

    master
        .try_handle_client_status(&client_status(0x20, ready_reply))
        .unwrap();
    assert!(master.is(SCState::Active));
    let playback_with_busy = master.update(100).unwrap();
    assert_eq!(playback_with_busy[2], 7);
    assert_eq!(playback_with_busy[3], SCSequenceState::PlayBack.as_u8());
    assert_eq!(playback_with_busy[4] & 0x03, 0x03);
    let active_reply = client
        .try_handle_master_status(&master_status(0x10, playback_with_busy))
        .unwrap()
        .expect("defined master busy bits must not block PlayBack acknowledgement");
    assert_eq!(active_reply, client_playback(7));
    assert!(client.is(SCState::Active));

    master.set_busy_nv_memory(false);
    master.set_busy_parsing_scd(false);
    let playback_without_busy = master.update(100).unwrap();
    assert_eq!(
        playback_without_busy[4], 0,
        "clearing the local busy conditions must clear the wire status bits"
    );
}

#[test]
fn sequence_control_rejects_malformed_statuses_without_state_mutation() {
    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    for bytes in [
        [
            0x12,
            SCMasterState::Active.as_u8(),
            SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_MASTER,
            SCMasterState::Active.as_u8(),
            7,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_MASTER,
            SCMasterState::Active.as_u8(),
            7,
            SCSequenceState::Recording.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_MASTER,
            SCMasterState::Active.as_u8(),
            7,
            SCSequenceState::RecordingCompletion.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_MASTER,
            0x7F,
            7,
            SCSequenceState::PlayBack.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_MASTER,
            SCMasterState::Active.as_u8(),
            SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
            SCSequenceState::Ready.as_u8(),
            0x80,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_MASTER,
            SCMasterState::Active.as_u8(),
            SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0,
        ],
    ] {
        assert!(
            client
                .try_handle_master_status(&master_status(0x10, bytes))
                .is_err()
        );
        assert!(client.is(SCState::Idle));
    }

    let mut master = SCMaster::new(
        SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(1_000_000),
    );
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    for bytes in [
        [
            0x12,
            SCClientState::Enabled.as_u8(),
            SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_CLIENT,
            SCClientState::Enabled.as_u8(),
            7,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_CLIENT,
            SCClientState::Enabled.as_u8(),
            7,
            SCSequenceState::Recording.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_CLIENT,
            SCClientState::Enabled.as_u8(),
            7,
            SCSequenceState::RecordingCompletion.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_CLIENT,
            0x7F,
            SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_CLIENT,
            SCClientState::Enabled.as_u8(),
            SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
            SCSequenceState::Ready.as_u8(),
            4,
            0xFF,
            0xFF,
            0xFF,
        ],
        [
            SC_MSG_CODE_CLIENT,
            SCClientState::Enabled.as_u8(),
            SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0,
            0xFF,
        ],
    ] {
        assert!(
            master
                .try_handle_client_status(&client_status(0x20, bytes))
                .is_err()
        );
        assert!(master.is(SCState::Ready));
    }
}

#[test]
fn sequence_control_inactive_statuses_reject_active_sequence_fields_without_mutation() {
    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap();
    client
        .try_handle_master_status(&master_status(0x10, master_playback(7)))
        .unwrap();
    assert!(client.is(SCState::Active));

    for bytes in [
        {
            let mut bytes = master_inactive();
            bytes[2] = 7;
            bytes
        },
        {
            let mut bytes = master_inactive();
            bytes[3] = SCSequenceState::Ready.as_u8();
            bytes
        },
        {
            let mut bytes = master_inactive();
            bytes[2] = 7;
            bytes[3] = SCSequenceState::PlayBack.as_u8();
            bytes
        },
        {
            let mut bytes = master_inactive();
            bytes[2] = 7;
            bytes[3] = SCSequenceState::Abort.as_u8();
            bytes
        },
    ] {
        let err = client
            .try_handle_master_status(&master_status(0x10, bytes))
            .expect_err("inactive SC master status must not carry active sequence fields");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(
            client.is(SCState::Active),
            "malformed inactive master status must not abort or idle an active client"
        );
    }

    let mut master = SCMaster::new(
        SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(1_000_000),
    );
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    master
        .try_handle_client_status(&client_status(0x20, client_ready()))
        .unwrap();
    assert!(master.is(SCState::Active));

    for bytes in [
        {
            let mut bytes = client_disabled();
            bytes[2] = 7;
            bytes
        },
        {
            let mut bytes = client_disabled();
            bytes[3] = SCSequenceState::Ready.as_u8();
            bytes
        },
        {
            let mut bytes = client_disabled();
            bytes[2] = 7;
            bytes[3] = SCSequenceState::PlayBack.as_u8();
            bytes
        },
        {
            let mut bytes = client_disabled();
            bytes[2] = 7;
            bytes[3] = SCSequenceState::Abort.as_u8();
            bytes
        },
    ] {
        let err = master
            .try_handle_client_status(&client_status(0x20, bytes))
            .expect_err("disabled SC client status must not carry active sequence fields");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(
            master.is(SCState::Active),
            "malformed disabled client status must not change active master state"
        );
    }
}

#[test]
fn sequence_control_rejects_reserved_raw_sequence_state_bytes_without_mutation() {
    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap();
    client
        .try_handle_master_status(&master_status(0x10, master_playback(7)))
        .unwrap();
    assert!(client.is(SCState::Active));

    for reserved_raw_sequence_state in [6, 0x7F, 0xFE] {
        let mut bytes = master_inactive();
        bytes[3] = reserved_raw_sequence_state;
        let err = client
            .try_handle_master_status(&master_status(0x10, bytes))
            .expect_err("inactive SC master status must reject reserved raw sequence-state bytes");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(
            client.is(SCState::Active),
            "reserved raw sequence states must not be coerced to inactive/reserved"
        );
    }

    // The one value F.2 does assign to an inactive master must be accepted.
    client
        .try_handle_master_status(&master_status(0x10, master_inactive()))
        .expect("F.2 byte 4 sentinel FF16 is the conformant inactive encoding");

    let mut master = SCMaster::new(
        SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(1_000_000),
    );
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    master
        .try_handle_client_status(&client_status(0x20, client_ready()))
        .unwrap();
    assert!(master.is(SCState::Active));

    for reserved_raw_sequence_state in [6, 0x7F, 0xFE] {
        let mut bytes = client_disabled();
        bytes[3] = reserved_raw_sequence_state;
        let err = master
            .try_handle_client_status(&client_status(0x20, bytes))
            .expect_err("disabled SC client status must reject reserved raw sequence-state bytes");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(
            master.is(SCState::Active),
            "reserved raw sequence states must not be coerced to disabled/reserved"
        );
    }

    master
        .try_handle_client_status(&client_status(0x20, client_disabled()))
        .expect("F.3 byte 4 sentinel FF16 is the conformant disabled encoding");
}

#[test]
fn sequence_control_rejects_initialization_states_as_unsupported_without_mutation() {
    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap();
    assert!(client.is(SCState::Ready));
    client
        .try_handle_master_status(&master_status(0x10, master_playback(7)))
        .unwrap();
    assert!(client.is(SCState::Active));

    let mut master_initializing = master_ready();
    master_initializing[1] = SCMasterState::Initialization.as_u8();
    assert!(
        client
            .try_handle_master_status(&master_status(0x10, master_initializing))
            .is_err(),
        "the Rust SC client does not implement master-initialization semantics yet"
    );
    assert!(
        client.is(SCState::Active),
        "unsupported initialization must not be coerced into idle/abort"
    );

    let mut master = SCMaster::new(
        SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(1_000_000),
    );
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    assert!(
        master
            .try_handle_client_status(&client_status(0x20, client_initialization()))
            .is_err(),
        "the Rust SC master does not implement client-initialization semantics yet"
    );
    assert!(
        master.is(SCState::Ready),
        "unsupported client initialization must not satisfy Ready acknowledgement"
    );
}

#[test]
fn sequence_control_rejects_wrong_pgn_and_invalid_sources_before_state_update() {
    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert!(
            client
                .try_handle_master_status(&master_status(bad_source, master_ready()))
                .is_err()
        );
        assert!(client.is(SCState::Idle));
    }
    let mut wrong_master_pgn = master_status(0x10, master_ready());
    wrong_master_pgn.pgn = PGN_SC_CLIENT_STATUS;
    assert!(client.try_handle_master_status(&wrong_master_pgn).is_err());
    assert!(client.is(SCState::Idle));

    let mut master = SCMaster::new(SCMasterConfig::default());
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert!(
            master
                .try_handle_client_status(&client_status(bad_source, client_ready()))
                .is_err()
        );
        assert!(master.is(SCState::Ready));
    }
    let mut wrong_client_pgn = client_status(0x20, client_ready());
    wrong_client_pgn.pgn = PGN_SC_MASTER_STATUS;
    assert!(master.try_handle_client_status(&wrong_client_pgn).is_err());
    assert!(master.is(SCState::Ready));
}

#[test]
fn sequence_control_two_client_acknowledgement_gates_ready_and_active_progress() {
    let master_cfg = SCMasterConfig::default()
        .with_required_client_count(2)
        .with_status_interval(100)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(250);
    let mut master = SCMaster::new(master_cfg);
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    assert_eq!(master.update(100).unwrap(), master_ready());

    master
        .try_handle_client_status(&client_status(0x20, client_ready()))
        .unwrap();
    assert!(
        master.is(SCState::Ready),
        "one Ready acknowledgement is not enough for a two-client sequence"
    );
    master
        .try_handle_client_status(&client_status(0x21, client_ready()))
        .unwrap();
    assert!(master.is(SCState::Active));
    assert_eq!(master.update(100).unwrap(), master_playback(7));

    master
        .try_handle_client_status(&client_status(0x20, client_playback(7)))
        .unwrap();
    assert!(master.is(SCState::Active));
    assert_eq!(
        master.update(300).unwrap(),
        master_abort(7),
        "only one active-step acknowledgement must time out a two-client sequence"
    );
    assert!(master.is(SCState::Error));
}

#[test]
fn sequence_control_master_disabled_client_status_reopens_active_ack_timeout() {
    let master_cfg = SCMasterConfig::default()
        .with_status_interval(1_000_000)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(250);
    let mut master = SCMaster::new(master_cfg);
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();

    master
        .try_handle_client_status(&client_status(0x20, client_ready()))
        .unwrap();
    assert!(master.is(SCState::Active));
    master
        .try_handle_client_status(&client_status(0x20, client_playback(7)))
        .unwrap();
    // G1 — the acknowledgement disarms the *missing-ack* timeout, but F.3's
    // 600 ms client-status timeout runs for the whole of Play Back. This used
    // to advance a full second and assert the master stayed Active, which is
    // the defect: a client that goes silent after acking never halts it.
    assert!(
        master.update(100).is_none(),
        "an active acknowledgement suppresses the missing-ack timeout"
    );
    assert!(master.is(SCState::Active));

    master
        .try_handle_client_status(&client_status(0x20, client_disabled()))
        .unwrap();
    assert!(
        master.update(249).is_none(),
        "disabled-client acknowledgement revocation must restart the timeout window"
    );
    assert!(master.is(SCState::Active));
    assert_eq!(
        master.update(1).unwrap(),
        master_abort(7),
        "a disabled client must not leave the active step permanently acknowledged"
    );
    assert!(master.is(SCState::Error));
}

#[test]
fn sequence_control_ready_and_active_timeouts_emit_protocol_visible_abort_status() {
    let ready_timeout_cfg = SCMasterConfig::default()
        .with_status_interval(1_000)
        .with_ready_timeout(250)
        .with_active_timeout(1_000_000);
    let mut ready_timeout_master = SCMaster::new(ready_timeout_cfg);
    ready_timeout_master.add_step(sc_step(7)).unwrap();
    ready_timeout_master.start().unwrap();
    assert!(
        ready_timeout_master.update(249).is_none(),
        "Ready timeout must not fire before the configured boundary"
    );
    assert!(ready_timeout_master.is(SCState::Ready));
    assert_eq!(
        ready_timeout_master.update(1).unwrap(),
        master_abort(7),
        "Ready timeout must be visible as an abort status for the selected step"
    );
    assert!(ready_timeout_master.is(SCState::Error));

    let active_timeout_cfg = SCMasterConfig::default()
        .with_status_interval(1_000)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(250);
    let mut active_timeout_master = SCMaster::new(active_timeout_cfg);
    active_timeout_master.add_step(sc_step(8)).unwrap();
    active_timeout_master.start().unwrap();
    active_timeout_master
        .try_handle_client_status(&client_status(0x20, client_ready()))
        .unwrap();
    assert!(active_timeout_master.is(SCState::Active));
    assert!(
        active_timeout_master.update(249).is_none(),
        "Active timeout must not fire before the configured boundary"
    );
    assert!(active_timeout_master.is(SCState::Active));
    assert_eq!(
        active_timeout_master.update(1).unwrap(),
        master_abort(8),
        "missing active acknowledgement must produce a protocol-visible abort"
    );
    assert!(active_timeout_master.is(SCState::Error));
}

#[test]
fn sequence_control_step_id_boundaries_are_enforced_before_mutation() {
    let mut master = SCMaster::new(SCMasterConfig::default());
    master.add_step(sc_step(0)).unwrap();
    master.add_step(sc_step(SC_MAX_SEQUENCE_STEP_ID)).unwrap();
    assert_eq!(master.steps().len(), 2);

    assert!(
        master
            .add_step(sc_step(SC_MAX_SEQUENCE_STEP_ID + 1))
            .is_err(),
        "the reserved wire step value must not be accepted as a configured step"
    );
    assert!(
        master.add_step(sc_step(0)).is_err(),
        "duplicate step ids would make acknowledgements ambiguous"
    );
    assert_eq!(master.steps().len(), 2);

    master.start().unwrap();
    assert!(
        master.add_step(sc_step(9)).is_err(),
        "sequence definitions must be immutable after leaving Idle"
    );
    assert_eq!(master.steps().len(), 2);
}

#[test]
fn sequence_control_rejects_reserved_sequence_numbers_without_state_progress() {
    let reserved_sequence = (SC_MAX_SEQUENCE_STEP_ID + 1) as u8;

    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    client
        .try_handle_master_status(&master_status(0x10, master_ready()))
        .unwrap();
    assert!(client.is(SCState::Ready));
    let reserved_playback = master_playback(reserved_sequence);
    assert!(
        client
            .try_handle_master_status(&master_status(0x10, reserved_playback))
            .is_err(),
        "reserved SC master sequence numbers must not be accepted as step ids"
    );
    assert!(
        client.is(SCState::Ready),
        "reserved master sequence numbers must not move the client into playback"
    );

    let mut master = SCMaster::new(
        SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(1_000_000),
    );
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    master
        .try_handle_client_status(&client_status(0x20, client_ready()))
        .unwrap();
    assert!(master.is(SCState::Active));
    let reserved_client_playback = client_playback(reserved_sequence);
    assert!(
        master
            .try_handle_client_status(&client_status(0x20, reserved_client_playback))
            .is_err(),
        "reserved SC client sequence numbers must not acknowledge playback"
    );
    assert!(
        master.is(SCState::Active),
        "reserved client sequence numbers must not force an error or complete acknowledgement"
    );

    master
        .try_handle_client_status(&client_status(0x20, client_playback(7)))
        .unwrap();
    assert!(master.is(SCState::Active));
    let _ = master.update(300);
    assert!(
        master.is(SCState::Active),
        "valid acknowledgement after the reserved-id rejection should still satisfy active progress"
    );
}

/// 6G — ISO 11783-14 B.1: both SC PGNs default to **priority 4**, and B.3
/// defines SCC → SCM as PDU1 (PF 141), i.e. destination-specific. The client
/// plugin broadcast its status at priority 6, so a conformant SCM expecting an
/// addressed reply never saw one.
#[test]
fn sequence_control_client_status_is_addressed_to_the_master_at_priority_four() {
    use machbus::net::{BROADCAST_ADDRESS, Frame, Identifier, Name, Priority};
    use machbus::session::{Session, plugins::ScClient};
    use machbus::time::Instant;

    let name = Name::default()
        .with_identity_number(0x5C)
        .with_function_code(0x80)
        .with_self_configurable(true);
    let mut session = Session::builder(name, 0x80)
        .plug(ScClient::new(Default::default()))
        .build()
        .unwrap();
    session.start().unwrap();

    let mut now = Instant::ZERO;
    for _ in 0..40 {
        now = now.add_millis(100);
        session.tick(now);
        while session.poll_transmit().is_some() {}
        if session.is_claimed() {
            break;
        }
    }
    assert!(session.is_claimed());

    // An SCM at 0x10 announces itself.
    let scm = Frame::new(
        Identifier::encode(
            Priority::BelowNormal,
            PGN_SC_MASTER_STATUS,
            0x10,
            BROADCAST_ADDRESS,
        ),
        master_ready(),
        8,
    );
    session.feed(0, &scm, now.add_millis(10));
    session.tick(now.add_millis(20));

    let mut seen = false;
    while let Some((_, frame)) = session.poll_transmit() {
        if frame.id.pgn() == PGN_SC_CLIENT_STATUS {
            assert_eq!(
                frame.id.priority(),
                Priority::BelowNormal,
                "SC messages default to priority 4 (B.1)"
            );
            assert_eq!(
                frame.destination(),
                0x10,
                "SCC to SCM is destination-specific (B.3), not broadcast"
            );
            seen = true;
        }
    }
    assert!(seen, "the client answered the SCM's status");
}
