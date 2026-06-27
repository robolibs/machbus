//! NMEA 2000 network management: Product Info, Config Info,
//! Heartbeat (PGNs 126993 / 126996 / 126998).
//!
//! Mirrors the C++ `machbus::nmea::n2k_management.hpp`. Pump-style
//! port — methods build outbound payloads and the caller dispatches
//! via `IsoNet::send`.

use crate::net::error::{Error, ErrorCode, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_CONFIG_INFO, PGN_HEARTBEAT_N2K, PGN_PRODUCT_INFO};
use crate::net::types::{Address, Pgn};
use crate::net::{BROADCAST_ADDRESS, NULL_ADDRESS};
use alloc::{format, string::String, vec, vec::Vec};

pub const N2K_REQUEST_TIMEOUT_MS: u32 = 5000;
/// NMEA 2000 network-management PGNs implemented by [`N2KManagement`].
pub const NMEA2000_MANAGEMENT_PGNS: [Pgn; 3] =
    [PGN_HEARTBEAT_N2K, PGN_PRODUCT_INFO, PGN_CONFIG_INFO];
const N2K_CONFIG_STRING_MAX_BYTES: usize = 70;

// ─── Product Information (PGN 126996) ──────────────────────────────────

/// Product info payload (134-byte fixed-layout, fast-packet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct N2KProductInfo {
    /// `0x0901` = NMEA2000 v2.1.
    pub nmea2000_version: u16,
    pub product_code: u16,
    /// Up to 32 chars.
    pub model_id: String,
    /// Up to 40 chars.
    pub software_version: String,
    /// Up to 24 chars.
    pub model_version: String,
    /// Up to 32 chars.
    pub serial_code: String,
    pub certification_level: u8,
    pub load_equivalency: u8,
}

impl Default for N2KProductInfo {
    fn default() -> Self {
        Self {
            nmea2000_version: 0x0901,
            product_code: 0,
            model_id: String::new(),
            software_version: String::new(),
            model_version: String::new(),
            serial_code: String::new(),
            certification_level: 0,
            load_equivalency: 1,
        }
    }
}

impl N2KProductInfo {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(134);
        data.extend_from_slice(&self.nmea2000_version.to_le_bytes());
        data.extend_from_slice(&self.product_code.to_le_bytes());
        push_fixed_string(&mut data, &self.model_id, 32, "model id")?;
        push_fixed_string(&mut data, &self.software_version, 40, "software version")?;
        push_fixed_string(&mut data, &self.model_version, 24, "model version")?;
        push_fixed_string(&mut data, &self.serial_code, 32, "serial code")?;
        data.push(self.certification_level);
        data.push(self.load_equivalency);
        Ok(data)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() != 134 {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "product info must be exactly 134 bytes",
            ));
        }
        Ok(Self {
            nmea2000_version: u16::from_le_bytes([data[0], data[1]]),
            product_code: u16::from_le_bytes([data[2], data[3]]),
            model_id: read_fixed_string(&data[4..36], "model id")?,
            software_version: read_fixed_string(&data[36..76], "software version")?,
            model_version: read_fixed_string(&data[76..100], "model version")?,
            serial_code: read_fixed_string(&data[100..132], "serial code")?,
            certification_level: data[132],
            load_equivalency: data[133],
        })
    }
}

// ─── Configuration Information (PGN 126998) ────────────────────────────

/// Config info payload; three length-prefixed strings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct N2KConfigInfo {
    /// Up to 70 chars.
    pub installation_desc1: String,
    /// Up to 70 chars.
    pub installation_desc2: String,
    pub manufacturer_info: String,
}

