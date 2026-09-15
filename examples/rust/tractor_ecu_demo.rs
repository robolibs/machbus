//! TECU classification + power-state machine. Mirrors
//! `tractor_ecu_demo.cpp`.

use machbus::isobus::{PowerState, TecuClass, TecuClassification};

fn main() {
    println!("=== Tractor ECU Demo ===");

    // ─── Classification (Class 2NF) ────────────────────────────────
    let cls = TecuClassification {
        base_class: TecuClass::Class2,
        navigation: true,
        front_mounted: true,
        ..Default::default()
    };
    println!("[classification] {cls}");

    // ─── Class 1, 2, 3 string forms ────────────────────────────────
    for c in [TecuClass::Class1, TecuClass::Class2, TecuClass::Class3] {
        let inner = TecuClassification {
            base_class: c,
            ..Default::default()
        };
        println!("  {c:?} → {inner}");
    }

    // ─── Power-state enum ──────────────────────────────────────────
    println!("\n[power-state]");
    for s in [
        PowerState::PowerOff,
        PowerState::IgnitionOn,
        PowerState::ShutdownInitiated,
        PowerState::FinalShutdown,
    ] {
        println!("  {s:?}");
    }
    println!("\n  default = {:?}  (boot state)", PowerState::default());
}
