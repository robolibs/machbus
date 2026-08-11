//! ISO 11783-5 §4.4.2 / §4.4.3 address-claim state machine.
//!
//! Mirrors the C++ `machbus::net::AddressClaimer`. The claimer is
//! pure logic — every method takes `&mut InternalCf` and returns a
//! [`Vec<Frame>`] of frames to transmit. The caller (Phase-6 `IsoNet`)
//! handles the actual driver I/O.
//!
//! # Standard conformance
//!
//! - **§4.4.2:** A CF that has not yet attempted an address claim must
//!   not respond to any message except request-for-address-claimed.
//!   Tracked by [`AddressClaimer::has_attempted_claim`].
//! - **§4.4.3:** After yielding to a higher-priority NAME, the CF must
//!   wait an RTxD delay (`0.6 ms × random[0..=255]`, capped at
//!   [`ADDRESS_CLAIM_RTXD_MAX_MS`]) before re-claiming at a different
//!   address. Tracked by `reclaim_pending` / `reclaim_delay_timer_ms`.
//!
//! [`ADDRESS_CLAIM_RTXD_MAX_MS`]: super::constants::ADDRESS_CLAIM_RTXD_MAX_MS

use alloc::{vec, vec::Vec};

use super::constants::{
    ADDRESS_CLAIM_RTXD_MAX_MS, ADDRESS_CLAIM_TIMEOUT_MS, BROADCAST_ADDRESS, MAX_ADDRESS,
    NULL_ADDRESS,
};
use super::control_function::CfState;
use super::frame::Frame;
use super::identifier::Identifier;
use super::internal_cf::{ClaimState, InternalCf};
use super::name::Name;
use super::pgn_defs::{PGN_ADDRESS_CLAIMED, PGN_REQUEST};
use super::types::{Address, Priority};

const CLAIMABLE_ADDRESS_COUNT: usize = (MAX_ADDRESS as usize) + 1;

/// Self-configurable control functions select a source address from the
/// configurable range 128..=247 (ISO 11783-5). Addresses below 128 and the
/// 248..=253 service region are reserved for specific/assigned functions, so
/// the automatic fallback search stays inside this range.
const SELF_CONFIG_ADDRESS_MIN: Address = 128;
const SELF_CONFIG_ADDRESS_MAX: Address = 247;

/// Address-claim protocol driver.
pub struct AddressClaimer {
    /// Total guard window after sending claim (250 ms + RTxD).
    timeout_ms: u32,
    /// Random transmit delay before re-claim (`0..=153 ms`).
    rtxd_ms: u32,

    /// §4.4.2 — true once `start()` has been called.
    attempted_claim: bool,

    /// Time since the most recent claim was sent, in ms.
    claim_guard_timer_ms: u32,

    /// §4.4.2.4 — a cannot-claim response is queued, waiting for RTxD.
    ///
    /// "A random transmit delay (RTxD) shall be inserted between the reception
    /// of a message triggering the cannot-claim-source-address response and the
    /// sending of the response in order to minimize the possibility of two such
    /// responses causing bus errors." Answering immediately meant every
    /// address-failed CF on the segment replied to the same global request in
    /// the same instant.
    cannot_claim_pending: bool,
    cannot_claim_delay_timer_ms: u32,

    /// §4.5.1 a) — the initial listen window is open: the request has gone
    /// out and we are collecting the source addresses already in use before
    /// picking one.
    ///
    /// `start()` used to emit the request and the claim back to back, so the
    /// address table was still empty at claim time and the stack evicted
    /// whoever held its preferred address instead of choosing a free one.
    listen_pending: bool,
    listen_timer_ms: u32,

    /// §4.4.3 — re-claim queued, waiting for RTxD to elapse.
    reclaim_pending: bool,
    reclaim_delay_timer_ms: u32,
    reclaim_address: Address,

    /// Addresses observed in peer Address Claimed frames on this network.
    ///
    /// This keeps self-configurable reclaims from cycling onto addresses that
    /// are already known to be occupied when the preferred address loses
    /// arbitration.
    occupied_addresses: [bool; CLAIMABLE_ADDRESS_COUNT],
}

impl AddressClaimer {
    /// `rtxd_ms` should be `0.6 × random_byte` (`0..=153`). Use
    /// [`rtxd_for_name`] when no RNG is available.
    #[must_use]
    pub const fn new(rtxd_ms: u32) -> Self {
        Self::with_timeout(ADDRESS_CLAIM_TIMEOUT_MS, rtxd_ms)
    }

    /// Override the base 250 ms timeout (rare; mostly for tests).
    #[must_use]
    pub const fn with_timeout(base_timeout_ms: u32, rtxd_ms: u32) -> Self {
        let rtxd_ms = if rtxd_ms > ADDRESS_CLAIM_RTXD_MAX_MS {
            ADDRESS_CLAIM_RTXD_MAX_MS
        } else {
            rtxd_ms
        };
        Self {
            timeout_ms: base_timeout_ms + rtxd_ms,
            rtxd_ms,
            attempted_claim: false,
            claim_guard_timer_ms: 0,
            cannot_claim_pending: false,
            cannot_claim_delay_timer_ms: 0,
            listen_pending: false,
            listen_timer_ms: 0,
            reclaim_pending: false,
            reclaim_delay_timer_ms: 0,
            reclaim_address: NULL_ADDRESS,
            occupied_addresses: [false; CLAIMABLE_ADDRESS_COUNT],
        }
    }

