use alloc::{
    boxed::Box,
    collections::{BTreeMap as HashMap, VecDeque},
    format,
    string::ToString,
    vec::Vec,
};
use core::cell::RefCell;

#[cfg(feature = "embedded")]
use crate::fixed::FixedQueue;
use alloc::rc::Rc;

use crate::j1939::pgn_request::decode_request;

use super::address_claimer::AddressClaimer;
use super::bus_load::BusLoad;
use super::can_adapter::{CanEndpoint, Link};
use super::constants::{
    ADDRESS_CLAIM_TIMEOUT_MS, BROADCAST_ADDRESS, CAN_DATA_LENGTH, FAST_PACKET_MAX_DATA,
    MAX_ADDRESS, NULL_ADDRESS, TP_MAX_DATA_LENGTH,
};
use super::control_function::CfState;
use super::error::{Error, ErrorCode, Result};
use super::etp::ExtendedTransportProtocol;
use super::event::Event;
use super::fast_packet::FastPacketProtocol;
use super::frame::Frame;
use super::identifier::Identifier;
use super::internal_cf::{ClaimState, InternalCf};
use super::message::Message;
use super::name::Name;
use super::partner_cf::{NameFilter, PartnerCf};
use super::pgn::pgn_is_valid;
use super::pgn_defs::{
    PGN_ADDRESS_CLAIMED, PGN_COMMANDED_ADDRESS, PGN_ETP_CM, PGN_ETP_DT, PGN_REQUEST, PGN_TP_CM,
    PGN_TP_DT,
};
use super::session::{TransportDirection, TransportSession};
use super::tp::TransportProtocol;
use super::types::{Address, Pgn, Priority};

/// Fluent network configuration.
#[derive(Debug, Clone, Copy)]
pub struct NetworkConfig {
    pub num_ports: u8,
    pub address_claim_timeout_ms: u32,
    pub enable_bus_load: bool,
    /// Enable NMEA2000 fast packet for [`IsoNet::register_fast_packet_pgn`]'d PGNs.
    pub enable_fast_packet: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            num_ports: 1,
            address_claim_timeout_ms: ADDRESS_CLAIM_TIMEOUT_MS,
            enable_bus_load: true,
            enable_fast_packet: false,
        }
    }
}

impl NetworkConfig {
    #[must_use]
    pub fn ports(mut self, n: u8) -> Self {
        self.num_ports = n;
        self
    }

    #[must_use]
    pub fn claim_timeout(mut self, ms: u32) -> Self {
        self.address_claim_timeout_ms = ms;
        self
    }

    #[must_use]
    pub fn bus_load(mut self, enable: bool) -> Self {
        self.enable_bus_load = enable;
        self
    }

    #[must_use]
    pub fn fast_packet(mut self, enable: bool) -> Self {
        self.enable_fast_packet = enable;
        self
    }
}

/// Opaque handle to an [`InternalCf`] tracked by an [`IsoNet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternalCfHandle(usize);

/// Opaque handle to a [`PartnerCf`] tracked by an [`IsoNet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartnerCfHandle(usize);

type CompletionsQueue = Rc<RefCell<Vec<TransportSession>>>;
type PgnCallback = Box<dyn FnMut(&Message)>;
const MAX_PENDING_TRANSPORT_TX: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingTransportKind {
    Tp,
    Etp,
}

#[derive(Debug, Clone)]
struct PendingTransportTx {
    kind: PendingTransportKind,
    pgn: Pgn,
    data: Vec<u8>,
    source: Address,
    destination: Address,
    port: u8,
    priority: Priority,
}

#[cfg(feature = "embedded")]
type PendingTransportQueue = FixedQueue<PendingTransportTx, MAX_PENDING_TRANSPORT_TX>;
#[cfg(any(feature = "default", feature = "cli"))]
type PendingTransportQueue = VecDeque<PendingTransportTx>;

/// ISOBUS network layer. Owns CAN endpoints, control functions,
/// transport engines, and the PGN dispatch.
/// Aggregate network-message statistics (ISO 11783-12 diagnostic UI).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NetworkStatistics {
    /// CAN frames transmitted via `send_frame`.
    pub frames_sent: u64,
    /// CAN frames received and processed.
    pub frames_received: u64,
}

pub struct IsoNet<L: Link> {
    config: NetworkConfig,
    stats: NetworkStatistics,
    internal_cfs: Vec<InternalCf>,
    partner_cfs: Vec<PartnerCf>,
    claimers: Vec<AddressClaimer>,
    endpoints: HashMap<u8, CanEndpoint<L>>,
    bus_loads: HashMap<u8, BusLoad>,

