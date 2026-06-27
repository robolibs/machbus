#[test]
fn render_runtime_copies_graphics_context_text_cells_to_picture_pixels() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([ChildRef::new(
                ObjectID::new(20),
                30,
                0,
            )]),
        )
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 20,
                    viewport_height: 16,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 20,
                    canvas_height: 16,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(
            12,
            &FontAttributesBody {
                font_color: 21,
                font_size: 0,
                font_type: 0,
                font_style: 0,
            },
        ))
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 20,
                    actual_width: 20,
                    actual_height: 16,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 20 * 16],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x03,
            payload: vec![7],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x06,
            payload: 12u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0D,
            payload: vec![0, 3, b'A', b' ', b'B'],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            width: 20,
            height: 16,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data[0] == 21
            && data[6] == 7
            && data[12] == 21
    )));

    let framebuffer = FramebufferRenderer::default()
        .render_runtime(&runtime)
        .expect("runtime picture update renders through framebuffer");
    let copied_text_colour = Palette::default_isobus().resolve(21);
    assert!(framebuffer.count_colour(copied_text_colour) > 0);
}

#[test]
fn render_runtime_expands_graphics_context_cursor_point_and_line_commands() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 60,
                    viewport_height: 16,
                    viewport_x: 4,
                    viewport_y: 5,
                    canvas_width: 80,
                    canvas_height: 30,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x02,
            payload: vec![16],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![2, 0, 3, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x08,
            payload: vec![5, 0, 0xFF, 0xFF],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x09,
            payload: vec![0xFE, 0xFF, 4, 0],
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    let foreground = Palette::default_isobus().resolve(16);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect {
            rect: Rect {
                x: 11,
                y: 7,
                w: 1,
                h: 1
            },
            colour,
        } if *colour == foreground
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Line {
            x0: 11,
            y0: 7,
            x1: 9,
            y1: 11,
            colour,
            width: 1,
            line_art: _,
        } if *colour == foreground
    )));
}

#[test]
fn render_runtime_expands_graphics_context_attribute_subcommands() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 80,
                    viewport_height: 30,
                    viewport_x: 4,
                    viewport_y: 5,
                    canvas_width: 80,
                    canvas_height: 30,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_line_attributes(
            12,
            &LineAttributesBody {
                line_color: 18,
                line_width: 3,
                line_art: 0xFFFF,
            },
        ))
        .with_object(
            create_fill_attributes(
                13,
                &FillAttributesBody {
                    fill_type: 2,
                    fill_color: 19,
                    fill_pattern: ObjectID::NULL,
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(
            14,
            &FontAttributesBody {
                font_color: 20,
                font_size: 2,
                font_type: 0,
                font_style: 0x01,
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    for (subcommand, target) in [(0x04, 12u16), (0x05, 13), (0x06, 14)] {
        runtime
            .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
                id: ObjectID::new(11),
                subcommand,
                payload: target.to_le_bytes().to_vec(),
            })
            .unwrap();
    }
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0A,
            payload: vec![12, 0, 8, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0D,
            payload: vec![1, 2, b'O', b'K'],
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    let palette = Palette::default_isobus();
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect {
            rect: Rect {
                x: 4,
                y: 5,
                w: 12,
                h: 8
            },
            colour,
        } if *colour == palette.resolve(19)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::StrokeRect {
            rect: Rect {
                x: 4,
                y: 5,
                w: 12,
                h: 8
            },
            colour,
            width: 3,
            ..
        } if *colour == palette.resolve(18)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::DrawText { text, style, .. } if text == "OK"
            && style.foreground == palette.resolve(20)
            && style.font == FontMetrics::for_size(2)
            && style.decoration.bold
    )));
}

#[test]
fn render_runtime_expands_graphics_context_viewport_subcommands() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 60,
                    viewport_height: 16,
                    viewport_x: 4,
                    viewport_y: 5,
                    canvas_width: 80,
                    canvas_height: 30,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0E,
            payload: vec![20, 0, 30, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x11,
            payload: vec![10, 0, 8, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0F,
            payload: 1.0f32.to_bits().to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x08,
            payload: vec![1, 0, 2, 0],
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextViewport {
            object_id,
            viewport: Rect {
                x: 20,
                y: 30,
                w: 60,
                h: 16,
            },
            zoom_raw: Some(bits),
        } if *object_id == ObjectID::new(11) && *bits == 1.0f32.to_bits()
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextViewport {
            object_id,
            viewport: Rect {
                x: 20,
                y: 30,
                w: 10,
                h: 8,
            },
            zoom_raw: Some(bits),
        } if *object_id == ObjectID::new(11) && *bits == 1.0f32.to_bits()
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect {
            rect: Rect {
                x: 21,
                y: 32,
                w: 1,
                h: 1,
            },
            ..
        }
    )));
}

