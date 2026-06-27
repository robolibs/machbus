//! ISO 11783-14 Sequence Control Client.
//!
//! Mirrors the C++ `machbus::isobus::sc::SCClient`. Pump-style port,
//! parallel to [`super::master::SCMaster`]:
//!
//! - [`SCClient::try_handle_master_status`] decodes an incoming
//!   `PGN_SC_MASTER_STATUS` `Message` with explicit validation errors.
//!   [`SCClient::handle_master_status`] remains the compatibility wrapper
//!   that ignores malformed or unrelated frames.
//! - [`SCClient::update`] advances pacing and busy timeouts and
//!   returns a `[u8; 8]` payload to emit on `PGN_SC_CLIENT_STATUS`,
//!   or `None`.
//!
//! Mutating methods (`set_busy`, `report_step_complete`) may also
//! return a payload immediately when the min-spacing window allows;
//! otherwise the request is deferred to the next [`SCClient::update`].

use super::types::{
    SC_MSG_CODE_CLIENT, SC_MSG_CODE_MASTER, SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
    SC_STATUS_PAYLOAD_LEN, SCClientConfig, SCClientFuncError, SCClientState, SCMasterState,
    SCSequenceState, SCState, sc_inactive_status_sequence_fields_are_valid,
    sc_master_busy_flags_are_valid, sc_master_state_byte_is_valid, sc_sequence_state_byte_is_valid,
    sc_status_reserved_tail_is_valid, sc_status_sequence_number_is_valid,
    sc_status_sequence_state_is_supported,
};
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::PGN_SC_MASTER_STATUS;
use crate::net::state_machine::StateMachine;

/// ISO 11783-14 Sequence Control Client.
pub struct SCClient {
    config: SCClientConfig,
    state_machine: StateMachine<SCState>,

    busy: bool,
    busy_timer_ms: u32,

    time_since_last_status_ms: u32,
    status_pending: bool,

    current_step_id: u16,

    pub on_state_change: Event<(SCState, SCState)>,
    pub on_sequence_start: Event<()>,
    pub on_step_request: Event<u16>,
    pub on_pause: Event<()>,
    pub on_resume: Event<()>,
    pub on_abort: Event<()>,
}

impl SCClient {
    #[must_use]
    pub fn new(config: SCClientConfig) -> Self {
        Self {
            config,
            state_machine: StateMachine::new(SCState::Idle),
            busy: false,
            busy_timer_ms: 0,
            time_since_last_status_ms: u32::MAX, // first send not throttled
            status_pending: false,
            current_step_id: 0,
            on_state_change: Event::new(),
            on_sequence_start: Event::new(),
            on_step_request: Event::new(),
            on_pause: Event::new(),
            on_resume: Event::new(),
            on_abort: Event::new(),
        }
    }

    // ─── Busy signaling ───────────────────────────────────────────────

    /// Set the busy flag. Returns a status payload immediately if the
    /// min-spacing window allows a send right now; otherwise the
    /// request is deferred to the next [`Self::update`].
    pub fn set_busy(&mut self, busy: bool) -> Option<[u8; 8]> {
        if self.busy == busy {
            return None;
        }
        self.busy = busy;
        self.busy_timer_ms = 0;
        self.request_status_send_inline()
    }