    tp: TransportProtocol,
    etp: ExtendedTransportProtocol,
    fast_packet: FastPacketProtocol,

    pgn_callbacks: HashMap<Pgn, Vec<PgnCallback>>,
    fast_packet_pgns: Vec<Pgn>,
    pending_transport_tx: PendingTransportQueue,

    tp_completions: CompletionsQueue,
    etp_completions: CompletionsQueue,

    /// Sans-IO outbound buffer. When [`Self::capture_outbound`] is set, frames
    /// that would be written to an endpoint are queued here instead, to be
    /// drained by a driver via [`Self::take_outbound`].
    outbound: VecDeque<(u8, Frame)>,
    /// When true, `send_frame` buffers into `outbound` instead of writing to a
    /// `CanEndpoint`. Drives the link-less ("sans-IO") path. Default `false`
    /// preserves the existing endpoint-backed behavior.
    capture_outbound: bool,
    /// Sans-IO inbound message buffer. When enabled, dispatched messages are
    /// cloned into this queue so an embedded driver can drain them directly
    /// without installing a boxed callback listener.
    captured_messages: VecDeque<Message>,
    /// When true, `dispatch_message` also buffers messages into
    /// `captured_messages`. Default `false` preserves hosted callback-only
    /// behaviour.
    capture_messages: bool,

    /// Fires for every dispatched [`Message`] (single-frame and
    /// reassembled).
    pub on_message: Event<Message>,
    /// Fires when another node uses an address we have claimed.
    pub on_address_violation: Event<Address>,
    /// Fires when an incoming claim proves that a local NAME is duplicated on
    /// the bus. The tuple is `(duplicate_name, claimed_source_address)`.
    pub on_duplicate_name: Event<(Name, Address)>,
}

impl<L: Link> IsoNet<L> {
    #[must_use]
    pub fn new(config: NetworkConfig) -> Self {
        let mut bus_loads = HashMap::new();
        if config.enable_bus_load {
            for i in 0..config.num_ports {
                bus_loads.insert(i, BusLoad::new());
            }
        }

        let mut tp = TransportProtocol::new();
        let mut etp = ExtendedTransportProtocol::new();
        let tp_completions: CompletionsQueue = Rc::new(RefCell::new(Vec::new()));
        let etp_completions: CompletionsQueue = Rc::new(RefCell::new(Vec::new()));
        {
            let q = tp_completions.clone();
            tp.on_complete
                .subscribe(move |s| q.borrow_mut().push(s.clone()));
        }
        {
            let q = etp_completions.clone();
            etp.on_complete
                .subscribe(move |s| q.borrow_mut().push(s.clone()));
        }

        Self {
            config,
            stats: NetworkStatistics::default(),
            internal_cfs: Vec::new(),
            partner_cfs: Vec::new(),
            claimers: Vec::new(),
            endpoints: HashMap::new(),
            bus_loads,
            tp,
            etp,
            fast_packet: FastPacketProtocol::new(),
            pgn_callbacks: HashMap::new(),
            fast_packet_pgns: Vec::new(),
            pending_transport_tx: PendingTransportQueue::new(),
            tp_completions,
            etp_completions,
            outbound: VecDeque::new(),
            capture_outbound: false,
            captured_messages: VecDeque::new(),
            capture_messages: false,
            on_message: Event::new(),
            on_address_violation: Event::new(),
            on_duplicate_name: Event::new(),
        }
    }

    // ─── Device management ────────────────────────────────────────

    pub fn create_internal(
        &mut self,
        name: Name,
        port: u8,
        preferred: Address,
    ) -> Result<InternalCfHandle> {
        if self.internal_cfs.iter().any(|cf| cf.name() == name) {
            return Err(Error::with_message(
                ErrorCode::AddressConflict,
                format!("duplicate internal NAME 0x{:016X}", name.raw),
            ));
        }
        let icf = InternalCf::new(name, port, preferred);
        let claimer = AddressClaimer::with_timeout(self.config.address_claim_timeout_ms, 0);
        self.internal_cfs.push(icf);
        self.claimers.push(claimer);
        let h = InternalCfHandle(self.internal_cfs.len() - 1);
        tracing::info!(target: "machbus.network", port, "internal CF created");
        Ok(h)
    }

    pub fn create_partner(
        &mut self,
        port: u8,
        filters: Vec<NameFilter>,
    ) -> Result<PartnerCfHandle> {
        self.partner_cfs.push(PartnerCf::new(port, filters));
        let h = PartnerCfHandle(self.partner_cfs.len() - 1);
        tracing::info!(target: "machbus.network", port, "partner CF created");
        Ok(h)
    }