impl N2KConfigInfo {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        push_len_prefixed(
            &mut data,
            &self.installation_desc1,
            N2K_CONFIG_STRING_MAX_BYTES,
            "installation desc1",
        )?;
        push_len_prefixed(
            &mut data,
            &self.installation_desc2,
            N2K_CONFIG_STRING_MAX_BYTES,
            "installation desc2",
        )?;
        push_len_prefixed(
            &mut data,
            &self.manufacturer_info,
            N2K_CONFIG_STRING_MAX_BYTES,
            "manufacturer info",
        )?;
        Ok(data)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 6 {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "config info too short",
            ));
        }
        let mut offset = 0;
        let installation_desc1 =
            read_len_prefixed(data, &mut offset, "desc1", N2K_CONFIG_STRING_MAX_BYTES)?;
        let installation_desc2 =
            read_len_prefixed(data, &mut offset, "desc2", N2K_CONFIG_STRING_MAX_BYTES)?;
        let manufacturer_info = read_len_prefixed(
            data,
            &mut offset,
            "manufacturer info",
            N2K_CONFIG_STRING_MAX_BYTES,
        )?;
        if offset != data.len() {
            return Err(Error::with_message(
                ErrorCode::InvalidData,
                "config info has trailing bytes",
            ));
        }
        Ok(Self {
            installation_desc1,
            installation_desc2,
            manufacturer_info,
        })
    }
}

// ─── Heartbeat (PGN 126993) ────────────────────────────────────────────

/// 8-byte broadcast heartbeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct N2KHeartbeat {
    pub update_interval_ms: u32,
    /// Low 4 bits.
    pub sequence_counter: u8,
    pub controller_class1: u8,
    pub controller_class2: u8,
}

impl Default for N2KHeartbeat {
    fn default() -> Self {
        Self {
            update_interval_ms: 60_000,
            sequence_counter: 0,
            controller_class1: 0xFF,
            controller_class2: 0xFF,
        }
    }
}

impl N2KHeartbeat {
    pub fn encode(&self) -> Result<[u8; 8]> {
        if !self.update_interval_ms.is_multiple_of(50) {
            return Err(Error::invalid_data(format!(
                "heartbeat interval {} ms is not a 50 ms multiple",
                self.update_interval_ms
            )));
        }
        let interval_raw = self.update_interval_ms / 50;
        if interval_raw > u16::MAX as u32 {
            return Err(Error::invalid_data(format!(
                "heartbeat interval {} ms exceeds the u16*50 ms wire range",
                self.update_interval_ms
            )));
        }
        if self.sequence_counter > 0x0F {
            return Err(Error::invalid_data(format!(
                "heartbeat sequence counter {} exceeds 4 bits",
                self.sequence_counter
            )));
        }
        let mut data = [0xFFu8; 8];
        let interval_raw = interval_raw as u16;
        data[0..2].copy_from_slice(&interval_raw.to_le_bytes());
        data[2] = self.sequence_counter | 0xF0;
        data[3] = self.controller_class1;
        data[4] = self.controller_class2;
        Ok(data)
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || data[2] & 0xF0 != 0xF0 || data[5..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        let interval_raw = u16::from_le_bytes([data[0], data[1]]);
        Some(Self {
            update_interval_ms: interval_raw as u32 * 50,
            sequence_counter: data[2] & 0x0F,
            controller_class1: data[3],
            controller_class2: data[4],
        })
    }
}

// ─── Configuration aggregate ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct N2KManagementConfig {
    pub product_info: N2KProductInfo,
    pub config_info: N2KConfigInfo,
    pub heartbeat_interval_ms: u32,
}

impl N2KManagementConfig {
    #[must_use]
    pub fn with_product(mut self, p: N2KProductInfo) -> Self {
        self.product_info = p;
        self
    }

    #[must_use]
    pub fn with_config(mut self, c: N2KConfigInfo) -> Self {
        self.config_info = c;
        self
    }

    #[must_use]
    pub const fn with_heartbeat_interval(mut self, ms: u32) -> Self {
        self.heartbeat_interval_ms = ms;
        self
    }
}

/// Pending request tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingRequest {
    pub pgn: Pgn,
    pub destination: Address,
    pub elapsed_ms: u32,
    pub active: bool,
}

impl Default for PendingRequest {
    fn default() -> Self {
        Self {
            pgn: 0,
            destination: 0xFF,
            elapsed_ms: 0,
            active: false,
        }
    }
}

// ─── N2K Management (pump-style) ──────────────────────────────────────

/// Outbound from the management layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct N2KOutbound {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub dest: Option<Address>,
}

