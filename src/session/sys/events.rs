//! Unified event stream for the `session` facade.
//!
//! The pump-style codecs each fire their own per-subsystem
//! [`crate::net::Event<T>`] callbacks. The session core captures those,
//! classifies them by subsystem, and re-emits them through the single public
//! [`Event`] enum so user code only has one match site to maintain.

use alloc::{collections::VecDeque, vec::Vec};
use core::task::Waker;

use crate::net::types::{Address, Pgn};

/// Top-level event from any subsystem, surfaced by
/// [`Session::poll_event`](crate::session::Session::poll_event).
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Address-claim lifecycle.
    AddressClaim(ClaimEvent),
    /// Bus-level events: errors, bus-off, bus-on, port up/down.
    Bus(BusEvent),
    /// Diagnostics — DM1 receive, DTC raise/clear.
    Diag(super::diag::DiagEvent),
    /// GNSS / NMEA — position, COG/SOG, attitude.
    Gnss(super::gnss::GnssEvent),
    /// Automatic guidance / autosteer — steering machine info (ISO 11783-7).
    Guidance(super::guidance::GuidanceEvent),
    /// Virtual Terminal — connection state, soft-key/button activations,
    /// numeric/string value changes.
    Vt(super::vt::VtEvent),
    /// Task Controller — connection state, value requests/commands.
    Tc(super::tc::TcEvent),
    /// File Server — open/close/read/write responses.
    Fs(super::fs::FsEvent),
    /// Implement messages — hitch / PTO / aux-valve commands.
    Imp(super::imp::ImplementEvent),
    /// ISO 11783-14 Sequence Control master/client lifecycle.
    Sc(super::sc::ScEvent),
    /// ISO 11783 AUX-O/AUX-N auxiliary input status.
    Auxiliary(super::auxiliary::AuxiliaryEvent),
    /// Tractor Implement Management authority, commands, and status.
    Tim(super::tim::TimEvent),
    /// Shortcut Button / ISB status.
    ShortcutButton(super::shortcut_button::ShortcutButtonEvent),
    /// ISO 11783-9 Maintain Power lifecycle/status.
    MaintainPower(super::maintain_power::MaintainPowerEvent),
    /// ISO 11783-7 heartbeat send/receive/miss workflow.
    Heartbeat(super::heartbeat::HeartbeatEvent),
    /// J1939 / ISO Language Command preferences.
    LanguageCommand(super::language_command::LanguageCommandEvent),
    /// J1939 engine, transmission, and powertrain status/identification.
    Powertrain(super::powertrain::PowertrainEvent),
    /// J1939 DM14/DM15/DM16 memory access and ECU Identification.
    DmMemory(super::dm_memory::DmMemoryEvent),
    /// VT *server* events — client connect/disconnect, value pushes
    /// from clients, soft-key activations.
    VtServer(super::vt_server::VtServerEvent),
    /// FS *server* events — client lifecycle and file operations.
    FsServer(super::fs_server::FsServerEvent),
    /// TC *server* events — server FSM transitions.
    TcServer(super::tc_server::TcServerEvent),
    /// Catch-all for inbound PGNs that no subsystem claimed. Useful
    /// for diagnostic dumps or when running the session core in pure
    /// pass-through mode.
    Custom {
        pgn: Pgn,
        source: Address,
        data: Vec<u8>,
    },
}

/// Address-claim outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimEvent {
    /// CF won the claim and now owns this address.
    Claimed { address: Address },
    /// CF lost arbitration and is searching for a new address.
    Lost { previous_address: Address },
    /// Final shutdown / disconnect.
    Disconnected,
}

/// Bus-level events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusEvent {
    /// Endpoint on `port` reported a transient error.
    Error { port: u8 },
    /// Inbound frame discarded (malformed, wrong DLC, etc.).
    DroppedFrame { port: u8 },
    /// `port`'s CAN controller changed error-confinement action (normal /
    /// degrade / fail-safe) — ISO 11783-2/-3 fault confinement.
    ConfinementChanged {
        port: u8,
        action: crate::net::fault_confinement::FaultConfinementAction,
    },
}

