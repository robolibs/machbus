use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};

use super::constants::{BROADCAST_ADDRESS, MAX_ADDRESS, NULL_ADDRESS};
use super::error::{Error, ErrorCode, Result};
use super::event::Event;
use super::frame::Frame;
use super::identifier::Identifier;
use super::message::Message;
use super::name::Name;
use super::pgn::pgn_is_valid;
use super::pgn_defs::{PGN_ADDRESS_CLAIMED, PGN_NIU_NETWORK_MSG};
use super::types::{Address, Pgn};

const DEFAULT_LOOP_GUARD_WINDOW_MS: u32 = 250;
const DEFAULT_LOOP_GUARD_MAX_RECENT_FORWARDS: usize = 256;

// ─── Enums ──────────────────────────────────────────────────────────────

/// Per-frame forwarding decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ForwardPolicy {
    #[default]
    Allow,
    Block,
    /// Forward, and additionally fire `on_monitored`.
    Monitor,
}

impl ForwardPolicy {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match Self::try_from_u8(value) {
            Some(policy) => policy,
            None => Self::Monitor,
        }
    }

    #[inline]
    #[must_use]
    pub const fn try_from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Allow),
            1 => Some(Self::Block),
            2 => Some(Self::Monitor),
            _ => None,
        }
    }
}

/// Which side of the bridge a frame originated on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum Side {
    #[default]
    Tractor,
    Implement,
}

impl Side {
    #[inline]
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Tractor => Self::Implement,
            Self::Implement => Self::Tractor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameSignature {
    raw_id: u32,
    length: u8,
    data: [u8; 8],
}

impl FrameSignature {
    #[must_use]
    fn from_frame(frame: &Frame) -> Self {
        let length = frame.length.min(8);
        let mut data = [0u8; 8];
        let n = length as usize;
        data[..n].copy_from_slice(&frame.data[..n]);
        Self {
            raw_id: frame.id.raw,
            length,
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecentForward {
    target_side: Side,
    signature: FrameSignature,
    forwarded_at_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum NiuState {
    #[default]
    Inactive,
    Active,
    Error,
}

/// ISO 11783-4 Table 4 — filter mode.
///
/// The two values were bound the wrong way round: `0` was `BlockAll` and `1`
/// was `PassAll`, which is the exact inverse of the table and of §6.2.2/§6.2.3.
/// A bridge configured for the standard's "preferred mode of operation" was
/// therefore blocking everything it should have forwarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum NiuFilterMode {
    /// `0` — "Block-specific PGNs (default = pass all)". §6.2.2: "the NIU shall
    /// default to forwarding all messages"; listed PGNs are blocked.
    #[default]
    BlockSpecific = 0,
    /// `1` — "Pass-specific PGNs (default = block all)". §6.2.3: "the NIU shall
    /// default to not forwarding messages"; only listed PGNs pass.
    PassSpecific = 1,
}

impl NiuFilterMode {
    #[inline]
    #[must_use]
    pub const fn try_from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::BlockSpecific),
            1 => Some(Self::PassSpecific),
            _ => None,
        }
    }
}

/// NIU Network Message function codes — ISO 11783-4 Table 2.
///
/// Every value here previously disagreed with the table: the codes were a
/// locally invented 1..=15 sequence, so no message this stack sent or accepted
/// matched a conforming NIU. Two of the operations it defined —
/// request/set filter mode — have no Table 2 code at all, and §6.6.2.3.3 says
/// why: the filter mode "cannot be changed without clearing and rebuilding the
/// database for that port pair", so it is configured out of band, not over the
/// wire. They are gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum NiuFunction {
    /// §6.6.2.3.1 — request a copy of the filter database (CF → NIU).
    #[default]
    RequestFilterDb = 0,
    /// §6.6.2.3.2 — response to a filter-database request (NIU → CF).
    FilterDbResponse = 1,
    /// §6.6.2.3.3 — add an entry to the filter database (CF → NIU).
    AddFilterEntry = 2,
    /// §6.6.2.3.4 — delete an entry from the filter database (CF → NIU).
    DeleteFilterEntry = 3,
    /// §6.6.2.3.5 — clear an entry from the filter database (CF → NIU).
    ClearFilterEntry = 4,
    // 5 is "Obsolete, not to be used" and is rejected on decode.
    /// §6.6.2.3.6 — create a filter database entry (CF → NIU).
    CreateFilterEntry = 6,
    /// §6.6.2.3.7 — request to add NAME-qualified entries (CF → NIU).
    AddNameQualifiedEntries = 7,
    /// §6.7.2.1 — request a list of source addresses (CF → NIU).
    RequestSourceAddressList = 64,
    /// §6.7.2.2 — response to a source-address list request (NIU → CF).
    SourceAddressListResponse = 65,
    /// §6.7.2.3 — request a source address and NAME list (CF → NIU).
    RequestSourceAddressNameList = 66,
    /// §6.7.2.4 — response to a source address and NAME request (NIU → CF).
    SourceAddressNameListResponse = 67,
    /// §6.8.3.1 — request NIU general parametrics (CF → NIU).
    RequestGeneralParametrics = 128,
    /// §6.8.3.2 — response to a general-parametrics request (NIU → CF).
    GeneralParametricsResponse = 129,
    /// §6.8.3.3 — reset general statistic parameters (CF → NIU).
    ResetGeneralStatistics = 130,
    /// §6.8.4.1 — request NIU-specific parametrics (CF → NIU).
    RequestSpecificParametrics = 131,
    /// §6.8.4.2 — response to a specific-parametrics request (NIU → CF).
    SpecificParametricsResponse = 132,
    /// §6.8.4.3 — reset specific statistic parameters (CF → NIU).
    ResetSpecificStatistics = 133,
    /// §6.9.5.1 — request to open a connection (CF → NIU).
    OpenConnection = 192,
    /// §6.9.5.2 — response to an open-connection request (NIU → CF).
    OpenConnectionResponse = 193,
    /// §6.9.5.3 — request to close a connection (CF → NIU).
    CloseConnection = 194,
    /// §6.9.5.4 — response to a close-connection request (NIU → CF).
    CloseConnectionResponse = 195,
}

impl NiuFunction {
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match Self::try_from_u8(value) {
            Some(function) => function,
            None => Self::RequestFilterDb,
        }
    }

    /// Decode a Table 2 function code.
    ///
    /// Returns `None` for code 5 ("Obsolete, not to be used") and for every
    /// reserved band: 8..=63, 68..=127, 134..=191 and 196..=255.
    #[must_use]
    pub const fn try_from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::RequestFilterDb),
            1 => Some(Self::FilterDbResponse),
            2 => Some(Self::AddFilterEntry),
            3 => Some(Self::DeleteFilterEntry),
            4 => Some(Self::ClearFilterEntry),
            6 => Some(Self::CreateFilterEntry),
            7 => Some(Self::AddNameQualifiedEntries),
            64 => Some(Self::RequestSourceAddressList),
            65 => Some(Self::SourceAddressListResponse),
            66 => Some(Self::RequestSourceAddressNameList),
            67 => Some(Self::SourceAddressNameListResponse),
            128 => Some(Self::RequestGeneralParametrics),
            129 => Some(Self::GeneralParametricsResponse),
            130 => Some(Self::ResetGeneralStatistics),
            131 => Some(Self::RequestSpecificParametrics),
            132 => Some(Self::SpecificParametricsResponse),
            133 => Some(Self::ResetSpecificStatistics),
            192 => Some(Self::OpenConnection),
            193 => Some(Self::OpenConnectionResponse),
            194 => Some(Self::CloseConnection),
            195 => Some(Self::CloseConnectionResponse),
            _ => None,
        }
    }

    /// The Table 2 code for this function.
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

// ─── FilterRule ────────────────────────────────────────────────────────

/// One forwarding rule. PGN `0` means "any PGN" (used with NAME-based
/// filters).
#[derive(Debug, Clone)]
pub struct FilterRule {
    pub pgn: Pgn,
    pub policy: ForwardPolicy,
    /// `true` ⇒ rule applies in both directions; `false` ⇒
    /// tractor-side only (matches C++).
    pub bidirectional: bool,

    pub source_name: Option<Name>,
    pub destination_name: Option<Name>,

