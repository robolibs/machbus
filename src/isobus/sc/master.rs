//! ISO 11783-14 Sequence Control Master.
//!
//! Mirrors the C++ `machbus::isobus::sc::SCMaster`. The C++ class
//! couples to `IsoNet&` and sends/receives directly; the Rust port is
//! pump-style:
//!
//! - [`SCMaster::try_handle_client_status`] decodes an incoming
//!   `PGN_SC_CLIENT_STATUS` `Message` with explicit validation errors.
//!   [`SCMaster::handle_client_status`] remains the compatibility wrapper
//!   that ignores malformed or unrelated frames.
//! - [`SCMaster::update`] advances pacing/timeouts and returns the
//!   8-byte payload to emit on `PGN_SC_MASTER_STATUS`, or `None`.
//!
//! Users dispatch I/O themselves via `IsoNet::register_pgn_callback`
//! / `IsoNet::send`.
//!
//! State mapping to the ISO wire: see private helpers
//! `iso_master_state` / `iso_sequence_state`.

use alloc::{
    collections::{BTreeMap, BTreeSet as HashSet},
    format,
    vec::Vec,
};

use super::types::{
    SC_MAX_SEQUENCE_STEP_ID, SC_MSG_CODE_CLIENT, SC_MSG_CODE_MASTER,
    SC_SEQUENCE_NUMBER_NOT_AVAILABLE, SC_STATUS_IDLE_RATE_MS, SC_STATUS_PAYLOAD_LEN,
    SCMasterConfig, SCMasterState, SCSequenceState, SCState, SequenceStep,
    sc_client_func_error_byte_is_valid, sc_client_state_byte_is_valid,
    sc_inactive_status_sequence_fields_are_valid, sc_sequence_state_byte_is_valid,
    sc_status_payload_len_is_canonical, sc_status_sequence_number_is_valid,
    sc_status_sequence_state_is_supported,
};
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::PGN_SC_CLIENT_STATUS;
use crate::net::state_machine::StateMachine;

/// How long ago an enrolled client last reported, and what it reported.
///
/// The sequence state matters because F.3 gives a client two cadences — 1 Hz in
/// Ready or when disabled, 5 Hz in the active states — and the master's timeout
/// has to match the cadence the *client* is using, not the one the master is in.
#[derive(Debug, Clone, Copy)]
struct ClientLiveness {
    age_ms: u32,
    reported: SCSequenceState,
}

/// ISO 11783-14 Sequence Control Master.
pub struct SCMaster {
    config: SCMasterConfig,
    state_machine: StateMachine<SCState>,

    steps: Vec<SequenceStep>,
    current_step_index: usize,

    status_timer_ms: u32,
    ready_timer_ms: u32,
    active_timer_ms: u32,
    client_ack_received: bool,
    /// Milliseconds since each client's last accepted SCClientStatus.
    ///
    /// G1 — ISO 11783-14 F.3 applies a 600 ms timeout in Play Back. The master
    /// only ran a timer *until* `client_ack_received` latched, so once the
    /// required clients had acknowledged a step it stopped watching them
    /// entirely: a client that then lost power, dropped off the bus, or hung
    /// never halted the sequence.
    client_seen: BTreeMap<u8, ClientLiveness>,
    ready_clients: HashSet<u8>,
    active_ack_clients: HashSet<u8>,
    /// Clients that have reported the Abort sequence state while the master is
    /// in Abort.
    ///
    /// G3 — §4.4.7.3 has the master return to Ready once every enabled SCC has
    /// confirmed the abort. `SCState::Error` used to be a terminal sink, so the
    /// only way to run another sequence was to build a new `SCMaster`.
    abort_confirmed: HashSet<u8>,
    /// Clients observed reporting Abort since the master entered it. A
    /// confirmation is only credited to a client that appears here first, so
    /// the frame that *caused* the abort cannot also confirm it (D2).
    abort_seen: HashSet<u8>,
    busy_nv_memory: bool,
    busy_parsing_scd: bool,

    /// Fires `(from, to)` after every state transition.
    pub on_state_change: Event<(SCState, SCState)>,
    /// Fires the step id when a step is dispatched to the client.
    pub on_step_started: Event<u16>,
    /// Fires the step id when the master records a step as complete.
    pub on_step_completed: Event<u16>,
    /// Fires when the last step has been completed.
    pub on_sequence_complete: Event<()>,
    /// Fires with a description when a Ready/Active timeout strikes.
    pub on_timeout: Event<&'static str>,
    /// Fires with `(client_addr, mapped_state)` whenever a client
    /// status arrives.
    pub on_client_status: Event<(u8, SCState)>,
}

impl SCMaster {
    #[must_use]
    pub fn new(config: SCMasterConfig) -> Self {
        Self {
            config,
            state_machine: StateMachine::new(SCState::Idle),
            steps: Vec::new(),
            current_step_index: 0,
            status_timer_ms: 0,
            ready_timer_ms: 0,
            active_timer_ms: 0,
            client_ack_received: false,
            client_seen: BTreeMap::new(),
            ready_clients: HashSet::new(),
            active_ack_clients: HashSet::new(),
            abort_confirmed: HashSet::new(),
            abort_seen: HashSet::new(),
            busy_nv_memory: false,
            busy_parsing_scd: false,
            on_state_change: Event::new(),
            on_step_started: Event::new(),
            on_step_completed: Event::new(),
            on_sequence_complete: Event::new(),
            on_timeout: Event::new(),
            on_client_status: Event::new(),
        }
    }

