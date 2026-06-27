//! Minimal `no_std + alloc` session facade.
//!
//! This is the embedded side of the public `machbus::session` module. It keeps
//! the same caller-driven shape as the hosted session: feed received CAN frames,
//! tick explicit monotonic time, drain outgoing frames, and drain events. The
//! richer hosted plugin set remains `std`-only while its ISOBUS/NMEA storage and
//! adapter dependencies are split.

use alloc::{collections::VecDeque, vec::Vec};

#[cfg(feature = "embedded")]
use crate::fixed::{FixedCapacityError, FixedMessage};
use crate::net::can_adapter;
use crate::net::{
    Address, ClaimState, Error, Frame, InternalCfHandle, IsoNet, Message, NULL_ADDRESS, Name,
    NetworkConfig, Priority, Result,
};
use crate::time::Instant;

/// Link marker used by the embedded sans-IO session.
///
/// A [`Session`] runs [`IsoNet`] in outbound-capture mode with no attached
/// endpoints. Received frames are supplied through [`Session::feed`], and
/// transmit frames are drained through [`Session::poll_transmit`].
pub struct NullLink;

impl can_adapter::Link for NullLink {
    fn send(&mut self, _frame: &can_adapter::Frame) -> can_adapter::Result<()> {
        Ok(())
    }

    fn recv(&mut self) -> can_adapter::Result<can_adapter::Frame> {
        Err(can_adapter::Error::Empty)
    }

    fn can_send(&self) -> bool {
        false
    }

    fn can_recv(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "embedded-null"
    }
}

/// Events surfaced by the embedded session facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The local control function claimed a source address.
    AddressClaimed { address: Address },
    /// An application-layer message was decoded from a single-frame,
    /// TP/ETP-completed, or Fast Packet-completed receive path.
    Message(Message),
}

/// Fixed-capacity event view for the `embedded-fixed` profile.
///
/// The embedded session still uses the heap-backed `no_std + alloc` network
/// core internally, but this lets callers drain bounded events without carrying
/// an allocation-backed [`Message`] past the session boundary. Oversized
/// messages are reported explicitly instead of being truncated.
#[cfg(feature = "embedded")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedEvent<const N: usize> {
    /// The local control function claimed a source address.
    AddressClaimed { address: Address },
    /// A decoded message whose payload fits in the fixed event capacity.
    Message(FixedMessage<N>),
    /// A decoded message exceeded the fixed payload capacity.
    MessageOverflow {
        pgn: crate::net::Pgn,
        source: Address,
        destination: Address,
        payload_len: usize,
        capacity: usize,
    },
}

/// Deterministic, caller-driven embedded protocol/session core.
pub struct Session {
    net: IsoNet<NullLink>,
    cf: InternalCfHandle,
    events: VecDeque<Event>,
    last_tick: Option<Instant>,
    last_claim: ClaimState,
}

impl Session {
    /// Start building a session for control function `name` preferring
    /// `preferred` as source address.
    #[must_use]
    pub fn builder(name: Name, preferred: Address) -> SessionBuilder {
        SessionBuilder::new(name, preferred)
    }

    /// Begin address claiming. Drive the claim forward with [`Self::tick`].
    pub fn start(&mut self) -> Result<()> {
        self.net.start_address_claiming()
    }

    /// Current source address (`NULL_ADDRESS` until the claim completes).
    #[must_use]
    pub fn address(&self) -> Address {
        self.net
            .internal_cf(self.cf)
            .map_or(NULL_ADDRESS, crate::net::InternalCf::address)
    }

    /// Current address-claim state.
    #[must_use]
    pub fn claim_state(&self) -> ClaimState {
        self.net
            .internal_cf(self.cf)
            .map_or(ClaimState::None, crate::net::InternalCf::claim_state)
    }

    /// Whether the local control function has claimed an address.
    #[must_use]
    pub fn is_claimed(&self) -> bool {
        self.claim_state() == ClaimState::Claimed
    }

    /// Feed one received frame on `port`, stamped with caller-supplied `now`.
    pub fn feed(&mut self, port: u8, frame: &Frame, now: Instant) {
        self.advance_time(now);
        self.net.feed(frame, port);
        self.route_inbox();
        self.detect_claim();
    }

    /// Advance all protocol timers to caller-supplied `now`.
    pub fn tick(&mut self, now: Instant) {
        self.advance_time(now);
        self.route_inbox();
        self.detect_claim();
    }

    /// Drain the next `(port, frame)` the stack wants to transmit.
    pub fn poll_transmit(&mut self) -> Option<(u8, Frame)> {
        self.net.take_outbound()
    }

    /// Drain the next embedded session event.
    pub fn poll_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Drain the next event as a fixed-capacity event.
    ///
    /// This is available in the `embedded-fixed` profile. It consumes one
    /// queued event. Message payloads larger than `N` produce
    /// [`FixedEvent::MessageOverflow`] so the caller can count/drop/escalate the
    /// condition without heap growth or silent truncation.
    #[cfg(feature = "embedded")]
    pub fn poll_fixed_event<const N: usize>(&mut self) -> Option<FixedEvent<N>> {
        self.poll_event().map(|event| match event {
            Event::AddressClaimed { address } => FixedEvent::AddressClaimed { address },
            Event::Message(message) => fixed_event_from_message(&message),
        })
    }

