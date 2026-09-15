//! Raw CAN frame layout, candump-style parsing/formatting, and the
//! ISOBUS/J1939 decode bridge into the `machbus` protocol model.
//!
//! The [`RawFrame`] layout is byte-compatible with the Linux
//! `struct can_frame` from `<linux/can.h>` so it can be `read`/`write`n
//! straight onto a `PF_CAN` / `SOCK_RAW` / `CAN_RAW` socket.

use std::fmt::Write as _;

// ── flag constants (identical to <linux/can.h>) ─────────────────────────
/// Extended frame format (29-bit ID) flag in `can_id`.
pub const CAN_EFF_FLAG: u32 = 0x8000_0000;
/// Remote-transmission-request flag in `can_id`.
pub const CAN_RTR_FLAG: u32 = 0x4000_0000;
/// Error-frame flag in `can_id`.
pub const CAN_ERR_FLAG: u32 = 0x2000_0000;
/// Mask for an 11-bit standard ID.
pub const CAN_SFF_MASK: u32 = 0x0000_07FF;
/// Mask for a 29-bit extended ID (without the flag bits).
pub const CAN_EFF_MASK: u32 = 0x1FFF_FFFF;

/// Maximum data length of a classical CAN frame.
pub const CAN_MAX_DLEN: usize = 8;

/// 16-byte Linux-compatible classical CAN frame.
///
/// Field order and total size match the kernel `struct can_frame`, so a
/// pointer to one can be handed to `write(2)` / `recvfrom(2)` directly.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawFrame {
    pub can_id: u32,
    pub can_dlc: u8,
    pub __pad: u8,
    pub __res0: u8,
    pub len8_dlc: u8,
    pub data: [u8; CAN_MAX_DLEN],
}

const _: () = assert!(std::mem::size_of::<RawFrame>() == 16);

impl Default for RawFrame {
    fn default() -> Self {
        Self {
            can_id: 0,
            can_dlc: 0,
            __pad: 0,
            __res0: 0,
            len8_dlc: 0,
            data: [0; CAN_MAX_DLEN],
        }
    }
}

impl RawFrame {
    /// Build a standard (11-bit ID) data frame.
    #[must_use]
    pub fn make_std(id: u32, data: &[u8]) -> Self {
        let mut cf = Self {
            can_id: id & CAN_SFF_MASK,
            can_dlc: data.len().min(CAN_MAX_DLEN) as u8,
            ..Default::default()
        };
        let n = cf.can_dlc as usize;
        cf.data[..n].copy_from_slice(&data[..n]);
        cf
    }

    /// Build an extended (29-bit ID) data frame.
    #[must_use]
    pub fn make_ext(id: u32, data: &[u8]) -> Self {
        let mut cf = Self {
            can_id: (id & CAN_EFF_MASK) | CAN_EFF_FLAG,
            can_dlc: data.len().min(CAN_MAX_DLEN) as u8,
            ..Default::default()
        };
        let n = cf.can_dlc as usize;
        cf.data[..n].copy_from_slice(&data[..n]);
        cf
    }

    /// Build a remote-transmission-request frame.
    #[must_use]
    pub fn make_rtr(id: u32, extended: bool) -> Self {
        let can_id = if extended {
            (id & CAN_EFF_MASK) | CAN_EFF_FLAG | CAN_RTR_FLAG
        } else {
            (id & CAN_SFF_MASK) | CAN_RTR_FLAG
        };
        Self {
            can_id,
            can_dlc: 0,
            ..Default::default()
        }
    }

    #[must_use]
    pub const fn is_extended(&self) -> bool {
        self.can_id & CAN_EFF_FLAG != 0
    }

    #[must_use]
    pub const fn is_rtr(&self) -> bool {
        self.can_id & CAN_RTR_FLAG != 0
    }

    #[must_use]
    pub const fn is_err(&self) -> bool {
        self.can_id & CAN_ERR_FLAG != 0
    }

    /// Effective CAN ID with all flag bits masked off.
    #[must_use]
    pub const fn id(&self) -> u32 {
        if self.is_extended() {
            self.can_id & CAN_EFF_MASK
        } else {
            self.can_id & CAN_SFF_MASK
        }
    }

    /// Valid data bytes (`0..=8`).
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        let n = (self.can_dlc as usize).min(CAN_MAX_DLEN);
        &self.data[..n]
    }
}

