//! NMEA 0183 sentence parser for serial GNSS streams.
//!
//! Mirrors the C++ `machbus::nmea::SerialGNSS`. The C++ holds a
//! `wirebit::SerialEndpoint&` and polls it on `update()`. The Rust
//! port decouples the I/O — feed raw bytes via
//! [`SerialGNSS::feed_bytes`]; complete sentences are parsed and
//! emit events.
//!
//! Supported sentences: `$GxGGA`, `$GxRMC`, `$GxVTG`, `$GxGSA`,
//! `$GxGLL`, `$GxGSV`, and `$GxZDA`.

use super::definitions::GNSSFixType;
use super::position::GNSSPosition;
use crate::geo::Wgs;
use crate::net::event::Event;
use alloc::{string::String, vec::Vec};
use core::f64::consts::PI;

/// Maximum buffered NMEA-0183 sentence bytes before dropping until the next
/// line terminator. NMEA-0183 sentences are ASCII and conventionally capped at
/// 82 characters including `$`, checksum, and CR/LF; this leaves room for the
/// supported GNSS sentences without permitting unbounded serial-buffer growth.
pub const NMEA0183_MAX_SENTENCE_BYTES: usize = 96;

/// Serial UART configuration. Matches the C++ struct one-for-one;
/// the parser itself only consumes bytes — these fields are passed
/// through to whichever serial backend the caller uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SerialGNSSConfig {
    pub baud: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: char,
}

impl Default for SerialGNSSConfig {
    fn default() -> Self {
        Self {
            baud: 115_200,
            data_bits: 8,
            stop_bits: 1,
            parity: 'N',
        }
    }
}

impl SerialGNSSConfig {
    #[must_use]
    pub const fn with_baud(mut self, rate: u32) -> Self {
        self.baud = rate;
        self
    }

    #[must_use]
    pub const fn with_data_bits(mut self, bits: u8) -> Self {
        self.data_bits = bits;
        self
    }

    #[must_use]
    pub const fn with_parity(mut self, p: char) -> Self {
        self.parity = p;
        self
    }

    #[must_use]
    pub const fn with_stop_bits(mut self, bits: u8) -> Self {
        self.stop_bits = bits;
        self
    }
}

/// Pump-style NMEA-0183 GNSS sentence parser.
pub struct SerialGNSS {
    line_buffer: String,
    dropping_oversize_line: bool,
    latest: GNSSPosition,
    satellites_in_view: Option<u8>,
    latest_utc_datetime: Option<NmeaUtcDateTime>,

    pub on_position: Event<GNSSPosition>,
    pub on_cog: Event<f64>,
    pub on_sog: Event<f64>,
}

/// UTC date/time decoded from a `$GxZDA` sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NmeaUtcDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub microsecond: u32,
    /// Local-zone offset hours from UTC as carried by ZDA.
    pub local_zone_hours: i8,
    /// Local-zone offset minutes from UTC as carried by ZDA.
    pub local_zone_minutes: u8,
}

