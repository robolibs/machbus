//! ISO 11783-5 §4.4.3 NAME Management Protocol.
//!
//! Mirrors the C++ `machbus::net::NameManager`, **decoupled from
//! `IsoNet`**: handlers return `Option<NameMgmtReply>` /
//! `Option<Address>` and the caller routes them through
//! [`IsoNet::send`]. Same pattern as [`Niu`].
//!
//! [`IsoNet::send`]: super::network_manager::IsoNet::send
//! [`Niu`]: super::niu::Niu
//!
//! Wire PGN: `0x9300`. Payload is 17 bytes
//! (`[mode:1][name:8][nack_reason:1][padding:7]`).

use super::constants::{MAX_ADDRESS, NULL_ADDRESS};
use super::error::{Error, Result};
use super::event::Event;
use super::message::Message;
use super::name::Name;
use super::pgn_defs::{PGN_COMMANDED_ADDRESS, PGN_NAME_MANAGEMENT};
use super::types::Address;

/// 9 protocol modes (ISO 11783-5 §4.4.3, Table 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum NameMgmtMode {
    SetPending = 0,
    RequestPendingResponse = 1,
    RequestCurrentResponse = 2,
    Acknowledge = 3,
    NegativeAcknowledge = 4,
    RequestPending = 5,
    #[default]
    RequestCurrent = 6,
    AdoptPending = 7,
    RequestAddressClaim = 8,
}

impl NameMgmtMode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Self::try_from_u8(v)
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::SetPending),
            1 => Some(Self::RequestPendingResponse),
            2 => Some(Self::RequestCurrentResponse),
            3 => Some(Self::Acknowledge),
            4 => Some(Self::NegativeAcknowledge),
            5 => Some(Self::RequestPending),
            6 => Some(Self::RequestCurrent),
            7 => Some(Self::AdoptPending),
            8 => Some(Self::RequestAddressClaim),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// NACK reason codes (ISO 11783-5 §4.4.3 Table 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum NameNackReason {
    Security = 0,
    InvalidItems = 1,
    Conflict = 2,
    Checksum = 3,
    PendingNotSet = 4,
    #[default]
    Other = 5,
}

impl NameNackReason {
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Self::try_from_u8(v)
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Security),
            1 => Some(Self::InvalidItems),
            2 => Some(Self::Conflict),
            3 => Some(Self::Checksum),
            4 => Some(Self::PendingNotSet),
            5 => Some(Self::Other),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Wire-format NAME management message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameManagementMsg {
    pub mode: NameMgmtMode,
    pub name_data: [u8; 8],
    pub nack_reason: NameNackReason,
}

impl Default for NameManagementMsg {
    fn default() -> Self {
        Self {
            mode: NameMgmtMode::default(),
            name_data: [0xFF; 8],
            nack_reason: NameNackReason::default(),
        }
    }
}

impl NameManagementMsg {
    /// Build a message carrying `name` for the given `mode`.
    #[must_use]
    pub fn for_name(mode: NameMgmtMode, name: Name) -> Self {
        Self {
            mode,
            name_data: name.to_bytes(),
            nack_reason: NameNackReason::Other,
        }
    }

    /// Encode to the 17-byte wire format (padded with `0xFF`).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = vec![0xFFu8; 17];
        data[0] = self.mode.as_u8();
        data[1..9].copy_from_slice(&self.name_data);
        if matches!(self.mode, NameMgmtMode::NegativeAcknowledge) {
            data[9] = self.nack_reason.as_u8();
        }
        data
    }

    /// Decode from the canonical 17-byte wire payload. Returns `None` for
    /// unknown modes, invalid NACK reasons, malformed padding, or non-canonical
    /// lengths.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 17 {
            return None;
        }
        let mut name_data = [0u8; 8];
        name_data.copy_from_slice(&data[1..9]);
        let mode = NameMgmtMode::from_u8(data[0])?;
        let nack_reason = if matches!(mode, NameMgmtMode::NegativeAcknowledge) {
            let reason = NameNackReason::from_u8(data[9])?;
            if data[10..].iter().any(|&byte| byte != 0xFF) {
                return None;
            }
            reason
        } else {
            if data[9..].iter().any(|&byte| byte != 0xFF) {
                return None;
            }
            NameNackReason::Other
        };
        Some(Self {
            mode,
            name_data,
            nack_reason,
        })
    }

    /// Extract the carried NAME.
    #[inline]
    #[must_use]
    pub fn name(&self) -> Name {
        Name::from_bytes(&self.name_data).unwrap_or(Name::from_raw(0))
    }
}

