use machbus::isobus::tc::{
    DDI, DDOP, DeviceElement, DeviceElementType, DeviceObject, DeviceProcessData, DeviceProperty,
    DeviceValuePresentation, ObjectID, TriggerMethod, ddi,
};

fn minimal_ddop() -> DDOP {
    DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("implement")
                .with_software_version("0.1.3")
                .with_serial_number("serial"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_number(1)
                .with_parent(1)
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
                .with_id(1)
                .with_designator("implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_parent(1)
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
                .with_id(1)
                .with_designator("implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_number(1)
                .with_parent(1)
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
                .with_id(1)
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
                .with_id(1)
                .with_designator("implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_parent(1)
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

#[test]
fn tc_ddop_rejects_unencodable_text_and_non_finite_scales() {
    let non_ascii = minimal_ddop().with_property(
        DeviceProperty::default()
            .with_id(4)
            .with_ddi(DDI(ddi::SECTION_CONTROL_STATE))
            .with_value(1)
            .with_designator("räte"),
    );
    assert!(
        non_ascii.serialize().is_err(),
        "wire text is validated before serialization"
    );
    assert!(
        non_ascii.validate().is_err(),
        "pool validation must catch the same text before activation"
    );

    let overlong_designator = "A".repeat(u8::MAX as usize + 1);
    let overlong = minimal_ddop().with_process_data(
        DeviceProcessData::default()
            .with_id(4)
            .with_ddi(DDI(ddi::ACTUAL_VOLUME_PER_AREA_APPLICATION_RATE))
            .with_designator(overlong_designator),
    );
    assert!(
        overlong.validate().is_err(),
        "one-byte wire text lengths must be enforced at validation time"
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
