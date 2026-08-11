use machbus::isobus::tc::{
    DDI, DDOP, DeviceElement, DeviceElementType, DeviceObject, DeviceProcessData, DeviceProperty,
    DeviceValuePresentation, ObjectID, TriggerMethod, ddi,
};

fn minimal_ddop() -> DDOP {
    DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(0u16)
                .with_designator("implement")
                .with_software_version("0.1.3")
                .with_serial_number("serial"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_number(0)
                .with_parent(0)
                .with_designator("root")
                .with_children([ObjectID(3)]),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(3)
                .with_ddi(DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE))
                .with_trigger(TriggerMethod::TimeInterval)
                .with_designator("rate"),
        )
}

#[test]
fn tc_ddop_validates_serializes_and_deserializes_minimal_graph() {
    let ddop = minimal_ddop();
    ddop.validate().unwrap();

    let bytes = ddop.serialize().unwrap();
    let decoded = DDOP::deserialize(&bytes).unwrap();
    decoded.validate().unwrap();

    assert_eq!(decoded.devices().len(), 1);
    assert_eq!(decoded.elements().len(), 1);
    assert_eq!(decoded.process_data().len(), 1);
}

#[test]
fn tc_ddop_rejects_missing_child_reference_before_claiming_completion() {
    let ddop = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(0u16)
                .with_designator("implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_parent(0)
                .with_designator("root")
                .with_children([ObjectID(99)]),
        );

    assert!(ddop.validate().is_err());
}

#[test]
fn tc_ddop_rejects_duplicate_ids_and_missing_presentation_references() {
    let duplicate = minimal_ddop().with_process_data(
        DeviceProcessData::default()
            .with_id(2)
            .with_ddi(DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE))
            .with_designator("duplicate-id"),
    );
    assert!(
        duplicate.validate().is_err(),
        "a DDOP must not contain the same object ID in two object classes"
    );

    let missing_process_data_presentation = minimal_ddop().with_process_data(
        DeviceProcessData::default()
            .with_id(4)
            .with_ddi(DDI(ddi::ACTUAL_VOLUME_PER_AREA_APPLICATION_RATE))
            .with_presentation(55)
            .with_designator("rate-actual"),
    );
    assert!(
        missing_process_data_presentation.validate().is_err(),
        "process data must not reference a value presentation that is absent"
    );

    let missing_property_presentation = minimal_ddop().with_property(
        DeviceProperty::default()
            .with_id(4)
            .with_ddi(DDI(ddi::SECTION_CONTROL_STATE))
            .with_value(1)
            .with_presentation(56)
            .with_designator("rate-mode"),
    );
    assert!(
        missing_property_presentation.validate().is_err(),
        "properties must not reference a value presentation that is absent"
    );
}

#[test]
fn tc_ddop_rejects_wrong_kind_element_parent_and_child_references() {
    let process_data_as_parent = minimal_ddop().with_element(
        DeviceElement::default()
            .with_id(4)
            .with_type(DeviceElementType::Section)
            .with_number(2)
            .with_parent(3)
            .with_designator("bad-parent"),
    );
    assert!(
        process_data_as_parent.validate().is_err(),
        "device-element parents must be device or device-element objects, not process data"
    );

    let value_presentation = DeviceValuePresentation::default()
        .with_id(4)
        .with_scale(1.0)
        .with_unit("l/ha");
    let value_presentation_as_child = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(0u16)
                .with_designator("implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_number(0)
                .with_parent(0)
                .with_designator("root")
                .with_children([ObjectID(4)]),
        )
        .with_value_presentation(value_presentation);
    assert!(
        value_presentation_as_child.validate().is_err(),
        "device-element child lists must not point directly at value presentations"
    );

    let nested_element_as_child = minimal_ddop().with_element(
        DeviceElement::default()
            .with_id(4)
            .with_type(DeviceElementType::Section)
            .with_number(2)
            .with_parent(2)
            .with_designator("section"),
    );
    nested_element_as_child.validate().unwrap();
}

#[test]
fn tc_ddop_rejects_null_and_self_referential_object_ids() {
    let null_device = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(ObjectID::NULL)
                .with_designator("bad-device"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_parent(ObjectID::NULL)
                .with_designator("root"),
        );
    assert!(
        null_device.validate().is_err(),
        "0xFFFF is the null/no-object marker and must not identify a real DDOP object"
    );

    let self_parent = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(0u16)
                .with_designator("implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_parent(2)
                .with_designator("root"),
        );
    assert!(
        self_parent.validate().is_err(),
        "a device element must not use itself as its parent"
    );

    let self_child = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(0u16)
                .with_designator("implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_parent(0)
                .with_designator("root")
                .with_children([ObjectID(2)]),
        );
    assert!(
        self_child.validate().is_err(),
        "a device element child list must not point back to the containing element"
    );
}

