//! ISO 11783-10 Task Controller client.
//!
//! Mirrors the C++ `machbus::isobus::tc::TaskControllerClient`.
//! Pump-style:
//!
//! - [`TaskControllerClient::try_handle_tc_message`] feeds inbound
//!   `PGN_TC_TO_ECU` messages with explicit envelope validation errors.
//!   [`TaskControllerClient::handle_tc_message`] remains the compatibility
//!   wrapper that ignores malformed or unrelated traffic.
//! - [`TaskControllerClient::update`] advances the connect FSM and
//!   returns the outbound frames the caller should ship.

use alloc::{boxed::Box, format, vec, vec::Vec};

use super::ddop::DDOP;
use super::objects::{DDI, ElementNumber};
use super::server_options::{
    ProcessDataAcknowledgeErrorCodes, ProcessDataCommands, tc_options_byte_is_valid,
};
use crate::net::constants::NULL_ADDRESS;
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_ECU_TO_TC, PGN_TC_TO_ECU, PGN_WORKING_SET_MASTER};
use crate::net::state_machine::StateMachine;
use crate::net::types::{Address, Pgn};

/// ISO 11783 TC process-data element numbers are carried in the high nibble
/// of byte 0 plus byte 1, i.e. a 12-bit unsigned field.
pub const MAX_TC_PROCESS_DATA_ELEMENT_NUMBER: u16 = 0x0FFF;

/// Client task status byte used in the TC client status payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TCClientTaskStatus {
    #[default]
    Idle = 0x00,
    Active = 0x01,
    Paused = 0x02,
    Completed = 0x03,
}