    /// Minimum interval (ms) between forwards. `0` disables rate
    /// limiting.
    pub max_frequency_ms: u32,
    /// Last successful forward timestamp (`now_ms` argument). `None`
    /// before any forward has happened — distinguishes "never seen"
    /// from "seen at time 0", which the C++ port silently confuses.
    pub last_forward_time_ms: Option<u32>,

    /// Survives [`Niu::clear_filters`] when persistence is loaded.
    pub persistent: bool,
}

/// Runtime-independent snapshot of one NIU filter rule.
///
/// This intentionally omits [`FilterRule::last_forward_time_ms`], because that
/// field is mutable rate-limiter state, not operator policy. Use this shape for
/// policy dumps, regression tests, and UI/audit displays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterRuleSnapshot {
    pub pgn: Pgn,
    pub policy: ForwardPolicy,
    pub bidirectional: bool,
    pub source_name: Option<Name>,
    pub destination_name: Option<Name>,
    pub max_frequency_ms: u32,
    pub persistent: bool,
}

impl From<&FilterRule> for FilterRuleSnapshot {
    fn from(rule: &FilterRule) -> Self {
        Self {
            pgn: rule.pgn,
            policy: rule.policy,
            bidirectional: rule.bidirectional,
            source_name: rule.source_name,
            destination_name: rule.destination_name,
            max_frequency_ms: rule.max_frequency_ms,
            persistent: rule.persistent,
        }
    }
}

/// Runtime-independent snapshot of NIU policy/configuration.
///
/// This intentionally excludes mutable runtime state such as counters, learned
/// address-claim NAMEs, rate-limiter timestamps, and loop-guard history. It is
/// suitable for operator policy dumps and regression fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NiuPolicySnapshot {
    pub name: String,
    pub filter_mode: NiuFilterMode,
    pub forward_global_by_default: bool,
    pub forward_specific_by_default: bool,
    pub loop_guard_window_ms: u32,
    pub loop_guard_max_recent_forwards: usize,
    pub persistence_file: Option<String>,
    pub filters: Vec<FilterRuleSnapshot>,
}

impl FilterRule {
    #[must_use]
    pub const fn new(pgn: Pgn, policy: ForwardPolicy, bidirectional: bool) -> Self {
        Self {
            pgn,
            policy,
            bidirectional,
            source_name: None,
            destination_name: None,
            max_frequency_ms: 0,
            last_forward_time_ms: None,
            persistent: false,
        }
    }

    #[must_use]
    pub fn with_source_name(mut self, name: Name) -> Self {
        self.source_name = Some(name);
        self
    }

    #[must_use]
    pub fn with_destination_name(mut self, name: Name) -> Self {
        self.destination_name = Some(name);
        self
    }

    #[must_use]
    pub const fn with_max_frequency_ms(mut self, ms: u32) -> Self {
        self.max_frequency_ms = ms;
        self
    }

    #[must_use]
    pub const fn persistent(mut self, p: bool) -> Self {
        self.persistent = p;
        self
    }

    /// Encode for storage (22 bytes).
    pub fn encode(&self) -> Result<Vec<u8>> {
        if !pgn_is_valid(self.pgn) {
            return Err(Error::invalid_data(format!(
                "NIU filter PGN 0x{:X} exceeds the 18-bit J1939/ISOBUS PGN range",
                self.pgn
            )));
        }
        if self.max_frequency_ms > u32::from(u16::MAX) {
            return Err(Error::invalid_data(format!(
                "NIU filter max_frequency_ms {} exceeds the 16-bit storage field",
                self.max_frequency_ms
            )));
        }

        let mut data = Vec::with_capacity(22);
        // PGN (3 bytes).
        data.push((self.pgn & 0xFF) as u8);
        data.push(((self.pgn >> 8) & 0xFF) as u8);
        data.push(((self.pgn >> 16) & 0x03) as u8);
        // Flags (1 byte).
        let mut flags = self.policy.as_u8() & 0x03;
        if self.bidirectional {
            flags |= 0x04;
        }
        if self.persistent {
            flags |= 0x08;
        }
        if self.source_name.is_some() {
            flags |= 0x10;
        }
        if self.destination_name.is_some() {
            flags |= 0x20;
        }
        data.push(flags);
        // Source NAME (8 bytes; 0xFF×8 if absent).
        let src_bytes = self.source_name.map_or([0xFFu8; 8], Name::to_bytes);
        data.extend_from_slice(&src_bytes);
        // Destination NAME (8 bytes; 0xFF×8 if absent).
        let dst_bytes = self.destination_name.map_or([0xFFu8; 8], Name::to_bytes);
        data.extend_from_slice(&dst_bytes);
        // Max frequency (2 bytes LE).
        data.push((self.max_frequency_ms & 0xFF) as u8);
        data.push(((self.max_frequency_ms >> 8) & 0xFF) as u8);
        Ok(data)
    }

    /// Decode from a 22-byte buffer.
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() != 22 {
            return Err(Error::invalid_data("filter rule must be exactly 22 bytes"));
        }
        if (data[2] & !0x03) != 0 {
            return Err(Error::invalid_data(
                "filter rule PGN high bits are reserved",
            ));
        }
        let pgn = (data[0] as Pgn) | ((data[1] as Pgn) << 8) | (((data[2] & 0x03) as Pgn) << 16);
        let flags = data[3];
        if (flags & 0xC0) != 0 {
            return Err(Error::invalid_data(
                "filter rule flags contain reserved bits",
            ));
        }
        let policy = match flags & 0x03 {
            0 => ForwardPolicy::Allow,
            1 => ForwardPolicy::Block,
            2 => ForwardPolicy::Monitor,
            _ => return Err(Error::invalid_data("filter rule policy is reserved")),
        };
        let bidirectional = (flags & 0x04) != 0;
        let persistent = (flags & 0x08) != 0;
        let has_source = (flags & 0x10) != 0;
        let has_dest = (flags & 0x20) != 0;
        if !has_source && data[4..12].iter().any(|&byte| byte != 0xFF) {
            return Err(Error::invalid_data(
                "filter rule absent source NAME must be padded with 0xFF",
            ));
        }
        if !has_dest && data[12..20].iter().any(|&byte| byte != 0xFF) {
            return Err(Error::invalid_data(
                "filter rule absent destination NAME must be padded with 0xFF",
            ));
        }
        let source_name = has_source.then(|| Name::from_bytes(&data[4..12]).unwrap());
        let destination_name = has_dest.then(|| Name::from_bytes(&data[12..20]).unwrap());
        let max_frequency_ms = (data[20] as u32) | ((data[21] as u32) << 8);
        Ok(Self {
            pgn,
            policy,
            bidirectional,
            source_name,
            destination_name,
            max_frequency_ms,
            last_forward_time_ms: None,
            persistent,
        })
    }
}

fn filter_snapshot_sort_key(snapshot: &FilterRuleSnapshot) -> (Pgn, u8, bool, u64, u64, u32, bool) {
    (
        snapshot.pgn,
        snapshot.policy.as_u8(),
        snapshot.bidirectional,
        snapshot.source_name.map_or(u64::MAX, |name| name.raw),
        snapshot.destination_name.map_or(u64::MAX, |name| name.raw),
        snapshot.max_frequency_ms,
        snapshot.persistent,
    )
}

// ─── NIU Network Message ───────────────────────────────────────────────

/// Wire-format NIU control message (PGN `0xED00`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NiuNetworkMsg {
    pub function: NiuFunction,
    pub port_number: u8,
    pub filter_pgn: Pgn,
    pub filter_mode: NiuFilterMode,
    pub msgs_forwarded: u32,
    pub msgs_blocked: u32,
}

impl Default for NiuNetworkMsg {
    fn default() -> Self {
        Self {
            function: NiuFunction::RequestFilterDb,
            port_number: 0,
            filter_pgn: 0,
            filter_mode: NiuFilterMode::BlockSpecific,
            msgs_forwarded: 0,
            msgs_blocked: 0,
        }
    }
}

/// N.MFDB_Response — a copy of one port pair's filter database (§6.6.2.3.2).
///
/// This message is **variable length**: byte 2 carries the port pair, byte 3
/// the filter mode, and bytes 4..n the PGN entries, three bytes each. It was
/// previously encoded through the 8-byte `AddFilterEntry` layout, which put a
/// single PGN where the port pair and filter mode belong and could not carry
/// more than one entry at all — so Table 5's own worked example was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NiuFilterDbResponse {
    /// Bits 7-4 of byte 2.
    pub from_port: u8,
    /// Bits 3-0 of byte 2.
    pub to_port: u8,
    /// Filter mode for this port pair (§6.6.2.3.3).
    pub filter_mode: u8,
    /// The filtered PGNs, in order.
    pub entries: Vec<Pgn>,
}

