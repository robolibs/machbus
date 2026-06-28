//! Data model for the live TUI: frames, decode results, statistics, tabs.

use std::collections::HashMap;
use std::time::Instant;

use crate::tui::decode::FrameKind;

/// One captured frame, fully decoded once at ingest time.
#[derive(Clone)]
pub struct FrameEntry {
    pub seq: u64,
    /// Milliseconds since capture start.
    pub rel_ms: u64,
    pub iface: String,
    pub raw_id: u32,
    pub extended: bool,
    pub rtr: bool,
    pub err: bool,
    pub dlc: u8,
    pub data: [u8; 8],
    pub decoded: DecodedFrame,
}

/// The protocol-layer decode of a single frame.
#[derive(Clone)]
pub struct DecodedFrame {
    pub kind: FrameKind,
    /// `None` for non-extended / raw frames.
    pub pgn: Option<u32>,
    pub priority: Option<u8>,
    pub source: Option<u8>,
    pub destination: Option<u8>,
    /// Human-readable PGN/message name from the machbus tables.
    pub name: Option<&'static str>,
    /// Field-level decode (e.g. `("COG", "123.4°")`) for the detail pane.
    pub fields: Vec<(String, String)>,
}

impl DecodedFrame {
    /// An "I have no idea what this is" decode for raw frames.
    pub fn raw() -> Self {
        Self {
            kind: FrameKind::Raw,
            pgn: None,
            priority: None,
            source: None,
            destination: None,
            name: None,
            fields: Vec::new(),
        }
    }
}

/// Aggregated statistics over all captured frames.
pub struct Stats {
    pub total: u64,
    pub ext: u64,
    pub std: u64,
    pub rtr: u64,
    pub err: u64,
    pub bytes: u64,
    pub per_iface: HashMap<String, u64>,
    pub per_pgn: HashMap<u32, PgnStat>,
    pub start: Instant,
    /// Frames captured in the last rolling window, for the rate readout.
    pub rate: RateWindow,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            total: 0,
            ext: 0,
            std: 0,
            rtr: 0,
            err: 0,
            bytes: 0,
            per_iface: HashMap::new(),
            per_pgn: HashMap::new(),
            start: Instant::now(),
            rate: RateWindow::new(),
        }
    }

    pub fn clear(&mut self) {
        *self = Stats {
            start: Instant::now(),
            ..Stats::new()
        };
    }

    pub fn observe(&mut self, e: &FrameEntry) {
        self.total += 1;
        self.bytes += u64::from(e.dlc);
        if e.extended {
            self.ext += 1;
        } else {
            self.std += 1;
        }
        if e.rtr {
            self.rtr += 1;
        }
        if e.err {
            self.err += 1;
        }
        *self.per_iface.entry(e.iface.clone()).or_default() += 1;
        if let Some(pgn) = e.decoded.pgn {
            let row = self.per_pgn.entry(pgn).or_insert_with(|| PgnStat {
                pgn,
                name: e.decoded.name,
                kind: e.decoded.kind,
                count: 0,
                last_src: 0,
                last_ms: 0,
                last_data: [0; 8],
                last_dlc: 0,
            });
            row.count += 1;
            row.last_src = e.decoded.source.unwrap_or(0);
            row.last_ms = e.rel_ms;
            row.last_dlc = e.dlc;
            row.last_data = e.data;
            row.name = e.decoded.name.or(row.name);
        }
        self.rate.tick();
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    pub fn fps(&self) -> f64 {
        self.rate.fps()
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

/// One PGN aggregate row.
pub struct PgnStat {
    pub pgn: u32,
    pub name: Option<&'static str>,
    pub kind: FrameKind,
    pub count: u64,
    pub last_src: u8,
    pub last_ms: u64,
    pub last_data: [u8; 8],
    pub last_dlc: u8,
}

/// A rolling window frame-rate meter (frames per second).
pub struct RateWindow {
    bucket_count: u64,
    last_bucket: u64,
    samples: std::collections::VecDeque<f64>,
}

impl RateWindow {
    pub fn new() -> Self {
        Self {
            bucket_count: 0,
            last_bucket: 0,
            samples: std::collections::VecDeque::with_capacity(5),
        }
    }

    pub fn tick(&mut self) {
        let now_ms = since_epoch_ms();
        self.bucket_count += 1;
        if now_ms - self.last_bucket >= 250 {
            let elapsed_s = (now_ms - self.last_bucket) as f64 / 1000.0;
            if elapsed_s > 0.0 {
                self.samples.push_back(self.bucket_count as f64 / elapsed_s);
                if self.samples.len() > 4 {
                    self.samples.pop_front();
                }
            }
            self.last_bucket = now_ms;
            self.bucket_count = 0;
        }
    }

    pub fn fps(&self) -> f64 {
        if self.samples.is_empty() {
            0.0
        } else {
            self.samples.iter().sum::<f64>() / self.samples.len() as f64
        }
    }
}

impl Default for RateWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Which view is active.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Live,
    Sniffer,
    Pgn,
    Nmea,
    Nodes,
    Stats,
    Filter,
    Help,
}

impl Tab {
    pub const ALL: [Tab; 8] = [
        Tab::Live,
        Tab::Sniffer,
        Tab::Pgn,
        Tab::Nmea,
        Tab::Nodes,
        Tab::Stats,
        Tab::Filter,
        Tab::Help,
    ];

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Tab::Live => "Live",
            Tab::Sniffer => "Sniffer",
            Tab::Pgn => "PGN",
            Tab::Nmea => "NMEA 2000",
            Tab::Nodes => "Nodes",
            Tab::Stats => "Stats",
            Tab::Filter => "Filter",
            Tab::Help => "Help",
        }
    }

    /// Numeric hotkey, derived from the tab's position (1-based).
    #[must_use]
    pub fn hotkey(self) -> &'static str {
        const KEYS: [&str; 8] = ["1", "2", "3", "4", "5", "6", "7", "8"];
        KEYS[self as usize]
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "live" => Some(Self::Live),
            "sniffer" | "sniff" => Some(Self::Sniffer),
            "pgn" => Some(Self::Pgn),
            "nmea" | "nmea2000" | "n2k" => Some(Self::Nmea),
            "nodes" | "node" | "j1939" | "addr" | "address" | "claim" => Some(Self::Nodes),
            "stats" => Some(Self::Stats),
            "filter" | "filters" => Some(Self::Filter),
            "help" | "?" => Some(Self::Help),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        let idx = self as usize;
        Tab::ALL[(idx + 1) % Tab::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = self as usize;
        Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()]
    }
}