#[test]
fn render_runtime_expands_graphics_context_draw_vt_object_subcommand() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 60,
                    viewport_height: 16,
                    viewport_x: 4,
                    viewport_y: 5,
                    canvas_width: 80,
                    canvas_height: 30,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_line_attributes(
            12,
            &LineAttributesBody {
                line_color: 18,
                line_width: 2,
                line_art: 0xFFFF,
            },
        ))
        .with_object(
            create_output_rectangle(
                20,
                &OutputRectangleBody {
                    width: 12,
                    height: 7,
                    line_attributes: ObjectID::new(12),
                    line_suppression: 0,
                    fill_attributes: ObjectID::NULL,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                30,
                &PictureGraphicBody {
                    width: 20,
                    actual_width: 20,
                    actual_height: 16,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 20 * 16],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![2, 0, 3, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x12,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x08,
            payload: vec![0, 0, 0, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 30u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    let line_colour = Palette::default_isobus().resolve(18);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::StrokeRect {
            rect: Rect {
                x: 6,
                y: 8,
                w: 12,
                h: 7,
            },
            colour,
            width: 2,
            ..
        } if *colour == line_colour
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect {
            rect: Rect {
                x: 17,
                y: 14,
                w: 1,
                h: 1,
            },
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            width: 20,
            height: 16,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(30)
            && data[2 + 3 * 20] == 18
            && data[3 + 4 * 20] == 18
            && data[13 + 9 * 20] == 1
    )));
}

#[test]
fn render_runtime_rejects_graphics_context_draw_vt_object_scaled_bitmap_target() {
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
        )
        .with_object(
            create_scaled_bitmap(
                45,
                &ScaledBitmapBody {
                    bitmap_data: ObjectID::NULL,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime.clear_dirty();
    assert!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
                id: ObjectID::new(11),
                subcommand: 0x12,
                payload: 45u16.to_le_bytes().to_vec(),
            })
            .is_err(),
        "Draw VT Object must reject ScaledBitmap because it is an machbus compatibility extension, not a drawable standard VT target"
    );
    assert!(
        !runtime.is_dirty(),
        "rejected Graphics Context targets must not append replay state"
    );
}

#[test]
fn render_runtime_copies_graphics_context_open_ellipse_arc_pixels() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 40,
                    viewport_height: 24,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 40,
                    canvas_height: 24,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_line_attributes(
            12,
            &LineAttributesBody {
                line_color: 18,
                line_width: 1,
                line_art: 0xFFFF,
            },
        ))
        .with_object(
            create_output_ellipse(
                20,
                &OutputEllipseBody {
                    width: 18,
                    height: 12,
                    line_attributes: ObjectID::new(12),
                    ellipse_type: 1,
                    start_angle: 0,
                    end_angle: 45,
                    fill_attributes: ObjectID::NULL,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                30,
                &PictureGraphicBody {
                    width: 40,
                    actual_width: 40,
                    actual_height: 24,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 40 * 24],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![4, 0, 6, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x12,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 30u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::EllipseArc {
            rect: Rect {
                x: 4,
                y: 6,
                w: 18,
                h: 12,
            },
            ellipse_type: 1,
            start_angle: 0,
            end_angle: 45,
            ..
        }
    )));

    let copied = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::GraphicsContextPictureData {
                object_id,
                picture_id,
                source: GraphicsContextCopySource::Canvas,
                data,
                ..
            } if *object_id == ObjectID::new(11) && *picture_id == ObjectID::new(30) => Some(data),
            _ => None,
        })
        .expect("Copy Canvas should produce Picture Graphic pixel data");
    let arc_pixels = copied.iter().filter(|&&pixel| pixel == 18).count();
    assert!(
        arc_pixels > 0,
        "Draw VT Object must replay open ellipse arcs into the copyable canvas"
    );
    assert!(
        arc_pixels < 18 * 12,
        "open arc replay must not degrade into a filled/full ellipse copy"
    );
}