    #[inline]
    #[must_use]
    pub const fn has_attempted_claim(&self) -> bool {
        self.attempted_claim
    }

    #[inline]
    #[must_use]
    pub const fn guard_timer(&self) -> u32 {
        self.claim_guard_timer_ms
    }

    /// Begin the claim sequence: emits a request-for-address-claimed
    /// then our claim at the preferred address, transitioning the CF
    /// to [`ClaimState::WaitForContest`].
    pub fn start(&mut self, cf: &mut InternalCf) -> Vec<Frame> {
        tracing::debug!(
            target: "machbus.network.claim",
            preferred = %format_args!("0x{:02X}", cf.preferred_address()),
            "starting address claim",
        );
        self.attempted_claim = true;
        self.occupied_addresses = [false; CLAIMABLE_ADDRESS_COUNT];

        if cf.preferred_address() > MAX_ADDRESS {
            tracing::error!(
                target: "machbus.network.claim",
                preferred = %format_args!("0x{:02X}", cf.preferred_address()),
                "preferred address is not claimable",
            );
            cf.set_state(CfState::Offline);
            cf.state_machine_mut().transition(ClaimState::Failed);
            cf.set_address(NULL_ADDRESS);
            self.reclaim_pending = false;
            self.reclaim_delay_timer_ms = 0;
            self.reclaim_address = NULL_ADDRESS;
            self.claim_guard_timer_ms = 0;
            return vec![make_claim_frame(cf.name(), NULL_ADDRESS)];
        }

        // §4.5.1 a): request for address claimed, then wait at least 250 ms
        // plus RTxD before claiming, so the address table reflects who is
        // already on the bus. The claim itself goes out from `update()`.
        cf.state_machine_mut().transition(ClaimState::SendRequest);
        self.listen_pending = true;
        self.listen_timer_ms = 0;
        cf.reset_claim_timer();
        self.claim_guard_timer_ms = 0;
        vec![make_request_frame()]
    }

    /// Whether the §4.5.1 a) listen window is still open.
    #[inline]
    #[must_use]
    pub const fn is_listening(&self) -> bool {
        self.listen_pending
    }

    /// Emit the initial claim once the listen window closes, choosing the
    /// preferred address when it is still free and the next unused one when
    /// somebody answered from it.
    fn finish_listen(&mut self, cf: &mut InternalCf) -> Vec<Frame> {
        self.listen_pending = false;
        self.listen_timer_ms = 0;

        let preferred = cf.preferred_address();
        let address = if self.is_occupied(preferred) {
            let next = self.find_next_address(cf, preferred);
            tracing::info!(
                target: "machbus.network.claim",
                preferred = %format_args!("0x{preferred:02X}"),
                chosen = %format_args!("0x{next:02X}"),
                "preferred address already in use — claiming an unused one",
            );
            next
        } else {
            preferred
        };

        if address > MAX_ADDRESS {
            tracing::error!(
                target: "machbus.network.claim",
                "no available address after the initial listen window",
            );
            cf.set_state(CfState::Offline);
            cf.state_machine_mut().transition(ClaimState::Failed);
            cf.set_address(NULL_ADDRESS);
            return vec![make_claim_frame(cf.name(), NULL_ADDRESS)];
        }

        cf.set_address(address);
        cf.state_machine_mut().transition(ClaimState::SendClaim);
        cf.reset_claim_timer();
        self.claim_guard_timer_ms = 0;
        let frames = vec![make_claim_frame(cf.name(), address)];
        cf.state_machine_mut()
            .transition(ClaimState::WaitForContest);
        frames
    }