// ── candump parsing ─────────────────────────────────────────────────────
/// A frame lifted from a `candump` line, with the standard/extended flag
/// preserved (the raw `can_id` alone is ambiguous).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFrame {
    pub timestamp_us: Option<u64>,
    pub interface: Option<String>,
    pub raw_id: u32,
    pub extended: bool,
    pub rtr: bool,
    pub data: Vec<u8>,
}

impl ParsedFrame {
    /// Convert into a kernel-layout [`RawFrame`] for transmission.
    #[must_use]
    pub fn to_raw(&self) -> RawFrame {
        if self.rtr {
            RawFrame::make_rtr(self.raw_id, self.extended)
        } else if self.extended {
            RawFrame::make_ext(self.raw_id, &self.data)
        } else {
            RawFrame::make_std(self.raw_id, &self.data)
        }
    }
}

/// Parse one `candump` line in any of the common forms:
///
/// * compact:        `123#DEADBEEF`  /  `18FEE680#A4..`
/// * compact timed:  `(1691423381.123456) 18FEE680#A4..`
/// * bracketed:      `can0  18FEE680   [8]  A4 31 ..`
/// * bracketed timed:`(1691423381.123456) can0  18FEE680   [8]  ..`
///
/// Returns `None` for blank lines, comments, CAN-FD (`##`), error IDs and
/// malformed hex/lengths.
///
/// This mirrors the acceptance rules of `machbus`'s own fixture replay
/// (see `examples/candump_replay.rs`) but additionally preserves the
/// standard/extended distinction.
#[must_use]
pub fn parse_candump_line(line: &str) -> Option<ParsedFrame> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let (rest, timestamp_us) = strip_timestamp(line);
    let (rest, interface) = strip_interface(rest);

    // If the remaining text still contains a '#' before any whitespace it
    // is a compact token (`ID#DATA`); otherwise treat it as bracketed.
    let first = rest.split_whitespace().next()?;
    if first.contains('#') {
        parse_compact_token(first).map(|mut f| {
            f.timestamp_us = timestamp_us;
            f.interface = interface;
            f
        })
    } else {
        parse_bracketed(rest).map(|mut f| {
            f.timestamp_us = timestamp_us;
            f.interface = interface;
            f
        })
    }
}

fn strip_timestamp(line: &str) -> (&str, Option<u64>) {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix('(')
        && let Some(close) = rest.find(')')
    {
        let ts = &rest[..close];
        if let Some((s, us)) = ts.split_once('.')
            && let (Ok(secs), Ok(micros)) = (s.parse::<u64>(), us.parse::<u64>())
        {
            let after = rest[close + 1..].trim_start();
            return (
                after,
                Some(secs.saturating_mul(1_000_000).saturating_add(micros)),
            );
        }
    }
    (trimmed, None)
}

fn strip_interface(line: &str) -> (&str, Option<String>) {
    // A bare compact token has no interface; a bracketed/candump-style line
    // starts with the interface name followed by the hex ID.
    let trimmed = line.trim_start();
    let mut toks = trimmed.split_whitespace();
    let Some(first) = toks.next() else {
        return (trimmed, None);
    };
    // If the first token looks like an ID (contains '#' or is pure hex with
    // no trailing data token) there is no interface prefix.
    if first.contains('#') || is_hex_id_only(first) {
        return (trimmed, None);
    }
    (trimmed.slice_after_token(first), Some(first.to_string()))
}

trait StrExt {
    fn slice_after_token(&self, token: &str) -> &str;
}
impl StrExt for str {
    fn slice_after_token(&self, token: &str) -> &str {
        // Skip the leading token plus following whitespace.
        if let Some(idx) = self.find(token) {
            self[idx + token.len()..].trim_start()
        } else {
            self
        }
    }
}

fn is_hex_id_only(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit()) && (s.len() == 3 || s.len() == 8)
}