    #[must_use]
    pub fn internal_cf(&self, h: InternalCfHandle) -> Option<&InternalCf> {
        self.internal_cfs.get(h.0)
    }

    pub fn internal_cf_mut(&mut self, h: InternalCfHandle) -> Option<&mut InternalCf> {
        self.internal_cfs.get_mut(h.0)
    }

    #[must_use]
    pub fn partner_cf(&self, h: PartnerCfHandle) -> Option<&PartnerCf> {
        self.partner_cfs.get(h.0)
    }

    pub fn partner_cf_mut(&mut self, h: PartnerCfHandle) -> Option<&mut PartnerCf> {
        self.partner_cfs.get_mut(h.0)
    }

    // ─── Endpoint registration ────────────────────────────────────

    pub fn set_endpoint(&mut self, port: u8, endpoint: CanEndpoint<L>) {
        self.endpoints.insert(port, endpoint);
        if self.config.enable_bus_load {
            self.bus_loads.entry(port).or_default();
        }
        tracing::debug!(target: "machbus.network", port, "endpoint set");
    }

    #[must_use]
    pub fn endpoint(&self, port: u8) -> Option<&CanEndpoint<L>> {
        self.endpoints.get(&port)
    }

    pub fn endpoint_mut(&mut self, port: u8) -> Option<&mut CanEndpoint<L>> {
        self.endpoints.get_mut(&port)
    }

    /// The CAN error-confinement state of `port`'s controller, if connected.
    /// Feed this to a [`FaultConfinementMonitor`](super::fault_confinement::FaultConfinementMonitor)
    /// to drive fail-safe behaviour on error-passive / bus-off.
    #[must_use]
    pub fn bus_state(&self, port: u8) -> Option<super::can_adapter::can::BusState> {
        self.endpoints.get(&port).map(CanEndpoint::bus_state)
    }

    /// The ports with a connected endpoint, ascending.
    #[must_use]
    pub fn ports(&self) -> Vec<u8> {
        let mut ports: Vec<u8> = self.endpoints.keys().copied().collect();
        ports.sort_unstable();
        ports
    }

    // ─── PGN callback registry ────────────────────────────────────

    pub fn register_pgn_callback<F>(&mut self, pgn: Pgn, callback: F) -> Result<()>
    where
        F: FnMut(&Message) + 'static,
    {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        self.pgn_callbacks
            .entry(pgn)
            .or_default()
            .push(Box::new(callback));
        Ok(())
    }