    // ─── Step management ───────────────────────────────────────────────

    pub fn add_step(&mut self, step: SequenceStep) -> Result<()> {
        if !self.state_machine.is(SCState::Idle) {
            return Err(Error::invalid_state("can only add steps in Idle state"));
        }
        if step.step_id > SC_MAX_SEQUENCE_STEP_ID {
            return Err(Error::invalid_data(format!(
                "SC step_id {} exceeds wire maximum {}",
                step.step_id, SC_MAX_SEQUENCE_STEP_ID
            )));
        }
        if self
            .steps
            .iter()
            .any(|existing| existing.step_id == step.step_id)
        {
            return Err(Error::invalid_data(format!(
                "duplicate SC step_id {}",
                step.step_id
            )));
        }
        self.steps.push(step);
        Ok(())
    }

    #[must_use]
    pub fn steps(&self) -> &[SequenceStep] {
        &self.steps
    }

    #[must_use]
    pub fn current_step(&self) -> Option<&SequenceStep> {
        self.steps.get(self.current_step_index)
    }

    // ─── Sequence control ─────────────────────────────────────────────

    pub fn start(&mut self) -> Result<()> {
        if !self.state_machine.is(SCState::Idle) {
            return Err(Error::invalid_state("can only start from Idle"));
        }
        if self.steps.is_empty() {
            return Err(Error::invalid_state("no steps defined"));
        }
        self.current_step_index = 0;
        self.ready_timer_ms = 0;
        self.status_timer_ms = 0;
        self.client_ack_received = false;
        self.ready_clients.clear();
        self.active_ack_clients.clear();
        self.transition(SCState::Ready);
        Ok(())
    }

    pub fn abort(&mut self) -> Result<()> {
        let s = self.state_machine.state();
        if matches!(s, SCState::Idle | SCState::Complete | SCState::Error) {
            return Err(Error::invalid_state("nothing to abort"));
        }
        self.transition(SCState::Error);
        // Abort is protocol-visible, not just local state. Force the next
        // update to emit an Abort sequence-state status immediately.
        self.status_timer_ms = self.config.status_interval_ms;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        if !self.state_machine.is(SCState::Active) {
            return Err(Error::invalid_state("can only pause in Active state"));
        }
        self.transition(SCState::Paused);
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        if !self.state_machine.is(SCState::Paused) {
            return Err(Error::invalid_state("can only resume from Paused state"));
        }
        self.active_timer_ms = 0;
        self.client_ack_received = false;
        self.active_ack_clients.clear();
        self.transition(SCState::Active);
        Ok(())
    }

    pub fn step_completed(&mut self, step_id: u16) -> Result<()> {
        if !self.state_machine.is(SCState::Active) {
            return Err(Error::invalid_state("not in Active state"));
        }
        let cur = self
            .steps
            .get_mut(self.current_step_index)
            .ok_or_else(|| Error::invalid_state("no current step"))?;
        if cur.step_id != step_id {
            return Err(Error::invalid_state("step_id mismatch"));
        }
        cur.completed = true;
        self.on_step_completed.emit(&step_id);

        self.current_step_index += 1;
        if self.current_step_index >= self.steps.len() {
            self.transition(SCState::Complete);
            self.on_sequence_complete.emit(&());
        } else {
            self.active_timer_ms = 0;
            self.client_ack_received = false;
            self.active_ack_clients.clear();
            let next_id = self.steps[self.current_step_index].step_id;
            self.on_step_started.emit(&next_id);
        }
        Ok(())
    }

    // ─── Periodic update ──────────────────────────────────────────────

