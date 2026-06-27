//! Protocol-wide constants: addresses, timing, payload limits.
//!
//! Mirrors the C++ `machbus::net::constants`. Values come from
//! ISO 11783 and J1939 specifications and **must remain bit-identical**
//! to interoperate with real ECUs and the C++ stack.

use super::types::Address;

// ─── Address constants (ISO 11783-5) ─────────────────────────────────────
/// Reserved "no address claimed" / null source address.
pub const NULL_ADDRESS: Address = 0xFE;
/// Global broadcast destination address.
pub const BROADCAST_ADDRESS: Address = 0xFF;
/// Highest valid claimable address (`0..=0xFD`).
pub const MAX_ADDRESS: Address = 0xFD;

// ─── Timing constants (milliseconds) ─────────────────────────────────────
/// Initial delay before claiming an address (ISO 11783-5).
pub const ADDRESS_CLAIM_TIMEOUT_MS: u32 = 250;

/// Maximum random transmit delay after losing arbitration: `0.6ms × 255`
/// (ISO 11783-5 §3.4).
pub const ADDRESS_CLAIM_RTXD_MAX_MS: u32 = 153;

/// Standard heartbeat interval.
pub const HEARTBEAT_INTERVAL_MS: u32 = 100;

/// Transport response timeout (ISO 11783-3 §5.13.3).
pub const TP_TIMEOUT_TR_MS: u32 = 200;
pub const TP_TIMEOUT_T1_MS: u32 = 750;
pub const TP_TIMEOUT_T2_MS: u32 = 1250;
pub const TP_TIMEOUT_T3_MS: u32 = 1250;
pub const TP_TIMEOUT_T4_MS: u32 = 1050;
pub const ETP_TIMEOUT_T1_MS: u32 = 750;

/// Minimum spacing between BAM data-transfer packets (J1939-21).
pub const TP_BAM_INTER_PACKET_MS: u32 = 50;

// ─── Power management constants (ISO 11783-9 §4.6) ──────────────────────
/// Minimum delay after key-off before a tractor ECU may shut down.
pub const POWER_SHUTDOWN_MIN_MS: u32 = 2_000;
/// Cadence for repeating maintain-power messages.
pub const POWER_MAINTAIN_REPEAT_MS: u32 = 1_000;
/// Maximum total power-extension window (3 minutes).
pub const POWER_MAX_EXTENSION_MS: u32 = 180_000;

// ─── Protocol limits ─────────────────────────────────────────────────────
/// CAN data length code maximum (classic CAN payload size).
pub const CAN_DATA_LENGTH: u32 = 8;

/// Largest payload deliverable via TP (ISO 11783-3 / J1939-21).
pub const TP_MAX_DATA_LENGTH: u32 = 1_785;

/// Largest payload deliverable via ETP.
pub const ETP_MAX_DATA_LENGTH: u32 = 117_440_505;

/// Bytes of payload per TP/ETP data-transfer frame (1 sequence byte + 7).
pub const TP_BYTES_PER_FRAME: u32 = 7;

/// Maximum packets a TP CTS may grant in a single round.
pub const TP_MAX_PACKETS_PER_CTS: u32 = 16;

/// Largest payload deliverable via NMEA2000 fast packet.
pub const FAST_PACKET_MAX_DATA: u32 = 223;

// Compile-time invariants: single-frame ≤ 8 < TP ≤ 1785 < ETP.
const _: () = assert!(CAN_DATA_LENGTH < TP_MAX_DATA_LENGTH);
const _: () = assert!(TP_MAX_DATA_LENGTH < ETP_MAX_DATA_LENGTH);
const _: () = assert!(TP_BYTES_PER_FRAME == 7);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_constants_match_iso_11783_5() {
        assert_eq!(NULL_ADDRESS, 0xFE);
        assert_eq!(BROADCAST_ADDRESS, 0xFF);
        assert_eq!(MAX_ADDRESS, 0xFD);
    }
}