#[test]
fn tc_ddop_rejects_process_data_reserved_trigger_bits() {
    let valid_all_triggers = minimal_ddop().with_process_data(
        DeviceProcessData::default()
            .with_id(4)
            .with_ddi(DDI(ddi::ACTUAL_VOLUME_PER_AREA_APPLICATION_RATE))
            .with_triggers(
                TriggerMethod::TimeInterval.as_u8()
                    | TriggerMethod::DistanceInterval.as_u8()
                    | TriggerMethod::ThresholdLimits.as_u8()
                    | TriggerMethod::OnChange.as_u8()
                    | TriggerMethod::Total.as_u8(),
            )
            .with_designator("actual-rate"),
    );
    valid_all_triggers.validate().unwrap();
    valid_all_triggers.serialize().unwrap();

    let invalid_trigger_bits = minimal_ddop().with_process_data(
        DeviceProcessData::default()
            .with_id(4)
            .with_ddi(DDI(ddi::ACTUAL_VOLUME_PER_AREA_APPLICATION_RATE))
            .with_triggers(0x20)
            .with_designator("actual-rate"),
    );
    assert!(
        invalid_trigger_bits.validate().is_err(),
        "DDOP validation must reject process-data trigger bits outside the defined bitmask"
    );
    assert!(
        invalid_trigger_bits.serialize().is_err(),
        "DDOP serialization must reject the same reserved trigger bits before emitting bytes"
    );
}

/// F7 — ISO 11783-10 A.7: "A device descriptor object pool shall contain only a
/// single device object", Figure A.1 puts it at `ObjectId = 0`, and A.3 gives
/// exactly one DeviceElement of type device as the root — element number 0
/// (B.3.2: "The element number would be 0 to address the implement sprayer").
///
/// None of this was validated, so every DDOP the crate built in its own tests
/// and examples — device at ObjectId 1, no device-type element at all in some —
/// validated clean and was invalid on the wire.
#[test]
fn tc_ddop_enforces_the_annex_a_object_hierarchy() {
    // A conformant pool is accepted.
    minimal_ddop()
        .validate()
        .expect("the corrected fixture is valid");

    // Two DeviceObjects.
    let two_devices = minimal_ddop().with_device(
        DeviceObject::default()
            .with_id(9u16)
            .with_designator("second"),
    );
    assert!(
        two_devices.validate().is_err(),
        "A.7: only a single device object"
    );

    // The DeviceObject anywhere but ObjectId 0.
    let mut moved = DDOP::default().with_device(
        DeviceObject::default()
            .with_id(1u16)
            .with_designator("implement"),
    );
    moved = moved.with_element(
        DeviceElement::default()
            .with_id(2)
            .with_type(DeviceElementType::Device)
            .with_number(0)
            .with_parent(1)
            .with_designator("root"),
    );
    assert!(
        moved.validate().is_err(),
        "A.7 Figure A.1: the DeviceObject is ObjectId 0"
    );

    // No device-type element.
    let headless = DDOP::default()
        .with_device(DeviceObject::default().with_id(0u16).with_designator("d"))
        .with_element(
            DeviceElement::default()
                .with_id(1)
                .with_type(DeviceElementType::Section)
                .with_number(1)
                .with_parent(0)
                .with_designator("S1"),
        );
    assert!(
        headless.validate().is_err(),
        "A.3: exactly one device-type element"
    );

    // A device-type element that is not element number 0.
    let misnumbered = DDOP::default()
        .with_device(DeviceObject::default().with_id(0u16).with_designator("d"))
        .with_element(
            DeviceElement::default()
                .with_id(1)
                .with_type(DeviceElementType::Device)
                .with_number(7)
                .with_parent(0)
                .with_designator("root"),
        );
    assert!(
        misnumbered.validate().is_err(),
        "B.3.2: the implement itself is element number 0"
    );

    // Element numbers are a 12-bit field.
    let overflowing = minimal_ddop().with_element(
        DeviceElement::default()
            .with_id(40)
            .with_type(DeviceElementType::Section)
            .with_number(4096)
            .with_parent(2)
            .with_designator("S"),
    );
    assert!(
        overflowing.validate().is_err(),
        "element numbers are 0 to 4095"
    );
}

#[test]
fn tc_ddop_rejects_unencodable_text_and_non_finite_scales() {
    // F4 — Annex A.1: "several attributes in this representation are coded as
    // UTF-8 strings. These strings do not have a preceding byte-order mark
    // (BOM)." A localized designator is ordinary content. These two assertions
    // used to require an error, which made such a pool unserializable at all.
    let localized = minimal_ddop().with_property(
        DeviceProperty::default()
            .with_id(4)
            .with_ddi(DDI(ddi::SECTION_CONTROL_STATE))
            .with_value(1)
            .with_designator("räte"),
    );
    assert!(
        localized.serialize().is_ok(),
        "BOM-less UTF-8 is what A.1 specifies"
    );
    assert!(localized.validate().is_ok());

    // Tables A.1-A.5 size these fields "0 to 128" bytes.
    let overlong_designator = "A".repeat(129);
    let overlong = minimal_ddop().with_process_data(
        DeviceProcessData::default()
            .with_id(4)
            .with_ddi(DDI(ddi::ACTUAL_VOLUME_PER_AREA_APPLICATION_RATE))
            .with_designator(overlong_designator),
    );
    assert!(
        overlong.validate().is_err(),
        "the 128-byte Table A.1-A.5 text limit is enforced at validation time"
    );

    let non_finite_scale = minimal_ddop().with_value_presentation(
        DeviceValuePresentation::default()
            .with_id(4)
            .with_scale(f32::NAN)
            .with_unit("l/ha"),
    );
    assert!(
        non_finite_scale.validate().is_err(),
        "value presentation scales must be finite"
    );
}
