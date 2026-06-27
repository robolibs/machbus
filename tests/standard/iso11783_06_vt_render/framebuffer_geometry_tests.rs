#[test]
fn framebuffer_text_cells_follow_resolved_font_metrics_and_decorations() {
    let small_colour = Colour::rgb(10, 20, 30);
    let large_colour = Colour::rgb(40, 50, 60);
    let inverted_foreground = Colour::rgb(70, 80, 90);
    let inverted_background = Colour::rgb(90, 80, 70);
    let regular_colour = Colour::rgb(100, 20, 30);
    let bold_colour = Colour::rgb(110, 20, 30);
    let italic_colour = Colour::rgb(120, 20, 30);
    let small_style = ResolvedStyle {
        foreground: small_colour,
        font: FontMetrics::for_size(0),
        ..ResolvedStyle::default()
    };
    let large_style = ResolvedStyle {
        foreground: large_colour,
        font: FontMetrics::for_size(14),
        ..ResolvedStyle::default()
    };
    let mut inverted_style = ResolvedStyle {
        foreground: inverted_foreground,
        background: inverted_background,
        font: FontMetrics::for_size(4),
        ..ResolvedStyle::default()
    };
    inverted_style.decoration.inverted = true;
    inverted_style.decoration.underline = true;
    inverted_style.decoration.strikethrough = true;
    let regular_style = ResolvedStyle {
        foreground: regular_colour,
        font: FontMetrics::for_size(0),
        ..ResolvedStyle::default()
    };
    let mut bold_style = ResolvedStyle {
        foreground: bold_colour,
        font: FontMetrics::for_size(0),
        ..ResolvedStyle::default()
    };
    bold_style.decoration.bold = true;
    let mut italic_style = ResolvedStyle {
        foreground: italic_colour,
        font: FontMetrics::for_size(0),
        ..ResolvedStyle::default()
    };
    italic_style.decoration.italic = true;
    let commands = vec![
        RenderCommand::DrawText {
            rect: Rect::new(0, 0, 40, 40),
            text: "A".to_string(),
            style: small_style,
            align: HorizontalAlign::Left,
            layout: text::layout_text(
                "A",
                small_style.font,
                40,
                40,
                HorizontalAlign::Left,
                VerticalAlign::Top,
                false,
            ),
        },
        RenderCommand::DrawText {
            rect: Rect::new(50, 0, 40, 40),
            text: "A".to_string(),
            style: large_style,
            align: HorizontalAlign::Left,
            layout: text::layout_text(
                "A",
                large_style.font,
                40,
                40,
                HorizontalAlign::Left,
                VerticalAlign::Top,
                false,
            ),
        },
        RenderCommand::DrawText {
            rect: Rect::new(100, 0, 40, 40),
            text: "A".to_string(),
            style: inverted_style,
            align: HorizontalAlign::Left,
            layout: text::layout_text(
                "A",
                inverted_style.font,
                40,
                40,
                HorizontalAlign::Left,
                VerticalAlign::Top,
                false,
            ),
        },
        RenderCommand::DrawText {
            rect: Rect::new(150, 0, 40, 40),
            text: "A".to_string(),
            style: regular_style,
            align: HorizontalAlign::Left,
            layout: text::layout_text(
                "A",
                regular_style.font,
                40,
                40,
                HorizontalAlign::Left,
                VerticalAlign::Top,
                false,
            ),
        },
        RenderCommand::DrawText {
            rect: Rect::new(180, 0, 40, 40),
            text: "A".to_string(),
            style: bold_style,
            align: HorizontalAlign::Left,
            layout: text::layout_text(
                "A",
                bold_style.font,
                40,
                40,
                HorizontalAlign::Left,
                VerticalAlign::Top,
                false,
            ),
        },
        RenderCommand::DrawText {
            rect: Rect::new(210, 0, 40, 40),
            text: "A".to_string(),
            style: italic_style,
            align: HorizontalAlign::Left,
            layout: text::layout_text(
                "A",
                italic_style.font,
                40,
                40,
                HorizontalAlign::Left,
                VerticalAlign::Top,
                false,
            ),
        },
    ];

    let frame = FramebufferRenderer::default()
        .render_commands(260, 50, &commands)
        .expect("font-sized DrawText commands render");

    assert!(
        frame.count_colour(large_colour) > frame.count_colour(small_colour) * 2,
        "larger FontAttributes metrics should produce larger framebuffer text cells"
    );
    assert!(
        frame.count_colour(inverted_foreground) > 0 && frame.count_colour(inverted_background) > 0,
        "inverted decoration should swap the resolved foreground/background colours into text cells"
    );
    assert!(
        frame.count_colour(bold_colour) > frame.count_colour(regular_colour),
        "bold decoration should thicken deterministic text-cell coverage"
    );
    assert_ne!(
        frame.pixel(210, 2),
        Some(italic_colour),
        "italic decoration should skew the top text-cell rows"
    );
    assert_eq!(frame.pixel(212, 2), Some(italic_colour));
    assert_eq!(frame.pixel(210, 9), Some(italic_colour));
}

