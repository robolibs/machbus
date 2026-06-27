//! CAN capture / replay / verification harness (ISO 11783-2/-3 evidence).
//!
//! GAP.md asks for a hardware/peer evidence path: capture bus traffic,
//! replay it, and verify expectations. Real capture needs physical or
//! virtual (`vcan`) CAN hardware, but the *harness* — the `candump` text
//! format reader/writer, an in-memory recorder, and a verifiable capture
//! log — is pure software and lives here so it is ready to run against a
//! `vcan0` interface (or a real adapter) without code changes.
//!
//! The `candump` line format is the de-facto SocketCAN capture format; this
//! reads both the bracketed (`(sec.usec) iface  ID   [n]  BB ..`) and the
//! compact hash (`ID#BBBB`) forms, and writes the bracketed form.

use alloc::{format, string::String, vec::Vec};

#[cfg(feature = "default")]
use std::io;

/// One captured CAN frame: a microsecond timestamp, the raw 29/11-bit CAN
/// id, and the data bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapturedFrame {
    pub timestamp_us: u64,
    pub can_id: u32,
    pub data: Vec<u8>,
}

/// Format one frame as a bracketed `candump` line (no trailing newline).
#[must_use]
pub fn format_candump_line(frame: &CapturedFrame, interface: &str) -> String {
    let secs = frame.timestamp_us / 1_000_000;
    let micros = frame.timestamp_us % 1_000_000;
    let hex: Vec<String> = frame.data.iter().map(|b| format!("{b:02X}")).collect();
    format!(
        "({secs}.{micros:06}) {interface}  {id:08X}   [{len}]  {bytes}",
        id = frame.can_id,
        len = frame.data.len(),
        bytes = hex.join(" "),
    )
}

/// Parse one `candump` line (bracketed or compact hash form). Returns
/// `None` for blank lines, comments, and malformed input.
#[must_use]
pub fn parse_candump_line(line: &str) -> Option<CapturedFrame> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if line.starts_with('(') {
        parse_bracketed(line)
    } else {
        parse_hash(line)
    }
}

fn parse_bracketed(line: &str) -> Option<CapturedFrame> {
    let close = line.find(')')?;
    let ts = &line[1..close];
    let (secs, micros) = ts.split_once('.')?;
    let timestamp_us =
        secs.parse::<u64>().ok()?.checked_mul(1_000_000)? + micros.parse::<u64>().ok()?;
    let mut toks = line[close + 1..].split_whitespace();
    let _interface = toks.next()?;
    let can_id = u32::from_str_radix(toks.next()?, 16).ok()?;
    let data = toks
        .filter(|t| !t.starts_with('['))
        .map(|t| u8::from_str_radix(t, 16))
        .collect::<Result<Vec<u8>, _>>()
        .ok()?;
    Some(CapturedFrame {
        timestamp_us,
        can_id,
        data,
    })
}