/// Bounded event queue used by [`Session::poll_event`].
///
/// Default capacity is 256 events. When full, the oldest event is
/// dropped to make room for the newest — matches the C++ pump-style
/// behavior where inbound bursts can outrun the consumer briefly.
///
/// [`Session::poll_event`]: crate::session::Session::poll_event
#[derive(Debug)]
pub struct EventQueue {
    inner: VecDeque<Event>,
    capacity: usize,
    overflow: OverflowPolicy,
    dropped: u64,
    waker: Option<Waker>,
}

impl Event {
    /// `true` if this event came from the address-claim FSM.
    #[must_use]
    pub const fn is_address_claim(&self) -> bool {
        matches!(self, Self::AddressClaim(_))
    }

    /// `true` if this is a bus-level event.
    #[must_use]
    pub const fn is_bus(&self) -> bool {
        matches!(self, Self::Bus(_))
    }

    /// `true` if this is a diagnostics event.
    #[must_use]
    pub const fn is_diag(&self) -> bool {
        matches!(self, Self::Diag(_))
    }

    /// `true` if this is a GNSS event.
    #[must_use]
    pub const fn is_gnss(&self) -> bool {
        matches!(self, Self::Gnss(_))
    }

    /// `true` if this is a VT event.
    #[must_use]
    pub const fn is_vt(&self) -> bool {
        matches!(self, Self::Vt(_))
    }

    /// `true` if this is a TC event.
    #[must_use]
    pub const fn is_tc(&self) -> bool {
        matches!(self, Self::Tc(_))
    }

    /// `true` if this is an FS event.
    #[must_use]
    pub const fn is_fs(&self) -> bool {
        matches!(self, Self::Fs(_))
    }

    /// `true` if this is an implement-message event.
    #[must_use]
    pub const fn is_imp(&self) -> bool {
        matches!(self, Self::Imp(_))
    }

    /// `true` if this is a Sequence Control event.
    #[must_use]
    pub const fn is_sc(&self) -> bool {
        matches!(self, Self::Sc(_))
    }

    /// `true` if this is an AUX-O/AUX-N auxiliary-input event.
    #[must_use]
    pub const fn is_auxiliary(&self) -> bool {
        matches!(self, Self::Auxiliary(_))
    }

    /// `true` if this is a TIM event.
    #[must_use]
    pub const fn is_tim(&self) -> bool {
        matches!(self, Self::Tim(_))
    }

    /// `true` if this is a Shortcut Button / ISB event.
    #[must_use]
    pub const fn is_shortcut_button(&self) -> bool {
        matches!(self, Self::ShortcutButton(_))
    }

    /// `true` if this is a Maintain Power event.
    #[must_use]
    pub const fn is_maintain_power(&self) -> bool {
        matches!(self, Self::MaintainPower(_))
    }

    /// `true` if this is a heartbeat event.
    #[must_use]
    pub const fn is_heartbeat(&self) -> bool {
        matches!(self, Self::Heartbeat(_))
    }

    /// `true` if this is a Language Command event.
    #[must_use]
    pub const fn is_language_command(&self) -> bool {
        matches!(self, Self::LanguageCommand(_))
    }

    /// `true` if this is a J1939 engine/transmission/powertrain event.
    #[must_use]
    pub const fn is_powertrain(&self) -> bool {
        matches!(self, Self::Powertrain(_))
    }

    /// `true` if this is a DM memory / ECU identification event.
    #[must_use]
    pub const fn is_dm_memory(&self) -> bool {
        matches!(self, Self::DmMemory(_))
    }

    /// `true` if this is a VT *server* event.
    #[must_use]
    pub const fn is_vt_server(&self) -> bool {
        matches!(self, Self::VtServer(_))
    }