impl N2KOutbound {
    #[must_use]
    pub fn broadcast(pgn: Pgn, data: Vec<u8>) -> Self {
        Self {
            pgn,
            data,
            dest: None,
        }
    }

    #[must_use]
    pub fn to(pgn: Pgn, data: Vec<u8>, dest: Address) -> Self {
        Self {
            pgn,
            data,
            dest: Some(dest),
        }
    }
}

/// NMEA2000 network-management handler.
pub struct N2KManagement {
    config: N2KManagementConfig,
    heartbeat_timer_ms: u32,
    heartbeat_seq: u8,
    pending_requests: Vec<PendingRequest>,

    pub on_product_info_received: Event<(N2KProductInfo, Address)>,
    pub on_config_info_received: Event<(N2KConfigInfo, Address)>,
    pub on_heartbeat_received: Event<(N2KHeartbeat, Address)>,
    pub on_product_info_requested: Event<Address>,
    pub on_config_info_requested: Event<Address>,
    /// `(pgn, destination)`.
    pub on_request_timeout: Event<(Pgn, Address)>,
}

impl N2KManagement {
    #[must_use]
    pub fn new(config: N2KManagementConfig) -> Self {
        Self {
            config,
            heartbeat_timer_ms: 0,
            heartbeat_seq: 0,
            pending_requests: Vec::new(),
            on_product_info_received: Event::new(),
            on_config_info_received: Event::new(),
            on_heartbeat_received: Event::new(),
            on_product_info_requested: Event::new(),
            on_config_info_requested: Event::new(),
            on_request_timeout: Event::new(),
        }
    }

    /// Advance timers; returns periodic heartbeat frames.
    pub fn update(&mut self, elapsed_ms: u32) -> Result<Vec<N2KOutbound>> {
        let mut out = Vec::new();
        self.heartbeat_timer_ms = self.heartbeat_timer_ms.saturating_add(elapsed_ms);
        if self.config.heartbeat_interval_ms > 0
            && self.heartbeat_timer_ms >= self.config.heartbeat_interval_ms
        {
            self.heartbeat_timer_ms %= self.config.heartbeat_interval_ms;
            out.push(self.send_heartbeat()?);
        }
        let mut i = 0;
        while i < self.pending_requests.len() {
            if self.pending_requests[i].active {
                self.pending_requests[i].elapsed_ms = self.pending_requests[i]
                    .elapsed_ms
                    .saturating_add(elapsed_ms);
                if self.pending_requests[i].elapsed_ms >= N2K_REQUEST_TIMEOUT_MS {
                    let pgn = self.pending_requests[i].pgn;
                    let dest = self.pending_requests[i].destination;
                    self.pending_requests.remove(i);
                    self.on_request_timeout.emit(&(pgn, dest));
                    continue;
                }
            }
            i += 1;
        }
        Ok(out)
    }

    pub fn send_product_info(&mut self) -> Result<N2KOutbound> {
        let data = self.config.product_info.encode()?;
        Ok(N2KOutbound::broadcast(
            crate::net::pgn_defs::PGN_PRODUCT_INFO,
            data,
        ))
    }

    pub fn send_config_info(&mut self, requester: Address) -> Result<N2KOutbound> {
        if !valid_n2k_peer_address(requester) {
            return Err(Error::invalid_address(requester));
        }
        let data = self.config.config_info.encode()?;
        self.on_config_info_requested.emit(&requester);
        Ok(N2KOutbound::to(
            crate::net::pgn_defs::PGN_CONFIG_INFO,
            data,
            requester,
        ))
    }

    pub fn send_heartbeat(&mut self) -> Result<N2KOutbound> {
        let hb = N2KHeartbeat {
            update_interval_ms: self.config.heartbeat_interval_ms,
            sequence_counter: self.heartbeat_seq,
            ..Default::default()
        };
        let data = hb.encode()?.to_vec();
        self.heartbeat_seq = (self.heartbeat_seq + 1) & 0x0F;
        Ok(N2KOutbound::broadcast(
            crate::net::pgn_defs::PGN_HEARTBEAT_N2K,
            data,
        ))
    }

