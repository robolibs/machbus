use alloc::{boxed::Box, format, vec, vec::Vec};
use core::num::NonZeroU32;

use super::client::tc_cmd;
use super::ddop::DDOP;
use super::objects::{DDI, ElementNumber};
use super::peer_control::PeerControlAssignment;
use super::server_options::{
    ObjectPoolActivationError, ObjectPoolDeletionErrors, ObjectPoolErrorCodes,
    ProcessDataAcknowledgeErrorCodes, ProcessDataCommands, TCServerState, tc_options_byte_is_valid,
};
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_ECU_TO_TC, PGN_TC_TO_ECU, PGN_WORKING_SET_MASTER};
use crate::net::state_machine::StateMachine;
use crate::net::types::{Address, Pgn};

/// Periodic TC Status cadence.
pub const TC_STATUS_INTERVAL_MS: u32 = 2000;

/// A client is considered lost if no message is seen from it within this many
/// milliseconds (three TC-status cadences).
pub const TC_CLIENT_TIMEOUT_MS: u32 = 3 * TC_STATUS_INTERVAL_MS;

/// ISO 11783 TC status/capability fields encode section count in one byte,
/// with `0xFF` reserved as not-available. Server construction therefore
/// only accepts representable real section counts.
pub const MAX_TC_SERVER_SECTIONS: u8 = 254;
/// TC channel count is also a one-byte advertised capability. Public
/// AgIsoStack++ examples treat it as an opaque count, not a section index, so
/// machbus preserves all `u8` values here and does not infer additional
/// section/channel topology constraints without an official-spec citation.
pub const MAX_TC_SERVER_CHANNELS: u8 = u8::MAX;
pub const MAX_PROCESS_DATA_ELEMENT_NUMBER: u16 = 0x0FFF;

/// Server-side measurement trigger runtime for one process-data value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementTriggerRuntime {
    pub destination: Address,
    pub element: ElementNumber,
    pub ddi: DDI,
    pub trigger_methods: u8,
    pub time_interval_ms: Option<u32>,
    pub distance_interval_mm: Option<u32>,
    pub minimum_threshold: Option<i32>,
    pub maximum_threshold: Option<i32>,
    pub change_threshold: Option<u32>,
    elapsed_ms: u32,
    distance_mm: u32,
    last_value: Option<i32>,
    last_requested_value: Option<i32>,
}

impl MeasurementTriggerRuntime {
    #[must_use]
    pub fn new(
        destination: Address,
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
    ) -> Self {
        Self {
            destination,
            element: element.into(),
            ddi: ddi.into(),
            trigger_methods: 0,
            time_interval_ms: None,
            distance_interval_mm: None,
            minimum_threshold: None,
            maximum_threshold: None,
            change_threshold: None,
            elapsed_ms: 0,
            distance_mm: 0,
            last_value: None,
            last_requested_value: None,
        }
    }

    #[must_use]
    pub fn with_trigger(mut self, trigger: super::objects::TriggerMethod) -> Self {
        self.trigger_methods |= trigger.as_u8();
        self
    }

    #[must_use]
    pub fn with_time_interval_ms(mut self, interval_ms: u32) -> Self {
        self.time_interval_ms = NonZeroU32::new(interval_ms).map(NonZeroU32::get);
        self
    }

    #[must_use]
    pub fn with_distance_interval_mm(mut self, interval_mm: u32) -> Self {
        self.distance_interval_mm = NonZeroU32::new(interval_mm).map(NonZeroU32::get);
        self
    }

    #[must_use]
    pub const fn with_minimum_threshold(mut self, threshold: i32) -> Self {
        self.minimum_threshold = Some(threshold);
        self
    }

    #[must_use]
    pub const fn with_maximum_threshold(mut self, threshold: i32) -> Self {
        self.maximum_threshold = Some(threshold);
        self
    }

    #[must_use]
    pub fn with_change_threshold(mut self, threshold: u32) -> Self {
        self.change_threshold = NonZeroU32::new(threshold).map(NonZeroU32::get);
        self
    }
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

/// Per-client server-side state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TCClientInfo {
    pub address: Address,
    pub ddop: DDOP,
    pub pool_activated: bool,
    /// Last accepted command-payload bytes for Object Pool Transfer, excluding
    /// the command byte. This lets the server treat an identical repeated
    /// transfer as idempotent instead of deactivating an already activated DDOP.
    pub last_ddop_transfer: Vec<u8>,
    pub last_status_ms: u32,
    pub tc_version: u8,
    pub tc_options: u8,
    pub tc_booms: u8,
    pub tc_sections: u8,
    pub tc_channels: u8,
}

/// Server config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TCServerConfig {
    pub tc_number: u8,
    pub tc_version: u8,
    pub num_booms: u8,
    pub num_sections: u8,
    pub num_channels: u8,
    pub server_options: u8,
}

impl Default for TCServerConfig {
    fn default() -> Self {
        Self {
            tc_number: 0,
            tc_version: 4,
            num_booms: 0,
            num_sections: 0,
            num_channels: 0,
            server_options: 0,
        }
    }
}