fn parse_compact_token(tok: &str) -> Option<ParsedFrame> {
    let (id_hex, payload) = tok.split_once('#')?;
    // Reject CAN FD ("##") and a second '#' fragment.
    if id_hex.is_empty() || payload.contains('#') {
        return None;
    }
    let raw_id = u32::from_str_radix(id_hex, 16).ok()?;
    let extended = classify_id(id_hex, raw_id)?;

    // RTR: `ID#R`, `ID#R0`, ... `ID#r`.
    let lower = payload.as_bytes();
    if !payload.is_empty() && (lower[0] == b'R' || lower[0] == b'r') {
        // Optional length digit after R is accepted but ignored for layout.
        return Some(ParsedFrame {
            timestamp_us: None,
            interface: None,
            raw_id,
            extended,
            rtr: true,
            data: Vec::new(),
        });
    }

    if payload.len() % 2 != 0 {
        return None;
    }
    let data = decode_hex(payload)?;
    if data.len() > CAN_MAX_DLEN {
        return None;
    }
    Some(ParsedFrame {
        timestamp_us: None,
        interface: None,
        raw_id,
        extended,
        rtr: false,
        data,
    })
}

fn parse_bracketed(line: &str) -> Option<ParsedFrame> {
    let mut toks = line.split_whitespace();
    let id_hex = toks.next()?;
    let dlc_tok = toks.next()?;
    let dlc_inner = dlc_tok.strip_prefix('[')?.strip_suffix(']')?;
    let dlc: usize = dlc_inner.parse().ok()?;
    if dlc > CAN_MAX_DLEN {
        return None;
    }

    let raw_id = u32::from_str_radix(id_hex, 16).ok()?;
    let extended = classify_id(id_hex, raw_id)?;

    let mut data = Vec::with_capacity(dlc);
    for tok in toks {
        if tok.len() != 2 {
            return None;
        }
        data.push(u8::from_str_radix(tok, 16).ok()?);
    }
    if data.len() != dlc {
        return None;
    }
    Some(ParsedFrame {
        timestamp_us: None,
        interface: None,
        raw_id,
        extended,
        rtr: false,
        data,
    })
}