impl NiuFilterDbResponse {
    /// Encode to the variable-length wire form.
    ///
    /// # Errors
    /// [`Error::invalid_data`] when a port nibble or an entry PGN is out of
    /// range.
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.from_port > 0x0F || self.to_port > 0x0F {
            return Err(Error::invalid_data(
                "NIU port numbers occupy one nibble each",
            ));
        }
        let mut data = Vec::with_capacity(3 + self.entries.len() * 3);
        data.push(NiuFunction::FilterDbResponse as u8);
        data.push((self.from_port << 4) | self.to_port);
        data.push(self.filter_mode);
        for &pgn in &self.entries {
            if !pgn_is_valid(pgn) {
                return Err(Error::invalid_data(format!(
                    "NIU filter PGN 0x{pgn:X} exceeds the 18-bit J1939/ISOBUS PGN range"
                )));
            }
            data.push((pgn & 0xFF) as u8);
            data.push(((pgn >> 8) & 0xFF) as u8);
            data.push(((pgn >> 16) & 0x03) as u8);
        }
        Ok(data)
    }

    /// Decode the variable-length wire form. Returns `None` for a payload that
    /// is too short, carries the wrong function, or whose entry region is not a
    /// whole number of 3-byte PGNs.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 3 || data[0] != NiuFunction::FilterDbResponse as u8 {
            return None;
        }
        let entries_region = &data[3..];
        if !entries_region.len().is_multiple_of(3) {
            return None;
        }
        let mut entries = Vec::with_capacity(entries_region.len() / 3);
        for chunk in entries_region.chunks_exact(3) {
            if chunk[2] & 0xFC != 0 {
                return None;
            }
            let pgn = (chunk[0] as Pgn) | ((chunk[1] as Pgn) << 8) | (((chunk[2] & 0x03) as Pgn) << 16);
            if !pgn_is_valid(pgn) {
                return None;
            }
            entries.push(pgn);
        }
        Some(Self {
            from_port: data[1] >> 4,
            to_port: data[1] & 0x0F,
            filter_mode: data[2],
            entries,
        })
    }
}

impl NiuNetworkMsg {
    /// Encode to the standard 8-byte wire format (padded with `0xFF`).
    pub fn encode(&self) -> Result<[u8; 8]> {
        let mut data = [0xFFu8; 8];
        data[0] = self.function as u8;
        data[1] = self.port_number;
        match self.function {
            NiuFunction::AddFilterEntry | NiuFunction::DeleteFilterEntry => {
                if !pgn_is_valid(self.filter_pgn) {
                    return Err(Error::invalid_data(format!(
                        "NIU filter PGN 0x{:X} exceeds the 18-bit J1939/ISOBUS PGN range",
                        self.filter_pgn
                    )));
                }
                data[2] = (self.filter_pgn & 0xFF) as u8;
                data[3] = ((self.filter_pgn >> 8) & 0xFF) as u8;
                data[4] = ((self.filter_pgn >> 16) & 0x03) as u8;
            }
            NiuFunction::GeneralParametricsResponse => {
                let forwarded = self.msgs_forwarded.min(u32::from(u16::MAX));
                let blocked = self.msgs_blocked.min(u32::from(u16::MAX));
                data[2] = (forwarded & 0xFF) as u8;
                data[3] = ((forwarded >> 8) & 0xFF) as u8;
                data[4] = (blocked & 0xFF) as u8;
                data[5] = ((blocked >> 8) & 0xFF) as u8;
            }
            _ => {}
        }
        Ok(data)
    }

    /// Decode from the canonical 8-byte payload.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }

        let function = NiuFunction::try_from_u8(data[0])?;
        let mut msg = Self {
            function,
            port_number: data[1],
            ..Default::default()
        };
        match msg.function {
            NiuFunction::AddFilterEntry | NiuFunction::DeleteFilterEntry => {
                if (data[4] & 0xFC) != 0 || data[5..].iter().any(|&b| b != 0xFF) {
                    return None;
                }
                msg.filter_pgn =
                    (data[2] as Pgn) | ((data[3] as Pgn) << 8) | (((data[4] & 0x03) as Pgn) << 16);
            }
            NiuFunction::GeneralParametricsResponse => {
                if data[6..].iter().any(|&b| b != 0xFF) {
                    return None;
                }
                msg.msgs_forwarded = (data[2] as u32) | ((data[3] as u32) << 8);
                msg.msgs_blocked = (data[4] as u32) | ((data[5] as u32) << 8);
            }
            _ => {
                if data[2..].iter().any(|&b| b != 0xFF) {
                    return None;
                }
            }
        }
        Some(msg)
    }

    /// Alias for [`Self::decode`] kept for call sites that spell fallible
    /// decoders as `try_decode`.
    #[must_use]
    pub fn try_decode(data: &[u8]) -> Option<Self> {
        Self::decode(data)
    }
}

// ─── NiuConfig ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NiuConfig {
    pub name: String,
    pub forward_global_by_default: bool,
    pub forward_specific_by_default: bool,
    pub filter_mode: NiuFilterMode,
    /// Recent forwarded-frame memory depth for loop prevention.
    ///
    /// A value of `0` disables loop-guard storage even when
    /// `loop_guard_window_ms` is non-zero. Keep this bounded for
    /// storm-resistance; raise it in lab topologies with many bridged ports.
    pub loop_guard_max_recent_forwards: usize,
    pub loop_guard_window_ms: u32,
    pub persistence_file: Option<String>,
}

impl Default for NiuConfig {
    fn default() -> Self {
        Self {
            name: "NIU".to_string(),
            forward_global_by_default: true,
            forward_specific_by_default: true,
            filter_mode: NiuFilterMode::BlockSpecific,
            loop_guard_max_recent_forwards: DEFAULT_LOOP_GUARD_MAX_RECENT_FORWARDS,
            loop_guard_window_ms: DEFAULT_LOOP_GUARD_WINDOW_MS,
            persistence_file: None,
        }
    }
}

impl NiuConfig {
    #[must_use]
    pub fn name(mut self, n: impl Into<String>) -> Self {
        self.name = n.into();
        self
    }

    #[must_use]
    pub fn global_default(mut self, allow: bool) -> Self {
        self.forward_global_by_default = allow;
        self
    }

    #[must_use]
    pub fn specific_default(mut self, allow: bool) -> Self {
        self.forward_specific_by_default = allow;
        self
    }

    #[must_use]
    pub fn mode(mut self, m: NiuFilterMode) -> Self {
        self.filter_mode = m;
        self
    }

    #[must_use]
    pub const fn loop_guard_window_ms(mut self, ms: u32) -> Self {
        self.loop_guard_window_ms = ms;
        self
    }

    #[must_use]
    pub const fn loop_guard_capacity(mut self, capacity: usize) -> Self {
        self.loop_guard_max_recent_forwards = capacity;
        self
    }

    #[must_use]
    pub fn persistence(mut self, file: impl Into<String>) -> Self {
        self.persistence_file = Some(file.into());
        self
    }

    /// Serialize the config to a stable line-based `key=value` text format
    /// (dependency-free). The `persistence_file` path is intentionally not
    /// serialized — it identifies the store, not the stored content.
    #[must_use]
    pub fn to_persisted_string(&self) -> String {
        format!(
            "name={}\nforward_global_by_default={}\nforward_specific_by_default={}\nfilter_mode={}\nloop_guard_max_recent_forwards={}\nloop_guard_window_ms={}\n",
            self.name,
            self.forward_global_by_default,
            self.forward_specific_by_default,
            self.filter_mode as u8,
            self.loop_guard_max_recent_forwards,
            self.loop_guard_window_ms,
        )
    }