    #[inline]
    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy
    }

    // ─── Step completion ──────────────────────────────────────────────

    /// Acknowledge step completion. Returns a status payload if the
    /// min-spacing window allows, else defers.
    pub fn report_step_complete(&mut self, step_id: u16) -> Result<Option<[u8; 8]>> {
        if !self.state_machine.is(SCState::Active) {
            return Err(Error::invalid_state("not in Active state"));
        }
        if step_id != self.current_step_id {
            return Err(Error::invalid_state("step_id mismatch"));
        }
        Ok(self.request_status_send_inline())
    }

    // ─── Periodic update ──────────────────────────────────────────────

    /// Advance timers. Returns a status payload to emit when a
    /// previously-deferred status is due, otherwise `None`. Drives
    /// the busy-pause timeout.
    pub fn update(&mut self, elapsed_ms: u32) -> Option<[u8; 8]> {
        self.time_since_last_status_ms = self.time_since_last_status_ms.saturating_add(elapsed_ms);

        if self.busy
            && (self.state_machine.is(SCState::Active) || self.state_machine.is(SCState::Paused))
        {
            self.busy_timer_ms = self.busy_timer_ms.saturating_add(elapsed_ms);
            if self.busy_timer_ms >= self.config.busy_pause_timeout_ms {
                self.transition(SCState::Error);
                self.on_abort.emit(&());
                self.status_pending = false;
                return Some(self.encode_client_status_now());
            }
        }

        if self.status_pending
            && self.time_since_last_status_ms >= self.config.min_status_spacing_ms
        {
            self.status_pending = false;
            return Some(self.encode_client_status_now());
        }
        None
    }

    // ─── State access ─────────────────────────────────────────────────

    #[inline]
    #[must_use]
    pub fn state(&self) -> SCState {
        self.state_machine.state()
    }

    #[inline]
    #[must_use]
    pub fn is(&self, s: SCState) -> bool {
        self.state_machine.is(s)
    }

    #[inline]
    #[must_use]
    pub const fn time_since_last_status(&self) -> u32 {
        self.time_since_last_status_ms
    }

    // ─── Inbound handler ──────────────────────────────────────────────

    /// Decode an incoming `PGN_SC_MASTER_STATUS` message. Compatibility
    /// wrapper: malformed or unrelated frames are ignored. Returns a
    /// client status payload if the state change triggered an immediate
    /// send, else `None`. Use [`Self::try_handle_master_status`] when a
    /// caller needs explicit validation errors.
    pub fn handle_master_status(&mut self, msg: &Message) -> Option<[u8; 8]> {
        self.try_handle_master_status(msg).ok().flatten()
    }

    /// Decode an incoming `PGN_SC_MASTER_STATUS` message, returning an
    /// explicit error for malformed or unrelated frames. Returns a client
    /// status payload if the state change triggered an immediate send, else
    /// `None`.
    pub fn try_handle_master_status(&mut self, msg: &Message) -> Result<Option<[u8; 8]>> {
        if msg.pgn != PGN_SC_MASTER_STATUS {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        if !msg.has_usable_source() {
            return Err(Error::invalid_address(msg.source));
        }
        if !msg.has_valid_destination_for_pgn() {
            return Err(Error::invalid_address(msg.destination));
        }
        if msg.data.len() != SC_STATUS_PAYLOAD_LEN {
            return Err(Error::invalid_data(
                "SC master status must be exactly 8 bytes",
            ));
        }
        if msg.get_u8(0) != SC_MSG_CODE_MASTER {
            return Err(Error::invalid_data(
                "SC master status has wrong message code",
            ));
        }
        if !sc_status_reserved_tail_is_valid(&msg.data) {
            return Err(Error::invalid_data(
                "SC master status has non-0xFF reserved tail bytes",
            ));
        }
        let master_state_raw = msg.get_u8(1);
        if !sc_master_state_byte_is_valid(master_state_raw) {
            return Err(Error::invalid_data(
                "SC master status has reserved master state",
            ));
        }
        if matches!(
            SCMasterState::try_from_u8(master_state_raw).expect("validated SC master state byte"),
            SCMasterState::Initialization
        ) {
            return Err(Error::invalid_data(
                "SC master initialization state is not supported",
            ));
        }
        if !sc_master_busy_flags_are_valid(msg.get_u8(4)) {
            return Err(Error::invalid_data(
                "SC master status has reserved busy flags",
            ));
        }
        let master_ms =
            SCMasterState::try_from_u8(master_state_raw).expect("validated SC master state byte");
        let seq_state_raw = msg.get_u8(3);
        if !sc_sequence_state_byte_is_valid(seq_state_raw) {
            return Err(Error::invalid_data(
                "SC master status has reserved sequence state",
            ));
        }
        let seq_state =
            SCSequenceState::try_from_u8(seq_state_raw).expect("validated SC sequence state byte");
        let seq_num = msg.get_u8(2);
        if !matches!(master_ms, SCMasterState::Active)
            && !sc_inactive_status_sequence_fields_are_valid(seq_state, seq_num)
        {
            return Err(Error::invalid_data(
                "SC inactive master status has active sequence fields",
            ));
        }
        if matches!(master_ms, SCMasterState::Active)
            && !sc_status_sequence_state_is_supported(seq_state)
        {
            return Err(Error::invalid_data(
                "SC master status has unsupported sequence state",
            ));
        }
        if !sc_status_sequence_number_is_valid(seq_state, seq_num) {
            return Err(Error::invalid_data(
                "SC master status has invalid sequence number",
            ));
        }

        let master_state = if matches!(master_ms, SCMasterState::Active) {
            match seq_state {
                SCSequenceState::Ready => {
                    if self.state_machine.is(SCState::Active)
                        || self.state_machine.is(SCState::Paused)
                    {
                        SCState::Paused
                    } else {
                        SCState::Ready
                    }
                }
                SCSequenceState::PlayBack => SCState::Active,
                SCSequenceState::Abort => SCState::Error,
                _ => SCState::Ready,
            }
        } else {
            SCState::Idle
        };

        let step_id: u16 = if seq_num == SC_SEQUENCE_NUMBER_NOT_AVAILABLE {
            0
        } else {
            u16::from(seq_num)
        };

        let mut to_send: Option<[u8; 8]> = None;
        match master_state {
            SCState::Ready => {
                if self.state_machine.is(SCState::Idle) {
                    self.transition(SCState::Ready);
                    self.on_sequence_start.emit(&());
                    to_send = self.request_status_send_inline();
                }
            }
            SCState::Active => {
                if self.state_machine.is(SCState::Ready)
                    || self.state_machine.is(SCState::Active)
                    || self.state_machine.is(SCState::Paused)
                {
                    if self.state_machine.is(SCState::Paused) {
                        self.transition(SCState::Active);
                        self.on_resume.emit(&());
                    } else if !self.state_machine.is(SCState::Active) {
                        self.transition(SCState::Active);
                    }
                    self.current_step_id = step_id;
                    self.on_step_request.emit(&step_id);
                    to_send = self.request_status_send_inline();
                }
            }
            SCState::Paused => {
                if self.state_machine.is(SCState::Active) {
                    self.transition(SCState::Paused);
                    self.on_pause.emit(&());
                    to_send = self.request_status_send_inline();
                }
            }
            SCState::Complete => {
                if self.state_machine.is(SCState::Active) || self.state_machine.is(SCState::Paused)
                {
                    self.transition(SCState::Complete);
                    to_send = self.request_status_send_inline();
                }
            }
            SCState::Error => {
                self.transition(SCState::Error);
                self.on_abort.emit(&());
                to_send = self.request_status_send_inline();
            }
            SCState::Idle => {
                if !self.state_machine.is(SCState::Idle) {
                    self.transition(SCState::Idle);
                    self.on_abort.emit(&());
                    self.clear_sequence_session();
                    to_send = self.request_status_send_inline();
                }
            }
        }
        Ok(to_send)
    }

    // ─── Internals ────────────────────────────────────────────────────

    fn transition(&mut self, new_state: SCState) {
        let old = self.state_machine.state();
        if old == new_state {
            return;
        }
        self.state_machine.transition(new_state);
        self.on_state_change.emit(&(old, new_state));
    }

    fn request_status_send_inline(&mut self) -> Option<[u8; 8]> {
        if self.time_since_last_status_ms >= self.config.min_status_spacing_ms {
            Some(self.encode_client_status_now())
        } else {
            self.status_pending = true;
            None
        }
    }

    fn clear_sequence_session(&mut self) {
        self.busy = false;
        self.busy_timer_ms = 0;
        self.status_pending = false;
        self.current_step_id = 0;
    }

    fn iso_client_state(&self) -> SCClientState {
        if self.state_machine.is(SCState::Idle) {
            SCClientState::Disabled
        } else {
            SCClientState::Enabled
        }
    }

    fn iso_sequence_state(&self) -> SCSequenceState {
        match self.state_machine.state() {
            SCState::Ready | SCState::Paused => SCSequenceState::Ready,
            SCState::Active => SCSequenceState::PlayBack,
            SCState::Error => SCSequenceState::Abort,
            _ => SCSequenceState::Reserved,
        }
    }

    fn encode_client_status_now(&mut self) -> [u8; 8] {
        let cs = self.iso_client_state();
        let seq = self.iso_sequence_state();
        let mut data = [0xFFu8; 8];
        data[0] = SC_MSG_CODE_CLIENT;
        data[1] = cs.as_u8();
        data[2] = if matches!(cs, SCClientState::Disabled) || matches!(seq, SCSequenceState::Ready)
        {
            SC_SEQUENCE_NUMBER_NOT_AVAILABLE
        } else {
            (self.current_step_id & 0xFF) as u8
        };
        data[3] = seq.as_u8();
        data[4] = SCClientFuncError::NoErrors.as_u8();
        self.time_since_last_status_ms = 0;
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
    use crate::net::pgn_defs::{PGN_SC_CLIENT_STATUS, PGN_SC_MASTER_STATUS};

    fn master_status(state: SCMasterState, seq: SCSequenceState, seq_num: u8) -> Message {
        let payload = vec![
            SC_MSG_CODE_MASTER,
            state.as_u8(),
            seq_num,
            seq.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        Message::new(PGN_SC_MASTER_STATUS, payload, 0x10)
    }

    #[test]
    fn idle_to_ready_on_master_ready() {
        let mut c = SCClient::new(SCClientConfig::default());
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        assert!(c.is(SCState::Ready));
    }

    #[test]
    fn ready_to_active_on_master_playback() {
        let mut c = SCClient::new(SCClientConfig::default());
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::PlayBack,
            7,
        ));
        assert!(c.is(SCState::Active));
    }

    #[test]
    fn master_status_rejects_short_and_overlong_payloads() {
        let mut c = SCClient::new(SCClientConfig::default());
        let mut short = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
        short.data.pop();
        assert!(c.handle_master_status(&short).is_none());
        assert!(c.is(SCState::Idle));

        let mut overlong = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
        overlong.data.push(0xFF);
        assert!(c.handle_master_status(&overlong).is_none());
        assert!(c.is(SCState::Idle));
    }

    #[test]
    fn try_master_status_reports_validation_errors_without_state_mutation() {
        let mut client = SCClient::new(SCClientConfig::default());

        let mut wrong_pgn = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
        wrong_pgn.pgn = PGN_SC_CLIENT_STATUS;
        let err = client.try_handle_master_status(&wrong_pgn).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidPgn);
        assert!(client.is(SCState::Idle));

        let mut bad_source = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
        bad_source.source = BROADCAST_ADDRESS;
        let err = client.try_handle_master_status(&bad_source).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidAddress);
        assert!(client.is(SCState::Idle));

        let mut short = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
        short.data.pop();
        let err = client.try_handle_master_status(&short).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("exactly 8 bytes"));
        assert!(client.is(SCState::Idle));
    }

    #[test]
    fn master_status_rejects_wrong_pgn_and_invalid_source_addresses() {
        let mut wrong_pgn_client = SCClient::new(SCClientConfig::default());
        let mut wrong_pgn = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
        wrong_pgn.pgn = PGN_SC_CLIENT_STATUS;
        assert!(wrong_pgn_client.handle_master_status(&wrong_pgn).is_none());
        assert!(wrong_pgn_client.is(SCState::Idle));

        for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut client = SCClient::new(SCClientConfig::default());
            let mut status = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
            status.source = bad_source;
            assert!(
                client.handle_master_status(&status).is_none(),
                "SC master status from invalid source 0x{bad_source:02X} must not emit a reply"
            );
            assert!(client.is(SCState::Idle));
        }
    }

    #[test]
    fn master_status_rejects_invalid_sequence_numbers_for_state() {
        let mut ready_client = SCClient::new(SCClientConfig::default());
        assert!(
            ready_client
                .handle_master_status(&master_status(
                    SCMasterState::Active,
                    SCSequenceState::Ready,
                    7,
                ))
                .is_none()
        );
        assert!(ready_client.is(SCState::Idle));

        let mut playback_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
        let _ = playback_client.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        assert!(playback_client.is(SCState::Ready));
        assert!(
            playback_client
                .handle_master_status(&master_status(
                    SCMasterState::Active,
                    SCSequenceState::PlayBack,
                    0xFF,
                ))
                .is_none()
        );
        assert!(playback_client.is(SCState::Ready));
    }

    #[test]
    fn master_status_rejects_unsupported_active_sequence_states() {
        let mut idle_client = SCClient::new(SCClientConfig::default());
        assert!(
            idle_client
                .handle_master_status(&master_status(
                    SCMasterState::Active,
                    SCSequenceState::Recording,
                    7,
                ))
                .is_none()
        );
        assert!(idle_client.is(SCState::Idle));

        let mut ready_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
        let _ = ready_client.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        assert!(ready_client.is(SCState::Ready));
        assert!(
            ready_client
                .handle_master_status(&master_status(
                    SCMasterState::Active,
                    SCSequenceState::RecordingCompletion,
                    7,
                ))
                .is_none()
        );
        assert!(ready_client.is(SCState::Ready));
    }

    #[test]
    fn master_status_rejects_reserved_master_state_bytes() {
        let mut ready_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
        let _ = ready_client.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        assert!(ready_client.is(SCState::Ready));

        let mut malformed_playback =
            master_status(SCMasterState::Active, SCSequenceState::PlayBack, 7);
        malformed_playback.data[1] = 0x7F;
        assert!(
            ready_client
                .handle_master_status(&malformed_playback)
                .is_none()
        );
        assert!(
            ready_client.is(SCState::Ready),
            "reserved master-state bytes must not be coerced to Inactive/Idle"
        );

        let _ = ready_client.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::PlayBack,
            7,
        ));
        assert!(ready_client.is(SCState::Active));
        let mut malformed_idle =
            master_status(SCMasterState::Inactive, SCSequenceState::Ready, 0xFF);
        malformed_idle.data[1] = 0x7F;
        assert!(ready_client.handle_master_status(&malformed_idle).is_none());
        assert!(ready_client.is(SCState::Active));
    }

    #[test]
    fn master_status_rejects_reserved_busy_bits_and_tail_bytes() {
        let mut ready_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));

        let mut bad_busy = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
        bad_busy.data[4] = 0x80;
        assert!(ready_client.handle_master_status(&bad_busy).is_none());
        assert!(ready_client.is(SCState::Idle));

        let mut bad_tail = master_status(SCMasterState::Active, SCSequenceState::Ready, 0xFF);
        bad_tail.data[7] = 0;
        assert!(ready_client.handle_master_status(&bad_tail).is_none());
        assert!(ready_client.is(SCState::Idle));
    }

    #[test]
    fn step_request_event_fires_with_seq_num() {
        let mut c = SCClient::new(SCClientConfig::default());
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));

        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<u16>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_step_request
            .subscribe(move |&id| lc.borrow_mut().push(id));

        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::PlayBack,
            12,
        ));
        assert_eq!(*log.borrow(), vec![12]);
    }

    #[test]
    fn busy_pause_timeout_drives_to_error() {
        let cfg = SCClientConfig::default().with_busy_timeout(100);
        let mut c = SCClient::new(cfg);
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::PlayBack,
            1,
        ));
        assert!(c.is(SCState::Active));
        let _ = c.set_busy(true);
        let abort_status = c.update(150).unwrap();
        assert!(c.is(SCState::Error));
        assert_eq!(abort_status[0], SC_MSG_CODE_CLIENT);
        assert_eq!(abort_status[1], SCClientState::Enabled.as_u8());
        assert_eq!(abort_status[2], 1);
        assert_eq!(abort_status[3], SCSequenceState::Abort.as_u8());
    }

    #[test]
    fn report_step_complete_requires_active() {
        let mut c = SCClient::new(SCClientConfig::default());
        assert!(c.report_step_complete(1).is_err());
    }

    #[test]
    fn min_spacing_defers_status_until_update() {
        let cfg = SCClientConfig::default().with_min_spacing(100);
        let mut c = SCClient::new(cfg);
        // First status sends immediately (init time is u32::MAX).
        let first = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        assert!(first.is_some());
        // Second status is throttled.
        let second = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::PlayBack,
            5,
        ));
        assert!(second.is_none());
        // After the spacing window, update() flushes it.
        assert!(c.update(50).is_none());
        let flushed = c.update(60).unwrap();
        assert_eq!(flushed[0], SC_MSG_CODE_CLIENT);
    }

    #[test]
    fn report_step_complete_step_id_mismatch_errors() {
        let mut c = SCClient::new(SCClientConfig::default());
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::PlayBack,
            5,
        ));
        assert!(c.report_step_complete(99).is_err());
        assert!(c.report_step_complete(5).is_ok());
    }

    #[test]
    fn master_ready_after_playback_pauses_and_playback_resumes() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut c = SCClient::new(SCClientConfig::default().with_min_spacing(0));
        let pauses = Rc::new(RefCell::new(0usize));
        let resumes = Rc::new(RefCell::new(0usize));
        let p = pauses.clone();
        c.on_pause.subscribe(move |&()| *p.borrow_mut() += 1);
        let r = resumes.clone();
        c.on_resume.subscribe(move |&()| *r.borrow_mut() += 1);

        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::Ready,
            0xFF,
        ));
        let _ = c.handle_master_status(&master_status(
            SCMasterState::Active,
            SCSequenceState::PlayBack,
            5,
        ));
        assert!(c.is(SCState::Active));

        let pause_response = c
            .handle_master_status(&master_status(
                SCMasterState::Active,
                SCSequenceState::Ready,
                0xFF,
            ))
            .expect("pause transition emits status");
        assert!(c.is(SCState::Paused));
        assert_eq!(*pauses.borrow(), 1);
        assert_eq!(pause_response[0], SC_MSG_CODE_CLIENT);

        let resume_response = c
            .handle_master_status(&master_status(
                SCMasterState::Active,
                SCSequenceState::PlayBack,
                5,
            ))
            .expect("resume transition emits status");
        assert!(c.is(SCState::Active));
        assert_eq!(*resumes.borrow(), 1);
        assert_eq!(resume_response[0], SC_MSG_CODE_CLIENT);
    }

    #[test]
    fn rejects_wrong_message_code() {
        let mut c = SCClient::new(SCClientConfig::default());
        let bad = Message::new(
            PGN_SC_MASTER_STATUS,
            vec![0x12, 1, 0, 1, 0, 0xFF, 0xFF, 0xFF],
            0x10,
        );
        assert!(c.handle_master_status(&bad).is_none());
        assert!(c.is(SCState::Idle));
    }
}