impl TCServerConfig {
    #[must_use]
    pub const fn with_number(mut self, n: u8) -> Self {
        self.tc_number = n;
        self
    }

    #[must_use]
    pub const fn with_version(mut self, v: u8) -> Self {
        self.tc_version = v;
        self
    }

    #[must_use]
    pub const fn with_booms(mut self, b: u8) -> Self {
        self.num_booms = b;
        self
    }

    #[must_use]
    pub const fn with_sections(mut self, s: u8) -> Self {
        self.num_sections = s;
        self
    }

    #[must_use]
    pub const fn with_channels(mut self, c: u8) -> Self {
        self.num_channels = c;
        self
    }

    #[must_use]
    pub const fn with_options(mut self, o: u8) -> Self {
        self.server_options = o;
        self
    }

    pub fn validate(&self) -> Result<()> {
        if !tc_options_byte_is_valid(self.server_options) {
            return Err(Error::invalid_data(
                "TCServerConfig: server_options contains reserved bits",
            ));
        }
        if self.num_booms == 0 {
            return Err(Error::invalid_data(
                "TCServerConfig: num_booms must be greater than zero",
            ));
        }
        if self.num_sections == 0 || self.num_sections > MAX_TC_SERVER_SECTIONS {
            return Err(Error::invalid_data(format!(
                "TCServerConfig: num_sections must be in 1..={MAX_TC_SERVER_SECTIONS}"
            )));
        }
        Ok(())
    }
}

/// Outbound frame: TC server always emits on `PGN_TC_TO_ECU`, but the
/// caller still needs the `dest` to know whether to broadcast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TCOutbound {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub dest: Option<Address>,
}

impl TCOutbound {
    #[must_use]
    pub fn broadcast(data: Vec<u8>) -> Self {
        Self {
            pgn: PGN_TC_TO_ECU,
            data,
            dest: None,
        }
    }

    #[must_use]
    pub fn to(data: Vec<u8>, dest: Address) -> Self {
        Self {
            pgn: PGN_TC_TO_ECU,
            data,
            dest: Some(dest),
        }
    }
}

// Callback signatures.
pub type ValueRequestCallback = Box<dyn FnMut(ElementNumber, DDI, Address) -> Result<i32>>;
pub type ValueCallback =
    Box<dyn FnMut(ElementNumber, DDI, i32, Address) -> Result<ProcessDataAcknowledgeErrorCodes>>;
pub type PeerControlCallback = Box<dyn FnMut(ElementNumber, DDI, ElementNumber, DDI) -> Result<()>>;

/// ISO 11783-10 Task Controller server.
pub struct TaskControllerServer {
    state: StateMachine<TCServerState>,
    clients: Vec<TCClientInfo>,
    server_options: u8,
    status_timer_ms: u32,
    tc_number: u8,
    tc_version: u8,
    num_booms: u8,
    num_sections: u8,
    num_channels: u8,
    structure_label: [u8; 7],
    localization_label: [u8; 7],
    command_busy: bool,
    current_command_source_address: Address,
    current_command_byte: u8,
    measurement_triggers: Vec<MeasurementTriggerRuntime>,

    value_request_cb: Option<ValueRequestCallback>,
    value_cb: Option<ValueCallback>,
    peer_control_cb: Option<PeerControlCallback>,

    pub on_state_change: Event<TCServerState>,
    pub on_client_connected: Event<Address>,
    pub on_client_disconnected: Event<Address>,
    pub on_pool_activation_error: Event<ObjectPoolActivationError>,
    pub on_client_version_received: Event<(Address, u8)>,
    pub on_peer_control_assignment_received: Event<PeerControlAssignment>,
}