    /// Parse a config from [`to_persisted_string`](Self::to_persisted_string)
    /// output. Unknown keys are ignored and absent keys keep their default,
    /// so the format tolerates forward/backward evolution.
    #[must_use]
    pub fn from_persisted_string(text: &str) -> Self {
        let mut cfg = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "name" => cfg.name = value.to_string(),
                "forward_global_by_default" => {
                    cfg.forward_global_by_default = value == "true";
                }
                "forward_specific_by_default" => {
                    cfg.forward_specific_by_default = value == "true";
                }
                "filter_mode" => {
                    if let Ok(b) = value.parse::<u8>()
                        && let Some(m) = NiuFilterMode::try_from_u8(b)
                    {
                        cfg.filter_mode = m;
                    }
                }
                "loop_guard_max_recent_forwards" => {
                    if let Ok(n) = value.parse() {
                        cfg.loop_guard_max_recent_forwards = n;
                    }
                }
                "loop_guard_window_ms" => {
                    if let Ok(n) = value.parse() {
                        cfg.loop_guard_window_ms = n;
                    }
                }
                _ => {}
            }
        }
        cfg
    }

    /// Persist the config to its `persistence_file`. Returns `Ok(false)`
    /// (no-op) when no persistence file is configured.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn save(&self) -> std::io::Result<bool> {
        match &self.persistence_file {
            Some(path) => {
                std::fs::write(path, self.to_persisted_string())?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Load a config from `path`, keeping `path` as the persistence file.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn load_from(path: impl Into<String>) -> std::io::Result<Self> {
        let path = path.into();
        let text = std::fs::read_to_string(&path)?;
        let mut cfg = Self::from_persisted_string(&text);
        cfg.persistence_file = Some(path);
        Ok(cfg)
    }
}

// ─── Niu (base) ────────────────────────────────────────────────────────

/// Base NIU: filter + rate-limit forwarder. No address translation —
/// see [`Router`] for that.
pub struct Niu {
    config: NiuConfig,
    filters: Vec<FilterRule>,
    observed_names: BTreeMap<(Side, Address), Name>,
    recent_forwards: Vec<RecentForward>,
    state: NiuState,
    forwarded_count: u32,
    blocked_count: u32,
    rate_limited_count: u32,

    pub on_forwarded: Event<(Frame, Side)>,
    pub on_blocked: Event<(Frame, Side)>,
    pub on_monitored: Event<(Frame, Side)>,
    pub on_niu_message: Event<(NiuNetworkMsg, Address)>,
}

impl Niu {
    #[must_use]
    pub fn new(config: NiuConfig) -> Self {
        Self {
            config,
            filters: Vec::new(),
            observed_names: BTreeMap::new(),
            recent_forwards: Vec::new(),
            state: NiuState::Inactive,
            forwarded_count: 0,
            blocked_count: 0,
            rate_limited_count: 0,
            on_forwarded: Event::new(),
            on_blocked: Event::new(),
            on_monitored: Event::new(),
            on_niu_message: Event::new(),
        }
    }

    // ─── Filter management ─────────────────────────────────────────

    pub fn add_filter(&mut self, rule: FilterRule) -> &mut Self {
        self.filters.push(rule);
        self
    }

    pub fn allow_pgn(&mut self, pgn: Pgn, bidirectional: bool) -> &mut Self {
        self.add_filter(FilterRule::new(pgn, ForwardPolicy::Allow, bidirectional))
    }

    pub fn block_pgn(&mut self, pgn: Pgn, bidirectional: bool) -> &mut Self {
        self.add_filter(FilterRule::new(pgn, ForwardPolicy::Block, bidirectional))
    }

    pub fn monitor_pgn(&mut self, pgn: Pgn, bidirectional: bool) -> &mut Self {
        self.add_filter(FilterRule::new(pgn, ForwardPolicy::Monitor, bidirectional))
    }

    pub fn allow_pgn_rate_limited(
        &mut self,
        pgn: Pgn,
        min_interval_ms: u32,
        bidirectional: bool,
    ) -> &mut Self {
        self.add_filter(
            FilterRule::new(pgn, ForwardPolicy::Allow, bidirectional)
                .with_max_frequency_ms(min_interval_ms),
        )
    }

    /// Clear runtime-loaded filter rules while retaining persistent policy.
    ///
    /// The NIU network-control `DeleteAllEntries` function still removes the
    /// whole table, including persistent rules. This helper is for local
    /// runtime reloads where persistent rules are the baseline to keep.
    pub fn clear_filters(&mut self) {
        self.filters.retain(|rule| rule.persistent);
    }

    pub fn clear_observed_names(&mut self) {
        self.observed_names.clear();
    }

    pub fn clear_loop_guard(&mut self) {
        self.recent_forwards.clear();
    }

    #[must_use]
    pub fn observed_name(&self, side: Side, address: Address) -> Option<Name> {
        self.observed_names.get(&(side, address)).copied()
    }

    #[must_use]
    pub fn filters(&self) -> &[FilterRule] {
        &self.filters
    }

    /// Return the configured filter policy in deterministic order.
    ///
    /// Rate-limiter runtime state is intentionally excluded. This makes the
    /// snapshot stable before and after traffic has exercised a rate-limited
    /// rule.
    #[must_use]
    pub fn filter_snapshot(&self) -> Vec<FilterRuleSnapshot> {
        let mut out: Vec<_> = self.filters.iter().map(FilterRuleSnapshot::from).collect();
        out.sort_by_key(filter_snapshot_sort_key);
        out
    }

    /// Return a deterministic policy/configuration dump for this NIU.
    ///
    /// The snapshot deliberately omits mutable runtime state, so it remains
    /// stable after traffic has changed counters, learned NAMEs, rate-limit
    /// timestamps, or loop-guard entries.
    #[must_use]
    pub fn policy_snapshot(&self) -> NiuPolicySnapshot {
        NiuPolicySnapshot {
            name: self.config.name.clone(),
            filter_mode: self.config.filter_mode,
            forward_global_by_default: self.config.forward_global_by_default,
            forward_specific_by_default: self.config.forward_specific_by_default,
            loop_guard_window_ms: self.config.loop_guard_window_ms,
            loop_guard_max_recent_forwards: self.config.loop_guard_max_recent_forwards,
            persistence_file: self.config.persistence_file.clone(),
            filters: self.filter_snapshot(),
        }
    }

    #[must_use]
    pub fn filter_mode(&self) -> NiuFilterMode {
        self.config.filter_mode
    }

    pub fn set_filter_mode(&mut self, mode: NiuFilterMode) {
        self.config.filter_mode = mode;
        let pass = matches!(mode, NiuFilterMode::BlockSpecific);
        self.config.forward_global_by_default = pass;
        self.config.forward_specific_by_default = pass;
        tracing::info!(
            target: "machbus.niu",
            mode = ?mode,
            "filter mode changed",
        );
    }

    // ─── Lifecycle ─────────────────────────────────────────────────

    pub fn start(&mut self) -> Result<()> {
        self.state = NiuState::Active;
        tracing::info!(target: "machbus.niu", name = %self.config.name, "started");
        Ok(())
    }

    pub fn stop(&mut self) {
        self.state = NiuState::Inactive;
        tracing::info!(target: "machbus.niu", name = %self.config.name, "stopped");
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> NiuState {
        self.state
    }

    #[inline]
    #[must_use]
    pub fn forwarded(&self) -> u32 {
        self.forwarded_count
    }

    #[inline]
    #[must_use]
    pub fn blocked(&self) -> u32 {
        self.blocked_count
    }

    /// Frames dropped specifically because a filter rule's rate limit was
    /// exceeded (a subset of [`blocked`](Self::blocked)). Distinguishing these
    /// from policy blocks helps diagnose a NIU that is silently throttling.
    #[inline]
    #[must_use]
    pub fn rate_limited(&self) -> u32 {
        self.rate_limited_count
    }

    #[inline]
    #[must_use]
    pub fn config(&self) -> &NiuConfig {
        &self.config
    }

    // ─── Frame processing ──────────────────────────────────────────

    /// Decide what to do with a frame arriving from `origin`.
    /// Returns the frame to send on the *other* side, or `None` if it
    /// is dropped (blocked, rate-limited, or NIU inactive).
    ///
    /// `now_ms` is the current monotonic time used for rate limiting.
    pub fn process_frame(&mut self, frame: Frame, origin: Side, now_ms: u32) -> Option<Frame> {
        self.process_frame_inner(frame, origin, now_ms, true)
    }

    fn process_frame_inner(
        &mut self,
        frame: Frame,
        origin: Side,
        now_ms: u32,
        remember_on_forward: bool,
    ) -> Option<Frame> {
        if !matches!(self.state, NiuState::Active) {
            return None;
        }
        let pgn = frame.pgn();
        if frame.source() == BROADCAST_ADDRESS
            || (frame.source() == NULL_ADDRESS && pgn != PGN_ADDRESS_CLAIMED)
        {
            self.blocked_count = self.blocked_count.saturating_add(1);
            self.on_blocked.emit(&(frame, origin));
            tracing::debug!(
                target: "machbus.niu",
                pgn,
                ?origin,
                source = %format_args!("0x{:02X}", frame.source()),
                "blocked frame with invalid source address",
            );
            return None;
        }
        if self.is_loop_echo(&frame, origin, now_ms) {
            self.blocked_count = self.blocked_count.saturating_add(1);
            self.on_blocked.emit(&(frame, origin));
            tracing::debug!(target: "machbus.niu", pgn, ?origin, "loop guard blocked echoed frame");
            return None;
        }
        self.observe_address_claim(&frame, origin);
        let (policy, rate_limited) = self.resolve_policy(&frame, origin, now_ms);

        if rate_limited {
            self.blocked_count = self.blocked_count.saturating_add(1);
            self.rate_limited_count = self.rate_limited_count.saturating_add(1);
            self.on_blocked.emit(&(frame, origin));
            tracing::debug!(target: "machbus.niu", pgn, ?origin, "rate limited");
            return None;
        }

        match policy {
            ForwardPolicy::Allow => {
                self.forwarded_count = self.forwarded_count.saturating_add(1);
                self.on_forwarded.emit(&(frame, origin));
                if remember_on_forward {
                    self.remember_forwarded_frame(&frame, origin.other(), now_ms);
                }
                Some(frame)
            }
            ForwardPolicy::Block => {
                self.blocked_count = self.blocked_count.saturating_add(1);
                self.on_blocked.emit(&(frame, origin));
                tracing::debug!(target: "machbus.niu", pgn, ?origin, "blocked");
                None
            }
            ForwardPolicy::Monitor => {
                self.forwarded_count = self.forwarded_count.saturating_add(1);
                self.on_forwarded.emit(&(frame, origin));
                self.on_monitored.emit(&(frame, origin));
                tracing::debug!(target: "machbus.niu", pgn, ?origin, "monitored");
                if remember_on_forward {
                    self.remember_forwarded_frame(&frame, origin.other(), now_ms);
                }
                Some(frame)
            }
        }
    }

    /// Returns `(policy, rate_limited)` — match the C++
    /// `resolve_policy_ex` signature.
    fn resolve_policy(
        &mut self,
        frame: &Frame,
        origin: Side,
        now_ms: u32,
    ) -> (ForwardPolicy, bool) {
        let pgn = frame.pgn();
        let is_broadcast = frame.is_broadcast();
        let source_name = self.observed_name(origin, frame.source());
        let destination_name = (!is_broadcast)
            .then(|| self.observed_name(origin, frame.destination()))
            .flatten();

        for rule in &mut self.filters {
            // `rule.pgn == 0` means "any PGN" — used by NAME-based
            // filters that should still match.
            if rule.pgn != 0 && rule.pgn != pgn {
                continue;
            }
            // Direction match: !bidirectional ⇒ tractor-side only.
            if !rule.bidirectional && origin != Side::Tractor {
                continue;
            }
            if rule
                .source_name
                .is_some_and(|required| source_name != Some(required))
            {
                continue;
            }
            if rule
                .destination_name
                .is_some_and(|required| destination_name != Some(required))
            {
                continue;
            }
            // Rate limiting. The first call (last_forward_time_ms ==
            // None) always passes — see the field's docstring.
            if rule.max_frequency_ms > 0 {
                if let Some(last) = rule.last_forward_time_ms {
                    let elapsed = now_ms.saturating_sub(last);
                    if elapsed < rule.max_frequency_ms {
                        return (rule.policy, true);
                    }
                }
                rule.last_forward_time_ms = Some(now_ms);
            }
            return (rule.policy, false);
        }

        // No match — apply default mode.
        match self.config.filter_mode {
            NiuFilterMode::PassSpecific => (ForwardPolicy::Block, false),
            NiuFilterMode::BlockSpecific => {
                let allow = if is_broadcast {
                    self.config.forward_global_by_default
                } else {
                    self.config.forward_specific_by_default
                };
                (
                    if allow {
                        ForwardPolicy::Allow
                    } else {
                        ForwardPolicy::Block
                    },
                    false,
                )
            }
        }
    }

    fn observe_address_claim(&mut self, frame: &Frame, origin: Side) {
        if frame.pgn() != PGN_ADDRESS_CLAIMED || frame.source() == NULL_ADDRESS {
            return;
        }
        if let Some(name) = Name::from_bytes(frame.payload()) {
            self.observed_names.insert((origin, frame.source()), name);
        }
    }

    fn is_loop_echo(&mut self, frame: &Frame, origin: Side, now_ms: u32) -> bool {
        let window = self.config.loop_guard_window_ms;
        if window == 0 || self.config.loop_guard_max_recent_forwards == 0 {
            return false;
        }

        self.recent_forwards
            .retain(|entry| now_ms.wrapping_sub(entry.forwarded_at_ms) <= window);
        let signature = FrameSignature::from_frame(frame);
        self.recent_forwards
            .iter()
            .any(|entry| entry.target_side == origin && entry.signature == signature)
    }

    fn remember_forwarded_frame(&mut self, frame: &Frame, target_side: Side, now_ms: u32) {
        let window = self.config.loop_guard_window_ms;
        let capacity = self.config.loop_guard_max_recent_forwards;
        if window == 0 || capacity == 0 {
            return;
        }

        self.recent_forwards
            .retain(|entry| now_ms.wrapping_sub(entry.forwarded_at_ms) <= window);
        let signature = FrameSignature::from_frame(frame);
        if let Some(entry) = self
            .recent_forwards
            .iter_mut()
            .find(|entry| entry.target_side == target_side && entry.signature == signature)
        {
            entry.forwarded_at_ms = now_ms;
            return;
        }

        while self.recent_forwards.len() >= capacity {
            self.recent_forwards.remove(0);
        }
        self.recent_forwards.push(RecentForward {
            target_side,
            signature,
            forwarded_at_ms: now_ms,
        });
    }

    // ─── NIU control protocol ──────────────────────────────────────

    /// Process an incoming NIU Network Message (PGN `0xED00`).
    pub fn handle_niu_message(&mut self, msg: &Message) {
        if msg.pgn != PGN_NIU_NETWORK_MSG {
            return;
        }
        // A configuration command reconfigures *this* bridge, so it must be
        // addressed to it. Accepting a globally addressed one let any CF on
        // either segment rewrite the filter database of every NIU listening.
        if !msg.has_usable_source()
            || msg.destination == NULL_ADDRESS
            || msg.destination == BROADCAST_ADDRESS
        {
            return;
        }
        let Some(niu_msg) = NiuNetworkMsg::try_decode(&msg.data) else {
            return;
        };
        tracing::debug!(
            target: "machbus.niu",
            func = ?niu_msg.function,
            port = niu_msg.port_number,
            "NIU msg received",
        );

        match niu_msg.function {
            NiuFunction::AddFilterEntry => {
                // What an entry *means* depends on the database's filter mode
                // (§6.6.2.3.3 with §6.2.2/§6.2.3): in block-specific mode a
                // listed PGN is blocked, in pass-specific mode it is passed.
                // This always installed `Allow`, so adding an entry to a
                // block-specific database asked the bridge to forward exactly
                // the PGN the caller wanted stopped.
                // PGN 0 is the wildcard "any PGN" (see `FilterRule`). Installed
                // from a remote command in block-specific mode it blocks every
                // frame in both directions — one message turns the bridge into
                // a blackhole. A wildcard is an operator decision, not
                // something a peer gets to assert over the bus.
                if niu_msg.filter_pgn == 0 {
                    tracing::warn!(
                        target: "machbus.niu",
                        source = %format_args!("0x{:02X}", msg.source),
                        "refusing a wildcard NIU filter entry",
                    );
                    return;
                }
                let policy = match self.filter_mode() {
                    NiuFilterMode::BlockSpecific => ForwardPolicy::Block,
                    NiuFilterMode::PassSpecific => ForwardPolicy::Allow,
                };
                self.add_filter(FilterRule::new(niu_msg.filter_pgn, policy, true));
                self.on_niu_message.emit(&(niu_msg, msg.source));
            }
            NiuFunction::DeleteFilterEntry => {
                if let Some(idx) = self
                    .filters
                    .iter()
                    .position(|f| f.pgn == niu_msg.filter_pgn)
                {
                    self.filters.remove(idx);
                }
                self.on_niu_message.emit(&(niu_msg, msg.source));
            }
            NiuFunction::ClearFilterEntry => {
                // Persistent rules are configured policy, not runtime state, so
                // a remote clear must not take them with it. `filters.clear()`
                // wiped the whole database including them.
                self.clear_filters();
                self.on_niu_message.emit(&(niu_msg, msg.source));
            }
            NiuFunction::RequestGeneralParametrics => {
                let reply = NiuNetworkMsg {
                    function: NiuFunction::GeneralParametricsResponse,
                    port_number: niu_msg.port_number,
                    msgs_forwarded: self.forwarded_count,
                    msgs_blocked: self.blocked_count,
                    ..Default::default()
                };
                self.on_niu_message.emit(&(reply, msg.source));
            }
            _ => {
                self.on_niu_message.emit(&(niu_msg, msg.source));
            }
        }
    }
}

// ─── Address translation database ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressTranslation {
    pub name: Name,
    pub tractor_address: Address,
    pub implement_address: Address,
    pub active: bool,
}

impl AddressTranslation {
    #[must_use]
    pub const fn new(name: Name, tractor_address: Address, implement_address: Address) -> Self {
        Self {
            name,
            tractor_address,
            implement_address,
            active: true,
        }
    }

    /// Translate an address from `from_side` to the other side. Returns
    /// `None` if the address is not part of this entry.
    #[must_use]
    pub const fn translate(&self, addr: Address, from_side: Side) -> Option<Address> {
        match from_side {
            Side::Tractor if addr == self.tractor_address => Some(self.implement_address),
            Side::Implement if addr == self.implement_address => Some(self.tractor_address),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct AddressTranslationDb {
    entries: Vec<AddressTranslation>,
}

impl AddressTranslationDb {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or replace the translation for `name`.
    ///
    /// Both addresses must be claimable node addresses (`0x00..=0xFD`).
    /// A side-local address may only be owned by one active NAME at a
    /// time; allowing duplicate tractor-side or implement-side mappings
    /// would make routing non-deterministic.
    pub fn add(
        &mut self,
        name: Name,
        tractor_addr: Address,
        implement_addr: Address,
    ) -> Result<()> {
        validate_translation_address(tractor_addr)?;
        validate_translation_address(implement_addr)?;
        self.ensure_side_addresses_available(name, tractor_addr, implement_addr)?;

        if let Some(t) = self.entries.iter_mut().find(|t| t.name == name) {
            t.tractor_address = tractor_addr;
            t.implement_address = implement_addr;
            t.active = true;
            return Ok(());
        }
        self.entries
            .push(AddressTranslation::new(name, tractor_addr, implement_addr));
        Ok(())
    }

    pub fn remove(&mut self, name: Name) {
        if let Some(idx) = self.entries.iter().position(|t| t.name == name) {
            self.entries.remove(idx);
        }
    }

    /// Translate `addr` from `from_side` to the other side. Returns
    /// `None` if no active entry covers it.
    #[must_use]
    pub fn translate(&self, addr: Address, from_side: Side) -> Option<Address> {
        self.entries
            .iter()
            .filter(|t| t.active)
            .find_map(|t| t.translate(addr, from_side))
    }

    #[must_use]
    pub fn lookup_by_address(&self, addr: Address, side: Side) -> Option<AddressTranslation> {
        self.entries.iter().copied().find(|t| {
            t.active
                && match side {
                    Side::Tractor => t.tractor_address == addr,
                    Side::Implement => t.implement_address == addr,
                }
        })
    }

    #[must_use]
    pub fn lookup_by_name(&self, name: Name) -> Option<AddressTranslation> {
        self.entries
            .iter()
            .copied()
            .find(|t| t.active && t.name == name)
    }

    #[must_use]
    pub fn is_address_available(&self, addr: Address, side: Side) -> bool {
        validate_translation_address(addr).is_ok() && self.lookup_by_address(addr, side).is_none()
    }

    #[must_use]
    pub fn entries(&self) -> &[AddressTranslation] {
        &self.entries
    }

    /// Return active translations in deterministic order for
    /// diagnostics, policy snapshots, and tests.
    #[must_use]
    pub fn snapshot(&self) -> Vec<AddressTranslation> {
        let mut entries: Vec<_> = self.entries.iter().copied().filter(|t| t.active).collect();
        entries.sort_by_key(|t| (t.name.raw, t.tractor_address, t.implement_address));
        entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    fn ensure_side_addresses_available(
        &self,
        name: Name,
        tractor_addr: Address,
        implement_addr: Address,
    ) -> Result<()> {
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.active && entry.name != name)
        {
            if entry.tractor_address == tractor_addr {
                return Err(address_conflict("tractor", tractor_addr, entry.name, name));
            }
            if entry.implement_address == implement_addr {
                return Err(address_conflict(
                    "implement",
                    implement_addr,
                    entry.name,
                    name,
                ));
            }
        }
        Ok(())
    }
}

fn validate_translation_address(addr: Address) -> Result<()> {
    if addr <= MAX_ADDRESS {
        return Ok(());
    }
    debug_assert!(addr == NULL_ADDRESS || addr == BROADCAST_ADDRESS);
    Err(Error::invalid_address(addr))
}

fn address_conflict(side: &str, addr: Address, existing: Name, requested: Name) -> Error {
    Error::with_message(
        ErrorCode::AddressConflict,
        format!(
            "{side}-side address 0x{addr:02X} already belongs to NAME 0x{:016X}, requested by NAME 0x{:016X}",
            existing.raw, requested.raw
        ),
    )
}

// ─── Router ────────────────────────────────────────────────────────────

/// NIU with address translation. Wraps a [`Niu`] and, on forward,
/// rewrites the source/destination addresses using the
/// [`AddressTranslationDb`].
pub struct Router {
    niu: Niu,
    db: AddressTranslationDb,
    /// §7.3.1: "Address claim messages do not cross through a router", repeated
    /// in §7.3.3 — a router joins two *separate* address spaces, so a claim
    /// made on one side says nothing about the other and forwarding it invites
    /// a spurious contest.
    ///
    /// A bridge deployed over a single shared address space is the case where
    /// claims legitimately cross; that is opt-in via
    /// [`Router::forward_address_claims`].
    forward_address_claims: bool,
}

/// Deterministic router policy snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterPolicySnapshot {
    pub niu: NiuPolicySnapshot,
    /// Backward-compatible direct access to the NIU filter policy.
    ///
    /// This mirrors `niu.filters` so existing policy-dump consumers can keep
    /// reading the rule table while newer consumers can use the fuller
    /// [`NiuPolicySnapshot`].
    pub filters: Vec<FilterRuleSnapshot>,
    pub translations: Vec<AddressTranslation>,
}

impl Router {
    #[must_use]
    pub fn new(config: NiuConfig) -> Self {
        Self {
            niu: Niu::new(config),
            db: AddressTranslationDb::new(),
            forward_address_claims: false,
        }
    }

    /// Allow Address Claimed frames to cross, for a bridge sharing one address
    /// space with both segments. Off by default per §7.3.1.
    #[must_use]
    pub const fn forward_address_claims(mut self, forward: bool) -> Self {
        self.forward_address_claims = forward;
        self
    }

    pub fn add_translation(
        &mut self,
        name: Name,
        tractor_addr: Address,
        implement_addr: Address,
    ) -> Result<()> {
        self.db.add(name, tractor_addr, implement_addr)?;
        tracing::info!(
            target: "machbus.niu.router",
            tractor = %format_args!("0x{tractor_addr:02X}"),
            implement = %format_args!("0x{implement_addr:02X}"),
            "translation added",
        );
        Ok(())
    }

    pub fn remove_translation(&mut self, name: Name) {
        self.db.remove(name);
    }

    #[must_use]
    pub fn policy_snapshot(&self) -> RouterPolicySnapshot {
        let niu = self.niu.policy_snapshot();
        RouterPolicySnapshot {
            filters: niu.filters.clone(),
            niu,
            translations: self.db.snapshot(),
        }
    }

    #[inline]
    #[must_use]
    pub fn translation_db(&self) -> &AddressTranslationDb {
        &self.db
    }

    /// Borrow the underlying [`Niu`] for filter management /
    /// statistics / events.
    #[inline]
    pub fn niu(&self) -> &Niu {
        &self.niu
    }

    #[inline]
    pub fn niu_mut(&mut self) -> &mut Niu {
        &mut self.niu
    }

    /// Process a frame and return the (possibly address-translated)
    /// frame to forward, or `None` if blocked.
    ///
    /// For destination-specific frames, blocks if the destination has
    /// no translation. For broadcast frames, only the source is
    /// translated.
    pub fn process_frame(&mut self, frame: Frame, origin: Side, now_ms: u32) -> Option<Frame> {
        // Run the base filter first.
        let frame = self.niu.process_frame_inner(frame, origin, now_ms, false)?;

        let source = frame.source();
        let destination = frame.destination();
        let is_broadcast = frame.is_broadcast();

        if frame.pgn() == PGN_ADDRESS_CLAIMED && !self.forward_address_claims {
            self.block_translated_frame(frame, origin, "address claims do not cross a router");
            return None;
        }
        let translated_source = if frame.pgn() == PGN_ADDRESS_CLAIMED {
            let Some(translated) = self.translate_address_claim_source(&frame, origin) else {
                self.block_translated_frame(frame, origin, "invalid address-claim translation");
                return None;
            };
            translated
        } else {
            self.db.translate(source, origin)
        };
        let translated_dest = if is_broadcast {
            None
        } else {
            self.db.translate(destination, origin)
        };

        // Destination-specific frame whose destination has no
        // translation: block (matches C++).
        if !is_broadcast && translated_dest.is_none() {
            self.niu.blocked_count = self.niu.blocked_count.saturating_add(1);
            self.niu.on_blocked.emit(&(frame, origin));
            tracing::debug!(
                target: "machbus.niu.router",
                dest = %format_args!("0x{destination:02X}"),
                "no translation for destination — blocking",
            );
            return None;
        }

        // Source and destination translate independently. Returning the frame
        // untouched when only the source lacked a mapping forwarded it with the
        // *untranslated* destination, so a destination-specific command — a
        // stop, a setpoint, a diagnostic write — was delivered to whichever CF
        // happened to hold that address on the far segment while the intended
        // recipient heard nothing.
        let new_source = translated_source.unwrap_or(source);
        let new_dest = if is_broadcast {
            destination
        } else {
            translated_dest.unwrap_or(destination)
        };
        if new_source == source && new_dest == destination {
            self.niu
                .remember_forwarded_frame(&frame, origin.other(), now_ms);
            return Some(frame);
        }
        let new_id = Identifier::encode(frame.priority(), frame.pgn(), new_source, new_dest);
        let mut translated = frame;
        translated.id = new_id;
        self.niu
            .remember_forwarded_frame(&translated, origin.other(), now_ms);
        Some(translated)
    }

    fn translate_address_claim_source(
        &self,
        frame: &Frame,
        origin: Side,
    ) -> Option<Option<Address>> {
        let source = frame.source();
        if source == NULL_ADDRESS {
            // Cannot Claim Address frames intentionally use SA 0xFE. There is
            // no side-local address to translate, but the failure should still
            // be visible across the bridge.
            return Some(None);
        }

        let claimed_name = Name::from_bytes(frame.payload())?;
        if let Some(entry) = self.db.lookup_by_address(source, origin) {
            if entry.name != claimed_name {
                tracing::warn!(
                    target: "machbus.niu.router",
                    source = %format_args!("0x{source:02X}"),
                    expected = %format_args!("0x{:016X}", entry.name.raw),
                    claimed = %format_args!("0x{:016X}", claimed_name.raw),
                    "blocking address claim whose NAME does not match the translation table",
                );
                return None;
            }
            return Some(entry.translate(source, origin));
        }

        if self.db.lookup_by_name(claimed_name).is_some() {
            tracing::warn!(
                target: "machbus.niu.router",
                source = %format_args!("0x{source:02X}"),
                claimed = %format_args!("0x{:016X}", claimed_name.raw),
                "blocking address claim from an unexpected side-local address",
            );
            return None;
        }

        Some(None)
    }

    fn block_translated_frame(&mut self, frame: Frame, origin: Side, reason: &'static str) {
        self.niu.blocked_count = self.niu.blocked_count.saturating_add(1);
        self.niu.on_blocked.emit(&(frame, origin));
        tracing::debug!(
            target: "machbus.niu.router",
            pgn = frame.pgn(),
            src = %format_args!("0x{:02X}", frame.source()),
            dst = %format_args!("0x{:02X}", frame.destination()),
            reason,
            "blocking routed frame",
        );
    }
}

// ─── Learning bridge (compact convenience) ─────────────────────────────

/// Tracks which side a given address last appeared on. Useful for
/// avoiding unnecessary forwards when both sides share an address
/// space.
#[derive(Debug, Default)]
pub struct AddressTable {
    table: BTreeMap<Address, Side>,
}

impl AddressTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn learn(&mut self, addr: Address, side: Side) {
        self.table.insert(addr, side);
    }

    #[must_use]
    pub fn lookup(&self, addr: Address) -> Option<Side> {
        self.table.get(&addr).copied()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.table.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    pub fn clear(&mut self) {
        self.table.clear();
    }
}

// ─── NIU product profiles (ISO 11783-4) ────────────────────────────────
//
// GAP.md (ISO 11783-4) asks to "split NIU into a stated product profile:
// simple router, managed gateway, bridge, test-only simulator" and to mark
// which behaviours each claims. This is that profile model as typed data.

/// A stated NIU product profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NiuProfile {
    /// Forwards and filters frames between two segments; no translation.
    SimpleRouter,
    /// Router plus source-address translation between segments.
    Bridge,
    /// Bridge plus runtime reconfiguration (filter mode / rules over the
    /// network-control message).
    ManagedGateway,
    /// In-memory profile for tests/simulation.
    TestSimulator,
}

/// machbus support level for a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NiuProfileStatus {
    /// Fully provided by the crate.
    Implemented,
    /// Provided except for a stated missing behaviour.
    PartialHelper,
}

/// The behaviour set + support level of one NIU profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NiuProfileSupport {
    pub profile: NiuProfile,
    pub forwarding: bool,
    pub filtering: bool,
    pub address_translation: bool,
    pub runtime_reconfiguration: bool,
    pub persistence: bool,
    pub status: NiuProfileStatus,
}