/// Reply produced by [`NameManager::handle_name_management`] — the
/// caller wraps it in a regular `IsoNet::send`.
#[derive(Debug, Clone, Copy)]
pub struct NameMgmtReply {
    pub destination: Address,
    pub msg: NameManagementMsg,
}

impl NameMgmtReply {
    #[must_use]
    pub fn new(destination: Address, msg: NameManagementMsg) -> Self {
        Self { destination, msg }
    }
}

/// Stateless protocol helper. Tracks a pending NAME and dispatches
/// incoming NAME-management traffic.
pub struct NameManager {
    pending_name: Option<Name>,

    /// Fires after [`NameManager::adopt_pending`] (the new NAME is
    /// already returned). Caller should now apply it to the
    /// [`InternalCf`] and trigger a re-claim.
    ///
    /// [`InternalCf`]: super::internal_cf::InternalCf
    pub on_name_changed: Event<Name>,
    /// Fires when an incoming PGN_COMMANDED_ADDRESS targets us with a
    /// valid new address. Caller applies + re-claims.
    pub on_commanded_address: Event<Address>,
    /// Fires for every received NAME management message
    /// `(msg, source)`.
    pub on_name_management: Event<(NameManagementMsg, Address)>,
    /// Fires when a targeted NAME Management Request Address Claim message is
    /// received. The stack owner should send a fresh Address Claimed sequence
    /// for the current NAME/address state.
    pub on_request_address_claim: Event<Name>,
}