#[test]
fn render_runtime_copies_graphics_context_png_draw_vt_object_as_palette_pixels() {
    let mut palette_entries = vec![0x00_00_00_00; 256];
    palette_entries[40] = 0x0010_2232;
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_colour_palette(
                90,
                &ColourPaletteBody {
                    options: 0,
                    entries_argb: palette_entries,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 2,
                    viewport_height: 2,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 2,
                    canvas_height: 2,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_graphic(
                20,
                &ScaledGraphicBody {
                    width: 2,
                    height: 2,
                    scale_type: 3,
                    value: ObjectID::new(21),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                21,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgba(2, 2),
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                30,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 4],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x12,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 30u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::RgbaImage {
            object_id,
            rect,
            width: 2,
            height: 2,
            data,
        } if *object_id == ObjectID::new(20)
            && *rect == Rect::new(0, 0, 2, 2)
            && data.chunks_exact(4).all(|pixel| pixel == [0x11, 0x22, 0x33, 0xFF])
    )));
    let copied = commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::GraphicsContextPictureData {
                object_id,
                picture_id,
                source: GraphicsContextCopySource::Canvas,
                data,
                ..
            } if *object_id == ObjectID::new(11) && *picture_id == ObjectID::new(30) => Some(data),
            _ => None,
        })
        .expect("Copy Canvas should produce Picture Graphic pixel data");
    assert_eq!(
        copied.as_slice(),
        &[40, 40, 40, 40],
        "PNG RGBA Draw VT Object pixels must be quantised to the nearest active VT palette entry instead of collapsing to colour index 0"
    );
}

#[test]
fn render_runtime_copies_graphics_context_line_attribute_colour_pixels() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 4,
                    viewport_height: 4,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 4,
                    canvas_height: 4,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_line_attributes(
            12,
            &LineAttributesBody {
                line_color: 18,
                line_width: 1,
                line_art: 0xFFFF,
            },
        ))
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 4,
                    actual_width: 4,
                    actual_height: 4,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 16],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x04,
            payload: 12u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![0, 0, 0, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x09,
            payload: vec![2, 0, 0, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    let line_colour = Palette::default_isobus().resolve(18);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Line {
            x0: 0,
            y0: 0,
            x1: 2,
            y1: 0,
            colour,
            width: 1,
            line_art: _,
        } if *colour == line_colour
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            width: 4,
            height: 4,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data[0] == 18
            && data[1] == 18
            && data[2] == 18
            && data[3] == 0
    )));
}

#[test]
fn render_runtime_copies_graphics_context_rectangle_line_art_pixels() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 8,
                    viewport_height: 4,
                    canvas_width: 8,
                    canvas_height: 4,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_line_attributes(
            12,
            &LineAttributesBody {
                line_color: 18,
                line_width: 1,
                line_art: 0xF0F0,
            },
        ))
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 8,
                    actual_width: 8,
                    actual_height: 4,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 32],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x04,
            payload: 12u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![0, 0, 0, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0A,
            payload: [8u16.to_le_bytes(), 4u16.to_le_bytes()].concat(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::StrokeRect {
            rect: Rect {
                x: 0,
                y: 0,
                w: 8,
                h: 4
            },
            line_art: 0xF0F0,
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            width: 8,
            height: 4,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data[0] == 18
            && data[1] == 18
            && data[2] == 18
            && data[3] == 18
            && data[4] == 0
            && data[5] == 0
            && data[6] == 0
            && data[7] == 18
    )));
}