use NiuProfile as P;
use NiuProfileStatus::{Implemented, PartialHelper};

/// The NIU product-profile matrix.
pub const NIU_PROFILES: [NiuProfileSupport; 4] = [
    NiuProfileSupport {
        profile: P::SimpleRouter,
        forwarding: true,
        filtering: true,
        address_translation: false,
        runtime_reconfiguration: false,
        persistence: false,
        status: Implemented,
    },
    NiuProfileSupport {
        profile: P::Bridge,
        forwarding: true,
        filtering: true,
        address_translation: true,
        runtime_reconfiguration: false,
        persistence: false,
        status: Implemented,
    },
    NiuProfileSupport {
        profile: P::ManagedGateway,
        forwarding: true,
        filtering: true,
        address_translation: true,
        runtime_reconfiguration: true,
        // Config persistence via NiuConfig::save / load_from.
        persistence: true,
        // Honest downgrade: the data plane + config persistence exist, but the
        // managed-gateway control plane (addressed-CF responses + Acknowledge,
        // parametrics/statistics, topology messages, connection/virtual-CF) and
        // gateway parameter repackaging are not implemented.
        status: PartialHelper,
    },
    NiuProfileSupport {
        profile: P::TestSimulator,
        forwarding: true,
        filtering: true,
        address_translation: true,
        runtime_reconfiguration: true,
        persistence: false,
        status: Implemented,
    },
];