// ── sniffer state ───────────────────────────────────────────────────────

/// How long a changed byte stays highlighted (the "hold" period).
pub const SNIFF_HOLD_MS: u64 = 1200;

/// One row in the sniffer grid: the current payload for a CAN ID, with a
/// per-byte timestamp of the last change so the view can flash diffs.
pub struct SniffRow {
    pub can_id: u32,
    pub extended: bool,
    pub data: [u8; 8],
    pub dlc: u8,
    /// `Some(when)` if byte `i` changed recently, used for highlight.
    pub changed_at: [Option<Instant>; 8],
    pub last_seen: Instant,
    pub count: u64,
}

/// The full sniffer table, keyed by raw CAN ID.
pub struct SniffTable {
    pub rows: HashMap<u32, SniffRow>,
}

impl SniffTable {
    pub fn new() -> Self {
        Self {
            rows: HashMap::new(),
        }
    }

    /// Record one frame; mark any byte that differs from the previous value.
    pub fn observe(&mut self, can_id: u32, extended: bool, data: &[u8], dlc: u8, now: Instant) {
        let row = self.rows.entry(can_id).or_insert_with(|| SniffRow {
            can_id,
            extended,
            data: [0; 8],
            dlc: 0,
            changed_at: [None; 8],
            last_seen: now,
            count: 0,
        });
        let n = (dlc as usize).min(8);
        for (i, &b) in data[..n].iter().enumerate() {
            if row.data[i] != b {
                row.data[i] = b;
                row.changed_at[i] = Some(now);
            }
        }
        // Bytes beyond the new DLC are no longer present.
        for i in n..8 {
            row.changed_at[i] = None;
        }
        row.dlc = dlc;
        row.extended = extended;
        row.last_seen = now;
        row.count += 1;
    }
}

impl Default for SniffTable {
    fn default() -> Self {
        Self::new()
    }
}

// ── J1939 node (address-claim) state ────────────────────────────────────

use machbus::net::Name;

/// PGN 60928 (0xEE00) — J1939 Address Claimed.
pub const PGN_ADDRESS_CLAIMED: u32 = 0xEE00;

/// One claimed address on the bus.
pub struct NodeEntry {
    pub address: u8,
    pub name: Name,
    pub last_seen: Instant,
    pub count: u64,
}

/// Live J1939 address-claim table, keyed by source address.
pub struct NodeTable {
    pub rows: HashMap<u8, NodeEntry>,
}

impl NodeTable {
    pub fn new() -> Self {
        Self {
            rows: HashMap::new(),
        }
    }

    /// Record an Address Claimed frame (PGN 60928): `src` is the claimed
    /// address, the payload is the 8-byte NAME.
    pub fn observe(&mut self, pgn: u32, src: u8, data: &[u8], now: Instant) {
        if pgn != PGN_ADDRESS_CLAIMED {
            return;
        }
        let Some(name) = Name::from_bytes(data) else {
            return;
        };
        let entry = self.rows.entry(src).or_insert(NodeEntry {
            address: src,
            name,
            last_seen: now,
            count: 0,
        });
        entry.name = name;
        entry.last_seen = now;
        entry.count += 1;
    }
}

impl Default for NodeTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Milliseconds since the Unix epoch (monotonic enough for rate bucketing).
pub fn since_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