#[test]
fn framebuffer_honours_line_attribute_art_pattern() {
    let black = Colour::rgb(0, 0, 0);
    let framebuffer = FramebufferRenderer::default()
        .render_commands(
            8,
            1,
            &[RenderCommand::Line {
                x0: 0,
                y0: 0,
                x1: 7,
                y1: 0,
                colour: black,
                width: 1,
                line_art: 0xF0F0,
            }],
        )
        .unwrap();

    assert_eq!(framebuffer.pixel(0, 0), Some(black));
    assert_eq!(framebuffer.pixel(3, 0), Some(black));
    assert_ne!(framebuffer.pixel(4, 0), Some(black));
    assert_ne!(framebuffer.pixel(7, 0), Some(black));
}

#[test]
fn framebuffer_preserves_shape_stroke_widths() {
    let black = Colour::rgb(0, 0, 0);
    let framebuffer = FramebufferRenderer::default()
        .render_commands(
            16,
            16,
            &[
                RenderCommand::Ellipse {
                    rect: Rect::new(2, 2, 8, 8),
                    colour: black,
                    fill_colour: Colour::rgb(255, 255, 255),
                    filled: false,
                    width: 2,
                    line_art: 0xFFFF,
                },
                RenderCommand::Polygon {
                    origin: (0, 0),
                    points: vec![(2, 12), (10, 12)],
                    colour: black,
                    fill_colour: Colour::rgb(255, 255, 255),
                    filled: false,
                    width: 2,
                    line_art: 0xFFFF,
                },
            ],
        )
        .unwrap();

    assert_eq!(framebuffer.pixel(6, 2), Some(black));
    assert_eq!(
        framebuffer.pixel(6, 3),
        Some(black),
        "even ellipse stroke widths must affect adjacent pixels"
    );
    assert_eq!(
        framebuffer.pixel(6, 13),
        Some(black),
        "even polygon/line stroke widths must affect adjacent pixels"
    );
}

#[test]
fn framebuffer_uses_meter_and_bar_graph_geometry() {
    let black = Colour::rgb(0, 0, 0);
    let grey = Colour::gray(128);
    let framebuffer = FramebufferRenderer::default()
        .render_commands(
            32,
            32,
            &[
                RenderCommand::Meter {
                    rect: Rect::new(0, 0, 11, 11),
                    value: 0,
                    min: 0,
                    max: 100,
                    needle_colour: black,
                    border_colour: Colour::rgb(255, 0, 0),
                    arc_colour: grey,
                    show_value: true,
                    number_of_ticks: 3,
                    start_angle: 0,
                    end_angle: 45,
                },
                RenderCommand::BarGraph {
                    rect: Rect::new(12, 0, 8, 10),
                    value: 50,
                    target_value: 75,
                    min: 0,
                    max: 100,
                    arched: false,
                    colour: black,
                    target_line_colour: Colour::rgb(0, 255, 0),
                    show_border: true,
                    show_target_line: true,
                    show_ticks: true,
                    number_of_ticks: 3,
                    line_only: false,
                    horizontal: false,
                    direction_positive: true,
                    clockwise: false,
                    start_angle: 0,
                    end_angle: 0,
                    bar_width: 0,
                },
                RenderCommand::BarGraph {
                    rect: Rect::new(20, 0, 11, 11),
                    value: 100,
                    target_value: 50,
                    min: 0,
                    max: 100,
                    arched: true,
                    colour: black,
                    target_line_colour: Colour::rgb(0, 255, 0),
                    show_border: false,
                    show_target_line: true,
                    show_ticks: false,
                    number_of_ticks: 0,
                    line_only: false,
                    horizontal: false,
                    direction_positive: true,
                    clockwise: false,
                    start_angle: 0,
                    end_angle: 45,
                    bar_width: 2,
                },
            ],
        )
        .unwrap();

    assert_eq!(
        framebuffer.pixel(10, 5),
        Some(black),
        "meter value at the start angle should point east, not always straight up"
    );
    assert_eq!(
        framebuffer.pixel(0, 5),
        Some(Colour::rgb(255, 0, 0)),
        "meter border colour should be rendered independently from arc/needle colour"
    );
    assert_eq!(
        framebuffer.pixel(19, 9),
        Some(black),
        "vertical linear bar graphs fill from the bottom"
    );
    assert_ne!(
        framebuffer.pixel(18, 1),
        Some(black),
        "vertical linear bar graphs must not fill the whole height at half value"
    );
    assert_eq!(
        framebuffer.pixel(13, 2),
        Some(Colour::rgb(0, 255, 0)),
        "linear bar graph target values should render as independent target lines"
    );
    assert_eq!(
        framebuffer.pixel(30, 5),
        Some(black),
        "arched bar graph should rasterise the arc start"
    );
    assert_ne!(
        framebuffer.pixel(25, 5),
        Some(black),
        "arched bar graph should not degrade into a solid rectangular fill"
    );
}

