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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum NiuFilterMode {
    /// Block all by default; only listed PGNs pass.
    BlockAll = 0,
    /// Pass all by default; only listed PGNs are blocked.
    #[default]
    PassAll = 1,
}

impl NiuFilterMode {
    #[inline]
    #[must_use]
    pub const fn try_from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::BlockAll),
            1 => Some(Self::PassAll),
            _ => None,
        }
    }
}

/// NIU Network Message function codes (ISO 11783-4 §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum NiuFunction {
    #[default]
    RequestFilterDb = 1,
    AddFilterEntry = 2,
    DeleteFilterEntry = 3,
    DeleteAllEntries = 4,
    RequestFilterMode = 5,
    SetFilterMode = 6,
    RequestPortConfig = 9,
    PortConfigResponse = 10,
    FilterDbResponse = 11,
    RequestPortStats = 12,
    PortStatsResponse = 13,
    OpenConnection = 14,
    CloseConnection = 15,
}

impl NiuFunction {
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match Self::try_from_u8(value) {
            Some(function) => function,
            None => Self::RequestFilterDb,
        }
    }

    #[must_use]
    pub const fn try_from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::RequestFilterDb),
            2 => Some(Self::AddFilterEntry),
            3 => Some(Self::DeleteFilterEntry),
            4 => Some(Self::DeleteAllEntries),
            5 => Some(Self::RequestFilterMode),
            6 => Some(Self::SetFilterMode),
            9 => Some(Self::RequestPortConfig),
            10 => Some(Self::PortConfigResponse),
            11 => Some(Self::FilterDbResponse),
            12 => Some(Self::RequestPortStats),
            13 => Some(Self::PortStatsResponse),
            14 => Some(Self::OpenConnection),
            15 => Some(Self::CloseConnection),
            _ => None,
        }
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
            filter_mode: NiuFilterMode::PassAll,
            msgs_forwarded: 0,
            msgs_blocked: 0,
        }
    }
}

impl NiuNetworkMsg {
    /// Encode to the standard 8-byte wire format (padded with `0xFF`).
    pub fn encode(&self) -> Result<[u8; 8]> {
        let mut data = [0xFFu8; 8];
        data[0] = self.function as u8;
        data[1] = self.port_number;
        match self.function {
            NiuFunction::AddFilterEntry
            | NiuFunction::DeleteFilterEntry
            | NiuFunction::FilterDbResponse => {
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
            NiuFunction::SetFilterMode | NiuFunction::RequestFilterMode => {
                data[2] = self.filter_mode as u8;
            }
            NiuFunction::PortStatsResponse => {
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
            NiuFunction::AddFilterEntry
            | NiuFunction::DeleteFilterEntry
            | NiuFunction::FilterDbResponse => {
                if (data[4] & 0xFC) != 0 || data[5..].iter().any(|&b| b != 0xFF) {
                    return None;
                }
                msg.filter_pgn =
                    (data[2] as Pgn) | ((data[3] as Pgn) << 8) | (((data[4] & 0x03) as Pgn) << 16);
            }
            NiuFunction::SetFilterMode | NiuFunction::RequestFilterMode => {
                if data[3..].iter().any(|&b| b != 0xFF) {
                    return None;
                }
                msg.filter_mode = NiuFilterMode::try_from_u8(data[2])?;
            }
            NiuFunction::PortStatsResponse => {
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
            filter_mode: NiuFilterMode::PassAll,
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
        let pass = matches!(mode, NiuFilterMode::PassAll);
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
            NiuFilterMode::BlockAll => (ForwardPolicy::Block, false),
            NiuFilterMode::PassAll => {
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
        if !msg.has_usable_source() || msg.destination == NULL_ADDRESS {
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
                self.add_filter(FilterRule::new(
                    niu_msg.filter_pgn,
                    ForwardPolicy::Allow,
                    true,
                ));
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
            NiuFunction::DeleteAllEntries => {
                self.filters.clear();
                self.on_niu_message.emit(&(niu_msg, msg.source));
            }
            NiuFunction::SetFilterMode => {
                self.set_filter_mode(niu_msg.filter_mode);
                self.on_niu_message.emit(&(niu_msg, msg.source));
            }
            NiuFunction::RequestPortStats => {
                let reply = NiuNetworkMsg {
                    function: NiuFunction::PortStatsResponse,
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
        }
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

        // No translation for source ⇒ forward as-is (matches C++).
        let Some(new_source) = translated_source else {
            self.niu
                .remember_forwarded_frame(&frame, origin.other(), now_ms);
            return Some(frame);
        };
        let new_dest = if is_broadcast {
            destination
        } else {
            translated_dest?
        };
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