/// The support record for a profile.
#[must_use]
pub fn niu_profile(profile: NiuProfile) -> NiuProfileSupport {
    NIU_PROFILES
        .into_iter()
        .find(|p| p.profile == profile)
        .expect("every NIU profile has a record")
}


// ─── Network topology messages (ISO 11783-4 §6.7.2) ────────────────────

/// One entry of a source-address/NAME list (§6.7.2.4): a 1-byte source address
/// followed by the 8-byte ISO 11783-5 NAME that claimed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceAddressName {
    pub address: u8,
    pub name: u64,
}

/// `N.NTX_Request` (§6.7.2.3) and `N.NTX_Response` (§6.7.2.4).
///
/// These let a CF discover the control functions on the far side of a router,
/// whose address claims it never saw because they are in a different address
/// space. Without them the CFs behind a router are undiscoverable, which is
/// what this stack's state was: the data existed internally and no message
/// could carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NiuTopologyMsg {
    pub function: NiuFunction,
    /// Requested port number, 0..=14. The global port is not allowed for the
    /// request (§6.7.2.3).
    pub port_number: u8,
    /// Populated on a response; empty on a request.
    pub entries: Vec<SourceAddressName>,
}

impl NiuTopologyMsg {
    /// Build a `N.NTX_Request` for `port_number`.
    #[must_use]
    pub fn request(port_number: u8) -> Self {
        Self {
            function: NiuFunction::RequestSourceAddressNameList,
            port_number,
            entries: Vec::new(),
        }
    }