#[test]
fn framebuffer_clamps_output_graphic_min_not_less_than_max_to_min_value() {
    let black = Colour::rgb(0, 0, 0);
    let green = Colour::rgb(0, 255, 0);
    let baseline = FramebufferRenderer::default()
        .render_commands(
            40,
            16,
            &[
                RenderCommand::Meter {
                    rect: Rect::new(0, 0, 11, 11),
                    value: 100,
                    min: 100,
                    max: 200,
                    needle_colour: black,
                    border_colour: black,
                    arc_colour: black,
                    show_value: false,
                    number_of_ticks: 0,
                    start_angle: 0,
                    end_angle: 45,
                },
                RenderCommand::BarGraph {
                    rect: Rect::new(14, 0, 8, 10),
                    value: 100,
                    target_value: 100,
                    min: 100,
                    max: 200,
                    arched: false,
                    colour: black,
                    target_line_colour: green,
                    show_border: false,
                    show_target_line: true,
                    show_ticks: false,
                    number_of_ticks: 0,
                    line_only: false,
                    horizontal: false,
                    direction_positive: true,
                    clockwise: false,
                    start_angle: 0,
                    end_angle: 0,
                    bar_width: 0,
                },
            ],
        )
        .unwrap();
    let min_not_less_than_max = FramebufferRenderer::default()
        .render_commands(
            40,
            16,
            &[
                RenderCommand::Meter {
                    rect: Rect::new(0, 0, 11, 11),
                    value: 200,
                    min: 100,
                    max: 10,
                    needle_colour: black,
                    border_colour: black,
                    arc_colour: black,
                    show_value: false,
                    number_of_ticks: 0,
                    start_angle: 0,
                    end_angle: 45,
                },
                RenderCommand::BarGraph {
                    rect: Rect::new(14, 0, 8, 10),
                    value: 200,
                    target_value: 200,
                    min: 100,
                    max: 10,
                    arched: false,
                    colour: black,
                    target_line_colour: green,
                    show_border: false,
                    show_target_line: true,
                    show_ticks: false,
                    number_of_ticks: 0,
                    line_only: false,
                    horizontal: false,
                    direction_positive: true,
                    clockwise: false,
                    start_angle: 0,
                    end_angle: 0,
                    bar_width: 0,
                },
            ],
        )
        .unwrap();

    assert_eq!(
        min_not_less_than_max.pixels(),
        baseline.pixels(),
        "Output Meter and Bar Graph rendering with min>=max must draw as if value/target value equals min"
    );
}

#[test]
fn render_bar_graphs_use_standard_target_and_option_fields() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(create_number_variable(
            10,
            &NumberVariableBody { value: 75 },
        ))
        .with_object(
            create_linear_bar_graph(
                3,
                &LinearBarGraphBody {
                    width: 20,
                    height: 10,
                    color: 1,
                    target_line_color: 2,
                    options: 0x3F,
                    number_of_ticks: 3,
                    min_value: 0,
                    max_value: 100,
                    value: 25,
                    target_value_variable_reference: ObjectID(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_arched_bar_graph(
                4,
                &ArchedBarGraphBody {
                    width: 20,
                    height: 20,
                    color: 3,
                    target_line_color: 4,
                    options: 0x1B,
                    start_angle: 0,
                    end_angle: 90,
                    bar_width: 2,
                    min_value: 0,
                    max_value: 100,
                    value: 25,
                    target_value: 50,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    let commands = GtuiRenderer::default().render(&scene);

    assert!(commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::BarGraph {
                value: 25,
                target_value: 75,
                arched: false,
                show_border: true,
                show_target_line: true,
                show_ticks: true,
                number_of_ticks: 3,
                line_only: true,
                horizontal: true,
                direction_positive: true,
                ..
            }
        )
    }));
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::BarGraph {
                value: 25,
                target_value: 50,
                arched: true,
                show_border: true,
                show_target_line: true,
                line_only: true,
                clockwise: true,
                start_angle: 0,
                end_angle: 90,
                bar_width: 2,
                ..
            }
        )
    }));
}