impl TaskControllerServer {
    #[must_use]
    pub fn new(config: TCServerConfig) -> Self {
        Self {
            state: StateMachine::new(TCServerState::Disconnected),
            clients: Vec::new(),
            server_options: config.server_options,
            status_timer_ms: 0,
            tc_number: config.tc_number,
            tc_version: config.tc_version,
            num_booms: config.num_booms,
            num_sections: config.num_sections,
            num_channels: config.num_channels,
            structure_label: [0xFF; 7],
            localization_label: [0xFF; 7],
            command_busy: false,
            current_command_source_address: 0,
            current_command_byte: 0,
            measurement_triggers: Vec::new(),
            value_request_cb: None,
            value_cb: None,
            peer_control_cb: None,
            on_state_change: Event::new(),
            on_client_connected: Event::new(),
            on_client_disconnected: Event::new(),
            on_pool_activation_error: Event::new(),
            on_client_version_received: Event::new(),
            on_peer_control_assignment_received: Event::new(),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if !tc_options_byte_is_valid(self.server_options) {
            return Err(Error::invalid_data(
                "TC server_options contains reserved bits",
            ));
        }
        if self.num_booms == 0 {
            return Err(Error::invalid_data(
                "TC server num_booms must be greater than zero",
            ));
        }
        if self.num_sections == 0 || self.num_sections > MAX_TC_SERVER_SECTIONS {
            return Err(Error::invalid_data(format!(
                "TC server num_sections must be in 1..={MAX_TC_SERVER_SECTIONS}"
            )));
        }
        self.transition(TCServerState::WaitForClients);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.transition(TCServerState::Disconnected);
        self.clients.clear();
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> TCServerState {
        self.state.state()
    }

    #[inline]
    #[must_use]
    pub fn clients(&self) -> &[TCClientInfo] {
        &self.clients
    }

    pub fn on_value_request<F>(&mut self, cb: F)
    where
        F: FnMut(ElementNumber, DDI, Address) -> Result<i32> + 'static,
    {
        self.value_request_cb = Some(Box::new(cb));
    }

    pub fn on_value_received<F>(&mut self, cb: F)
    where
        F: FnMut(ElementNumber, DDI, i32, Address) -> Result<ProcessDataAcknowledgeErrorCodes>
            + 'static,
    {
        self.value_cb = Some(Box::new(cb));
    }

    pub fn on_peer_control_assignment<F>(&mut self, cb: F)
    where
        F: FnMut(ElementNumber, DDI, ElementNumber, DDI) -> Result<()> + 'static,
    {
        self.peer_control_cb = Some(Box::new(cb));
    }

    pub fn set_command_busy(&mut self, busy: bool) {
        if busy {
            self.command_busy = true;
        } else {
            self.command_busy = false;
            self.current_command_source_address = 0;
            self.current_command_byte = 0;
        }
    }

    pub fn set_command_busy_for(&mut self, source: Address, command: u8) {
        self.command_busy = true;
        self.current_command_source_address = source;
        self.current_command_byte = command;
    }

    #[must_use]
    pub const fn is_command_busy(&self) -> bool {
        self.command_busy
    }

    pub const fn set_structure_label(&mut self, label: [u8; 7]) {
        self.structure_label = label;
    }

    #[must_use]
    pub const fn structure_label(&self) -> [u8; 7] {
        self.structure_label
    }

    pub const fn clear_structure_label(&mut self) {
        self.structure_label = [0xFF; 7];
    }

    pub const fn set_localization_label(&mut self, label: [u8; 7]) {
        self.localization_label = label;
    }

    #[must_use]
    pub const fn localization_label(&self) -> [u8; 7] {
        self.localization_label
    }

    pub const fn clear_localization_label(&mut self) {
        self.localization_label = [0xFF; 7];
    }

    #[must_use]
    pub fn get_client_version(&self, addr: Address) -> u8 {
        self.clients
            .iter()
            .find(|c| c.address == addr)
            .map(|c| c.tc_version)
            .unwrap_or(0)
    }

    #[must_use]
    pub fn request_client_version(&self, addr: Address) -> Option<TCOutbound> {
        self.clients.iter().any(|c| c.address == addr).then(|| {
            TCOutbound::to(
                [
                    tc_cmd::VERSION_REQUEST,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                ]
                .to_vec(),
                addr,
            )
        })
    }

    pub fn activate_pool(&mut self, addr: Address) -> ObjectPoolActivationError {
        let error = match self.find_client_mut(addr) {
            Some(c) if !c.ddop.devices().is_empty() => {
                c.pool_activated = true;
                ObjectPoolActivationError::NoErrors
            }
            _ => ObjectPoolActivationError::ThereAreErrorsInTheDDOP,
        };
        if error != ObjectPoolActivationError::NoErrors {
            self.on_pool_activation_error.emit(&error);
        }
        error
    }

    /// Register or replace a measurement trigger runtime entry.
    ///
    /// The TC server uses this local runtime to emit `RequestValue` commands
    /// from elapsed time, accumulated distance, threshold crossings, and
    /// on-change observations instead of only exposing passive command
    /// builders.
    pub fn configure_measurement_trigger(
        &mut self,
        trigger: MeasurementTriggerRuntime,
    ) -> Result<()> {
        if trigger.destination == NULL_ADDRESS || trigger.destination == BROADCAST_ADDRESS {
            return Err(Error::invalid_address(trigger.destination));
        }
        let element: u16 = trigger.element.into();
        let ddi: u16 = trigger.ddi.into();
        if element > MAX_PROCESS_DATA_ELEMENT_NUMBER {
            return Err(Error::invalid_data(
                "measurement trigger element number exceeds 12-bit command field",
            ));
        }
        let _ = Self::build_request_value(element, ddi)?;
        if trigger.trigger_methods == 0
            && trigger.time_interval_ms.is_none()
            && trigger.distance_interval_mm.is_none()
            && trigger.minimum_threshold.is_none()
            && trigger.maximum_threshold.is_none()
            && trigger.change_threshold.is_none()
        {
            return Err(Error::invalid_data(
                "measurement trigger must define at least one trigger condition",
            ));
        }
        if let Some(existing) = self.measurement_triggers.iter_mut().find(|t| {
            t.destination == trigger.destination
                && t.element == trigger.element
                && t.ddi == trigger.ddi
        }) {
            *existing = trigger;
        } else {
            self.measurement_triggers.push(trigger);
        }
        Ok(())
    }

    /// Configure a measurement trigger and return the mandatory initial
    /// `RequestValue` command the TC must send once on measurement start
    /// (ISO 11783-10), so the client reports a baseline value before any
    /// trigger-driven updates.
    pub fn configure_measurement_trigger_with_initial(
        &mut self,
        trigger: MeasurementTriggerRuntime,
    ) -> Result<TCOutbound> {
        let destination = trigger.destination;
        let element = trigger.element;
        let ddi = trigger.ddi;
        self.configure_measurement_trigger(trigger)?;
        let payload = Self::build_request_value(element, ddi)?;
        Ok(TCOutbound::to(payload.to_vec(), destination))
    }

    pub fn clear_measurement_trigger(
        &mut self,
        destination: Address,
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
    ) -> bool {
        let element = element.into();
        let ddi = ddi.into();
        let before = self.measurement_triggers.len();
        self.measurement_triggers
            .retain(|t| !(t.destination == destination && t.element == element && t.ddi == ddi));
        self.measurement_triggers.len() != before
    }

    #[must_use]
    pub fn measurement_triggers(&self) -> &[MeasurementTriggerRuntime] {
        &self.measurement_triggers
    }

    /// Advance time-based measurement triggers and return any generated
    /// `RequestValue` commands.
    pub fn update_measurements(&mut self, elapsed_ms: u32) -> Vec<TCOutbound> {
        let mut out = Vec::new();
        for trigger in &mut self.measurement_triggers {
            let Some(interval) = trigger.time_interval_ms else {
                continue;
            };
            trigger.elapsed_ms = trigger.elapsed_ms.saturating_add(elapsed_ms);
            while trigger.elapsed_ms >= interval {
                trigger.elapsed_ms -= interval;
                if let Some(request) = measurement_request(trigger) {
                    out.push(request);
                }
            }
        }
        out
    }

    /// Add travelled distance to one client and emit due distance-triggered
    /// `RequestValue` commands.
    pub fn record_measurement_distance(
        &mut self,
        destination: Address,
        distance_mm: u32,
    ) -> Vec<TCOutbound> {
        let mut out = Vec::new();
        for trigger in self
            .measurement_triggers
            .iter_mut()
            .filter(|t| t.destination == destination)
        {
            let Some(interval) = trigger.distance_interval_mm else {
                continue;
            };
            trigger.distance_mm = trigger.distance_mm.saturating_add(distance_mm);
            while trigger.distance_mm >= interval {
                trigger.distance_mm -= interval;
                if let Some(request) = measurement_request(trigger) {
                    out.push(request);
                }
            }
        }
        out
    }

    /// Build a TC→ECU `RequestValue` payload.
    pub fn build_request_value(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload(ProcessDataCommands::RequestValue, element, ddi, None)
    }

    /// Build a TC→ECU `SetValue` payload.
    pub fn build_set_value(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        value: i32,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload(ProcessDataCommands::Value, element, ddi, Some(value))
    }

    /// Build a TC→ECU `SetValueAndAcknowledge` payload.
    pub fn build_set_value_and_acknowledge(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        value: i32,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload(
            ProcessDataCommands::SetValueAndAcknowledge,
            element,
            ddi,
            Some(value),
        )
    }

    /// Build a TC→ECU `MeasurementTimeInterval` command payload.
    pub fn build_time_interval_measurement_command(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        interval_ms: u32,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload_u32(
            ProcessDataCommands::MeasurementTimeInterval,
            element,
            ddi,
            interval_ms,
        )
    }

    /// Build a TC→ECU `MeasurementDistanceInterval` command payload.
    pub fn build_distance_interval_measurement_command(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        interval_mm: u32,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload_u32(
            ProcessDataCommands::MeasurementDistanceInterval,
            element,
            ddi,
            interval_mm,
        )
    }

    /// Build a TC→ECU `MeasurementMinimumWithinThreshold` command payload.
    pub fn build_minimum_threshold_measurement_command(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        threshold: u32,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload_u32(
            ProcessDataCommands::MeasurementMinimumWithinThreshold,
            element,
            ddi,
            threshold,
        )
    }

    /// Build a TC→ECU `MeasurementMaximumWithinThreshold` command payload.
    pub fn build_maximum_threshold_measurement_command(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        threshold: u32,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload_u32(
            ProcessDataCommands::MeasurementMaximumWithinThreshold,
            element,
            ddi,
            threshold,
        )
    }

    /// Build a TC→ECU `MeasurementChangeThreshold` command payload.
    pub fn build_change_threshold_measurement_command(
        element: impl Into<ElementNumber>,
        ddi: impl Into<DDI>,
        threshold: u32,
    ) -> Result<[u8; 8]> {
        let element: u16 = element.into().into();
        let ddi: u16 = ddi.into().into();
        encode_process_data_payload_u32(
            ProcessDataCommands::MeasurementChangeThreshold,
            element,
            ddi,
            threshold,
        )
    }

    /// Periodic status. Returns the broadcast payload at the cadence,
    /// otherwise `None`.
    pub fn update(&mut self, elapsed_ms: u32) -> Option<[u8; 8]> {
        if matches!(self.state(), TCServerState::Disconnected) {
            return None;
        }
        // Client-loss detection: age each client and drop any that has been
        // silent past the timeout, emitting a disconnect event.
        let mut lost = Vec::new();
        for client in &mut self.clients {
            client.last_status_ms = client.last_status_ms.saturating_add(elapsed_ms);
            if client.last_status_ms >= TC_CLIENT_TIMEOUT_MS {
                lost.push(client.address);
            }
        }
        if !lost.is_empty() {
            self.clients.retain(|c| !lost.contains(&c.address));
            for addr in lost {
                self.on_client_disconnected.emit(&addr);
            }
        }
        self.status_timer_ms = self.status_timer_ms.saturating_add(elapsed_ms);
        if self.status_timer_ms >= TC_STATUS_INTERVAL_MS {
            self.status_timer_ms -= TC_STATUS_INTERVAL_MS;
            return Some(self.encode_tc_status());
        }
        None
    }

    fn encode_tc_status(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = 0xF0 | ProcessDataCommands::Status.as_u8();
        data[1] = self.tc_number;
        data[2] = 0x00;
        data[3] = self.tc_version;
        data[4] = self.server_options | if self.command_busy { 0x08 } else { 0x00 };
        data[5] = if self.command_busy {
            self.current_command_source_address
        } else {
            0x00
        };
        data[6] = if self.command_busy {
            self.current_command_byte
        } else {
            0x00
        };
        data[7] = self.num_channels;
        data
    }

    /// Feed an inbound `PGN_ECU_TO_TC` message; returns the outbound
    /// frames the caller should ship. Compatibility wrapper: malformed or
    /// unrelated frames are ignored. Use [`Self::try_handle_client_message`]
    /// when caller-owned dispatch needs explicit validation errors.
    pub fn handle_client_message(&mut self, msg: &Message) -> Vec<TCOutbound> {
        self.try_handle_client_message(msg).unwrap_or_default()
    }

    /// Feed an inbound `PGN_ECU_TO_TC` message with explicit validation errors
    /// for wrong PGNs, invalid source addresses, empty messages, malformed
    /// fixed-size lifecycle requests, and malformed process-data frames.
    pub fn try_handle_client_message(&mut self, msg: &Message) -> Result<Vec<TCOutbound>> {
        if msg.pgn != PGN_ECU_TO_TC {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        if !msg.has_usable_source() {
            return Err(Error::invalid_address(msg.source));
        }
        if !msg.has_valid_destination_for_pgn() {
            return Err(Error::invalid_address(msg.destination));
        }
        if msg.data.is_empty() {
            return Err(Error::invalid_data("ECU-to-TC message must not be empty"));
        }
        if msg.data[0] == tc_cmd::VERSION_RESPONSE {
            if !version_response_is_canonical(&msg.data) {
                return Err(Error::invalid_data(
                    "TC client version response must be an 8-byte canonical frame",
                ));
            }
            return Ok(self.handle_client_version_response(msg));
        }
        if msg.data[0] == tc_cmd::VERSION_REQUEST {
            if !is_padded_fixed8(&msg.data, 1) {
                return Err(Error::invalid_data(
                    "TC technical-capabilities request must be an 8-byte padded frame",
                ));
            }
            return Ok(self.handle_tech_capabilities(msg));
        }
        if msg.data[0] == tc_cmd::REQUEST_STRUCTURE_LABEL {
            if !is_padded_fixed8(&msg.data, 1) {
                return Err(Error::invalid_data(
                    "TC structure-label request must be an 8-byte padded frame",
                ));
            }
            return Ok(self.handle_structure_label_request(msg));
        }
        if msg.data[0] == tc_cmd::REQUEST_LOCALIZATION_LABEL {
            if !is_padded_fixed8(&msg.data, 1) {
                return Err(Error::invalid_data(
                    "TC localization-label request must be an 8-byte padded frame",
                ));
            }
            return Ok(self.handle_localization_label_request(msg));
        }
        if msg.data[0] == tc_cmd::OBJECT_POOL_TRANSFER {
            return Ok(self.handle_object_pool_transfer(msg));
        }
        if msg.data[0] == tc_cmd::ACTIVATE_POOL {
            if !is_activate_pool_request(msg) && !is_deactivate_pool_request(msg) {
                return Err(Error::invalid_data(
                    "TC activate/deactivate pool request must be an 8-byte canonical frame",
                ));
            }
            return Ok(self.handle_activate_deactivate_pool_request(msg));
        }
        if msg.data[0] == tc_cmd::DELETE_POOL {
            if !is_delete_pool_request(msg) {
                return Err(Error::invalid_data(
                    "TC delete-pool request must be an 8-byte canonical frame",
                ));
            }
            return Ok(self.handle_delete_pool_request(msg));
        }
        let Some(cmd) = ProcessDataCommands::try_from_u8(msg.data[0]) else {
            return Err(Error::invalid_data(
                "ECU-to-TC message has reserved command byte",
            ));
        };
        Ok(match cmd {
            ProcessDataCommands::Value => {
                if msg.data.len() != 8 {
                    return Err(Error::invalid_data(
                        "TC value process-data frame must be an 8-byte frame",
                    ));
                }
                self.handle_value(msg, false)
            }
            ProcessDataCommands::SetValueAndAcknowledge => {
                if msg.data.len() != 8 {
                    return Err(Error::invalid_data(
                        "TC set-value-and-acknowledge frame must be an 8-byte frame",
                    ));
                }
                self.handle_value(msg, true)
            }
            ProcessDataCommands::RequestValue => {
                if !is_padded_fixed8(&msg.data, 4) {
                    return Err(Error::invalid_data(
                        "TC request-value process-data frame must be an 8-byte padded frame",
                    ));
                }
                self.handle_request_value(msg)
            }
            ProcessDataCommands::PeerControlAssignment => {
                if msg.data.len() != 8 {
                    return Err(Error::invalid_data(
                        "TC peer-control process-data frame must be an 8-byte frame",
                    ));
                }
                self.handle_peer_control(msg)
            }
            command => {
                return Err(Error::invalid_state(format!(
                    "unsupported ECU-to-TC process-data command {:?}",
                    command
                )));
            }
        })
    }

    pub fn handle_working_set_master(&mut self, msg: &Message) -> Vec<TCOutbound> {
        self.try_handle_working_set_master(msg).unwrap_or_default()
    }

    pub fn try_handle_working_set_master(&mut self, msg: &Message) -> Result<Vec<TCOutbound>> {
        if msg.pgn != PGN_WORKING_SET_MASTER {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        if !msg.has_usable_source() {
            return Err(Error::invalid_address(msg.source));
        }
        if !msg.has_valid_destination_for_pgn() {
            return Err(Error::invalid_address(msg.destination));
        }
        if !is_padded_fixed8(&msg.data, 1) {
            return Err(Error::invalid_data(
                "Working Set Master must be an 8-byte padded frame",
            ));
        }
        self.ensure_client(msg.source);
        Ok(self
            .request_client_version(msg.source)
            .into_iter()
            .collect())
    }

    fn handle_object_pool_transfer(&mut self, msg: &Message) -> Vec<TCOutbound> {
        let error = if msg.data.len() <= 8 {
            ObjectPoolErrorCodes::AnyOtherError
        } else {
            let payload = &msg.data[1..];
            let is_duplicate_accepted_pool = self.clients.iter().any(|client| {
                client.address == msg.source
                    && !client.ddop.devices().is_empty()
                    && client.last_ddop_transfer == payload
            });
            if is_duplicate_accepted_pool {
                ObjectPoolErrorCodes::NoErrors
            } else {
                match DDOP::deserialize(payload).and_then(|pool| {
                    pool.validate()?;
                    Ok(pool)
                }) {
                    Ok(pool) => {
                        self.ensure_client(msg.source);
                        match self.find_client_mut(msg.source) {
                            Some(client) => {
                                client.ddop = pool;
                                client.last_ddop_transfer = payload.to_vec();
                                client.pool_activated = false;
                                ObjectPoolErrorCodes::NoErrors
                            }
                            None => ObjectPoolErrorCodes::AnyOtherError,
                        }
                    }
                    Err(_) => ObjectPoolErrorCodes::AnyOtherError,
                }
            }
        };
        let mut data = [0xFFu8; 8];
        data[0] = tc_cmd::OBJECT_POOL_RESPONSE;
        data[1] = error.as_u8();
        vec![TCOutbound::to(data.to_vec(), msg.source)]
    }

    fn handle_activate_deactivate_pool_request(&mut self, msg: &Message) -> Vec<TCOutbound> {
        let error = if is_deactivate_pool_request(msg) {
            if let Some(client) = self.find_client_mut(msg.source) {
                client.pool_activated = false;
                ObjectPoolActivationError::NoErrors
            } else {
                ObjectPoolActivationError::ThereAreErrorsInTheDDOP
            }
        } else {
            self.activate_pool(msg.source)
        };
        let mut data = [0xFFu8; 8];
        data[0] = tc_cmd::ACTIVATE_RESPONSE;
        data[1] = error.as_u8();
        vec![TCOutbound::to(data.to_vec(), msg.source)]
    }

    fn handle_delete_pool_request(&mut self, msg: &Message) -> Vec<TCOutbound> {
        if let Some(client) = self.find_client_mut(msg.source) {
            client.ddop = DDOP::default();
            client.pool_activated = false;
            client.last_ddop_transfer.clear();
        }
        let mut data = [0xFFu8; 8];
        data[0] = tc_cmd::DELETE_POOL_RESPONSE;
        data[1] = ObjectPoolDeletionErrors::ErrorDetailsNotAvailable.as_u8();
        vec![TCOutbound::to(data.to_vec(), msg.source)]
    }

    fn handle_tech_capabilities(&mut self, msg: &Message) -> Vec<TCOutbound> {
        if !is_padded_fixed8(&msg.data, 1) {
            return Vec::new();
        }
        self.ensure_client(msg.source);
        let mut data = [0xFFu8; 8];
        data[0] = tc_cmd::VERSION_RESPONSE;
        data[1] = self.tc_version;
        data[2] = 0xFF;
        data[3] = self.server_options;
        data[4] = 0x00;
        data[5] = self.num_booms;
        data[6] = self.num_sections;
        data[7] = self.num_channels;
        vec![TCOutbound::to(data.to_vec(), msg.source)]
    }

    fn handle_structure_label_request(&mut self, msg: &Message) -> Vec<TCOutbound> {
        if !is_padded_fixed8(&msg.data, 1) {
            return Vec::new();
        }
        self.ensure_client(msg.source);
        let mut data = [0xFFu8; 8];
        data[0] = tc_cmd::STRUCTURE_LABEL;
        data[1..8].copy_from_slice(&self.structure_label);
        vec![TCOutbound::to(data.to_vec(), msg.source)]
    }

    fn handle_localization_label_request(&mut self, msg: &Message) -> Vec<TCOutbound> {
        if !is_padded_fixed8(&msg.data, 1) {
            return Vec::new();
        }
        self.ensure_client(msg.source);
        let mut data = [0xFFu8; 8];
        data[0] = tc_cmd::LOCALIZATION_LABEL;
        data[1..8].copy_from_slice(&self.localization_label);
        vec![TCOutbound::to(data.to_vec(), msg.source)]
    }

    fn handle_client_version_response(&mut self, msg: &Message) -> Vec<TCOutbound> {
        if !version_response_is_canonical(&msg.data) {
            return Vec::new();
        }
        let version = msg.data[1];
        let options = msg.data[3];
        let booms = msg.data[5];
        let sections = msg.data[6];
        let channels = msg.data[7];
        let Some(client) = self.find_client_mut(msg.source) else {
            return Vec::new();
        };
        client.tc_version = version;
        client.tc_options = options;
        client.tc_booms = booms;
        client.tc_sections = sections;
        client.tc_channels = channels;
        self.on_client_version_received.emit(&(msg.source, version));
        Vec::new()
    }

    fn handle_value(&mut self, msg: &Message, acknowledge: bool) -> Vec<TCOutbound> {
        if msg.data.len() != 8 {
            return Vec::new();
        }
        let elem_raw = ((msg.data[0] >> 4) & 0x0F) as u16 | ((msg.data[1] as u16) << 4);
        let ddi_raw = (msg.data[2] as u16) | ((msg.data[3] as u16) << 8);
        let element = ElementNumber(elem_raw);
        let ddi = DDI(ddi_raw);
        let value = i32::from_le_bytes(msg.data[4..8].try_into().unwrap());
        let error = match self.value_cb.as_mut() {
            Some(cb) => cb(element, ddi, value, msg.source)
                .unwrap_or(ProcessDataAcknowledgeErrorCodes::NoProcessingResourcesAvailable),
            None => ProcessDataAcknowledgeErrorCodes::ElementNotSupportedByThisDevice,
        };
        let mut out = self.update_measurement_value(msg.source, element, ddi, value);
        if acknowledge {
            let data = encode_process_data_ack(elem_raw, ddi_raw, error);
            out.insert(0, TCOutbound::to(data.to_vec(), msg.source));
        }
        out
    }

    fn update_measurement_value(
        &mut self,
        destination: Address,
        element: ElementNumber,
        ddi: DDI,
        value: i32,
    ) -> Vec<TCOutbound> {
        let mut out = Vec::new();
        for trigger in self
            .measurement_triggers
            .iter_mut()
            .filter(|t| t.destination == destination && t.element == element && t.ddi == ddi)
        {
            let previous = trigger.last_value;
            trigger.last_value = Some(value);
            let mut due = false;
            if trigger.trigger_methods & super::objects::TriggerMethod::OnChange.as_u8() != 0 {
                due |= previous.is_some_and(|old| old != value);
            }
            if let Some(delta) = trigger.change_threshold {
                due |= previous.is_some_and(|old| old.abs_diff(value) >= delta);
            }
            if let Some(minimum) = trigger.minimum_threshold {
                due |= value <= minimum && previous.is_none_or(|old| old > minimum);
            }
            if let Some(maximum) = trigger.maximum_threshold {
                due |= value >= maximum && previous.is_none_or(|old| old < maximum);
            }
            if trigger.trigger_methods & super::objects::TriggerMethod::Total.as_u8() != 0 {
                due |= previous.is_some();
            }
            if due && let Some(request) = measurement_request(trigger) {
                out.push(request);
            }
        }
        out
    }

    fn handle_request_value(&mut self, msg: &Message) -> Vec<TCOutbound> {
        if !is_padded_fixed8(&msg.data, 4) {
            return Vec::new();
        }
        let elem_raw = ((msg.data[0] >> 4) & 0x0F) as u16 | ((msg.data[1] as u16) << 4);
        let ddi_raw = (msg.data[2] as u16) | ((msg.data[3] as u16) << 8);
        let element = ElementNumber(elem_raw);
        let ddi = DDI(ddi_raw);
        let Some(cb) = self.value_request_cb.as_mut() else {
            return Vec::new();
        };
        let Ok(value) = cb(element, ddi, msg.source) else {
            return Vec::new();
        };
        let mut data = [0xFFu8; 8];
        data[0] = (ProcessDataCommands::Value.as_u8() & 0x0F) | ((elem_raw as u8 & 0x0F) << 4);
        data[1] = ((elem_raw >> 4) & 0xFF) as u8;
        data[2] = (ddi_raw & 0xFF) as u8;
        data[3] = ((ddi_raw >> 8) & 0xFF) as u8;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        vec![TCOutbound::to(data.to_vec(), msg.source)]
    }

    fn handle_peer_control(&mut self, msg: &Message) -> Vec<TCOutbound> {
        let Ok(assignment) = PeerControlAssignment::decode(&msg.data, msg.source, msg.destination)
        else {
            return Vec::new();
        };
        self.on_peer_control_assignment_received.emit(&assignment);

        let Some(cb) = self.peer_control_cb.as_mut() else {
            return Vec::new();
        };
        let result = cb(
            assignment.source_element,
            assignment.source_ddi,
            assignment.destination_element,
            assignment.destination_ddi,
        );

        let error = if result.is_ok() {
            ProcessDataAcknowledgeErrorCodes::NoError
        } else {
            ProcessDataAcknowledgeErrorCodes::NoProcessingResourcesAvailable
        };
        let ack =
            encode_process_data_ack(assignment.source_element.0, assignment.source_ddi.0, error);
        vec![TCOutbound::to(ack.to_vec(), msg.source)]
    }

    fn ensure_client(&mut self, addr: Address) {
        if let Some(client) = self.clients.iter_mut().find(|c| c.address == addr) {
            // Any message from the client is a liveness keepalive.
            client.last_status_ms = 0;
            return;
        }
        self.clients.push(TCClientInfo {
            address: addr,
            ..Default::default()
        });
        self.on_client_connected.emit(&addr);
        if self.state() == TCServerState::WaitForClients {
            self.transition(TCServerState::Active);
        }
    }

    fn find_client_mut(&mut self, addr: Address) -> Option<&mut TCClientInfo> {
        self.clients.iter_mut().find(|c| c.address == addr)
    }

    fn transition(&mut self, new_state: TCServerState) {
        if self.state() == new_state {
            return;
        }
        self.state.transition(new_state);
        self.on_state_change.emit(&new_state);
    }
}

fn is_activate_pool_request(msg: &Message) -> bool {
    msg.data[0] == tc_cmd::ACTIVATE_POOL
        && msg.data.len() == 8
        && msg.data[1] == 0xFF
        && msg.data[1..].iter().all(|&b| b == 0xFF)
}

fn is_deactivate_pool_request(msg: &Message) -> bool {
    msg.data[0] == tc_cmd::ACTIVATE_POOL
        && msg.data.len() == 8
        && msg.data[1] == 0x00
        && msg.data[2..].iter().all(|&b| b == 0xFF)
}

fn is_delete_pool_request(msg: &Message) -> bool {
    msg.data[0] == tc_cmd::DELETE_POOL
        && msg.data.len() == 8
        && msg.data[1..].iter().all(|&b| b == 0xFF)
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
) -> Result<[u8; 8]> {
    encode_process_data_payload_bytes(command, element, ddi, value.map(i32::to_le_bytes))
}

fn encode_process_data_payload_u32(
    command: ProcessDataCommands,
    element: u16,
    ddi: u16,
    value: u32,
) -> Result<[u8; 8]> {
    encode_process_data_payload_bytes(command, element, ddi, Some(value.to_le_bytes()))
}

fn encode_process_data_payload_bytes(
    command: ProcessDataCommands,
    element: u16,
    ddi: u16,
    value: Option<[u8; 4]>,
) -> Result<[u8; 8]> {
    if element > MAX_PROCESS_DATA_ELEMENT_NUMBER {
        return Err(Error::invalid_data(format!(
            "TC process-data element {} exceeds 12-bit wire maximum {}",
            element, MAX_PROCESS_DATA_ELEMENT_NUMBER
        )));
    }
    let mut data = [0xFFu8; 8];
    data[0] = (command.as_u8() & 0x0F) | ((element as u8 & 0x0F) << 4);
    data[1] = ((element >> 4) & 0xFF) as u8;
    data[2] = (ddi & 0xFF) as u8;
    data[3] = ((ddi >> 8) & 0xFF) as u8;
    if let Some(value) = value {
        data[4..8].copy_from_slice(&value);
    }
    Ok(data)
}

fn measurement_request(trigger: &mut MeasurementTriggerRuntime) -> Option<TCOutbound> {
    let payload = TaskControllerServer::build_request_value(trigger.element, trigger.ddi).ok()?;
    trigger.last_requested_value = trigger.last_value;
    Some(TCOutbound::to(payload.to_vec(), trigger.destination))
}

impl Default for TaskControllerServer {
    fn default() -> Self {
        Self::new(TCServerConfig::default())
    }
}

