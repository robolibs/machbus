//! Live-capture filtering: a structured filter plus interactive field editor.

use crate::tui::decode::FrameKind;
use crate::tui::model::FrameEntry;

/// Which filter field is currently being edited in the Filter tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterField {
    Interface,
    IdExact,
    IdMin,
    IdMax,
    Pgn,
    Source,
    Dest,
    Text,
}

impl FilterField {
    pub const ALL: [FilterField; 8] = [
        FilterField::Interface,
        FilterField::IdExact,
        FilterField::IdMin,
        FilterField::IdMax,
        FilterField::Pgn,
        FilterField::Source,
        FilterField::Dest,
        FilterField::Text,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            FilterField::Interface => "Interface",
            FilterField::IdExact => "CAN ID ==",
            FilterField::IdMin => "ID min",
            FilterField::IdMax => "ID max",
            FilterField::Pgn => "PGN ==",
            FilterField::Source => "Source ==",
            FilterField::Dest => "Dest ==",
            FilterField::Text => "Data text",
        }
    }

    #[must_use]
    pub fn hint(self) -> &'static str {
        match self {
            FilterField::Interface => "name (empty = any)",
            FilterField::IdExact => "hex (e.g. 18FEE680)",
            FilterField::IdMin => "hex",
            FilterField::IdMax => "hex",
            FilterField::Pgn => "hex (e.g. 0FEE6)",
            FilterField::Source => "hex (e.g. 80)",
            FilterField::Dest => "hex (e.g. FF)",
            FilterField::Text => "substring of data hex",
        }
    }

    pub fn next(self) -> Self {
        let i = self as usize;
        FilterField::ALL[(i + 1) % FilterField::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let i = self as usize;
        FilterField::ALL[(i + FilterField::ALL.len() - 1) % FilterField::ALL.len()]
    }
}

/// A structured capture filter. Empty / `None` fields match anything.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub interface: Option<String>,
    pub id_exact: Option<u32>,
    pub id_min: Option<u32>,
    pub id_max: Option<u32>,
    pub pgn: Option<u32>,
    pub source: Option<u8>,
    pub dest: Option<u8>,
    pub text: Option<String>,
    pub extended_only: bool,
    pub std_only: bool,
    /// When set, only show frames of this coarse kind.
    pub kind: Option<FrameKind>,
}

impl Filter {
    /// Number of active constraints (for the status line badge).
    pub fn active_count(&self) -> usize {
        [
            self.interface.is_some(),
            self.id_exact.is_some(),
            self.id_min.is_some(),
            self.id_max.is_some(),
            self.pgn.is_some(),
            self.source.is_some(),
            self.dest.is_some(),
            self.text.is_some(),
            self.extended_only,
            self.std_only,
            self.kind.is_some(),
        ]
        .iter()
        .filter(|&&b| b)
        .count()
    }

    /// Does this frame pass every active constraint?
    #[must_use]
    pub fn matches(&self, e: &FrameEntry) -> bool {
        if let Some(iface) = &self.interface
            && !iface.is_empty()
            && &e.iface != iface
        {
            return false;
        }
        if let Some(id) = self.id_exact
            && e.raw_id != id
        {
            return false;
        }
        if let Some(lo) = self.id_min
            && e.raw_id < lo
        {
            return false;
        }
        if let Some(hi) = self.id_max
            && e.raw_id > hi
        {
            return false;
        }
        if self.extended_only && !e.extended {
            return false;
        }
        if self.std_only && e.extended {
            return false;
        }
        if let Some(kind) = self.kind
            && e.decoded.kind != kind
        {
            return false;
        }
        if let Some(pgn) = self.pgn
            && e.decoded.pgn != Some(pgn)
        {
            return false;
        }
        if let Some(src) = self.source
            && e.decoded.source != Some(src)
        {
            return false;
        }
        if let Some(dst) = self.dest
            && e.decoded.destination != Some(dst)
        {
            return false;
        }
        if let Some(needle) = &self.text
            && !needle.is_empty()
        {
            let hay = data_hex(&e.data[..(e.dlc as usize).min(8)]);
            if !hay.to_lowercase().contains(&needle.to_lowercase()) {
                return false;
            }
        }
        true
    }

    /// Parse a committed text value for the given field. Returns an error
    /// string on parse failure (hex/int ranges).
    pub fn set(&mut self, field: FilterField, value: &str) -> Result<(), String> {
        let trimmed = value.trim();
        let none_if_empty = |s: &str| -> Option<String> {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        };
        match field {
            FilterField::Interface => self.interface = none_if_empty(trimmed),
            FilterField::Text => self.text = none_if_empty(trimmed),
            FilterField::IdExact => self.id_exact = parse_opt_hex(trimmed, "CAN ID")?,
            FilterField::IdMin => self.id_min = parse_opt_hex(trimmed, "ID min")?,
            FilterField::IdMax => self.id_max = parse_opt_hex(trimmed, "ID max")?,
            FilterField::Pgn => self.pgn = parse_opt_hex(trimmed, "PGN")?,
            FilterField::Source => self.source = parse_opt_u8(trimmed, "Source")?,
            FilterField::Dest => self.dest = parse_opt_u8(trimmed, "Dest")?,
        }
        Ok(())
    }

    /// Render a field's current value as an editable string.
    #[must_use]
    pub fn render(&self, field: FilterField) -> String {
        match field {
            FilterField::Interface => self.interface.clone().unwrap_or_default(),
            FilterField::Text => self.text.clone().unwrap_or_default(),
            FilterField::IdExact => self.id_exact.map_or_else(String::new, |v| format!("{v:X}")),
            FilterField::IdMin => self.id_min.map_or_else(String::new, |v| format!("{v:X}")),
            FilterField::IdMax => self.id_max.map_or_else(String::new, |v| format!("{v:X}")),
            FilterField::Pgn => self.pgn.map_or_else(String::new, |v| format!("{v:X}")),
            FilterField::Source => self.source.map_or_else(String::new, |v| format!("{v:02X}")),
            FilterField::Dest => self.dest.map_or_else(String::new, |v| format!("{v:02X}")),
        }
    }

    pub fn clear(&mut self) {
        *self = Filter::default();
    }
}

fn data_hex(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        use std::fmt::Write;
        let _ = write!(s, "{b:02X}");
    }
    s
}

fn parse_opt_hex(s: &str, label: &str) -> Result<Option<u32>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    u32::from_str_radix(s, 16)
        .map(Some)
        .map_err(|e| format!("{label}: {e}"))
}

fn parse_opt_u8(s: &str, label: &str) -> Result<Option<u8>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    u8::from_str_radix(s, 16)
        .map(Some)
        .map_err(|e| format!("{label}: {e}"))
}
