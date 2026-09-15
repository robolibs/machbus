//! ISO 11783-7 §8.3 heartbeat sequencing. Mirrors `heartbeat_demo.cpp`.
//!
//! The sequence counter starts at INIT (0xFE) and then walks
//! `0..=240` cyclically.

use machbus::j1939::heartbeat::hb_seq;
use machbus::j1939::{HeartbeatReceiver, HeartbeatSender, HeartbeatTracker};
use machbus::net::Message;
use machbus::net::pgn_defs::PGN_HEARTBEAT;

fn main() {
    println!("=== Heartbeat Demo ===");

    // Sender side: INIT sentinel then 0..240 cyclically.
    let mut sender = HeartbeatSender::default();
    println!("[sender] INIT sentinel = 0x{:02X}", hb_seq::INIT);
    let walk: Vec<u8> = (0..6).map(|_| sender.next_sequence()).collect();
    println!("[sender] first 6 sequences: {walk:?}");

    // Single-source receiver: track liveness for one peer.
    let mut rx = HeartbeatReceiver::new();
    rx.process(hb_seq::INIT);
    for n in 0..3u8 {
        rx.process(n);
        rx.update(100);
    }
    println!(
        "[receiver] state after 3 normal sequences = {:?}, healthy={}",
        rx.state(),
        rx.is_healthy()
    );

    // Multi-source tracker: monitors several peers by address.
    let mut tracker = HeartbeatTracker::new(100);
    for src in [0x10u8, 0x20] {
        tracker.track(src);
    }
    for tick in 0..3u8 {
        for src in [0x10u8, 0x20] {
            let mut data = [0xFFu8; 8];
            data[0] = tick;
            tracker.handle_message(&Message::new(PGN_HEARTBEAT, data.to_vec(), src));
        }
        tracker.update(100);
    }
    println!("[tracker] after 3 ticks:");
    for src in [0x10u8, 0x20] {
        println!(
            "  source 0x{src:02X} → last_seq={:?}, missed={}",
            tracker.last_sequence(src),
            tracker.missed_count(src)
        );
    }
}
