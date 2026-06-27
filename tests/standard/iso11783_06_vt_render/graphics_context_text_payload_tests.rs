#[test]
fn render_runtime_accepts_zero_length_graphics_context_draw_text() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 16,
                    viewport_height: 16,
                    canvas_width: 16,
                    canvas_height: 16,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0D,
            payload: vec![1, 0],
        })
        .unwrap();
    assert!(matches!(
        update,
        RenderUpdate::CommandStreamChanged { .. }
    ));

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextReplay {
            object_id: ObjectID(11),
            subcommand: 0x0D,
            payload,
        } if payload.as_slice() == [1, 0]
    )));
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::DrawText { text, .. } if text.is_empty())),
        "zero-length Draw Text is a real no-text draw command, not an invalid Graphics Context payload"
    );
}