    /// Drive elapsed time. Emits frames when the RTxD-delayed re-claim
    /// fires; otherwise advances the contention guard timer and may
    /// transition the CF to [`ClaimState::Claimed`].
    pub fn update(&mut self, cf: &mut InternalCf, elapsed_ms: u32) -> Vec<Frame> {
        let mut frames = Vec::new();

        if self.listen_pending {
            self.listen_timer_ms = self.listen_timer_ms.saturating_add(elapsed_ms);
            if self.listen_timer_ms >= self.timeout_ms {
                return self.finish_listen(cf);
            }
            return frames;
        }

        if self.cannot_claim_pending {
            self.cannot_claim_delay_timer_ms =
                self.cannot_claim_delay_timer_ms.saturating_add(elapsed_ms);
            if self.cannot_claim_delay_timer_ms >= self.rtxd_ms {
                self.cannot_claim_pending = false;
                self.cannot_claim_delay_timer_ms = 0;
                if cf.claim_state() == ClaimState::Failed {
                    frames.push(make_claim_frame(cf.name(), NULL_ADDRESS));
                }
            }
        }

        if self.reclaim_pending {
            self.reclaim_delay_timer_ms = self.reclaim_delay_timer_ms.saturating_add(elapsed_ms);
            if self.reclaim_delay_timer_ms >= self.rtxd_ms {
                self.reclaim_pending = false;
                let mut addr = self.reclaim_address;
                if addr > MAX_ADDRESS || addr == cf.preferred_address() || self.is_occupied(addr) {
                    addr = self.find_next_address(cf, addr);
                }
                if addr > MAX_ADDRESS {
                    self.reclaim_address = NULL_ADDRESS;
                    self.reclaim_delay_timer_ms = 0;
                    self.claim_guard_timer_ms = 0;
                    cf.set_address(NULL_ADDRESS);
                    cf.set_state(CfState::Offline);
                    cf.state_machine_mut().transition(ClaimState::Failed);
                    frames.push(make_claim_frame(cf.name(), NULL_ADDRESS));
                    tracing::error!(
                        target: "machbus.network.claim",
                        "no available address at delayed re-claim time — claim failed",
                    );
                    return frames;
                }
                self.reclaim_address = addr;
                cf.set_address(addr);
                cf.reset_claim_timer();
                self.claim_guard_timer_ms = 0;
                cf.state_machine_mut().transition(ClaimState::SendClaim);
                frames.push(make_claim_frame(cf.name(), addr));
                cf.state_machine_mut()
                    .transition(ClaimState::WaitForContest);
                tracing::debug!(
                    target: "machbus.network.claim",
                    addr = %format_args!("0x{:02X}", addr),
                    "re-claim sent after RTxD",
                );
            }
            return frames;
        }

        if cf.claim_state() == ClaimState::WaitForContest {
            cf.add_claim_time(elapsed_ms);
            self.claim_guard_timer_ms = self.claim_guard_timer_ms.saturating_add(elapsed_ms);

            if self.claim_guard_timer_ms >= self.timeout_ms {
                cf.state_machine_mut().transition(ClaimState::Claimed);
                if cf.address() == NULL_ADDRESS {
                    let pref = cf.preferred_address();
                    cf.set_address(pref);
                }
                cf.set_state(CfState::Online);
                let addr = cf.address();
                cf.on_address_claimed.emit(&addr);
                tracing::info!(
                    target: "machbus.network.claim",
                    addr = %format_args!("0x{:02X}", addr),
                    "address claimed",
                );
            }
        }
        frames
    }

    /// React to another node's address claim. `claimed_address` is
    /// the address it tried to claim; `other_name` is its NAME.
    ///
    /// If we win arbitration (lower NAME), we re-broadcast our claim.
    /// If we lose, we either queue an RTxD-delayed re-claim at the
    /// next address (when self-configurable) or transition to
    /// [`ClaimState::Failed`] and emit a cannot-claim frame.
    pub fn handle_claim(
        &mut self,
        cf: &mut InternalCf,
        claimed_address: Address,
        other_name: Name,
    ) -> Vec<Frame> {
        let mut frames = Vec::new();
        if claimed_address > MAX_ADDRESS {
            return frames;
        }
        if other_name != cf.name() {
            self.mark_occupied(claimed_address);
        }
        if !self.attempted_claim {
            return frames;
        }
        if cf.claim_state() == ClaimState::Failed {
            return frames;
        }
        // While the §4.5.1 a) window is open we hold no address yet, so a peer
        // claim is only an entry in the address table, never a contest.
        if self.listen_pending {
            return frames;
        }
        // §4.5.3: "A CF shall transmit an address claim if it receives an
        // address-claimed message with an SA matching **its own**". Treating a
        // claim for our merely *preferred* address as a contest meant an
        // unrelated CF taking an address we were not using knocked us off the
        // one we validly held: steering and speed setpoints stopped for the
        // ~400 ms of a fresh claim, repeatable indefinitely.
        if claimed_address != cf.address() {
            return frames; // not contesting our address
        }
        if other_name == cf.name() {
            return frames; // local echo or duplicate NAME; do not self-displace
        }

        if cf.name() < other_name {
            tracing::debug!(
                target: "machbus.network.claim",
                "won address contest",
            );
            frames.push(make_claim_frame(cf.name(), cf.address()));
            return frames;
        }

        // We lose — yield.
        tracing::warn!(
            target: "machbus.network.claim",
            yielded_addr = %format_args!("0x{:02X}", claimed_address),
            "lost address contest",
        );
        cf.set_state(CfState::Offline);
        cf.on_address_lost.emit(&());

        if !cf.name().self_configurable() {
            tracing::error!(
                target: "machbus.network.claim",
                "not self-configurable — claim failed",
            );
            cf.state_machine_mut().transition(ClaimState::Failed);
            cf.set_address(NULL_ADDRESS);
            frames.push(make_claim_frame(cf.name(), NULL_ADDRESS));
            return frames;
        }

        let next = self.find_next_address(cf, claimed_address);
        if next > MAX_ADDRESS {
            tracing::error!(
                target: "machbus.network.claim",
                "no available address — claim failed",
            );
            cf.state_machine_mut().transition(ClaimState::Failed);
            cf.set_address(NULL_ADDRESS);
            frames.push(make_claim_frame(cf.name(), NULL_ADDRESS));
            return frames;
        }

        if self.rtxd_ms > 0 {
            self.reclaim_pending = true;
            self.reclaim_delay_timer_ms = 0;
            self.reclaim_address = next;
            // Leave Claimed the moment arbitration is lost, not when the RTxD
            // re-claim fires. §4.5.3 has the CF discontinue the address right
            // away; staying in Claimed for the whole RTxD delay made
            // `is_claimed()` report an address we no longer hold. A CF that
            // was still in WaitForContest never reported itself claimed, so
            // its state is left alone.
            if cf.claim_state() == ClaimState::Claimed {
                cf.state_machine_mut().transition(ClaimState::WaitForClaim);
            }
            tracing::debug!(
                target: "machbus.network.claim",
                rtxd_ms = self.rtxd_ms,
                addr = %format_args!("0x{:02X}", next),
                "re-claim queued with RTxD",
            );
        } else {
            cf.set_address(next);
            cf.reset_claim_timer();
            self.claim_guard_timer_ms = 0;
            cf.state_machine_mut().transition(ClaimState::SendClaim);
            frames.push(make_claim_frame(cf.name(), next));
            cf.state_machine_mut()
                .transition(ClaimState::WaitForContest);
        }
        frames
    }

