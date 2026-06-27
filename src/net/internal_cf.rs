//! Local ECU representation: [`InternalCf`].
//!
//! Mirrors the C++ `machbus::net::InternalCF`. The address-claim
//! [`ClaimState`] machine, the elapsed-time accumulator, and the
//! `on_address_claimed` / `on_address_lost` events all live here so
//! the [`AddressClaimer`] can drive them via `&mut InternalCf`.
//!
//! [`AddressClaimer`]: super::address_claimer::AddressClaimer

use super::control_function::{CfState, CfType, ControlFunction};
use super::event::Event;
use super::name::Name;
use super::state_machine::StateMachine;
use super::types::Address;

/// Address-claim FSM states (ISO 11783-5 §4.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClaimState {
    /// Initial / not yet started.
    #[default]
    None,
    /// Reserved for the C++ "WaitForClaim" intermediate state.
    WaitForClaim,
    /// Sending the request-for-address-claimed PGN.
    SendRequest,
    /// Sent our claim; waiting out the contention guard window.
    WaitForContest,
    /// Sending an address claim frame.
    SendClaim,
    /// Address successfully claimed.
    Claimed,
    /// Could not claim; will emit cannot-claim (SA = `NULL_ADDRESS`).
    Failed,
}

/// Local ECU. Owned by the application; driven by
/// [`AddressClaimer`].
///
/// [`AddressClaimer`]: super::address_claimer::AddressClaimer
pub struct InternalCf {
    cf: ControlFunction,
    state_machine: StateMachine<ClaimState>,
    preferred_address: Address,
    claim_timer_ms: u32,

    /// Fires with the claimed [`Address`] when the contention guard
    /// window expires without a higher-priority claim.
    pub on_address_claimed: Event<Address>,
    /// Fires after losing arbitration (before the loser re-claims at
    /// a different address, if possible).
    pub on_address_lost: Event<()>,
}

impl InternalCf {
    /// Build a local CF with a given NAME, CAN port, and preferred
    /// (initial) address.
    #[must_use]
    pub fn new(name: Name, port: u8, preferred: Address) -> Self {
        let mut cf = ControlFunction::new(name, port, CfType::Internal);
        cf.address = preferred;
        Self {
            cf,
            state_machine: StateMachine::new(ClaimState::None),
            preferred_address: preferred,
            claim_timer_ms: 0,
            on_address_claimed: Event::new(),
            on_address_lost: Event::new(),
        }
    }

    // ─── Borrow base CF ────────────────────────────────────────────
    #[inline]
    #[must_use]
    pub fn cf(&self) -> &ControlFunction {
        &self.cf
    }
    #[inline]
    pub fn cf_mut(&mut self) -> &mut ControlFunction {
        &mut self.cf
    }

    // ─── Read-only queries ─────────────────────────────────────────
    #[inline]
    #[must_use]
    pub fn name(&self) -> Name {
        self.cf.name
    }
    #[inline]
    #[must_use]
    pub fn address(&self) -> Address {
        self.cf.address
    }
    #[inline]
    #[must_use]
    pub fn port(&self) -> u8 {
        self.cf.can_port
    }
    #[inline]
    #[must_use]
    pub fn preferred_address(&self) -> Address {
        self.preferred_address
    }
    #[inline]
    #[must_use]
    pub fn claim_state(&self) -> ClaimState {
        self.state_machine.state()
    }
    #[inline]
    #[must_use]
    pub fn claim_timer(&self) -> u32 {
        self.claim_timer_ms
    }

    // ─── Mutators (called by AddressClaimer) ──────────────────────
    #[inline]
    pub fn set_address(&mut self, addr: Address) {
        self.cf.address = addr;
    }
    #[inline]
    pub fn set_name(&mut self, name: Name) {
        self.cf.name = name;
    }
    #[inline]
    pub fn set_state(&mut self, state: CfState) {
        self.cf.state = state;
    }

    #[inline]
    pub fn state_machine(&self) -> &StateMachine<ClaimState> {
        &self.state_machine
    }
    #[inline]
    pub fn state_machine_mut(&mut self) -> &mut StateMachine<ClaimState> {
        &mut self.state_machine
    }

    #[inline]
    pub fn add_claim_time(&mut self, ms: u32) {
        self.claim_timer_ms = self.claim_timer_ms.saturating_add(ms);
    }
    #[inline]
    pub fn reset_claim_timer(&mut self) {
        self.claim_timer_ms = 0;
    }
}

impl core::fmt::Debug for InternalCf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InternalCf")
            .field("name", &format_args!("0x{:016X}", self.cf.name.raw))
            .field("address", &format_args!("0x{:02X}", self.cf.address))
            .field(
                "preferred_address",
                &format_args!("0x{:02X}", self.preferred_address),
            )
            .field("port", &self.cf.can_port)
            .field("cf_state", &self.cf.state)
            .field("claim_state", &self.state_machine.state())
            .field("claim_timer_ms", &self.claim_timer_ms)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_name() -> Name {
        Name::default()
            .with_identity_number(0x12345)
            .with_function_code(0x80)
            .with_self_configurable(true)
    }

    #[test]
    fn defaults_to_claim_state_none() {
        let cf = InternalCf::new(sample_name(), 0, 0x80);
        assert_eq!(cf.claim_state(), ClaimState::None);
        assert_eq!(cf.address(), 0x80);
        assert_eq!(cf.preferred_address(), 0x80);
        assert_eq!(cf.port(), 0);
        assert_eq!(cf.claim_timer(), 0);
        assert_eq!(cf.cf().r#type, CfType::Internal);
    }

    #[test]
    fn add_and_reset_claim_timer() {
        let mut cf = InternalCf::new(sample_name(), 0, 0x80);
        cf.add_claim_time(100);
        cf.add_claim_time(50);
        assert_eq!(cf.claim_timer(), 150);
        cf.reset_claim_timer();
        assert_eq!(cf.claim_timer(), 0);
    }

    #[test]
    fn state_machine_transitions_observable() {
        let mut cf = InternalCf::new(sample_name(), 0, 0x80);
        cf.state_machine_mut().transition(ClaimState::SendRequest);
        assert_eq!(cf.claim_state(), ClaimState::SendRequest);
        cf.state_machine_mut().transition(ClaimState::Claimed);
        assert_eq!(cf.claim_state(), ClaimState::Claimed);
    }

    #[test]
    fn set_address_and_name_update_base_cf() {
        let mut cf = InternalCf::new(sample_name(), 0, 0x80);
        cf.set_address(0x42);
        assert_eq!(cf.address(), 0x42);
        let new_name = Name::from_raw(0xAABB_CCDD_EEFF_0011);
        cf.set_name(new_name);
        assert_eq!(cf.name(), new_name);
    }
}
