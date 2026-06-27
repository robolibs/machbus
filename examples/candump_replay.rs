//! Replay/summarize classic `candump` text captures.
//!
//! This tool is deliberately small: it parses captured CAN frames, rejects
//! non-J1939/ISOBUS/NMEA2K frame shapes through [`machbus::net::Frame`], and
//! prints PGN/source/destination summaries that can be promoted into golden
//! fixtures.
//!
//! Both compact `candump -L` tokens (`18FEE680#...`) and the common bracketed
//! text format (`vcan0  18FEE680  [8]  ...`) are accepted. CAN FD/error/flagged
//! captures are intentionally ignored rather than guessed into classic CAN.
//!
//! ```text
//! cargo run --example candump_replay -- tests/fixtures/traces/time_date_agisostack.candump
//! ```

use std::env;
use std::fs;

use machbus::net::Frame;
use wirebit::can::{CAN_EFF_MASK, CAN_SFF_MASK, CanFrame};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedFrame {
    raw_id: u32,
    data: Vec<u8>,
    extended: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: cargo run --example candump_replay -- <capture.candump>")?;
    let text = fs::read_to_string(&path)?;

    let mut parsed = 0usize;
    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for (line_no, line) in text.lines().enumerate() {
        let Some(captured) = parse_candump_line(line) else {
            continue;
        };
        parsed += 1;

        let can = if captured.extended {
            CanFrame::make_ext(captured.raw_id, &captured.data)
        } else {
            CanFrame::make_std(captured.raw_id, &captured.data)
        };
        let Some(frame) = Frame::from_can_frame(&can) else {
            rejected += 1;
            println!("{:>5}: rejected non-ISOBUS frame", line_no + 1);
            continue;
        };
        accepted += 1;
        println!(
            "{:>5}: raw=0x{:08X} pgn=0x{:05X} src=0x{:02X} dst=0x{:02X} len={} data={}",
            line_no + 1,
            captured.raw_id,
            frame.pgn(),
            frame.source(),
            frame.destination(),
            frame.length,
            hex_bytes(frame.payload()),
        );
    }

    println!("summary: parsed={parsed} accepted={accepted} rejected={rejected}");
    Ok(())
}

fn parse_candump_line(line: &str) -> Option<CapturedFrame> {
    parse_hash_candump_line(line).or_else(|| parse_bracketed_candump_line(line))
}

fn parse_hash_candump_line(line: &str) -> Option<CapturedFrame> {
    let token = line.split_whitespace().find(|part| part.contains('#'))?;
    let (id_hex, payload_hex) = token.split_once('#')?;
    if id_hex.is_empty() || payload_hex.contains('#') {
        return None;
    }
    let raw_id = u32::from_str_radix(id_hex, 16).ok()?;
    let data = parse_hex_payload(payload_hex)?;
    if data.len() > 8 {
        return None;
    }
    let extended = match id_hex.len() {
        1..=3 if raw_id <= CAN_SFF_MASK => false,
        8 if raw_id <= CAN_EFF_MASK => true,
        _ => return None,
    };
    Some(CapturedFrame {
        raw_id,
        data,
        extended,
    })
}

fn parse_bracketed_candump_line(line: &str) -> Option<CapturedFrame> {
    let mut parts = line.split_whitespace().peekable();
    if parts
        .peek()
        .is_some_and(|token| token.starts_with('(') && token.ends_with(')') && token.len() > 2)
    {
        let _timestamp = parts.next();
    }

    let _interface = parts.next()?;
    let id_hex = parts.next()?;
    let dlc_token = parts.next()?;
    if !(dlc_token.starts_with('[') && dlc_token.ends_with(']')) {
        return None;
    }
    let dlc_text = &dlc_token[1..dlc_token.len() - 1];
    let dlc = dlc_text.parse::<usize>().ok()?;
    if dlc > 8 {
        return None;
    }

    let raw_id = u32::from_str_radix(id_hex, 16).ok()?;
    let extended = match id_hex.len() {
        1..=3 if raw_id <= CAN_SFF_MASK => false,
        8 if raw_id <= CAN_EFF_MASK => true,
        _ => return None,
    };

    let mut data = Vec::with_capacity(dlc);
    for token in parts {
        if token.len() != 2 {
            return None;
        }
        data.push(u8::from_str_radix(token, 16).ok()?);
    }
    if data.len() != dlc {
        return None;
    }

    Some(CapturedFrame {
        raw_id,
        data,
        extended,
    })
}

