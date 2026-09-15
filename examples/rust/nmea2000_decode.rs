//! Decode NMEA 2000 navigation PGNs with machbus — **no ISOBUS, no session**.
//!
//! [`machbus::nmea::NMEAInterface`] is a pump-style decoder: you subscribe to
//! the `on_*` events for the PGNs you care about, then feed it [`Message`]s.
//! It needs no protocol stack and no IO. Here we encode a fix and a
//! course/speed update with the matching `build_*` helpers, then feed the
//! bytes back in so the decode path actually fires.
//!
//! Run with `cargo run --example nmea2000_decode`.

use machbus::geo::Wgs;
use machbus::net::Message;
use machbus::net::pgn_defs::{PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_POSITION_RAPID};
use machbus::nmea::{GNSSPosition, NMEAConfig, NMEAInterface};

fn main() {
    println!("=== Decoding NMEA 2000 navigation (machbus::nmea only) ===\n");

    // ANCHOR: setup
    // A decoder listening for the GNSS navigation profile (rapid position,
    // COG/SOG, heading, attitude, …). `NMEAConfig` chooses which PGNs to decode.
    let mut nmea = NMEAInterface::new(NMEAConfig::default().with_gnss_navigation(true));

    // Subscribe to decoded fixes. Handlers are `FnMut(&T)`.
    nmea.on_position.subscribe(|pos: &GNSSPosition| {
        println!(
            "position: {:.6}, {:.6}  (sats {}, fix {:?})",
            pos.wgs.latitude, pos.wgs.longitude, pos.satellites_used, pos.fix_type
        );
    });
    nmea.on_cog.subscribe(|cog_rad: &f64| {
        println!("course over ground: {:.1}°", cog_rad.to_degrees());
    });
    nmea.on_sog.subscribe(|sog_mps: &f64| {
        println!("speed over ground: {:.1} km/h", sog_mps * 3.6);
    });
    // ANCHOR_END: setup

    // ANCHOR: feed
    // Build a rapid-position payload (PGN 129025) and feed it as a Message.
    let fix = GNSSPosition {
        wgs: Wgs::new(52.379_189, 4.899_431, 0.0), // Amsterdam
        satellites_used: 12,
        ..Default::default()
    };
    let pos_bytes = NMEAInterface::build_position(&fix);
    nmea.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        pos_bytes.to_vec(),
        0x80,
    ));

    // Build a course/speed payload (PGN 129026): heading ~60°, 5 m/s.
    let cog_bytes = NMEAInterface::build_cog_sog(60.0_f64.to_radians(), 5.0);
    nmea.handle_message(&Message::new(
        PGN_GNSS_COG_SOG_RAPID,
        cog_bytes.to_vec(),
        0x80,
    ));
    // ANCHOR_END: feed
}