impl Default for NameManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NameManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending_name: None,
            on_name_changed: Event::new(),
            on_commanded_address: Event::new(),
            on_name_management: Event::new(),
            on_request_address_claim: Event::new(),
        }
    }

    /// Set a pending NAME. ISO 11783-5 forbids changing the
    /// identity-number field, so it must equal `current_identity`.
    pub fn set_pending(&mut self, current_identity: u32, new_name: Name) -> Result<()> {
        if new_name.identity_number() != current_identity {
            return Err(Error::invalid_state("identity_number must not change"));
        }
        self.pending_name = Some(new_name);
        tracing::debug!(target: "machbus.network.name_mgr", "pending NAME set");
        Ok(())
    }

    /// Take ownership of the pending NAME and clear it. Caller is
    /// responsible for applying it to the CF and triggering a
    /// re-claim. Fires [`Self::on_name_changed`].
    pub fn adopt_pending(&mut self) -> Result<Name> {
        let new_name = self
            .pending_name
            .take()
            .ok_or_else(|| Error::invalid_state("no pending NAME set"))?;
        tracing::info!(target: "machbus.network.name_mgr", "adopting pending NAME — re-claim required");
        self.on_name_changed.emit(&new_name);
        Ok(new_name)
    }

    #[inline]
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.pending_name.is_some()
    }

    #[inline]
    #[must_use]
    pub fn pending_name(&self) -> Option<Name> {
        self.pending_name
    }

    /// Dispatch an incoming PGN_NAME_MANAGEMENT message. Returns the
    /// reply to send (if any).
    ///
    /// `current_name` is the CF's current NAME — used for validation
    /// of `SetPending` (the identity-number must match) and as the
    /// source NAME in any reply.
    pub fn handle_name_management(
        &mut self,
        msg: &Message,
        current_name: Name,
    ) -> Option<NameMgmtReply> {
        if msg.pgn != PGN_NAME_MANAGEMENT {
            return None;
        }
        if !msg.has_usable_source() || msg.destination == NULL_ADDRESS {
            return None;
        }
        let nm = NameManagementMsg::decode(&msg.data)?;
        tracing::debug!(
            target: "machbus.network.name_mgr",
            mode = nm.mode.as_u8(),
            from = msg.source,
            "NAME mgmt received",
        );

        let reply = match nm.mode {
            NameMgmtMode::SetPending => {
                let proposed = nm.name();
                let nack_reason = match self.set_pending(current_name.identity_number(), proposed) {
                    Ok(()) => None,
                    Err(_) => Some(NameNackReason::InvalidItems),
                };
                Some(self.build_reply_for_set_pending(msg.source, current_name, nack_reason))
            }
            NameMgmtMode::RequestCurrent => Some(NameMgmtReply::new(
                msg.source,
                NameManagementMsg::for_name(NameMgmtMode::RequestCurrentResponse, current_name),
            )),
            NameMgmtMode::RequestPending => {
                if let Some(p) = self.pending_name {
                    Some(NameMgmtReply::new(
                        msg.source,
                        NameManagementMsg::for_name(NameMgmtMode::RequestPendingResponse, p),
                    ))
                } else {
                    Some(self.build_nack(msg.source, current_name, NameNackReason::PendingNotSet))
                }
            }
            NameMgmtMode::AdoptPending => match self.adopt_pending() {
                Ok(_) => Some(NameMgmtReply::new(
                    msg.source,
                    NameManagementMsg::for_name(NameMgmtMode::Acknowledge, current_name),
                )),
                Err(_) => {
                    Some(self.build_nack(msg.source, current_name, NameNackReason::PendingNotSet))
                }
            },
            NameMgmtMode::Acknowledge
            | NameMgmtMode::NegativeAcknowledge
            | NameMgmtMode::RequestPendingResponse
            | NameMgmtMode::RequestCurrentResponse => None,
            NameMgmtMode::RequestAddressClaim => {
                if nm.name() == current_name {
                    self.on_request_address_claim.emit(&current_name);
                }
                None
            }
        };

        self.on_name_management.emit(&(nm, msg.source));
        reply
    }

    /// Dispatch an incoming PGN_COMMANDED_ADDRESS message. Returns
    /// `Some(new_address)` only if the command targets `our_name`
    /// and the new address is in `0..=MAX_ADDRESS`. Fires
    /// [`Self::on_commanded_address`].
    pub fn handle_commanded_address(&mut self, msg: &Message, our_name: Name) -> Option<Address> {
        if msg.pgn != PGN_COMMANDED_ADDRESS {
            return None;
        }
        if !msg.has_usable_source() || msg.destination == NULL_ADDRESS {
            return None;
        }
        // Commanded Address payload: 8 bytes NAME + 1 byte new address.
        if msg.data.len() != 9 {
            return None;
        }
        let target = Name::from_bytes(&msg.data[..8])?;
        if target != our_name {
            return None;
        }
        let new_address = msg.data[8];
        if new_address > MAX_ADDRESS {
            tracing::warn!(
                target: "machbus.network.name_mgr",
                addr = %format_args!("0x{new_address:02X}"),
                "commanded address invalid",
            );
            return None;
        }
        tracing::info!(
            target: "machbus.network.name_mgr",
            new = %format_args!("0x{new_address:02X}"),
            "commanded address accepted",
        );
        self.on_commanded_address.emit(&new_address);
        Some(new_address)
    }

    /// Build the Commanded Address payload a commanding control function sends
    /// to relocate the CF with `target` NAME to `new_address` (ISO 11783-5):
    /// 8-byte NAME (little-endian) + 1-byte new address. It is transmitted on
    /// `PGN_COMMANDED_ADDRESS` via TP/BAM (the 9-byte payload exceeds one frame).
    #[must_use]
    pub fn build_commanded_address(target: Name, new_address: Address) -> [u8; 9] {
        let mut data = [0u8; 9];
        data[..8].copy_from_slice(&target.to_bytes());
        data[8] = new_address;
        data
    }

    fn build_reply_for_set_pending(
        &self,
        dest: Address,
        current_name: Name,
        nack_reason: Option<NameNackReason>,
    ) -> NameMgmtReply {
        match nack_reason {
            None => NameMgmtReply::new(
                dest,
                NameManagementMsg::for_name(NameMgmtMode::Acknowledge, current_name),
            ),
            Some(reason) => self.build_nack(dest, current_name, reason),
        }
    }

    fn build_nack(
        &self,
        dest: Address,
        current_name: Name,
        reason: NameNackReason,
    ) -> NameMgmtReply {
        let mut msg = NameManagementMsg::for_name(NameMgmtMode::NegativeAcknowledge, current_name);
        msg.nack_reason = reason;
        NameMgmtReply::new(dest, msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn name_with(identity: u32) -> Name {
        Name::default()
            .with_identity_number(identity)
            .with_function_code(0x80)
            .with_self_configurable(true)
    }

    fn nm_message(payload: Vec<u8>, source: Address) -> Message {
        Message::new(PGN_NAME_MANAGEMENT, payload, source)
    }

    fn commanded_address_message(payload: Vec<u8>, source: Address) -> Message {
        Message::new(PGN_COMMANDED_ADDRESS, payload, source)
    }

    // ─── encode / decode ───────────────────────────────────────────

    #[test]
    fn encode_is_seventeen_bytes() {
        let m = NameManagementMsg::for_name(NameMgmtMode::Acknowledge, name_with(0x100));
        let bytes = m.encode();
        assert_eq!(bytes.len(), 17);
        assert_eq!(bytes[0], NameMgmtMode::Acknowledge.as_u8());
        assert_eq!(&bytes[1..9], &name_with(0x100).to_bytes()[..]);
        // padding
        assert_eq!(&bytes[10..], &[0xFFu8; 7]);
    }

    #[test]
    fn nack_encodes_reason_byte() {
        let mut m =
            NameManagementMsg::for_name(NameMgmtMode::NegativeAcknowledge, name_with(0x100));
        m.nack_reason = NameNackReason::Conflict;
        let bytes = m.encode();
        assert_eq!(bytes[9], NameNackReason::Conflict.as_u8());
    }

    #[test]
    fn decode_round_trip() {
        let original =
            NameManagementMsg::for_name(NameMgmtMode::RequestCurrentResponse, name_with(0x42));
        let decoded = NameManagementMsg::decode(&original.encode()).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn decode_rejects_malformed_payloads() {
        let valid =
            NameManagementMsg::for_name(NameMgmtMode::RequestCurrentResponse, name_with(0x42))
                .encode();
        assert!(NameManagementMsg::decode(&valid[..16]).is_none());

        let mut overlong = valid.clone();
        overlong.push(0xFF);
        assert!(NameManagementMsg::decode(&overlong).is_none());

        let mut bad_mode = valid.clone();
        bad_mode[0] = 0x09;
        assert!(NameManagementMsg::decode(&bad_mode).is_none());

        let mut bad_padding = valid.clone();
        bad_padding[9] = 0x00;
        assert!(NameManagementMsg::decode(&bad_padding).is_none());

        let mut bad_nack =
            NameManagementMsg::for_name(NameMgmtMode::NegativeAcknowledge, name_with(0x42))
                .encode();
        bad_nack[9] = 0x06;
        assert!(NameManagementMsg::decode(&bad_nack).is_none());

        let mut bad_nack_padding =
            NameManagementMsg::for_name(NameMgmtMode::NegativeAcknowledge, name_with(0x42))
                .encode();
        bad_nack_padding[10] = 0x00;
        assert!(NameManagementMsg::decode(&bad_nack_padding).is_none());
    }

    // ─── set_pending / adopt_pending ──────────────────────────────

    #[test]
    fn set_pending_rejects_changed_identity() {
        let mut nm = NameManager::new();
        let r = nm.set_pending(0x100, name_with(0x200));
        assert!(r.is_err());
        assert!(!nm.has_pending());
    }

    #[test]
    fn set_pending_accepts_same_identity() {
        let mut nm = NameManager::new();
        let new = name_with(0x100).with_function_code(0x42);
        nm.set_pending(0x100, new).unwrap();
        assert!(nm.has_pending());
        assert_eq!(nm.pending_name().unwrap().function_code(), 0x42);
    }

    #[test]
    fn adopt_pending_consumes_and_fires_event() {
        let mut nm = NameManager::new();
        let new = name_with(0x100).with_function_code(0x42);
        nm.set_pending(0x100, new).unwrap();

        let log = Rc::new(RefCell::new(Vec::<Name>::new()));
        let l = log.clone();
        nm.on_name_changed
            .subscribe(move |n| l.borrow_mut().push(*n));

        let adopted = nm.adopt_pending().unwrap();
        assert_eq!(adopted, new);
        assert!(!nm.has_pending());
        assert_eq!(*log.borrow(), vec![new]);

        // Second adopt fails — pending was consumed.
        assert!(nm.adopt_pending().is_err());
    }

    // ─── handle_name_management dispatch ──────────────────────────

    #[test]
    fn request_current_returns_current_response() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let payload =
            NameManagementMsg::for_name(NameMgmtMode::RequestCurrent, name_with(0x000)).encode();
        let reply = nm
            .handle_name_management(&nm_message(payload, 0x42), current)
            .unwrap();
        assert_eq!(reply.destination, 0x42);
        assert_eq!(reply.msg.mode, NameMgmtMode::RequestCurrentResponse);
        assert_eq!(reply.msg.name(), current);
    }

    #[test]
    fn set_pending_with_matching_identity_acks() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let proposed = name_with(0x100).with_function_code(0xAB);
        let payload = NameManagementMsg::for_name(NameMgmtMode::SetPending, proposed).encode();
        let reply = nm
            .handle_name_management(&nm_message(payload, 0x42), current)
            .unwrap();
        assert_eq!(reply.msg.mode, NameMgmtMode::Acknowledge);
        assert!(nm.has_pending());
    }

    #[test]
    fn set_pending_with_different_identity_nacks_invalid_items() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let proposed = name_with(0x999); // different identity
        let payload = NameManagementMsg::for_name(NameMgmtMode::SetPending, proposed).encode();
        let reply = nm
            .handle_name_management(&nm_message(payload, 0x42), current)
            .unwrap();
        assert_eq!(reply.msg.mode, NameMgmtMode::NegativeAcknowledge);
        assert_eq!(reply.msg.nack_reason, NameNackReason::InvalidItems);
        assert!(!nm.has_pending());
    }

    #[test]
    fn request_pending_when_set_returns_pending_response() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let pending = name_with(0x100).with_function_code(0x77);
        nm.set_pending(0x100, pending).unwrap();

        let payload = NameManagementMsg::for_name(NameMgmtMode::RequestPending, current).encode();
        let reply = nm
            .handle_name_management(&nm_message(payload, 0x42), current)
            .unwrap();
        assert_eq!(reply.msg.mode, NameMgmtMode::RequestPendingResponse);
        assert_eq!(reply.msg.name(), pending);
    }

    #[test]
    fn request_pending_when_unset_nacks_pending_not_set() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let payload = NameManagementMsg::for_name(NameMgmtMode::RequestPending, current).encode();
        let reply = nm
            .handle_name_management(&nm_message(payload, 0x42), current)
            .unwrap();
        assert_eq!(reply.msg.mode, NameMgmtMode::NegativeAcknowledge);
        assert_eq!(reply.msg.nack_reason, NameNackReason::PendingNotSet);
    }

    #[test]
    fn adopt_pending_via_message_acks() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let pending = name_with(0x100).with_function_code(0x77);
        nm.set_pending(0x100, pending).unwrap();

        let payload = NameManagementMsg::for_name(NameMgmtMode::AdoptPending, current).encode();
        let reply = nm
            .handle_name_management(&nm_message(payload, 0x42), current)
            .unwrap();
        assert_eq!(reply.msg.mode, NameMgmtMode::Acknowledge);
        assert!(!nm.has_pending());
    }

    #[test]
    fn adopt_pending_when_unset_nacks() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let payload = NameManagementMsg::for_name(NameMgmtMode::AdoptPending, current).encode();
        let reply = nm
            .handle_name_management(&nm_message(payload, 0x42), current)
            .unwrap();
        assert_eq!(reply.msg.mode, NameMgmtMode::NegativeAcknowledge);
        assert_eq!(reply.msg.nack_reason, NameNackReason::PendingNotSet);
    }

    #[test]
    fn unhandled_modes_return_no_reply_but_emit_event() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        nm.on_name_management
            .subscribe(move |_| *c.borrow_mut() += 1);

        let payload =
            NameManagementMsg::for_name(NameMgmtMode::RequestAddressClaim, current).encode();
        assert!(
            nm.handle_name_management(&nm_message(payload, 0x42), current)
                .is_none()
        );
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn response_modes_do_not_generate_response_loops_or_state_changes() {
        let mut nm = NameManager::new();
        let current = name_with(0x100);
        let pending = current.with_function_code(0x55);
        nm.set_pending(current.identity_number(), pending).unwrap();

        let log = Rc::new(RefCell::new(Vec::<NameMgmtMode>::new()));
        let l = log.clone();
        nm.on_name_management
            .subscribe(move |(msg, _)| l.borrow_mut().push(msg.mode));

        for mode in [
            NameMgmtMode::Acknowledge,
            NameMgmtMode::NegativeAcknowledge,
            NameMgmtMode::RequestCurrentResponse,
            NameMgmtMode::RequestPendingResponse,
        ] {
            let mut msg = NameManagementMsg::for_name(mode, current);
            if mode == NameMgmtMode::NegativeAcknowledge {
                msg.nack_reason = NameNackReason::Conflict;
            }
            assert!(
                nm.handle_name_management(&nm_message(msg.encode(), 0x42), current)
                    .is_none(),
                "{mode:?} must be observed without replying"
            );
            assert_eq!(
                nm.pending_name(),
                Some(pending),
                "{mode:?} must not mutate the pending NAME"
            );
        }

        assert_eq!(
            *log.borrow(),
            vec![
                NameMgmtMode::Acknowledge,
                NameMgmtMode::NegativeAcknowledge,
                NameMgmtMode::RequestCurrentResponse,
                NameMgmtMode::RequestPendingResponse,
            ]
        );
    }

    #[test]
    fn handle_name_mgmt_short_payload_is_no_op() {
        let mut nm = NameManager::new();
        let msg = nm_message(vec![0u8; 5], 0x42);
        assert!(nm.handle_name_management(&msg, name_with(0x100)).is_none());
    }

    // ─── Commanded address ─────────────────────────────────────────

    #[test]
    fn commanded_address_targeting_us_returns_new_addr() {
        let mut nm = NameManager::new();
        let our = name_with(0x100);
        let mut payload = our.to_bytes().to_vec();
        payload.push(0x42); // new address
        let r = nm.handle_commanded_address(&commanded_address_message(payload, 0x10), our);
        assert_eq!(r, Some(0x42));
    }

    #[test]
    fn commanded_address_builder_round_trips_through_handler() {
        let our = name_with(0x100);
        // A commanding CF builds the payload; the target decodes it back.
        let payload = NameManager::build_commanded_address(our, 0x42);
        assert_eq!(payload.len(), 9);
        let mut nm = NameManager::new();
        let r =
            nm.handle_commanded_address(&commanded_address_message(payload.to_vec(), 0x10), our);
        assert_eq!(r, Some(0x42));
        // Built for a different NAME ⇒ our handler ignores it.
        let other = NameManager::build_commanded_address(name_with(0x200), 0x42);
        assert_eq!(
            nm.handle_commanded_address(&commanded_address_message(other.to_vec(), 0x10), our),
            None
        );
    }

    #[test]
    fn commanded_address_targeting_someone_else_returns_none() {
        let mut nm = NameManager::new();
        let our = name_with(0x100);
        let other = name_with(0x999);
        let mut payload = other.to_bytes().to_vec();
        payload.push(0x42);
        assert!(
            nm.handle_commanded_address(&commanded_address_message(payload, 0x10), our)
                .is_none()
        );
    }

    #[test]
    fn commanded_address_invalid_addr_returns_none() {
        let mut nm = NameManager::new();
        let our = name_with(0x100);
        let mut payload = our.to_bytes().to_vec();
        payload.push(0xFE); // NULL_ADDRESS — > MAX_ADDRESS
        assert!(
            nm.handle_commanded_address(&commanded_address_message(payload, 0x10), our)
                .is_none()
        );
    }

    #[test]
    fn commanded_address_short_payload_is_no_op() {
        let mut nm = NameManager::new();
        let our = name_with(0x100);
        let payload = vec![0u8; 8]; // missing addr byte
        assert!(
            nm.handle_commanded_address(&commanded_address_message(payload, 0x10), our)
                .is_none()
        );
    }

    #[test]
    fn commanded_address_overlong_payload_is_no_op() {
        let mut nm = NameManager::new();
        let our = name_with(0x100);
        let mut payload = our.to_bytes().to_vec();
        payload.push(0x42);
        payload.push(0xFF);
        assert!(
            nm.handle_commanded_address(&commanded_address_message(payload, 0x10), our)
                .is_none()
        );
    }
}