    /// Build a destination-specific Product-Info request (sent on
    /// `PGN_REQUEST`).
    pub fn request_product_info(&mut self, target: Address) -> Result<N2KOutbound> {
        if !valid_n2k_peer_address(target) {
            return Err(Error::invalid_address(target));
        }
        if self.has_pending_request_for(PGN_PRODUCT_INFO, target) {
            return Err(Error::invalid_state(
                "request already pending to destination",
            ));
        }
        self.pending_requests.push(PendingRequest {
            pgn: PGN_PRODUCT_INFO,
            destination: target,
            elapsed_ms: 0,
            active: true,
        });
        Ok(N2KOutbound::to(
            crate::net::pgn_defs::PGN_REQUEST,
            request_payload(PGN_PRODUCT_INFO),
            target,
        ))
    }

    pub fn request_config_info(&mut self, target: Address) -> Result<N2KOutbound> {
        if !valid_n2k_peer_address(target) {
            return Err(Error::invalid_address(target));
        }
        if self.has_pending_request_for(crate::net::pgn_defs::PGN_CONFIG_INFO, target) {
            return Err(Error::invalid_state(
                "request already pending to destination",
            ));
        }
        self.pending_requests.push(PendingRequest {
            pgn: crate::net::pgn_defs::PGN_CONFIG_INFO,
            destination: target,
            elapsed_ms: 0,
            active: true,
        });
        Ok(N2KOutbound::to(
            crate::net::pgn_defs::PGN_REQUEST,
            request_payload(crate::net::pgn_defs::PGN_CONFIG_INFO),
            target,
        ))
    }

    /// Feed an inbound message; returns nothing (events fire as side
    /// effects). Routes on the message's PGN.
    pub fn handle_message(&mut self, msg: &Message) {
        if !msg.has_usable_envelope_for_pgn(msg.pgn) {
            return;
        }
        match msg.pgn {
            crate::net::pgn_defs::PGN_PRODUCT_INFO => {
                if let Ok(info) = N2KProductInfo::decode(&msg.data) {
                    self.clear_pending_request(PGN_PRODUCT_INFO, msg.source);
                    self.on_product_info_received.emit(&(info, msg.source));
                }
            }
            crate::net::pgn_defs::PGN_CONFIG_INFO => {
                if let Ok(info) = N2KConfigInfo::decode(&msg.data) {
                    self.clear_pending_request(crate::net::pgn_defs::PGN_CONFIG_INFO, msg.source);
                    self.on_config_info_received.emit(&(info, msg.source));
                }
            }
            crate::net::pgn_defs::PGN_HEARTBEAT_N2K => {
                if let Some(hb) = N2KHeartbeat::decode(&msg.data) {
                    self.on_heartbeat_received.emit(&(hb, msg.source));
                }
            }
            _ => {}
        }
    }

    pub fn set_product_info(&mut self, info: N2KProductInfo) {
        self.config.product_info = info;
    }

    pub fn set_config_info(&mut self, info: N2KConfigInfo) {
        self.config.config_info = info;
    }

    #[inline]
    #[must_use]
    pub const fn product_info(&self) -> &N2KProductInfo {
        &self.config.product_info
    }

    #[inline]
    #[must_use]
    pub const fn config_info(&self) -> &N2KConfigInfo {
        &self.config.config_info
    }

    #[inline]
    #[must_use]
    pub const fn heartbeat_interval(&self) -> u32 {
        self.config.heartbeat_interval_ms
    }

    #[inline]
    #[must_use]
    pub const fn heartbeat_sequence(&self) -> u8 {
        self.heartbeat_seq
    }

    #[inline]
    #[must_use]
    pub fn pending_requests(&self) -> &[PendingRequest] {
        &self.pending_requests
    }

    #[must_use]
    pub fn has_pending_request_to(&self, dest: Address) -> bool {
        self.pending_requests
            .iter()
            .any(|r| r.active && r.destination == dest)
    }

    #[must_use]
    pub fn has_pending_request_for(&self, pgn: Pgn, dest: Address) -> bool {
        self.pending_requests
            .iter()
            .any(|r| r.active && r.pgn == pgn && r.destination == dest)
    }

