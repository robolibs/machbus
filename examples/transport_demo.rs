//! TP CMDT + ETP + Fast Packet round-trip demo. Mirrors
//! `transport_demo.cpp`.

use std::cell::RefCell;
use std::rc::Rc;

use machbus::net::{
    ExtendedTransportProtocol, FastPacketProtocol, Priority, TransportProtocol, TransportSession,
    pgn_defs::PGN_GNSS_POSITION,
};

fn main() {
    println!("=== Transport Protocol Demo ===");

    // ─── TP CMDT (40 bytes, dest-specific) ──────────────────────────
    println!("\n[TP CMDT]");
    {
        let payload: Vec<u8> = (0..40u32).map(|n| n as u8).collect();
        let mut tx = TransportProtocol::new();
        let mut rx = TransportProtocol::new();
        let received = Rc::new(RefCell::new(None::<TransportSession>));
        let r = received.clone();
        rx.on_complete
            .subscribe(move |s| *r.borrow_mut() = Some(s.clone()));

        let rts = tx
            .send(0xEF00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        println!(
            "  RTS for {} bytes (≈{} packets)",
            payload.len(),
            payload.len().div_ceil(7)
        );
        let cts = rx.process_frame(&rts, 0)[0];
        println!("  CTS num={} next_seq={}", cts.data[1], cts.data[2]);

        let _ = tx.process_frame(&cts, 0);
        let dt_frames = tx.get_pending_data_frames();
        println!("  TX queued {} DT frames", dt_frames.len());

        let mut eoma_seen = None;
        for dt in &dt_frames {
            for resp in rx.process_frame(dt, 0) {
                if resp.data[0] == machbus::net::tp::tp_cm::EOMA {
                    eoma_seen = Some(resp);
                }
            }
        }
        let _ = tx.process_frame(&eoma_seen.unwrap(), 0);
        let got = received.borrow().clone().unwrap();
        println!(
            "  delivered {} bytes (equal? {})",
            got.data.len(),
            got.data == payload
        );
    }

    // ─── ETP (2.5 KiB) ──────────────────────────────────────────────
    println!("\n[ETP]");
    {
        let payload: Vec<u8> = (0..2500u32).map(|n| (n & 0xFF) as u8).collect();
        let mut tx = ExtendedTransportProtocol::new();
        let mut rx = ExtendedTransportProtocol::new();
        let received = Rc::new(RefCell::new(None::<TransportSession>));
        let r = received.clone();
        rx.on_complete
            .subscribe(move |s| *r.borrow_mut() = Some(s.clone()));

        let rts = tx
            .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        let mut to_tx = rx.process_frame(&rts, 0);
        let mut turns = 0;
        for turn in 0..50 {
            for f in to_tx.drain(..) {
                let _ = tx.process_frame(&f, 0);
            }
            let dt = tx.get_pending_data_frames();
            if dt.is_empty() {
                break;
            }
            for f in &dt {
                to_tx.extend(rx.process_frame(f, 0));
            }
            turns = turn + 1;
            if received.borrow().is_some() {
                break;
            }
        }
        let got = received.borrow().clone().unwrap();
        println!(
            "  converged in {turns} turns, delivered {} bytes (equal? {})",
            got.data.len(),
            got.data == payload
        );
    }

    // ─── Fast Packet ────────────────────────────────────────────────
    println!("\n[Fast Packet]");
    {
        let payload: Vec<u8> = (0..30u32).map(|n| (n + 0x40) as u8).collect();
        let mut tx = FastPacketProtocol::new();
        let mut rx = FastPacketProtocol::new();
        let frames = tx.send(PGN_GNSS_POSITION, &payload, 0x10).unwrap();
        println!("  {} frames for {} bytes", frames.len(), payload.len());

        let mut got = None;
        for f in &frames {
            if let Some(m) = rx.process_frame(f) {
                got = Some(m);
            }
        }
        let msg = got.unwrap();
        println!(
            "  reassembled {} bytes (equal? {})",
            msg.data.len(),
            msg.data == payload
        );
    }
}