impl Default for SerialGNSS {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialGNSS {
    #[must_use]
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            dropping_oversize_line: false,
            latest: GNSSPosition::default(),
            satellites_in_view: None,
            latest_utc_datetime: None,
            on_position: Event::new(),
            on_cog: Event::new(),
            on_sog: Event::new(),
        }
    }

    /// Feed raw bytes from the serial port. Sentences are split on
    /// `\n` / `\r`; complete ones are parsed and the appropriate
    /// events are fired.
    pub fn feed_bytes(&mut self, data: &[u8]) {
        for &b in data {
            if b == b'\n' || b == b'\r' {
                if self.dropping_oversize_line {
                    self.dropping_oversize_line = false;
                    self.line_buffer.clear();
                } else if !self.line_buffer.is_empty() {
                    let line = core::mem::take(&mut self.line_buffer);
                    self.parse_sentence(&line);
                }
            } else if !b.is_ascii() {
                self.line_buffer.clear();
                self.dropping_oversize_line = true;
            } else if self.dropping_oversize_line {
                continue;
            } else if self.line_buffer.len() >= NMEA0183_MAX_SENTENCE_BYTES {
                self.line_buffer.clear();
                self.dropping_oversize_line = true;
            } else {
                self.line_buffer.push(b as char);
            }
        }
    }

    /// Latest known position; `None` if no fix yet.
    #[must_use]
    pub fn latest_position(&self) -> Option<GNSSPosition> {
        if self.latest.has_fix() {
            Some(self.latest)
        } else {
            None
        }
    }

    /// Latest satellites-in-view count from `$GxGSV`, if one has been seen.
    #[must_use]
    pub const fn latest_satellites_in_view(&self) -> Option<u8> {
        self.satellites_in_view
    }

    /// Latest UTC date/time from `$GxZDA`, if one has been seen.
    #[must_use]
    pub const fn latest_utc_datetime(&self) -> Option<NmeaUtcDateTime> {
        self.latest_utc_datetime
    }

    fn parse_sentence(&mut self, sentence: &str) {
        if sentence.len() < 6
            || sentence.len() > NMEA0183_MAX_SENTENCE_BYTES
            || !sentence.starts_with('$')
            || !sentence.is_ascii()
        {
            return;
        }
        // Verify checksum if present. A malformed checksum field is treated as
        // a bad sentence instead of falling through as "checksum absent".
        if let Some(star_idx) = sentence.find('*') {
            if star_idx + 3 != sentence.len() {
                return;
            }
            let mut computed: u8 = 0;
            for &b in &sentence.as_bytes()[1..star_idx] {
                computed ^= b;
            }
            let expected_str = &sentence[star_idx + 1..star_idx + 3];
            let Ok(expected) = u8::from_str_radix(expected_str, 16) else {
                return;
            };
            if computed != expected {
                return;
            }
        }
        // Skip the talker ID (3 chars after `$`); the next 3 chars are
        // the sentence type.
        if sentence.len() < 6 {
            return;
        }
        let kind = &sentence.as_bytes()[3..6];
        match kind {
            b"GGA" => self.parse_gga(sentence),
            b"RMC" => self.parse_rmc(sentence),
            b"VTG" => self.parse_vtg(sentence),
            b"GSA" => self.parse_gsa(sentence),
            b"GLL" => self.parse_gll(sentence),
            b"GSV" => self.parse_gsv(sentence),
            b"ZDA" => self.parse_zda(sentence),
            _ => {}
        }
    }

    fn parse_gga(&mut self, sentence: &str) {
        let fields = split_fields(sentence);
        if fields.len() < 15 {
            return;
        }
        let Ok(quality) = fields[6].parse::<u8>() else {
            return;
        };
        if quality == 0 {
            self.latest.fix_type = GNSSFixType::NoFix;
            return;
        }
        let Some(fix_type) = gga_quality_to_fix_type(quality) else {
            return;
        };
        if fields[10] != "M" || fields[12] != "M" {
            return;
        }
        let Some(lat) = parse_lat(fields[2], fields[3]) else {
            return;
        };
        let Some(lon) = parse_lon(fields[4], fields[5]) else {
            return;
        };
        let timestamp_us = if !fields[1].is_empty() {
            let Some(timestamp_us) = parse_utc_time_us(fields[1]) else {
                return;
            };
            Some(timestamp_us)
        } else {
            None
        };
        let Some(altitude) = parse_finite_number(fields[9]) else {
            return;
        };
        let Ok(satellites_used) = fields[7].parse::<u8>() else {
            return;
        };
        if satellites_used > 99 {
            return;
        }
        let hdop = if !fields[8].is_empty() {
            let Some(hdop) = parse_nonnegative_finite_number(fields[8]) else {
                return;
            };
            Some(hdop)
        } else {
            None
        };
        let geoidal = if !fields[11].is_empty() {
            let Some(geoidal) = parse_finite_number(fields[11]) else {
                return;
            };
            Some(geoidal)
        } else {
            None
        };
        if let Some(timestamp_us) = timestamp_us {
            self.latest.timestamp_us = timestamp_us;
        }
        self.latest.wgs = Wgs::new(lat, lon, altitude);
        self.latest.fix_type = fix_type;
        self.latest.satellites_used = satellites_used;
        if let Some(hdop) = hdop {
            self.latest.hdop = Some(hdop);
        }
        if let Some(geoidal) = geoidal {
            self.latest.geoidal_separation_m = Some(geoidal);
        }
        let snapshot = self.latest;
        self.on_position.emit(&snapshot);
    }

    fn parse_rmc(&mut self, sentence: &str) {
        let fields = split_fields(sentence);
        if fields.len() < 12 {
            return;
        }
        if fields[2] != "A" {
            return;
        }
        let Some(lat) = parse_lat(fields[3], fields[4]) else {
            return;
        };
        let Some(lon) = parse_lon(fields[5], fields[6]) else {
            return;
        };
        let timestamp_us = if !fields[1].is_empty() {
            let Some(timestamp_us) = parse_utc_time_us(fields[1]) else {
                return;
            };
            Some(timestamp_us)
        } else {
            None
        };
        let speed_mps = if !fields[7].is_empty() {
            let Some(speed_knots) = parse_nonnegative_finite_number(fields[7]) else {
                return;
            };
            Some(speed_knots * 0.514_444)
        } else {
            None
        };
        let cog_rad = if !fields[8].is_empty() {
            let Some(cog_deg) = parse_course_degrees(fields[8]) else {
                return;
            };
            Some(cog_deg * PI / 180.0)
        } else {
            None
        };
        if !is_valid_rmc_date(fields[9]) {
            return;
        }
        if !fields[10].is_empty() {
            if !matches!(fields[11], "E" | "W")
                || parse_nonnegative_finite_number(fields[10]).is_none()
            {
                return;
            }
        } else if !fields[11].is_empty() {
            return;
        }
        if fields
            .get(12)
            .is_some_and(|mode| !mode.is_empty() && !nmea0183_mode_allows_valid_data(mode))
        {
            return;
        }
        if let Some(timestamp_us) = timestamp_us {
            self.latest.timestamp_us = timestamp_us;
        }
        self.latest.wgs = Wgs::new(lat, lon, self.latest.wgs.altitude);
        if let Some(mps) = speed_mps {
            self.latest.speed_mps = Some(mps);
            self.on_sog.emit(&mps);
        }
        if let Some(rad) = cog_rad {
            self.latest.cog_rad = Some(rad);
            self.on_cog.emit(&rad);
        }
        if matches!(self.latest.fix_type, GNSSFixType::NoFix) {
            self.latest.fix_type = GNSSFixType::GNSSFix;
        }
        let snapshot = self.latest;
        self.on_position.emit(&snapshot);
    }

    fn parse_vtg(&mut self, sentence: &str) {
        let fields = split_fields(sentence);
        if fields.len() < 9 {
            return;
        }
        let track_deg = if fields[1].is_empty() {
            None
        } else {
            if fields[2] != "T" {
                return;
            }
            let Some(track_deg) = parse_course_degrees(fields[1]) else {
                return;
            };
            Some(track_deg)
        };
        if !fields[3].is_empty() && (fields[4] != "M" || parse_course_degrees(fields[3]).is_none())
        {
            return;
        }
        if !fields[5].is_empty()
            && (fields[6] != "N" || parse_nonnegative_finite_number(fields[5]).is_none())
        {
            return;
        }
        let speed_kmh = if fields[7].is_empty() {
            None
        } else {
            if fields[8] != "K" {
                return;
            }
            let Some(speed_kmh) = parse_nonnegative_finite_number(fields[7]) else {
                return;
            };
            Some(speed_kmh)
        };
        if fields
            .get(9)
            .is_some_and(|mode| !mode.is_empty() && !nmea0183_mode_allows_valid_data(mode))
        {
            return;
        }
        if let Some(track_deg) = track_deg {
            let rad = track_deg * PI / 180.0;
            self.latest.cog_rad = Some(rad);
            self.on_cog.emit(&rad);
        }
        if let Some(speed_kmh) = speed_kmh {
            let mps = speed_kmh / 3.6;
            self.latest.speed_mps = Some(mps);
            self.on_sog.emit(&mps);
        }
    }

    fn parse_gsa(&mut self, sentence: &str) {
        let fields = split_fields(sentence);
        if fields.len() < 18 {
            return;
        }
        if !matches!(fields[1], "A" | "M") {
            return;
        }
        let Ok(fix) = fields[2].parse::<u8>() else {
            return;
        };
        if !(1..=3).contains(&fix) {
            return;
        }
        if !fields[3..15].iter().all(|satellite_id| {
            parse_optional_u16_bounded(satellite_id, 1, u16::from(u8::MAX)).is_some()
        }) {
            return;
        }
        if fields.get(18).is_some_and(|system_id| {
            parse_optional_u16_bounded(system_id, 0, u16::from(u8::MAX)).is_none()
        }) {
            return;
        }
        let pdop = if !fields[15].is_empty() {
            let Some(pdop) = parse_nonnegative_finite_number(fields[15]) else {
                return;
            };
            Some(pdop)
        } else {
            None
        };
        let hdop = if !fields[16].is_empty() {
            let Some(hdop) = parse_nonnegative_finite_number(fields[16]) else {
                return;
            };
            Some(hdop)
        } else {
            None
        };
        let vdop = if !fields[17].is_empty() {
            let Some(vdop) = parse_nonnegative_finite_number(fields[17]) else {
                return;
            };
            Some(vdop)
        } else {
            None
        };
        if fix == 1 {
            self.latest.fix_type = GNSSFixType::NoFix;
        } else if fix >= 2 && matches!(self.latest.fix_type, GNSSFixType::NoFix) {
            self.latest.fix_type = GNSSFixType::GNSSFix;
        }
        if let Some(pdop) = pdop {
            self.latest.pdop = Some(pdop);
        }
        if let Some(hdop) = hdop {
            self.latest.hdop = Some(hdop);
        }
        if let Some(vdop) = vdop {
            self.latest.vdop = Some(vdop);
        }
    }

    fn parse_gll(&mut self, sentence: &str) {
        let fields = split_fields(sentence);
        if fields.len() < 7 || fields[6] != "A" {
            return;
        }
        let Some(lat) = parse_lat(fields[1], fields[2]) else {
            return;
        };
        let Some(lon) = parse_lon(fields[3], fields[4]) else {
            return;
        };
        let timestamp_us = if !fields[5].is_empty() {
            let Some(timestamp_us) = parse_utc_time_us(fields[5]) else {
                return;
            };
            Some(timestamp_us)
        } else {
            None
        };
        if fields
            .get(7)
            .is_some_and(|mode| !mode.is_empty() && !nmea0183_mode_allows_valid_data(mode))
        {
            return;
        }
        if let Some(timestamp_us) = timestamp_us {
            self.latest.timestamp_us = timestamp_us;
        }
        self.latest.wgs = Wgs::new(lat, lon, self.latest.wgs.altitude);
        if matches!(self.latest.fix_type, GNSSFixType::NoFix) {
            self.latest.fix_type = GNSSFixType::GNSSFix;
        }
        let snapshot = self.latest;
        self.on_position.emit(&snapshot);
    }

    fn parse_gsv(&mut self, sentence: &str) {
        let fields = split_fields(sentence);
        if fields.len() < 4 {
            return;
        }
        let Ok(total_messages) = fields[1].parse::<u8>() else {
            return;
        };
        let Ok(message_number) = fields[2].parse::<u8>() else {
            return;
        };
        let Ok(satellites) = fields[3].parse::<u8>() else {
            return;
        };
        if total_messages == 0
            || message_number == 0
            || message_number > total_messages
            || satellites > 99
            || !gsv_satellite_fields_are_canonical(&fields[4..])
        {
            return;
        }
        self.satellites_in_view = Some(satellites);
    }

    fn parse_zda(&mut self, sentence: &str) {
        let fields = split_fields(sentence);
        if fields.len() < 7 {
            return;
        }
        let Some((hour, minute, second, microsecond, timestamp_us)) =
            parse_utc_time_parts(fields[1])
        else {
            return;
        };
        let Ok(day) = fields[2].parse::<u8>() else {
            return;
        };
        let Ok(month) = fields[3].parse::<u8>() else {
            return;
        };
        let Ok(year) = fields[4].parse::<u16>() else {
            return;
        };
        let Ok(local_zone_hours) = fields[5].parse::<i8>() else {
            return;
        };
        let Ok(local_zone_minutes) = fields[6].parse::<u8>() else {
            return;
        };
        if !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
            || !(-13..=13).contains(&local_zone_hours)
            || local_zone_minutes > 59
        {
            return;
        }
        self.latest.timestamp_us = timestamp_us;
        self.latest_utc_datetime = Some(NmeaUtcDateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            microsecond,
            local_zone_hours,
            local_zone_minutes,
        });
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────

