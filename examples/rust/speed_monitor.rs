//! Wheel-based + ground-based speed monitor. Mirrors `speed_monitor.cpp`.

use machbus::isobus::implement::{
    GroundBasedSpeedDist, MachineDirection, WheelBasedSpeedDist, wheel_slip_percent,
};
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
        speed_mps: 5.5.into(),
        distance_m: 12_345.0.into(),
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
        dec.speed_mps.unwrap_or(0.0),
        dec.distance_m.unwrap_or(0.0),
        dec.direction
    );

    let ground = GroundBasedSpeedDist {
        speed_mps: 5.4.into(),
        distance_m: 12_300.0.into(),
        direction: MachineDirection::Forward,
    };
    let gbytes = ground.encode();
    let gd = GroundBasedSpeedDist::decode(&gbytes).unwrap();
    println!(
        "[GS]   {:.2} m/s, {:.0} m total, dir={:?}",
        gd.speed_mps.unwrap_or(0.0),
        gd.distance_m.unwrap_or(0.0),
        gd.direction
    );

    // Slip is only meaningful when both sources actually measured something.
    match (dec.speed_mps.value(), gd.speed_mps.value()) {
        (Some(wheel_mps), Some(ground_mps)) => match wheel_slip_percent(wheel_mps, ground_mps) {
            Some(slip) => println!("\n[derived] estimated wheel slip = {slip:.2}%"),
            None => println!("\n[derived] wheel slip undefined while stationary"),
        },
        _ => println!("\n[derived] wheel slip unavailable: a speed source is not reporting"),
    }
}