    pub fn register_fast_packet_pgn(&mut self, pgn: Pgn) -> Result<()> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        if !self.fast_packet_pgns.contains(&pgn) {
            self.fast_packet_pgns.push(pgn);
        }
        Ok(())
    }

    // ─── Sending ──────────────────────────────────────────────────

    /// Auto-select the right transport mode and send.
    ///
    /// - ≤ 8 bytes → single CAN frame
    /// - registered fast-packet PGN, ≤ 223 bytes → NMEA2000 fast packet
    /// - 9..=1785 → ISO 11783 / J1939 TP (BAM if `dst == BROADCAST`)
    /// - > 1785 → ETP (rejects broadcast)
    pub fn send(
        &mut self,
        pgn: Pgn,
        data: &[u8],
        src: InternalCfHandle,
        dst: Address,
        priority: Priority,
    ) -> Result<()> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        let icf = self
            .internal_cfs
            .get(src.0)
            .ok_or_else(Error::not_connected)?;
        if icf.claim_state() != ClaimState::Claimed || !icf.cf().is_online() {
            return Err(Error::invalid_state(
                "control function has not claimed an address",
            ));
        }
        if !icf.cf().address_valid() {
            return Err(Error::not_connected());
        }
        let src_addr = icf.address();
        let port = icf.port();

        if (data.len() as u32) <= CAN_DATA_LENGTH {
            return self.send_single_frame(pgn, data, src_addr, dst, priority, port);
        }

        if self.is_fast_packet_pgn(pgn) && (data.len() as u32) <= FAST_PACKET_MAX_DATA {
            let frames = self.fast_packet.send(pgn, data, src_addr)?;
            return self.send_frames(&frames, port);
        }

        if (data.len() as u32) <= TP_MAX_DATA_LENGTH {
            match self.tp.send(pgn, data, src_addr, dst, port, priority) {
                Ok(frames) => return self.send_frames(&frames, port),
                Err(err) if err.code == ErrorCode::SessionExists => {
                    return self.queue_transport_tx(PendingTransportTx {
                        kind: PendingTransportKind::Tp,
                        pgn,
                        data: data.to_vec(),
                        source: src_addr,
                        destination: dst,
                        port,
                        priority,
                    });
                }
                Err(err) => return Err(err),
            }
        }

        if dst == BROADCAST_ADDRESS {
            return Err(Error::invalid_state("ETP does not support broadcast"));
        }
        match self.etp.send(pgn, data, src_addr, dst, port, priority) {
            Ok(frames) => self.send_frames(&frames, port),
            Err(err) if err.code == ErrorCode::SessionExists => {
                self.queue_transport_tx(PendingTransportTx {
                    kind: PendingTransportKind::Etp,
                    pgn,
                    data: data.to_vec(),
                    source: src_addr,
                    destination: dst,
                    port,
                    priority,
                })
            }
            Err(err) => Err(err),
        }
    }

    /// Send a single pre-formed [`Frame`] on the given port.
    ///
    /// In the default endpoint-backed mode this writes to the `CanEndpoint` for
    /// `port`. When [`Self::set_capture_outbound`] is enabled (the sans-IO
    /// path), the frame is buffered for a driver to drain via
    /// [`Self::take_outbound`] instead.
    pub fn send_frame(&mut self, frame: &Frame, port: u8) -> Result<()> {
        if self.capture_outbound {
            self.outbound.push_back((port, *frame));
            self.stats.frames_sent += 1;
            if self.config.enable_bus_load
                && let Some(bl) = self.bus_loads.get_mut(&port)
            {
                bl.add_frame(frame.length);
            }
            return Ok(());
        }
        let endpoint = self
            .endpoints
            .get_mut(&port)
            .ok_or_else(Error::not_connected)?;
        endpoint.send_can(&frame.to_can_frame()).map_err(|e| {
            Error::with_message(super::error::ErrorCode::DriverError, e.to_string())
        })?;
        self.stats.frames_sent += 1;
        if self.config.enable_bus_load
            && let Some(bl) = self.bus_loads.get_mut(&port)
        {
            bl.add_frame(frame.length);
        }
        Ok(())
    }

    // ─── Sans-IO seam (link-less feed / drain) ────────────────────────

    /// Enable or disable link-less ("sans-IO") operation. When enabled,
    /// outbound frames are buffered (see [`Self::take_outbound`]) instead of
    /// being written to a `CanEndpoint`, and inbound frames arrive via
    /// [`Self::feed`] rather than by polling endpoints. Default: disabled.
    pub fn set_capture_outbound(&mut self, enabled: bool) {
        self.capture_outbound = enabled;
    }

    /// Whether link-less outbound capture is enabled.
    #[must_use]
    pub fn is_capturing_outbound(&self) -> bool {
        self.capture_outbound
    }

    /// Take the next buffered outbound `(port, frame)`, or `None` if the buffer
    /// is empty. A driver drains this and writes the frames to its transport.
    pub fn take_outbound(&mut self) -> Option<(u8, Frame)> {
        self.outbound.pop_front()
    }

    /// Number of buffered outbound frames awaiting drain.
    #[must_use]
    pub fn outbound_len(&self) -> usize {
        self.outbound.len()
    }

    /// Feed one received [`Frame`] on `port` into the routing path, exactly as
    /// the endpoint-polling `update` loop would. This is the inbound half of the
    /// sans-IO seam: a driver reads a frame off its transport and hands it here.
    pub fn feed(&mut self, frame: &Frame, port: u8) {
        if self.config.enable_bus_load
            && let Some(bl) = self.bus_loads.get_mut(&port)
        {
            bl.add_frame(frame.length);
        }
        self.process_frame(frame, port);
    }

    fn send_single_frame(
        &mut self,
        pgn: Pgn,
        data: &[u8],
        src: Address,
        dst: Address,
        priority: Priority,
        port: u8,
    ) -> Result<()> {
        let mut frame_data = [0xFFu8; 8];
        let n = data.len().min(8);
        frame_data[..n].copy_from_slice(&data[..n]);
        let frame = Frame::new(Identifier::encode(priority, pgn, src, dst), frame_data, 8);
        self.send_frame(&frame, port)
    }

    fn send_frames(&mut self, frames: &[Frame], port: u8) -> Result<()> {
        for f in frames {
            self.send_frame(f, port)?;
        }
        Ok(())
    }

    fn queue_transport_tx(&mut self, tx: PendingTransportTx) -> Result<()> {
        if !self.pending_transport_tx_has_capacity() {
            return Err(Error::with_message(
                ErrorCode::NoResources,
                "pending transport transmit queue full",
            ));
        }
        tracing::debug!(
            target: "machbus.network",
            pgn = tx.pgn,
            src = tx.source,
            dst = tx.destination,
            queued = self.pending_transport_tx.len() + 1,
            "queued transport transmit behind active DT endpoint path"
        );
        self.push_pending_transport_tx(tx)
    }

    #[cfg(feature = "embedded")]
    fn pending_transport_tx_has_capacity(&self) -> bool {
        !self.pending_transport_tx.is_full()
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    fn pending_transport_tx_has_capacity(&self) -> bool {
        self.pending_transport_tx.len() < MAX_PENDING_TRANSPORT_TX
    }

    #[cfg(feature = "embedded")]
    fn push_pending_transport_tx(&mut self, tx: PendingTransportTx) -> Result<()> {
        self.pending_transport_tx.push_back(tx).map_err(|_| {
            Error::with_message(
                ErrorCode::NoResources,
                "pending transport transmit queue full",
            )
        })
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    fn push_pending_transport_tx(&mut self, tx: PendingTransportTx) -> Result<()> {
        self.pending_transport_tx.push_back(tx);
        Ok(())
    }

    fn try_start_pending_transport_tx(&mut self, tx: &PendingTransportTx) -> Result<()> {
        let frames = match tx.kind {
            PendingTransportKind::Tp => self.tp.send(
                tx.pgn,
                &tx.data,
                tx.source,
                tx.destination,
                tx.port,
                tx.priority,
            )?,
            PendingTransportKind::Etp => self.etp.send(
                tx.pgn,
                &tx.data,
                tx.source,
                tx.destination,
                tx.port,
                tx.priority,
            )?,
        };
        self.send_frames(&frames, tx.port)
    }

    fn start_ready_pending_transport(&mut self) {
        let attempts = self.pending_transport_tx.len();
        for _ in 0..attempts {
            let Some(tx) = self.pending_transport_tx.pop_front() else {
                break;
            };
            match self.try_start_pending_transport_tx(&tx) {
                Ok(()) => {
                    tracing::debug!(
                        target: "machbus.network",
                        pgn = tx.pgn,
                        src = tx.source,
                        dst = tx.destination,
                        "started queued transport transmit"
                    );
                }
                Err(err)
                    if matches!(err.code, ErrorCode::SessionExists | ErrorCode::NoResources) =>
                {
                    if let Err(requeue_err) = self.push_pending_transport_tx(tx) {
                        tracing::warn!(
                            target: "machbus.network",
                            error = %requeue_err,
                            "dropping queued transport transmit after requeue failed"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        target: "machbus.network",
                        pgn = tx.pgn,
                        src = tx.source,
                        dst = tx.destination,
                        error = %err,
                        "dropping queued transport transmit"
                    );
                }
            }
        }
    }

    fn send_frames_best_effort(&mut self, frames: &[Frame], port: u8) {
        for f in frames {
            let _ = self.send_frame(f, port);
        }
    }

    fn is_fast_packet_pgn(&self, pgn: Pgn) -> bool {
        self.config.enable_fast_packet && self.fast_packet_pgns.contains(&pgn)
    }

    // ─── Address claiming ─────────────────────────────────────────

    pub fn start_address_claiming(&mut self) -> Result<()> {
        if self.claimers.is_empty() {
            return Err(Error::invalid_state("no control functions registered"));
        }
        let mut emitted: Vec<(u8, Frame)> = Vec::new();
        for (icf, claimer) in self.internal_cfs.iter_mut().zip(self.claimers.iter_mut()) {
            let port = icf.port();
            for f in claimer.start(icf) {
                emitted.push((port, f));
            }
        }
        for (port, f) in &emitted {
            let _ = self.send_frame(f, *port);
        }
        tracing::debug!(target: "machbus.network", "address claiming started");
        Ok(())
    }

    /// Send the current Address Claimed response for registered internal
    /// control functions.
    ///
    /// This is used by stack-owned NAME Management RequestAddressClaim handling
    /// and mirrors the normal PGN Request-for-Address-Claim responder without
    /// requiring a synthetic inbound Request frame.
    pub fn send_address_claim_responses(&mut self) -> Result<()> {
        if self.claimers.is_empty() {
            return Err(Error::invalid_state("no control functions registered"));
        }
        let mut emitted: Vec<(u8, Frame)> = Vec::new();
        for (icf, claimer) in self.internal_cfs.iter_mut().zip(self.claimers.iter_mut()) {
            let port = icf.port();
            for f in claimer.handle_request_for_claim(icf) {
                emitted.push((port, f));
            }
        }
        for (port, f) in &emitted {
            let _ = self.send_frame(f, *port);
        }
        Ok(())
    }

    // ─── Main poll loop ────────────────────────────────────────────

    pub fn update(&mut self, elapsed_ms: u32) {
        for partner in &mut self.partner_cfs {
            partner.update_claim_validation(elapsed_ms);
        }

        // 1) Drain RX from every endpoint.
        let mut rx: Vec<(u8, Frame)> = Vec::new();
        let ports: Vec<u8> = self.endpoints.keys().copied().collect();
        for port in ports {
            if let Some(ep) = self.endpoints.get_mut(&port) {
                while let Ok(cf) = ep.recv_can() {
                    if let Some(frame) = Frame::from_can_frame(&cf) {
                        rx.push((port, frame));
                    }
                }
            }
        }
        for (port, frame) in rx {
            if self.config.enable_bus_load
                && let Some(bl) = self.bus_loads.get_mut(&port)
            {
                bl.add_frame(frame.length);
            }
            self.process_frame(&frame, port);
        }

        // 2) Drive transport timers / outputs.
        let tp_frames = self.tp.update(elapsed_ms);
        for f in &tp_frames {
            let port = self.port_for_address(f.source());
            let _ = self.send_frame(f, port);
        }
        let tp_data = self.tp.get_pending_data_frames();
        for f in &tp_data {
            let port = self.port_for_address(f.source());
            let _ = self.send_frame(f, port);
        }

        let etp_frames = self.etp.update(elapsed_ms);
        for f in &etp_frames {
            let port = self.port_for_address(f.source());
            let _ = self.send_frame(f, port);
        }
        let etp_data = self.etp.get_pending_data_frames();
        for f in &etp_data {
            let port = self.port_for_address(f.source());
            let _ = self.send_frame(f, port);
        }

        self.fast_packet.update(elapsed_ms);

        // 3) Drain transport completions and dispatch.
        self.drain_transport_completions();
        self.start_ready_pending_transport();

        // 4) Drive address claimers.
        let mut emitted: Vec<(u8, Frame)> = Vec::new();
        for (icf, claimer) in self.internal_cfs.iter_mut().zip(self.claimers.iter_mut()) {
            let port = icf.port();
            for f in claimer.update(icf, elapsed_ms) {
                emitted.push((port, f));
            }
        }
        for (port, f) in &emitted {
            let _ = self.send_frame(f, *port);
        }

        // 5) Bus-load sample binning.
        if self.config.enable_bus_load {
            for bl in self.bus_loads.values_mut() {
                bl.update(elapsed_ms);
            }
        }
    }

    /// Inject a [`Message`] directly into the dispatch path. Useful
    /// for unit tests and synthetic scenarios.
    pub fn inject_message(&mut self, msg: &Message) {
        self.dispatch_message(msg);
    }

    /// Enable or disable direct message capture for sans-IO drivers.
    ///
    /// This is off by default so hosted users only pay for their subscribed
    /// callbacks. Embedded sessions enable it to avoid a boxed listener and
    /// drain messages with [`Self::take_message`].
    pub fn set_capture_messages(&mut self, enabled: bool) {
        self.capture_messages = enabled;
        if !enabled {
            self.captured_messages.clear();
        }
    }

    /// Drain the next captured message when message capture is enabled.
    pub fn take_message(&mut self) -> Option<Message> {
        self.captured_messages.pop_front()
    }

    #[must_use]
    pub fn bus_load(&self, port: u8) -> f32 {
        self.bus_loads.get(&port).map_or(0.0, BusLoad::load_percent)
    }

    /// Aggregate network-message statistics (frames sent / received).
    #[must_use]
    pub fn statistics(&self) -> NetworkStatistics {
        self.stats
    }

    pub fn transport_protocol(&mut self) -> &mut TransportProtocol {
        &mut self.tp
    }

    pub fn extended_transport_protocol(&mut self) -> &mut ExtendedTransportProtocol {
        &mut self.etp
    }

    pub fn fast_packet_protocol(&mut self) -> &mut FastPacketProtocol {
        &mut self.fast_packet
    }

    // ─── Internal frame routing ───────────────────────────────────

    fn process_frame(&mut self, frame: &Frame, port: u8) {
        self.stats.frames_received += 1;
        let pgn = frame.pgn();

        if pgn == PGN_ADDRESS_CLAIMED {
            self.handle_address_claim(frame, port);
            self.dispatch_message(&Message::with_addressing(
                frame.pgn(),
                frame.payload().to_vec(),
                frame.source(),
                frame.destination(),
                frame.priority(),
            ));
            return;
        }

        if pgn == PGN_REQUEST && self.handle_request_for_address_claim(frame, port) {
            return;
        }

        self.check_address_violation(frame, port);

        if pgn == PGN_TP_CM || pgn == PGN_TP_DT {
            let responses = self.tp.process_frame(frame, port);
            self.send_frames_best_effort(&responses, port);
            self.drain_transport_completions();
            return;
        }

        if pgn == PGN_ETP_CM || pgn == PGN_ETP_DT {
            let responses = self.etp.process_frame(frame, port);
            self.send_frames_best_effort(&responses, port);
            self.drain_transport_completions();
            return;
        }

        if self.is_fast_packet_pgn(pgn) {
            if let Some(msg) = self.fast_packet.process_frame(frame) {
                self.dispatch_message(&msg);
            }
            return;
        }

        // Single-frame: build a Message and dispatch.
        let msg = Message {
            pgn,
            source: frame.source(),
            destination: frame.destination(),
            priority: frame.priority(),
            timestamp_us: frame.timestamp_us,
            data: frame.payload().to_vec(),
        };
        self.dispatch_message(&msg);
    }

    fn drain_transport_completions(&mut self) {
        // Drain TP first, then ETP. Clone-and-drain pattern keeps the
        // queue's `borrow_mut()` short-lived.
        let mut sessions: Vec<TransportSession> =
            self.tp_completions.borrow_mut().drain(..).collect();
        sessions.extend(self.etp_completions.borrow_mut().drain(..));
        for session in sessions {
            if session.direction != TransportDirection::Receive {
                continue;
            }
            let port = session.can_port;
            let msg = Message {
                pgn: session.pgn,
                source: session.source_address,
                destination: session.destination_address,
                priority: session.priority,
                data: session.data,
                timestamp_us: 0,
            };
            tracing::debug!(
                target: "machbus.network",
                pgn = msg.pgn,
                bytes = msg.data.len(),
                "transport complete",
            );
            self.handle_commanded_address_message(&msg, port);
            self.dispatch_message(&msg);
        }
    }

    fn handle_commanded_address_message(&mut self, msg: &Message, port: u8) -> bool {
        if msg.pgn != PGN_COMMANDED_ADDRESS || msg.data.len() != 9 {
            return false;
        }
        let Some(target_name) = Name::from_bytes(&msg.data[..8]) else {
            return false;
        };
        let new_address = msg.data[8];
        if new_address > MAX_ADDRESS {
            return false;
        }

        // The commanded address cannot be taken if another (non-target) CF on
        // the same port already holds it. Per ISO 11783-5 the addressed CF then
        // keeps its current address and re-announces that claim rather than
        // silently dropping the command.
        let occupied_by_other = self.internal_cfs.iter().any(|icf| {
            icf.port() == port
                && icf.address() == new_address
                && icf.name() != target_name
                && icf.claim_state() == ClaimState::Claimed
        }) || self.partner_cfs.iter().any(|p| {
            p.port() == port
                && p.address() == new_address
                && p.name() != target_name
                && p.cf().is_online()
        });

        let mut emitted = Vec::new();
        let mut matched = false;
        for icf in &mut self.internal_cfs {
            if icf.port() != port || icf.name() != target_name {
                continue;
            }
            if icf.claim_state() != ClaimState::Claimed || !icf.cf().is_online() {
                continue;
            }

            if occupied_by_other {
                tracing::warn!(
                    target: "machbus.network.claim",
                    current = %format_args!("0x{:02X}", icf.address()),
                    refused = %format_args!("0x{new_address:02X}"),
                    "commanded address occupied — re-announcing current claim",
                );
                // Re-announce the existing claim at the current address.
                emitted.push(Frame::new(
                    Identifier::encode(
                        Priority::Default,
                        PGN_ADDRESS_CLAIMED,
                        icf.address(),
                        BROADCAST_ADDRESS,
                    ),
                    target_name.to_bytes(),
                    CAN_DATA_LENGTH as u8,
                ));
                matched = true;
                continue;
            }

            tracing::info!(
                target: "machbus.network.claim",
                old = %format_args!("0x{:02X}", icf.address()),
                new = %format_args!("0x{new_address:02X}"),
                "commanded address accepted",
            );
            icf.set_address(new_address);
            icf.set_state(CfState::Online);
            icf.reset_claim_timer();
            icf.on_address_claimed.emit(&new_address);
            emitted.push(Frame::new(
                Identifier::encode(
                    Priority::Default,
                    PGN_ADDRESS_CLAIMED,
                    new_address,
                    BROADCAST_ADDRESS,
                ),
                target_name.to_bytes(),
                CAN_DATA_LENGTH as u8,
            ));
            matched = true;
        }

        self.send_frames_best_effort(&emitted, port);
        matched
    }

    fn check_address_violation(&mut self, frame: &Frame, port: u8) {
        let src = frame.source();
        if src == NULL_ADDRESS || src == BROADCAST_ADDRESS {
            return;
        }
        let mut emitted: Vec<Frame> = Vec::new();
        let mut violated = false;
        for (icf, claimer) in self.internal_cfs.iter_mut().zip(self.claimers.iter_mut()) {
            if icf.port() == port
                && icf.claim_state() == ClaimState::Claimed
                && icf.address() == src
            {
                tracing::warn!(
                    target: "machbus.network",
                    sa = %format_args!("0x{src:02X}"),
                    "address violation detected",
                );
                emitted.extend(claimer.handle_request_for_claim(icf));
                violated = true;
            }
        }
        if violated {
            self.on_address_violation.emit(&src);
            self.send_frames_best_effort(&emitted, port);
        }
    }

    fn handle_request_for_address_claim(&mut self, frame: &Frame, port: u8) -> bool {
        let Some(requested_pgn) = decode_request(frame.payload()) else {
            return false;
        };
        if requested_pgn != PGN_ADDRESS_CLAIMED {
            return false;
        }

        let requested_destination = frame.destination();
        let mut emitted: Vec<Frame> = Vec::new();
        for (icf, claimer) in self.internal_cfs.iter_mut().zip(self.claimers.iter_mut()) {
            if icf.port() != port {
                continue;
            }
            if requested_destination != BROADCAST_ADDRESS && icf.address() != requested_destination
            {
                continue;
            }
            emitted.extend(claimer.handle_request_for_claim(icf));
        }
        for partner in &mut self.partner_cfs {
            if partner.port() != port || !partner.cf().is_online() {
                continue;
            }
            if requested_destination != BROADCAST_ADDRESS
                && partner.address() != requested_destination
            {
                continue;
            }
            partner.begin_claim_validation();
        }
        self.send_frames_best_effort(&emitted, port);
        true
    }

    fn handle_address_claim(&mut self, frame: &Frame, port: u8) {
        let claimed_name = match Name::from_bytes(frame.payload()) {
            Some(n) => n,
            None => return,
        };
        let claimed_addr = frame.source();
        if claimed_addr == BROADCAST_ADDRESS {
            return;
        }
        tracing::debug!(
            target: "machbus.network.claim",
            addr = %format_args!("0x{claimed_addr:02X}"),
            name = %format_args!("0x{:016X}", claimed_name.raw),
            "address claim received",
        );

        if claimed_addr <= MAX_ADDRESS {
            self.handle_duplicate_internal_name(claimed_name, claimed_addr, port);

            // Notify our claimers.
            let mut emitted: Vec<Frame> = Vec::new();
            for (icf, claimer) in self.internal_cfs.iter_mut().zip(self.claimers.iter_mut()) {
                if icf.port() == port {
                    emitted.extend(claimer.handle_claim(icf, claimed_addr, claimed_name));
                }
            }
            self.send_frames_best_effort(&emitted, port);
        }

        // Update partner CFs whose filters match.
        for partner in &mut self.partner_cfs {
            if partner.port() == port && partner.matches_name(&claimed_name) {
                if claimed_addr == NULL_ADDRESS {
                    partner.note_cannot_claim_seen(claimed_name);
                } else {
                    partner.note_address_claim_seen(claimed_name, claimed_addr);
                }
            }
        }
    }

    fn handle_duplicate_internal_name(
        &mut self,
        claimed_name: Name,
        claimed_addr: Address,
        port: u8,
    ) {
        if claimed_addr == NULL_ADDRESS || claimed_addr == BROADCAST_ADDRESS {
            return;
        }

        let mut emitted: Vec<Frame> = Vec::new();
        let mut duplicate_events: Vec<(Name, Address)> = Vec::new();
        for (icf, claimer) in self.internal_cfs.iter_mut().zip(self.claimers.iter_mut()) {
            if icf.port() != port
                || icf.name() != claimed_name
                || !claimer.has_attempted_claim()
                || icf.claim_state() == ClaimState::Failed
                || icf.address() == claimed_addr
            {
                continue;
            }

            duplicate_events.push((claimed_name, claimed_addr));
            emitted.extend(claimer.handle_duplicate_name(icf));
        }

        for event in &duplicate_events {
            self.on_duplicate_name.emit(event);
        }
        self.send_frames_best_effort(&emitted, port);
    }

    fn dispatch_message(&mut self, msg: &Message) {
        if self.capture_messages {
            self.captured_messages.push_back(msg.clone());
        }
        self.on_message.emit(msg);
        if let Some(callbacks) = self.pgn_callbacks.get_mut(&msg.pgn) {
            for cb in callbacks {
                cb(msg);
            }
        }
    }

    fn port_for_address(&self, addr: Address) -> u8 {
        for icf in &self.internal_cfs {
            if icf.address() == addr {
                return icf.port();
            }
        }
        0
    }
}