    /// `true` if this is an FS *server* event.
    #[must_use]
    pub const fn is_fs_server(&self) -> bool {
        matches!(self, Self::FsServer(_))
    }

    /// `true` if this is a TC *server* event.
    #[must_use]
    pub const fn is_tc_server(&self) -> bool {
        matches!(self, Self::TcServer(_))
    }

    /// `true` if this is an unrouted PGN.
    #[must_use]
    pub const fn is_custom(&self) -> bool {
        matches!(self, Self::Custom { .. })
    }

    /// PGN that triggered this event, if any. `AddressClaim`, `Bus`,
    /// and most subsystem events return `None` since they don't carry
    /// a single canonical PGN.
    #[must_use]
    pub fn pgn(&self) -> Option<Pgn> {
        match self {
            Self::Custom { pgn, .. } => Some(*pgn),
            _ => None,
        }
    }
}

/// What to do when the queue fills up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowPolicy {
    /// Drop the oldest event to make room (default).
    #[default]
    DropOldest,
    /// Drop the new event; counter increments.
    DropNewest,
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::with_capacity(256)
    }
}

impl EventQueue {
    /// Build a queue with the given capacity and default
    /// (`DropOldest`) overflow policy.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: VecDeque::with_capacity(capacity),
            capacity,
            overflow: OverflowPolicy::DropOldest,
            dropped: 0,
            waker: None,
        }
    }

    /// Override the overflow policy.
    #[must_use]
    pub const fn with_policy(mut self, policy: OverflowPolicy) -> Self {
        self.overflow = policy;
        self
    }

    /// Push an event. If the queue is full, applies the overflow
    /// policy and increments [`Self::dropped_count`].
    pub fn push(&mut self, ev: Event) {
        let mut accepted = false;
        if self.capacity == 0 {
            self.dropped += 1;
        } else if self.inner.len() >= self.capacity {
            match self.overflow {
                OverflowPolicy::DropOldest => {
                    self.inner.pop_front();
                    self.dropped += 1;
                    self.inner.push_back(ev);
                    accepted = true;
                }
                OverflowPolicy::DropNewest => {
                    self.dropped += 1;
                }
            }
        } else {
            self.inner.push_back(ev);
            accepted = true;
        }

        if accepted && let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    /// Drain one event in FIFO order.
    pub fn pop(&mut self) -> Option<Event> {
        self.inner.pop_front()
    }

    /// Register the task that should be woken when the next accepted
    /// event is pushed into an empty queue.
    ///
    /// Only the most recent async stream poll is retained. This matches the
    /// stack's single-threaded `Rc<RefCell<_>>` design: the queue is a local
    /// event source, not a multi-consumer async channel.
    #[allow(dead_code)]
    pub(crate) fn register_waker(&mut self, waker: &core::task::Waker) {
        if self.inner.is_empty() {
            match &self.waker {
                Some(existing) if existing.will_wake(waker) => {}
                _ => self.waker = Some(waker.clone()),
            }
        }
    }

    /// `true` iff no events are queued.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Number of events currently queued.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Capacity (max in-flight events).
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Total events dropped due to overflow since construction.
    #[inline]
    #[must_use]
    pub const fn dropped_count(&self) -> u64 {
        self.dropped
    }

    /// Discard every queued event without firing them. The drop
    /// counter is preserved.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(addr: u8) -> Event {
        Event::AddressClaim(ClaimEvent::Claimed { address: addr })
    }

    #[test]
    fn fifo_order() {
        let mut q = EventQueue::with_capacity(8);
        q.push(ev(1));
        q.push(ev(2));
        q.push(ev(3));
        assert_eq!(q.len(), 3);
        assert_eq!(q.pop(), Some(ev(1)));
        assert_eq!(q.pop(), Some(ev(2)));
        assert_eq!(q.pop(), Some(ev(3)));
        assert!(q.pop().is_none());
        assert!(q.is_empty());
    }

    #[test]
    fn default_capacity_is_256() {
        let q = EventQueue::default();
        assert_eq!(q.capacity(), 256);
        assert_eq!(q.len(), 0);
        assert_eq!(q.dropped_count(), 0);
    }

    #[test]
    fn drop_oldest_is_default() {
        let mut q = EventQueue::with_capacity(2);
        q.push(ev(1));
        q.push(ev(2));
        q.push(ev(3)); // evicts 1
        assert_eq!(q.dropped_count(), 1);
        assert_eq!(q.pop(), Some(ev(2)));
        assert_eq!(q.pop(), Some(ev(3)));
    }

    #[test]
    fn drop_newest_keeps_initial_events() {
        let mut q = EventQueue::with_capacity(2).with_policy(OverflowPolicy::DropNewest);
        q.push(ev(1));
        q.push(ev(2));
        q.push(ev(3)); // dropped
        assert_eq!(q.dropped_count(), 1);
        assert_eq!(q.pop(), Some(ev(1)));
        assert_eq!(q.pop(), Some(ev(2)));
    }

    #[test]
    fn zero_capacity_drops_every_event() {
        let mut q = EventQueue::with_capacity(0);
        q.push(ev(1));
        q.push(ev(2));

        assert_eq!(q.len(), 0);
        assert_eq!(q.capacity(), 0);
        assert_eq!(q.dropped_count(), 2);
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn zero_capacity_drop_newest_also_drops_every_event() {
        let mut q = EventQueue::with_capacity(0).with_policy(OverflowPolicy::DropNewest);
        q.push(ev(1));

        assert_eq!(q.len(), 0);
        assert_eq!(q.dropped_count(), 1);
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn capacity_one_drop_oldest_keeps_latest_event() {
        let mut q = EventQueue::with_capacity(1);
        q.push(ev(1));
        q.push(ev(2));
        q.push(ev(3));

        assert_eq!(q.capacity(), 1);
        assert_eq!(q.len(), 1);
        assert_eq!(q.dropped_count(), 2);
        assert_eq!(q.pop(), Some(ev(3)));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn capacity_one_drop_newest_keeps_first_event() {
        let mut q = EventQueue::with_capacity(1).with_policy(OverflowPolicy::DropNewest);
        q.push(ev(1));
        q.push(ev(2));
        q.push(ev(3));

        assert_eq!(q.capacity(), 1);
        assert_eq!(q.len(), 1);
        assert_eq!(q.dropped_count(), 2);
        assert_eq!(q.pop(), Some(ev(1)));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn large_drop_oldest_burst_keeps_newest_capacity_events_in_order() {
        let mut q = EventQueue::with_capacity(64);
        for addr in 0..200 {
            q.push(ev(addr));
        }

        assert_eq!(q.len(), 64);
        assert_eq!(q.dropped_count(), 136);
        for addr in 136..200 {
            assert_eq!(q.pop(), Some(ev(addr)));
        }
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn large_drop_newest_burst_keeps_first_capacity_events_in_order() {
        let mut q = EventQueue::with_capacity(64).with_policy(OverflowPolicy::DropNewest);
        for addr in 0..200 {
            q.push(ev(addr));
        }

        assert_eq!(q.len(), 64);
        assert_eq!(q.dropped_count(), 136);
        for addr in 0..64 {
            assert_eq!(q.pop(), Some(ev(addr)));
        }
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn clear_keeps_drop_counter() {
        let mut q = EventQueue::with_capacity(1);
        q.push(ev(1));
        q.push(ev(2)); // drops 1
        q.clear();
        assert_eq!(q.dropped_count(), 1);
        assert!(q.is_empty());
    }

    #[test]
    fn custom_event_carries_payload() {
        let ev = Event::Custom {
            pgn: 0xCAFE,
            source: 0x42,
            data: vec![1, 2, 3],
        };
        let mut q = EventQueue::default();
        q.push(ev.clone());
        assert_eq!(q.pop(), Some(ev));
    }
}
