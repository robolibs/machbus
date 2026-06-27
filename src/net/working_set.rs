//! Working Set Protocol (ISO 11783-7 §10).
//!
//! Mirrors the C++ `machbus::net::WorkingSetManager`, **refactored to
//! have no [`IsoNet`] dependency** — instead the manager exposes
//! [`WorkingSetManager::start_broadcast`] (returns the master payload),
//! [`WorkingSetManager::update`] (returns the next due member message),
//! and [`WorkingSetManager::handle_master`] /
//! [`WorkingSetManager::handle_member`] for incoming bytes.
//!
//! Phase 6 (`net::network_manager`) wires this manager to the actual
//! send/receive plumbing.
//!
//! [`IsoNet`]: https://example.invalid/machbus/net/IsoNet
//!
//! Per §10.2 the master broadcasts the working-set master message
//! immediately, then `set_size − 1` member messages spaced **at least
//! 100 ms** apart.

use std::collections::HashMap;
use std::collections::VecDeque;

use super::event::Event;
use super::name::Name;
use super::types::Address;

/// 100 ms minimum spacing between member messages (ISO 11783-7 §10.2).
pub const MEMBER_MSG_INTERVAL_MS: u32 = 100;

/// Working Set master and remote-set tracker.
pub struct WorkingSetManager {
    members: Vec<Name>,
    remote_sets: HashMap<Address, Vec<Name>>,

    pending_members: VecDeque<Name>,
    member_timer_ms: u32,
    broadcasting: bool,

    /// Fires `(source, master_name)` on receipt of a master message.
    /// `master_name` is left at default; callers correlate with their
    /// address-claim cache to fill in the actual NAME.
    pub on_master_received: Event<(Address, Name)>,
    /// Fires `(source, member_name)` for each member announcement.
    pub on_member_received: Event<(Address, Name)>,
}

