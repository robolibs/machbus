fn overwide_png_rgba_16bit() -> Vec<u8> {
    let raw = [
        0, // filter
        0x12, 0x34, // red
        0x56, 0x78, // green
        0x9A, 0xBC, // blue
        0xDE, 0xF0, // alpha
    ];
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 16, 6, 0, 0, 0]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

#[test]
fn gtui_render_decodes_16bit_rgba_png_graphic_data_by_downsampling() {
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
                    data: overwide_png_rgba_16bit(),
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let cmds = GtuiRenderer::default().render(&scene);
    // 16-bit RGBA is decoded by down-sampling each channel to its high byte:
    // R 0x1234 -> 0x12, G 0x5678 -> 0x56, B 0x9ABC -> 0x9A, A 0xDEF0 -> 0xDE.
    assert!(
        cmds.iter().any(|c| matches!(
            c,
            RenderCommand::RgbaImage { width: 1, height: 1, data, .. }
                if *data == [0x12u8, 0x56, 0x9A, 0xDE]
        )),
        "{cmds:?}"
    );
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, RenderCommand::Placeholder { .. })),
        "a decodable 16-bit RGBA PNG must not produce a placeholder: {cmds:?}"
    );
}
