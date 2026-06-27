#[test]
fn render_runtime_draw_vt_object_colours_outside_graphics_context_format_are_transparent() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 1,
                    viewport_height: 1,
                    canvas_width: 1,
                    canvas_height: 1,
                    format: 0,
                    options: 0,
                    transparency_colour: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0,
                    transparency: 0,
                    data: vec![1],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                30,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0,
                    transparency: u8::MAX,
                    data: vec![2],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    for command in [
        VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x12,
            payload: 30u16.to_le_bytes().to_vec(),
        },
        VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 20u16.to_le_bytes().to_vec(),
        },
    ] {
        runtime.apply_ecu_command(&command).unwrap();
    }

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            transparent_index: 0,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data.as_slice() == [0]
    )));
}