    /// Send an application-layer message from the local control function.
    pub fn send_raw(
        &mut self,
        pgn: crate::net::Pgn,
        data: &[u8],
        dst: Address,
        priority: Priority,
    ) -> Result<()> {
        self.net.send(pgn, data, self.cf, dst, priority)
    }

    /// Register a NMEA2000 Fast Packet PGN so inbound frames are reassembled
    /// before surfacing as [`Event::Message`].
    pub fn register_fast_packet_pgn(&mut self, pgn: crate::net::Pgn) -> Result<()> {
        self.net.register_fast_packet_pgn(pgn)
    }

    /// Mutable access to the embedded network core for lower-level tuning.
    pub fn network_mut(&mut self) -> &mut IsoNet<NullLink> {
        &mut self.net
    }

    fn advance_time(&mut self, now: Instant) {
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        if self.last_tick.is_none() || elapsed > 0 {
            self.net.update(elapsed);
        }
        self.last_tick = Some(now);
    }

    fn route_inbox(&mut self) {
        while let Some(msg) = self.net.take_message() {
            self.events.push_back(Event::Message(msg));
        }
    }

    fn detect_claim(&mut self) {
        let state = self.claim_state();
        if state != self.last_claim {
            if state == ClaimState::Claimed {
                self.events.push_back(Event::AddressClaimed {
                    address: self.address(),
                });
            }
            self.last_claim = state;
        }
    }
}

#[cfg(feature = "embedded")]
fn fixed_event_from_message<const N: usize>(message: &Message) -> FixedEvent<N> {
    match FixedMessage::<N>::from_message(message) {
        Ok(fixed) => FixedEvent::Message(fixed),
        Err(FixedCapacityError { capacity, .. }) => FixedEvent::MessageOverflow {
            pgn: message.pgn,
            source: message.source,
            destination: message.destination,
            payload_len: message.data.len(),
            capacity,
        },
    }
}

/// Builder for an embedded [`Session`].
pub struct SessionBuilder {
    name: Name,
    preferred: Address,
    config: NetworkConfig,
    fast_packet_pgns: Vec<crate::net::Pgn>,
}

impl SessionBuilder {
    fn new(name: Name, preferred: Address) -> Self {
        Self {
            name,
            preferred,
            config: NetworkConfig::default(),
            fast_packet_pgns: Vec::new(),
        }
    }

    /// Override the network configuration.
    #[must_use]
    pub fn network_config(mut self, config: NetworkConfig) -> Self {
        self.config = config;
        self
    }

    /// Register a NMEA2000 Fast Packet PGN at build time.
    #[must_use]
    pub fn fast_packet_pgn(mut self, pgn: crate::net::Pgn) -> Self {
        self.fast_packet_pgns.push(pgn);
        self
    }

    /// Finalize the embedded session.
    pub fn build(self) -> Result<Session> {
        let mut net = IsoNet::<NullLink>::new(self.config);
        net.set_capture_outbound(true);
        net.set_capture_messages(true);
        let cf = net.create_internal(self.name, 0, self.preferred)?;
        for pgn in self.fast_packet_pgns {
            net.register_fast_packet_pgn(pgn)?;
        }

        Ok(Session {
            net,
            cf,
            events: VecDeque::new(),
            last_tick: None,
            last_claim: ClaimState::None,
        })
    }
}

/// Non-blocking embedded CAN boundary.
pub use crate::net::CanTransport as Transport;

/// Driver that pumps a [`Session`] with a caller-supplied transport and time.
pub struct Driver<T: Transport> {
    session: Session,
    transport: T,
}

impl<T> Driver<T>
where
    T: Transport,
    T::Error: Into<Error>,
{
    #[must_use]
    pub fn new(session: Session, transport: T) -> Self {
        Self { session, transport }
    }

    /// Mutable access to the owned session.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Pump once at explicit caller-supplied time.
    pub fn poll_at(&mut self, now: Instant) -> Result<Option<Event>> {
        while let Some((port, frame)) = self.transport.recv() {
            self.session.feed(port, &frame, now);
        }
        self.session.tick(now);
        while let Some((port, frame)) = self.session.poll_transmit() {
            self.transport.send(port, &frame).map_err(Into::into)?;
        }
        Ok(self.session.poll_event())
    }

    /// Pump once at explicit caller-supplied time and drain a fixed-capacity
    /// event view.
    #[cfg(feature = "embedded")]
    pub fn poll_fixed_at<const N: usize>(&mut self, now: Instant) -> Result<Option<FixedEvent<N>>> {
        while let Some((port, frame)) = self.transport.recv() {
            self.session.feed(port, &frame, now);
        }
        self.session.tick(now);
        while let Some((port, frame)) = self.session.poll_transmit() {
            self.transport.send(port, &frame).map_err(Into::into)?;
        }
        Ok(self.session.poll_fixed_event())
    }
}
