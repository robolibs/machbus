#[test]
fn gtui_render_emits_indexed_image_for_uncompressed_picture_graphic() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 0,
                    transparency: 3,
                    data: vec![0b1000_0000, 0b1000_0000],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(cmds.iter().any(|c| {
        matches!(
            c,
            RenderCommand::IndexedImage {
                width: 2,
                height: 2,
                format: 0,
                transparency: 3,
                data,
                ..
            } if data.as_slice() == [0b1000_0000, 0b1000_0000]
        )
    }));
}

#[test]
fn gtui_render_decodes_compressed_picture_graphic() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 12,
                    actual_width: 12,
                    actual_height: 1,
                    format: 2,
                    options: 0x04,
                    data: vec![6, 0, 3, 3, 2, 1, 1, 2],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(cmds.iter().any(|c| {
        matches!(
            c,
            RenderCommand::IndexedImage {
                width: 12,
                height: 1,
                format: 2,
                data,
                ..
            } if data.as_slice() == [0, 0, 0, 0, 0, 0, 3, 3, 3, 1, 1, 2]
        )
    }));
}

#[test]
fn gtui_render_decodes_standard_png_graphic_data_with_stored_deflate() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 8,
                    height: 4,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgba(2, 2),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 2,
                height: 2,
                data,
                ..
            } if *rect == Rect::new(0, 0, 8, 4)
                && data.len() == 16
                && data.chunks_exact(4).all(|pixel| pixel == [0x11, 0x22, 0x33, 0xFF])
        )),
        "{cmds:?}"
    );
    let fb = FramebufferRenderer::default()
        .render_scene(&scene)
        .expect("decoded PNG renders to framebuffer");
    assert_eq!(
        fb.count_colour(Colour::rgb(0x11, 0x22, 0x33)),
        usize::from(8u16) * usize::from(4u16),
        "scaled PNG GraphicData should paint the full destination rectangle"
    );
}

#[test]
fn gtui_render_decodes_standard_png_graphic_data_with_fixed_huffman_deflate() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 2,
                    height: 2,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgba_fixed_deflate(2, 2),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 2,
                height: 2,
                data,
                ..
            } if *rect == Rect::new(0, 0, 2, 2)
                && data.len() == 16
                && data.chunks_exact(4).all(|pixel| pixel == [0x11, 0x22, 0x33, 0xFF])
        )),
        "{cmds:?}"
    );
}

#[test]
fn gtui_render_decodes_standard_png_graphic_data_with_dynamic_huffman_deflate() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 2,
                    height: 2,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgba_dynamic_deflate(2, 2),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 2,
                height: 2,
                data,
                ..
            } if *rect == Rect::new(0, 0, 2, 2)
                && data.len() == 16
                && data.chunks_exact(4).all(|pixel| pixel == [0x11, 0x22, 0x33, 0xFF])
        )),
        "{cmds:?}"
    );
}

#[test]
fn gtui_render_decodes_standard_png_graphic_data_with_dynamic_huffman_backref() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 2,
                    height: 2,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgba_dynamic_deflate_backref(2, 2),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 2,
                height: 2,
                data,
                ..
            } if *rect == Rect::new(0, 0, 2, 2)
                && data.len() == 16
                && data.chunks_exact(4).all(|pixel| pixel == [0x11, 0x22, 0x33, 0xFF])
        )),
        "{cmds:?}"
    );
}

#[test]
fn gtui_render_decodes_standard_png_graphic_data_scanline_filters() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 3,
                    height: 5,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgba_filter_suite(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    let expected = expected_png_filter_suite_rgba();
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 3,
                height: 5,
                data,
                ..
            } if *rect == Rect::new(0, 0, 3, 5) && data == &expected
        )),
        "{cmds:?}"
    );
}