/// Decide standard vs extended from the candump convention: 3 hex digits →
/// standard (11-bit), 8 hex digits → extended (29-bit). Rejects anything
/// else (CAN FD/error use 8 digits with the error flag bit set, handled by
/// the caller's ID-range checks elsewhere).
fn classify_id(id_hex: &str, raw_id: u32) -> Option<bool> {
    match id_hex.len() {
        1..=3 if raw_id <= CAN_SFF_MASK => Some(false),
        8 => {
            if raw_id & CAN_ERR_FLAG != 0 {
                None
            } else if raw_id <= CAN_EFF_MASK {
                Some(true)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 2);
    for chunk in s.as_bytes().chunks(2) {
        let txt = std::str::from_utf8(chunk).ok()?;
        out.push(u8::from_str_radix(txt, 16).ok()?);
    }
    Some(out)
}

// ── formatting ──────────────────────────────────────────────────────────
/// `candump -L`-style compact line, e.g. `(1691423381.123456) can0 18FEE680#A431..`.
#[must_use]
pub fn format_compact(frame: &RawFrame, iface: &str, timestamp_us: Option<u64>) -> String {
    let mut out = String::new();
    if let Some(ts) = timestamp_us {
        let _ = write!(out, "({}.{:06}) ", ts / 1_000_000, ts % 1_000_000);
    }
    let _ = write!(out, "{iface} ");
    let id = frame.id();
    if frame.is_extended() {
        let _ = write!(out, "{id:08X}");
    } else {
        let _ = write!(out, "{id:03X}");
    }
    out.push('#');
    if frame.is_rtr() {
        out.push('R');
    } else {
        for b in frame.payload() {
            let _ = write!(out, "{b:02X}");
        }
    }
    out
}

/// Human-readable `candump` line, e.g. `  can0  18FEE680   [8]  A4 31 ..`.
#[must_use]
pub fn format_readable(frame: &RawFrame, iface: &str) -> String {
    let id = frame.id();
    let dlc = frame.payload().len();
    let mut data = String::new();
    if frame.is_rtr() {
        data.push_str("remote request");
    } else {
        for b in frame.payload() {
            let _ = write!(data, " {b:02X}");
        }
    }
    if frame.is_extended() {
        format!("  {iface}  {id:08X}   [{dlc}]{data}")
    } else {
        format!("  {iface}    {id:03X}   [{dlc}]{data}")
    }
}

/// ISOBUS / J1939 decode of an extended data frame via `machbus`.
///
/// Returns the `(pgn, source, destination, pgn_name)` tuple when the frame
/// is a decodable 29-bit ISOBUS/J1939 ID, or `None` for standard frames,
/// RTR, or error frames.
#[must_use]
pub fn decode_isobus(frame: &RawFrame) -> Option<DecodeInfo> {
    if !frame.is_extended() || frame.is_rtr() || frame.is_err() {
        return None;
    }
    let id = machbus::net::Identifier::from_raw(frame.id());
    let pgn = id.pgn();
    let name = machbus::net::pgn_lookup(pgn).map(|info| info.name);
    Some(DecodeInfo {
        pgn,
        source: id.source(),
        destination: id.destination(),
        priority: id.priority().into(),
        name,
    })
}

/// Decoded ISOBUS/J1939 addressing for a single frame.
#[derive(Debug, Clone, Copy)]
pub struct DecodeInfo {
    pub pgn: u32,
    pub source: u8,
    pub destination: u8,
    pub priority: u8,
    pub name: Option<&'static str>,
}

impl DecodeInfo {
    /// Render as a `pgn=.. pri=.. src=.. dst=..` annotation suffix.
    #[must_use]
    pub fn annotate(&self) -> String {
        let mut s = format!(
            "  | pgn={:05X} pri={} src={:02X} dst={:02X}",
            self.pgn, self.priority, self.source, self.destination
        );
        if let Some(name) = self.name {
            let _ = write!(s, " [{name}]");
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compact_extended() {
        let f = parse_candump_line("18FEE680#A43116081C267D78").unwrap();
        assert!(f.extended);
        assert_eq!(f.raw_id, 0x18FEE680);
        assert_eq!(f.data, vec![0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78]);
        assert!(!f.rtr);
    }

    #[test]
    fn parses_compact_standard() {
        let f = parse_candump_line("123#DEADBEEF").unwrap();
        assert!(!f.extended);
        assert_eq!(f.raw_id, 0x123);
        assert_eq!(f.data.len(), 4);
    }

    #[test]
    fn parses_bracketed_with_timestamp_and_iface() {
        let f = parse_candump_line(
            "(1691423381.123456) vcan0  18FEE680   [8]  A4 31 16 08 1C 26 7D 78",
        )
        .unwrap();
        assert_eq!(f.timestamp_us, Some(1_691_423_381 * 1_000_000 + 123_456));
        assert_eq!(f.interface.as_deref(), Some("vcan0"));
        assert!(f.extended);
        assert_eq!(f.raw_id, 0x18FEE680);
        assert_eq!(f.data.len(), 8);
    }

    #[test]
    fn parses_rtr_compact() {
        let f = parse_candump_line("123#R").unwrap();
        assert!(!f.extended);
        assert!(f.rtr);
        let raw = f.to_raw();
        assert!(raw.is_rtr());
    }

    #[test]
    fn rejects_can_fd_and_overlong() {
        assert!(parse_candump_line("18FEE680##1AABB").is_none());
        assert!(parse_candump_line("18FEE680#000102030405060708").is_none());
        assert!(parse_candump_line("18FEE680#0").is_none());
        assert!(parse_candump_line("18FEE680#GG").is_none());
        assert!(parse_candump_line("can0 18FEE680 [8] A4 31").is_none()); // dlc mismatch
    }

    #[test]
    fn round_trips_through_raw() {
        let parsed = parse_candump_line("18FEE680#A43116081C267D78").unwrap();
        let raw = parsed.to_raw();
        assert!(raw.is_extended());
        assert_eq!(raw.id(), 0x18FEE680);
        assert_eq!(raw.payload(), parsed.data);
    }

    #[test]
    fn decode_isobus_on_time_date_frame() {
        let raw = RawFrame::make_ext(
            0x18FEE680,
            &[0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78],
        );
        let info = decode_isobus(&raw).expect("extended ISOBUS frame decodes");
        assert_eq!(info.pgn, 0x0FEE6);
        assert_eq!(info.source, 0x80);
        assert_eq!(info.destination, 0xFF);
        assert!(info.name.is_some());
    }

    #[test]
    fn decode_skips_standard_frames() {
        let raw = RawFrame::make_std(0x123, &[0xAA]);
        assert!(decode_isobus(&raw).is_none());
    }
}