#[test]
fn render_runtime_emits_graphics_context_shape_line_art_commands() {
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
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_line_attributes(
            12,
            &LineAttributesBody {
                line_color: 18,
                line_width: 1,
                line_art: 0xF0F0,
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x04,
            payload: 12u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![1, 0, 1, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0B,
            payload: [6u16.to_le_bytes(), 4u16.to_le_bytes()].concat(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![2, 0, 8, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0C,
            payload: vec![
                3, // number of points
                4, 0, 0, 0, // right
                4, 0, 4, 0, // down
                0, 0, 0, 0, // close
            ],
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ellipse {
            rect: Rect {
                x: 1,
                y: 1,
                w: 6,
                h: 4
            },
            line_art: 0xF0F0,
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Polygon {
            origin: (2, 8),
            points,
            line_art: 0xF0F0,
            ..
        } if points.as_slice() == [(2, 8), (6, 8), (6, 12), (2, 8)]
    )));
}

#[test]
fn render_runtime_initialises_graphics_context_standard_body_colour_mode() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 4,
                    viewport_height: 4,
                    canvas_width: 4,
                    canvas_height: 4,
                    foreground_colour: 3,
                    line_attributes: ObjectID::new(12),
                    options: 0x02,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_line_attributes(
            12,
            &LineAttributesBody {
                line_color: 18,
                line_width: 1,
                line_art: 0xFFFF,
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x09,
            payload: vec![2, 0, 0, 0],
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    let line_colour = Palette::default_isobus().resolve(18);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Line {
            x0: 0,
            y0: 0,
            x1: 2,
            y1: 0,
            colour,
            width: 1,
            line_art: _,
        } if *colour == line_colour
    )));
}

#[test]
fn render_runtime_expands_graphics_context_copy_to_picture_subcommands() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 60,
                    viewport_height: 16,
                    viewport_x: 4,
                    viewport_y: 5,
                    canvas_width: 80,
                    canvas_height: 30,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 60,
                    actual_width: 60,
                    actual_height: 16,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 60 * 16],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x03,
            payload: vec![7],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![0, 0, 0, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x07,
            payload: vec![2, 0, 2, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0E,
            payload: vec![20, 0, 30, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0F,
            payload: 1.0f32.to_bits().to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x14,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextCopyToPicture {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            viewport: Rect {
                x: 20,
                y: 30,
                w: 60,
                h: 16,
            },
            zoom_raw: Some(bits),
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && *bits == 1.0f32.to_bits()
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            width: 60,
            height: 16,
            format: 2,
            transparent_index: _,
            data,
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data[0] == 7
            && data[1] == 7
            && data[60] == 7
            && data[61] == 7
            && data[2] == 0
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextCopyToPicture {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Viewport,
            viewport: Rect {
                x: 20,
                y: 30,
                w: 60,
                h: 16,
            },
            zoom_raw: Some(bits),
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && *bits == 1.0f32.to_bits()
    )));
}

#[test]
fn render_runtime_treats_copy_colours_outside_picture_format_as_transparent() {
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
                    format: 0,
                    options: 0,
                    transparency: 0,
                    data: vec![0b1000_0000],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    for command in [
        VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x02,
            payload: vec![2],
        },
        VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x08,
            payload: vec![0, 0, 0, 0],
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

#[test]
fn render_runtime_copy_viewport_picture_pixels_apply_zoom() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 4,
                    viewport_height: 4,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 8,
                    canvas_height: 8,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 4,
                    actual_width: 4,
                    actual_height: 4,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 16],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x02,
            payload: vec![7],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![1, 0, 1, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x08,
            payload: vec![0, 0, 0, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0F,
            payload: 2.0f32.to_bits().to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x14,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Viewport,
            width: 4,
            height: 4,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data.as_slice() == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 7, 0, 0, 7, 7]
    )));
}

#[test]
fn render_runtime_places_graphics_context_replay_at_scene_position_and_copies_viewport_pixels() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([ChildRef::new(
                ObjectID::new(11),
                100,
                50,
            )]),
        )
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 8,
                    viewport_height: 8,
                    viewport_x: 4,
                    viewport_y: 5,
                    canvas_width: 20,
                    canvas_height: 20,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 4,
                    actual_width: 4,
                    actual_height: 4,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 16],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x02,
            payload: vec![7],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![4, 0, 5, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x08,
            payload: vec![0, 0, 0, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x14,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect {
            rect: Rect {
                x: 108,
                y: 60,
                w: 1,
                h: 1,
            },
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Viewport,
            width: 4,
            height: 4,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data[0] == 7
    )));
}

#[test]
fn render_runtime_expands_packed_picture_graphic_pixels_when_drawing_into_graphics_context_canvas()
{
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 4,
                    viewport_height: 4,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 4,
                    canvas_height: 4,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                30,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 0,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0b1000_0000, 0b1000_0000],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                40,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 4],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x12,
            payload: 30u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 40u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            width: 2,
            height: 2,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(40)
            && data.as_slice() == [1, 0, 1, 0]
    )));
}