    /// React to a detected duplicate NAME conflict.
    ///
    /// Address-claim arbitration has no ordering rule when two distinct
    /// devices present the same NAME. The network manager only calls this for
    /// conflicts that are distinguishable from a local echo, such as the same
    /// NAME claiming a different source address. In that case the local CF must
    /// leave the bus identity space and emit Cannot Claim Address instead of
    /// continuing online under an ambiguous identity.
    pub fn handle_duplicate_name(&mut self, cf: &mut InternalCf) -> Vec<Frame> {
        if !self.attempted_claim || cf.claim_state() == ClaimState::Failed {
            return Vec::new();
        }

        tracing::error!(
            target: "machbus.network.claim",
            name = %format_args!("0x{:016X}", cf.name().raw),
            "duplicate NAME detected — cannot resolve address arbitration",
        );

        self.reclaim_pending = false;
        self.reclaim_delay_timer_ms = 0;
        self.reclaim_address = NULL_ADDRESS;
        self.claim_guard_timer_ms = 0;

        cf.set_state(CfState::Offline);
        cf.on_address_lost.emit(&());
        cf.state_machine_mut().transition(ClaimState::Failed);
        cf.set_address(NULL_ADDRESS);

        vec![make_claim_frame(cf.name(), NULL_ADDRESS)]
    }

    /// React to an incoming request-for-address-claimed (PGN
    /// 0xEA00 with the `PGN_ADDRESS_CLAIMED` data field).
    ///
    /// - If we have not attempted to claim, send nothing (§4.4.2).
    /// - If [`ClaimState::Claimed`] or [`ClaimState::WaitForContest`],
    ///   send a claim with our current address.
    /// - If [`ClaimState::Failed`], send cannot-claim
    ///   (SA = [`NULL_ADDRESS`]).
    pub fn handle_request_for_claim(&mut self, cf: &mut InternalCf) -> Vec<Frame> {
        let mut frames = Vec::new();
        if !self.attempted_claim {
            return frames;
        }
        if self.reclaim_pending {
            return frames;
        }
        match cf.claim_state() {
            ClaimState::Claimed | ClaimState::WaitForContest => {
                frames.push(make_claim_frame(cf.name(), cf.address()));
            }
            ClaimState::Failed => {
                // Deferred by RTxD; `update` emits it. With no delay every
                // failed CF answers a global request simultaneously.
                if self.rtxd_ms == 0 {
                    frames.push(make_claim_frame(cf.name(), NULL_ADDRESS));
                } else {
                    self.cannot_claim_pending = true;
                    self.cannot_claim_delay_timer_ms = 0;
                }
            }
            _ => {}
        }
        frames
    }

    /// Successor to `current` within the self-configurable address range
    /// [128, 247], skipping the preferred and any occupied address. Returns
    /// [`NULL_ADDRESS`] when no address in the range is available.
    ///
    /// Self-configurable CFs must auto-select inside the configurable range
    /// (ISO 11783-5); the preferred address is tried separately before this
    /// fallback, so it may legitimately lie outside the range.
    fn find_next_address(&self, cf: &InternalCf, current: Address) -> Address {
        const LO: Address = SELF_CONFIG_ADDRESS_MIN;
        const HI: Address = SELF_CONFIG_ADDRESS_MAX;

        let mut next = if (LO..HI).contains(&current) {
            current + 1
        } else {
            LO
        };

        for _ in 0..=(HI - LO) {
            if next != cf.preferred_address() && !self.is_occupied(next) {
                return next;
            }
            next = if next < HI { next + 1 } else { LO };
        }

        NULL_ADDRESS
    }

    fn mark_occupied(&mut self, addr: Address) {
        if addr <= MAX_ADDRESS {
            self.occupied_addresses[addr as usize] = true;
        }
    }

    fn is_occupied(&self, addr: Address) -> bool {
        addr <= MAX_ADDRESS && self.occupied_addresses[addr as usize]
    }
}

// ─── Frame builders ─────────────────────────────────────────────────────