fn parse_hash(line: &str) -> Option<CapturedFrame> {
    let token = line.split_whitespace().next()?;
    let (id, payload) = token.split_once('#')?;
    let can_id = u32::from_str_radix(id, 16).ok()?;
    if payload.len() % 2 != 0 {
        return None;
    }
    let data = (0..payload.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .ok()?;
    Some(CapturedFrame {
        timestamp_us: 0,
        can_id,
        data,
    })
}

/// In-memory capture recorder. Records frames as they are seen on a (real
/// or `vcan`) interface, then writes a `candump` capture for evidence.
#[derive(Debug, Clone, Default)]
pub struct CaptureRecorder {
    interface: String,
    frames: Vec<CapturedFrame>,
}

impl CaptureRecorder {
    #[must_use]
    pub fn new(interface: impl Into<String>) -> Self {
        Self {
            interface: interface.into(),
            frames: Vec::new(),
        }
    }

    pub fn record(&mut self, timestamp_us: u64, can_id: u32, data: impl Into<Vec<u8>>) {
        self.frames.push(CapturedFrame {
            timestamp_us,
            can_id,
            data: data.into(),
        });
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Render the capture as `candump` text (one line per frame).
    #[must_use]
    pub fn to_candump(&self) -> String {
        let mut out = String::new();
        for f in &self.frames {
            out.push_str(&format_candump_line(f, &self.interface));
            out.push('\n');
        }
        out
    }

    /// Write the capture to a `candump` file for evidence.
    #[cfg(feature = "default")]
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> io::Result<()> {
        std::fs::write(path, self.to_candump())
    }
}

/// A parsed capture log that evidence checks can be run against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureLog {
    pub frames: Vec<CapturedFrame>,
}

impl CaptureLog {
    /// Parse `candump` text, ignoring blank/comment/malformed lines.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        Self {
            frames: text.lines().filter_map(parse_candump_line).collect(),
        }
    }

    /// Load a `candump` capture file.
    #[cfg(feature = "default")]
    pub fn load(path: impl AsRef<std::path::Path>) -> io::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(path)?))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// `true` if any captured frame has this raw CAN id.
    #[must_use]
    pub fn contains_can_id(&self, can_id: u32) -> bool {
        self.frames.iter().any(|f| f.can_id == can_id)
    }

    /// Number of captured frames with this raw CAN id.
    #[must_use]
    pub fn count_can_id(&self, can_id: u32) -> usize {
        self.frames.iter().filter(|f| f.can_id == can_id).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trips_through_candump_text() {
        let f = CapturedFrame {
            timestamp_us: 1_500_000,
            can_id: 0x18FE_E647,
            data: vec![0xA4, 0x31, 0x16, 0x08],
        };
        let line = format_candump_line(&f, "vcan0");
        assert!(line.starts_with("(1.500000) vcan0  18FEE647   [4]  A4 31 16 08"));
        assert_eq!(parse_candump_line(&line), Some(f));
    }

    #[test]
    fn parses_both_candump_forms_and_skips_noise() {
        // Bracketed form.
        let b = parse_candump_line("(0.000000) vcan0  18FEE680   [3]  01 02 03").unwrap();
        assert_eq!(b.can_id, 0x18FE_E680);
        assert_eq!(b.data, vec![1, 2, 3]);
        // Compact hash form.
        let h = parse_candump_line("18FEE680#AABBCC").unwrap();
        assert_eq!(h.can_id, 0x18FE_E680);
        assert_eq!(h.data, vec![0xAA, 0xBB, 0xCC]);
        // Noise.
        assert!(parse_candump_line("").is_none());
        assert!(parse_candump_line("# comment").is_none());
        assert!(parse_candump_line("garbage line").is_none());
    }

    #[test]
    fn recorder_and_log_support_capture_then_verify() {
        let mut rec = CaptureRecorder::new("vcan0");
        rec.record(0, 0x18EE_FF00, vec![1; 8]); // address claim
        rec.record(1_000, 0x18FE_E680, vec![2; 8]);
        rec.record(2_000, 0x18EE_FF00, vec![3; 8]);
        assert_eq!(rec.len(), 3);

        // Re-parse the rendered capture and run evidence checks.
        let log = CaptureLog::parse(&rec.to_candump());
        assert_eq!(log.len(), 3);
        assert!(log.contains_can_id(0x18EE_FF00));
        assert_eq!(log.count_can_id(0x18EE_FF00), 2);
        assert!(!log.contains_can_id(0x1234_5678));
    }

    #[test]
    fn recorder_saves_and_log_loads_from_disk() {
        let path = std::env::temp_dir().join("machbus_capture_test.candump");
        let mut rec = CaptureRecorder::new("vcan0");
        rec.record(5_000_000, 0x0CF0_0400, vec![0xDE, 0xAD]);
        rec.save(&path).unwrap();
        let log = CaptureLog::load(&path).unwrap();
        assert_eq!(log.len(), 1);
        assert!(log.contains_can_id(0x0CF0_0400));
        let _ = std::fs::remove_file(&path);
    }
}
