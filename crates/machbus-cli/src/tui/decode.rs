//! Raw-frame → [`DecodedFrame`] decode bridge into the `machbus` protocol
//! model, plus inline field decoders for the most common NMEA 2000 PGNs.

use crate::tui::model::{DecodedFrame, FrameEntry};

use machbus::net::{Identifier, pgn_lookup};
use machbus::nmea::NMEA_PGN_CATALOG;

/// Coarse classification of a decoded frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameKind {
    /// Standard 11-bit, RTR, or error — not an ISOBUS/J1939 ID.
    Raw,
    /// J1939 / ISOBUS (29-bit, PGN below the NMEA 2000 range).
    J1939,
    /// NMEA 2000 (PGN in the ~126000+ range or present in the NMEA catalog).
    Nmea2000,
}

/// Decode a raw CAN frame into a [`DecodedFrame`].
///
/// Non-extended, RTR, and error frames come back as [`FrameKind::Raw`].
/// Extended frames are split into J1939/ISOBUS vs NMEA 2000 by PGN range
/// and catalog membership.
#[must_use]
pub fn decode_frame(
    raw_id: u32,
    extended: bool,
    rtr: bool,
    err: bool,
    data: &[u8],
) -> DecodedFrame {
    if !extended || rtr || err {
        return DecodedFrame::raw();
    }

    let id = Identifier::from_raw(raw_id);
    let pgn = id.pgn();
    let source = id.source();
    let destination = id.destination();
    let priority: u8 = id.priority().into();

    let (kind, name) = classify(pgn);
    let fields = if kind == FrameKind::Nmea2000 {
        decode_nmea_fields(pgn, data)
    } else {
        Vec::new()
    };

    DecodedFrame {
        kind,
        pgn: Some(pgn),
        priority: Some(priority),
        source: Some(source),
        destination: Some(destination),
        name,
        fields,
    }
}

/// Classify a PGN and look up its human-readable name across the J1939 and
/// NMEA 2000 tables that `machbus` ships.
fn classify(pgn: u32) -> (FrameKind, Option<&'static str>) {
    // NMEA 2000 first: explicit catalog membership, then the numeric range.
    if let Some(entry) = NMEA_PGN_CATALOG.iter().find(|e| e.pgn == pgn) {
        return (FrameKind::Nmea2000, Some(entry.name));
    }
    if pgn >= 126_000 {
        return (FrameKind::Nmea2000, None);
    }
    let name = pgn_lookup(pgn).map(|info| info.name);
    (FrameKind::J1939, name)
}

