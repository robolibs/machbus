//! Decode J1939 PGNs with machbus — **no ISOBUS, no session**.
//!
//! machbus ships the J1939 wire codecs as plain `decode`/`encode` functions in
//! [`machbus::j1939`]. You can use them on their own to turn an 8-byte CAN
//! payload into a typed struct (or the reverse) without any protocol stack.
//!
//! Each block below *encodes* a struct and then *decodes* the bytes back, so
//! the output is self-validating — but in a real reader you would feed the
//! `data` bytes straight off the bus.
//!
//! Run with `cargo run --example j1939_decode`.

use machbus::j1939::{DiagnosticLamps, DmDtcList, Dtc, Eec1, Fmi};
use machbus::net::Message;

fn main() {
    println!("=== Decoding J1939 PGNs (machbus::j1939 only) ===\n");

    // ANCHOR: eec1
    // EEC1 (PGN 61444) — engine speed, torque, driver demand.
    let eec1 = Eec1 {
        engine_speed_rpm: 1500.0,
        driver_demand_percent: 40.0,
        actual_engine_percent: 38.0,
        engine_torque_percent: 35.0,
        starter_mode: 0,
        source_address: 0x00,
    };
    let bytes = eec1.encode(); // [u8; 8] you would put on the bus
    let back = Eec1::decode(&bytes).expect("valid EEC1 payload");
    println!(
        "EEC1: {:.0} rpm, driver demand {:.0}%, actual {:.0}%",
        back.engine_speed_rpm, back.driver_demand_percent, back.actual_engine_percent
    );
    // ANCHOR_END: eec1

    // ANCHOR: from_message
    // If you already assembled a `Message` (pgn + data + source), decode from it:
    let msg = Message::new(61444, bytes.to_vec(), 0x00);
    if let Some(e) = Eec1::from_message(&msg) {
        println!(
            "from_message: {:.0} rpm from source 0x{:02X}",
            e.engine_speed_rpm, msg.source
        );
    }
    // ANCHOR_END: from_message

    // ANCHOR: dm1
    // DM1 (active diagnostic trouble codes) — the J1939 "DM" family.
    let dm1 = DmDtcList {
        lamps: DiagnosticLamps::default(),
        dtcs: vec![
            Dtc {
                spn: 110,
                fmi: Fmi::AboveNormalModerate,
                occurrence_count: 3,
            }, // coolant temp high
            Dtc {
                spn: 190,
                fmi: Fmi::BelowNormal,
                occurrence_count: 1,
            }, // engine speed low
        ],
    };
    let payload = dm1.encode(); // variable length (TP if > 8 bytes)
    let decoded = DmDtcList::decode(&payload).expect("valid DM1 payload");
    println!("\nDM1: {} active fault(s):", decoded.dtcs.len());
    for dtc in &decoded.dtcs {
        println!(
            "  SPN {:<5} FMI {:?} (count {})",
            dtc.spn, dtc.fmi, dtc.occurrence_count
        );
    }
    // ANCHOR_END: dm1
}
