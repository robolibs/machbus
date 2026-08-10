//! Engine + powertrain message round-trips: EEC1 (RPM/torque),
//! EngineTemp1, FuelEconomy, EngineHours. Mirrors
//! `engine_powertrain_demo.cpp`.

use machbus::isobus::implement::Signal;
use machbus::j1939::{Eec1, EngineHours, EngineTemp1, FuelEconomy};

fn main() {
    println!("=== Engine Powertrain Demo ===");

    // EEC1 — Electronic Engine Controller 1.
    let eec1 = Eec1 {
        engine_torque_percent: Signal::Value(50.0),
        driver_demand_percent: Signal::Value(75.0),
        actual_engine_percent: Signal::Value(45.0),
        engine_speed_rpm: Signal::Value(1500.0),
        starter_mode: 1,
        source_address: 0x00,
    };
    let bytes = eec1.encode();
    let d = Eec1::decode(&bytes).unwrap();
    println!(
        "[EEC1]  rpm={:?}, torque={:?}%, driver_demand={:?}%",
        d.engine_speed_rpm.value(),
        d.engine_torque_percent.value(),
        d.driver_demand_percent.value()
    );

    // Engine temperature.
    // The turbo and intercooler sensors are left unreported, which is what an
    // engine without them actually puts on the wire.
    let temp = EngineTemp1 {
        coolant_temp_c: Signal::Value(88.0),
        fuel_temp_c: Signal::Value(32.0),
        oil_temp_c: Signal::Value(92.0),
        ..Default::default()
    };
    let tbytes = temp.encode();
    let td = EngineTemp1::decode(&tbytes).unwrap();
    println!(
        "[ET1]   coolant={:?}°C, fuel={:?}°C, oil={:?}°C, turbo oil={:?}",
        td.coolant_temp_c.value(),
        td.fuel_temp_c.value(),
        td.oil_temp_c.value(),
        td.turbo_oil_temp_c
    );

    // Fuel economy.
    let fe = FuelEconomy {
        fuel_rate_lph: Signal::Value(7.25),
        instantaneous_lph: Signal::Value(4.8),
        throttle_position: Signal::Value(35.0),
    };
    let fbytes = fe.encode();
    let fd = FuelEconomy::decode(&fbytes).unwrap();
    println!(
        "[LFE]   rate={:?} L/h, inst={:?} L/h, throttle={:?}%",
        fd.fuel_rate_lph.value(),
        fd.instantaneous_lph.value(),
        fd.throttle_position.value()
    );

    // Engine hours.
    let hours = EngineHours {
        total_hours: Signal::Value(1234.75),
        total_revolutions: Signal::Value(1_000_000.0),
    };
    let hbytes = hours.encode();
    let hd = EngineHours::decode(&hbytes).unwrap();
    println!(
        "[Hours] total={:?} h, revs={:?}",
        hd.total_hours.value(),
        hd.total_revolutions.value()
    );
}
