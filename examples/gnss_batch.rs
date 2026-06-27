//! Batch-convert WGS84 positions to local ENU/NED/ECEF via machbus' geo seam.
//! Mirrors `gnss_batch.cpp`.

use machbus::geo::{Geo, Wgs};
use machbus::nmea::{GNSSBatch, GNSSPosition};

fn main() {
    println!("=== GNSS Batch Conversion Demo ===");

    let reference = Geo::new(52.0, 4.0, 0.0);
    println!(
        "[reference origin] lat={}, lon={}, alt={}",
        reference.latitude, reference.longitude, reference.altitude
    );

    // 5-point trajectory centered on the reference.
    let mut batch = GNSSBatch::default();
    for i in 0..5 {
        let dlat = (i as f64) * 1e-5;
        let dlon = (i as f64) * 2e-5;
        batch.positions.push(GNSSPosition {
            wgs: Wgs::new(reference.latitude + dlat, reference.longitude + dlon, 0.0),
            ..Default::default()
        });
    }
    println!("[input] {} WGS samples", batch.positions.len());

    let enu = batch.to_enu_batch(reference);
    println!("\n[ENU]");
    for (i, p) in enu.iter().enumerate() {
        println!(
            "  [{i}] east={:>9.3} m, north={:>9.3} m, up={:>9.3} m",
            p.x(),
            p.y(),
            p.z()
        );
    }

    let ned = batch.to_ned_batch(reference);
    println!("\n[NED]");
    for (i, p) in ned.iter().enumerate() {
        println!(
            "  [{i}] north={:>9.3} m, east={:>9.3} m, down={:>9.3} m",
            p.x(),
            p.y(),
            p.z()
        );
    }

    let ecf = batch.to_ecf_batch();
    println!("\n[ECEF]");
    for (i, p) in ecf.iter().enumerate() {
        let (x, y, z) = (p.x, p.y, p.z);
        println!(
            "  [{i}] x={:>15.3} m, y={:>15.3} m, z={:>15.3} m  |r|={:.0} m",
            x,
            y,
            z,
            (x * x + y * y + z * z).sqrt()
        );
    }
}
