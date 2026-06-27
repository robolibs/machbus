//! Engine + powertrain message round-trips: EEC1 (RPM/torque),
//! EngineTemp1, FuelEconomy, EngineHours. Mirrors
//! `engine_powertrain_demo.cpp`.

use machbus::j1939::{Eec1, EngineHours, EngineTemp1, FuelEconomy};

fn main() {
    println!("=== Engine Powertrain Demo ===");

    // EEC1 — Electronic Engine Controller 1.
    let eec1 = Eec1 {
        engine_torque_percent: 50.0,
        driver_demand_percent: 75.0,
        actual_engine_percent: 45.0,
        engine_speed_rpm: 1500.0,
        starter_mode: 1,
        source_address: 0x00,
    };
    let bytes = eec1.encode();
    let d = Eec1::decode(&bytes).unwrap();
    println!(
        "[EEC1]  rpm={:.1}, torque={:.1}%, driver_demand={:.1}%",
        d.engine_speed_rpm, d.engine_torque_percent, d.driver_demand_percent
    );

    // Engine temperature.
    let temp = EngineTemp1 {
        coolant_temp_c: 88.0,
        fuel_temp_c: 32.0,
        oil_temp_c: 92.0,
        ..Default::default()
    };
    let tbytes = temp.encode();
    let td = EngineTemp1::decode(&tbytes).unwrap();
    println!(
        "[ET1]   coolant={:.1}°C, fuel={:.1}°C, oil={:.1}°C",
        td.coolant_temp_c, td.fuel_temp_c, td.oil_temp_c
    );

    // Fuel economy.
    let fe = FuelEconomy {
        fuel_rate_lph: 7.25,
        instantaneous_lph: 4.8,
        throttle_position: 35.0,
    };
    let fbytes = fe.encode();
    let fd = FuelEconomy::decode(&fbytes).unwrap();
    println!(
        "[LFE]   rate={:.2} L/h, inst={:.1} L/h, throttle={:.1}%",
        fd.fuel_rate_lph, fd.instantaneous_lph, fd.throttle_position
    );

    // Engine hours.
    let hours = EngineHours {
        total_hours: 1234.75,
        total_revolutions: 1_000_000.0,
    };
    let hbytes = hours.encode();
    let hd = EngineHours::decode(&hbytes).unwrap();
    println!(
        "[Hours] total={:.2} h, revs={:.0}",
        hd.total_hours, hd.total_revolutions
    );
}