#[test]
fn gtui_render_decodes_adam7_interlaced_png_graphic_data() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 8,
                    height: 8,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgba_adam7(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    let image = cmds
        .iter()
        .find_map(|command| match command {
            RenderCommand::RgbaImage {
                rect,
                width,
                height,
                data,
                ..
            } if *rect == Rect::new(0, 0, 8, 8) && *width == 8 && *height == 8 => Some(data),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Adam7 PNG GraphicData should decode to RGBA image: {cmds:?}"));
    for (x, y) in [(0usize, 0usize), (4, 0), (0, 4), (2, 2), (7, 7)] {
        let offset = (y * 8 + x) * 4;
        assert_eq!(&image[offset..offset + 4], &adam7_test_pixel(x, y));
    }

    let fb = FramebufferRenderer::default()
        .render_scene(&scene)
        .expect("Adam7 PNG renders to framebuffer");
    assert_eq!(fb.pixel(7, 7), Some(Colour::rgb(119, 133, 142)));
}

#[test]
fn gtui_render_decodes_indexed_colour_png_graphic_data() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 4,
                    height: 1,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_indexed_2bit(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 4,
                height: 1,
                data,
                ..
            } if *rect == Rect::new(0, 0, 4, 1)
                && data.as_slice()
                    == [
                        0xFF, 0x00, 0x00, 0xFF,
                        0x00, 0xFF, 0x00, 0xFF,
                        0x00, 0x00, 0xFF, 0xFF,
                        0xFF, 0xFF, 0x00, 0x00,
                    ]
        )),
        "{cmds:?}"
    );
    let fb = FramebufferRenderer::default()
        .render_scene(&scene)
        .expect("indexed PNG renders to framebuffer");
    assert_eq!(fb.pixel(0, 0), Some(Colour::rgb(0xFF, 0x00, 0x00)));
    assert_eq!(fb.pixel(1, 0), Some(Colour::rgb(0x00, 0xFF, 0x00)));
    assert_eq!(fb.pixel(2, 0), Some(Colour::rgb(0x00, 0x00, 0xFF)));
    assert_eq!(
        fb.pixel(3, 0),
        Some(Palette::default_isobus().resolve(0)),
        "tRNS alpha zero should leave the scene background visible"
    );
}

#[test]
fn gtui_render_decodes_sub_byte_grayscale_png_graphic_data_with_trns() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 2,
                    height: 1,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_grayscale_1bit_trns(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 2,
                height: 1,
                data,
                ..
            } if *rect == Rect::new(0, 0, 2, 1)
                && data.as_slice() == [0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]
        )),
        "{cmds:?}"
    );
    let fb = FramebufferRenderer::default()
        .render_scene(&scene)
        .expect("grayscale PNG renders to framebuffer");
    assert_eq!(fb.pixel(0, 0), Some(Colour::rgb(0, 0, 0)));
    assert_eq!(
        fb.pixel(1, 0),
        Some(Palette::default_isobus().resolve(0)),
        "grayscale tRNS should make the white sample transparent"
    );
}

#[test]
fn gtui_render_decodes_grayscale_alpha_png_graphic_data() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 2,
                    height: 1,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_grayscale_alpha_8bit(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 2,
                height: 1,
                data,
                ..
            } if *rect == Rect::new(0, 0, 2, 1)
                && data.as_slice() == [0x44, 0x44, 0x44, 0x80, 0xCC, 0xCC, 0xCC, 0xFF]
        )),
        "{cmds:?}"
    );
    let fb = FramebufferRenderer::default()
        .render_scene(&scene)
        .expect("grayscale-alpha PNG renders to framebuffer");
    assert_eq!(fb.pixel(1, 0), Some(Colour::rgb(0xCC, 0xCC, 0xCC)));
}

#[test]
fn gtui_render_decodes_true_colour_png_graphic_data_with_trns() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 2,
                    height: 1,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgb_8bit_trns(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                rect,
                width: 2,
                height: 1,
                data,
                ..
            } if *rect == Rect::new(0, 0, 2, 1)
                && data.as_slice() == [0x11, 0x22, 0x33, 0x00, 0xAA, 0xBB, 0xCC, 0xFF]
        )),
        "{cmds:?}"
    );
    let fb = FramebufferRenderer::default()
        .render_scene(&scene)
        .expect("true-colour PNG renders to framebuffer");
    assert_eq!(
        fb.pixel(0, 0),
        Some(Palette::default_isobus().resolve(0)),
        "true-colour tRNS should leave the scene background visible"
    );
    assert_eq!(fb.pixel(1, 0), Some(Colour::rgb(0xAA, 0xBB, 0xCC)));
}

#[test]
fn gtui_render_decodes_16bit_grayscale_alpha_png_graphic_data() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 2,
                    height: 1,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_grayscale_alpha_16bit(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage {
                object_id: ObjectID(3),
                width: 2,
                height: 1,
                data,
                ..
            } if data.as_slice() == [0x12, 0x12, 0x12, 0x80, 0xDE, 0xDE, 0xDE, 0x7F]
        )),
        "{cmds:?}"
    );
}

#[test]
fn gtui_render_decodes_16bit_rgb_png_graphic_data_by_downsampling() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 2,
                    height: 1,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: overwide_png_rgb_16bit(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    // 16-bit RGB is decoded by down-sampling each channel to its high byte and
    // assigning opaque alpha: pixel0 (0x1234,0xABCD,0x5678) -> 12 AB 56 FF,
    // pixel1 (0xDEAD,0xBEEF,0xCAFE) -> DE BE CA FF.
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage { width: 2, height: 1, data, .. }
                if *data == [0x12u8, 0xAB, 0x56, 0xFF, 0xDE, 0xBE, 0xCA, 0xFF]
        )),
        "{cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, RenderCommand::Placeholder { .. })),
        "a decodable 16-bit RGB PNG must not produce a placeholder: {cmds:?}"
    );
}