#[test]
fn render_runtime_scales_draw_vt_object_images_inside_graphics_context_canvas() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 4,
                    viewport_height: 4,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 4,
                    canvas_height: 4,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                30,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![1, 2, 3, 4],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_graphic(
                31,
                &ScaledGraphicBody {
                    width: 4,
                    height: 4,
                    scale_type: 3,
                    value: ObjectID::new(30),
                    options: 0,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                40,
                &PictureGraphicBody {
                    width: 4,
                    actual_width: 4,
                    actual_height: 4,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 16],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x12,
            payload: 31u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 40u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            width: 4,
            height: 4,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(40)
            && data.as_slice() == [
                1, 1, 2, 2,
                1, 1, 2, 2,
                3, 3, 4, 4,
                3, 3, 4, 4,
            ]
    )));
}

#[test]
fn render_runtime_copies_ellipse_and_polygon_pixels_from_graphics_context_canvas() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 20,
                    viewport_height: 20,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 20,
                    canvas_height: 20,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_fill_attributes(
                12,
                &FillAttributesBody {
                    fill_type: 2,
                    fill_color: 18,
                    fill_pattern: ObjectID::NULL,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 12,
                    actual_width: 12,
                    actual_height: 12,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0; 12 * 12],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x05,
            payload: 12u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![1, 0, 1, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0B,
            payload: vec![5, 0, 5, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![7, 0, 1, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0C,
            payload: vec![
                4, // number of points
                4, 0, 0, 0, // right
                4, 0, 4, 0, // down
                0, 0, 4, 0, // left
                0, 0, 0, 0, // closed
            ],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source: GraphicsContextCopySource::Canvas,
            width: 12,
            height: 12,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data[3 + 3 * 12] == 18
            && data[8 + 2 * 12] == 18
            && data[7 + 12] == 1
    )));
}

#[test]
fn render_runtime_expands_graphics_context_erase_rectangle_and_ellipse_commands() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 60,
                    viewport_height: 16,
                    viewport_x: 4,
                    viewport_y: 5,
                    canvas_width: 80,
                    canvas_height: 30,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x03,
            payload: vec![17],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x07,
            payload: vec![6, 0, 4, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0B,
            payload: vec![3, 0, 2, 0],
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    let background = Palette::default_isobus().resolve(17);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect {
            rect: Rect {
                x: 4,
                y: 5,
                w: 6,
                h: 4
            },
            colour,
        } if *colour == background
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ellipse {
            rect: Rect {
                x: 9,
                y: 8,
                w: 3,
                h: 2
            },
            filled: false,
            ..
        }
    )));
}

#[test]
fn gtui_render_draws_graphics_context_extension_as_fill_and_border_swatch() {
    // machbus compatibility extension (object type 50): the geometry-less
    // GraphicsContext state renders best-effort as a fixed-extent fill+border
    // swatch using its 24-bit RGB fill/line state (R is the low byte).
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_graphics_context(
            3,
            &GraphicsContextBody {
                context: GraphicsContextV6 {
                    fill_color_rgb: 0x0033_2211,
                    line_color_rgb: 0x00CC_BBAA,
                    line_width: 2,
                    ..GraphicsContextV6::default()
                },
            },
        ));
    let scene = render(&pool, ObjectID::NULL);
    assert!(
        !scene
            .unsupported
            .iter()
            .any(|record| record.object_type == ObjectType::GraphicsContext),
        "{:?}",
        scene.unsupported
    );
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::FillRect { colour, .. } if *colour == Colour::rgb(0x11, 0x22, 0x33)
        )),
        "fill swatch from the 24-bit fill colour: {cmds:?}"
    );
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::StrokeRect { colour, width: 2, .. }
                if *colour == Colour::rgb(0xAA, 0xBB, 0xCC)
        )),
        "border from the 24-bit line colour and line width: {cmds:?}"
    );
}

#[test]
fn framebuffer_render_rasterises_graphics_context_extension_swatch() {
    // The plural GraphicsContext extension renders through the shared command
    // stream, so the framebuffer backend rasterises its fill swatch too.
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_graphics_context(
            3,
            &GraphicsContextBody {
                context: GraphicsContextV6 {
                    fill_color_rgb: 0x0033_2211,
                    line_width: 0,
                    ..GraphicsContextV6::default()
                },
            },
        ));
    let scene = render(&pool, ObjectID::NULL);
    let frame = FramebufferRenderer::default()
        .render_scene(&scene)
        .expect("framebuffer renders the scene");
    assert_eq!(frame.pixel(2, 2), Some(Colour::rgb(0x11, 0x22, 0x33)));
}
