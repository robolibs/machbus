//! Diagnostic message round-trips: DM1 (active DTCs), DM2 (previous),
//! DM13 (broadcast control). Mirrors `diagnostic_demo.cpp`.

use machbus::j1939::{DiagnosticLamps, DmDtcList, Dtc, Fmi, LampStatus};

fn main() {
    println!("=== Diagnostic Demo ===");

    // ─── Build a DM1 with two DTCs ─────────────────────────────────
    let dm1 = DmDtcList {
        lamps: DiagnosticLamps {
            amber_warning: LampStatus::On,
            ..Default::default()
        },
        dtcs: vec![
            Dtc {
                spn: 520,
                fmi: Fmi::VoltageLow,
                occurrence_count: 0,
            },
            Dtc {
                spn: 190,
                fmi: Fmi::AbnormalRateChange,
                occurrence_count: 7,
            },
        ],
    };
    println!(
        "[DM1] {} active DTCs, amber={:?}",
        dm1.dtcs.len(),
        dm1.lamps.amber_warning
    );

    let bytes = dm1.encode();
    let decoded = DmDtcList::decode(&bytes).unwrap();
    println!(
        "[encode/decode] {} bytes round-trip → {} DTCs",
        bytes.len(),
        decoded.dtcs.len()
    );
    for d in &decoded.dtcs {
        println!(
            "  SPN=0x{:05X} FMI={:?} OC={}",
            d.spn, d.fmi, d.occurrence_count
        );
    }

    // ─── DTC field encoding ────────────────────────────────────────
    let dtc = Dtc {
        spn: 0x1_2345,
        fmi: Fmi::MechanicalFail,
        occurrence_count: 12,
    };
    let dtc_bytes = dtc.encode();
    let dtc_decoded = Dtc::decode(&dtc_bytes).unwrap();
    println!(
        "\n[DTC] 4-byte field: SPN=0x{:05X} → wire {:02X?} → SPN=0x{:05X}",
        dtc.spn, dtc_bytes, dtc_decoded.spn,
    );
    assert_eq!(dtc, dtc_decoded);
    println!("✓ DTC round-trips byte-exact");
}