#[test]
fn gtui_render_emits_precise_placeholder_for_malformed_png_graphic_data() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 8,
                    height: 4,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: b"not-png".to_vec(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(cmds.iter().any(|c| matches!(
        c,
        RenderCommand::Placeholder {
            object_type: ObjectType::ScaledGraphic,
            reason,
            ..
        } if reason.contains("malformed standard PNG GraphicData")
            && reason.contains("too short")
    )));
}

#[test]
fn gtui_render_rejects_png_graphic_data_with_bad_chunk_crc() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 1,
                    height: 1,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: png_with_corrupt_idat_crc(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(cmds.iter().any(|c| matches!(
        c,
        RenderCommand::Placeholder {
            object_type: ObjectType::ScaledGraphic,
            reason,
            ..
        } if reason.contains("malformed standard PNG GraphicData")
            && reason.contains("CRC check failed")
    )));
}

#[test]
fn gtui_render_rejects_non_standard_graphic_data_format() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 8,
                    height: 4,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(VTObject {
            id: ObjectID::new(4),
            r#type: ObjectType::GraphicData,
            body: vec![2, 0, 0, 0, 0],
            children: Vec::new(),
            children_pos: Vec::new(),
            macros: Vec::new(),
        });
    let scene = render(&pool, ObjectID::NULL);
    assert!(scene.unsupported.iter().any(|record| {
        record.id == ObjectID::new(3)
            && record.object_type == ObjectType::ScaledGraphic
            && record.reason.contains("undecodable GraphicData body")
    }));
}

#[test]
fn gtui_render_decodes_scaled_bitmap_indexed_graphic_data() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_bitmap(
                3,
                &ScaledBitmapBody {
                    width: 6,
                    height: 2,
                    offset_x: 4,
                    offset_y: -1,
                    format: 2,
                    bitmap_data: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    // 6x2 8-bit-indexed payload (12 bytes): the ScaledBitmap
                    // interprets GraphicData bytes as a raw indexed bitmap, not
                    // as PNG.
                    data: (0u8..12).collect(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    // ScaledBitmap is now a drawable machbus compatibility extension, so no
    // unsupported record is produced.
    assert!(
        !scene
            .unsupported
            .iter()
            .any(|record| record.object_type == ObjectType::ScaledBitmap),
        "{:?}",
        scene.unsupported
    );
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::IndexedImage {
                object_id,
                width: 6,
                height: 2,
                format: 2,
                data,
                ..
            } if *object_id == ObjectID::new(3) && *data == (0u8..12).collect::<Vec<u8>>()
        )),
        "{cmds:?}"
    );
}

#[test]
fn gtui_render_emits_placeholder_for_short_uncompressed_picture_graphic() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 4,
                    actual_width: 4,
                    actual_height: 4,
                    format: 2,
                    data: vec![1, 2],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(cmds.iter().any(|c| matches!(
        c,
        RenderCommand::Placeholder {
            object_type: ObjectType::PictureGraphic,
            reason,
            ..
        } if reason.contains("short bitmap payload")
    )));
}

#[test]
fn gtui_render_ignores_extra_uncompressed_picture_graphic_bytes() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 4,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    data: vec![1, 2, 3, 4, 5],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(!cmds.iter().any(|c| matches!(
        c,
        RenderCommand::Placeholder {
            object_type: ObjectType::PictureGraphic,
            ..
        }
    )));
    assert!(cmds.iter().any(|c| matches!(
        c,
        RenderCommand::IndexedImage {
            object_id,
            width: 2,
            height: 2,
            data,
            ..
        } if *object_id == ObjectID::new(3) && data == &[1, 2, 3, 4]
    )));
}

#[test]
fn gtui_render_emits_placeholder_for_invalid_compressed_picture_graphic() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    options: 0x04,
                    data: vec![0x11],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(cmds.iter().any(|c| matches!(
        c,
        RenderCommand::Placeholder {
            object_type: ObjectType::PictureGraphic,
            reason,
            ..
        } if reason.contains("invalid compressed")
    )));
}

#[test]
fn gtui_render_ignores_extra_decoded_compressed_picture_graphic_bytes() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    options: 0x04,
                    data: vec![5, 7],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(!cmds.iter().any(|c| matches!(
        c,
        RenderCommand::Placeholder {
            object_type: ObjectType::PictureGraphic,
            ..
        }
    )));
    assert!(cmds.iter().any(|c| matches!(
        c,
        RenderCommand::IndexedImage {
            object_id,
            width: 2,
            height: 2,
            data,
            ..
        } if *object_id == ObjectID::new(3) && data == &[7, 7, 7, 7]
    )));
}

