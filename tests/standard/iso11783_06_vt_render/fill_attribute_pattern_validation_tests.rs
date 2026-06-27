use machbus::isobus::vt::render::macros::{MacroEffect, apply_macro_effects};

fn bad_monochrome_pattern(id: u16) -> VTObject {
    create_picture_graphic(
        id,
        &PictureGraphicBody {
            width: 7,
            actual_width: 7,
            actual_height: 1,
            format: 0,
            options: 0,
            transparency: 0xFF,
            data: vec![0],
        },
    )
    .unwrap()
}

#[test]
fn render_runtime_rejects_generic_fill_pattern_with_unused_row_bits() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_fill_attributes(
                3,
                &FillAttributesBody {
                    fill_type: 3,
                    fill_pattern: ObjectID::NULL,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(bad_monochrome_pattern(4));
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(3),
                attribute_id: 3,
                value: 4,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "type-3 Fill Attributes pattern changes must reject PictureGraphic rows with unused packed bits"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(3))
            .unwrap()
            .get_fill_attributes_body()
            .unwrap()
            .fill_pattern,
        ObjectID::NULL
    );
}

#[test]
fn macro_report_rejects_generic_fill_pattern_with_unused_row_bits() {
    let mut pool = ObjectPool::default()
        .with_object(
            create_fill_attributes(
                3,
                &FillAttributesBody {
                    fill_type: 3,
                    fill_pattern: ObjectID::NULL,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_fill_attributes(
                5,
                &FillAttributesBody {
                    fill_type: 0,
                    fill_pattern: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(bad_monochrome_pattern(4));

    let report = apply_macro_effects(
        &mut pool,
        &[
            MacroEffect::ChangeGenericAttribute {
                object: ObjectID::new(3),
                attribute_id: 3,
                value: 4,
            },
            MacroEffect::ChangeGenericAttribute {
                object: ObjectID::new(5),
                attribute_id: 1,
                value: 3,
            },
        ],
    );

    assert!(report.generic_attribute_changes.is_empty());
    assert_eq!(
        report.skipped, 2,
        "macro Change Attribute must not report Fill Attributes changes that would activate an invalid pattern buffer"
    );
}