    /// Build a `N.NTX_Response` carrying `entries`.
    #[must_use]
    pub fn response(port_number: u8, entries: Vec<SourceAddressName>) -> Self {
        Self {
            function: NiuFunction::SourceAddressNameListResponse,
            port_number,
            entries,
        }
    }

    /// Encode per §6.7.2.3 / §6.7.2.4.
    ///
    /// # Errors
    /// A port number outside 0..=14, the global port on a request, or more
    /// entries than a single message can describe.
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.port_number > 14 {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "NIU topology port number must be 0..=14",
            ));
        }
        if self.entries.len() > usize::from(u8::MAX) {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "NIU topology entry count exceeds one byte",
            ));
        }

        let mut data = Vec::with_capacity(3 + self.entries.len() * 9);
        data.push(self.function.as_u8());
        // Byte 2: port pair — requested port in bits 1-4, bits 5-8 set to F.
        data.push(0xF0 | (self.port_number & 0x0F));

        match self.function {
            NiuFunction::RequestSourceAddressNameList => {
                // Bytes 3-8 reserved, transmitted as FF.
                data.extend_from_slice(&[0xFF; 6]);
            }
            NiuFunction::SourceAddressNameListResponse => {
                data.push(self.entries.len() as u8);
                for entry in &self.entries {
                    data.push(entry.address);
                    data.extend_from_slice(&entry.name.to_le_bytes());
                }
            }
            _ => {
                return Err(Error::with_message(
                    ErrorCode::InvalidData,
                    "not a NIU topology function",
                ));
            }
        }
        Ok(data)
    }

    /// Decode a topology request or response.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        let function = NiuFunction::try_from_u8(*data.first()?)?;
        let port_byte = *data.get(1)?;
        // Bits 5-8 of the port pair are set to F on both messages.
        if port_byte & 0xF0 != 0xF0 {
            return None;
        }
        let port_number = port_byte & 0x0F;
        if port_number > 14 {
            return None;
        }

        match function {
            NiuFunction::RequestSourceAddressNameList => {
                if data.len() != 8 || data[2..].iter().any(|&b| b != 0xFF) {
                    return None;
                }
                Some(Self::request(port_number))
            }
            NiuFunction::SourceAddressNameListResponse => {
                let count = usize::from(*data.get(2)?);
                if data.len() != 3 + count * 9 {
                    return None;
                }
                let mut entries = Vec::with_capacity(count);
                for i in 0..count {
                    let base = 3 + i * 9;
                    let mut name = [0u8; 8];
                    name.copy_from_slice(&data[base + 1..base + 9]);
                    entries.push(SourceAddressName {
                        address: data[base],
                        name: u64::from_le_bytes(name),
                    });
                }
                Some(Self::response(port_number, entries))
            }
            _ => None,
        }
    }
}