    /// Advance timers. Returns the `[u8; 8]` payload to emit on
    /// `PGN_SC_MASTER_STATUS` if the status interval has elapsed,
    /// otherwise `None`. Also drives Ready/Active timeouts.
    pub fn update(&mut self, elapsed_ms: u32) -> Option<[u8; 8]> {
        let s = self.state_machine.state();

        let mut to_send: Option<[u8; 8]> = None;

        // D3 — the master used to return here in Idle and Complete, so it went
        // silent the moment a sequence finished. Every client then hit its own
        // F.2 reception timeout and latched Abort: a *successful* sequence
        // ended with a permanent fault indication on every SCC on the bus. F.2
        // byte 4's "FF16 When byte 2 is set to inactive" only has a purpose if
        // an inactive master is still transmitting, and F.3 makes the parallel
        // case explicit for a disabled client.
        self.status_timer_ms = self.status_timer_ms.saturating_add(elapsed_ms);
        let interval = if matches!(s, SCState::Idle | SCState::Complete) {
            SC_STATUS_IDLE_RATE_MS
        } else {
            self.config.status_interval_ms
        };
        if self.status_timer_ms >= interval {
            self.status_timer_ms -= interval;
            to_send = Some(self.encode_master_status());
        }
        if matches!(s, SCState::Idle | SCState::Complete) {
            return to_send;
        }

        match s {
            SCState::Ready => {
                self.ready_timer_ms = self.ready_timer_ms.saturating_add(elapsed_ms);
                if self.ready_timer_ms >= self.config.ready_timeout_ms {
                    self.transition(SCState::Error);
                    self.on_timeout.emit(&"ready timeout");
                    self.status_timer_ms = 0;
                    to_send = Some(self.encode_master_status());
                }
            }
            SCState::Active => {
                if !self.client_ack_received {
                    self.active_timer_ms = self.active_timer_ms.saturating_add(elapsed_ms);
                    if self.active_timer_ms >= self.config.active_timeout_ms {
                        self.transition(SCState::Error);
                        self.on_timeout.emit(&"active timeout - no client ack");
                        self.status_timer_ms = 0;
                        to_send = Some(self.encode_master_status());
                    }
                }
            }
            _ => {}
        }

        // G1 — F.3: "A timeout of 600 ms for the SCClientStatus message shall be
        // applied in the 'Recording', 'Play Back' or 'Abort' state. A timeout of
        // 3 s shall be applied in the 'Ready' state." This runs for the whole
        // time a client is enrolled, not only until it first acknowledges.
        //
        // D1 — the limit belongs to the *reporting client's* state, not the
        // master's. F.3's cadence sentence has a client transmit "once per
        // second during the 'Ready' state or if SCC is disabled, and 5 messages
        // per second during the active states", so holding an enrolled-but-Ready
        // SCC to the 600 ms active limit while the master is in Play Back aged
        // it out every cycle. Two machbus SCCs on one bus self-aborted every
        // sequence after 600 ms, because 431df01 makes an idle client emit its
        // disabled status at exactly 1 Hz.
        if matches!(s, SCState::Ready | SCState::Active | SCState::Paused) {
            let mut expired = false;
            for seen in self.client_seen.values_mut() {
                seen.age_ms = seen.age_ms.saturating_add(elapsed_ms);
                expired |= seen.age_ms >= self.config.limit_for(seen.reported);
            }
            if expired && !matches!(self.state_machine.state(), SCState::Error) {
                let config = self.config;
                self.client_seen
                    .retain(|_, seen| seen.age_ms < config.limit_for(seen.reported));
                self.transition(SCState::Error);
                self.on_timeout.emit(&"client status timeout");
                self.status_timer_ms = 0;
                to_send = Some(self.encode_master_status());
            }
        }
        to_send
    }

    // ─── State / busy access ──────────────────────────────────────────

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

    pub fn set_busy_nv_memory(&mut self, busy: bool) {
        self.busy_nv_memory = busy;
    }

    pub fn set_busy_parsing_scd(&mut self, busy: bool) {
        self.busy_parsing_scd = busy;
    }

    // ─── Inbound handler ──────────────────────────────────────────────

    /// Decode an incoming `PGN_SC_CLIENT_STATUS` message and update
    /// internal state. Compatibility wrapper: malformed or unrelated
    /// frames are ignored. Use [`Self::try_handle_client_status`] when a
    /// caller needs an explicit validation error instead of a no-op.
    pub fn handle_client_status(&mut self, msg: &Message) {
        let _ = self.try_handle_client_status(msg);
    }