fn parse_hex_payload(payload_hex: &str) -> Option<Vec<u8>> {
    if !payload_hex.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(payload_hex.len() / 2);
    for chunk in payload_hex.as_bytes().chunks(2) {
        let text = std::str::from_utf8(chunk).ok()?;
        out.push(u8::from_str_radix(text, 16).ok()?);
    }
    Some(out)
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02X}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timestamped_candump_line() {
        let line = "(1691423381.123456) vcan0 18FEE680#E70708162A6400FA";
        let captured = parse_candump_line(line).unwrap();
        assert!(captured.extended);
        assert_eq!(captured.raw_id, 0x18FEE680);
        assert_eq!(
            captured.data,
            vec![0xE7, 0x07, 0x08, 0x16, 0x2A, 0x64, 0x00, 0xFA]
        );
    }

    #[test]
    fn preserves_standard_id_shape_for_rejection() {
        let line = "(1691423381.123456) vcan0 123#AABBCCDD";
        let captured = parse_candump_line(line).unwrap();
        assert!(!captured.extended);
        let standard = CanFrame::make_std(captured.raw_id, &captured.data);
        assert!(Frame::from_can_frame(&standard).is_none());
    }

    #[test]
    fn parses_bracketed_candump_line() {
        let line = "(1691423381.123456) vcan0  18FEE680   [8]  A4 31 16 08 1C 26 7D 78";
        let captured = parse_candump_line(line).unwrap();
        assert!(captured.extended);
        assert_eq!(captured.raw_id, 0x18FEE680);
        assert_eq!(
            captured.data,
            vec![0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78]
        );
    }

    #[test]
    fn rejects_bracketed_dlc_mismatches_and_fd_lengths() {
        assert!(parse_candump_line("vcan0 18FEE680 [8] A4 31").is_none());
        assert!(parse_candump_line("vcan0 18FEE680 [12] A4 31 16 08 1C 26 7D 78").is_none());
    }

    #[test]
    fn bracketed_fixture_replays_like_compact_trace() {
        let fixture = include_str!("../tests/fixtures/traces/bracketed_time_date.candump");
        let captured = parse_candump_line(fixture.trim()).unwrap();
        let can = CanFrame::make_ext(captured.raw_id, &captured.data);
        let frame = Frame::from_can_frame(&can).unwrap();
        assert_eq!(frame.pgn(), 0x0FEE6);
        assert_eq!(frame.source(), 0x47);
        assert_eq!(frame.destination(), 0xFF);
        assert_eq!(
            frame.payload(),
            &[0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78]
        );
    }

    #[test]
    fn rejects_can_fd_or_flagged_tokens() {
        assert!(parse_candump_line("(1691423381.123456) vcan0 18FEE680##1AABB").is_none());
        assert!(parse_candump_line("(1691423381.123456) vcan0 20000004#00000000").is_none());
    }

    #[test]
    fn rejects_compact_classic_overlong_and_bad_hex_payloads() {
        assert!(
            parse_candump_line("(1691423381.123456) vcan0 18FEE680#000102030405060708").is_none()
        );
        assert!(parse_candump_line("(1691423381.123456) vcan0 18FEE680#0").is_none());
        assert!(parse_candump_line("(1691423381.123456) vcan0 18FEE680#GG").is_none());
    }

    #[test]
    fn malformed_fixture_lines_are_ignored_before_driver_conversion() {
        let fixture = include_str!("../tests/fixtures/traces/malformed_candump.candump");
        for line in fixture.lines().filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        }) {
            assert!(parse_candump_line(line).is_none(), "{line}");
        }
    }

    #[test]
    fn rejection_fixture_preserves_standard_ids_without_promotion() {
        let fixture = include_str!("../tests/fixtures/traces/standard_id_rejection.candump");
        let mut parsed = 0usize;
        let mut accepted = 0usize;
        let mut rejected = 0usize;

        for line in fixture.lines() {
            let Some(captured) = parse_candump_line(line) else {
                continue;
            };
            parsed += 1;
            let can = if captured.extended {
                CanFrame::make_ext(captured.raw_id, &captured.data)
            } else {
                CanFrame::make_std(captured.raw_id, &captured.data)
            };
            if Frame::from_can_frame(&can).is_some() {
                accepted += 1;
            } else {
                rejected += 1;
            }
        }

        assert_eq!(parsed, 3);
        assert_eq!(accepted, 1);
        assert_eq!(rejected, 2);
    }
}
