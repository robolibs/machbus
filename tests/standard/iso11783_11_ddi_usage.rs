use machbus::isobus::tc::ddi_database::{
    DDI_DATABASE, DDI_DATABASE_FINGERPRINT_FNV1A64, DDI_DATABASE_SIZE, DataDictionary, ddi,
    ddi_data_dictionary_entry, ddi_database_fingerprint, ddi_from_engineering, ddi_is_proprietary,
    ddi_lookup, ddi_resolution, ddi_to_engineering,
};
use machbus::isobus::tc::ddop::DDOP;
use machbus::isobus::tc::ddop_helpers::{extract_geometry, extract_rates, extract_totals};
use machbus::isobus::tc::objects::{
    DeviceElement, DeviceElementType, DeviceObject, DeviceProcessData, DeviceProperty, ObjectID,
};

#[test]
fn ddi_database_identity_is_explicit_and_sorted() {
    assert_eq!(DDI_DATABASE.len(), DDI_DATABASE_SIZE);
    assert!(DDI_DATABASE.len() > 700);
    assert_ne!(ddi_database_fingerprint(), 0);
    assert_eq!(ddi_database_fingerprint(), DDI_DATABASE_FINGERPRINT_FNV1A64);

    for pair in DDI_DATABASE.windows(2) {
        assert!(
            pair[0].ddi < pair[1].ddi,
            "DDI database must stay sorted and duplicate-free"
        );
    }
}

#[test]
fn ddi_lookup_distinguishes_known_application_rate_from_unknown_sentinel() {
    let known = ddi_lookup(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE)
        .expect("selected DDI should exist in the generated implementation table");
    assert_eq!(known.ddi, ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE);
    assert!(!known.name.is_empty());
    assert!(!known.unit.is_empty());

    assert_eq!(ddi_lookup(u16::MAX - 1), None);
}

#[test]
fn ddi_proprietary_range_boundaries_do_not_collide_with_unknown_sentinel() {
    assert!(!ddi_is_proprietary(0xDFFF));
    assert!(ddi_is_proprietary(0xE000));
    assert!(ddi_is_proprietary(0xFFFE));
    assert!(!ddi_is_proprietary(u16::MAX));

    assert!(ddi_lookup(u16::MAX).is_some());
    assert_eq!(ddi_lookup(0xFFFE), None);
    assert_eq!(
        ddi_data_dictionary_entry(0xFFFE),
        DataDictionary::get_entry(0xFFFE)
    );
    assert_eq!(DataDictionary::get_entry(0xFFFE).ddi, u16::MAX);
    assert_eq!(DataDictionary::get_entry(0xFFFE).name, "Unknown");
}

#[test]
fn ddi_engineering_conversion_uses_resolution_and_saturates_invalid_inputs() {
    let ddi = ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE;
    let resolution = ddi_resolution(ddi);
    assert!(resolution.is_finite());
    assert!(resolution > 0.0);

    let raw = 12_345;
    let engineering = ddi_to_engineering(ddi, raw);
    assert_eq!(engineering, f64::from(raw) * resolution);
    assert_eq!(ddi_from_engineering(ddi, engineering), raw);

    assert_eq!(ddi_to_engineering(0xFFFE, raw), f64::from(raw));
    assert_eq!(ddi_from_engineering(ddi, f64::NAN), 0);
    assert_eq!(ddi_from_engineering(ddi, f64::INFINITY), 0);
    assert_eq!(
        ddi_from_engineering(ddi, f64::from(i32::MAX) * resolution * 2.0),
        i32::MAX
    );
    assert_eq!(
        ddi_from_engineering(ddi, f64::from(i32::MIN) * resolution * 2.0),
        i32::MIN
    );
}

#[test]
fn ddi_helpers_preserve_geometry_rate_and_total_semantics_with_named_dictionary_refs() {
    for ddi in [
        ddi::CONNECTOR_PIVOT_X_OFFSET,
        ddi::DEVICE_ELEMENT_OFFSET_X,
        ddi::DEVICE_ELEMENT_OFFSET_Y,
        ddi::DEVICE_ELEMENT_OFFSET_Z,
        ddi::MAXIMUM_WORKING_WIDTH,
        ddi::ACTUAL_WORKING_WIDTH,
        ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE,
        ddi::DEFAULT_MASS_PER_AREA_APPLICATION_RATE,
        ddi::TOTAL_AREA,
    ] {
        assert!(
            ddi_lookup(ddi).is_some(),
            "DDOP helper standard-test fixture must use known named DDI references"
        );
    }

    let ddop = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("Named DDI implement"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(10)
                .with_ddi(ddi::CONNECTOR_PIVOT_X_OFFSET)
                .with_value(1_500),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Connector)
                .with_children(vec![ObjectID(10)]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(3)
                .with_type(DeviceElementType::Function)
                .with_designator("Boom")
                .with_children(vec![ObjectID(4), ObjectID(18)]),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(11)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_X)
                .with_value(250),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(12)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Y)
                .with_value(-750),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(13)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Z)
                .with_value(125),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(14)
                .with_ddi(ddi::MAXIMUM_WORKING_WIDTH)
                .with_value(3_500),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(15)
                .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                .with_value(3_000),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(16)
                .with_ddi(ddi::DEFAULT_MASS_PER_AREA_APPLICATION_RATE)
                .with_value(12_000)
                .with_designator("Fallback mass rate"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(17)
                .with_ddi(ddi::TOTAL_AREA)
                .with_value(45_000)
                .with_designator("Area total"),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(18)
                .with_ddi(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE)
                .with_designator("Runtime volume rate")
                .with_presentation(ObjectID(99)),
        )
        .with_element(
            DeviceElement::default()
                .with_id(4)
                .with_type(DeviceElementType::Section)
                .with_number(1)
                .with_parent(ObjectID(3))
                .with_designator("Section 1")
                .with_children(vec![
                    ObjectID(11),
                    ObjectID(12),
                    ObjectID(13),
                    ObjectID(14),
                    ObjectID(15),
                    ObjectID(16),
                    ObjectID(17),
                ]),
        );

    let geometry = extract_geometry(&ddop);
    assert_eq!(geometry.connector_x_mm, 1_500);
    assert_eq!(geometry.sections.len(), 1);
    assert_eq!(geometry.sections[0].offset_x_mm, 250);
    assert_eq!(geometry.sections[0].offset_y_mm, -750);
    assert_eq!(geometry.sections[0].offset_z_mm, 125);
    assert_eq!(
        geometry.sections[0].width_mm, 3_000,
        "actual working width must win over maximum width when both are present"
    );
    assert_eq!(geometry.total_width_mm, 3_000);

    let rates = extract_rates(&ddop);
    assert_eq!(rates.len(), 2);
    assert!(
        rates.iter().any(|rate| {
            rate.ddi.0 == ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE
                && rate.editable
                && rate.value.is_none()
                && rate.presentation_id == ObjectID(99)
        }),
        "process-data application-rate DDI must remain runtime-editable"
    );
    assert!(
        rates.iter().any(|rate| {
            rate.ddi.0 == ddi::DEFAULT_MASS_PER_AREA_APPLICATION_RATE
                && !rate.editable
                && rate.value == Some(12_000)
        }),
        "property-backed rate DDI must remain a definition-time constant"
    );

    let totals = extract_totals(&ddop);
    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].ddi.0, ddi::TOTAL_AREA);
    assert_eq!(totals[0].value, Some(45_000));
    assert!(!totals[0].editable);
}