    /// Decode an incoming `PGN_SC_CLIENT_STATUS` message and update
    /// internal state, returning an explicit error for malformed or
    /// unrelated frames. A client Abort status is treated as a
    /// sequence-level error and makes the next [`Self::update`] emit an
    /// Abort status immediately.
    pub fn try_handle_client_status(&mut self, msg: &Message) -> Result<()> {
        if msg.pgn != PGN_SC_CLIENT_STATUS {
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
                "SC client status must be exactly 8 bytes",
            ));
        }
        if msg.get_u8(0) != SC_MSG_CODE_CLIENT {
            return Err(Error::invalid_data(
                "SC client status has wrong message code",
            ));
        }
        if !sc_status_payload_len_is_canonical(&msg.data) {
            return Err(Error::invalid_data(
                "SC client status has a wrong payload length",
            ));
        }
        let client_state_raw = msg.get_u8(1);
        if !sc_client_state_byte_is_valid(client_state_raw) {
            return Err(Error::invalid_data(
                "SC client status has reserved client state",
            ));
        }
        let client_state = super::types::SCClientState::try_from_u8(client_state_raw)
            .expect("validated SC client state byte");
        if matches!(client_state, super::types::SCClientState::Initialization) {
            return Err(Error::invalid_data(
                "SC client initialization state is not supported",
            ));
        }
        if !sc_client_func_error_byte_is_valid(msg.get_u8(4)) {
            return Err(Error::invalid_data(
                "SC client status has reserved function error byte",
            ));
        }
        let seq_state_raw = msg.get_u8(3);
        if !sc_sequence_state_byte_is_valid(seq_state_raw) {
            return Err(Error::invalid_data(
                "SC client status has reserved sequence state",
            ));
        }
        let seq_state =
            SCSequenceState::try_from_u8(seq_state_raw).expect("validated SC sequence state byte");
        let seq_num = msg.get_u8(2);
        if !matches!(client_state, super::types::SCClientState::Enabled)
            && !sc_inactive_status_sequence_fields_are_valid(seq_state, seq_num)
        {
            return Err(Error::invalid_data(
                "SC disabled client status has active sequence fields",
            ));
        }
        if matches!(client_state, super::types::SCClientState::Enabled)
            && !sc_status_sequence_state_is_supported(seq_state)
        {
            return Err(Error::invalid_data(
                "SC client status has unsupported sequence state",
            ));
        }
        if !sc_status_sequence_number_is_valid(seq_state, seq_num) {
            return Err(Error::invalid_data(
                "SC client status has invalid sequence number",
            ));
        }
        let client_addr = msg.source;
        // Any well-formed status is evidence the client is still there — but a
        // *disabled* SCC is not part of this sequence at all, so enrolling it
        // only creates something for the watchdog to trip over (D1). Drop it
        // instead, and remember what an enabled one reported so its timeout
        // matches the cadence it is actually transmitting on.
        if matches!(client_state, super::types::SCClientState::Enabled) {
            self.client_seen.insert(
                client_addr,
                ClientLiveness {
                    age_ms: 0,
                    reported: seq_state,
                },
            );
        } else {
            self.client_seen.remove(&client_addr);
        }

        let mapped_state = match client_state {
            super::types::SCClientState::Enabled => match seq_state {
                SCSequenceState::Ready => SCState::Ready,
                SCSequenceState::PlayBack => SCState::Active,
                SCSequenceState::Abort => SCState::Error,
                _ => SCState::Ready,
            },
            _ => SCState::Idle,
        };

        self.on_client_status.emit(&(client_addr, mapped_state));

        if mapped_state == SCState::Error
            && !matches!(
                self.state_machine.state(),
                SCState::Idle | SCState::Complete | SCState::Error
            )
        {
            self.transition(SCState::Error);
            self.status_timer_ms = self.config.status_interval_ms;
            return Ok(());
        }

        // §4.4.7.3 — the master leaves Abort once every enabled SCC has
        // confirmed it.
        //
        // D2 — "confirmed" is the *trailing* edge, not the leading one. The
        // first version credited any Abort status, including the very frame
        // that had just driven the master into Abort one call earlier: frame 1
        // took the branch above (Active -> Error), frame 2 was identical and
        // counted as its own confirmation, `rearm_after_abort` returned the
        // master to Ready, frame 3 aborted it again. That oscillated at the
        // client's 5 Hz cadence, and because rearming resets the step index a
        // second healthy SCC answering one of the Ready frames re-dispatched
        // step 1 to the machine, over and over, for as long as the faulted
        // client held Abort.
        //
        // So: a client has to be *seen* in Abort and then leave it. Only
        // clients enrolled when the abort began can confirm, and the bar is
        // every enabled client rather than a static configured count.
        if self.state_machine.is(SCState::Error) {
            if mapped_state == SCState::Error {
                self.abort_seen.insert(client_addr);
                self.abort_confirmed.remove(&client_addr);
            } else if self.abort_seen.contains(&client_addr) {
                self.abort_confirmed.insert(client_addr);
            }
            let enabled = self.client_seen.len().max(1);
            if self.abort_confirmed.len() >= enabled {
                self.rearm_after_abort();
            }
            return Ok(());
        }

        if self.state_machine.is(SCState::Ready) && mapped_state != SCState::Ready {
            self.ready_clients.remove(&client_addr);
        }

        if self.state_machine.is(SCState::Active)
            && (mapped_state != SCState::Active || msg.get_u8(2) != self.current_sequence_number())
        {
            let had_active_ack = self.active_ack_clients.remove(&client_addr);
            if had_active_ack && self.active_ack_clients.len() < self.config.required_client_count()
            {
                self.client_ack_received = false;
                self.active_timer_ms = 0;
            }
        }

        if self.state_machine.is(SCState::Ready) && mapped_state == SCState::Ready {
            self.ready_clients.insert(client_addr);
            if self.ready_clients.len() >= self.config.required_client_count() {
                self.active_timer_ms = 0;
                self.client_ack_received = false;
                self.active_ack_clients.clear();
                self.transition(SCState::Active);
                if let Some(first) = self.steps.first() {
                    let id = first.step_id;
                    self.on_step_started.emit(&id);
                }
            }
        }

        if self.state_machine.is(SCState::Active)
            && mapped_state == SCState::Active
            && msg.get_u8(2) == self.current_sequence_number()
        {
            self.active_ack_clients.insert(client_addr);
            if self.active_ack_clients.len() >= self.config.required_client_count() {
                self.client_ack_received = true;
                self.active_timer_ms = 0;
            }
        }
        Ok(())
    }

    // ─── Internals ────────────────────────────────────────────────────

    fn transition(&mut self, new_state: SCState) {
        let old = self.state_machine.state();
        if old == new_state {
            return;
        }
        if matches!(new_state, SCState::Error) {
            self.abort_confirmed.clear();
            self.abort_seen.clear();
        }
        self.state_machine.transition(new_state);
        self.on_state_change.emit(&(old, new_state));
    }

    /// Leave Abort for Ready once every enabled SCC has confirmed (§4.4.7.3).
    /// The aborted run is discarded, not resumed: the sequence restarts from
    /// the first step, so a later `step_completed` cannot credit a step the
    /// abort already invalidated.
    fn rearm_after_abort(&mut self) {
        self.abort_confirmed.clear();
        self.abort_seen.clear();
        self.ready_clients.clear();
        self.active_ack_clients.clear();
        self.client_ack_received = false;
        self.current_step_index = 0;
        for step in &mut self.steps {
            step.completed = false;
        }
        self.ready_timer_ms = 0;
        self.active_timer_ms = 0;
        self.transition(SCState::Ready);
        self.status_timer_ms = self.config.status_interval_ms;
    }

    fn iso_master_state(&self) -> SCMasterState {
        match self.state_machine.state() {
            SCState::Idle | SCState::Complete => SCMasterState::Inactive,
            _ => SCMasterState::Active,
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

    fn encode_master_status(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let master = self.iso_master_state();
        let seq = self.iso_sequence_state();
        data[0] = SC_MSG_CODE_MASTER;
        data[1] = master.as_u8();
        data[2] =
            if matches!(master, SCMasterState::Inactive) || matches!(seq, SCSequenceState::Ready) {
                SC_SEQUENCE_NUMBER_NOT_AVAILABLE
            } else {
                self.current_sequence_number()
            };
        // G4 — F.2 byte 4: "FF16 When byte 2 is set to inactive".
        data[3] = if matches!(master, SCMasterState::Inactive) {
            SCSequenceState::NotApplicable.as_u8()
        } else {
            seq.as_u8()
        };
        // G3 — the six unallocated bits of byte 5 go out as 1, not clobbered
        // to 0 along with the two busy flags.
        data[4] = 0xFC | u8::from(self.busy_nv_memory) | (u8::from(self.busy_parsing_scd) << 1);
        data
    }

    fn current_sequence_number(&self) -> u8 {
        self.steps
            .get(self.current_step_index)
            .filter(|step| step.step_id <= SC_MAX_SEQUENCE_STEP_ID)
            .map_or(SC_SEQUENCE_NUMBER_NOT_AVAILABLE, |step| step.step_id as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
    use crate::net::pgn_defs::{PGN_SC_CLIENT_STATUS, PGN_SC_MASTER_STATUS};

    fn step(id: u16) -> SequenceStep {
        SequenceStep {
            step_id: id,
            description: format!("step {id}"),
            duration_ms: 0,
            completed: false,
        }
    }

    fn client_status(addr: u8, sequence_state: SCSequenceState, sequence_number: u8) -> Message {
        Message::new(
            PGN_SC_CLIENT_STATUS,
            vec![
                SC_MSG_CODE_CLIENT,
                super::super::types::SCClientState::Enabled.as_u8(),
                sequence_number,
                sequence_state.as_u8(),
                0,
                0xFF,
                0xFF,
                0xFF,
            ],
            addr,
        )
    }

    #[test]
    fn add_step_only_in_idle() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        assert!(m.add_step(step(1)).is_ok());
        m.add_step(step(2)).unwrap();
        m.start().unwrap();
        assert!(m.add_step(step(3)).is_err());
    }

    #[test]
    fn add_step_rejects_wire_unencodable_or_duplicate_ids() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        m.add_step(step(0)).unwrap();

        let dup = m.add_step(step(0)).unwrap_err();
        assert_eq!(dup.code, crate::net::error::ErrorCode::InvalidData);
        assert!(dup.message.contains("duplicate"));

        let too_large = m.add_step(step(SC_MAX_SEQUENCE_STEP_ID + 1)).unwrap_err();
        assert_eq!(too_large.code, crate::net::error::ErrorCode::InvalidData);
        assert!(too_large.message.contains("wire maximum"));
    }

    #[test]
    fn start_requires_steps() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        assert!(m.start().is_err());
    }

    #[test]
    fn start_transitions_to_ready() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        m.add_step(step(1)).unwrap();
        m.start().unwrap();
        assert!(m.is(SCState::Ready));
    }

    #[test]
    fn ready_timeout_emits_error() {
        let cfg = SCMasterConfig::default()
            .with_ready_timeout(100)
            .with_status_interval(1_000_000);
        let mut m = SCMaster::new(cfg);
        m.add_step(step(1)).unwrap();
        m.start().unwrap();
        m.update(50);
        assert!(m.is(SCState::Ready));
        m.update(60);
        assert!(m.is(SCState::Error));
    }

    #[test]
    fn update_emits_status_at_interval() {
        let cfg = SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000);
        let mut m = SCMaster::new(cfg);
        m.add_step(step(1)).unwrap();
        m.start().unwrap();
        assert_eq!(m.update(50), None);
        let bytes = m.update(50).unwrap();
        assert_eq!(bytes[0], SC_MSG_CODE_MASTER);
        assert_eq!(bytes[1], SCMasterState::Active.as_u8());
        // Sequence state in Ready ⇒ Ready, sequence number is 0xFF.
        assert_eq!(bytes[2], 0xFF);
        assert_eq!(bytes[3], SCSequenceState::Ready.as_u8());
    }

    #[test]
    fn client_ready_drives_master_to_active() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        m.add_step(step(7)).unwrap();
        m.start().unwrap();

        let payload = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            0xFF,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        let msg = Message::new(PGN_SC_CLIENT_STATUS, payload, 0x42);
        m.handle_client_status(&msg);
        assert!(m.is(SCState::Active));
    }

    #[test]
    fn client_status_rejects_short_and_overlong_payloads() {
        let mut short_master = SCMaster::new(SCMasterConfig::default());
        short_master.add_step(step(7)).unwrap();
        short_master.start().unwrap();
        let mut short = client_status(0x41, SCSequenceState::Ready, 0xFF);
        short.data.pop();
        short_master.handle_client_status(&short);
        assert!(short_master.is(SCState::Ready));

        let mut overlong_master = SCMaster::new(SCMasterConfig::default());
        overlong_master.add_step(step(7)).unwrap();
        overlong_master.start().unwrap();
        let mut overlong = client_status(0x41, SCSequenceState::Ready, 0xFF);
        overlong.data.push(0xFF);
        overlong_master.handle_client_status(&overlong);
        assert!(overlong_master.is(SCState::Ready));
    }

    #[test]
    fn try_client_status_reports_validation_errors_without_state_mutation() {
        let mut master = SCMaster::new(SCMasterConfig::default());
        master.add_step(step(7)).unwrap();
        master.start().unwrap();

        let mut wrong_pgn = client_status(0x41, SCSequenceState::Ready, 0xFF);
        wrong_pgn.pgn = PGN_SC_MASTER_STATUS;
        let err = master.try_handle_client_status(&wrong_pgn).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidPgn);
        assert!(master.is(SCState::Ready));

        let err = master
            .try_handle_client_status(&client_status(NULL_ADDRESS, SCSequenceState::Ready, 0xFF))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidAddress);
        assert!(master.is(SCState::Ready));

        let mut short = client_status(0x41, SCSequenceState::Ready, 0xFF);
        short.data.pop();
        let err = master.try_handle_client_status(&short).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("exactly 8 bytes"));
        assert!(master.is(SCState::Ready));
    }

    #[test]
    fn client_status_rejects_wrong_pgn_and_invalid_source_addresses() {
        let mut wrong_pgn_master = SCMaster::new(SCMasterConfig::default());
        wrong_pgn_master.add_step(step(7)).unwrap();
        wrong_pgn_master.start().unwrap();
        let mut wrong_pgn = client_status(0x41, SCSequenceState::Ready, 0xFF);
        wrong_pgn.pgn = PGN_SC_MASTER_STATUS;
        wrong_pgn_master.handle_client_status(&wrong_pgn);
        assert!(wrong_pgn_master.is(SCState::Ready));

        for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut master = SCMaster::new(SCMasterConfig::default());
            master.add_step(step(7)).unwrap();
            master.start().unwrap();
            master.handle_client_status(&client_status(bad_source, SCSequenceState::Ready, 0xFF));
            assert!(
                master.is(SCState::Ready),
                "SC client status from invalid source 0x{bad_source:02X} must be ignored"
            );
        }
    }

    #[test]
    fn client_status_rejects_invalid_sequence_numbers_for_state() {
        let mut ready_master = SCMaster::new(SCMasterConfig::default());
        ready_master.add_step(step(7)).unwrap();
        ready_master.start().unwrap();
        ready_master.handle_client_status(&client_status(0x41, SCSequenceState::Ready, 7));
        assert!(
            ready_master.is(SCState::Ready),
            "Ready statuses must use the 0xFF not-applicable sequence number"
        );

        let cfg = SCMasterConfig::default()
            .with_ready_timeout(1_000_000)
            .with_active_timeout(100)
            .with_status_interval(1_000_000);
        let mut active_master = SCMaster::new(cfg);
        active_master.add_step(step(7)).unwrap();
        active_master.start().unwrap();
        active_master.handle_client_status(&client_status(0x41, SCSequenceState::Ready, 0xFF));
        assert!(active_master.is(SCState::Active));

        active_master.handle_client_status(&client_status(0x41, SCSequenceState::PlayBack, 0xFF));
        let abort_status = active_master.update(101).unwrap();
        assert!(active_master.is(SCState::Error));
        assert_eq!(abort_status[3], SCSequenceState::Abort.as_u8());
    }

    #[test]
    fn client_status_rejects_unsupported_enabled_sequence_states() {
        let mut ready_master = SCMaster::new(SCMasterConfig::default());
        ready_master.add_step(step(7)).unwrap();
        ready_master.start().unwrap();
        ready_master.handle_client_status(&client_status(0x41, SCSequenceState::Recording, 7));
        assert!(
            ready_master.is(SCState::Ready),
            "unsupported enabled client states must not start playback"
        );

        let cfg = SCMasterConfig::default()
            .with_ready_timeout(1_000_000)
            .with_active_timeout(100)
            .with_status_interval(1_000_000);
        let mut active_master = SCMaster::new(cfg);
        active_master.add_step(step(7)).unwrap();
        active_master.start().unwrap();
        active_master.handle_client_status(&client_status(0x41, SCSequenceState::Ready, 0xFF));
        assert!(active_master.is(SCState::Active));

        active_master.handle_client_status(&client_status(
            0x41,
            SCSequenceState::RecordingCompletion,
            7,
        ));
        let abort_status = active_master.update(101).unwrap();
        assert!(active_master.is(SCState::Error));
        assert_eq!(abort_status[3], SCSequenceState::Abort.as_u8());
    }

    #[test]
    fn client_status_rejects_reserved_client_state_bytes() {
        let mut ready_master = SCMaster::new(SCMasterConfig::default());
        ready_master.add_step(step(7)).unwrap();
        ready_master.start().unwrap();
        let mut malformed_ready = client_status(0x41, SCSequenceState::Ready, 0xFF);
        malformed_ready.data[1] = 0x7F;
        ready_master.handle_client_status(&malformed_ready);
        assert!(
            ready_master.is(SCState::Ready),
            "reserved client-state bytes must not be coerced to Disabled/Idle"
        );

        let cfg = SCMasterConfig::default()
            .with_ready_timeout(1_000_000)
            .with_active_timeout(100)
            .with_status_interval(1_000_000);
        let mut active_master = SCMaster::new(cfg);
        active_master.add_step(step(7)).unwrap();
        active_master.start().unwrap();
        active_master.handle_client_status(&client_status(0x41, SCSequenceState::Ready, 0xFF));
        assert!(active_master.is(SCState::Active));

        let mut malformed_ack = client_status(0x41, SCSequenceState::PlayBack, 7);
        malformed_ack.data[1] = 0x7F;
        active_master.handle_client_status(&malformed_ack);
        let abort_status = active_master.update(101).unwrap();
        assert!(active_master.is(SCState::Error));
        assert_eq!(abort_status[3], SCSequenceState::Abort.as_u8());
    }

    #[test]
    fn client_status_rejects_a_reserved_function_error_value() {
        let mut bad_error_master = SCMaster::new(SCMasterConfig::default());
        bad_error_master.add_step(step(7)).unwrap();
        bad_error_master.start().unwrap();
        let mut bad_error = client_status(0x41, SCSequenceState::Ready, 0xFF);
        bad_error.data[4] = 0x04;
        bad_error_master.handle_client_status(&bad_error);
        assert!(bad_error_master.is(SCState::Ready));

        // G3 — a zero-padded reserved tail is still a valid client status. This
        // used to be rejected before any state update, so the master never left
        // Ready for a peer that differed in bits neither side uses.
        let mut zero_padded_master = SCMaster::new(SCMasterConfig::default());
        zero_padded_master.add_step(step(7)).unwrap();
        zero_padded_master.start().unwrap();
        let mut zeroed = client_status(0x41, SCSequenceState::Ready, 0xFF);
        zeroed.data[7] = 0;
        zero_padded_master.handle_client_status(&zeroed);
        assert!(
            !zero_padded_master.is(SCState::Ready),
            "the client's Ready must be counted despite the zero-padded tail"
        );
    }

    #[test]
    fn required_client_count_waits_for_unique_ready_clients() {
        let mut m = SCMaster::new(
            SCMasterConfig::default()
                .with_required_client_count(2)
                .with_ready_timeout(1_000_000),
        );
        m.add_step(step(7)).unwrap();
        m.start().unwrap();

        m.handle_client_status(&client_status(0x41, SCSequenceState::Ready, 0xFF));
        assert!(
            m.is(SCState::Ready),
            "one ready client must not start a two-client sequence"
        );
        m.handle_client_status(&client_status(0x41, SCSequenceState::Ready, 0xFF));
        assert!(
            m.is(SCState::Ready),
            "duplicate Ready from the same client is not a second participant"
        );
        m.handle_client_status(&client_status(0x42, SCSequenceState::Ready, 0xFF));
        assert!(m.is(SCState::Active));
    }

    #[test]
    fn step_completed_advances_then_completes() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        m.add_step(step(1)).unwrap();
        m.add_step(step(2)).unwrap();
        m.start().unwrap();
        // Force into Active for testing.
        let payload = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            0xFF,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(PGN_SC_CLIENT_STATUS, payload, 0x42));
        assert!(m.is(SCState::Active));
        m.step_completed(1).unwrap();
        assert!(m.is(SCState::Active));
        assert_eq!(m.current_step().unwrap().step_id, 2);
        m.step_completed(2).unwrap();
        assert!(m.is(SCState::Complete));
    }

    #[test]
    fn pause_only_from_active() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        assert!(m.pause().is_err());
        m.add_step(step(1)).unwrap();
        m.start().unwrap();
        assert!(m.pause().is_err());
    }

    #[test]
    fn resume_only_from_paused() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        assert!(m.resume().is_err());
    }

    #[test]
    fn abort_from_active_ok() {
        let mut m = SCMaster::new(SCMasterConfig::default());
        m.add_step(step(1)).unwrap();
        m.start().unwrap();
        m.abort().unwrap();
        assert!(m.is(SCState::Error));
        let abort_status = m.update(0).unwrap();
        assert_eq!(abort_status[0], SC_MSG_CODE_MASTER);
        assert_eq!(abort_status[1], SCMasterState::Active.as_u8());
        assert_eq!(abort_status[2], 1);
        assert_eq!(abort_status[3], SCSequenceState::Abort.as_u8());
        assert!(m.abort().is_err());
    }

    #[test]
    fn active_timeout_without_ack_errors() {
        let cfg = SCMasterConfig::default()
            .with_active_timeout(100)
            .with_ready_timeout(1_000_000)
            .with_status_interval(1_000_000);
        let mut m = SCMaster::new(cfg);
        m.add_step(step(1)).unwrap();
        m.start().unwrap();
        // Drive into Active.
        let payload = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            0xFF,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(PGN_SC_CLIENT_STATUS, payload, 0x42));
        assert!(m.is(SCState::Active));
        let abort_status = m.update(150).unwrap();
        assert!(m.is(SCState::Error));
        assert_eq!(abort_status[3], SCSequenceState::Abort.as_u8());
    }

    #[test]
    fn required_client_count_requires_all_playback_acks() {
        let cfg = SCMasterConfig::default()
            .with_required_client_count(2)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(100)
            .with_status_interval(1_000_000);

        let mut partial = SCMaster::new(cfg);
        partial.add_step(step(7)).unwrap();
        partial.start().unwrap();
        partial.handle_client_status(&client_status(0x41, SCSequenceState::Ready, 0xFF));
        partial.handle_client_status(&client_status(0x42, SCSequenceState::Ready, 0xFF));
        assert!(partial.is(SCState::Active));

        partial.handle_client_status(&client_status(0x41, SCSequenceState::PlayBack, 7));
        assert!(
            partial.update(99).is_none(),
            "one client ack is not enough but timeout has not elapsed yet"
        );
        let abort_status = partial.update(2).unwrap();
        assert!(partial.is(SCState::Error));
        assert_eq!(abort_status[3], SCSequenceState::Abort.as_u8());

        let mut complete = SCMaster::new(cfg);
        complete.add_step(step(7)).unwrap();
        complete.start().unwrap();
        complete.handle_client_status(&client_status(0x41, SCSequenceState::Ready, 0xFF));
        complete.handle_client_status(&client_status(0x42, SCSequenceState::Ready, 0xFF));
        complete.handle_client_status(&client_status(0x41, SCSequenceState::PlayBack, 7));
        complete.handle_client_status(&client_status(0x42, SCSequenceState::PlayBack, 7));
        assert!(complete.is(SCState::Active));

        // G1 — F.3 applies the 600 ms client-status timeout for the whole of
        // Play Back, not only until the first acknowledgement. This used to
        // assert the timeout was "disarmed" once every required client had
        // acked, so a client that then lost power never halted the sequence.
        assert!(complete.update(50).is_none(), "no timeout before the limit");
        assert!(complete.is(SCState::Active));
        let halted = complete
            .update(1_000)
            .expect("silent clients must halt the sequence");
        assert!(complete.is(SCState::Error));
        assert_eq!(halted[3], SCSequenceState::Abort.as_u8());
    }

    #[test]
    fn client_abort_drives_master_error_and_visible_abort_status() {
        let cfg = SCMasterConfig::default()
            .with_ready_timeout(1_000_000)
            .with_active_timeout(1_000_000)
            .with_status_interval(1_000_000);
        let mut m = SCMaster::new(cfg);
        m.add_step(step(7)).unwrap();
        m.start().unwrap();

        let ready = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            0xFF,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(PGN_SC_CLIENT_STATUS, ready, 0x42));
        assert!(m.is(SCState::Active));

        let abort = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            7,
            SCSequenceState::Abort.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(PGN_SC_CLIENT_STATUS, abort, 0x42));
        assert!(m.is(SCState::Error));
        let abort_status = m.update(0).unwrap();
        assert_eq!(abort_status[0], SC_MSG_CODE_MASTER);
        assert_eq!(abort_status[1], SCMasterState::Active.as_u8());
        assert_eq!(abort_status[2], 7);
        assert_eq!(abort_status[3], SCSequenceState::Abort.as_u8());
    }

    #[test]
    fn active_ack_requires_current_sequence_number() {
        let cfg = SCMasterConfig::default()
            .with_active_timeout(100)
            .with_ready_timeout(1_000_000)
            .with_status_interval(1_000_000);
        let mut m = SCMaster::new(cfg);
        m.add_step(step(7)).unwrap();
        m.start().unwrap();

        let ready = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            0xFF,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(PGN_SC_CLIENT_STATUS, ready, 0x42));
        assert!(m.is(SCState::Active));

        let wrong_sequence = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            0,
            SCSequenceState::PlayBack.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(PGN_SC_CLIENT_STATUS, wrong_sequence, 0x42));
        m.update(150);
        assert!(m.is(SCState::Error));
    }

    #[test]
    fn active_ack_for_current_sequence_prevents_timeout() {
        let cfg = SCMasterConfig::default()
            .with_active_timeout(100)
            .with_ready_timeout(1_000_000)
            .with_status_interval(1_000_000);
        let mut m = SCMaster::new(cfg);
        m.add_step(step(7)).unwrap();
        m.start().unwrap();

        let ready = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            0xFF,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(PGN_SC_CLIENT_STATUS, ready, 0x42));
        assert!(m.is(SCState::Active));

        let current_sequence = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            7,
            SCSequenceState::PlayBack.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(
            PGN_SC_CLIENT_STATUS,
            current_sequence.clone(),
            0x42,
        ));

        // The acknowledgement disarms the *ack* timeout. The F.3 client-status
        // timeout is separate and keeps running, so the client has to go on
        // reporting — here it does, at half the 100 ms limit.
        for _ in 0..4 {
            m.update(50);
            assert!(m.is(SCState::Active));
            m.handle_client_status(&Message::new(
                PGN_SC_CLIENT_STATUS,
                current_sequence.clone(),
                0x42,
            ));
        }
        assert!(m.is(SCState::Active));
    }

    #[test]
    fn status_uses_wire_visible_step_id_not_step_index() {
        let cfg = SCMasterConfig::default()
            .with_status_interval(10)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(1_000_000);
        let mut m = SCMaster::new(cfg);
        m.add_step(step(7)).unwrap();
        m.add_step(step(SC_MAX_SEQUENCE_STEP_ID)).unwrap();
        m.start().unwrap();

        let ready = vec![
            SC_MSG_CODE_CLIENT,
            super::super::types::SCClientState::Enabled.as_u8(),
            0xFF,
            SCSequenceState::Ready.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
        ];
        m.handle_client_status(&Message::new(PGN_SC_CLIENT_STATUS, ready, 0x42));
        assert!(m.is(SCState::Active));

        let first = m.update(10).expect("active status emits");
        assert_eq!(first[2], 7);

        m.step_completed(7).unwrap();
        let second = m.update(10).expect("next active status emits");
        assert_eq!(second[2], SC_MAX_SEQUENCE_STEP_ID as u8);
    }

    #[test]
    fn busy_flags_show_in_status_byte_4() {
        let cfg = SCMasterConfig::default().with_status_interval(10);
        let mut m = SCMaster::new(cfg);
        m.add_step(step(1)).unwrap();
        m.start().unwrap();
        m.set_busy_nv_memory(true);
        m.set_busy_parsing_scd(true);
        let bytes = m.update(20).unwrap();
        assert_eq!(bytes[4] & 0x03, 0x03);
    }
}
