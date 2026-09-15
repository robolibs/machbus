//! `machbus term client` — upload an object pool to a live VT over CAN.
//! The test counterpart to `machbus term server`.

use std::time::{Duration, Instant};

use machbus::isobus::vt::{
    DataMaskBody, FillAttributesBody, FontAttributesBody, LineAttributesBody, ObjectID, ObjectPool,
    ObjectType, OutputRectangleBody, OutputStringBody, VTClientConfig, VTState, VTVersion,
    WorkingSet, WorkingSetBody, create_data_mask, create_fill_attributes, create_font_attributes,
    create_line_attributes, create_output_rectangle, create_output_string, create_working_set,
};
use machbus::net::Name;
use machbus::session::Session;
use machbus::session::plugins::VtClient as VtClientPlugin;
use machbus::time::Instant as MbInstant;

use crate::bus::Bus;
use crate::cli::TermClientArgs;
use crate::signal;

/// Entry point for `machbus term client`.
pub fn run(args: TermClientArgs) -> Result<(), String> {
    signal::install_cancel_handler();
    let addr = parse_addr(&args.addr)?;

    let (pool, label) = if args.demo || args.iop.is_none() {
        (build_demo_pool(), "<built-in demo pool>".to_string())
    } else {
        let path = args.iop.as_deref().unwrap();
        let bytes = std::fs::read(path).map_err(|e| format!("read '{path}': {e}"))?;
        let p = ObjectPool::deserialize(&bytes).map_err(|e| format!("deserialize pool: {e}"))?;
        (p, path.to_string())
    };
    println!("term client: loaded {label} ({} objects)", pool.size());

    // First mask becomes the working set's active mask.
    let first_mask = pool
        .objects()
        .iter()
        .find(|o| matches!(o.r#type, ObjectType::DataMask | ObjectType::AlarmMask))
        .map(|o| o.id)
        .unwrap_or(ObjectID::NULL);
    let mut ws = WorkingSet::default();
    ws.set_active_mask(first_mask);

    let vt_version = match args.vt_version {
        3 => VTClientConfig::default().with_version(VTVersion::Version3),
        5 => VTClientConfig::default().with_version(VTVersion::Version5),
        6 => VTClientConfig::default().with_version(VTVersion::Version6),
        _ => VTClientConfig::default(), // version 4
    };
    let plugin = VtClientPlugin::new(vt_version, pool, ws);
    let mut session = Session::builder(client_name(), addr)
        .plug(plugin)
        .build()
        .map_err(|e| format!("session build: {e}"))?;
    session.start().map_err(|e| format!("session start: {e}"))?;
    session
        .get_mut::<VtClientPlugin>()
        .expect("vt-client plugin")
        .client_mut()
        .connect()
        .map_err(|e| format!("connect: {e}"))?;

    let bus = Bus::open(&args.iface).map_err(|e| format!("open {}: {e}", args.iface))?;
    println!(
        "term client: claiming 0x{addr:02X} on {} — waiting for a VT…",
        args.iface
    );

    let start = Instant::now();
    let mut last = VTState::Disconnected;
    // Tight pump: the ETP upload is gated by CTS round-trips with the server.
    let pump = Duration::from_millis(2);
    loop {
        let now = MbInstant::ZERO.add_millis(start.elapsed().as_millis() as u64);
        bus.pump(&mut session, now);
        session.tick(now);

        while let Some(ev) = session.poll_event() {
            println!("term client: event {ev:?}");
        }

        let st: VTState = session
            .get::<VtClientPlugin>()
            .map(|p| p.state())
            .unwrap_or(VTState::Disconnected);
        if st != last {
            println!(
                "term client: [{:>5.1}s] state = {:?}",
                start.elapsed().as_secs_f64(),
                st
            );
            last = st;
        }

        if signal::cancel_requested() {
            println!("term client: interrupted");
            break;
        }
        if last == VTState::Connected && start.elapsed() > Duration::from_secs(3) {
            println!("term client: upload complete — VT should be rendering");
            break;
        }
        if start.elapsed() > Duration::from_secs(30) {
            println!("term client: timeout before connecting");
            break;
        }
        std::thread::sleep(pump);
    }
    Ok(())
}

fn client_name() -> Name {
    Name::default()
        .with_self_configurable(true)
        .with_function_code(0x80)
        .with_identity_number(0x0002)
}

fn parse_addr(spec: &str) -> Result<u8, String> {
    u8::from_str_radix(spec.trim_start_matches("0x"), 16)
        .map_err(|_| format!("--addr '{spec}': expected hex byte, e.g. 80"))
}

/// A small, self-contained but vivid demo pool: a dark canvas with a colored
/// title bar, two filled panels, and labels — so the rendered VT screen
/// actually looks like a UI instead of an empty rectangle. machbus paces
/// TP/ETP realistically, so keeping this tiny makes the upload finish fast.
fn build_demo_pool() -> ObjectPool {
    // Palette indices: 0=white, 1=black, 16.. = vivid generated colours.
    let fill_bg = create_fill_attributes(
        20,
        &FillAttributesBody {
            fill_type: 2,
            fill_color: 1, // black background
            ..Default::default()
        },
    )
    .unwrap();
    let fill_title = create_fill_attributes(
        21,
        &FillAttributesBody {
            fill_type: 2,
            fill_color: 16, // teal
            ..Default::default()
        },
    )
    .unwrap();
    let fill_a = create_fill_attributes(
        22,
        &FillAttributesBody {
            fill_type: 2,
            fill_color: 17, // green
            ..Default::default()
        },
    )
    .unwrap();
    let fill_b = create_fill_attributes(
        23,
        &FillAttributesBody {
            fill_type: 2,
            fill_color: 18, // purple
            ..Default::default()
        },
    )
    .unwrap();
    let line_white = create_line_attributes(
        24,
        &LineAttributesBody {
            line_color: 0, // white
            line_width: 2,
            ..Default::default()
        },
    );
    let font = create_font_attributes(
        25,
        &FontAttributesBody {
            font_color: 0, // white
            font_size: 6,
            ..Default::default()
        },
    );

    let bg = create_output_rectangle(
        30,
        &OutputRectangleBody {
            width: 480,
            height: 240,
            fill_attributes: fill_bg.id,
            ..Default::default()
        },
    )
    .unwrap();
    let title_bar = create_output_rectangle(
        31,
        &OutputRectangleBody {
            width: 480,
            height: 36,
            fill_attributes: fill_title.id,
            ..Default::default()
        },
    )
    .unwrap();
    let panel_a = create_output_rectangle(
        32,
        &OutputRectangleBody {
            width: 220,
            height: 180,
            line_attributes: line_white.id,
            fill_attributes: fill_a.id,
            ..Default::default()
        },
    )
    .unwrap();
    let panel_b = create_output_rectangle(
        33,
        &OutputRectangleBody {
            width: 220,
            height: 180,
            line_attributes: line_white.id,
            fill_attributes: fill_b.id,
            ..Default::default()
        },
    )
    .unwrap();

    let title = create_output_string(
        40,
        &OutputStringBody {
            width: 300,
            height: 28,
            font_attributes: font.id,
            value: b"MACHBUS VT DEMO".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    let label_a = create_output_string(
        41,
        &OutputStringBody {
            width: 160,
            height: 28,
            font_attributes: font.id,
            value: b"PANEL A".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    let label_b = create_output_string(
        42,
        &OutputStringBody {
            width: 160,
            height: 28,
            font_attributes: font.id,
            value: b"PANEL B".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();

    let mut mask = create_data_mask(2, &DataMaskBody::default());
    mask.add_child_pos(30, 0, 0); // background
    mask.add_child_pos(31, 0, 0); // title bar
    mask.add_child_pos(32, 10, 50); // panel A
    mask.add_child_pos(33, 250, 50); // panel B
    mask.add_child_pos(40, 20, 4); // title text
    mask.add_child_pos(41, 30, 60); // label A
    mask.add_child_pos(42, 270, 60); // label B

    ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(mask)
        .with_object(fill_bg)
        .with_object(fill_title)
        .with_object(fill_a)
        .with_object(fill_b)
        .with_object(line_white)
        .with_object(font)
        .with_object(bg)
        .with_object(title_bar)
        .with_object(panel_a)
        .with_object(panel_b)
        .with_object(title)
        .with_object(label_a)
        .with_object(label_b)
}
