//! Parse synthetic NMEA-0183 sentences via [`SerialGNSS::feed_bytes`].
//! Mirrors `serial_gnss.cpp` but without an actual UART — feed raw
//! bytes from any source.

use std::cell::RefCell;
use std::rc::Rc;

use machbus::nmea::{GNSSPosition, SerialGNSS};

fn checksum(s: &str) -> u8 {
    s.as_bytes().iter().fold(0u8, |acc, &b| acc ^ b)
}

fn with_checksum(body: &str) -> String {
    // Skip the leading '$' for the XOR.
    let cs = checksum(&body[1..]);
    format!("{body}*{:02X}\n", cs)
}

fn main() {
    println!("=== Serial GNSS (NMEA-0183) Demo ===");

    let mut parser = SerialGNSS::new();
    let positions: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
    let p = positions.clone();
    parser
        .on_position
        .subscribe(move |&pos| p.borrow_mut().push(pos));
    parser
        .on_cog
        .subscribe(|c| println!("  on_cog → {:.3} rad", c));
    parser
        .on_sog
        .subscribe(|s| println!("  on_sog → {:.2} m/s", s));

    // GGA: position fix.
    let gga = with_checksum("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,");
    parser.feed_bytes(gga.as_bytes());

    // RMC: speed + course.
    let rmc = with_checksum("$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W");
    parser.feed_bytes(rmc.as_bytes());

    // GSA: 3D fix DOPs.
    let gsa = with_checksum("$GPGSA,A,3,04,05,,09,12,,,24,,,,,2.5,1.3,2.1");
    parser.feed_bytes(gsa.as_bytes());

    println!(
        "\n[summary] {} position events parsed",
        positions.borrow().len()
    );
    let last = parser.latest_position().unwrap();
    println!(
        "  lat={:.6}°N  lon={:.6}°E  alt={:.1}m",
        last.wgs.latitude, last.wgs.longitude, last.wgs.altitude
    );
    println!(
        "  fix={:?}  sats={}  hdop={:?}  pdop={:?}",
        last.fix_type, last.satellites_used, last.hdop, last.pdop
    );
    println!("  cog={:?} rad  sog={:?} m/s", last.cog_rad, last.speed_mps);

    // Demonstrate split-buffer parsing (UART chunk boundaries).
    println!("\n[chunked feed] split GGA across 3 reads:");
    let mut p2 = SerialGNSS::new();
    let body = with_checksum("$GPGGA,124519,4807.500,N,01132.000,E,1,06,1.2,540.0,M,46.9,M,,");
    let bytes = body.as_bytes();
    p2.feed_bytes(&bytes[..30]);
    p2.feed_bytes(&bytes[30..50]);
    p2.feed_bytes(&bytes[50..]);
    let last = p2.latest_position().unwrap();
    println!(
        "  lat={:.6}, lon={:.6} ({} sats)",
        last.wgs.latitude, last.wgs.longitude, last.satellites_used
    );
}