fn split_fields(sentence: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut last = 0;
    for (i, c) in sentence.char_indices() {
        if c == ',' {
            out.push(&sentence[last..i]);
            last = i + 1;
        } else if c == '*' {
            out.push(&sentence[last..i]);
            return out;
        }
    }
    out.push(&sentence[last..]);
    out
}

fn parse_lat(value: &str, dir: &str) -> Option<f64> {
    parse_deg_min(value, dir, "N", "S", 90.0)
}

fn parse_lon(value: &str, dir: &str) -> Option<f64> {
    parse_deg_min(value, dir, "E", "W", 180.0)
}

fn parse_utc_time_us(value: &str) -> Option<u64> {
    parse_utc_time_parts(value).map(|(_, _, _, _, us)| us)
}

fn parse_finite_number(value: &str) -> Option<f64> {
    let parsed = value.parse::<f64>().ok()?;
    parsed.is_finite().then_some(parsed)
}

fn parse_nonnegative_finite_number(value: &str) -> Option<f64> {
    let parsed = parse_finite_number(value)?;
    (parsed >= 0.0).then_some(parsed)
}

fn parse_course_degrees(value: &str) -> Option<f64> {
    let parsed = parse_nonnegative_finite_number(value)?;
    (parsed <= 360.0).then_some(parsed)
}

