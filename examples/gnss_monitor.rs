//! Subscribe to inbound NMEA 2000 GNSS PGNs and watch the position
//! cache fill in. Mirrors `gnss_monitor.cpp`.

use std::cell::RefCell;
use std::rc::Rc;

use machbus::geo::Wgs;
use machbus::net::Message;
use machbus::net::pgn_defs::{PGN_ATTITUDE, PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_POSITION_RAPID};
use machbus::nmea::{GNSSPosition, NMEAConfig, NMEAInterface};

fn main() {
    println!("=== NMEA 2000 GNSS Monitor ===");

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
    let p = positions.clone();
    iface
        .on_position
        .subscribe(move |&pos| p.borrow_mut().push(pos));
    iface
        .on_cog
        .subscribe(|cog| println!("  COG event: {cog:.3} rad"));
    iface
        .on_sog
        .subscribe(|sog| println!("  SOG event: {sog:.2} m/s"));
    iface.on_attitude.subscribe(|&(yaw, pitch, roll)| {
        println!("  Attitude: yaw={yaw:.3} pitch={pitch:.3} roll={roll:.3}")
    });

    // ─── 1. Position rapid (PGN 129025) ───────────────────────────
    let pos = GNSSPosition {
        wgs: Wgs::new(52.0123, 4.5678, 0.0),
        ..Default::default()
    };
    let bytes = NMEAInterface::build_position(&pos);
    iface.handle_message(&Message::new(PGN_GNSS_POSITION_RAPID, bytes.to_vec(), 0x80));

    // ─── 2. COG/SOG rapid (PGN 129026) ────────────────────────────
    let bytes = NMEAInterface::build_cog_sog(std::f64::consts::FRAC_PI_4, 12.5);
    iface.handle_message(&Message::new(PGN_GNSS_COG_SOG_RAPID, bytes.to_vec(), 0x80));

    // ─── 3. Attitude (PGN 127257) — 1 byte SID + 3×i16 angles ─────
    let mut attitude = [0xFFu8; 8];
    attitude[0] = 0x00; // SID
    attitude[1..3].copy_from_slice(&((std::f64::consts::FRAC_PI_2 / 0.0001) as i16).to_le_bytes());
    attitude[3..5].copy_from_slice(&((0.0524 / 0.0001) as i16).to_le_bytes());
    attitude[5..7].copy_from_slice(&((-0.0349 / 0.0001) as i16).to_le_bytes());
    iface.handle_message(&Message::new(PGN_ATTITUDE, attitude.to_vec(), 0x80));

    // ─── Final cached position ───────────────────────────────────
    let last = iface.latest_position().unwrap();
    println!(
        "\n[cache] position events received: {}",
        positions.borrow().len()
    );
    println!(
        "  lat={:.6}, lon={:.6}, cog={:?} rad, sog={:?} m/s",
        last.wgs.latitude, last.wgs.longitude, last.cog_rad, last.speed_mps
    );
    println!(
        "  heading={:?} rad, pitch={:?} rad, roll={:?} rad",
        last.heading_rad, last.pitch_rad, last.roll_rad
    );
}
