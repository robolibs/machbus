//! GTUI VT server demo — build an object pool, render it to the GTUI
//! retained-mode command list, feed a few simulated operator events
//! through the input runtime, and print the resulting scene + the
//! semantic VT events a real VT server would bridge back onto the bus.
//!
//! This demonstrates the product-level VT loop called out as the top P0
//! gap in `GAP.md`:
//!
//! ```text
//! object pool → validated scene → GTUI command list
//!                                   ↑
//!            operator input → InputRuntime → VtEvent
//! ```
//!
//! Run with `cargo run --example vt_gtui_server`.

use machbus::isobus::vt::render::VtRenderRuntime;
use machbus::isobus::vt::render::gtui::{GtuiRenderer, RenderCommand};
use machbus::isobus::vt::render::input::{OperatorEvent, VtEvent};
use machbus::isobus::vt::render::layout::LayoutConfig;
use machbus::isobus::vt::{
    AnimationBody, ChildRef, ContainerBody, DataMaskBody, FontAttributesBody, InputNumberBody,
    InputStringBody, ObjectID, ObjectPool, OutputRectangleBody, OutputStringBody, WorkingSetBody,
    create_animation, create_container, create_data_mask, create_font_attributes,
    create_input_number, create_input_string, create_output_rectangle, create_output_string,
    create_working_set,
};

fn main() {
    println!("=== GTUI VT Server Demo ===\n");

    let mut runtime =
        VtRenderRuntime::from_pool(build_demo_pool(), LayoutConfig::default()).unwrap();

    println!("active mask : 0x{:04X}", runtime.active_mask().raw());
    println!("scene nodes : {}", runtime.scene().nodes.len());

    // 1) Render the static scene.
    let renderer = GtuiRenderer::default();
    let cmds = runtime.render(&renderer);
    println!("GTUI commands emitted: {}", cmds.len());
    print_command_summary(&cmds);
    println!(
        "animation scheduler hint: {:?}",
        runtime.animation_refresh_interval_ms()
    );

    // 2) Simulate an operator typing into the input string + number.
    println!("\n--- operator interaction ---");
    run(&mut runtime, OperatorEvent::FocusNext);
    run(&mut runtime, OperatorEvent::Char('F'));
    run(&mut runtime, OperatorEvent::Char('X'));
    run(&mut runtime, OperatorEvent::FocusNext);
    run(&mut runtime, OperatorEvent::Char('4'));
    run(&mut runtime, OperatorEvent::Char('2'));
    run(&mut runtime, OperatorEvent::Commit);

    // 3) After the value-change events a host VT server would write the
    //    edited values back into the pool's variables and rebuild the
    //    scene. Here we tick the hosted animation scheduler to show the
    //    same runtime can drive redraw cadence without a product UI.
    let tick = runtime.tick_animation(100);
    println!(
        "\nanimation tick: update={:?} next={:?}",
        tick.update, tick.next_refresh_interval_ms
    );
    let redraw = runtime.render(&renderer);
    println!("re-rendered {} commands after runtime tick", redraw.len());
}

fn run(runtime: &mut VtRenderRuntime, ev: OperatorEvent) {
    let events = runtime.handle_operator_event(ev);
    for ev in events {
        match &ev {
            VtEvent::FocusChanged { id } => {
                println!("  {ev:?} -> focus 0x{:04X}", id.raw())
            }
            VtEvent::StringValueChanged { id, text } => {
                println!("  {ev:?} -> string 0x{:04X} = {text:?}", id.raw())
            }
            VtEvent::NumberValueChanged { id, raw } => {
                println!("  {ev:?} -> number 0x{:04X} = {raw}", id.raw())
            }
            other => println!("  {other:?}"),
        }
    }
}

fn print_command_summary(cmds: &[RenderCommand]) {
    let mut fill = 0usize;
    let mut stroke = 0usize;
    let mut text = 0usize;
    let mut other = 0usize;
    for c in cmds {
        match c {
            RenderCommand::FillRect { .. } => fill += 1,
            RenderCommand::StrokeRect { .. } => stroke += 1,
            RenderCommand::DrawText { .. } => text += 1,
            _ => other += 1,
        }
    }
    println!("  fill={fill} stroke={stroke} text={text} other={other}");
}

fn build_demo_pool() -> ObjectPool {
    let font = create_font_attributes(
        20,
        &FontAttributesBody {
            font_color: 1,
            font_size: 6,
            ..Default::default()
        },
    );
    let title = create_output_string(
        10,
        &OutputStringBody {
            width: 240,
            height: 28,
            font_attributes: font.id,
            value: b"GTUI DEMO".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    let frame = create_output_rectangle(
        12,
        &OutputRectangleBody {
            width: 320,
            height: 180,
            ..Default::default()
        },
    )
    .unwrap();
    let name_field = create_input_string(
        11,
        &InputStringBody {
            width: 200,
            height: 28,
            font_attributes: font.id,
            options: 0x01, // enabled
            ..Default::default()
        },
    )
    .unwrap();
    let rate_field = create_input_number(
        13,
        &InputNumberBody {
            width: 200,
            height: 28,
            font_attributes: font.id,
            options: 0x01, // enabled
            min_value: 0,
            max_value: 500,
            ..Default::default()
        },
    )
    .unwrap();
    let panel = create_container(
        14,
        &ContainerBody {
            width: 320,
            height: 180,
            hidden: false,
        },
    )
    .with_children([10u16, 11u16, 13u16]);
    let anim_a = create_output_string(
        15,
        &OutputStringBody {
            width: 120,
            height: 28,
            value: b"FRAME A".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    let anim_b = create_output_string(
        16,
        &OutputStringBody {
            width: 120,
            height: 28,
            value: b"FRAME B".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    let animation = create_animation(
        17,
        &AnimationBody {
            width: 120,
            height: 28,
            refresh_interval_ms: 100,
            value: 0,
            enabled: 1,
            first_child_index: 0,
            default_child_index: 0,
            last_child_index: 1,
            options: 0,
        },
    )
    .unwrap()
    .with_children_pos([
        ChildRef::at_origin(ObjectID::new(15)),
        ChildRef::at_origin(ObjectID::new(16)),
    ]);

    ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([12u16, 14u16, 17u16]),
        )
        .with_object(font)
        .with_object(title)
        .with_object(frame)
        .with_object(name_field)
        .with_object(rate_field)
        .with_object(panel)
        .with_object(anim_a)
        .with_object(anim_b)
        .with_object(animation)
}
