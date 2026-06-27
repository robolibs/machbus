#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::vt::render::gtui::GraphicsContextCopySource;
    use crate::isobus::vt::render::scene::{NodeKind, SceneNode};
    use crate::isobus::vt::render::style::{FontMetrics, ResolvedStyle};
    use crate::isobus::vt::render::text::{self, HorizontalAlign, VerticalAlign};
    use crate::isobus::vt::{ObjectID, ObjectType};

    #[test]
    fn framebuffer_rasterises_basic_commands_with_clip() {
        let red = Colour::rgb(255, 0, 0);
        let blue = Colour::rgb(0, 0, 255);
        let commands = vec![
            RenderCommand::Clip(Rect::new(2, 2, 5, 5)),
            RenderCommand::FillRect {
                rect: Rect::new(0, 0, 10, 10),
                colour: red,
            },
            RenderCommand::Line {
                x0: 2,
                y0: 2,
                x1: 6,
                y1: 6,
                colour: blue,
                width: 1,
                line_art: 0xFFFF,
            },
        ];

        let fb = FramebufferRenderer::default()
            .render_commands(10, 10, &commands)
            .unwrap();

        assert_eq!(fb.pixel(1, 1), Some(Colour::rgb(255, 255, 255)));
        assert_eq!(fb.pixel(3, 3), Some(blue));
        assert!(fb.count_colour(red) > 0);
    }

    #[test]
    fn framebuffer_expands_indexed_images_and_text_cells() {
        let black = Colour::rgb(0, 0, 0);
        let layout = text::layout_text(
            "A",
            FontMetrics::default(),
            20,
            12,
            HorizontalAlign::Left,
            VerticalAlign::Top,
            false,
        );
        let commands = vec![
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 0, 2, 1),
                width: 2,
                height: 1,
                format: 0,
                transparent: false,
                transparency: 255,
                data: vec![0b1000_0000],
            },
            RenderCommand::DrawText {
                rect: Rect::new(0, 2, 20, 12),
                text: "A".into(),
                style: ResolvedStyle::default(),
                align: HorizontalAlign::Left,
                layout,
            },
        ];

        let fb = FramebufferRenderer::default()
            .render_commands(20, 20, &commands)
            .unwrap();

        assert_eq!(fb.pixel(0, 0), Some(black));
        assert_eq!(fb.pixel(1, 0), Some(Colour::rgb(255, 255, 255)));
        assert!(fb.count_colour(black) > 1);
    }

    #[test]
    fn framebuffer_expands_indexed_images_with_renderer_palette() {
        let calibrated = Colour::rgb(12, 34, 56);
        let background = Colour::rgb(9, 9, 9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, calibrated);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), background);
        let commands = vec![
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 0, 1, 1),
                width: 1,
                height: 1,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![2],
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(43),
                rect: Rect::new(1, 0, 1, 1),
                width: 1,
                height: 1,
                format: 2,
                transparent: true,
                transparency: 2,
                data: vec![2],
            },
        ];

        let fb = renderer.render_commands(2, 1, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(calibrated));
        assert_eq!(fb.pixel(1, 0), Some(background));
    }

    #[test]
    fn framebuffer_applies_picture_graphic_updates_in_command_order() {
        let red = Colour::rgb(200, 0, 0);
        let green = Colour::rgb(0, 200, 0);
        let mut palette = Palette::default_isobus();
        palette.set_entry(1, red);
        palette.set_entry(2, green);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 0, 1, 1),
                width: 1,
                height: 1,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![1],
            },
            RenderCommand::GraphicsContextPictureData {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                width: 1,
                height: 1,
                format: 2,
                transparent_index: 255,
                data: vec![2],
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(1, 0, 1, 1),
                width: 1,
                height: 1,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![1],
            },
        ];

        let fb = renderer.render_commands(2, 1, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(red));
        assert_eq!(fb.pixel(1, 0), Some(green));
    }

    #[test]
    fn framebuffer_picture_update_treats_out_of_range_colour_as_transparent() {
        let base = Colour::rgb(4, 5, 6);
        let copied = Colour::rgb(7, 8, 9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(1, base);
        palette.set_entry(2, copied);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextPictureData {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                width: 1,
                height: 1,
                format: 2,
                transparent_index: 0,
                data: vec![2],
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 0, 1, 1),
                width: 1,
                height: 1,
                format: 0,
                transparent: false,
                transparency: 0,
                data: vec![0b1000_0000],
            },
        ];

        let fb = renderer.render_commands(1, 1, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(base));
        assert_eq!(fb.count_colour(copied), 0);
    }

    #[test]
    fn framebuffer_copies_direct_graphics_context_pixels_to_picture() {
        let copied = Colour::rgb(1, 2, 3);
        let fallback = Colour::rgb(4, 5, 6);
        let mut palette = Palette::default_isobus();
        palette.set_entry(1, fallback);
        palette.set_entry(2, copied);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 3, 3),
                canvas_width: 3,
                canvas_height: 3,
                background: 0,
                transparency_colour: 7,
                transparent: false,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![1, 0, 1, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Viewport,
                viewport: Rect::new(1, 1, 1, 1),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 0, 1, 1),
                width: 1,
                height: 1,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![1],
            },
        ];

        let fb = renderer.render_commands(3, 3, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(copied));
    }

    #[test]
    fn framebuffer_copy_canvas_uses_graphics_context_canvas_not_visible_outline() {
        let canvas_zero = Colour::rgb(0, 0, 0);
        let outline = Colour::gray(96);
        let background = Colour::rgb(250, 250, 250);
        let mut palette = Palette::default_isobus();
        palette.set_entry(0, canvas_zero);
        palette.set_entry(4, outline);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), background);
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 3, 3),
                canvas_width: 3,
                canvas_height: 3,
                background: 0,
                transparency_colour: 7,
                transparent: false,
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                viewport: Rect::new(0, 0, 3, 3),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(4, 0, 1, 1),
                width: 1,
                height: 1,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![4],
            },
        ];

        let fb = renderer.render_commands(5, 3, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(outline));
        assert_eq!(fb.pixel(4, 0), Some(canvas_zero));
    }

    #[test]
    fn framebuffer_copy_canvas_to_larger_picture_leaves_extra_pixels_unchanged() {
        let base = Colour::rgb(3, 4, 5);
        let copied = Colour::rgb(7, 8, 9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(1, base);
        palette.set_entry(2, copied);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 1, 1),
                canvas_width: 1,
                canvas_height: 1,
                background: 0,
                transparency_colour: 0xFF,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                viewport: Rect::new(0, 0, 1, 1),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(2, 0, 2, 2),
                width: 2,
                height: 2,
                format: 2,
                transparent: false,
                transparency: 0xFF,
                data: vec![1; 4],
            },
        ];

        let fb = renderer.render_commands(4, 2, &commands).unwrap();

        assert_eq!(fb.pixel(2, 0), Some(copied));
        assert_eq!(fb.pixel(3, 0), Some(base));
        assert_eq!(fb.pixel(2, 1), Some(base));
        assert_eq!(fb.pixel(3, 1), Some(base));
    }

    #[test]
    fn framebuffer_copy_canvas_without_backing_does_not_sample_visible_pixels() {
        let base = Colour::rgb(4, 5, 6);
        let outline = Colour::gray(96);
        let mut palette = Palette::default_isobus();
        palette.set_entry(1, base);
        palette.set_entry(2, outline);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 1, 1),
                canvas_width: u16::MAX,
                canvas_height: u16::MAX,
                background: 0,
                transparency_colour: 0xFF,
                transparent: true,
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                viewport: Rect::new(0, 0, 1, 1),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(2, 0, 1, 1),
                width: 1,
                height: 1,
                format: 2,
                transparent: false,
                transparency: 0xFF,
                data: vec![1],
            },
        ];

        let fb = renderer.render_commands(3, 1, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(outline));
        assert_eq!(fb.pixel(2, 0), Some(base));
    }

    #[test]
    fn framebuffer_picture_updates_clip_to_picture_pixel_coordinates() {
        let mut palette = Palette::default_isobus();
        for (index, colour) in [
            (1, Colour::rgb(1, 1, 1)),
            (2, Colour::rgb(2, 2, 2)),
            (3, Colour::rgb(3, 3, 3)),
            (4, Colour::rgb(4, 4, 4)),
            (5, Colour::rgb(5, 5, 5)),
            (6, Colour::rgb(6, 6, 6)),
            (7, Colour::rgb(7, 7, 7)),
            (8, Colour::rgb(8, 8, 8)),
        ] {
            palette.set_entry(index, colour);
        }
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextPictureData {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                width: 4,
                height: 4,
                format: 2,
                transparent_index: 0xFF,
                data: vec![2, 3, 6, 6, 4, 5, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8],
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 0, 2, 2),
                width: 2,
                height: 2,
                format: 2,
                transparent: false,
                transparency: 0xFF,
                data: vec![1; 4],
            },
        ];

        let fb = renderer.render_commands(2, 2, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(Colour::rgb(2, 2, 2)));
        assert_eq!(fb.pixel(1, 0), Some(Colour::rgb(3, 3, 3)));
        assert_eq!(fb.pixel(0, 1), Some(Colour::rgb(4, 4, 4)));
        assert_eq!(fb.pixel(1, 1), Some(Colour::rgb(5, 5, 5)));
    }

    #[test]
    fn framebuffer_direct_graphics_context_line_attribute_selection_can_restore_lines() {
        let line_colour = Colour::rgb(2, 4, 6);
        let background = Colour::rgb(0, 0, 0);
        let mut palette = Palette::default_isobus();
        palette.set_entry(0, background);
        palette.set_entry(2, line_colour);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 4, 2),
                canvas_width: 4,
                canvas_height: 2,
                background: 0,
                transparency_colour: 0,
                transparent: false,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x04,
                payload: ObjectID::NULL.to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x09,
                payload: vec![3, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x04,
                payload: 99u16.to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 1, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x09,
                payload: vec![3, 0, 0, 0],
            },
        ];

        let fb = renderer.render_commands(4, 2, &commands).unwrap();

        assert_ne!(fb.pixel(0, 0), Some(line_colour));
        assert_ne!(fb.pixel(3, 0), Some(line_colour));
        assert_eq!(fb.pixel(0, 1), Some(line_colour));
        assert_eq!(fb.pixel(1, 1), Some(line_colour));
        assert_eq!(fb.pixel(2, 1), Some(line_colour));
        assert_eq!(fb.pixel(3, 1), Some(line_colour));
    }

    #[test]
    fn framebuffer_direct_graphics_context_null_font_resets_draw_text_colour() {
        let background = Colour::rgb(0, 0, 0);
        let default_text = Colour::rgb(1, 2, 3);
        let drawing_foreground = Colour::rgb(9, 8, 7);
        let mut palette = Palette::default_isobus();
        palette.set_entry(0, background);
        palette.set_entry(1, default_text);
        palette.set_entry(3, drawing_foreground);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 20, 20),
                canvas_width: 20,
                canvas_height: 20,
                background: 0,
                transparency_colour: 0xFF,
                transparent: false,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![3],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x06,
                payload: ObjectID::NULL.to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0D,
                payload: vec![1, 1, b'A'],
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                viewport: Rect::new(0, 0, 20, 20),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(21, 0, 20, 20),
                width: 20,
                height: 20,
                format: 2,
                transparent: false,
                transparency: 0xFF,
                data: vec![0; 400],
            },
        ];

        let fb = renderer.render_commands(41, 20, &commands).unwrap();

        assert_eq!(fb.pixel(0, 3), Some(default_text));
        assert_eq!(fb.pixel(21, 0), Some(default_text));
        assert_eq!(fb.count_colour(drawing_foreground), 0);
    }

    #[test]
    fn framebuffer_direct_graphics_context_fill_attribute_fills_rectangles_and_copy_canvas() {
        let fill_colour = Colour::rgb(8, 9, 10);
        let fallback = Colour::rgb(1, 1, 1);
        let mut palette = Palette::default_isobus();
        palette.set_entry(0, fallback);
        palette.set_entry(2, fill_colour);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 3, 2),
                canvas_width: 3,
                canvas_height: 2,
                background: 0,
                transparency_colour: 0,
                transparent: false,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x03,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x04,
                payload: ObjectID::NULL.to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x05,
                payload: 99u16.to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0A,
                payload: [3u16.to_le_bytes(), 2u16.to_le_bytes()].concat(),
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                viewport: Rect::new(0, 0, 3, 2),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(4, 0, 3, 2),
                width: 3,
                height: 2,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![0; 6],
            },
        ];

        let fb = renderer.render_commands(7, 2, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(fill_colour));
        assert_eq!(fb.pixel(2, 1), Some(fill_colour));
        assert_eq!(fb.pixel(4, 0), Some(fill_colour));
        assert_eq!(fb.pixel(6, 1), Some(fill_colour));
    }

    #[test]
    fn framebuffer_direct_graphics_context_fill_attribute_fills_curved_and_polygon_copy_canvas() {
        let fill_colour = Colour::rgb(11, 12, 13);
        let fallback = Colour::rgb(1, 1, 1);
        let mut palette = Palette::default_isobus();
        palette.set_entry(0, fallback);
        palette.set_entry(2, fill_colour);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let mut polygon_payload = vec![4];
        polygon_payload.extend_from_slice(&4i16.to_le_bytes());
        polygon_payload.extend_from_slice(&0i16.to_le_bytes());
        polygon_payload.extend_from_slice(&4i16.to_le_bytes());
        polygon_payload.extend_from_slice(&4i16.to_le_bytes());
        polygon_payload.extend_from_slice(&0i16.to_le_bytes());
        polygon_payload.extend_from_slice(&4i16.to_le_bytes());
        polygon_payload.extend_from_slice(&0i16.to_le_bytes());
        polygon_payload.extend_from_slice(&0i16.to_le_bytes());
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 13, 5),
                canvas_width: 13,
                canvas_height: 5,
                background: 0,
                transparency_colour: 0,
                transparent: false,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x03,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x04,
                payload: ObjectID::NULL.to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x05,
                payload: 99u16.to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0B,
                payload: [5u16.to_le_bytes(), 5u16.to_le_bytes()].concat(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![7, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0C,
                payload: polygon_payload,
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                viewport: Rect::new(0, 0, 13, 5),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(14, 0, 13, 5),
                width: 13,
                height: 5,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![0; 65],
            },
        ];

        let fb = renderer.render_commands(27, 5, &commands).unwrap();

        assert_eq!(fb.pixel(2, 2), Some(fill_colour));
        assert_eq!(fb.pixel(16, 2), Some(fill_colour));
        assert_eq!(fb.pixel(9, 2), Some(fill_colour));
        assert_eq!(fb.pixel(23, 2), Some(fill_colour));
    }

    #[test]
    fn framebuffer_applies_zoom_when_copying_direct_graphics_context_viewport() {
        let first = Colour::rgb(1, 2, 3);
        let second = Colour::rgb(4, 5, 6);
        let fallback = Colour::rgb(7, 8, 9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(1, fallback);
        palette.set_entry(2, first);
        palette.set_entry(3, second);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 2, 2),
                canvas_width: 2,
                canvas_height: 2,
                background: 0,
                transparency_colour: 0,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![3],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![1, 0, 0, 0],
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Viewport,
                viewport: Rect::new(0, 0, 2, 1),
                zoom_raw: Some(2.0f32.to_bits()),
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 1, 2, 1),
                width: 2,
                height: 1,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![1, 1],
            },
        ];

        let fb = renderer.render_commands(2, 2, &commands).unwrap();

        assert_eq!(fb.pixel(0, 1), Some(first));
        assert_eq!(fb.pixel(1, 1), Some(first));
        assert_eq!(fb.pixel(1, 0), Some(second));
    }

    #[test]
    fn framebuffer_remembers_graphics_context_zoom_replay_for_later_copy_viewport() {
        let first = Colour::rgb(1, 2, 3);
        let second = Colour::rgb(4, 5, 6);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, first);
        palette.set_entry(3, second);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 2, 1),
                canvas_width: 2,
                canvas_height: 1,
                background: 0,
                transparency_colour: 0,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![3],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![1, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0F,
                payload: 2.0f32.to_bits().to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Viewport,
                viewport: Rect::new(0, 0, 2, 1),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 1, 2, 1),
                width: 2,
                height: 1,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![0, 0],
            },
        ];

        let fb = renderer.render_commands(2, 2, &commands).unwrap();

        assert_eq!(fb.pixel(0, 1), Some(first));
        assert_eq!(fb.pixel(1, 1), Some(first));
        assert_eq!(fb.pixel(1, 0), Some(second));
    }

    #[test]
    fn framebuffer_replays_basic_graphics_context_subcommands() {
        let foreground = Colour::rgb(1, 2, 3);
        let background = Colour::rgb(4, 5, 6);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, foreground);
        palette.set_entry(3, background);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 5, 5),
                canvas_width: 5,
                canvas_height: 5,
                background: 0,
                transparency_colour: 0,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x03,
                payload: vec![3],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![1, 0, 1, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![2, 0, 2, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x07,
                payload: vec![2, 0, 2, 0],
            },
        ];

        let fb = renderer.render_commands(5, 5, &commands).unwrap();

        assert_eq!(fb.pixel(1, 1), Some(foreground));
        assert_eq!(fb.pixel(2, 2), Some(background));
        assert_eq!(fb.pixel(3, 3), Some(background));
    }

    #[test]
    fn framebuffer_clips_graphics_context_replay_to_viewport() {
        let replay_colour = Colour::rgb(4, 5, 6);
        let background = Colour::gray(9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(3, replay_colour);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), background);
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(1, 1, 2, 2),
                canvas_width: 2,
                canvas_height: 2,
                background: 0,
                transparency_colour: 0,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x03,
                payload: vec![3],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x07,
                payload: vec![4, 0, 4, 0],
            },
        ];

        let fb = renderer.render_commands(5, 5, &commands).unwrap();

        assert_eq!(fb.pixel(1, 1), Some(replay_colour));
        assert_eq!(fb.pixel(2, 2), Some(replay_colour));
        assert_eq!(fb.pixel(0, 0), Some(background));
        assert_eq!(fb.pixel(3, 3), Some(background));
    }

    #[test]
    fn framebuffer_replays_graphics_context_ellipse_and_polygon_outline() {
        let foreground = Colour::rgb(1, 2, 3);
        let background = Colour::gray(9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, foreground);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), background);
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 8, 8),
                canvas_width: 8,
                canvas_height: 8,
                background: 0,
                transparency_colour: 0,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![1, 0, 1, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0B,
                payload: vec![4, 0, 4, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![1, 0, 6, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0C,
                payload: vec![3, 3, 0, 0, 0, 3, 0, 1, 0, 0, 0, 1, 0],
            },
        ];

        let fb = renderer.render_commands(8, 8, &commands).unwrap();

        assert_eq!(fb.pixel(2, 1), Some(foreground));
        assert_eq!(fb.pixel(1, 2), Some(foreground));
        assert_eq!(fb.pixel(2, 6), Some(foreground));
        assert_eq!(fb.pixel(4, 7), Some(foreground));
    }

    #[test]
    fn framebuffer_replays_graphics_context_draw_text() {
        let foreground = Colour::rgb(1, 2, 3);
        let text_background = Colour::rgb(4, 5, 6);
        let background = Colour::gray(9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, foreground);
        palette.set_entry(3, text_background);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), background);
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 30, 20),
                canvas_width: 30,
                canvas_height: 20,
                background: 0,
                transparency_colour: 0,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x03,
                payload: vec![3],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0D,
                payload: vec![0, 1, b'A'],
            },
        ];

        let fb = renderer.render_commands(30, 20, &commands).unwrap();

        assert_eq!(fb.pixel(0, 0), Some(text_background));
        assert_eq!(fb.pixel(0, 3), Some(foreground));
        assert_eq!(fb.pixel(4, 6), Some(foreground));
        assert_eq!(fb.pixel(10, 10), Some(text_background));
        assert_eq!(fb.pixel(29, 19), Some(text_background));
    }

    #[test]
    fn framebuffer_replays_graphics_context_viewport_updates() {
        let foreground = Colour::rgb(1, 2, 3);
        let viewport_background = Colour::rgb(4, 5, 6);
        let background = Colour::gray(9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, foreground);
        palette.set_entry(3, viewport_background);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), background);
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(1, 1, 4, 4),
                canvas_width: 4,
                canvas_height: 4,
                background: 0,
                transparency_colour: 0,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x03,
                payload: vec![3],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0E,
                payload: vec![1, 0, 1, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x11,
                payload: vec![1, 0, 1, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x07,
                payload: vec![3, 0, 3, 0],
            },
        ];

        let fb = renderer.render_commands(6, 6, &commands).unwrap();

        assert_eq!(fb.pixel(2, 2), Some(viewport_background));
        assert_eq!(fb.pixel(1, 1), Some(Colour::gray(96)));
        assert_eq!(fb.pixel(3, 3), Some(background));
    }

    #[test]
    fn framebuffer_replay_copy_canvas_updates_later_picture_draws() {
        let foreground = Colour::rgb(1, 2, 3);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, foreground);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 4, 4),
                canvas_width: 4,
                canvas_height: 4,
                background: 0,
                transparency_colour: 0xFF,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![1, 0, 1, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x13,
                payload: 30u16.to_le_bytes().to_vec(),
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(30),
                rect: Rect::new(5, 0, 4, 4),
                width: 4,
                height: 4,
                format: 2,
                transparent: false,
                transparency: 0xFF,
                data: vec![0; 16],
            },
        ];

        let fb = renderer.render_commands(9, 4, &commands).unwrap();

        assert_eq!(fb.pixel(1, 1), Some(foreground));
        assert_eq!(fb.pixel(6, 1), Some(foreground));
    }

    #[test]
    fn framebuffer_replay_copy_viewport_updates_later_picture_draws() {
        let foreground = Colour::rgb(1, 2, 3);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, foreground);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 4, 4),
                canvas_width: 4,
                canvas_height: 4,
                background: 0,
                transparency_colour: 0xFF,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x0E,
                payload: vec![1, 0, 1, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x11,
                payload: vec![2, 0, 2, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x14,
                payload: 31u16.to_le_bytes().to_vec(),
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(31),
                rect: Rect::new(5, 0, 2, 2),
                width: 2,
                height: 2,
                format: 2,
                transparent: false,
                transparency: 0xFF,
                data: vec![0; 4],
            },
        ];

        let fb = renderer.render_commands(7, 4, &commands).unwrap();

        assert_eq!(fb.pixel(1, 1), Some(foreground));
        assert_eq!(fb.pixel(5, 0), Some(foreground));
    }

    #[test]
    fn framebuffer_replays_null_graphics_context_line_attributes() {
        let foreground = Colour::rgb(1, 2, 3);
        let background = Colour::gray(9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, foreground);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), background);
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 5, 5),
                canvas_width: 5,
                canvas_height: 5,
                background: 0,
                transparency_colour: 0,
                transparent: true,
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x02,
                payload: vec![2],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x04,
                payload: ObjectID::NULL.to_le_bytes().to_vec(),
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x00,
                payload: vec![1, 0, 2, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x09,
                payload: vec![2, 0, 0, 0],
            },
            RenderCommand::GraphicsContextReplay {
                object_id: ObjectID::new(7),
                subcommand: 0x08,
                payload: vec![0, 0, 1, 0],
            },
        ];

        let fb = renderer.render_commands(5, 5, &commands).unwrap();

        assert_eq!(fb.pixel(1, 2), Some(background));
        assert_eq!(fb.pixel(2, 2), Some(background));
        assert_eq!(fb.pixel(3, 2), Some(background));
        assert_eq!(fb.pixel(3, 3), Some(foreground));
    }

    #[test]
    fn framebuffer_renders_opaque_graphics_context_canvas_surface() {
        let canvas_background = Colour::rgb(4, 5, 6);
        let frame_background = Colour::rgb(9, 9, 9);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, canvas_background);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), frame_background);
        let opaque = [RenderCommand::GraphicsContextCanvas {
            object_id: ObjectID::new(7),
            rect: Rect::new(0, 0, 3, 3),
            canvas_width: 3,
            canvas_height: 3,
            background: 2,
            transparency_colour: 0,
            transparent: false,
        }];
        let transparent = [RenderCommand::GraphicsContextCanvas {
            object_id: ObjectID::new(7),
            rect: Rect::new(0, 0, 3, 3),
            canvas_width: 3,
            canvas_height: 3,
            background: 2,
            transparency_colour: 0,
            transparent: true,
        }];

        let opaque_fb = renderer.render_commands(3, 3, &opaque).unwrap();
        let transparent_fb = renderer.render_commands(3, 3, &transparent).unwrap();

        assert_eq!(opaque_fb.pixel(1, 1), Some(canvas_background));
        assert_eq!(transparent_fb.pixel(1, 1), Some(frame_background));
    }

    #[test]
    fn framebuffer_copies_opaque_graphics_context_background_to_picture() {
        let canvas_background = Colour::rgb(4, 5, 6);
        let mut palette = Palette::default_isobus();
        palette.set_entry(2, canvas_background);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 2, 2),
                canvas_width: 2,
                canvas_height: 2,
                background: 2,
                transparency_colour: 0,
                transparent: false,
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                viewport: Rect::new(0, 0, 2, 2),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 0, 2, 2),
                width: 2,
                height: 2,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![0; 4],
            },
        ];

        let fb = renderer.render_commands(2, 2, &commands).unwrap();

        assert_eq!(fb.count_colour(canvas_background), 4);
    }

    #[test]
    fn framebuffer_does_not_copy_graphics_context_transparency_colour_to_picture() {
        let transparency_colour = Colour::rgb(7, 8, 9);
        let original_picture_colour = Colour::rgb(1, 2, 3);
        let mut palette = Palette::default_isobus();
        palette.set_entry(1, original_picture_colour);
        palette.set_entry(7, transparency_colour);
        let renderer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::gray(9));
        let commands = vec![
            RenderCommand::GraphicsContextCanvas {
                object_id: ObjectID::new(7),
                rect: Rect::new(0, 0, 2, 2),
                canvas_width: 2,
                canvas_height: 2,
                background: 2,
                transparency_colour: 7,
                transparent: true,
            },
            RenderCommand::GraphicsContextCopyToPicture {
                object_id: ObjectID::new(7),
                picture_id: ObjectID::new(42),
                source: GraphicsContextCopySource::Canvas,
                viewport: Rect::new(0, 0, 2, 2),
                zoom_raw: None,
            },
            RenderCommand::IndexedImage {
                object_id: ObjectID::new(42),
                rect: Rect::new(0, 0, 2, 2),
                width: 2,
                height: 2,
                format: 2,
                transparent: false,
                transparency: 255,
                data: vec![1; 4],
            },
        ];

        let fb = renderer.render_commands(2, 2, &commands).unwrap();

        assert_eq!(fb.count_colour(transparency_colour), 0);
        assert_eq!(fb.count_colour(original_picture_colour), 4);
    }

    #[test]
    fn framebuffer_renders_scene_via_command_stream() {
        let mut scene = Scene::new(ObjectID::new(1), (80, 40));
        scene.nodes.push(SceneNode {
            id: ObjectID::new(2),
            object_type: ObjectType::OutputString,
            parent: ObjectID::new(1),
            rect: Rect::new(5, 5, 40, 12),
            clip: None,
            style: ResolvedStyle::default(),
            visible: true,
            enabled: true,
            kind: NodeKind::OutputString {
                text: "VT".into(),
                transparent_bg: false,
                justification: 0,
            },
        });

        let fb = FramebufferRenderer::default().render_scene(&scene).unwrap();

        assert_eq!(fb.width(), 80);
        assert_eq!(fb.height(), 40);
        assert!(fb.count_colour(Colour::rgb(0, 0, 0)) > 0);
    }

    #[test]
    fn framebuffer_fallible_methods_report_empty_or_oversized_targets() {
        let renderer = FramebufferRenderer::default();

        assert_eq!(
            renderer.try_render_commands_auto_sized(&[]),
            Err(FramebufferError::InvalidCommandBounds)
        );
        assert_eq!(
            renderer.try_render_commands(u16::MAX, u16::MAX, &[]),
            Err(FramebufferError::FrameTooLarge)
        );
    }

    #[test]
    fn framebuffer_exports_rgb888_and_rgb565_for_display_handoff() {
        let commands = vec![
            RenderCommand::FillRect {
                rect: Rect::new(0, 0, 1, 1),
                colour: Colour::rgb(255, 0, 0),
            },
            RenderCommand::FillRect {
                rect: Rect::new(1, 0, 1, 1),
                colour: Colour::rgb(0, 255, 0),
            },
            RenderCommand::FillRect {
                rect: Rect::new(0, 1, 1, 1),
                colour: Colour::rgb(0, 0, 255),
            },
            RenderCommand::FillRect {
                rect: Rect::new(1, 1, 1, 1),
                colour: Colour::rgb(255, 255, 255),
            },
        ];
        let fb = FramebufferRenderer::default()
            .render_commands(2, 2, &commands)
            .unwrap();

        assert_eq!(
            fb.to_rgb888(),
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]
        );
        assert_eq!(
            fb.to_rgb565_be(),
            vec![0xF8, 0x00, 0x07, 0xE0, 0x00, 0x1F, 0xFF, 0xFF]
        );
        assert_eq!(
            fb.to_rgb565_le(),
            vec![0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0xFF, 0xFF]
        );

        let mut short = [0u8; 7];
        assert_eq!(
            fb.write_rgb565_be(&mut short),
            Err(FramebufferExportError::BufferTooSmall {
                required: 8,
                available: 7
            })
        );

        let mut exact = [0u8; 12];
        fb.write_rgb888(&mut exact).unwrap();
        assert_eq!(exact, [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
    }
}