impl Default for WorkingSetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkingSetManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
            remote_sets: HashMap::new(),
            pending_members: VecDeque::new(),
            member_timer_ms: 0,
            broadcasting: false,
            on_master_received: Event::new(),
            on_member_received: Event::new(),
        }
    }

    // ─── Local set management ──────────────────────────────────────
    pub fn add_member(&mut self, member_name: Name) {
        self.members.push(member_name);
    }

    pub fn clear_members(&mut self) {
        self.members.clear();
    }

    /// Remove the first member equal to `member_name`. Returns `true` if one was
    /// removed (e.g. when an implement leaves the working set).
    pub fn remove_member(&mut self, member_name: Name) -> bool {
        if let Some(idx) = self.members.iter().position(|m| *m == member_name) {
            self.members.remove(idx);
            true
        } else {
            false
        }
    }

    /// Whether `member_name` is currently in the local working set.
    #[must_use]
    pub fn is_member(&self, member_name: Name) -> bool {
        self.members.contains(&member_name)
    }

    #[must_use]
    pub fn members(&self) -> &[Name] {
        &self.members
    }

    /// Total set size = master + members.
    #[must_use]
    pub fn set_size(&self) -> usize {
        self.members.len() + 1
    }

    // ─── Broadcasting ──────────────────────────────────────────────

    /// Begin a working-set broadcast. Returns the 8-byte
    /// master-message payload (byte 0 = `set_size`, rest `0xFF`);
    /// the caller is expected to send it immediately on
    /// `PGN_WORKING_SET_MASTER`.
    ///
    /// Subsequent member messages are emitted by [`Self::update`] at
    /// 100 ms cadence.
    pub fn start_broadcast(&mut self) -> [u8; 8] {
        let mut payload = [0xFFu8; 8];
        payload[0] = self.set_size() as u8;

        self.pending_members = self.members.iter().copied().collect();
        self.member_timer_ms = 0;
        self.broadcasting = !self.pending_members.is_empty();

        tracing::debug!(
            target: "machbus.network.working_set",
            size = self.set_size(),
            queued = self.pending_members.len(),
            "broadcast queued",
        );
        payload
    }

    /// Advance the 100 ms member-message pacing. Returns the next
    /// member NAME and its 8-byte payload when due, otherwise `None`.
    ///
    /// At most one member is returned per call (matches the C++
    /// behavior, which assumes `update` is called more often than
    /// the interval).
    pub fn update(&mut self, elapsed_ms: u32) -> Option<(Name, [u8; 8])> {
        if !self.broadcasting || self.pending_members.is_empty() {
            return None;
        }
        self.member_timer_ms = self.member_timer_ms.saturating_add(elapsed_ms);
        if self.member_timer_ms < MEMBER_MSG_INTERVAL_MS {
            return None;
        }
        self.member_timer_ms -= MEMBER_MSG_INTERVAL_MS;

        let member = self.pending_members.pop_front()?;
        if self.pending_members.is_empty() {
            self.broadcasting = false;
            tracing::debug!(target: "machbus.network.working_set", "broadcast complete");
        }
        Some((member, member.to_bytes()))
    }

    #[inline]
    #[must_use]
    pub const fn is_broadcasting(&self) -> bool {
        self.broadcasting
    }

    // ─── Incoming message ingestion ────────────────────────────────

    /// Process an incoming Working-Set Master message.
    ///
    /// `data` must be the raw payload (≥ 1 byte). Resets the cached
    /// member list for `source`.
    pub fn handle_master(&mut self, source: Address, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let _announced_size = data[0];
        self.remote_sets.entry(source).or_default().clear();
        // Master NAME is unknown at this layer; the IsoNet glue will
        // resolve it from the address-claim cache.
        self.on_master_received.emit(&(source, Name::default()));
        tracing::debug!(
            target: "machbus.network.working_set",
            source = %format_args!("0x{:02X}", source),
            size = _announced_size,
            "master received",
        );
    }

    /// Process an incoming Working-Set Member message. `data` must
    /// hold ≥ 8 bytes of NAME.
    pub fn handle_member(&mut self, source: Address, data: &[u8]) {
        let Some(name) = Name::from_bytes(data) else {
            return;
        };
        self.remote_sets.entry(source).or_default().push(name);
        self.on_member_received.emit(&(source, name));
        tracing::debug!(
            target: "machbus.network.working_set",
            source = %format_args!("0x{:02X}", source),
            "member received",
        );
    }

    // ─── Remote-set queries ───────────────────────────────────────
    #[inline]
    #[must_use]
    pub fn remote_sets(&self) -> &HashMap<Address, Vec<Name>> {
        &self.remote_sets
    }

    #[must_use]
    pub fn get_remote_set(&self, master_addr: Address) -> Option<&[Name]> {
        self.remote_sets.get(&master_addr).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn member(id: u32) -> Name {
        Name::default().with_identity_number(id)
    }

    #[test]
    fn empty_set_broadcasts_size_one_and_emits_no_members() {
        let mut ws = WorkingSetManager::new();
        let payload = ws.start_broadcast();
        assert_eq!(payload[0], 1); // master only
        assert!(!ws.is_broadcasting());
        // No update should ever return a member.
        assert!(ws.update(1_000).is_none());
    }

    #[test]
    fn members_can_be_added_queried_and_removed() {
        let mut ws = WorkingSetManager::new();
        ws.add_member(member(0x10));
        ws.add_member(member(0x20));
        assert!(ws.is_member(member(0x10)));
        assert!(!ws.is_member(member(0x30)));
        assert_eq!(ws.set_size(), 3); // 2 members + master

        // Removing a present member succeeds; a missing one is a no-op.
        assert!(ws.remove_member(member(0x10)));
        assert!(!ws.is_member(member(0x10)));
        assert!(!ws.remove_member(member(0x10)));
        assert_eq!(ws.set_size(), 2);
        assert_eq!(ws.members(), &[member(0x20)]);
    }

    #[test]
    fn member_messages_paced_at_100ms() {
        let mut ws = WorkingSetManager::new();
        ws.add_member(member(0x10));
        ws.add_member(member(0x20));
        let payload = ws.start_broadcast();
        assert_eq!(payload[0], 3); // 2 members + master
        assert!(ws.is_broadcasting());

        // Until 100 ms elapses no member is due.
        assert!(ws.update(50).is_none());
        let (n1, _) = ws.update(60).expect("first member due");
        assert_eq!(n1, member(0x10));

        // Need another full 100 ms for the next.
        assert!(ws.update(50).is_none());
        let (n2, payload2) = ws.update(60).expect("second member due");
        assert_eq!(n2, member(0x20));
        assert_eq!(payload2, member(0x20).to_bytes());

        assert!(!ws.is_broadcasting());
        assert!(ws.update(1_000).is_none());
    }

    #[test]
    fn handle_master_resets_remote_set() {
        let mut ws = WorkingSetManager::new();
        let log = Rc::new(RefCell::new(Vec::<(Address, Name)>::new()));
        let l = log.clone();
        ws.on_master_received
            .subscribe(move |t| l.borrow_mut().push(*t));

        // Pretend a previous member announcement existed.
        ws.handle_member(0x42, &member(0x99).to_bytes());
        assert_eq!(ws.get_remote_set(0x42), Some(&[member(0x99)][..]));

        // New master from same source clears the list.
        ws.handle_master(0x42, &[3, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(ws.get_remote_set(0x42), Some(&[][..]));
        assert_eq!(log.borrow().len(), 1);
    }

    #[test]
    fn handle_member_appends_to_remote_set() {
        let mut ws = WorkingSetManager::new();
        ws.handle_member(0x10, &member(0x01).to_bytes());
        ws.handle_member(0x10, &member(0x02).to_bytes());
        assert_eq!(
            ws.get_remote_set(0x10),
            Some(&[member(0x01), member(0x02)][..])
        );
    }

    #[test]
    fn handle_master_with_empty_data_is_noop() {
        let mut ws = WorkingSetManager::new();
        ws.handle_master(0x10, &[]);
        assert!(ws.remote_sets().is_empty());
    }

    #[test]
    fn handle_member_with_short_data_is_noop() {
        let mut ws = WorkingSetManager::new();
        ws.handle_member(0x10, &[0u8; 7]);
        assert!(ws.remote_sets().is_empty());
    }

    #[test]
    fn member_overshoot_preserved_across_intervals() {
        let mut ws = WorkingSetManager::new();
        ws.add_member(member(0x01));
        ws.add_member(member(0x02));
        let _ = ws.start_broadcast();

        // 150 ms in one shot: emits one (overshoot 50 ms preserved).
        assert!(ws.update(150).is_some());
        // Only 50 ms remaining toward the next interval.
        assert!(ws.update(40).is_none());
        assert!(ws.update(20).is_some());
    }
}