fn gga_quality_to_fix_type(quality: u8) -> Option<GNSSFixType> {
    match quality {
        1 => Some(GNSSFixType::GNSSFix),
        2 => Some(GNSSFixType::DGNSSFix),
        3 => Some(GNSSFixType::PreciseGNSS),
        4 => Some(GNSSFixType::RTKFixed),
        5 => Some(GNSSFixType::RTKFloat),
        6 => Some(GNSSFixType::DeadReckon),
        7 => Some(GNSSFixType::ManualInput),
        8 => Some(GNSSFixType::SimulateMode),
        _ => None,
    }
}

fn is_valid_rmc_date(value: &str) -> bool {
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let Ok(day) = value[0..2].parse::<u8>() else {
        return false;
    };
    let Ok(month) = value[2..4].parse::<u8>() else {
        return false;
    };
    let Ok(year) = value[4..6].parse::<u16>() else {
        return false;
    };
    (1..=12).contains(&month) && day != 0 && day <= days_in_month(2000 + year, month)
}

fn nmea0183_mode_allows_valid_data(value: &str) -> bool {
    matches!(value, "A" | "D" | "E" | "M" | "S")
}

fn parse_optional_u16_bounded(value: &str, min: u16, max: u16) -> Option<()> {
    if value.is_empty() {
        return Some(());
    }
    let parsed = value.parse::<u16>().ok()?;
    (min..=max).contains(&parsed).then_some(())
}

