//! Wheel-based + ground-based speed monitor. Mirrors `speed_monitor.cpp`.

use machbus::isobus::implement::{GroundBasedSpeedDist, MachineDirection, WheelBasedSpeedDist};
use machbus::j1939::SpeedAndDistance;

fn main() {
    println!("=== Speed Monitor Demo ===");

    // J1939 wheel speed (PGN 0xFE6E).
    let speed = SpeedAndDistance {
        speed_mps: Some(12.7),
        distance_m: Some(1234.5),
        ..Default::default()
    };
    let _ = speed.encode();
    println!(
        "[J1939 SAD] speed={:.2} m/s, distance={:.0} m",
        speed.speed_mps.unwrap_or(0.0),
        speed.distance_m.unwrap_or(0.0),
    );

    // ISO 11783-7 wheel + ground speed (encode + decode round-trip).
    let wheel = WheelBasedSpeedDist {
        speed_mps: 5.5,
        distance_m: 12_345.0,
        direction: MachineDirection::Forward,
        max_power_time_min: 120,
        key_switch_state: 1,
        implement_start_stop_operations_state: 1,
        operator_direction_reversed_state: 0,
    };
    let bytes = wheel.encode();
    let dec = WheelBasedSpeedDist::decode(&bytes).unwrap();
    println!(
        "[WS]   {:.2} m/s, {:.0} m total, dir={:?}",
        dec.speed_mps, dec.distance_m, dec.direction
    );

    let ground = GroundBasedSpeedDist {
        speed_mps: 5.4,
        distance_m: 12_300.0,
        direction: MachineDirection::Forward,
    };
    let gbytes = ground.encode();
    let gd = GroundBasedSpeedDist::decode(&gbytes).unwrap();
    println!(
        "[GS]   {:.2} m/s, {:.0} m total, dir={:?}",
        gd.speed_mps, gd.distance_m, gd.direction
    );

    let slip = (dec.speed_mps - gd.speed_mps) / dec.speed_mps * 100.0;
    println!("\n[derived] estimated wheel slip = {slip:.2}%");
}