#[test]
fn render_open_ellipse_emits_arc_command_instead_of_full_ellipse() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_ellipse(
                3,
                &OutputEllipseBody {
                    width: 30,
                    height: 20,
                    ellipse_type: 1,
                    start_angle: 45,
                    end_angle: 135,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert!(matches!(
        &scene.find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputEllipse {
            closed: false,
            ellipse_type: 1,
            start_angle: 45,
            end_angle: 135,
            ..
        }
    ));

    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Ellipse { .. })),
        "open ellipse arcs must not be falsely rendered as full ellipses"
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::EllipseArc {
            rect: Rect { w: 30, h: 20, .. },
            ellipse_type: 1,
            start_angle: 45,
            end_angle: 135,
            ..
        }
    )));

    let arc_commands = commands
        .iter()
        .filter(|command| matches!(command, RenderCommand::EllipseArc { .. }))
        .cloned()
        .collect::<Vec<_>>();
    let framebuffer = FramebufferRenderer::default()
        .render_commands(40, 30, &arc_commands)
        .unwrap();
    let arc_pixels = framebuffer.count_colour(Colour::rgb(0, 0, 0));
    assert!(arc_pixels > 0, "arc rasterisation must draw visible pixels");
    assert!(
        arc_pixels < 30 * 20,
        "arc rasterisation must not fill the whole ellipse bounding box"
    );
}

#[test]
fn render_container_hidden_propagates_to_visibility() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_container(
                3,
                &ContainerBody {
                    width: 10,
                    height: 10,
                    hidden: true,
                },
            )
            .with_children([4u16]),
        )
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());
    let scene = render(&pool, ObjectID::NULL);
    let container = scene.find(ObjectID::new(3)).expect("container present");
    assert!(!container.visible, "hidden container must be invisible");
}

// ─── Active mask changes ───────────────────────────────────────────

#[test]
fn render_active_mask_change_swaps_visible_children() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 3u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(create_alarm_mask(3, &AlarmMaskBody::default()).unwrap())
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());

    let scene_a = render(&pool, ObjectID::new(2));
    assert_eq!(scene_a.active_mask, ObjectID::new(2));
    assert!(scene_a.nodes.iter().any(|n| n.id == 4));

    let scene_b = render(&pool, ObjectID::new(3));
    assert_eq!(scene_b.active_mask, ObjectID::new(3));
    // The output string from mask 2 is not present in mask 3's scene.
    assert!(!scene_b.nodes.iter().any(|n| n.id == 4));
}

#[test]
fn render_null_active_mask_falls_back_to_first_working_set_child() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    let scene = render(&pool, ObjectID::NULL);
    assert_eq!(scene.active_mask, ObjectID::new(2));
}

#[test]
fn render_runtime_rebuilds_scene_on_active_mask_change_without_reparsing() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 3u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_alarm_mask(3, &AlarmMaskBody::default())
                .unwrap()
                .with_children([5u16]),
        )
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap())
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap());

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(runtime.active_mask(), ObjectID::new(2));
    assert!(runtime.scene().nodes.iter().any(|n| n.id == 4));
    assert!(!runtime.scene().nodes.iter().any(|n| n.id == 5));
    assert!(!runtime.is_dirty());

    let update = runtime.set_active_mask(ObjectID::new(3)).unwrap();
    assert_eq!(
        update,
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(3)
        }
    );
    assert_eq!(runtime.active_mask(), ObjectID::new(3));
    assert!(runtime.scene().nodes.iter().any(|n| n.id == 5));
    assert!(!runtime.scene().nodes.iter().any(|n| n.id == 4));
    assert!(runtime.is_dirty());

    runtime.clear_dirty();
    assert_eq!(
        runtime.set_active_mask(ObjectID::new(3)).unwrap(),
        RenderUpdate::Unchanged
    );
    assert!(!runtime.is_dirty());
}

#[test]
fn render_runtime_rejects_non_mask_active_object() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap());

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(runtime.set_active_mask(ObjectID::new(3)).is_err());
    assert_eq!(runtime.active_mask(), ObjectID::new(2));
}

