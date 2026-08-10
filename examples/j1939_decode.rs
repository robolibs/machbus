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

use machbus::isobus::implement::Signal;
use machbus::j1939::{DiagnosticLamps, DmDtcList, Dtc, Eec1, Fmi};
use machbus::net::Message;

fn main() {
    println!("=== Decoding J1939 PGNs (machbus::j1939 only) ===\n");

    // ANCHOR: eec1
    // EEC1 (PGN 61444) — engine speed, torque, driver demand.
    // Each parameter is a `Signal`: a value, or the engine reporting it as
    // faulted / not provided. One absent parameter no longer costs the whole PG.
    let eec1 = Eec1 {
        engine_speed_rpm: Signal::Value(1500.0),
        driver_demand_percent: Signal::Value(40.0),
        actual_engine_percent: Signal::Value(38.0),
        engine_torque_percent: Signal::Value(35.0),
        starter_mode: 0,
        source_address: 0x00,
    };
    let bytes = eec1.encode(); // [u8; 8] you would put on the bus
    let back = Eec1::decode(&bytes).expect("valid EEC1 payload");
    println!(
        "EEC1: {:?} rpm, driver demand {:?}%, actual {:?}%",
        back.engine_speed_rpm.value(),
        back.driver_demand_percent.value(),
        back.actual_engine_percent.value()
    );
    // ANCHOR_END: eec1

    // ANCHOR: from_message
    // If you already assembled a `Message` (pgn + data + source), decode from it:
    let msg = Message::new(61444, bytes.to_vec(), 0x00);
    if let Some(e) = Eec1::from_message(&msg) {
        println!(
            "from_message: {:?} rpm from source 0x{:02X}",
            e.engine_speed_rpm.value(),
            msg.source
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
                conversion_method: false,
            }, // coolant temp high
            Dtc {
                spn: 190,
                fmi: Fmi::BelowNormal,
                occurrence_count: 1,
                conversion_method: false,
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