fn gsv_satellite_fields_are_canonical(fields: &[&str]) -> bool {
    let (satellite_fields, signal_id) = match fields.len() % 4 {
        0 => (fields, None),
        1 => (&fields[..fields.len() - 1], fields.last().copied()),
        _ => return false,
    };
    if let Some(signal_id) = signal_id
        && parse_optional_u16_bounded(signal_id, 0, u16::from(u8::MAX)).is_none()
    {
        return false;
    }
    satellite_fields.chunks_exact(4).all(|group| {
        parse_optional_u16_bounded(group[0], 1, u16::from(u8::MAX)).is_some()
            && parse_optional_u16_bounded(group[1], 0, 90).is_some()
            && parse_optional_u16_bounded(group[2], 0, 359).is_some()
            && parse_optional_u16_bounded(group[3], 0, 99).is_some()
    })
}

fn parse_utc_time_parts(value: &str) -> Option<(u8, u8, u8, u32, u64)> {
    if value.len() < 6 || !value.is_ascii() {
        return None;
    }
    let (whole, fraction) = match value.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (value, None),
    };
    if whole.len() != 6 || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hour = whole[0..2].parse::<u8>().ok()?;
    let minute = whole[2..4].parse::<u8>().ok()?;
    let second = whole[4..6].parse::<u8>().ok()?;
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let mut microsecond = 0u32;
    if let Some(fraction) = fraction {
        if fraction.is_empty()
            || fraction.len() > 6
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let mut scale = 100_000u32;
        for b in fraction.bytes() {
            microsecond += u32::from(b - b'0') * scale;
            scale /= 10;
        }
    }
    let timestamp_us = ((u64::from(hour) * 60 + u64::from(minute)) * 60 + u64::from(second))
        * 1_000_000
        + u64::from(microsecond);
    Some((hour, minute, second, microsecond, timestamp_us))
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn parse_deg_min(
    value: &str,
    dir: &str,
    positive_dir: &str,
    negative_dir: &str,
    max_degrees: f64,
) -> Option<f64> {
    if value.is_empty() {
        return None;
    }
    let sign = if dir == positive_dir {
        1.0
    } else if dir == negative_dir {
        -1.0
    } else {
        return None;
    };
    let raw: f64 = value.parse().ok()?;
    if !raw.is_finite() || raw < 0.0 {
        return None;
    }
    let degrees = (raw / 100.0) as u32 as f64;
    let minutes = raw - degrees * 100.0;
    if !(0.0..60.0).contains(&minutes) {
        return None;
    }
    let result = degrees + minutes / 60.0;
    if result > max_degrees {
        return None;
    }
    Some(sign * result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn checksum(s: &str) -> u8 {
        let mut c = 0u8;
        for &b in s.as_bytes() {
            c ^= b;
        }
        c
    }

    fn with_checksum(body: &str) -> String {
        let cs = checksum(&body[1..]); // skip leading $
        format!("{body}*{:02X}\n", cs)
    }

    #[test]
    fn config_builders() {
        let c = SerialGNSSConfig::default()
            .with_baud(9600)
            .with_data_bits(7)
            .with_parity('E')
            .with_stop_bits(2);
        assert_eq!(c.baud, 9600);
        assert_eq!(c.parity, 'E');
    }

    #[test]
    fn parses_gga_with_fix() {
        let mut p = SerialGNSS::new();
        let log: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        p.on_position.subscribe(move |&x| lc.borrow_mut().push(x));
        let body = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        p.feed_bytes(with_checksum(body).as_bytes());
        assert_eq!(log.borrow().len(), 1);
        let pos = log.borrow()[0];
        assert!((pos.wgs.latitude - 48.117_3).abs() < 0.001);
        assert!((pos.wgs.longitude - 11.516_67).abs() < 0.001);
        assert_eq!(pos.fix_type, GNSSFixType::GNSSFix);
        assert_eq!(pos.satellites_used, 8);
    }

    #[test]
    fn parses_rmc_speed_and_cog() {
        let mut p = SerialGNSS::new();
        let body = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W";
        p.feed_bytes(with_checksum(body).as_bytes());
        let pos = p.latest_position().unwrap();
        // 22.4 knots ≈ 11.52 m/s.
        assert!((pos.speed_mps.unwrap() - 11.52).abs() < 0.05);
        // 84.4° ≈ 1.473 rad.
        assert!((pos.cog_rad.unwrap() - 1.473).abs() < 0.01);
    }

    #[test]
    fn invalid_checksum_drops_sentence() {
        let mut p = SerialGNSS::new();
        let log: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        p.on_position.subscribe(move |&x| lc.borrow_mut().push(x));
        p.feed_bytes(b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*FF\n");
        assert!(log.borrow().is_empty());
    }

    #[test]
    fn malformed_checksum_fields_drop_sentence() {
        for bad in [
            b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*GG\n".as_slice(),
            b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*4\n".as_slice(),
            b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47junk\n".as_slice(),
        ] {
            let mut p = SerialGNSS::new();
            let log: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
            let lc = log.clone();
            p.on_position.subscribe(move |&x| lc.borrow_mut().push(x));

            p.feed_bytes(bad);

            assert!(log.borrow().is_empty());
            assert!(p.latest_position().is_none());
        }
    }

    #[test]
    fn invalid_coordinates_do_not_overwrite_last_fix() {
        let mut p = SerialGNSS::new();
        let log: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        p.on_position.subscribe(move |&x| lc.borrow_mut().push(x));

        let good = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        p.feed_bytes(with_checksum(good).as_bytes());

        // The parser still accepts no-checksum sentences for receivers/loggers
        // that omit the field, but invalid coordinate fields must not update
        // the cache or emit a position event.
        p.feed_bytes(b"$GPGGA,123520,9960.000,N,18100.000,E,1,99,9.9,999.9,M,99.9,M,,\n");
        p.feed_bytes(b"$GPRMC,123520,A,4807.038,E,01131.000,N,001.0,001.0,230394,003.1,W\n");

        assert_eq!(log.borrow().len(), 1);
        let latest = p.latest_position().unwrap();
        assert_eq!(latest.satellites_used, 8);
        assert!((latest.wgs.latitude - 48.117_3).abs() < 0.001);
        assert!((latest.wgs.longitude - 11.516_67).abs() < 0.001);
        assert!((latest.wgs.altitude - 545.4).abs() < 0.001);
    }

    #[test]
    fn vtg_updates_cog_and_sog() {
        let mut p = SerialGNSS::new();
        let body = "$GPVTG,054.7,T,034.4,M,005.5,N,010.2,K";
        p.feed_bytes(with_checksum(body).as_bytes());
        // 10.2 km/h → 2.833 m/s.
        assert!((p.latest.speed_mps.unwrap() - 2.833).abs() < 0.05);
    }

    #[test]
    fn malformed_numeric_fields_and_unit_markers_do_not_mutate_state() {
        let mut p = SerialGNSS::new();
        let log: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
        let cog_log: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
        let sog_log: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        let cc = cog_log.clone();
        let sc = sog_log.clone();
        p.on_position.subscribe(move |&x| lc.borrow_mut().push(x));
        p.on_cog.subscribe(move |&x| cc.borrow_mut().push(x));
        p.on_sog.subscribe(move |&x| sc.borrow_mut().push(x));

        let good_gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        p.feed_bytes(with_checksum(good_gga).as_bytes());
        let good_vtg = "$GPVTG,054.7,T,034.4,M,005.5,N,010.2,K";
        p.feed_bytes(with_checksum(good_vtg).as_bytes());
        let baseline = p.latest_position().unwrap();
        let baseline_speed = p.latest.speed_mps.unwrap();
        let baseline_cog = p.latest.cog_rad.unwrap();

        for body in [
            // GGA altitude and geoidal separation require metre unit markers.
            "$GPGGA,123520,4807.038,N,01131.000,E,9,08,0.9,999.9,M,46.9,M,,",
            "$GPGGA,123520,4807.038,N,01131.000,E,1,08,0.9,999.9,F,46.9,M,,",
            "$GPGGA,123520,4807.038,N,01131.000,E,1,08,0.9,999.9,M,46.9,F,,",
            // Rust accepts these tokens as f64 values; the parser must not.
            "$GPGGA,123520,4807.038,N,01131.000,E,1,08,NaN,999.9,M,46.9,M,,",
            "$GPRMC,123520,A,4807.038,N,01131.000,E,inf,084.4,230394,003.1,W",
            "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,361.0,230394,003.1,W",
            "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,310299,003.1,W",
            "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,X",
            "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,230394,,W",
            "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W,N",
            "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W,X",
            // VTG must validate all unit markers before emitting either event.
            "$GPVTG,055.0,X,034.4,M,005.5,N,010.2,K",
            "$GPVTG,055.0,T,034.4,M,005.5,X,010.2,K",
            "$GPVTG,055.0,T,034.4,M,005.5,N,010.2,X",
            "$GPVTG,055.0,T,034.4,M,005.5,N,010.2,K,N",
            "$GPVTG,055.0,T,034.4,M,005.5,N,010.2,K,X",
            // GSA mode and finite DOP values are state-bearing.
            "$GPGSA,X,3,04,05,,09,12,,,24,,,,,2.5,1.3,2.1",
            "$GPGSA,A,3,04,05,,09,12,,,24,,,,,NaN,1.3,2.1",
            "$GPGSA,A,3,00,05,,09,12,,,24,,,,,2.5,1.3,2.1",
            "$GPGSA,A,3,256,05,,09,12,,,24,,,,,2.5,1.3,2.1",
            "$GPGSA,A,3,04,05,,09,12,,,24,,,,,2.5,1.3,2.1,999",
            // GLL must reject invalid timestamps and invalid optional data modes
            // before updating the cached fix.
            "$GPGLL,4916.45,N,12311.12,W,246060,A,A",
            "$GPGLL,4916.45,N,12311.12,W,225444,A,N",
            "$GPGLL,4916.45,N,12311.12,W,225444,A,X",
            // GSV must not update the satellites-in-view cache from malformed
            // satellite groups.
            "$GPGSV,3,1,11,07,91,048,42",
            "$GPGSV,3,1,11,07,79,360,42",
            "$GPGSV,3,1,11,07,79,048,100",
            "$GPGSV,3,1,11,07,79,048",
            "$GPGSV,3,1,11,07,79,048,42,999",
        ] {
            p.feed_bytes(with_checksum(body).as_bytes());
        }

        assert_eq!(log.borrow().len(), 1);
        assert_eq!(cog_log.borrow().len(), 1);
        assert_eq!(sog_log.borrow().len(), 1);
        assert_eq!(p.latest_satellites_in_view(), None);
        let latest = p.latest_position().unwrap();
        assert_eq!(latest, baseline);
        assert_eq!(p.latest.speed_mps, Some(baseline_speed));
        assert_eq!(p.latest.cog_rad, Some(baseline_cog));
    }

    #[test]
    fn gga_quality_codes_are_defined_before_position_update() {
        for (quality, expected) in [
            (1, GNSSFixType::GNSSFix),
            (2, GNSSFixType::DGNSSFix),
            (3, GNSSFixType::PreciseGNSS),
            (4, GNSSFixType::RTKFixed),
            (5, GNSSFixType::RTKFloat),
            (6, GNSSFixType::DeadReckon),
            (7, GNSSFixType::ManualInput),
            (8, GNSSFixType::SimulateMode),
        ] {
            let mut p = SerialGNSS::new();
            let body =
                format!("$GPGGA,123519,4807.038,N,01131.000,E,{quality},08,0.9,545.4,M,46.9,M,,");
            p.feed_bytes(with_checksum(&body).as_bytes());
            assert_eq!(p.latest_position().unwrap().fix_type, expected);
        }

        let mut p = SerialGNSS::new();
        let good = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        p.feed_bytes(with_checksum(good).as_bytes());
        let baseline = p.latest_position().unwrap();
        let bad_quality = "$GPGGA,123520,4807.038,N,01131.000,E,9,08,0.9,999.9,M,46.9,M,,";
        p.feed_bytes(with_checksum(bad_quality).as_bytes());
        assert_eq!(p.latest_position(), Some(baseline));
    }

    #[test]
    fn gsa_3d_fix_sets_fix_type() {
        let mut p = SerialGNSS::new();
        let body = "$GPGSA,A,3,04,05,,09,12,,,24,,,,,2.5,1.3,2.1";
        p.feed_bytes(with_checksum(body).as_bytes());
        assert_eq!(p.latest.fix_type, GNSSFixType::GNSSFix);
        assert!((p.latest.pdop.unwrap() - 2.5).abs() < 0.01);
        assert!((p.latest.hdop.unwrap() - 1.3).abs() < 0.01);
        assert!((p.latest.vdop.unwrap() - 2.1).abs() < 0.01);
    }

    #[test]
    fn parses_gll_position_and_time() {
        let mut p = SerialGNSS::new();
        let log: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        p.on_position.subscribe(move |&x| lc.borrow_mut().push(x));

        let body = "$GPGLL,4916.45,N,12311.12,W,225444,A,";
        p.feed_bytes(with_checksum(body).as_bytes());

        assert_eq!(log.borrow().len(), 1);
        let pos = p.latest_position().unwrap();
        assert!((pos.wgs.latitude - 49.274_166_666_7).abs() < 0.000_001);
        assert!((pos.wgs.longitude - -123.185_333_333_3).abs() < 0.000_001);
        assert_eq!(pos.timestamp_us, 82_484_000_000);
        assert_eq!(pos.fix_type, GNSSFixType::GNSSFix);
    }

    #[test]
    fn parses_gsv_satellites_in_view_without_overwriting_fix_satellite_count() {
        let mut p = SerialGNSS::new();
        let gga = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        p.feed_bytes(with_checksum(gga).as_bytes());
        let gsv = "$GPGSV,3,1,11,07,79,048,42,08,62,326,43,10,60,131,45,13,51,182,43";
        p.feed_bytes(with_checksum(gsv).as_bytes());

        assert_eq!(p.latest_satellites_in_view(), Some(11));
        assert_eq!(p.latest_position().unwrap().satellites_used, 8);

        for malformed in [
            "$GPGSV,3,1,12,07,91,048,42",
            "$GPGSV,3,1,12,07,79,360,42",
            "$GPGSV,3,1,12,07,79,048,100",
            "$GPGSV,3,1,12,07,79,048",
            "$GPGSV,3,1,12,07,79,048,42,999",
        ] {
            p.feed_bytes(with_checksum(malformed).as_bytes());
        }
        assert_eq!(
            p.latest_satellites_in_view(),
            Some(11),
            "malformed GSV satellite groups must not overwrite the last canonical satellites-in-view count"
        );
    }

    #[test]
    fn parses_zda_utc_datetime_and_rejects_invalid_calendar_values() {
        let mut p = SerialGNSS::new();
        let body = "$GPZDA,201530.25,04,07,2002,00,00";
        p.feed_bytes(with_checksum(body).as_bytes());

        assert_eq!(
            p.latest_utc_datetime(),
            Some(NmeaUtcDateTime {
                year: 2002,
                month: 7,
                day: 4,
                hour: 20,
                minute: 15,
                second: 30,
                microsecond: 250_000,
                local_zone_hours: 0,
                local_zone_minutes: 0,
            })
        );
        assert_eq!(p.latest.timestamp_us, 72_930_250_000);

        let invalid_day = "$GPZDA,201531.00,31,02,2002,00,00";
        p.feed_bytes(with_checksum(invalid_day).as_bytes());
        assert_eq!(p.latest.timestamp_us, 72_930_250_000);
    }

    #[test]
    fn gll_gsv_zda_malformed_sentences_do_not_mutate_state() {
        let mut p = SerialGNSS::new();
        p.feed_bytes(with_checksum("$GPGLL,4916.45,N,12311.12,W,225444,V,").as_bytes());
        p.feed_bytes(with_checksum("$GPGSV,3,4,11,07,79,048,42").as_bytes());
        p.feed_bytes(with_checksum("$GPZDA,246060.00,04,07,2002,00,00").as_bytes());

        assert!(p.latest_position().is_none());
        assert_eq!(p.latest_satellites_in_view(), None);
        assert_eq!(p.latest_utc_datetime(), None);
        assert_eq!(p.latest.timestamp_us, 0);
    }

    #[test]
    fn split_buffers_across_chunks() {
        let mut p = SerialGNSS::new();
        let body = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        let sentence = with_checksum(body);
        let bytes = sentence.as_bytes();
        // Split across two feeds.
        p.feed_bytes(&bytes[..20]);
        p.feed_bytes(&bytes[20..]);
        assert!(p.latest_position().is_some());
    }

    #[test]
    fn oversize_sentence_is_dropped_until_terminator() {
        let mut p = SerialGNSS::new();
        p.feed_bytes(b"$GPGGA,");
        p.feed_bytes(&[b'A'; NMEA0183_MAX_SENTENCE_BYTES + 32]);
        assert!(p.line_buffer.is_empty());
        assert!(p.dropping_oversize_line);

        // The tail of the oversized line must not be parsed when the
        // terminator eventually arrives.
        p.feed_bytes(b"\n");
        assert!(!p.dropping_oversize_line);
        assert!(p.latest_position().is_none());

        let body = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,";
        p.feed_bytes(with_checksum(body).as_bytes());
        assert!(p.latest_position().is_some());
    }

    #[test]
    fn non_ascii_bytes_drop_corrupted_sentence_without_poisoning_next_one() {
        let mut p = SerialGNSS::new();
        p.feed_bytes(b"$GP");
        p.feed_bytes(&[0xFF, 0x80, b'G', b'G', b'A', b'\n']);
        assert!(p.latest_position().is_none());
        assert!(p.line_buffer.is_empty());
        assert!(!p.dropping_oversize_line);

        let body = "$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W";
        p.feed_bytes(with_checksum(body).as_bytes());
        assert!(p.latest_position().is_some());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_arbitrary_serial_bytes_do_not_grow_unbounded(
            chunks in proptest::collection::vec(
                proptest::collection::vec(any::<u8>(), 0..=128),
                0..=16,
            ),
        ) {
            let mut p = SerialGNSS::new();
            for chunk in chunks {
                p.feed_bytes(&chunk);
                prop_assert!(p.line_buffer.len() <= NMEA0183_MAX_SENTENCE_BYTES);
                if p.dropping_oversize_line {
                    prop_assert!(p.line_buffer.is_empty());
                }
            }
        }
    }
}
