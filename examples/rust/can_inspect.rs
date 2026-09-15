//! Inspect raw CAN identifiers with machbus — **no ISOBUS, no session**.
//!
//! Every 29-bit extended CAN identifier on a J1939 / ISOBUS / NMEA 2000 bus
//! decomposes into priority + PGN + source + (sometimes) destination. This
//! example uses only [`machbus::net`] to pull those apart, with no protocol
//! stack, no address claim, and no IO.
//!
//! Run with `cargo run --example can_inspect`.

use machbus::net::Identifier;

// ANCHOR: decode
/// Pretty-print everything encoded in one 29-bit identifier.
fn describe(raw: u32) {
    let id = Identifier::from_raw(raw);
    let kind = if id.is_pdu2() {
        "PDU2 (broadcast)"
    } else {
        "PDU1 (peer-to-peer)"
    };
    print!(
        "0x{raw:08X}  prio={}  PGN=0x{:04X} ({:>6})  src=0x{:02X}  ",
        u8::from(id.priority()),
        id.pgn(),
        id.pgn(),
        id.source(),
    );
    if id.is_pdu2() {
        println!("dst=ALL   {kind}");
    } else {
        println!("dst=0x{:02X}  {kind}", id.destination());
    }
}
// ANCHOR_END: decode

fn main() {
    println!("=== Inspecting CAN identifiers (machbus::net only) ===\n");

    // ANCHOR: samples
    // A handful of real-world 29-bit identifiers:
    let samples = [
        0x0CF0_0400, // EEC1 (engine speed) — PDU2 broadcast, priority 3
        0x18FE_E500, // Engine hours       — PDU2 broadcast, priority 6
        0x18EA_FF00, // PGN request to ALL — PDU1, destination 0xFF
        0x18EE_FF80, // Address claimed    — PDU1 broadcast from 0x80
        0x0CFE_6CEE, // Wheel-based speed   — PDU2 broadcast
        0x09F8_0180, // NMEA 2000 rapid position (PGN 129025)
    ];
    for raw in samples {
        describe(raw);
    }
    // ANCHOR_END: samples

    // ANCHOR: pdu
    // The PDU1/PDU2 split is the classic trap: for PDU1 (PF < 240) the low
    // byte of the PGN is actually the *destination address*; for PDU2
    // (PF >= 240) it is part of the PGN and the frame is broadcast.
    let p2p = Identifier::from_raw(0x18EA26EE); // request, sent TO 0x26
    assert!(!p2p.is_pdu2());
    assert_eq!(p2p.destination(), 0x26);

    let bcast = Identifier::from_raw(0x0CF00400); // EEC1, to everyone
    assert!(bcast.is_pdu2());
    assert!(bcast.is_broadcast());
    println!(
        "\nPDU1 example routes to 0x{:02X}; PDU2 example is broadcast.",
        p2p.destination()
    );
    // ANCHOR_END: pdu
}