#[test]
fn gtui_render_ignores_extra_picture_graphic_bytes_for_scaled_graphic_sources() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 4,
                    height: 4,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    options: 0,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                4,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    data: vec![9, 8, 7, 6, 5],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(!cmds.iter().any(|c| matches!(
        c,
        RenderCommand::Placeholder {
            object_type: ObjectType::ScaledGraphic,
            ..
        }
    )));
    assert!(cmds.iter().any(|c| matches!(
        c,
        RenderCommand::IndexedImage {
            object_id,
            rect,
            width: 2,
            height: 2,
            data,
            ..
        } if *object_id == ObjectID::new(3)
            && *rect == Rect::new(0, 0, 4, 4)
            && data == &[9, 8, 7, 6]
    )));
}

#[test]
fn framebuffer_uses_picture_graphic_padding_at_the_end_of_each_row() {
    let mono_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 10,
                    actual_width: 10,
                    actual_height: 2,
                    format: 0,
                    data: vec![0x80, 0x00, 0x40, 0x00],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mono_scene = render(&mono_pool, ObjectID::NULL);
    let mono_commands = GtuiRenderer::default().render(&mono_scene);
    assert!(mono_commands.iter().any(|c| matches!(
        c,
        RenderCommand::IndexedImage {
            object_id,
            width: 10,
            height: 2,
            data,
            ..
        } if *object_id == ObjectID::new(3) && data == &[0x80, 0x00, 0x40, 0x00]
    )));
    let mono_fb = FramebufferRenderer::default()
        .render_scene(&mono_scene)
        .expect("monochrome row-padded image renders");
    assert_eq!(mono_fb.pixel(0, 0), Some(Colour::rgb(0, 0, 0)));
    assert_eq!(mono_fb.pixel(1, 1), Some(Colour::rgb(0, 0, 0)));
    assert_eq!(mono_fb.pixel(1, 0), Some(Colour::rgb(255, 255, 255)));
    assert_eq!(mono_fb.pixel(0, 1), Some(Colour::rgb(255, 255, 255)));

    let indexed_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 3,
                    actual_width: 3,
                    actual_height: 2,
                    format: 1,
                    data: vec![0x12, 0x30, 0x45, 0x60],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let indexed_scene = render(&indexed_pool, ObjectID::NULL);
    let indexed_fb = FramebufferRenderer::default()
        .render_scene(&indexed_scene)
        .expect("4-bit row-padded image renders");
    let palette = Palette::default();
    assert_eq!(indexed_fb.pixel(0, 1), Some(palette.resolve(4)));
    assert_eq!(indexed_fb.pixel(1, 1), Some(palette.resolve(5)));
    assert_eq!(indexed_fb.pixel(2, 1), Some(palette.resolve(6)));
}

#[test]
fn gtui_render_soft_keys_emit_softkey_commands() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(6, &KeyBody::default()));
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, RenderCommand::SoftKey { .. }))
    );
}

#[test]
fn gtui_solid_style_helper_builds_style() {
    use machbus::isobus::vt::render::gtui::solid_style;
    let s = solid_style(Colour::rgb(1, 2, 3), Colour::rgb(4, 5, 6));
    assert_eq!(s.foreground, Colour::rgb(1, 2, 3));
    assert_eq!(s.background, Colour::rgb(4, 5, 6));
}

#[test]
fn scene_rect_contains_and_translate_helpers() {
    let r = Rect::new(5, 6, 10, 20);
    assert!(r.contains(5, 6));
    assert!(!r.contains(15, 6));
    let r2 = r.translate(1, 1);
    assert_eq!(r2.x, 6);
    assert_eq!(r2.y, 7);
}

// ─── Reference cycle guard helpers ─────────────────────────────────

fn option_object_type(raw: u8) -> Option<ObjectType> {
    ObjectType::try_from_u8(raw)
}

#[test]
fn gtui_render_decodes_scaled_bitmap_24bit_rgb_graphic_data() {
    // ScaledBitmap format 3 is direct 24-bit RGB: 2x1 pixels (6 bytes) expand
    // to opaque RGBA8.
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_bitmap(
                3,
                &ScaledBitmapBody {
                    width: 2,
                    height: 1,
                    format: 3,
                    bitmap_data: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66],
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage { width: 2, height: 1, data, .. }
                if *data == [0x11, 0x22, 0x33, 0xFF, 0x44, 0x55, 0x66, 0xFF]
        )),
        "{cmds:?}"
    );
}