impl TCClientTaskStatus {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Configuration for [`TaskControllerClient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TCClientConfig {
    pub timeout_ms: u32,
}

impl Default for TCClientConfig {
    fn default() -> Self {
        Self { timeout_ms: 6000 }
    }
}

impl TCClientConfig {
    #[must_use]
    pub const fn with_timeout(mut self, ms: u32) -> Self {
        self.timeout_ms = ms;
        self
    }
}

/// Capability payload advertised by a TC client when a TC asks for the
/// client's supported TC/SC/GEO options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TCClientCapabilities {
    pub version: u8,
    pub max_boot_time: u8,
    pub options: u8,
    pub booms: u8,
    pub sections: u8,
    pub channels: u8,
}

impl Default for TCClientCapabilities {
    fn default() -> Self {
        Self {
            version: 4,
            max_boot_time: 0xFF,
            options: 0,
            booms: 0,
            sections: 0,
            channels: 0,
        }
    }
}

impl TCClientCapabilities {
    #[must_use]
    pub const fn with_version(mut self, version: u8) -> Self {
        self.version = version;
        self
    }

    #[must_use]
    pub const fn with_max_boot_time(mut self, max_boot_time: u8) -> Self {
        self.max_boot_time = max_boot_time;
        self
    }

    #[must_use]
    pub const fn with_options(mut self, options: u8) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub const fn with_booms(mut self, booms: u8) -> Self {
        self.booms = booms;
        self
    }

    #[must_use]
    pub const fn with_sections(mut self, sections: u8) -> Self {
        self.sections = sections;
        self
    }

    #[must_use]
    pub const fn with_channels(mut self, channels: u8) -> Self {
        self.channels = channels;
        self
    }
}

/// TC client connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TCState {
    #[default]
    Disconnected,
    WaitForStartup,
    WaitForServerStatus,
    SendWorkingSetMaster,
    RequestVersion,
    WaitForVersion,
    ProcessDDOP,
    RequestStructureLabel,
    WaitForStructureLabel,
    RequestLocalizationLabel,
    WaitForLocalizationLabel,
    TransferDDOP,
    WaitForPoolResponse,
    ActivatePool,
    WaitForActivation,
    Connected,
    DeactivatePool,
    WaitForDeactivation,
    DeletePool,
    WaitForDeletePool,
}

/// TC command codes (ISO 11783-10).
pub mod tc_cmd {
    pub const VERSION_REQUEST: u8 = 0x00;
    pub const VERSION_RESPONSE: u8 = 0x10;
    pub const REQUEST_STRUCTURE_LABEL: u8 = 0x01;
    pub const STRUCTURE_LABEL: u8 = 0x11;
    pub const REQUEST_LOCALIZATION_LABEL: u8 = 0x21;
    pub const LOCALIZATION_LABEL: u8 = 0x31;
    pub const REQUEST_OBJECT_POOL: u8 = 0x41;
    pub const REQUEST_OBJECT_POOL_RESPONSE: u8 = 0x51;
    pub const OBJECT_POOL_TRANSFER: u8 = 0x61;
    pub const OBJECT_POOL_RESPONSE: u8 = 0x71;
    pub const ACTIVATE_POOL: u8 = 0x81;
    pub const ACTIVATE_RESPONSE: u8 = 0x91;
    pub const DELETE_POOL: u8 = 0xA1;
    pub const DELETE_POOL_RESPONSE: u8 = 0xB1;
    pub const REQUEST_TC_IDENTIFICATION: u8 = 0x20;
    pub const TC_STATUS: u8 = 0xFE;
}

fn has_ff_tail(data: &[u8], used: usize) -> bool {
    data[used..].iter().all(|&byte| byte == 0xFF)
}

fn is_padded_fixed8(data: &[u8], used: usize) -> bool {
    data.len() == 8 && has_ff_tail(data, used)
}

fn version_response_is_canonical(data: &[u8]) -> bool {
    data.len() == 8 && tc_options_byte_is_valid(data[3]) && data[4] == 0x00
}

pub type ValueCallback = Box<dyn FnMut(ElementNumber, DDI) -> Result<i32>>;
pub type CommandCallback = Box<dyn FnMut(ElementNumber, DDI, i32) -> Result<()>>;

/// One outbound frame from the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TCClientOutbound {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub dest: Option<Address>,
}

impl TCClientOutbound {
    #[must_use]
    pub fn broadcast(pgn: Pgn, data: Vec<u8>) -> Self {
        Self {
            pgn,
            data,
            dest: None,
        }
    }

    #[must_use]
    pub fn to(pgn: Pgn, data: Vec<u8>, dest: Address) -> Self {
        Self {
            pgn,
            data,
            dest: Some(dest),
        }
    }
}

/// ISO 11783-10 Task Controller client.
pub struct TaskControllerClient {
    config: TCClientConfig,
    state: StateMachine<TCState>,
    ddop: DDOP,
    timer_ms: u32,
    tc_address: Address,
    tc_version: u8,
    num_booms: u8,
    num_sections: u8,

    value_cb: Option<ValueCallback>,
    command_cb: Option<CommandCallback>,

    pub on_state_change: Event<TCState>,
}

impl TaskControllerClient {
    #[must_use]
    pub fn new(config: TCClientConfig) -> Self {
        Self {
            config,
            state: StateMachine::new(TCState::Disconnected),
            ddop: DDOP::default(),
            timer_ms: 0,
            tc_address: NULL_ADDRESS,
            tc_version: 0,
            num_booms: 0,
            num_sections: 0,
            value_cb: None,
            command_cb: None,
            on_state_change: Event::new(),
        }
    }

    pub fn set_ddop(&mut self, pool: DDOP) {
        self.ddop = pool;
    }

    #[must_use]
    pub fn ddop(&self) -> &DDOP {
        &self.ddop
    }

    /// Number of booms the connected TC reported supporting in its version
    /// response (0 until a version response has been received).
    #[must_use]
    pub const fn server_supported_booms(&self) -> u8 {
        self.num_booms
    }

    /// Number of sections the connected TC reported supporting.
    #[must_use]
    pub const fn server_supported_sections(&self) -> u8 {
        self.num_sections
    }

    /// `true` if `caps` fit within the server's reported boom/section limits.
    /// A reported limit of `0xFF` is treated as "no stated limit". Use this to
    /// clamp/validate a DDOP's boom and section counts against the TC before
    /// uploading (ISO 11783-10 capability negotiation).
    #[must_use]
    pub fn capabilities_within_server_limits(&self, caps: &TCClientCapabilities) -> bool {
        let fits = |want: u8, limit: u8| limit == 0xFF || want <= limit;
        fits(caps.booms, self.num_booms) && fits(caps.sections, self.num_sections)
    }

    pub fn connect(&mut self) -> Result<()> {
        self.ddop
            .validate()
            .map_err(|_| Error::invalid_state("DDOP validation failed"))?;
        self.tc_address = NULL_ADDRESS;
        self.tc_version = 0;
        self.transition(TCState::WaitForServerStatus);
        self.timer_ms = 0;
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.transition(TCState::Disconnected);
        Ok(())
    }

    /// Delete the currently active DDOP on the TC, upload `pool`, and
    /// reactivate it without rediscovering the server.
    pub fn reupload_ddop(&mut self, pool: DDOP) -> Result<()> {
        if self.state() != TCState::Connected {
            return Err(Error::invalid_state("TC client is not connected"));
        }
        pool.validate()
            .map_err(|_| Error::invalid_state("DDOP validation failed"))?;
        self.ddop = pool;
        self.transition(TCState::DeactivatePool);
        self.timer_ms = 0;
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> TCState {
        self.state.state()
    }

    #[inline]
    #[must_use]
    pub const fn tc_address(&self) -> Address {
        self.tc_address
    }

    #[inline]
    #[must_use]
    pub const fn tc_version(&self) -> u8 {
        self.tc_version
    }

    pub fn on_value_request<F>(&mut self, cb: F)
    where
        F: FnMut(ElementNumber, DDI) -> Result<i32> + 'static,
    {
        self.value_cb = Some(Box::new(cb));
    }

    pub fn on_value_command<F>(&mut self, cb: F)
    where
        F: FnMut(ElementNumber, DDI, i32) -> Result<()> + 'static,
    {
        self.command_cb = Some(Box::new(cb));
    }

    /// Build the Working Set Master payload sent by an ECU before TC
    /// negotiation. AgIsoStack's default client emits member-count `1` and
    /// reserves the remaining bytes as `0xFF`.
    #[must_use]
    pub fn build_working_set_master(member_count: u8) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = member_count;
        data
    }

    /// Build the fixed TC version-request payload (`00 FF FF FF FF FF FF FF`).
    #[must_use]
    pub fn build_version_request() -> [u8; 8] {
        fixed_mux_payload(tc_cmd::VERSION_REQUEST)
    }

    /// Build the TC-client status payload. The first four bytes are the
    /// process-data not-available sentinel (`FF FF FF FF`); bytes 4..6 carry
    /// status, command-address, and command code; byte 7 is reserved zero.
    #[must_use]
    pub fn build_status(
        status: TCClientTaskStatus,
        command_address: Address,
        command: u8,
    ) -> [u8; 8] {
        [
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            status.as_u8(),
            command_address,
            command,
            0x00,
        ]
    }

    /// Build a response to a TC version/capabilities request.
    #[must_use]
    pub fn build_request_version_response(capabilities: TCClientCapabilities) -> [u8; 8] {
        [
            tc_cmd::VERSION_RESPONSE,
            capabilities.version,
            capabilities.max_boot_time,
            capabilities.options,
            0x00,
            capabilities.booms,
            capabilities.sections,
            capabilities.channels,
        ]
    }

    /// Build the fixed request for a TC structure label.
    #[must_use]
    pub fn build_request_structure_label() -> [u8; 8] {
        fixed_mux_payload(tc_cmd::REQUEST_STRUCTURE_LABEL)
    }

    /// Build the fixed request for a TC localization label.
    #[must_use]
    pub fn build_request_localization_label() -> [u8; 8] {
        fixed_mux_payload(tc_cmd::REQUEST_LOCALIZATION_LABEL)
    }

    /// Build the fixed delete-object-pool request.
    #[must_use]
    pub fn build_delete_object_pool() -> [u8; 8] {
        fixed_mux_payload(tc_cmd::DELETE_POOL)
    }

    /// Build a process-data acknowledge payload, rejecting element numbers
    /// that cannot fit into the 12-bit ISO 11783 process-data element field.
    pub fn build_process_data_ack(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        error: ProcessDataAcknowledgeErrorCodes,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload(ProcessDataCommands::Acknowledge, element, ddi, None, error)
    }

    /// Build an ECU→TC process-data value payload.
    pub fn build_value_command(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        value: i32,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload(
            ProcessDataCommands::Value,
            element,
            ddi,
            Some(value),
            ProcessDataAcknowledgeErrorCodes::NoError,
        )
    }

    /// Build the fixed request for TC identification (`20 FF ... FF`).
    #[must_use]
    pub fn build_task_controller_identification_request() -> [u8; 8] {
        fixed_mux_payload(tc_cmd::REQUEST_TC_IDENTIFICATION)
    }

    /// Advance the FSM and return outbound frames to ship.
    pub fn update(&mut self, elapsed_ms: u32) -> Vec<TCClientOutbound> {
        self.timer_ms = self.timer_ms.saturating_add(elapsed_ms);
        let mut out = Vec::new();
        match self.state() {
            TCState::WaitForServerStatus
            | TCState::WaitForVersion
            | TCState::WaitForStructureLabel
            | TCState::WaitForLocalizationLabel
            | TCState::WaitForPoolResponse
            | TCState::WaitForActivation
            | TCState::WaitForDeactivation
            | TCState::WaitForDeletePool => {
                if self.timer_ms >= self.config.timeout_ms {
                    self.transition(TCState::Disconnected);
                }
            }
            TCState::SendWorkingSetMaster => {
                let data = Self::build_working_set_master(1);
                out.push(TCClientOutbound::broadcast(
                    PGN_WORKING_SET_MASTER,
                    data.to_vec(),
                ));
                self.transition(TCState::RequestVersion);
                self.timer_ms = 0;
            }
            TCState::RequestVersion => {
                let data = Self::build_version_request();
                out.push(TCClientOutbound::to(
                    PGN_ECU_TO_TC,
                    data.to_vec(),
                    self.tc_address,
                ));
                self.transition(TCState::WaitForVersion);
                self.timer_ms = 0;
            }
            TCState::RequestStructureLabel => {
                let data = Self::build_request_structure_label();
                out.push(TCClientOutbound::to(
                    PGN_ECU_TO_TC,
                    data.to_vec(),
                    self.tc_address,
                ));
                self.transition(TCState::WaitForStructureLabel);
                self.timer_ms = 0;
            }
            TCState::RequestLocalizationLabel => {
                let data = Self::build_request_localization_label();
                out.push(TCClientOutbound::to(
                    PGN_ECU_TO_TC,
                    data.to_vec(),
                    self.tc_address,
                ));
                self.transition(TCState::WaitForLocalizationLabel);
                self.timer_ms = 0;
            }
            TCState::TransferDDOP => {
                if let Ok(pool_data) = self.ddop.serialize() {
                    let mut data = Vec::with_capacity(pool_data.len() + 1);
                    data.push(tc_cmd::OBJECT_POOL_TRANSFER);
                    data.extend_from_slice(&pool_data);
                    out.push(TCClientOutbound::to(PGN_ECU_TO_TC, data, self.tc_address));
                    self.transition(TCState::WaitForPoolResponse);
                    self.timer_ms = 0;
                } else {
                    self.transition(TCState::Disconnected);
                }
            }
            TCState::ActivatePool => {
                let mut data = [0xFFu8; 8];
                data[0] = tc_cmd::ACTIVATE_POOL;
                out.push(TCClientOutbound::to(
                    PGN_ECU_TO_TC,
                    data.to_vec(),
                    self.tc_address,
                ));
                self.transition(TCState::WaitForActivation);
                self.timer_ms = 0;
            }
            TCState::DeactivatePool => {
                let mut data = [0xFFu8; 8];
                data[0] = tc_cmd::ACTIVATE_POOL;
                data[1] = 0x00;
                out.push(TCClientOutbound::to(
                    PGN_ECU_TO_TC,
                    data.to_vec(),
                    self.tc_address,
                ));
                self.transition(TCState::WaitForDeactivation);
                self.timer_ms = 0;
            }
            TCState::DeletePool => {
                let mut data = [0xFFu8; 8];
                data[0] = tc_cmd::DELETE_POOL;
                out.push(TCClientOutbound::to(
                    PGN_ECU_TO_TC,
                    data.to_vec(),
                    self.tc_address,
                ));
                self.transition(TCState::WaitForDeletePool);
                self.timer_ms = 0;
            }
            TCState::Connected => {
                // Loss of the TC's periodic status beyond the timeout means the
                // connection is gone; drop to Disconnected so the app reconnects.
                if self.timer_ms >= self.config.timeout_ms {
                    self.transition(TCState::Disconnected);
                }
            }
            _ => {}
        }
        out
    }

    /// Feed an inbound `PGN_TC_TO_ECU` message; returns any outbound
    /// frame produced by the message (e.g. response to a value
    /// request). Compatibility wrapper: malformed or unrelated frames are
    /// ignored. Use [`Self::try_handle_tc_message`] when caller-owned dispatch
    /// needs explicit validation errors.
    pub fn handle_tc_message(&mut self, msg: &Message) -> Vec<TCClientOutbound> {
        self.try_handle_tc_message(msg).unwrap_or_default()
    }

    /// Feed an inbound `PGN_TC_TO_ECU` message with explicit validation errors
    /// for wrong PGNs, invalid/bound source addresses, empty messages, and
    /// malformed fixed-size request frames.
    pub fn try_handle_tc_message(&mut self, msg: &Message) -> Result<Vec<TCClientOutbound>> {
        if msg.pgn != PGN_TC_TO_ECU {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        if !msg.has_usable_source() {
            return Err(Error::invalid_address(msg.source));
        }
        if !msg.has_valid_destination_for_pgn() {
            return Err(Error::invalid_address(msg.destination));
        }
        if self.tc_address != NULL_ADDRESS && msg.source != self.tc_address {
            return Err(Error::invalid_state(format!(
                "message from unbound task controller source 0x{:02X}",
                msg.source
            )));
        }
        if msg.data.is_empty() {
            return Err(Error::invalid_data("TC-to-ECU message must not be empty"));
        }
        if msg.data[0] == tc_cmd::VERSION_REQUEST {
            if !is_padded_fixed8(&msg.data, 1) {
                return Err(Error::invalid_data(
                    "TC version request must be an 8-byte padded frame",
                ));
            }
            let data = Self::build_request_version_response(TCClientCapabilities::default());
            return Ok(vec![TCClientOutbound::to(
                PGN_ECU_TO_TC,
                data.to_vec(),
                msg.source,
            )]);
        }
        if ProcessDataCommands::try_from_u8(msg.data[0]) == Some(ProcessDataCommands::Status)
            && msg.data[0] != tc_cmd::TC_STATUS
        {
            return Err(Error::invalid_data(
                "TC status must use the canonical fixed command byte",
            ));
        }
        if msg.data[0] == tc_cmd::TC_STATUS && msg.data.len() != 8 {
            return Err(Error::invalid_data(
                "TC status must be an 8-byte fixed frame",
            ));
        }
        Ok(match (self.state(), msg.data[0]) {
            (_, tc_cmd::TC_STATUS) => {
                self.handle_tc_status(msg);
                Vec::new()
            }
            (TCState::WaitForVersion, tc_cmd::VERSION_RESPONSE) => {
                if !version_response_is_canonical(&msg.data) {
                    return Err(Error::invalid_data(
                        "TC version response must be an 8-byte canonical frame",
                    ));
                }
                self.handle_version_response(msg);
                Vec::new()
            }
            (TCState::WaitForStructureLabel, tc_cmd::STRUCTURE_LABEL) => {
                if msg.data.len() != 8 {
                    return Err(Error::invalid_data(
                        "TC structure-label response must be an 8-byte frame",
                    ));
                }
                self.handle_structure_label_response(msg);
                Vec::new()
            }
            (TCState::WaitForLocalizationLabel, tc_cmd::LOCALIZATION_LABEL) => {
                if msg.data.len() != 8 {
                    return Err(Error::invalid_data(
                        "TC localization-label response must be an 8-byte frame",
                    ));
                }
                self.handle_localization_label_response(msg);
                Vec::new()
            }
            (TCState::WaitForPoolResponse, tc_cmd::OBJECT_POOL_RESPONSE) => {
                if !is_padded_fixed8(&msg.data, 2) {
                    return Err(Error::invalid_data(
                        "TC object-pool response must be an 8-byte padded frame",
                    ));
                }
                self.handle_pool_response(msg);
                Vec::new()
            }
            (TCState::WaitForActivation, tc_cmd::ACTIVATE_RESPONSE) => {
                if !is_padded_fixed8(&msg.data, 2) {
                    return Err(Error::invalid_data(
                        "TC activate-pool response must be an 8-byte padded frame",
                    ));
                }
                self.handle_activate_response(msg);
                Vec::new()
            }
            (TCState::WaitForDeactivation, tc_cmd::ACTIVATE_RESPONSE) => {
                if !is_padded_fixed8(&msg.data, 2) {
                    return Err(Error::invalid_data(
                        "TC deactivate-pool response must be an 8-byte padded frame",
                    ));
                }
                self.handle_deactivate_response(msg);
                Vec::new()
            }
            (TCState::WaitForDeletePool, tc_cmd::DELETE_POOL_RESPONSE) => {
                if !is_padded_fixed8(&msg.data, 2) {
                    return Err(Error::invalid_data(
                        "TC delete-pool response must be an 8-byte padded frame",
                    ));
                }
                self.handle_delete_pool_response(msg);
                Vec::new()
            }
            _ => match ProcessDataCommands::try_from_u8(msg.data[0]) {
                None => {
                    return Err(Error::invalid_data(
                        "TC-to-ECU message has reserved process-data command nibble",
                    ));
                }
                Some(ProcessDataCommands::Status) => {
                    self.handle_tc_status(msg);
                    Vec::new()
                }
                Some(ProcessDataCommands::RequestValue) => {
                    if !is_padded_fixed8(&msg.data, 4) {
                        return Err(Error::invalid_data(
                            "TC request-value process-data frame must be an 8-byte padded frame",
                        ));
                    }
                    self.handle_value_request(msg)
                }
                Some(ProcessDataCommands::Value) => {
                    if msg.data.len() != 8 {
                        return Err(Error::invalid_data(
                            "TC value process-data frame must be an 8-byte frame",
                        ));
                    }
                    self.handle_value_command(msg, false)
                }
                Some(ProcessDataCommands::SetValueAndAcknowledge) => {
                    if msg.data.len() != 8 {
                        return Err(Error::invalid_data(
                            "TC set-value-and-acknowledge frame must be an 8-byte frame",
                        ));
                    }
                    self.handle_value_command(msg, true)
                }
                Some(command) => {
                    return Err(Error::invalid_state(format!(
                        "unsupported TC-to-ECU process-data command {:?}",
                        command
                    )));
                }
            },
        })
    }

    fn handle_deactivate_response(&mut self, msg: &Message) {
        if !is_padded_fixed8(&msg.data, 2) {
            return;
        }
        if msg.data[1] == 0 {
            self.transition(TCState::DeletePool);
        } else {
            self.transition(TCState::Disconnected);
        }
        self.timer_ms = 0;
    }

    fn handle_delete_pool_response(&mut self, msg: &Message) {
        if !is_padded_fixed8(&msg.data, 2) {
            return;
        }
        self.transition(TCState::TransferDDOP);
        self.timer_ms = 0;
    }

    fn handle_tc_status(&mut self, msg: &Message) {
        self.tc_address = msg.source;
        match self.state() {
            TCState::WaitForServerStatus => {
                self.transition(TCState::SendWorkingSetMaster);
                self.timer_ms = 0;
            }
            // The TC's periodic status is the connection keepalive — reset the
            // connection-loss watchdog.
            TCState::Connected => self.timer_ms = 0,
            _ => {}
        }
    }

    fn handle_version_response(&mut self, msg: &Message) {
        if !version_response_is_canonical(&msg.data) {
            return;
        }
        self.tc_version = msg.data[1];
        self.num_booms = msg.data[5];
        self.num_sections = msg.data[6];
        self.transition(TCState::RequestStructureLabel);
        self.timer_ms = 0;
    }

    fn ddop_structure_label(&self) -> Option<[u8; 7]> {
        self.ddop
            .devices()
            .first()
            .map(|device| device.structure_label)
    }

    fn ddop_localization_label(&self) -> Option<[u8; 7]> {
        self.ddop
            .devices()
            .first()
            .map(|device| device.localization_label)
    }

    fn handle_structure_label_response(&mut self, msg: &Message) {
        if msg.data.len() != 8 {
            return;
        }
        let mut label = [0u8; 7];
        label.copy_from_slice(&msg.data[1..8]);
        if label == [0xFF; 7] {
            self.transition(TCState::TransferDDOP);
        } else if Some(label) == self.ddop_structure_label() {
            self.transition(TCState::RequestLocalizationLabel);
        } else {
            self.transition(TCState::DeletePool);
        }
        self.timer_ms = 0;
    }

    fn handle_localization_label_response(&mut self, msg: &Message) {
        if msg.data.len() != 8 {
            return;
        }
        let mut label = [0u8; 7];
        label.copy_from_slice(&msg.data[1..8]);
        if label == [0xFF; 7] {
            self.transition(TCState::TransferDDOP);
        } else if Some(label) == self.ddop_localization_label() {
            self.transition(TCState::ActivatePool);
        } else {
            self.transition(TCState::DeletePool);
        }
        self.timer_ms = 0;
    }

    fn handle_pool_response(&mut self, msg: &Message) {
        if !is_padded_fixed8(&msg.data, 2) {
            return;
        }
        if msg.data[1] == 0 {
            self.transition(TCState::ActivatePool);
        } else {
            self.transition(TCState::Disconnected);
        }
        self.timer_ms = 0;
    }

    fn handle_activate_response(&mut self, msg: &Message) {
        if !is_padded_fixed8(&msg.data, 2) {
            return;
        }
        if msg.data[1] == 0 {
            self.transition(TCState::Connected);
            // Start the connection-loss watchdog fresh on connect.
            self.timer_ms = 0;
        } else {
            self.transition(TCState::Disconnected);
        }
    }

    fn handle_value_request(&mut self, msg: &Message) -> Vec<TCClientOutbound> {
        if !is_padded_fixed8(&msg.data, 4) {
            return Vec::new();
        }
        let elem_raw = ((msg.data[0] >> 4) & 0x0F) as u16 | ((msg.data[1] as u16) << 4);
        let ddi_raw = (msg.data[2] as u16) | ((msg.data[3] as u16) << 8);
        let elem = ElementNumber(elem_raw);
        let ddi = DDI(ddi_raw);
        let Some(cb) = self.value_cb.as_mut() else {
            return Vec::new();
        };
        let Ok(value) = cb(elem, ddi) else {
            return Vec::new();
        };
        let mut data = [0xFFu8; 8];
        data[0] = (ProcessDataCommands::Value.as_u8() & 0x0F) | ((elem_raw as u8 & 0x0F) << 4);
        data[1] = ((elem_raw >> 4) & 0xFF) as u8;
        data[2] = (ddi_raw & 0xFF) as u8;
        data[3] = ((ddi_raw >> 8) & 0xFF) as u8;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        vec![TCClientOutbound::to(
            PGN_ECU_TO_TC,
            data.to_vec(),
            msg.source,
        )]
    }

    fn handle_value_command(&mut self, msg: &Message, acknowledge: bool) -> Vec<TCClientOutbound> {
        if msg.data.len() != 8 {
            return Vec::new();
        }
        let elem_raw = ((msg.data[0] >> 4) & 0x0F) as u16 | ((msg.data[1] as u16) << 4);
        let ddi_raw = (msg.data[2] as u16) | ((msg.data[3] as u16) << 8);
        let elem = ElementNumber(elem_raw);
        let ddi = DDI(ddi_raw);
        let value = i32::from_le_bytes(msg.data[4..8].try_into().unwrap());
        let error = match self.command_cb.as_mut() {
            Some(cb) => {
                if cb(elem, ddi, value).is_ok() {
                    ProcessDataAcknowledgeErrorCodes::NoError
                } else {
                    ProcessDataAcknowledgeErrorCodes::NoProcessingResourcesAvailable
                }
            }
            None => ProcessDataAcknowledgeErrorCodes::ElementNotSupportedByThisDevice,
        };
        if acknowledge {
            let data = encode_process_data_ack(elem_raw, ddi_raw, error);
            vec![TCClientOutbound::to(
                PGN_ECU_TO_TC,
                data.to_vec(),
                msg.source,
            )]
        } else {
            Vec::new()
        }
    }

    fn transition(&mut self, new_state: TCState) {
        if self.state() == new_state {
            return;
        }
        self.state.transition(new_state);
        self.on_state_change.emit(&new_state);
    }
}

fn fixed_mux_payload(mux: u8) -> [u8; 8] {
    let mut data = [0xFFu8; 8];
    data[0] = mux;
    data
}

fn encode_process_data_ack(
    elem_raw: u16,
    ddi_raw: u16,
    error: ProcessDataAcknowledgeErrorCodes,
) -> [u8; 8] {
    let mut data = [0xFFu8; 8];
    data[0] = (ProcessDataCommands::Acknowledge.as_u8() & 0x0F) | ((elem_raw as u8 & 0x0F) << 4);
    data[1] = ((elem_raw >> 4) & 0xFF) as u8;
    data[2] = (ddi_raw & 0xFF) as u8;
    data[3] = ((ddi_raw >> 8) & 0xFF) as u8;
    data[4] = error.as_u8();
    data
}

fn encode_process_data_payload(
    command: ProcessDataCommands,
    element: u16,
    ddi: u16,
    value: Option<i32>,
    error: ProcessDataAcknowledgeErrorCodes,
) -> Result<[u8; 8]> {
    if element > MAX_TC_PROCESS_DATA_ELEMENT_NUMBER {
        return Err(Error::invalid_data(format!(
            "TC process-data element {} exceeds 12-bit wire maximum {}",
            element, MAX_TC_PROCESS_DATA_ELEMENT_NUMBER
        )));
    }
    let mut data = [0xFFu8; 8];
    data[0] = (command.as_u8() & 0x0F) | ((element as u8 & 0x0F) << 4);
    data[1] = ((element >> 4) & 0xFF) as u8;
    data[2] = (ddi & 0xFF) as u8;
    data[3] = ((ddi >> 8) & 0xFF) as u8;
    if let Some(value) = value {
        data[4..8].copy_from_slice(&value.to_le_bytes());
    } else if command == ProcessDataCommands::Acknowledge {
        data[4] = error.as_u8();
    }
    Ok(data)
}

impl Default for TaskControllerClient {
    fn default() -> Self {
        Self::new(TCClientConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::super::objects::{DeviceElement, DeviceElementType, DeviceObject};
    use super::super::server::TaskControllerServer;
    use super::*;
    use crate::net::constants::BROADCAST_ADDRESS;
    use crate::net::pgn_defs::PGN_TC_TO_ECU;

    fn dummy_ddop() -> DDOP {
        DDOP::default()
            .with_device(DeviceObject::default().with_id(1).with_designator("D"))
            .with_element(
                DeviceElement::default()
                    .with_id(2)
                    .with_type(DeviceElementType::Device),
            )
    }

    fn named_ddop(name: &str) -> DDOP {
        DDOP::default()
            .with_device(DeviceObject::default().with_id(1).with_designator(name))
            .with_element(
                DeviceElement::default()
                    .with_id(2)
                    .with_type(DeviceElementType::Device),
            )
    }

    fn tc_msg(data: Vec<u8>, src: Address) -> Message {
        Message::new(PGN_TC_TO_ECU, data, src)
    }

    fn tc_fixed_response(command: u8, status: u8) -> Vec<u8> {
        let mut data = [0xFFu8; 8];
        data[0] = command;
        data[1] = status;
        data.to_vec()
    }

    fn version_response(version: u8, booms: u8, sections: u8, channels: u8) -> Vec<u8> {
        TaskControllerClient::build_request_version_response(
            TCClientCapabilities::default()
                .with_version(version)
                .with_booms(booms)
                .with_sections(sections)
                .with_channels(channels),
        )
        .to_vec()
    }

    fn accept_no_existing_structure_label(c: &mut TaskControllerClient) {
        assert_eq!(c.state(), TCState::RequestStructureLabel);
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].data,
            TaskControllerClient::build_request_structure_label()
        );
        assert_eq!(c.state(), TCState::WaitForStructureLabel);
        c.handle_tc_message(&tc_msg(
            [
                tc_cmd::STRUCTURE_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ]
            .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::TransferDDOP);
    }

    fn connected_client() -> TaskControllerClient {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();
        c.handle_tc_message(&tc_msg(vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0], 0x33));
        c.update(1); // Working Set Master.
        c.update(1); // Version request.
        c.handle_tc_message(&tc_msg(version_response(4, 1, 4, 0), 0x33));
        accept_no_existing_structure_label(&mut c); // → TransferDDOP
        c.update(1); // OBJECT_POOL_TRANSFER → WaitForPoolResponse
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::OBJECT_POOL_RESPONSE, 0),
            0x33,
        ));
        c.update(1); // ACTIVATE_POOL → WaitForActivation
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::Connected);
        c
    }

    #[test]
    fn connection_loss_watchdog_disconnects_without_tc_status() {
        let timeout = TCClientConfig::default().timeout_ms;
        let mut c = connected_client();

        // Half the timeout elapses, then a TC status keepalive arrives.
        c.update(timeout / 2);
        assert_eq!(c.state(), TCState::Connected);
        c.handle_tc_message(&tc_msg(vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0], 0x33));
        c.update(timeout / 2);
        assert_eq!(
            c.state(),
            TCState::Connected,
            "keepalive must hold the connection"
        );

        // No TC status for a full timeout ⇒ connection lost.
        c.update(timeout);
        assert_eq!(c.state(), TCState::Disconnected);
    }

    #[test]
    fn device_descriptor_command_bytes_are_canonical_mux_values() {
        assert_eq!(tc_cmd::REQUEST_STRUCTURE_LABEL, 0x01);
        assert_eq!(tc_cmd::STRUCTURE_LABEL, 0x11);
        assert_eq!(tc_cmd::REQUEST_LOCALIZATION_LABEL, 0x21);
        assert_eq!(tc_cmd::LOCALIZATION_LABEL, 0x31);
        assert_eq!(tc_cmd::REQUEST_OBJECT_POOL, 0x41);
        assert_eq!(tc_cmd::REQUEST_OBJECT_POOL_RESPONSE, 0x51);
        assert_eq!(tc_cmd::OBJECT_POOL_TRANSFER, 0x61);
        assert_eq!(tc_cmd::OBJECT_POOL_RESPONSE, 0x71);
        assert_eq!(tc_cmd::ACTIVATE_POOL, 0x81);
        assert_eq!(tc_cmd::ACTIVATE_RESPONSE, 0x91);
        assert_eq!(tc_cmd::DELETE_POOL, 0xA1);
        assert_eq!(tc_cmd::DELETE_POOL_RESPONSE, 0xB1);
    }

    #[test]
    fn connect_requires_valid_ddop() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        // Empty DDOP fails validation.
        assert!(c.connect().is_err());
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();
        assert_eq!(c.state(), TCState::WaitForServerStatus);
    }

    #[test]
    fn full_connect_flow() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();

        // 1. TC status arrives ⇒ state = SendWorkingSetMaster.
        c.handle_tc_message(&tc_msg(vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0], 0x33));
        assert_eq!(c.state(), TCState::SendWorkingSetMaster);
        assert_eq!(c.tc_address(), 0x33);

        // 2. update() emits Working Set Master frame, transitions to RequestVersion.
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pgn, PGN_WORKING_SET_MASTER);

        // 3. update() emits VERSION_REQUEST.
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::VERSION_REQUEST);
        assert_eq!(c.state(), TCState::WaitForVersion);

        // 4. canonical version/technical-capabilities response ⇒ request the
        // TC's stored structure label before deciding whether to upload.
        c.handle_tc_message(&tc_msg(version_response(4, 1, 4, 0), 0x33));
        assert_eq!(c.state(), TCState::RequestStructureLabel);
        assert_eq!(c.tc_version(), 4);

        // 5. update() emits structure-label request; all-FF label means no
        // matching DDOP exists on the TC, so the client uploads.
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].data,
            TaskControllerClient::build_request_structure_label()
        );
        assert_eq!(c.state(), TCState::WaitForStructureLabel);
        c.handle_tc_message(&tc_msg(
            [
                tc_cmd::STRUCTURE_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ]
            .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::TransferDDOP);

        // 6. update() emits DDOP, transitions to WaitForPoolResponse.
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::OBJECT_POOL_TRANSFER);
        assert!(out[0].data.len() > 8); // command + serialized DDOP, multi-frame
        DDOP::deserialize(&out[0].data[1..])
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(c.state(), TCState::WaitForPoolResponse);

        // 7. POOL_RESPONSE success ⇒ ActivatePool.
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::OBJECT_POOL_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::ActivatePool);

        // 8. update() emits ACTIVATE_POOL.
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::ACTIVATE_POOL);
        assert_eq!(c.state(), TCState::WaitForActivation);

        // 9. ACTIVATE_RESPONSE success ⇒ Connected.
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::Connected);
    }

    #[test]
    fn server_reported_boom_section_limits_are_exposed_and_checkable() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();
        c.handle_tc_message(&tc_msg(vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0], 0x33));
        c.update(1); // Working Set Master.
        c.update(1); // Version request.
        // Server reports it supports 2 booms and 6 sections.
        c.handle_tc_message(&tc_msg(version_response(4, 2, 6, 0), 0x33));

        assert_eq!(c.server_supported_booms(), 2);
        assert_eq!(c.server_supported_sections(), 6);

        // A DDOP within the limits fits; one exceeding either does not.
        let within = TCClientCapabilities::default()
            .with_booms(2)
            .with_sections(6);
        let too_many_sections = TCClientCapabilities::default()
            .with_booms(1)
            .with_sections(7);
        let too_many_booms = TCClientCapabilities::default()
            .with_booms(3)
            .with_sections(1);
        assert!(c.capabilities_within_server_limits(&within));
        assert!(!c.capabilities_within_server_limits(&too_many_sections));
        assert!(!c.capabilities_within_server_limits(&too_many_booms));
    }

    #[test]
    fn malformed_fixed_size_handshake_responses_are_ignored() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();
        c.handle_tc_message(&tc_msg(vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0], 0x33));
        c.update(1); // Working Set Master.
        c.update(1); // Version request.
        assert_eq!(c.state(), TCState::WaitForVersion);

        c.handle_tc_message(&tc_msg(vec![tc_cmd::VERSION_RESPONSE, 4, 0xFF, 0, 0], 0x33));
        assert_eq!(c.state(), TCState::WaitForVersion);
        let mut bad_version = version_response(4, 1, 4, 0);
        bad_version[4] = 0xFF;
        c.handle_tc_message(&tc_msg(bad_version, 0x33));
        assert_eq!(c.state(), TCState::WaitForVersion);

        c.handle_tc_message(&tc_msg(version_response(4, 1, 4, 0), 0x33));
        assert_eq!(c.state(), TCState::RequestStructureLabel);
        c.update(1);
        assert_eq!(c.state(), TCState::WaitForStructureLabel);
        c.handle_tc_message(&tc_msg(vec![tc_cmd::STRUCTURE_LABEL], 0x33));
        assert_eq!(c.state(), TCState::WaitForStructureLabel);
        c.handle_tc_message(&tc_msg(
            [
                tc_cmd::STRUCTURE_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ]
            .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::TransferDDOP);
        c.update(1);
        assert_eq!(c.state(), TCState::WaitForPoolResponse);

        c.handle_tc_message(&tc_msg(vec![tc_cmd::OBJECT_POOL_RESPONSE, 0], 0x33));
        assert_eq!(c.state(), TCState::WaitForPoolResponse);
        let mut bad_pool = tc_fixed_response(tc_cmd::OBJECT_POOL_RESPONSE, 0);
        bad_pool[2] = 0x00;
        c.handle_tc_message(&tc_msg(bad_pool, 0x33));
        assert_eq!(c.state(), TCState::WaitForPoolResponse);

        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::OBJECT_POOL_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::ActivatePool);
        c.update(1);
        assert_eq!(c.state(), TCState::WaitForActivation);

        c.handle_tc_message(&tc_msg(vec![tc_cmd::ACTIVATE_RESPONSE, 0], 0x33));
        assert_eq!(c.state(), TCState::WaitForActivation);
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::Connected);

        c.reupload_ddop(named_ddop("D2")).unwrap();
        c.update(1);
        assert_eq!(c.state(), TCState::WaitForDeactivation);
        c.handle_tc_message(&tc_msg(vec![tc_cmd::ACTIVATE_RESPONSE, 0], 0x33));
        assert_eq!(c.state(), TCState::WaitForDeactivation);
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::DeletePool);

        c.update(1);
        assert_eq!(c.state(), TCState::WaitForDeletePool);
        c.handle_tc_message(&tc_msg(vec![tc_cmd::DELETE_POOL_RESPONSE, 0xFF], 0x33));
        assert_eq!(c.state(), TCState::WaitForDeletePool);
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::DELETE_POOL_RESPONSE, 0xFF),
            0x33,
        ));
        assert_eq!(c.state(), TCState::TransferDDOP);
    }

    #[test]
    fn inbound_messages_require_tc_to_ecu_pgn_valid_source_and_bound_tc() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();
        let status = vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0];

        let mut wrong_pgn = Message::new(PGN_ECU_TO_TC, status.clone(), 0x33);
        assert!(c.handle_tc_message(&wrong_pgn).is_empty());
        assert_eq!(c.state(), TCState::WaitForServerStatus);

        for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            assert!(
                c.handle_tc_message(&tc_msg(status.clone(), bad_source))
                    .is_empty()
            );
            assert_eq!(c.state(), TCState::WaitForServerStatus);
        }

        c.handle_tc_message(&tc_msg(status, 0x33));
        assert_eq!(c.state(), TCState::SendWorkingSetMaster);
        assert_eq!(c.tc_address(), 0x33);
        c.update(1);
        c.update(1);
        assert_eq!(c.state(), TCState::WaitForVersion);

        wrong_pgn = Message::new(PGN_TC_TO_ECU, version_response(4, 1, 4, 0), 0x34);
        assert!(c.handle_tc_message(&wrong_pgn).is_empty());
        assert_eq!(
            c.state(),
            TCState::WaitForVersion,
            "response from a different TC source must not drive the bound handshake"
        );
    }

    #[test]
    fn try_handle_tc_message_reports_envelope_errors_without_state_mutation() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();
        let status = vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0];

        let err = c
            .try_handle_tc_message(&Message::new(PGN_ECU_TO_TC, status.clone(), 0x33))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidPgn);
        assert_eq!(c.state(), TCState::WaitForServerStatus);

        let err = c
            .try_handle_tc_message(&tc_msg(status.clone(), BROADCAST_ADDRESS))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidAddress);
        assert_eq!(c.state(), TCState::WaitForServerStatus);

        c.try_handle_tc_message(&tc_msg(status, 0x33)).unwrap();
        assert_eq!(c.state(), TCState::SendWorkingSetMaster);
        assert_eq!(c.tc_address(), 0x33);
        c.update(1);
        c.update(1);
        assert_eq!(c.state(), TCState::WaitForVersion);

        let err = c
            .try_handle_tc_message(&tc_msg(version_response(4, 1, 4, 0), 0x34))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidState);
        assert_eq!(c.state(), TCState::WaitForVersion);

        let err = c
            .try_handle_tc_message(&tc_msg(Vec::new(), 0x33))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);

        let err = c
            .try_handle_tc_message(&tc_msg(vec![tc_cmd::VERSION_REQUEST, 0x00], 0x33))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
    }

    #[test]
    fn timeout_disconnects_in_wait_for_server_status() {
        let mut c = TaskControllerClient::new(TCClientConfig::default().with_timeout(100));
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();
        c.update(50);
        assert_eq!(c.state(), TCState::WaitForServerStatus);
        c.update(60);
        assert_eq!(c.state(), TCState::Disconnected);
    }

    #[test]
    fn connected_reupload_deactivates_deletes_uploads_and_reactivates() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(named_ddop("D1"));
        c.connect().unwrap();
        c.handle_tc_message(&tc_msg(vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0], 0x33));
        c.update(1); // working set master
        c.update(1); // version request
        c.handle_tc_message(&tc_msg(version_response(4, 1, 4, 0), 0x33));
        accept_no_existing_structure_label(&mut c);
        c.update(1); // DDOP upload
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::OBJECT_POOL_RESPONSE, 0),
            0x33,
        ));
        c.update(1); // activate
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::Connected);

        c.reupload_ddop(named_ddop("D2")).unwrap();
        assert_eq!(c.state(), TCState::DeactivatePool);

        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].data,
            vec![
                tc_cmd::ACTIVATE_POOL,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );
        assert_eq!(c.state(), TCState::WaitForDeactivation);

        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::DeletePool);
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].data,
            vec![
                tc_cmd::DELETE_POOL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );
        assert_eq!(c.state(), TCState::WaitForDeletePool);

        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::DELETE_POOL_RESPONSE, 0xFF),
            0x33,
        ));
        assert_eq!(c.state(), TCState::TransferDDOP);
        let out = c.update(1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::OBJECT_POOL_TRANSFER);
        let uploaded = DDOP::deserialize(&out[0].data[1..]).unwrap();
        assert_eq!(uploaded.devices()[0].designator, "D2");
        assert_eq!(c.state(), TCState::WaitForPoolResponse);

        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::OBJECT_POOL_RESPONSE, 0),
            0x33,
        ));
        c.update(1);
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::Connected);
    }

    #[test]
    fn value_request_callback_responds() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<(ElementNumber, DDI)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.on_value_request(move |elem, ddi| {
            lc.borrow_mut().push((elem, ddi));
            Ok(99)
        });
        let req = TaskControllerServer::build_request_value(3, 0x1234).unwrap();
        let out = c.handle_tc_message(&tc_msg(req.to_vec(), 0x33));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dest, Some(0x33));
        assert_eq!(out[0].data[0] & 0x0F, ProcessDataCommands::Value.as_u8());
        assert_eq!(out[0].data[0] >> 4, 3);
        let v = i32::from_le_bytes(out[0].data[4..8].try_into().unwrap());
        assert_eq!(v, 99);
        assert_eq!(*log.borrow(), vec![(ElementNumber(3), DDI(0x1234))]);
    }

    #[test]
    fn version_request_responds_with_client_capabilities() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        let out = c.handle_tc_message(&tc_msg(
            TaskControllerClient::build_version_request().to_vec(),
            0x33,
        ));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pgn, PGN_ECU_TO_TC);
        assert_eq!(out[0].dest, Some(0x33));
        assert_eq!(
            out[0].data,
            TaskControllerClient::build_request_version_response(TCClientCapabilities::default())
                .to_vec()
        );

        let malformed = c.handle_tc_message(&tc_msg(vec![tc_cmd::VERSION_REQUEST], 0x33));
        assert!(malformed.is_empty());
    }

    #[test]
    fn structure_and_localization_labels_choose_upload_delete_or_activate() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(
            DDOP::default()
                .with_device(
                    DeviceObject::default()
                        .with_id(1)
                        .with_designator("D")
                        .with_structure_label(*b"I++1.0 ")
                        .with_localization_label([1, 0, 0, 0, 0, 0, 0]),
                )
                .with_element(
                    DeviceElement::default()
                        .with_id(2)
                        .with_type(DeviceElementType::Device),
                ),
        );

        c.connect().unwrap();
        c.handle_tc_message(&tc_msg(vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0], 0x33));
        c.update(1);
        c.update(1);
        c.handle_tc_message(&tc_msg(version_response(4, 1, 4, 0), 0x33));
        assert_eq!(c.state(), TCState::RequestStructureLabel);
        let out = c.update(1);
        assert_eq!(
            out[0].data,
            TaskControllerClient::build_request_structure_label()
        );
        assert_eq!(c.state(), TCState::WaitForStructureLabel);

        c.handle_tc_message(&tc_msg(
            [
                tc_cmd::STRUCTURE_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ]
            .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::TransferDDOP);

        c.transition(TCState::WaitForStructureLabel);
        c.handle_tc_message(&tc_msg(
            [
                tc_cmd::STRUCTURE_LABEL,
                4,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ]
            .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::DeletePool);

        c.transition(TCState::WaitForStructureLabel);
        c.handle_tc_message(&tc_msg(
            [
                tc_cmd::STRUCTURE_LABEL,
                b'I',
                b'+',
                b'+',
                b'1',
                b'.',
                b'0',
                b' ',
            ]
            .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::RequestLocalizationLabel);
        let out = c.update(1);
        assert_eq!(
            out[0].data,
            TaskControllerClient::build_request_localization_label()
        );
        assert_eq!(c.state(), TCState::WaitForLocalizationLabel);

        c.handle_tc_message(&tc_msg(
            [tc_cmd::LOCALIZATION_LABEL, 1, 0, 0, 0, 0, 0, 0].to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::ActivatePool);

        c.transition(TCState::WaitForLocalizationLabel);
        c.handle_tc_message(&tc_msg(
            [
                tc_cmd::LOCALIZATION_LABEL,
                1,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ]
            .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::DeletePool);
    }

    #[test]
    fn value_request_callback_error_is_silent() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.on_value_request(|_, _| Err(Error::invalid_state("sensor offline")));
        let req = TaskControllerServer::build_request_value(3, 0x1234).unwrap();
        let out = c.handle_tc_message(&tc_msg(req.to_vec(), 0x33));
        assert!(out.is_empty());
    }

    #[test]
    fn value_command_callback_receives_value() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<(ElementNumber, DDI, i32)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.on_value_command(move |e, d, v| {
            lc.borrow_mut().push((e, d, v));
            Ok(())
        });
        let data = TaskControllerServer::build_set_value(5, 0xABCD, 42).unwrap();
        c.handle_tc_message(&tc_msg(data.to_vec(), 0x33));
        assert_eq!(*log.borrow(), vec![(ElementNumber(5), DDI(0xABCD), 42i32)]);
    }

    #[test]
    fn set_value_and_acknowledge_callback_error_returns_ack() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.on_value_command(|_, _, _| Err(Error::invalid_state("actuator busy")));
        let data = TaskControllerServer::build_set_value_and_acknowledge(5, 0xCAFE, 42).unwrap();
        let out = c.handle_tc_message(&tc_msg(data.to_vec(), 0x33));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dest, Some(0x33));
        assert_eq!(
            out[0].data[0] & 0x0F,
            ProcessDataCommands::Acknowledge.as_u8()
        );
        assert_eq!(out[0].data[0] >> 4, 5);
        assert_eq!(
            out[0].data[4],
            ProcessDataAcknowledgeErrorCodes::NoProcessingResourcesAvailable.as_u8()
        );
    }

    #[test]
    fn connected_process_data_is_not_misread_as_handshake_response() {
        let mut c = TaskControllerClient::new(TCClientConfig::default());
        c.set_ddop(dummy_ddop());
        c.connect().unwrap();
        c.handle_tc_message(&tc_msg(vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0], 0x33));
        c.update(1);
        c.update(1);
        c.handle_tc_message(&tc_msg(version_response(4, 1, 4, 0), 0x33));
        accept_no_existing_structure_label(&mut c);
        c.update(1);
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::OBJECT_POOL_RESPONSE, 0),
            0x33,
        ));
        c.update(1);
        c.handle_tc_message(&tc_msg(
            tc_fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
            0x33,
        ));
        assert_eq!(c.state(), TCState::Connected);

        // A RequestValue on element 1 used to collide with the translated
        // object-pool response byte. Keep it as process-data in Connected.
        c.on_value_request(|_, _| Ok(7));
        let out = c.handle_tc_message(&tc_msg(
            TaskControllerServer::build_request_value(1, 0x1234)
                .unwrap()
                .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::Connected);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0] & 0x0F, ProcessDataCommands::Value.as_u8());

        // Likewise, a Value on element 2 used to collide with the translated
        // activate-response byte.
        c.on_value_command(|_, _, _| Ok(()));
        let out = c.handle_tc_message(&tc_msg(
            TaskControllerServer::build_set_value(2, 0x1234, 9)
                .unwrap()
                .to_vec(),
            0x33,
        ));
        assert_eq!(c.state(), TCState::Connected);
        assert!(out.is_empty());
    }
}
