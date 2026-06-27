//! Walk through the primitives: PGN helpers, Identifier encode/decode,
//! Frame round-trip, NAME arbitration. Mirrors `types_demo.cpp`.

use machbus::net::{
    BROADCAST_ADDRESS, Frame, Identifier, NULL_ADDRESS, Name, Priority, pgn,
    pgn_defs::{PGN_DM1, PGN_HEARTBEAT, PGN_REQUEST},
};

fn main() {
    println!("=== Types Demo ===\n");

    // ─── PGN helpers ─────────────────────────────────────────────────
    println!("[pgn]");
    println!("  PGN_REQUEST       = 0x{PGN_REQUEST:04X}  (PDU1: dest-specific)");
    println!("  PGN_DM1           = 0x{PGN_DM1:04X}  (PDU2: broadcast)");
    println!(
        "  pdu_format(REQUEST) = 0x{:02X}",
        pgn::pgn_pdu_format(PGN_REQUEST)
    );
    println!("  is_pdu2(REQUEST)    = {}", pgn::pgn_is_pdu2(PGN_REQUEST));
    println!("  is_pdu2(DM1)        = {}", pgn::pgn_is_pdu2(PGN_DM1));
    if let Some(info) = pgn::pgn_lookup(PGN_HEARTBEAT) {
        println!(
            "  lookup(HEARTBEAT)   = name={:?}, len={}, broadcast={}",
            info.name, info.data_length, info.is_broadcast,
        );
    }

    // ─── Identifier encode/decode ───────────────────────────────────
    println!("\n[identifier]");
    let id = Identifier::encode(Priority::High, PGN_REQUEST, 0x80, 0x42);
    println!("  encode(REQUEST, 0x80→0x42, High) -> raw=0x{:08X}", id.raw);
    println!(
        "  decode: prio={:?}, pgn=0x{:04X}, src=0x{:02X}, dst=0x{:02X}",
        id.priority(),
        id.pgn(),
        id.source(),
        id.destination()
    );
    let id2 = Identifier::encode(Priority::Default, PGN_DM1, 0x80, 0x42);
    println!(
        "  PDU2 (DM1): destination forced to 0x{:02X} (broadcast)",
        id2.destination()
    );

    // ─── Frame round-trip via wirebit::CanFrame ─────────────────────
    println!("\n[frame]");
    let frame = Frame::from_message(
        Priority::High,
        PGN_HEARTBEAT,
        0x80,
        BROADCAST_ADDRESS,
        &[0xDE, 0xAD, 0xBE, 0xEF],
    );
    println!("  payload    = {:02X?}", frame.payload());
    println!("  length     = {}", frame.length);
    let cf = frame.to_can_frame();
    println!(
        "  CAN id     = 0x{:08X} (ext={})",
        cf.id(),
        cf.is_extended()
    );
    let restored = Frame::from_can_frame(&cf).unwrap();
    println!(
        "  decoded    = pgn=0x{:04X}, src=0x{:02X}",
        restored.pgn(),
        restored.source()
    );

    // ─── NAME arbitration ───────────────────────────────────────────
    println!("\n[name]");
    let n_low = Name::default()
        .with_identity_number(0x12345)
        .with_manufacturer_code(0x100);
    let n_high = Name::default()
        .with_identity_number(0x99999)
        .with_manufacturer_code(0x500);
    println!("  low.raw  = 0x{:016X}", n_low.raw);
    println!("  high.raw = 0x{:016X}", n_high.raw);
    println!("  low <  high? {}  (lower NAME wins)", n_low < n_high);

    println!("\n[constants]");
    println!("  NULL_ADDRESS      = 0x{NULL_ADDRESS:02X}");
    println!("  BROADCAST_ADDRESS = 0x{BROADCAST_ADDRESS:02X}");
}