fn make_request_frame() -> Frame {
    let id = Identifier::encode(
        Priority::Default,
        PGN_REQUEST,
        NULL_ADDRESS,
        BROADCAST_ADDRESS,
    );
    let mut data = [0xFFu8; 8];
    data[0] = (PGN_ADDRESS_CLAIMED & 0xFF) as u8;
    data[1] = ((PGN_ADDRESS_CLAIMED >> 8) & 0xFF) as u8;
    data[2] = ((PGN_ADDRESS_CLAIMED >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_claim_frame(name: Name, addr: Address) -> Frame {
    let id = Identifier::encode(
        Priority::Default,
        PGN_ADDRESS_CLAIMED,
        addr,
        BROADCAST_ADDRESS,
    );
    Frame::new(id, name.to_bytes(), 8)
}

/// A per-control-function RTxD delay derived from its NAME.
///
/// ISO 11783-5 §4.4.3 wants the re-claim delay to differ between control
/// functions so they do not answer in lockstep after a contention. Production
/// passed a constant `0`, which is the worst case: every CF re-claims on the
/// same millisecond, and the delay exists precisely to prevent that.
///
/// A true random value would be ideal, but the crate has no RNG under
/// `no_std`. Hashing the NAME gives a value that is stable for a given device
/// and differs between devices, which is what the requirement is actually for.
/// A caller with an entropy source should prefer it and pass its own value.
#[must_use]
pub const fn rtxd_for_name(name: Name) -> u32 {
    // FNV-1a over the 64-bit NAME, reduced to the 0..=255 random-byte domain.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let raw = name.raw;
    let mut i = 0;
    while i < 8 {
        hash ^= (raw >> (i * 8)) & 0xFF;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        i += 1;
    }
    let random_byte = (hash & 0xFF) as u32;
    // 0.6 ms per unit, capped at the standard's maximum.
    let delay = random_byte * 6 / 10;
    if delay > ADDRESS_CLAIM_RTXD_MAX_MS {
        ADDRESS_CLAIM_RTXD_MAX_MS
    } else {
        delay
    }
}

#[cfg(test)]
mod rtxd_tests {
    use super::*;

    /// §4.4.3 — the re-claim delay exists so control functions do not answer in
    /// lockstep. Production passed a constant 0, which guaranteed the lockstep.
    #[test]
    fn rtxd_differs_between_control_functions_and_stays_in_range() {
        let names: Vec<Name> = (1u32..=64)
            .map(|i| Name::default().with_identity_number(i))
            .collect();

        let delays: Vec<u32> = names.iter().map(|n| rtxd_for_name(*n)).collect();

        for delay in &delays {
            assert!(
                *delay <= ADDRESS_CLAIM_RTXD_MAX_MS,
                "RTxD must stay within the 0..=153 ms window, got {delay}"
            );
        }

        // The point is spread: a handful of distinct values is not enough to
        // break a contention between many CFs.
        let mut unique = delays.clone();
        unique.sort_unstable();
        unique.dedup();
        assert!(
            unique.len() > 32,
            "64 different NAMEs produced only {} distinct delays",
            unique.len()
        );
    }

    /// It must be stable for a given device, so a CF does not shuffle its own
    /// delay between claims within one power cycle.
    #[test]
    fn rtxd_is_stable_for_a_given_name() {
        let name = Name::default().with_identity_number(0xABC);
        assert_eq!(rtxd_for_name(name), rtxd_for_name(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Build a NAME with a given lower 21-bit "identity" so we can
    /// control arbitration easily — lower identity ⇒ lower NAME ⇒
    /// wins.
    fn name_with_identity(id: u32, self_config: bool) -> Name {
        Name::default()
            .with_identity_number(id)
            .with_function_code(0x80)
            .with_self_configurable(self_config)
    }

    /// Start a claim and run out the §4.5.1 a) listen window, returning the
    /// request frame and the claim that follows it.
    fn start_and_claim(clm: &mut AddressClaimer, cf: &mut InternalCf) -> Vec<Frame> {
        let mut frames = clm.start(cf);
        frames.extend(clm.update(cf, ADDRESS_CLAIM_TIMEOUT_MS + ADDRESS_CLAIM_RTXD_MAX_MS));
        frames
    }

    /// §4.5.1 a): request, then wait at least 250 ms + RTxD, then claim. The
    /// two frames used to go out back to back, so the claim was made against
    /// an empty address table.
    #[test]
    fn start_requests_then_claims_only_after_the_listen_window() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);

        let request = clm.start(&mut cf);
        assert_eq!(request.len(), 1);
        assert_eq!(request[0].pgn(), PGN_REQUEST);
        assert!(clm.is_listening());
        assert_eq!(cf.claim_state(), ClaimState::SendRequest);
        assert!(clm.has_attempted_claim());

        assert!(
            clm.update(&mut cf, ADDRESS_CLAIM_TIMEOUT_MS - 1).is_empty(),
            "no claim may go out before the listen window closes"
        );
        assert!(clm.is_listening());

        let claim = clm.update(&mut cf, 1);
        assert_eq!(claim.len(), 1);
        assert_eq!(claim[0].pgn(), PGN_ADDRESS_CLAIMED);
        assert_eq!(claim[0].source(), 0x80);
        assert!(!clm.is_listening());
        assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    }

    /// The point of the window: an address heard during it is not taken.
    #[test]
    fn an_address_heard_during_the_listen_window_is_not_claimed() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        clm.start(&mut cf);

        // An incumbent answers the request from our preferred address.
        let incumbent = name_with_identity(0x900, true);
        assert!(
            clm.handle_claim(&mut cf, 0x80, incumbent).is_empty(),
            "a peer claim during the listen window is a table entry, not a contest"
        );

        let claim = clm.update(&mut cf, ADDRESS_CLAIM_TIMEOUT_MS);
        assert_eq!(claim.len(), 1);
        assert_ne!(
            claim[0].source(),
            0x80,
            "the incumbent must not be evicted from an address we never held"
        );
        assert_eq!(cf.address(), claim[0].source());
    }

    #[test]
    fn before_start_request_for_claim_is_silent() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        // §4.4.2: must not respond to RFC if never attempted.
        assert!(clm.handle_request_for_claim(&mut cf).is_empty());
    }

    #[test]
    fn before_start_incoming_claim_is_silent() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let other = name_with_identity(0x999, true);

        assert!(clm.handle_claim(&mut cf, 0x80, other).is_empty());
        assert_eq!(cf.address(), 0x80);
        assert_eq!(cf.claim_state(), ClaimState::None);
        assert_eq!(cf.cf().state, CfState::Offline);
    }

    #[test]
    fn guard_window_completes_to_claimed_state() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let claimed = Rc::new(RefCell::new(None::<Address>));
        let c = claimed.clone();
        cf.on_address_claimed
            .subscribe(move |&a| *c.borrow_mut() = Some(a));

        let _ = start_and_claim(&mut clm, &mut cf);
        // Default timeout is 250 ms.
        let _ = clm.update(&mut cf, 100);
        assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
        let _ = clm.update(&mut cf, 200); // total 300 ≥ 250
        assert_eq!(cf.claim_state(), ClaimState::Claimed);
        assert!(cf.cf().is_online());
        assert_eq!(*claimed.borrow(), Some(0x80));
    }

    #[test]
    fn winning_contest_resends_claim() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let _ = start_and_claim(&mut clm, &mut cf);
        // Other CF claims same address with a higher (worse) NAME.
        let other = name_with_identity(0x999, true);
        let frames = clm.handle_claim(&mut cf, 0x80, other);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].pgn(), PGN_ADDRESS_CLAIMED);
        assert_eq!(frames[0].source(), 0x80);
        assert!(cf.cf().state == CfState::Offline || cf.cf().state == CfState::Online);
    }

    /// C14 — §4.5.3 triggers a contest on "an address-claimed message with an
    /// SA matching **its own**". Treating a claim for the CF's merely
    /// *preferred* address as a contest let an unrelated CF knock this one off
    /// an address it validly held: guidance and speed setpoints stopped for the
    /// duration of a fresh claim, repeatably.
    #[test]
    fn a_claim_for_our_preferred_address_does_not_displace_us() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let _ = start_and_claim(&mut clm, &mut cf);
        let _ = clm.update(&mut cf, 300);
        assert_eq!(cf.claim_state(), ClaimState::Claimed);

        // We ended up on a different address than we preferred.
        cf.set_address(0x81);
        assert_eq!(cf.preferred_address(), 0x80);

        // A stronger CF claims 0x80 — which we are not using.
        let stronger = name_with_identity(0x001, true);
        let frames = clm.handle_claim(&mut cf, 0x80, stronger);

        assert!(
            frames.is_empty(),
            "a claim for an address we do not hold is not a contest"
        );
        assert_eq!(cf.cf().address, 0x81, "our held address is untouched");
        assert_eq!(cf.claim_state(), ClaimState::Claimed);
        assert!(cf.cf().is_online());

        // A claim for the address we *do* hold still contests.
        let frames = clm.handle_claim(&mut cf, 0x81, stronger);
        assert!(
            !frames.is_empty(),
            "a claim for our own SA must still be answered"
        );
    }

    /// H31 — §4.4.2.4: "A random transmit delay (RTxD) shall be inserted
    /// between the reception of a message triggering the
    /// cannot-claim-source-address response and the sending of the response in
    /// order to minimize the possibility of two such responses causing bus
    /// errors." Answering immediately had every failed CF on the segment reply
    /// to one global request in the same instant.
    #[test]
    fn a_cannot_claim_response_waits_out_the_random_transmit_delay() {
        let mut cf = InternalCf::new(name_with_identity(0x100, false), 0, 0x80);
        let mut clm = AddressClaimer::with_timeout(250, 40);
        let _ = start_and_claim(&mut clm, &mut cf);

        // A stronger NAME takes our address. `self_configurable` is NAME bit
        // 63, so a self-configurable peer is *weaker*, not stronger — both
        // sides here are non-configurable and the lower identity wins.
        let stronger = name_with_identity(0x001, false);
        let _ = clm.handle_claim(&mut cf, 0x80, stronger);
        assert_eq!(cf.claim_state(), ClaimState::Failed);

        let immediate = clm.handle_request_for_claim(&mut cf);
        assert!(
            immediate.is_empty(),
            "the response must not go out in the same instant as the request"
        );

        // Nothing before the delay expires...
        assert!(clm.update(&mut cf, 39).is_empty());
        // ...and exactly one cannot-claim after it.
        let delayed = clm.update(&mut cf, 1);
        assert_eq!(delayed.len(), 1);
        assert_eq!(delayed[0].pgn(), PGN_ADDRESS_CLAIMED);
        assert_eq!(delayed[0].source(), NULL_ADDRESS);

        // It is a one-shot, not a repeating cadence.
        assert!(clm.update(&mut cf, 500).is_empty());
    }

    #[test]
    fn equal_name_claim_is_ignored_as_local_echo() {
        let name = name_with_identity(0x100, true);
        let mut cf = InternalCf::new(name, 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let _ = start_and_claim(&mut clm, &mut cf);

        let frames = clm.handle_claim(&mut cf, 0x80, name);

        assert!(frames.is_empty());
        assert_eq!(cf.address(), 0x80);
        assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
        assert_eq!(cf.cf().state, CfState::Offline);
    }

    #[test]
    fn duplicate_name_detection_fails_with_cannot_claim() {
        let name = name_with_identity(0x100, true);
        let mut cf = InternalCf::new(name, 0, 0x80);
        let mut clm = AddressClaimer::new(50);
        let lost = Rc::new(RefCell::new(0u32));
        let l = lost.clone();
        cf.on_address_lost.subscribe(move |_| *l.borrow_mut() += 1);
        let _ = start_and_claim(&mut clm, &mut cf);

        let frames = clm.handle_duplicate_name(&mut cf);

        assert_eq!(*lost.borrow(), 1);
        assert_eq!(cf.claim_state(), ClaimState::Failed);
        assert_eq!(cf.address(), NULL_ADDRESS);
        assert_eq!(cf.cf().state, CfState::Offline);
        assert!(!clm.reclaim_pending);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].pgn(), PGN_ADDRESS_CLAIMED);
        assert_eq!(frames[0].source(), NULL_ADDRESS);
        assert_eq!(frames[0].payload(), &name.to_bytes());

        assert!(
            clm.handle_duplicate_name(&mut cf).is_empty(),
            "already-failed duplicate handling must not emit repeated Cannot Claim frames",
        );
    }

    #[test]
    fn losing_contest_self_configurable_no_rtxd_immediate_reclaim() {
        let mut cf = InternalCf::new(name_with_identity(0x999, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0); // rtxd=0 ⇒ immediate
        let lost = Rc::new(RefCell::new(0u32));
        let l = lost.clone();
        cf.on_address_lost.subscribe(move |_| *l.borrow_mut() += 1);

        let _ = start_and_claim(&mut clm, &mut cf);
        let other = name_with_identity(0x100, true); // lower id ⇒ wins
        let frames = clm.handle_claim(&mut cf, 0x80, other);

        assert_eq!(*lost.borrow(), 1);
        assert_eq!(cf.address(), 0x81); // moved to next
        assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].source(), 0x81);
    }

    #[test]
    fn self_configurable_reclaim_skips_observed_occupied_addresses_until_cannot_claim() {
        let mut cf = InternalCf::new(name_with_identity(0xFFFF, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let _ = start_and_claim(&mut clm, &mut cf);

        // Learn every address except our currently-contested preferred
        // address before we lose arbitration there. This mirrors a saturated
        // network where all dynamic alternatives are already claimed.
        for addr in 0..=MAX_ADDRESS {
            if addr != 0x80 {
                let _ = clm.handle_claim(&mut cf, addr, name_with_identity(addr as u32, true));
            }
        }

        let frames = clm.handle_claim(&mut cf, 0x80, name_with_identity(1, true));

        assert_eq!(cf.claim_state(), ClaimState::Failed);
        assert_eq!(cf.address(), NULL_ADDRESS);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].source(), NULL_ADDRESS);
        assert_eq!(frames[0].payload(), &cf.name().to_bytes());
    }

    #[test]
    fn self_configurable_reclaim_stays_in_configurable_range_and_wraps() {
        // Preferred at the top of the configurable range; losing it must wrap
        // the auto-selection back to the bottom of the range (128), never into
        // the reserved 248..=253 region.
        let mut cf = InternalCf::new(name_with_identity(0x999, true), 0, 247);
        let mut clm = AddressClaimer::new(0); // immediate reclaim
        let _ = start_and_claim(&mut clm, &mut cf);
        let frames = clm.handle_claim(&mut cf, 247, name_with_identity(1, true));
        assert_eq!(cf.address(), SELF_CONFIG_ADDRESS_MIN); // wrapped to 128
        assert!(cf.address() >= SELF_CONFIG_ADDRESS_MIN);
        assert!(cf.address() <= SELF_CONFIG_ADDRESS_MAX);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].source(), SELF_CONFIG_ADDRESS_MIN);
    }

    #[test]
    fn losing_contest_with_rtxd_queues_delayed_reclaim() {
        let mut cf = InternalCf::new(name_with_identity(0x999, true), 0, 0x80);
        let mut clm = AddressClaimer::new(50); // 50 ms RTxD
        let _ = start_and_claim(&mut clm, &mut cf);

        let other = name_with_identity(0x100, true);
        let frames = clm.handle_claim(&mut cf, 0x80, other);
        assert!(frames.is_empty(), "no immediate frame when RTxD > 0");
        assert!(clm.reclaim_pending);

        // Until RTxD elapses, no claim emitted.
        let frames = clm.update(&mut cf, 30);
        assert!(frames.is_empty());
        assert!(clm.reclaim_pending);

        // Crossing the threshold emits the queued claim.
        let frames = clm.update(&mut cf, 30);
        assert_eq!(frames.len(), 1);
        assert!(!clm.reclaim_pending);
        assert_eq!(cf.address(), 0x81);
        assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    }

    #[test]
    fn losing_contest_not_self_configurable_fails() {
        // self_configurable is bit 63 of the NAME (the MSB), so it
        // dominates arbitration. Both CFs must share the bit for
        // identity-based comparison to decide the winner.
        let mut cf = InternalCf::new(name_with_identity(0x999, false), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let _ = start_and_claim(&mut clm, &mut cf);
        let other = name_with_identity(0x100, false);
        let frames = clm.handle_claim(&mut cf, 0x80, other);

        assert_eq!(cf.claim_state(), ClaimState::Failed);
        assert_eq!(cf.address(), NULL_ADDRESS);
        assert_eq!(frames.len(), 1);
        // Cannot-claim has source = NULL_ADDRESS.
        assert_eq!(frames[0].source(), NULL_ADDRESS);
    }

    #[test]
    fn handle_claim_ignores_unrelated_address() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let _ = start_and_claim(&mut clm, &mut cf);
        let frames = clm.handle_claim(&mut cf, 0x42, name_with_identity(0x999, true));
        assert!(frames.is_empty());
    }

    #[test]
    fn rfc_after_claimed_emits_current_claim() {
        let mut cf = InternalCf::new(name_with_identity(0x100, true), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let _ = start_and_claim(&mut clm, &mut cf);
        let _ = clm.update(&mut cf, 1000); // jump past timeout
        assert_eq!(cf.claim_state(), ClaimState::Claimed);

        let frames = clm.handle_request_for_claim(&mut cf);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].pgn(), PGN_ADDRESS_CLAIMED);
        assert_eq!(frames[0].source(), 0x80);
    }

    #[test]
    fn rfc_after_failed_emits_cannot_claim() {
        // Both NAMEs share the self_configurable bit (= 0) so identity
        // determines the winner. See the comment in
        // `losing_contest_not_self_configurable_fails`.
        let mut cf = InternalCf::new(name_with_identity(0x999, false), 0, 0x80);
        let mut clm = AddressClaimer::new(0);
        let _ = start_and_claim(&mut clm, &mut cf);
        let _ = clm.handle_claim(&mut cf, 0x80, name_with_identity(0x100, false));
        assert_eq!(cf.claim_state(), ClaimState::Failed);

        let frames = clm.handle_request_for_claim(&mut cf);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].source(), NULL_ADDRESS);
    }

    /// Phase-4 canary: two CFs with conflicting NAMEs claim 0x80; the
    /// lower-NAME one wins, the higher-NAME one yields and re-claims
    /// at 0x81 after RTxD.
    #[test]
    fn canary_two_cfs_contest_lower_name_wins() {
        let low_name = name_with_identity(0x100, true);
        let high_name = name_with_identity(0x999, true);
        let mut low = InternalCf::new(low_name, 0, 0x80);
        let mut high = InternalCf::new(high_name, 0, 0x80);
        let mut low_clm = AddressClaimer::new(0);
        let mut high_clm = AddressClaimer::new(50);

        // Both start.
        let _ = start_and_claim(&mut low_clm, &mut low);
        let _ = start_and_claim(&mut high_clm, &mut high);

        // Each sees the other's claim.
        let low_response = low_clm.handle_claim(&mut low, 0x80, high_name);
        let high_response = high_clm.handle_claim(&mut high, 0x80, low_name);

        // Low wins → re-broadcasts at 0x80.
        assert_eq!(low_response.len(), 1);
        assert_eq!(low_response[0].source(), 0x80);
        assert_eq!(low.claim_state(), ClaimState::WaitForContest);

        // High loses → no immediate frame (RTxD pending).
        assert!(high_response.is_empty());
        assert!(high_clm.reclaim_pending);
        assert_eq!(high.cf().state, CfState::Offline);

        // After RTxD, high re-claims at next address (0x81).
        let frames = high_clm.update(&mut high, 60);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].source(), 0x81);
        assert_eq!(high.address(), 0x81);
        assert_eq!(high.claim_state(), ClaimState::WaitForContest);

        // Both finish their guard windows uncontested.
        let _ = low_clm.update(&mut low, 300);
        let _ = high_clm.update(&mut high, 300);
        assert_eq!(low.claim_state(), ClaimState::Claimed);
        assert_eq!(high.claim_state(), ClaimState::Claimed);
        assert_eq!(low.address(), 0x80);
        assert_eq!(high.address(), 0x81);
    }
}
