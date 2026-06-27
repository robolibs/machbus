//! NIU bridge: forward, filter, rate-limit. Mirrors `niu_demo.cpp`.

use machbus::net::{
    BROADCAST_ADDRESS, Frame, Niu, NiuConfig, NiuFilterMode, Priority, Side,
    pgn_defs::{PGN_DM1, PGN_HEARTBEAT, PGN_REQUEST},
};

fn main() {
    println!("=== NIU Bridge Demo ===");

    let mut niu = Niu::new(
        NiuConfig::default()
            .name("demo-niu")
            .mode(NiuFilterMode::BlockAll),
    );
    niu.allow_pgn(PGN_HEARTBEAT, true);
    niu.allow_pgn_rate_limited(PGN_DM1, 100, true); // 1 frame / 100 ms
    niu.block_pgn(PGN_REQUEST, true);
    niu.start().unwrap();

    println!(
        "[config] mode={:?}, {} filter rules",
        niu.filter_mode(),
        niu.filters().len()
    );

    // 1. Heartbeat (allowed) — forwards.
    let hb = Frame::from_message(
        Priority::Default,
        PGN_HEARTBEAT,
        0x10,
        BROADCAST_ADDRESS,
        &[1; 8],
    );
    println!(
        "[hb ] tractor → forwarded? {}",
        niu.process_frame(hb, Side::Tractor, 0).is_some()
    );

    // 2. Request PGN (explicitly blocked).
    let req = Frame::from_message(Priority::Default, PGN_REQUEST, 0x10, 0x42, &[0; 3]);
    println!(
        "[req] tractor → forwarded? {}",
        niu.process_frame(req, Side::Tractor, 0).is_some()
    );

    // 3. DM1 with 100 ms rate limit.
    let dm1 = Frame::from_message(Priority::Default, PGN_DM1, 0x10, BROADCAST_ADDRESS, &[0; 8]);
    let r1 = niu.process_frame(dm1, Side::Tractor, 50);
    let r2 = niu.process_frame(dm1, Side::Tractor, 100);
    let r3 = niu.process_frame(dm1, Side::Tractor, 200);
    println!("[dm1] t=50  → {} (first allowed)", r1.is_some());
    println!("[dm1] t=100 → {} (within window)", r2.is_some());
    println!("[dm1] t=200 → {} (window elapsed)", r3.is_some());

    println!(
        "\n[stats] forwarded={}, blocked={}",
        niu.forwarded(),
        niu.blocked()
    );
}