// ─── Connection management (ISO 11783-4 §6.9.5) ────────────────────────

/// Why an open/close connection request failed (§6.9.5.3 byte 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ConnectionFailureReason {
    CannotFindCfWithName = 0,
    ConnectionsToNameExceeded = 1,
    ConnectionsInNiuExceeded = 2,
    Busy = 3,
    RequestTypeNotSupported = 4,
    #[default]
    NotAvailable = 255,
}

impl ConnectionFailureReason {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::CannotFindCfWithName),
            1 => Some(Self::ConnectionsToNameExceeded),
            2 => Some(Self::ConnectionsInNiuExceeded),
            3 => Some(Self::Busy),
            4 => Some(Self::RequestTypeNotSupported),
            255 => Some(Self::NotAvailable),
            // 5..=254 reserved.
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// `N.CO_Request` / `N.CC_Request` (§6.9.5.1, §6.9.5.3) — 10 bytes.
///
/// A CF asks the NIU to open or close a virtual connection to a named control
/// function on another network segment. Section 6.9 was entirely absent, so a
/// CF could not reach a peer behind a router at all. The NAME is obtained from
/// the topology response of §6.7.2.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NiuConnectionRequest {
    pub function: NiuFunction,
    /// Port of the NIU on the target CF's segment (low nibble).
    pub to_port: u8,
    /// Port of the NIU on the requester's segment (high nibble); normally 0.
    pub from_port: u8,
    /// NAME of the CF to connect to on the "to" port.
    pub name: u64,
}

impl NiuConnectionRequest {
    /// # Errors
    /// A port number outside 0..=15, or a function that is not an open/close
    /// request.
    pub fn encode(&self) -> Result<[u8; 10]> {
        if !matches!(
            self.function,
            NiuFunction::OpenConnection | NiuFunction::CloseConnection
        ) {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "not a NIU connection request function",
            ));
        }
        if self.to_port > 15 || self.from_port > 15 {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "NIU port numbers are 4-bit",
            ));
        }
        let mut data = [0xFFu8; 10];
        data[0] = self.function.as_u8();
        data[1] = (self.from_port << 4) | (self.to_port & 0x0F);
        data[2..10].copy_from_slice(&self.name.to_le_bytes());
        Ok(data)
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 10 {
            return None;
        }
        let function = NiuFunction::try_from_u8(data[0])?;
        if !matches!(
            function,
            NiuFunction::OpenConnection | NiuFunction::CloseConnection
        ) {
            return None;
        }
        let mut name = [0u8; 8];
        name.copy_from_slice(&data[2..10]);
        Some(Self {
            function,
            to_port: data[1] & 0x0F,
            from_port: data[1] >> 4,
            name: u64::from_le_bytes(name),
        })
    }
}

/// `N.CO_Response` / `N.CC_Response` (§6.9.5.2, §6.9.5.4) — 8 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NiuConnectionResponse {
    pub function: NiuFunction,
    /// Port of the NIU on the requester's segment (low nibble).
    pub to_port: u8,
    /// Port of the NIU on the target CF's segment (high nibble).
    pub from_port: u8,
    pub success: bool,
    /// Meaningful only when `success` is false.
    pub reason: ConnectionFailureReason,
}

impl NiuConnectionResponse {
    /// # Errors
    /// A function that is not an open/close response, or an out-of-range port.
    pub fn encode(&self) -> Result<[u8; 8]> {
        if !matches!(
            self.function,
            NiuFunction::OpenConnectionResponse | NiuFunction::CloseConnectionResponse
        ) {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "not a NIU connection response function",
            ));
        }
        if self.to_port > 15 || self.from_port > 15 {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "NIU port numbers are 4-bit",
            ));
        }
        let mut data = [0xFFu8; 8];
        data[0] = self.function.as_u8();
        data[1] = (self.from_port << 4) | (self.to_port & 0x0F);
        // Byte 3 bits 1-2 carry success; bits 3-8 are reserved and set to 1.
        data[2] = 0xFC | u8::from(self.success);
        data[3] = self.reason.as_u8();
        Ok(data)
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        let function = NiuFunction::try_from_u8(data[0])?;
        if !matches!(
            function,
            NiuFunction::OpenConnectionResponse | NiuFunction::CloseConnectionResponse
        ) {
            return None;
        }
        Some(Self {
            function,
            to_port: data[1] & 0x0F,
            from_port: data[1] >> 4,
            success: data[2] & 0x03 == 0x01,
            reason: ConnectionFailureReason::from_u8(data[3])?,
        })
    }
}