// ── little-endian readers (NMEA 2000 payload layout is LE) ──────────────
fn u16le(d: &[u8], o: usize) -> Option<u16> {
    let s = d.get(o..o + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}
fn i16le(d: &[u8], o: usize) -> Option<i16> {
    let s = d.get(o..o + 2)?;
    Some(i16::from_le_bytes([s[0], s[1]]))
}
fn u32le(d: &[u8], o: usize) -> Option<u32> {
    let s = d.get(o..o + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn i32le(d: &[u8], o: usize) -> Option<i32> {
    let s = d.get(o..o + 4)?;
    Some(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

const DATA_ERR: u16 = 0xFFFF;

/// Inline field decoders for the most common NMEA 2000 PGNs.
///
/// These return short `(label, value)` pairs shown in the detail pane.
/// Resolutions follow the NMEA 2000 field definitions; the data-errored
/// sentinel (`0xFFFF`/`0xFFFFFFFF`) is shown as `—`.
#[must_use]
pub fn decode_nmea_fields(pgn: u32, d: &[u8]) -> Vec<(String, String)> {
    match pgn {
        126_992 => {
            // System Time
            let mut v = Vec::new();
            if let Some(date) = u16le(d, 1) {
                v.push(("Date".into(), format_days(date)));
            }
            if let Some(t) = u32le(d, 3) {
                v.push(("Time".into(), format!("{}.{:04}s", t / 10_000, t % 10_000)));
            }
            v
        }
        127_250 => {
            // Vessel Heading
            let mut v = Vec::new();
            if let Some(h) = u16le(d, 1) {
                v.push(("Heading".into(), rad_to_deg_named(h)));
            }
            if let Some(dev) = i16le(d, 3) {
                v.push(("Deviation".into(), rad_to_deg_named_signed(dev)));
            }
            if let Some(var) = i16le(d, 5) {
                v.push(("Variation".into(), rad_to_deg_named_signed(var)));
            }
            v
        }
        127_488 => {
            // Engine Parameters, Rapid
            let mut v = Vec::new();
            if let Some(p) = u16le(d, 0) {
                v.push((
                    "Oil Press".into(),
                    if p == DATA_ERR {
                        "—".into()
                    } else {
                        format!("{:.0} kPa", f64::from(p) * 6.895)
                    },
                ));
            }
            if let Some(t) = u16le(d, 2) {
                v.push(("Oil Temp".into(), kelvin_u16_to_c(t)));
            }
            if let Some(rpm) = u16le(d, 4) {
                v.push((
                    "RPM".into(),
                    if rpm == DATA_ERR {
                        "—".into()
                    } else {
                        format!("{:.1}", f64::from(rpm) * 0.25)
                    },
                ));
            }
            v
        }
        128_259 => {
            // Speed, Water-referenced
            let mut v = Vec::new();
            if let Some(s) = u16le(d, 1) {
                v.push(("SOW".into(), speed_u16(s)));
            }
            v
        }
        128_267 => {
            // Water Depth
            let mut v = Vec::new();
            if let Some(depth) = u32le(d, 1) {
                v.push((
                    "Depth".into(),
                    if depth == 0xFFFFFFFF {
                        "—".into()
                    } else {
                        format!("{:.2} m", f64::from(depth) * 0.01)
                    },
                ));
            }
            if let Some(off) = i16le(d, 5) {
                v.push(("Offset".into(), format!("{:.3} m", f64::from(off) * 0.001)));
            }
            v
        }
        129_025 => {
            // Position, Rapid Update
            let mut v = Vec::new();
            if let Some(lat) = i32le(d, 0) {
                v.push(("Latitude".into(), geo_i32(lat)));
            }
            if let Some(lon) = i32le(d, 4) {
                v.push(("Longitude".into(), geo_i32(lon)));
            }
            v
        }
        129_026 => {
            // COG & SOG, Rapid Update
            let mut v = Vec::new();
            if let Some(cog) = u16le(d, 1) {
                v.push(("COG".into(), rad_to_deg_named(cog)));
            }
            if let Some(sog) = u16le(d, 3) {
                v.push(("SOG".into(), speed_u16(sog)));
            }
            v
        }
        130_306 => {
            // Wind Data
            let mut v = Vec::new();
            if let Some(ws) = u16le(d, 1) {
                v.push(("Wind Speed".into(), speed_u16(ws)));
            }
            if let Some(wa) = u16le(d, 3) {
                v.push(("Wind Angle".into(), rad_to_deg_named(wa)));
            }
            v
        }
        130_310 => {
            // Outside Environmental
            let mut v = Vec::new();
            if let Some(t) = i16le(d, 0) {
                v.push(("Temp".into(), kelvin_i16_to_c(t)));
            }
            if let Some(p) = u16le(d, 2) {
                v.push((
                    "Pressure".into(),
                    if p == DATA_ERR {
                        "—".into()
                    } else {
                        format!("{:.0} Pa", f64::from(p) * 100.0)
                    },
                ));
            }
            if let Some(h) = u16le(d, 4) {
                v.push((
                    "Humidity".into(),
                    if h == DATA_ERR {
                        "—".into()
                    } else {
                        format!("{:.1} %", f64::from(h) * 0.004)
                    },
                ));
            }
            v
        }
        _ => Vec::new(),
    }
}

// ── formatting helpers ──────────────────────────────────────────────────
fn rad_to_deg(rad: f64) -> f64 {
    rad.to_degrees()
}
fn rad_to_deg_named(raw: u16) -> String {
    if raw == DATA_ERR {
        "—".into()
    } else {
        format!("{:.1}°", rad_to_deg(f64::from(raw) * 1e-4))
    }
}
fn rad_to_deg_named_signed(raw: i16) -> String {
    if raw as u16 == DATA_ERR {
        "—".into()
    } else {
        format!("{:.1}°", rad_to_deg(f64::from(raw) * 1e-4))
    }
}
fn speed_u16(raw: u16) -> String {
    if raw == DATA_ERR {
        "—".into()
    } else {
        let mps = f64::from(raw) * 0.01;
        format!("{:.2} m/s ({:.1} kn)", mps, mps * 1.94384449)
    }
}
fn kelvin_u16_to_c(raw: u16) -> String {
    if raw == DATA_ERR {
        "—".into()
    } else {
        format!("{:.1} °C", f64::from(raw) * 0.1 - 273.15)
    }
}
fn kelvin_i16_to_c(raw: i16) -> String {
    if raw as u16 == DATA_ERR {
        "—".into()
    } else {
        format!("{:.1} °C", f64::from(raw) * 0.01 - 273.15)
    }
}
fn geo_i32(raw: i32) -> String {
    if raw == 0x7FFFFFFF {
        "—".into()
    } else {
        format!("{:.7}°", f64::from(raw) * 1e-7)
    }
}
fn format_days(days: u16) -> String {
    // Days since 1970-01-01.
    let secs = u64::from(days) * 86_400;
    let (y, mo, day) = days_to_ymd(secs);
    format!("{y:04}-{mo:02}-{day:02}")
}

/// Convert "seconds since 1970" to (year, month, day) using a civil calendar
/// algorithm (Howard Hinnant). Good enough for a display-only date.
fn days_to_ymd(secs_since_epoch: u64) -> (i32, u32, u32) {
    let days = (secs_since_epoch / 86_400) as i64 + 719_468;
    let era = if days >= 0 {
        days / 146_097
    } else {
        (days - 146_096) / 146_097
    };
    let doe = (days - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (y as i32 + if m <= 2 { 1 } else { 0 }, m as u32, d as u32)
}

/// Build a [`FrameEntry`] from raw capture fields, decoding inline.
pub(crate) fn build_entry(
    seq: u64,
    rel_ms: u64,
    iface: String,
    raw: &crate::can::RawFrame,
) -> FrameEntry {
    let dlc = raw.can_dlc;
    let data = raw.data;
    let decoded = decode_frame(
        raw.id(),
        raw.is_extended(),
        raw.is_rtr(),
        raw.is_err(),
        &data[..(dlc as usize).min(8)],
    );
    FrameEntry {
        seq,
        rel_ms,
        iface,
        raw_id: raw.id(),
        extended: raw.is_extended(),
        rtr: raw.is_rtr(),
        err: raw.is_err(),
        dlc,
        data,
        decoded,
    }
}
