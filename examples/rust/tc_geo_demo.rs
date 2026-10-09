//! TC-GEO: load a prescription map and look up the application rate
//! at a position. Mirrors `tc_geo_demo.cpp`.

use machbus::geo::Wgs;
use machbus::nmea::GNSSPosition;
use std::cell::RefCell;
use std::rc::Rc;

use machbus::isobus::tc::{DDI, GeoPoint, PrescriptionMap, PrescriptionZone, TCGEOInterface, ddi};

fn main() {
    println!("=== TC-GEO Prescription Map Demo ===");

    let mut tc = TCGEOInterface::new();

    // Two zones: 100 mm³/m² inside the south square, 200 inside the north.
    tc.add_prescription_map(PrescriptionMap {
        structure_label: "demo-map".to_string(),
        zones: vec![
            PrescriptionZone {
                boundary: vec![
                    Wgs::new(52.0, 4.0, 0.0),
                    Wgs::new(52.0, 4.001, 0.0),
                    Wgs::new(52.001, 4.001, 0.0),
                    Wgs::new(52.001, 4.0, 0.0),
                ],
                holes: Vec::new(),
                application_rate: 100,
            },
            PrescriptionZone {
                boundary: vec![
                    Wgs::new(52.001, 4.0, 0.0),
                    Wgs::new(52.001, 4.001, 0.0),
                    Wgs::new(52.002, 4.001, 0.0),
                    Wgs::new(52.002, 4.0, 0.0),
                ],
                holes: Vec::new(),
                application_rate: 200,
            },
        ],
    });
    println!(
        "[map] {} zones loaded",
        tc.prescription_maps()[0].zones.len()
    );

    // Subscribe to rate-change events.
    let log: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let l = log.clone();
    tc.on_application_rate_changed
        .subscribe(move |&r| l.borrow_mut().push(r));

    // Drive a 3-point trajectory through the field.
    for (label, pos) in [
        ("south zone   ", Wgs::new(52.0005, 4.0005, 0.0)),
        ("between zones", Wgs::new(52.001, 4.0005, 0.0)),
        ("north zone   ", Wgs::new(52.0015, 4.0005, 0.0)),
        ("outside      ", Wgs::new(53.0, 5.0, 0.0)),
    ] {
        tc.set_position(GeoPoint {
            position: pos,
            timestamp_us: 0,
        });
        tc.update(0);
        let rate = tc.get_rate_at_position(pos);
        let rate_eng = tc
            .get_rate_at_position_engineering(
                pos,
                DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE),
            )
            .unwrap();
        println!(
            "  {label}: lat={:.4} lon={:.4} → raw={:?}, mm³/m²={:?}",
            pos.latitude, pos.longitude, rate, rate_eng
        );
    }
    println!("\n[events] rate transitions observed: {:?}", log.borrow());

    // Position reaches the TC over PGN_GNSS_POSITION. It used to be sent as
    // Process Data under DDI 0x0087/0x0088, which the ISO 11783-11 dictionary
    // assigns to Device Element Offset Y and Z in millimetres — a latitude of
    // 52 degrees told the TC the element sat 520 km off the datum.
    let position_pos = Wgs::new(52.0005, 4.0005, 0.0);
    let mut tc2 = TCGEOInterface::new();
    tc2.set_position(GeoPoint {
        position: position_pos,
        timestamp_us: 0,
    });
    println!(
        "\n[state] TC-GEO position: {:?}",
        tc2.current_position().map(|p| p.position)
    );

    // Silence unused import for non-test builds.
    let _ = std::mem::size_of::<GNSSPosition>();
}