#[test]
fn render_runtime_repeated_visibility_and_enable_commands_are_unchanged() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16, 4u16]))
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(create_input_boolean(4, &InputBooleanBody::default()).unwrap());

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::HideShow {
                id: ObjectID::new(5),
                visible: true,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "setting an already visible object visible must not mark the scene dirty"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::EnableDisable {
                id: ObjectID::new(4),
                enabled: false,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "setting an already disabled object disabled must not mark the scene dirty"
    );
    assert!(!runtime.is_dirty());

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::HideShow {
                id: ObjectID::new(3),
                visible: false,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "Hide/Show Object is Container-only and must ignore ordinary drawable objects"
    );
    assert!(!runtime.is_dirty());

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::EnableDisable {
                id: ObjectID::new(3),
                enabled: false,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "Enable/Disable Object is limited to input fields, Buttons, and Animation objects"
    );
    assert!(!runtime.is_dirty());

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::HideShow {
                id: ObjectID::new(5),
                visible: false,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    runtime.clear_dirty();
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::HideShow {
                id: ObjectID::new(5),
                visible: false,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "replaying the same Hide/Show state must not mark the scene dirty"
    );
    assert!(!runtime.is_dirty());

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::EnableDisable {
                id: ObjectID::new(4),
                enabled: true,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    runtime.clear_dirty();
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::EnableDisable {
                id: ObjectID::new(4),
                enabled: true,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "replaying the same Enable/Disable state must not mark the scene dirty"
    );
    assert!(!runtime.is_dirty());
}

#[test]
fn render_runtime_macro_overlay_noops_are_unchanged_against_effective_state() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16, 4u16]))
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(create_input_boolean(4, &InputBooleanBody::default()).unwrap())
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![
                    MacroCommand {
                        command_type: cmd::HIDE_SHOW,
                        parameters: vec![5, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::ENABLE_DISABLE,
                        parameters: vec![4, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::ENABLE_DISABLE,
                        parameters: vec![3, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                ],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
                id: ObjectID::new(9),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "macro overlay effects matching effective object state must not rebuild"
    );
    assert!(!runtime.is_dirty());
}

#[test]
fn render_runtime_repeated_pool_mutation_commands_are_unchanged() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16, 5u16]),
        )
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 10,
                    height: 8,
                    background_color: 1,
                    font_attributes: ObjectID::new(10),
                    value: b"same".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                4,
                &InputListBody {
                    width: 10,
                    height: 8,
                    items: vec![ObjectID::new(3)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_polygon(
                5,
                &OutputPolygonBody {
                    width: 20,
                    height: 20,
                    points: vec![
                        PolygonPoint { x: 0, y: 0 },
                        PolygonPoint { x: 20, y: 0 },
                        PolygonPoint { x: 20, y: 20 },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_number(
                6,
                &OutputNumberBody {
                    width: 10,
                    height: 8,
                    value: 42,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_number_variable(7, &NumberVariableBody { value: 42 }))
        .with_object(create_string_variable(
            8,
            &StringVariableBody {
                length: 4,
                value: b"same".to_vec(),
            },
        ))
        .with_object(create_font_attributes(
            10,
            &FontAttributesBody {
                font_color: 7,
                ..Default::default()
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeBackgroundColour {
                id: ObjectID::new(3),
                colour: 1,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same background colour must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeSize {
                id: ObjectID::new(3),
                width: 10,
                height: 8,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same size must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeFontAttributes {
                id: ObjectID::new(3),
                attributes: ObjectID::new(10),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same FontAttributes reference must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(3),
                attribute_id: 1,
                value: 10,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same generic width attribute must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(3),
                attribute_id: 4,
                value: 10,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same generic style-reference attribute must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(10),
                attribute_id: 1,
                value: 7,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same generic shared style-object attribute must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                id: ObjectID::new(6),
                value: 42,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same inline numeric value must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                id: ObjectID::new(7),
                value: 42,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same Number Variable value must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeStringValue {
                id: ObjectID::new(3),
                text: "same".into(),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same inline string value must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeStringValue {
                id: ObjectID::new(8),
                text: "same".into(),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same String Variable value must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeListItem {
                list: ObjectID::new(4),
                index: 0,
                item: ObjectID::new(3),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same list item must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangePolygonPoint {
                id: ObjectID::new(5),
                index: 1,
                x: 20,
                y: 0,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same polygon point must not rebuild"
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangePolygonScale {
                id: ObjectID::new(5),
                width: 20,
                height: 20,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same polygon scale must not rebuild"
    );
    assert!(!runtime.is_dirty());
}

#[test]
fn render_runtime_change_fill_attributes_requires_typed_non_null_pattern() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 16,
                    height: 16,
                    fill_attributes: ObjectID::new(8),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap())
        .with_object(create_fill_attributes(8, &FillAttributesBody::default()).unwrap())
        .with_object(
            create_picture_graphic(
                23,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![1],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                24,
                &PictureGraphicBody {
                    width: 7,
                    actual_width: 7,
                    actual_height: 1,
                    format: 0,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0x80],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeFillAttributeValues {
                id: ObjectID::new(8),
                fill_type: 0,
                colour: 9,
                pattern: ObjectID::new(5),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "fixed Change Fill Attributes must reject non-PictureGraphic pattern references even when fill type is not pattern-fill"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(8))
            .unwrap()
            .get_fill_attributes_body()
            .unwrap(),
        FillAttributesBody::default()
    );

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeFillAttributeValues {
                id: ObjectID::new(8),
                fill_type: 3,
                colour: 4,
                pattern: ObjectID::NULL,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(8))
            .unwrap()
            .get_fill_attributes_body()
            .unwrap()
            .fill_pattern,
        ObjectID::NULL,
        "NULL remains a standard no-pattern selector for fixed Change Fill Attributes"
    );

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeFillAttributeValues {
                id: ObjectID::new(8),
                fill_type: 3,
                colour: 4,
                pattern: ObjectID::new(24),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "pattern fill must reject PictureGraphic rows with unused packed bits"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(8))
            .unwrap()
            .get_fill_attributes_body()
            .unwrap()
            .fill_pattern,
        ObjectID::NULL,
        "rejected pattern-fill updates must not overwrite the retained Fill Pattern"
    );

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeFillAttributeValues {
                id: ObjectID::new(8),
                fill_type: 0,
                colour: 5,
                pattern: ObjectID::new(23),
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(8))
            .unwrap()
            .get_fill_attributes_body()
            .unwrap(),
        FillAttributesBody {
            fill_type: 0,
            fill_color: 5,
            fill_pattern: ObjectID::new(23),
        },
        "non-NULL fixed Change Fill Attributes patterns are valid when typed as PictureGraphic"
    );
}

#[test]
fn render_runtime_applies_value_commands_to_scene() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(
            create_output_number(
                3,
                &OutputNumberBody {
                    width: 40,
                    height: 12,
                    variable_reference: ObjectID::new(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    width: 80,
                    height: 12,
                    variable_reference: ObjectID::new(11),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_number_variable(10, &NumberVariableBody { value: 1 }))
        .with_object(create_string_variable(
            11,
            &StringVariableBody {
                length: 4,
                value: b"old ".to_vec(),
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(10),
            value: 42,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    let number = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert!(matches!(
        &number.kind,
        NodeKind::OutputNumber { text, .. } if text == "42"
    ));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeStringValue {
            id: ObjectID::new(11),
            text: "LIVE".to_owned(),
        })
        .unwrap();
    let string = runtime.scene().find(ObjectID::new(4)).unwrap();
    assert!(matches!(
        &string.kind,
        NodeKind::OutputString { text, .. } if text == "LIVE"
    ));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeStringValue {
            id: ObjectID::new(11),
            text: "OK".to_owned(),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    let string = runtime.scene().find(ObjectID::new(4)).unwrap();
    assert!(matches!(
        &string.kind,
        NodeKind::OutputString { text, .. } if text == "OK  "
    ));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeStringValue {
            id: ObjectID::new(11),
            text: "TOOLONG".to_owned(),
        }),
        Ok(RenderUpdate::Unchanged),
        "Change String Value must not increase a fixed-length String Variable"
    );
    let string = runtime.scene().find(ObjectID::new(4)).unwrap();
    assert!(matches!(
        &string.kind,
        NodeKind::OutputString { text, .. } if text == "OK  "
    ));
}

#[test]
fn render_runtime_applies_generic_attribute_commands_to_scene() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 40,
                    height: 12,
                    value: b"abc".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_number(
                4,
                &OutputNumberBody {
                    width: 40,
                    height: 12,
                    value: 8,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 1,
            value: 90,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 7,
            value: 2,
        })
        .unwrap();
    let text = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(text.rect.w, 90);
    assert!(matches!(
        &text.kind,
        NodeKind::OutputString { justification, .. } if *justification == 2
    ));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(4),
            attribute_id: 8,
            value: 0.25f32.to_bits(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(4),
            attribute_id: 9,
            value: 2,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(4),
            value: 12,
        })
        .unwrap();
    let number = runtime.scene().find(ObjectID::new(4)).unwrap();
    assert!(matches!(
        &number.kind,
        NodeKind::OutputNumber { text, .. } if text == "3.00"
    ));
}

#[test]
fn render_runtime_applies_standard_input_string_options_attribute() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    width: 40,
                    height: 12,
                    options: 0,
                    max_length: 8,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 6,
            value: 0x05,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    let body = runtime
        .pool()
        .find(ObjectID::new(3))
        .unwrap()
        .get_input_string_body()
        .unwrap();
    assert_eq!(body.options, 0x05);
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::InputString { enabled, .. } if *enabled
    ));
}

#[test]
fn output_list_value_changes_by_generic_attribute_and_numeric_value() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_list(
                3,
                &OutputListBody {
                    width: 80,
                    height: 20,
                    value: 0,
                    items: vec![10.into(), 11.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"First".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    value: b"Second".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeSize {
            id: ObjectID::new(3),
            width: 90,
            height: 24,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime.scene().find(ObjectID::new(3)).unwrap().rect,
        Rect::new(0, 0, 90, 24),
        "Output List width/height are mutable through Change Size"
    );
    // Output List value (selected index) is settable via Change Attribute
    // (AID 4) as well as Change Numeric Value, matching the reference VT stack.
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 4,
            value: 1,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputList { selected_text, .. } if selected_text.as_deref() == Some("Second")
    ));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(3),
            value: 0,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputList { selected_text, .. } if selected_text.as_deref() == Some("First")
    ));
}

#[test]
fn render_runtime_applies_generic_mask_container_key_and_window_attributes() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    background_color: 1,
                    soft_key_mask: ObjectID::new(20),
                },
            )
            .with_children_pos([
                ChildRef::new(ObjectID::new(3), 0, 0),
                ChildRef::new(ObjectID::new(30), 0, 30),
                ChildRef::new(ObjectID::new(40), 0, 60),
            ]),
        )
        .with_object(create_container(3, &ContainerBody::default()))
        .with_object(create_soft_key_mask(20, &SoftKeyMaskBody::default()).with_children([21u16]))
        .with_object(create_soft_key_mask(23, &SoftKeyMaskBody::default()).with_children([22u16]))
        .with_object(create_key(
            21,
            &KeyBody {
                key_code: 1,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            22,
            &KeyBody {
                key_code: 7,
                ..Default::default()
            },
        ))
        .with_object(create_key_group(30, &KeyGroupBody::default()).with_children([31u16]))
        .with_object(create_key(
            31,
            &KeyBody {
                key_code: 5,
                ..Default::default()
            },
        ))
        .with_object(
            create_window_mask(
                40,
                &WindowMaskBody {
                    width_cells: 1,
                    height_cells: 1,
                    window_type: 0,
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    for command in [
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(2),
            attribute_id: 1,
            value: 9,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(2),
            attribute_id: 2,
            value: 23,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(30),
            attribute_id: 1,
            value: 1,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(31),
            attribute_id: 2,
            value: 42,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(40),
            attribute_id: 1,
            value: 2,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(40),
            attribute_id: 2,
            value: 2,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(40),
            attribute_id: 4,
            value: 4,
        },
    ] {
        assert!(matches!(
            runtime.apply_ecu_command(&command),
            Ok(RenderUpdate::SceneRebuilt { .. })
        ));
    }
    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeSize {
            id: ObjectID::new(3),
            width: 55,
            height: 22,
        }),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::HideShow {
            id: ObjectID::new(3),
            visible: false,
        }),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));

    assert_eq!(runtime.scene().background, 9);
    assert_eq!(runtime.scene().soft_keys.len(), 1);
    assert_eq!(runtime.scene().soft_keys[0].id, ObjectID::new(22));
    assert_eq!(runtime.scene().soft_keys[0].label, "7");

    let container = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(container.rect, Rect::new(0, 0, 55, 22));
    assert!(!container.visible);

    assert!(matches!(
        &runtime.scene().find(ObjectID::new(30)).unwrap().kind,
        NodeKind::KeyGroup {
            available: true,
            labels,
            ..
        } if labels.as_slice() == ["42"]
    ));

    let window = runtime.scene().find(ObjectID::new(40)).unwrap();
    assert_eq!(window.rect, Rect::new(0, 60, 480, 80));
    assert!(matches!(
        &window.kind,
        NodeKind::Group { background: 4, .. }
    ));
}

#[test]
fn fill_attributes_type_one_fills_shapes_with_line_colour() {
    let mut palette = Palette::default_isobus();
    let line_colour = Colour::rgb(12, 34, 56);
    let ignored_fill_colour = Colour::rgb(210, 20, 30);
    palette.set_entry(4, line_colour);
    palette.set_entry(7, ignored_fill_colour);

    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([
            3u16, 4u16, 5u16,
        ]))
        .with_object(create_line_attributes(
            10,
            &LineAttributesBody {
                line_color: 4,
                line_width: 1,
                line_art: 0xFFFF,
            },
        ))
        .with_object(
            create_fill_attributes(
                11,
                &FillAttributesBody {
                    fill_type: 1,
                    fill_color: 7,
                    fill_pattern: ObjectID::NULL,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 30,
                    height: 20,
                    line_attributes: ObjectID::new(10),
                    fill_attributes: ObjectID::new(11),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_ellipse(
                4,
                &OutputEllipseBody {
                    width: 22,
                    height: 14,
                    line_attributes: ObjectID::new(10),
                    fill_attributes: ObjectID::new(11),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_polygon(
                5,
                &OutputPolygonBody {
                    width: 18,
                    height: 16,
                    line_attributes: ObjectID::new(10),
                    fill_attributes: ObjectID::new(11),
                    points: vec![
                        PolygonPoint { x: 0, y: 0 },
                        PolygonPoint { x: 18, y: 0 },
                        PolygonPoint { x: 9, y: 16 },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = LayoutEngine::new(LayoutConfig::default())
        .with_palette(palette.clone())
        .build(&pool, ObjectID::NULL);
    for id in [3u16, 4, 5] {
        let node = scene.find(ObjectID::new(id)).unwrap();
        assert_eq!(node.style.fill_type, FillType::LineColour);
        assert_eq!(node.style.fill_colour, line_colour);
        assert_ne!(node.style.fill_colour, ignored_fill_colour);
    }
    let rect_rect = scene.find(ObjectID::new(3)).unwrap().rect;
    let ellipse_rect = scene.find(ObjectID::new(4)).unwrap().rect;

    let commands = GtuiRenderer::new(palette).render(&scene);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect {
            rect,
            colour,
        } if *rect == rect_rect && *colour == line_colour
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ellipse {
            rect,
            filled: true,
            fill_colour,
            ..
        } if *rect == ellipse_rect && *fill_colour == line_colour
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Polygon {
            filled: true,
            fill_colour,
            ..
        } if *fill_colour == line_colour
    )));
}

#[test]
fn fill_attributes_type_three_tiles_picture_graphic_pattern() {
    let mut palette = Palette::default_isobus();
    let left_colour = Colour::rgb(20, 40, 60);
    let right_colour = Colour::rgb(200, 120, 30);
    palette.set_entry(2, left_colour);
    palette.set_entry(3, right_colour);

    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_line_attributes(
            10,
            &LineAttributesBody {
                line_width: 0,
                ..Default::default()
            },
        ))
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 1,
                    format: 1,
                    options: 0x01,
                    transparency: 3,
                    data: vec![0x23],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_fill_attributes(
                11,
                &FillAttributesBody {
                    fill_type: 3,
                    fill_color: 9,
                    fill_pattern: ObjectID::new(20),
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 4,
                    height: 2,
                    line_attributes: ObjectID::new(10),
                    fill_attributes: ObjectID::new(11),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = LayoutEngine::new(LayoutConfig::default())
        .with_placements(PlacementMap::new().set(3u16, 1, 0))
        .with_palette(palette.clone())
        .build(&pool, ObjectID::NULL);
    let rect = scene.find(ObjectID::new(3)).unwrap().rect;
    assert_eq!(rect.x, 1);
    let commands = GtuiRenderer::new(palette.clone()).render(&scene);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::PatternFillRect { rect: r, anchor, pattern }
            if *r == rect && *anchor == (0, 0) && pattern.object_id == ObjectID::new(20)
    )));

    let framebuffer = FramebufferRenderer::new(GtuiRenderer::new(palette), Colour::rgb(0, 0, 0))
        .render_scene(&scene)
        .unwrap();
    let x = u16::try_from(rect.x).unwrap();
    let y = u16::try_from(rect.y).unwrap();
    assert_eq!(framebuffer.pixel(x, y), Some(right_colour));
    assert_eq!(framebuffer.pixel(x + 1, y), Some(left_colour));
    assert_eq!(framebuffer.pixel(x + 2, y), Some(right_colour));
}