    fn clear_pending_request(&mut self, pgn: Pgn, source: Address) {
        if let Some(pos) = self
            .pending_requests
            .iter()
            .position(|r| r.active && r.pgn == pgn && r.destination == source)
        {
            self.pending_requests.remove(pos);
        }
    }
}

impl Default for N2KManagement {
    fn default() -> Self {
        Self::new(N2KManagementConfig::default())
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────

fn push_fixed_string(out: &mut Vec<u8>, s: &str, len: usize, label: &str) -> Result<()> {
    validate_n2k_text(s, len, label)?;
    let bytes = s.as_bytes();
    for i in 0..len {
        out.push(if i < bytes.len() { bytes[i] } else { 0xFF });
    }
    Ok(())
}

fn read_fixed_string(buf: &[u8], label: &str) -> Result<String> {
    let mut s = String::new();
    let mut padding = false;
    for &b in buf {
        if padding {
            if b != 0xFF {
                return Err(Error::invalid_data(format!(
                    "{label} contains non-0xFF bytes after string padding"
                )));
            }
            continue;
        }
        if b == 0xFF || b == 0x00 {
            padding = true;
            continue;
        }
        if !(0x20..=0x7E).contains(&b) {
            return Err(Error::invalid_data(format!(
                "{label} contains non-printable or non-ASCII byte 0x{b:02X}"
            )));
        }
        s.push(b as char);
    }
    Ok(s)
}

fn push_len_prefixed(out: &mut Vec<u8>, s: &str, max_len: usize, label: &str) -> Result<()> {
    validate_n2k_text(s, max_len, label)?;
    let bytes = s.as_bytes();
    out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn validate_n2k_text(s: &str, max_len: usize, label: &str) -> Result<()> {
    if s.len() > max_len {
        return Err(Error::invalid_data(format!(
            "{label} length {} exceeds {max_len} bytes",
            s.len()
        )));
    }
    if let Some(b) = s.bytes().find(|b| !(0x20..=0x7E).contains(b)) {
        return Err(Error::invalid_data(format!(
            "{label} contains non-printable or non-ASCII byte 0x{b:02X}"
        )));
    }
    Ok(())
}

fn read_len_prefixed(
    data: &[u8],
    offset: &mut usize,
    label: &str,
    max_len: usize,
) -> Result<String> {
    if *offset + 2 > data.len() {
        return Err(Error::with_message(
            ErrorCode::InvalidData,
            "config info truncated",
        ));
    }
    let len = u16::from_le_bytes([data[*offset], data[*offset + 1]]) as usize;
    *offset += 2;
    if len > max_len {
        return Err(Error::with_message(
            ErrorCode::InvalidData,
            format!("{label} exceeds maximum length"),
        ));
    }
    if *offset + len > data.len() {
        return Err(Error::with_message(
            ErrorCode::InvalidData,
            format!("{label} truncated"),
        ));
    }
    let s: String = data[*offset..*offset + len]
        .iter()
        .map(|&b| b as char)
        .collect();
    validate_n2k_text(&s, max_len, label)?;
    *offset += len;
    Ok(s)
}

fn request_payload(pgn: Pgn) -> Vec<u8> {
    vec![
        (pgn & 0xFF) as u8,
        ((pgn >> 8) & 0xFF) as u8,
        ((pgn >> 16) & 0xFF) as u8,
    ]
}

#[inline]
const fn valid_n2k_peer_address(address: Address) -> bool {
    address != NULL_ADDRESS && address != BROADCAST_ADDRESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::{PGN_CONFIG_INFO, PGN_HEARTBEAT_N2K, PGN_PRODUCT_INFO, PGN_REQUEST};

    #[test]
    fn product_info_round_trip() {
        let p = N2KProductInfo {
            nmea2000_version: 0x0901,
            product_code: 1234,
            model_id: "MyModel".to_string(),
            software_version: "1.0".to_string(),
            model_version: "v1".to_string(),
            serial_code: "SN-123".to_string(),
            certification_level: 0,
            load_equivalency: 1,
        };
        let bytes = p.encode().unwrap();
        assert_eq!(bytes.len(), 134);
        assert_eq!(N2KProductInfo::decode(&bytes).unwrap(), p);
    }

    #[test]
    fn config_info_round_trip() {
        let c = N2KConfigInfo {
            installation_desc1: "Bridge".to_string(),
            installation_desc2: "Cabin".to_string(),
            manufacturer_info: "AcmeCo".to_string(),
        };
        let bytes = c.encode().unwrap();
        assert_eq!(N2KConfigInfo::decode(&bytes).unwrap(), c);
    }

    #[test]
    fn heartbeat_round_trip() {
        let hb = N2KHeartbeat {
            update_interval_ms: 60_000,
            sequence_counter: 5,
            controller_class1: 0xAA,
            controller_class2: 0xBB,
        };
        let bytes = hb.encode().unwrap();
        let d = N2KHeartbeat::decode(&bytes).unwrap();
        assert_eq!(d, hb);
    }

    #[test]
    fn public_encoders_reject_n2k_text_truncation_and_non_ascii() {
        let mut product = N2KProductInfo {
            model_id: "A".repeat(33),
            ..Default::default()
        };
        assert_eq!(product.encode().unwrap_err().code, ErrorCode::InvalidData);

        product.model_id = "Model".to_string();
        product.serial_code = "SN\n001".to_string();
        assert_eq!(product.encode().unwrap_err().code, ErrorCode::InvalidData);

        let config = N2KConfigInfo {
            installation_desc1: "A".repeat(N2K_CONFIG_STRING_MAX_BYTES + 1),
            ..Default::default()
        };
        assert_eq!(config.encode().unwrap_err().code, ErrorCode::InvalidData);

        let config = N2KConfigInfo {
            installation_desc1: "Bridge".to_string(),
            manufacturer_info: "München".to_string(),
            ..Default::default()
        };
        assert_eq!(config.encode().unwrap_err().code, ErrorCode::InvalidData);
    }

    #[test]
    fn heartbeat_encode_rejects_unencodable_interval_and_sequence() {
        let mut hb = N2KHeartbeat {
            update_interval_ms: 51,
            ..Default::default()
        };
        assert_eq!(hb.encode().unwrap_err().code, ErrorCode::InvalidData);

        hb.update_interval_ms = u16::MAX as u32 * 50 + 50;
        assert_eq!(hb.encode().unwrap_err().code, ErrorCode::InvalidData);

        hb.update_interval_ms = 1000;
        hb.sequence_counter = 0x10;
        assert_eq!(hb.encode().unwrap_err().code, ErrorCode::InvalidData);
    }

    #[test]
    fn n2k_management_decoders_reject_wrong_size_or_trailing_payloads() {
        let product = N2KProductInfo::default().encode().unwrap();
        assert!(N2KProductInfo::decode(&product[..133]).is_err());
        assert!(N2KProductInfo::decode(&[product.as_slice(), &[0x00]].concat()).is_err());

        let config = N2KConfigInfo::default().encode().unwrap();
        assert!(N2KConfigInfo::decode(&[config.as_slice(), &[0x00]].concat()).is_err());
        let mut overlong = Vec::from([71u8, 0x00]);
        overlong.extend_from_slice(&[b'A'; 71]);
        overlong.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        assert!(N2KConfigInfo::decode(&overlong).is_err());

        assert!(N2KHeartbeat::decode(&[0xB0, 0x04, 0xF5, 0xAA, 0xBB, 0xFF, 0xFF]).is_none());
        assert!(
            N2KHeartbeat::decode(&[0xB0, 0x04, 0xF5, 0xAA, 0xBB, 0xFF, 0xFF, 0xFF, 0x00]).is_none()
        );
        assert!(N2KHeartbeat::decode(&[0xB0, 0x04, 0x05, 0xAA, 0xBB, 0xFF, 0xFF, 0xFF]).is_none());
        assert!(N2KHeartbeat::decode(&[0xB0, 0x04, 0xF5, 0xAA, 0xBB, 0xFF, 0xFF, 0x00]).is_none());
    }

    #[test]
    fn heartbeat_emitted_at_cadence() {
        let mut m = N2KManagement::new(N2KManagementConfig::default().with_heartbeat_interval(100));
        // Under cadence: nothing.
        let out = m.update(50).unwrap();
        assert!(out.is_empty());
        // Crosses threshold.
        let out = m.update(60).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pgn, PGN_HEARTBEAT_N2K);
    }

    #[test]
    fn heartbeat_long_tick_emits_once_and_preserves_phase() {
        let mut m = N2KManagement::new(N2KManagementConfig::default().with_heartbeat_interval(100));

        let out = m.update(350).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(
            N2KHeartbeat::decode(&out[0].data).unwrap().sequence_counter,
            0
        );

        assert!(m.update(49).unwrap().is_empty());
        let out = m.update(1).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(
            N2KHeartbeat::decode(&out[0].data).unwrap().sequence_counter,
            1
        );
    }

    #[test]
    fn request_then_response_clears_pending() {
        let mut m = N2KManagement::default();
        let frame = m.request_product_info(0x42).unwrap();
        assert_eq!(frame.pgn, PGN_REQUEST);
        assert_eq!(frame.dest, Some(0x42));
        assert!(m.has_pending_request_to(0x42));
        // Response arrives.
        let response = N2KProductInfo::default().encode().unwrap();
        let msg = Message::new(PGN_PRODUCT_INFO, response, 0x42);
        m.handle_message(&msg);
        assert!(!m.has_pending_request_to(0x42));
    }

    #[test]
    fn duplicate_request_to_same_dest_errors() {
        let mut m = N2KManagement::default();
        m.request_product_info(0x42).unwrap();
        assert!(m.request_product_info(0x42).is_err());
    }

    #[test]
    fn pending_request_times_out() {
        let mut m = N2KManagement::default();
        let _ = m.request_product_info(0x42).unwrap();
        // Advance past the timeout.
        let _ = m.update(N2K_REQUEST_TIMEOUT_MS + 10).unwrap();
        assert!(!m.has_pending_request_to(0x42));
    }

    #[test]
    fn config_info_route_decodes_inbound() {
        let mut m = N2KManagement::default();
        use std::cell::RefCell;
        use std::rc::Rc;
        let log: Rc<RefCell<Vec<(N2KConfigInfo, Address)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        m.on_config_info_received
            .subscribe(move |v| lc.borrow_mut().push(v.clone()));
        let info = N2KConfigInfo {
            installation_desc1: "X".to_string(),
            ..Default::default()
        };
        let msg = Message::new(PGN_CONFIG_INFO, info.encode().unwrap(), 0x33);
        m.handle_message(&msg);
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(log.borrow()[0].1, 0x33);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_n2k_management_decoders_accept_or_reject_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=180),
        ) {
            if let Ok(info) = N2KProductInfo::decode(&data) {
                prop_assert_eq!(data.len(), 134);
                prop_assert_eq!(info.encode().unwrap().len(), 134);
            }

            if let Ok(info) = N2KConfigInfo::decode(&data) {
                prop_assert!(N2KConfigInfo::decode(&info.encode().unwrap()).is_ok());
            }

            if let Some(hb) = N2KHeartbeat::decode(&data) {
                let encoded = hb.encode().unwrap();
                prop_assert_eq!(N2KHeartbeat::decode(&encoded), Some(hb));
            }
        }

        #[test]
        fn proptest_n2k_management_handle_message_and_update_are_bounded_for_arbitrary_inputs(
            pgn_index in 0usize..4,
            data in proptest::collection::vec(any::<u8>(), 0..=180),
            source in any::<u8>(),
            elapsed_ms in any::<u32>(),
        ) {
            let pgn = [
                PGN_PRODUCT_INFO,
                PGN_CONFIG_INFO,
                PGN_HEARTBEAT_N2K,
                0x3_FFFF,
            ][pgn_index];
            let mut manager = N2KManagement::default();
            let msg = Message::new(pgn, data, source);

            manager.handle_message(&msg);
            let out = manager.update(elapsed_ms).unwrap();

            prop_assert!(manager.pending_requests().is_empty());
            prop_assert!(out.len() <= 1);
            for frame in out {
                prop_assert_eq!(frame.pgn, PGN_HEARTBEAT_N2K);
                prop_assert_eq!(frame.data.len(), 8);
                prop_assert!(N2KHeartbeat::decode(&frame.data).is_some());
            }
        }
    }
}
