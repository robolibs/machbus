//! VT trace inspector — upload an object pool through `VTServer`, replay
//! reviewable ECU-to-VT command payload bytes, then render the resulting
//! `VtRenderRuntime` snapshot.
//!
//! This is Phase 10 evidence tooling. The default pool and trace are
//! repo-owned reduced fixtures, not certification evidence, but the same
//! report shape can be used when a redistributable external pool/trace is
//! promoted.
//!
//! Run with:
//! `cargo run --example vt_trace_inspect -- --strict --physical-soft-keys 10 --write-report-json report.json --write-final-rgb888 final.rgb --write-final-rgb565-be final.rgb565`.

use machbus::isobus::vt::render::framebuffer::FramebufferRenderer;
use machbus::isobus::vt::render::scene::Rect;
use machbus::isobus::vt::render::{LayoutConfig, VtRenderRuntime};
use machbus::isobus::vt::{
    ContainerBody, DataMaskBody, ObjectID, ObjectPool, OutputStringBody, StringVariableBody,
    VTServer, VTServerConfig, WorkingSetBody, cmd, create_container, create_data_mask,
    create_output_string, create_string_variable, create_working_set,
};
use machbus::net::Message;
use machbus::net::pgn_defs::PGN_ECU_TO_VT;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

const DEFAULT_TRACE_HEX: &str = include_str!("../tests/fixtures/isobus/vt_render_trace.hex");

fn main() {
    println!("=== VT Trace Inspector (server replay → render runtime) ===\n");

    let raw_args: Vec<_> = env::args_os().skip(1).collect();
    if raw_args.iter().any(|arg| {
        arg.to_str()
            .is_some_and(|arg| matches!(arg, "-h" | "--help"))
    }) {
        print_usage();
        return;
    }

    let args = match TraceInspectArgs::parse(&raw_args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    };

    let (pool_source, pool_bytes) = match load_pool_bytes(&args) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    };
    let (trace_source, trace_text) = match load_trace_text(&args) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    };
    let commands = match parse_trace_commands(&trace_text) {
        Ok(commands) => commands,
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    };

    let pool_hash = fnv1a64(&pool_bytes);
    let trace_hash = fnv1a64_trace(&commands);
    let layout_config = args.layout_config();
    println!("pool source : {pool_source}");
    println!(
        "pool bytes  : {} fnv1a64=0x{pool_hash:016X}",
        pool_bytes.len()
    );
    println!("trace source: {trace_source}");
    println!(
        "layout     : canvas={}x{} soft_keys=({}, {}, {}, {}) physical={} navigation={} page={}",
        layout_config.canvas.0,
        layout_config.canvas.1,
        layout_config.soft_key_area.x,
        layout_config.soft_key_area.y,
        layout_config.soft_key_area.w,
        layout_config.soft_key_area.h,
        layout_config.physical_soft_key_count,
        layout_config.navigation_soft_key_count,
        layout_config.soft_key_page,
    );
    println!(
        "trace rows  : {} payload_fnv1a64=0x{trace_hash:016X}",
        commands.len()
    );

    let mut failures = Vec::new();
    let mut server = VTServer::new(VTServerConfig::default());
    if let Err(error) = server.start() {
        eprintln!("failed to start VT server: {error:?}");
        process::exit(1);
    }
    let source = args.source_address;
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_vt_command(cmd::GET_MEMORY),
        source,
    ));
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend_from_slice(&pool_bytes);
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, transfer, source));
    let end_response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_vt_command(cmd::END_OF_POOL),
        source,
    ));
    if end_response
        .first()
        .and_then(|message| message.data.get(1))
        .copied()
        != Some(0)
    {
        failures.push("object pool upload did not finish with success".to_string());
    }
    if server.clients().is_empty() {
        failures.push("VT server has no uploaded client after pool transfer".to_string());
    }

    let initial = render_summary(
        server.clients().first(),
        layout_config,
        "initial",
        &mut failures,
    );

    for command in &commands {
        println!(
            "replay      : {} ({} bytes)",
            command.name,
            command.payload.len()
        );
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            command.payload.clone(),
            source,
        ));
    }

    let final_summary = render_summary(
        server.clients().first(),
        layout_config,
        "final",
        &mut failures,
    );
    let accepted_effects: Vec<String> = server
        .clients()
        .first()
        .map(|client| {
            client
                .object_state
                .accepted_effects
                .iter()
                .map(|effect| format!("{effect:?}"))
                .collect()
        })
        .unwrap_or_default();

    println!("accepted   : {} render effects", accepted_effects.len());
    for effect in &accepted_effects {
        println!("  - {effect}");
    }

    if args.strict && accepted_effects.len() != commands.len() {
        failures.push(format!(
            "strict mode: {} trace commands produced {} accepted render effects",
            commands.len(),
            accepted_effects.len()
        ));
    }
    if let Some(expected) = args.expected_accepted_effects
        && expected != accepted_effects.len()
    {
        failures.push(format!(
            "accepted render effect count mismatch: expected {expected}, got {}",
            accepted_effects.len()
        ));
    }
    if let Some(expected) = args.expected_initial_placeholder_pixels
        && initial.placeholder_pixels != Some(expected)
    {
        failures.push(format!(
            "initial framebuffer placeholder pixel count mismatch: expected {expected}, got {}",
            initial
                .placeholder_pixels
                .map(|count| count.to_string())
                .unwrap_or_else(|| "null".to_string())
        ));
    }
    if let Some(expected) = args.expected_final_placeholder_pixels
        && final_summary.placeholder_pixels != Some(expected)
    {
        failures.push(format!(
            "final framebuffer placeholder pixel count mismatch: expected {expected}, got {}",
            final_summary
                .placeholder_pixels
                .map(|count| count.to_string())
                .unwrap_or_else(|| "null".to_string())
        ));
    }
    if let Some(expected) = args.expected_rgb888_fnv64
        && final_summary.rgb888_fnv64 != Some(expected)
    {
        failures.push(format!(
            "final framebuffer RGB888 FNV-1a hash mismatch: expected 0x{expected:016X}, got {}",
            final_summary
                .rgb888_fnv64
                .map(|hash| format!("0x{hash:016X}"))
                .unwrap_or_else(|| "null".to_string())
        ));
    }
    if let Some(expected) = args.expected_rgb565_be_fnv64
        && final_summary.rgb565_be_fnv64 != Some(expected)
    {
        failures.push(format!(
            "final framebuffer RGB565 big-endian FNV-1a hash mismatch: expected 0x{expected:016X}, got {}",
            final_summary
                .rgb565_be_fnv64
                .map(|hash| format!("0x{hash:016X}"))
                .unwrap_or_else(|| "null".to_string())
        ));
    }
    if let Some(expected) = args.expected_rgb565_le_fnv64
        && final_summary.rgb565_le_fnv64 != Some(expected)
    {
        failures.push(format!(
            "final framebuffer RGB565 little-endian FNV-1a hash mismatch: expected 0x{expected:016X}, got {}",
            final_summary
                .rgb565_le_fnv64
                .map(|hash| format!("0x{hash:016X}"))
                .unwrap_or_else(|| "null".to_string())
        ));
    }

    if let Some(path) = args.write_report_json_path.as_deref() {
        let report = trace_report_json(TraceReportInput {
            pool_source: &pool_source,
            pool_buffer_bytes: pool_bytes.len(),
            pool_buffer_fnv64: pool_hash,
            layout_config,
            trace_source: &trace_source,
            trace_payload_fnv64: trace_hash,
            commands: &commands,
            strict: args.strict,
            expected_accepted_effects: args.expected_accepted_effects,
            expected_initial_placeholder_pixels: args.expected_initial_placeholder_pixels,
            expected_final_placeholder_pixels: args.expected_final_placeholder_pixels,
            expected_rgb888_fnv64: args.expected_rgb888_fnv64,
            expected_rgb565_be_fnv64: args.expected_rgb565_be_fnv64,
            expected_rgb565_le_fnv64: args.expected_rgb565_le_fnv64,
            write_report_json_path: args.write_report_json_path.as_deref(),
            write_initial_rgb888_path: args.write_initial_rgb888_path.as_deref(),
            write_final_rgb888_path: args.write_final_rgb888_path.as_deref(),
            write_initial_rgb565_be_path: args.write_initial_rgb565_be_path.as_deref(),
            write_initial_rgb565_le_path: args.write_initial_rgb565_le_path.as_deref(),
            write_final_rgb565_be_path: args.write_final_rgb565_be_path.as_deref(),
            write_final_rgb565_le_path: args.write_final_rgb565_le_path.as_deref(),
            initial: &initial,
            final_summary: &final_summary,
            accepted_effects: &accepted_effects,
            failures: &failures,
        });
        write_text_file("report_json", path, &report, &mut failures);
    }
    if let Some(path) = args.write_initial_rgb888_path.as_deref() {
        write_frame_rgb888_file("initial_rgb888", path, &initial, &mut failures);
    }
    if let Some(path) = args.write_final_rgb888_path.as_deref() {
        write_frame_rgb888_file("final_rgb888", path, &final_summary, &mut failures);
    }
    if let Some(path) = args.write_initial_rgb565_be_path.as_deref() {
        write_frame_rgb565_be_file("initial_rgb565_be", path, &initial, &mut failures);
    }
    if let Some(path) = args.write_initial_rgb565_le_path.as_deref() {
        write_frame_rgb565_le_file("initial_rgb565_le", path, &initial, &mut failures);
    }
    if let Some(path) = args.write_final_rgb565_be_path.as_deref() {
        write_frame_rgb565_be_file("final_rgb565_be", path, &final_summary, &mut failures);
    }
    if let Some(path) = args.write_final_rgb565_le_path.as_deref() {
        write_frame_rgb565_le_file("final_rgb565_le", path, &final_summary, &mut failures);
    }

    if args.requires_final_check() {
        if failures.is_empty() {
            if args.strict {
                println!("\nstrict: pass");
            }
            if args.expects_framebuffer_hash() {
                println!("hash check: pass");
            }
        } else {
            if args.strict {
                eprintln!("\nstrict: fail");
            } else {
                eprintln!("\ncheck: fail");
            }
            for failure in failures {
                eprintln!("  - {failure}");
            }
            process::exit(1);
        }
    }
}

fn print_usage() {
    println!("Usage:");
    println!("  cargo run --example vt_trace_inspect");
    println!("  cargo run --example vt_trace_inspect -- --strict");
    println!(
        "  cargo run --example vt_trace_inspect -- --trace tests/fixtures/isobus/vt_render_trace.hex"
    );
    println!("  cargo run --example vt_trace_inspect -- --pool tests/fixtures/isobus/pool.iop");
    println!(
        "  cargo run --example vt_trace_inspect -- --pool fixtures.hex --pool-fixture named_pool"
    );
    println!(
        "  cargo run --example vt_trace_inspect -- --canvas 480x240 --soft-key-area 480,0,64,240"
    );
    println!(
        "  cargo run --example vt_trace_inspect -- --physical-soft-keys 10 --navigation-soft-keys 2"
    );
    println!("  cargo run --example vt_trace_inspect -- --expect-rgb888-fnv64 0x0123456789ABCDEF");
    println!(
        "  cargo run --example vt_trace_inspect -- --expect-rgb565-be-fnv64 0x0123456789ABCDEF"
    );
    println!(
        "  cargo run --example vt_trace_inspect -- --expect-rgb565-le-fnv64 0x0123456789ABCDEF"
    );
    println!("  cargo run --example vt_trace_inspect -- --expect-accepted-effects 3");
    println!("  cargo run --example vt_trace_inspect -- --expect-final-placeholder-pixels 0");
    println!("  cargo run --example vt_trace_inspect -- --write-report-json trace-report.json");
    println!("  cargo run --example vt_trace_inspect -- --write-initial-rgb888 before.rgb");
    println!("  cargo run --example vt_trace_inspect -- --write-final-rgb888 after.rgb");
    println!("  cargo run --example vt_trace_inspect -- --write-initial-rgb565-be before.rgb565");
    println!(
        "  cargo run --example vt_trace_inspect -- --write-initial-rgb565-le before-le.rgb565"
    );
    println!("  cargo run --example vt_trace_inspect -- --write-final-rgb565-be after.rgb565");
    println!("  cargo run --example vt_trace_inspect -- --write-final-rgb565-le after-le.rgb565");
    println!();
    println!("The default pool and trace are repo-owned reduced fixtures. Raw .iop/.bin");
    println!("pool files are accepted; .hex/.txt pool files may contain one plain hex");
    println!("stream or named fixture rows selected by --pool-fixture. Trace files use");
    println!("named rows: name=HEX_PAYLOAD. Layout flags make the final render hash");
    println!("specific to the target display / soft-key profile under review. Raw");
    println!("RGB888 and RGB565 frame dumps are tightly packed row-major bytes for");
    println!("archival, byte-for-byte external trace comparison, or display-driver smoke");
    println!("tests.");
}

#[derive(Debug, Clone)]
struct TraceInspectArgs {
    pool_path: Option<PathBuf>,
    pool_fixture_name: Option<String>,
    trace_path: Option<PathBuf>,
    strict: bool,
    expected_accepted_effects: Option<usize>,
    expected_initial_placeholder_pixels: Option<usize>,
    expected_final_placeholder_pixels: Option<usize>,
    expected_rgb888_fnv64: Option<u64>,
    expected_rgb565_be_fnv64: Option<u64>,
    expected_rgb565_le_fnv64: Option<u64>,
    write_report_json_path: Option<PathBuf>,
    write_initial_rgb888_path: Option<PathBuf>,
    write_final_rgb888_path: Option<PathBuf>,
    write_initial_rgb565_be_path: Option<PathBuf>,
    write_initial_rgb565_le_path: Option<PathBuf>,
    write_final_rgb565_be_path: Option<PathBuf>,
    write_final_rgb565_le_path: Option<PathBuf>,
    source_address: u8,
    canvas: (u16, u16),
    soft_key_area: Rect,
    physical_soft_key_count: u8,
    navigation_soft_key_count: u8,
    soft_key_page: u16,
}

impl TraceInspectArgs {
    fn parse(raw_args: &[std::ffi::OsString]) -> Result<Self, String> {
        let defaults = LayoutConfig::default();
        let mut pool_path = None;
        let mut pool_fixture_name = None;
        let mut trace_path = None;
        let mut strict = false;
        let mut expected_accepted_effects = None;
        let mut expected_initial_placeholder_pixels = None;
        let mut expected_final_placeholder_pixels = None;
        let mut expected_rgb888_fnv64 = None;
        let mut expected_rgb565_be_fnv64 = None;
        let mut expected_rgb565_le_fnv64 = None;
        let mut write_report_json_path = None;
        let mut write_initial_rgb888_path = None;
        let mut write_final_rgb888_path = None;
        let mut write_initial_rgb565_be_path = None;
        let mut write_initial_rgb565_le_path = None;
        let mut write_final_rgb565_be_path = None;
        let mut write_final_rgb565_le_path = None;
        let mut source_address = 0x42;
        let mut canvas = defaults.canvas;
        let mut soft_key_area = defaults.soft_key_area;
        let mut physical_soft_key_count = defaults.physical_soft_key_count;
        let mut navigation_soft_key_count = defaults.navigation_soft_key_count;
        let mut soft_key_page = defaults.soft_key_page;
        let mut iter = raw_args.iter();
        while let Some(arg) = iter.next() {
            match arg.to_str() {
                Some("--strict") => strict = true,
                Some("--pool") => {
                    let Some(value) = iter.next() else {
                        return Err("--pool requires a path".to_string());
                    };
                    set_optional_path(&mut pool_path, "--pool", value)?;
                }
                Some("--pool-fixture") => {
                    let Some(value) = iter.next() else {
                        return Err("--pool-fixture requires a fixture name".to_string());
                    };
                    if pool_fixture_name.is_some() {
                        return Err("--pool-fixture may only be provided once".to_string());
                    }
                    pool_fixture_name = Some(
                        value
                            .to_str()
                            .ok_or("--pool-fixture value must be valid UTF-8")?
                            .to_string(),
                    );
                }
                Some("--trace") => {
                    let Some(value) = iter.next() else {
                        return Err("--trace requires a path".to_string());
                    };
                    set_optional_path(&mut trace_path, "--trace", value)?;
                }
                Some("--source-address") => {
                    let Some(value) = iter.next() else {
                        return Err(
                            "--source-address requires a hex or decimal address".to_string()
                        );
                    };
                    source_address = parse_u8_value(value, "--source-address")?;
                }
                Some("--canvas") => {
                    let Some(value) = iter.next() else {
                        return Err("--canvas requires WIDTHxHEIGHT".to_string());
                    };
                    canvas = parse_canvas(value, "--canvas")?;
                }
                Some("--soft-key-area") => {
                    let Some(value) = iter.next() else {
                        return Err("--soft-key-area requires X,Y,W,H".to_string());
                    };
                    soft_key_area = parse_rect(value, "--soft-key-area")?;
                }
                Some("--physical-soft-keys") => {
                    let Some(value) = iter.next() else {
                        return Err("--physical-soft-keys requires a count".to_string());
                    };
                    physical_soft_key_count = parse_u8_value(value, "--physical-soft-keys")?;
                }
                Some("--navigation-soft-keys") => {
                    let Some(value) = iter.next() else {
                        return Err("--navigation-soft-keys requires a count".to_string());
                    };
                    navigation_soft_key_count = parse_u8_value(value, "--navigation-soft-keys")?;
                }
                Some("--soft-key-page") => {
                    let Some(value) = iter.next() else {
                        return Err("--soft-key-page requires a zero-based page".to_string());
                    };
                    soft_key_page = parse_u16_value(value, "--soft-key-page")?;
                }
                Some("--expect-rgb888-fnv64") => {
                    let Some(value) = iter.next() else {
                        return Err("--expect-rgb888-fnv64 requires a hex hash value".to_string());
                    };
                    let value = value
                        .to_str()
                        .ok_or("--expect-rgb888-fnv64 value must be valid UTF-8")?;
                    expected_rgb888_fnv64 = Some(parse_u64_hash(value)?);
                }
                Some("--expect-accepted-effects") => {
                    let Some(value) = iter.next() else {
                        return Err("--expect-accepted-effects requires a count".to_string());
                    };
                    expected_accepted_effects =
                        Some(parse_usize_value(value, "--expect-accepted-effects")?);
                }
                Some("--expect-initial-placeholder-pixels") => {
                    let Some(value) = iter.next() else {
                        return Err(
                            "--expect-initial-placeholder-pixels requires a count".to_string()
                        );
                    };
                    expected_initial_placeholder_pixels = Some(parse_usize_value(
                        value,
                        "--expect-initial-placeholder-pixels",
                    )?);
                }
                Some("--expect-final-placeholder-pixels") => {
                    let Some(value) = iter.next() else {
                        return Err(
                            "--expect-final-placeholder-pixels requires a count".to_string()
                        );
                    };
                    expected_final_placeholder_pixels = Some(parse_usize_value(
                        value,
                        "--expect-final-placeholder-pixels",
                    )?);
                }
                Some("--expect-rgb565-be-fnv64") => {
                    let Some(value) = iter.next() else {
                        return Err(
                            "--expect-rgb565-be-fnv64 requires a hex hash value".to_string()
                        );
                    };
                    let value = value
                        .to_str()
                        .ok_or("--expect-rgb565-be-fnv64 value must be valid UTF-8")?;
                    expected_rgb565_be_fnv64 = Some(parse_u64_hash(value)?);
                }
                Some("--expect-rgb565-le-fnv64") => {
                    let Some(value) = iter.next() else {
                        return Err(
                            "--expect-rgb565-le-fnv64 requires a hex hash value".to_string()
                        );
                    };
                    let value = value
                        .to_str()
                        .ok_or("--expect-rgb565-le-fnv64 value must be valid UTF-8")?;
                    expected_rgb565_le_fnv64 = Some(parse_u64_hash(value)?);
                }
                Some("--write-report-json") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-report-json requires an output path".to_string());
                    };
                    set_optional_path(&mut write_report_json_path, "--write-report-json", value)?;
                }
                Some("--write-initial-rgb888") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-initial-rgb888 requires an output path".to_string());
                    };
                    set_optional_path(
                        &mut write_initial_rgb888_path,
                        "--write-initial-rgb888",
                        value,
                    )?;
                }
                Some("--write-final-rgb888") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-final-rgb888 requires an output path".to_string());
                    };
                    set_optional_path(&mut write_final_rgb888_path, "--write-final-rgb888", value)?;
                }
                Some("--write-initial-rgb565-be") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-initial-rgb565-be requires an output path".to_string());
                    };
                    set_optional_path(
                        &mut write_initial_rgb565_be_path,
                        "--write-initial-rgb565-be",
                        value,
                    )?;
                }
                Some("--write-initial-rgb565-le") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-initial-rgb565-le requires an output path".to_string());
                    };
                    set_optional_path(
                        &mut write_initial_rgb565_le_path,
                        "--write-initial-rgb565-le",
                        value,
                    )?;
                }
                Some("--write-final-rgb565-be") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-final-rgb565-be requires an output path".to_string());
                    };
                    set_optional_path(
                        &mut write_final_rgb565_be_path,
                        "--write-final-rgb565-be",
                        value,
                    )?;
                }
                Some("--write-final-rgb565-le") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-final-rgb565-le requires an output path".to_string());
                    };
                    set_optional_path(
                        &mut write_final_rgb565_le_path,
                        "--write-final-rgb565-le",
                        value,
                    )?;
                }
                Some(flag) if flag.starts_with('-') => {
                    return Err(format!("unknown option `{flag}`"));
                }
                Some(positional) => {
                    return Err(format!("unexpected positional argument `{positional}`"));
                }
                None => {
                    return Err("argument is not valid UTF-8".to_string());
                }
            }
        }
        if pool_fixture_name.is_some() && pool_path.is_none() {
            return Err("--pool-fixture requires --pool".to_string());
        }
        Ok(Self {
            pool_path,
            pool_fixture_name,
            trace_path,
            strict,
            expected_accepted_effects,
            expected_initial_placeholder_pixels,
            expected_final_placeholder_pixels,
            expected_rgb888_fnv64,
            expected_rgb565_be_fnv64,
            expected_rgb565_le_fnv64,
            write_report_json_path,
            write_initial_rgb888_path,
            write_final_rgb888_path,
            write_initial_rgb565_be_path,
            write_initial_rgb565_le_path,
            write_final_rgb565_be_path,
            write_final_rgb565_le_path,
            source_address,
            canvas,
            soft_key_area,
            physical_soft_key_count,
            navigation_soft_key_count,
            soft_key_page,
        })
    }

    fn layout_config(&self) -> LayoutConfig {
        let mut config = LayoutConfig {
            canvas: self.canvas,
            soft_key_area: self.soft_key_area,
            physical_soft_key_count: self.physical_soft_key_count,
            navigation_soft_key_count: self.navigation_soft_key_count,
            soft_key_page: self.soft_key_page,
            auto_layout_gap: LayoutConfig::default().auto_layout_gap,
        };
        config.navigation_soft_key_count = config
            .navigation_soft_key_count
            .min(config.physical_soft_key_count);
        config
    }

    fn expects_framebuffer_hash(&self) -> bool {
        self.expected_rgb888_fnv64.is_some()
            || self.expected_rgb565_be_fnv64.is_some()
            || self.expected_rgb565_le_fnv64.is_some()
    }

    fn expects_trace_counts(&self) -> bool {
        self.expected_accepted_effects.is_some()
            || self.expected_initial_placeholder_pixels.is_some()
            || self.expected_final_placeholder_pixels.is_some()
    }

    fn requires_final_check(&self) -> bool {
        self.strict
            || self.expects_trace_counts()
            || self.expects_framebuffer_hash()
            || self.write_report_json_path.is_some()
            || self.write_initial_rgb888_path.is_some()
            || self.write_final_rgb888_path.is_some()
            || self.write_initial_rgb565_be_path.is_some()
            || self.write_initial_rgb565_le_path.is_some()
            || self.write_final_rgb565_be_path.is_some()
            || self.write_final_rgb565_le_path.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceCommand {
    name: String,
    payload: Vec<u8>,
}

#[derive(Debug, Clone)]
struct FramebufferSummary {
    rendered: bool,
    width: u16,
    height: u16,
    rgb888_bytes: usize,
    rgb565_bytes: usize,
    rgb888_fnv64: Option<u64>,
    rgb565_be_fnv64: Option<u64>,
    rgb565_le_fnv64: Option<u64>,
    rgb888: Vec<u8>,
    rgb565_be: Vec<u8>,
    rgb565_le: Vec<u8>,
    placeholder_pixels: Option<usize>,
    error: Option<String>,
}

fn load_pool_bytes(args: &TraceInspectArgs) -> Result<(String, Vec<u8>), String> {
    let Some(path) = args.pool_path.as_deref() else {
        return Ok(("synthetic trace pool".to_string(), demo_trace_pool_bytes()));
    };
    let fixture_name = args.pool_fixture_name.as_deref();
    let raw = fs::read(path)
        .map_err(|error| format!("failed to read pool {}: {error}", path.display()))?;
    let bytes = match path.extension().and_then(|ext| ext.to_str()) {
        Some("hex" | "txt") => {
            let text = std::str::from_utf8(&raw).map_err(|error| {
                format!("{} is not valid UTF-8 hex text: {error}", path.display())
            })?;
            parse_hex_fixture(text, fixture_name)?
        }
        _ => raw,
    };
    let source = match fixture_name {
        Some(name) => format!("{}#{name}", path.display()),
        None => path.display().to_string(),
    };
    Ok((source, bytes))
}

fn load_trace_text(args: &TraceInspectArgs) -> Result<(String, String), String> {
    let Some(path) = args.trace_path.as_deref() else {
        return Ok((
            "tests/fixtures/isobus/vt_render_trace.hex".to_string(),
            DEFAULT_TRACE_HEX.to_string(),
        ));
    };
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read trace {}: {error}", path.display()))?;
    Ok((path.display().to_string(), text))
}

fn render_summary(
    client: Option<&machbus::isobus::vt::ServerWorkingSet>,
    layout_config: LayoutConfig,
    label: &str,
    failures: &mut Vec<String>,
) -> FramebufferSummary {
    let Some(client) = client else {
        let error = "no uploaded client".to_string();
        failures.push(format!("{label} render failed: {error}"));
        return FramebufferSummary::error(error);
    };
    let runtime = match VtRenderRuntime::from_server_working_set(client, layout_config) {
        Ok(runtime) => runtime,
        Err(error) => {
            let error = format!("{error:?}");
            failures.push(format!("{label} render failed: {error}"));
            return FramebufferSummary::error(error);
        }
    };
    match FramebufferRenderer::default().try_render_runtime(&runtime) {
        Ok(frame) => {
            let rgb888 = frame.to_rgb888();
            let rgb565_be = frame.to_rgb565_be();
            let rgb565_le = frame.to_rgb565_le();
            let rgb888_fnv64 = fnv1a64(&rgb888);
            let rgb565_be_fnv64 = fnv1a64(&rgb565_be);
            let rgb565_le_fnv64 = fnv1a64(&rgb565_le);
            let placeholder_pixels =
                frame.count_colour(machbus::isobus::vt::render::style::Colour::rgb(255, 0, 255));
            println!(
                "{label:<11}: {}x{} rgb888_fnv64=0x{rgb888_fnv64:016X} rgb565_be_fnv64=0x{rgb565_be_fnv64:016X} rgb565_le_fnv64=0x{rgb565_le_fnv64:016X} placeholder_pixels={placeholder_pixels}",
                frame.width(),
                frame.height()
            );
            FramebufferSummary {
                rendered: true,
                width: frame.width(),
                height: frame.height(),
                rgb888_bytes: frame.rgb888_len(),
                rgb565_bytes: frame.rgb565_len(),
                rgb888_fnv64: Some(rgb888_fnv64),
                rgb565_be_fnv64: Some(rgb565_be_fnv64),
                rgb565_le_fnv64: Some(rgb565_le_fnv64),
                rgb888,
                rgb565_be,
                rgb565_le,
                placeholder_pixels: Some(placeholder_pixels),
                error: None,
            }
        }
        Err(error) => {
            let error = format!("{error:?}");
            failures.push(format!("{label} render failed: {error}"));
            FramebufferSummary::error(error)
        }
    }
}

impl FramebufferSummary {
    fn error(error: String) -> Self {
        Self {
            rendered: false,
            width: 0,
            height: 0,
            rgb888_bytes: 0,
            rgb565_bytes: 0,
            rgb888_fnv64: None,
            rgb565_be_fnv64: None,
            rgb565_le_fnv64: None,
            rgb888: Vec::new(),
            rgb565_be: Vec::new(),
            rgb565_le: Vec::new(),
            placeholder_pixels: None,
            error: Some(error),
        }
    }
}

fn demo_trace_pool_bytes() -> Vec<u8> {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16]))
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            4,
            &StringVariableBody {
                length: 4,
                value: b"old ".to_vec(),
            },
        ));
    pool.serialize().unwrap()
}

fn fixed_vt_command(command: u8) -> Vec<u8> {
    let mut data = [0xFFu8; 8];
    data[0] = command;
    data.to_vec()
}

fn parse_trace_commands(text: &str) -> Result<Vec<TraceCommand>, String> {
    let mut commands = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, hex)) = line.split_once('=') else {
            return Err(format!("trace row must use name=HEX: {line}"));
        };
        let name = name.trim();
        if name.is_empty() {
            return Err("trace row has an empty name".to_string());
        }
        commands.push(TraceCommand {
            name: name.to_string(),
            payload: parse_hex_bytes(hex.trim()).map_err(|error| format!("{name}: {error}"))?,
        });
    }
    if commands.is_empty() {
        return Err("trace file did not contain any command rows".to_string());
    }
    Ok(commands)
}

fn parse_hex_fixture(text: &str, fixture_name: Option<&str>) -> Result<Vec<u8>, String> {
    let mut first_named: Option<(&str, &str)> = None;
    let mut plain_hex = String::new();
    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some((name, hex)) = line.split_once('=') {
            let name = name.trim();
            let hex = hex.trim();
            if fixture_name == Some(name) {
                return parse_hex_bytes(hex).map_err(|error| format!("{name}: {error}"));
            }
            first_named.get_or_insert((name, hex));
        } else if fixture_name.is_none() {
            plain_hex.push_str(line);
        }
    }
    if let Some(name) = fixture_name {
        return Err(format!("missing named pool fixture `{name}`"));
    }
    if !plain_hex.is_empty() {
        return parse_hex_bytes(&plain_hex);
    }
    if let Some((name, hex)) = first_named {
        return parse_hex_bytes(hex).map_err(|error| format!("{name}: {error}"));
    }
    Err("pool hex fixture is empty".to_string())
}

fn parse_hex_bytes(hex: &str) -> Result<Vec<u8>, String> {
    let mut compact = String::with_capacity(hex.len());
    for ch in hex.chars() {
        if ch.is_ascii_whitespace() || ch == '_' {
            continue;
        }
        if !ch.is_ascii_hexdigit() {
            return Err(format!("invalid hex character `{ch}`"));
        }
        compact.push(ch);
    }
    if !compact.len().is_multiple_of(2) {
        return Err("hex stream has an odd number of digits".to_string());
    }
    let mut bytes = Vec::with_capacity(compact.len() / 2);
    for chunk in compact.as_bytes().chunks_exact(2) {
        let digits =
            std::str::from_utf8(chunk).map_err(|error| format!("invalid hex digits: {error}"))?;
        bytes.push(
            u8::from_str_radix(digits, 16)
                .map_err(|error| format!("invalid hex byte `{digits}`: {error}"))?,
        );
    }
    Ok(bytes)
}

fn parse_u64_hash(value: &str) -> Result<u64, String> {
    let value = value.trim();
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.is_empty() || value.len() > 16 {
        return Err("expected a one-to-sixteen digit hexadecimal hash".to_string());
    }
    u64::from_str_radix(value, 16).map_err(|error| format!("invalid hex hash `{value}`: {error}"))
}

fn parse_u8_value(value: &std::ffi::OsString, flag: &str) -> Result<u8, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{flag} value must be valid UTF-8"))?;
    let (digits, radix) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or((value, 10), |digits| (digits, 16));
    u8::from_str_radix(digits, radix)
        .map_err(|error| format!("invalid {flag} value `{value}`: {error}"))
}

fn parse_u16_value(value: &std::ffi::OsString, flag: &str) -> Result<u16, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{flag} value must be valid UTF-8"))?;
    let (digits, radix) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or((value, 10), |digits| (digits, 16));
    u16::from_str_radix(digits, radix)
        .map_err(|error| format!("invalid {flag} value `{value}`: {error}"))
}

fn parse_usize_value(value: &std::ffi::OsString, flag: &str) -> Result<usize, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{flag} value must be valid UTF-8"))?;
    value
        .parse::<usize>()
        .map_err(|error| format!("invalid {flag} count `{value}`: {error}"))
}

fn parse_canvas(value: &std::ffi::OsString, flag: &str) -> Result<(u16, u16), String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{flag} value must be valid UTF-8"))?;
    let Some((width, height)) = value.split_once('x').or_else(|| value.split_once('X')) else {
        return Err(format!("{flag} must use WIDTHxHEIGHT"));
    };
    let width = parse_nonzero_u16_str(width, flag, "width")?;
    let height = parse_nonzero_u16_str(height, flag, "height")?;
    Ok((width, height))
}

fn parse_rect(value: &std::ffi::OsString, flag: &str) -> Result<Rect, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{flag} value must be valid UTF-8"))?;
    let mut parts = value.split(',');
    let x = parse_i32_str(parts.next().unwrap_or(""), flag, "x")?;
    let y = parse_i32_str(parts.next().unwrap_or(""), flag, "y")?;
    let w = parse_nonzero_u16_str(parts.next().unwrap_or(""), flag, "width")?;
    let h = parse_nonzero_u16_str(parts.next().unwrap_or(""), flag, "height")?;
    if parts.next().is_some() {
        return Err(format!("{flag} must use X,Y,W,H"));
    }
    Ok(Rect::new(x, y, w, h))
}

fn parse_u16_str(value: &str, flag: &str, field: &str) -> Result<u16, String> {
    if value.is_empty() {
        return Err(format!("{flag} missing {field}"));
    }
    value
        .parse::<u16>()
        .map_err(|error| format!("invalid {flag} {field} `{value}`: {error}"))
}

fn parse_i32_str(value: &str, flag: &str, field: &str) -> Result<i32, String> {
    if value.is_empty() {
        return Err(format!("{flag} missing {field}"));
    }
    value
        .parse::<i32>()
        .map_err(|error| format!("invalid {flag} {field} `{value}`: {error}"))
}

fn parse_nonzero_u16_str(value: &str, flag: &str, field: &str) -> Result<u16, String> {
    let parsed = parse_u16_str(value, flag, field)?;
    if parsed == 0 {
        return Err(format!("{flag} {field} must be non-zero"));
    }
    Ok(parsed)
}

fn set_optional_path(
    slot: &mut Option<PathBuf>,
    flag: &str,
    value: &std::ffi::OsString,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("{flag} may only be provided once"));
    }
    if value.is_empty() {
        return Err(format!("{flag} path may not be empty"));
    }
    *slot = Some(PathBuf::from(value));
    Ok(())
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

fn fnv1a64_trace(commands: &[TraceCommand]) -> u64 {
    let mut bytes = Vec::new();
    for command in commands {
        bytes.extend_from_slice(command.name.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&command.payload);
        bytes.push(0xFF);
    }
    fnv1a64(&bytes)
}

fn write_text_file(format: &str, path: &Path, text: &str, failures: &mut Vec<String>) {
    match fs::write(path, text) {
        Ok(()) => println!("wrote_{format}: {} bytes -> {}", text.len(), path.display()),
        Err(error) => failures.push(format!(
            "failed to write {format} {}: {error}",
            path.display()
        )),
    }
}

fn write_frame_rgb888_file(
    format: &str,
    path: &Path,
    summary: &FramebufferSummary,
    failures: &mut Vec<String>,
) {
    if !summary.rendered {
        failures.push(format!(
            "failed to write {format} {}: framebuffer was not rendered",
            path.display()
        ));
        return;
    }
    match fs::write(path, &summary.rgb888) {
        Ok(()) => println!(
            "wrote_{format}: {} bytes -> {}",
            summary.rgb888.len(),
            path.display()
        ),
        Err(error) => failures.push(format!(
            "failed to write {format} {}: {error}",
            path.display()
        )),
    }
}

fn write_frame_rgb565_be_file(
    format: &str,
    path: &Path,
    summary: &FramebufferSummary,
    failures: &mut Vec<String>,
) {
    write_frame_bytes_file(format, path, summary, &summary.rgb565_be, failures);
}

fn write_frame_rgb565_le_file(
    format: &str,
    path: &Path,
    summary: &FramebufferSummary,
    failures: &mut Vec<String>,
) {
    write_frame_bytes_file(format, path, summary, &summary.rgb565_le, failures);
}

fn write_frame_bytes_file(
    format: &str,
    path: &Path,
    summary: &FramebufferSummary,
    bytes: &[u8],
    failures: &mut Vec<String>,
) {
    if !summary.rendered {
        failures.push(format!(
            "failed to write {format} {}: framebuffer was not rendered",
            path.display()
        ));
        return;
    }
    match fs::write(path, bytes) {
        Ok(()) => println!(
            "wrote_{format}: {} bytes -> {}",
            bytes.len(),
            path.display()
        ),
        Err(error) => failures.push(format!(
            "failed to write {format} {}: {error}",
            path.display()
        )),
    }
}

struct TraceReportInput<'a> {
    pool_source: &'a str,
    pool_buffer_bytes: usize,
    pool_buffer_fnv64: u64,
    layout_config: LayoutConfig,
    trace_source: &'a str,
    trace_payload_fnv64: u64,
    commands: &'a [TraceCommand],
    strict: bool,
    expected_accepted_effects: Option<usize>,
    expected_initial_placeholder_pixels: Option<usize>,
    expected_final_placeholder_pixels: Option<usize>,
    expected_rgb888_fnv64: Option<u64>,
    expected_rgb565_be_fnv64: Option<u64>,
    expected_rgb565_le_fnv64: Option<u64>,
    write_report_json_path: Option<&'a Path>,
    write_initial_rgb888_path: Option<&'a Path>,
    write_final_rgb888_path: Option<&'a Path>,
    write_initial_rgb565_be_path: Option<&'a Path>,
    write_initial_rgb565_le_path: Option<&'a Path>,
    write_final_rgb565_be_path: Option<&'a Path>,
    write_final_rgb565_le_path: Option<&'a Path>,
    initial: &'a FramebufferSummary,
    final_summary: &'a FramebufferSummary,
    accepted_effects: &'a [String],
    failures: &'a [String],
}

fn trace_report_json(input: TraceReportInput<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"schema\": \"machbus-vt-trace-inspect-report-v1\",\n");
    out.push_str("  \"pool\": { \"source\": ");
    push_json_string(&mut out, input.pool_source);
    out.push_str(&format!(
        ", \"bytes\": {}, \"fnv1a64\": \"0x{:016X}\" }},\n",
        input.pool_buffer_bytes, input.pool_buffer_fnv64
    ));
    out.push_str(&format!(
        "  \"layout_profile\": {{ \"canvas_width\": {}, \"canvas_height\": {}, \"soft_key_area\": {{ \"x\": {}, \"y\": {}, \"width\": {}, \"height\": {} }}, \"physical_soft_key_count\": {}, \"navigation_soft_key_count\": {}, \"soft_key_page\": {} }},\n",
        input.layout_config.canvas.0,
        input.layout_config.canvas.1,
        input.layout_config.soft_key_area.x,
        input.layout_config.soft_key_area.y,
        input.layout_config.soft_key_area.w,
        input.layout_config.soft_key_area.h,
        input.layout_config.physical_soft_key_count,
        input.layout_config.navigation_soft_key_count,
        input.layout_config.soft_key_page
    ));
    out.push_str("  \"trace\": { \"source\": ");
    push_json_string(&mut out, input.trace_source);
    out.push_str(&format!(
        ", \"command_count\": {}, \"payload_fnv1a64\": \"0x{:016X}\", \"commands\": ",
        input.commands.len(),
        input.trace_payload_fnv64
    ));
    push_trace_command_array(&mut out, input.commands);
    out.push_str(" },\n");
    out.push_str("  \"initial_framebuffer\": ");
    push_framebuffer_summary(&mut out, input.initial);
    out.push_str(",\n  \"final_framebuffer\": ");
    push_framebuffer_summary(&mut out, input.final_summary);
    out.push_str(",\n  \"accepted_effects\": ");
    push_json_string_array(&mut out, input.accepted_effects);
    out.push_str(",\n  \"artifacts\": {\n");
    out.push_str("    \"report_json\": ");
    push_json_path_or_null(&mut out, input.write_report_json_path);
    out.push_str(",\n    \"initial_rgb888\": ");
    push_json_path_or_null(&mut out, input.write_initial_rgb888_path);
    out.push_str(",\n    \"final_rgb888\": ");
    push_json_path_or_null(&mut out, input.write_final_rgb888_path);
    out.push_str(",\n    \"initial_rgb565_be\": ");
    push_json_path_or_null(&mut out, input.write_initial_rgb565_be_path);
    out.push_str(",\n    \"initial_rgb565_le\": ");
    push_json_path_or_null(&mut out, input.write_initial_rgb565_le_path);
    out.push_str(",\n    \"final_rgb565_be\": ");
    push_json_path_or_null(&mut out, input.write_final_rgb565_be_path);
    out.push_str(",\n    \"final_rgb565_le\": ");
    push_json_path_or_null(&mut out, input.write_final_rgb565_le_path);
    out.push_str("\n  }");
    out.push_str(",\n  \"checks\": {\n");
    out.push_str(&format!("    \"strict_requested\": {},\n", input.strict));
    out.push_str("    \"expected_accepted_effects\": ");
    push_json_usize_or_null(&mut out, input.expected_accepted_effects);
    out.push_str(",\n    \"expected_initial_placeholder_pixels\": ");
    push_json_usize_or_null(&mut out, input.expected_initial_placeholder_pixels);
    out.push_str(",\n    \"expected_final_placeholder_pixels\": ");
    push_json_usize_or_null(&mut out, input.expected_final_placeholder_pixels);
    out.push_str(",\n");
    out.push_str("    \"expected_rgb888_fnv64\": ");
    push_json_hash_or_null(&mut out, input.expected_rgb888_fnv64);
    out.push_str(",\n    \"expected_rgb565_be_fnv64\": ");
    push_json_hash_or_null(&mut out, input.expected_rgb565_be_fnv64);
    out.push_str(",\n    \"expected_rgb565_le_fnv64\": ");
    push_json_hash_or_null(&mut out, input.expected_rgb565_le_fnv64);
    out.push_str(",\n    \"failures\": ");
    push_json_string_array(&mut out, input.failures);
    out.push_str("\n  }\n");
    out.push_str("}\n");
    out
}

fn push_trace_command_array(out: &mut String, commands: &[TraceCommand]) {
    out.push('[');
    for (index, command) in commands.iter().enumerate() {
        if index != 0 {
            out.push_str(", ");
        }
        out.push_str("{ \"name\": ");
        push_json_string(out, &command.name);
        out.push_str(&format!(", \"bytes\": {} }}", command.payload.len()));
    }
    out.push(']');
}

fn push_framebuffer_summary(out: &mut String, summary: &FramebufferSummary) {
    out.push_str("{ ");
    out.push_str(&format!(
        "\"rendered\": {}, \"width\": {}, \"height\": {}, \"rgb888_bytes\": {}, \"rgb565_bytes\": {}, \"rgb888_fnv64\": ",
        summary.rendered, summary.width, summary.height, summary.rgb888_bytes, summary.rgb565_bytes
    ));
    push_json_hash_or_null(out, summary.rgb888_fnv64);
    out.push_str(", \"rgb565_be_fnv64\": ");
    push_json_hash_or_null(out, summary.rgb565_be_fnv64);
    out.push_str(", \"rgb565_le_fnv64\": ");
    push_json_hash_or_null(out, summary.rgb565_le_fnv64);
    out.push_str(", \"placeholder_pixels\": ");
    push_json_usize_or_null(out, summary.placeholder_pixels);
    out.push_str(", \"error\": ");
    push_json_str_or_null(out, summary.error.as_deref());
    out.push_str(" }");
}

fn push_json_hash_or_null(out: &mut String, value: Option<u64>) {
    match value {
        Some(value) => out.push_str(&format!("\"0x{value:016X}\"")),
        None => out.push_str("null"),
    }
}

fn push_json_usize_or_null(out: &mut String, value: Option<usize>) {
    match value {
        Some(value) => out.push_str(&value.to_string()),
        None => out.push_str("null"),
    }
}

fn push_json_str_or_null(out: &mut String, value: Option<&str>) {
    match value {
        Some(value) => push_json_string(out, value),
        None => out.push_str("null"),
    }
}

fn push_json_path_or_null(out: &mut String, value: Option<&Path>) {
    match value {
        Some(value) => push_json_string(out, &value.display().to_string()),
        None => out.push_str("null"),
    }
}

fn push_json_string_array(out: &mut String, values: &[String]) {
    out.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.push_str(", ");
        }
        push_json_string(out, value);
    }
    out.push(']');
}

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(ch))),
            ch => out.push(ch),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trace_commands_reads_named_rows() {
        let commands = parse_trace_commands(
            "
            # comment
            first=0102
            second = AA_BB
            ",
        )
        .unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].name, "first");
        assert_eq!(commands[0].payload, vec![1, 2]);
        assert_eq!(commands[1].payload, vec![0xAA, 0xBB]);
    }

    #[test]
    fn parse_trace_commands_rejects_malformed_rows() {
        assert!(parse_trace_commands("").is_err());
        assert!(parse_trace_commands("not-named").is_err());
        assert!(parse_trace_commands("x=0").is_err());
        assert!(parse_trace_commands("x=zz").is_err());
    }

    #[test]
    fn parse_pool_hex_fixture_handles_plain_and_named_rows() {
        assert_eq!(
            parse_hex_fixture(
                "
                # comment
                01 02
                03_04
                ",
                None
            )
            .unwrap(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            parse_hex_fixture("first=0102\nsecond=0A0B", Some("second")).unwrap(),
            vec![0x0A, 0x0B]
        );
        assert!(parse_hex_fixture("first=0102", Some("missing")).is_err());
    }

    #[test]
    fn parse_args_handles_trace_pool_report_and_hash() {
        let args = TraceInspectArgs::parse(&[
            "--strict".into(),
            "--pool".into(),
            "pool.iop".into(),
            "--pool-fixture".into(),
            "valid_pool".into(),
            "--trace".into(),
            "trace.hex".into(),
            "--canvas".into(),
            "320x240".into(),
            "--soft-key-area".into(),
            "320,0,64,240".into(),
            "--physical-soft-keys".into(),
            "10".into(),
            "--navigation-soft-keys".into(),
            "2".into(),
            "--soft-key-page".into(),
            "1".into(),
            "--source-address".into(),
            "0x42".into(),
            "--expect-rgb888-fnv64".into(),
            "0x0123456789ABCDEF".into(),
            "--expect-rgb565-be-fnv64".into(),
            "0x1111222233334444".into(),
            "--expect-rgb565-le-fnv64".into(),
            "0x5555666677778888".into(),
            "--expect-accepted-effects".into(),
            "3".into(),
            "--expect-initial-placeholder-pixels".into(),
            "0".into(),
            "--expect-final-placeholder-pixels".into(),
            "0".into(),
            "--write-report-json".into(),
            "report.json".into(),
            "--write-initial-rgb888".into(),
            "before.rgb".into(),
            "--write-final-rgb888".into(),
            "after.rgb".into(),
            "--write-initial-rgb565-be".into(),
            "before-be.rgb565".into(),
            "--write-initial-rgb565-le".into(),
            "before-le.rgb565".into(),
            "--write-final-rgb565-be".into(),
            "after-be.rgb565".into(),
            "--write-final-rgb565-le".into(),
            "after-le.rgb565".into(),
        ])
        .unwrap();
        assert!(args.strict);
        assert_eq!(args.pool_path.as_deref(), Some(Path::new("pool.iop")));
        assert_eq!(args.pool_fixture_name.as_deref(), Some("valid_pool"));
        assert_eq!(args.trace_path.as_deref(), Some(Path::new("trace.hex")));
        assert_eq!(args.layout_config().canvas, (320, 240));
        assert_eq!(
            args.layout_config().soft_key_area,
            Rect::new(320, 0, 64, 240)
        );
        assert_eq!(args.layout_config().physical_soft_key_count, 10);
        assert_eq!(args.layout_config().navigation_soft_key_count, 2);
        assert_eq!(args.layout_config().soft_key_page, 1);
        assert_eq!(args.source_address, 0x42);
        assert_eq!(args.expected_rgb888_fnv64, Some(0x0123_4567_89AB_CDEF));
        assert_eq!(args.expected_rgb565_be_fnv64, Some(0x1111_2222_3333_4444));
        assert_eq!(args.expected_rgb565_le_fnv64, Some(0x5555_6666_7777_8888));
        assert_eq!(args.expected_accepted_effects, Some(3));
        assert_eq!(args.expected_initial_placeholder_pixels, Some(0));
        assert_eq!(args.expected_final_placeholder_pixels, Some(0));
        assert!(args.expects_framebuffer_hash());
        assert!(args.expects_trace_counts());
        assert_eq!(
            args.write_report_json_path.as_deref(),
            Some(Path::new("report.json"))
        );
        assert_eq!(
            args.write_initial_rgb888_path.as_deref(),
            Some(Path::new("before.rgb"))
        );
        assert_eq!(
            args.write_final_rgb888_path.as_deref(),
            Some(Path::new("after.rgb"))
        );
        assert_eq!(
            args.write_initial_rgb565_be_path.as_deref(),
            Some(Path::new("before-be.rgb565"))
        );
        assert_eq!(
            args.write_initial_rgb565_le_path.as_deref(),
            Some(Path::new("before-le.rgb565"))
        );
        assert_eq!(
            args.write_final_rgb565_be_path.as_deref(),
            Some(Path::new("after-be.rgb565"))
        );
        assert_eq!(
            args.write_final_rgb565_le_path.as_deref(),
            Some(Path::new("after-le.rgb565"))
        );
    }

    #[test]
    fn parse_args_rejects_unknown_or_duplicate_options() {
        assert!(TraceInspectArgs::parse(&["--nope".into()]).is_err());
        assert!(TraceInspectArgs::parse(&["positional".into()]).is_err());
        assert!(TraceInspectArgs::parse(&["--pool".into()]).is_err());
        assert!(TraceInspectArgs::parse(&["--pool-fixture".into(), "x".into()]).is_err());
        assert!(TraceInspectArgs::parse(&["--canvas".into(), "320".into()]).is_err());
        assert!(TraceInspectArgs::parse(&["--soft-key-area".into(), "0,0,0,1".into()]).is_err());
        assert!(TraceInspectArgs::parse(&["--expect-rgb565-be-fnv64".into()]).is_err());
        assert!(
            TraceInspectArgs::parse(&["--expect-rgb565-le-fnv64".into(), "not-hex".into()])
                .is_err()
        );
        assert!(TraceInspectArgs::parse(&["--expect-accepted-effects".into()]).is_err());
        assert!(
            TraceInspectArgs::parse(&[
                "--expect-final-placeholder-pixels".into(),
                "not-a-count".into()
            ])
            .is_err()
        );
        assert!(
            TraceInspectArgs::parse(&[
                "--trace".into(),
                "a.hex".into(),
                "--trace".into(),
                "b.hex".into()
            ])
            .is_err()
        );
        assert!(
            TraceInspectArgs::parse(&[
                "--write-final-rgb888".into(),
                "a.rgb".into(),
                "--write-final-rgb888".into(),
                "b.rgb".into()
            ])
            .is_err()
        );
        assert!(
            TraceInspectArgs::parse(&[
                "--write-final-rgb565-be".into(),
                "a.rgb565".into(),
                "--write-final-rgb565-be".into(),
                "b.rgb565".into()
            ])
            .is_err()
        );
    }

    #[test]
    fn trace_report_json_contains_reproducibility_fields() {
        let commands = vec![TraceCommand {
            name: "cmd".to_string(),
            payload: vec![1, 2, 3],
        }];
        let effects = vec!["ChangeStringValue { id: ObjectID(4) }".to_string()];
        let failures = vec!["bad \"quote\"".to_string()];
        let initial = FramebufferSummary {
            rendered: true,
            width: 2,
            height: 1,
            rgb888_bytes: 6,
            rgb565_bytes: 4,
            rgb888_fnv64: Some(0x1111),
            rgb565_be_fnv64: Some(0x3333),
            rgb565_le_fnv64: Some(0x4444),
            rgb888: vec![1, 2, 3, 4, 5, 6],
            rgb565_be: vec![1, 2, 3, 4],
            rgb565_le: vec![2, 1, 4, 3],
            placeholder_pixels: Some(0),
            error: None,
        };
        let final_summary = FramebufferSummary {
            rgb888_fnv64: Some(0x2222),
            ..initial.clone()
        };
        let json = trace_report_json(TraceReportInput {
            pool_source: "pool.iop",
            pool_buffer_bytes: 42,
            pool_buffer_fnv64: 0xAAAA,
            layout_config: LayoutConfig {
                canvas: (320, 240),
                soft_key_area: Rect::new(320, 0, 64, 240),
                physical_soft_key_count: 10,
                navigation_soft_key_count: 2,
                soft_key_page: 1,
                auto_layout_gap: 4,
            },
            trace_source: "trace.hex",
            trace_payload_fnv64: 0xBBBB,
            commands: &commands,
            strict: true,
            expected_accepted_effects: Some(1),
            expected_initial_placeholder_pixels: Some(0),
            expected_final_placeholder_pixels: Some(0),
            expected_rgb888_fnv64: Some(0x2222),
            expected_rgb565_be_fnv64: Some(0x3333),
            expected_rgb565_le_fnv64: Some(0x4444),
            write_report_json_path: Some(Path::new("trace-report.json")),
            write_initial_rgb888_path: Some(Path::new("before.rgb")),
            write_final_rgb888_path: Some(Path::new("after.rgb")),
            write_initial_rgb565_be_path: Some(Path::new("before-be.rgb565")),
            write_initial_rgb565_le_path: Some(Path::new("before-le.rgb565")),
            write_final_rgb565_be_path: Some(Path::new("after-be.rgb565")),
            write_final_rgb565_le_path: Some(Path::new("after-le.rgb565")),
            initial: &initial,
            final_summary: &final_summary,
            accepted_effects: &effects,
            failures: &failures,
        });
        assert!(json.contains("\"schema\": \"machbus-vt-trace-inspect-report-v1\""));
        assert!(json.contains("\"fnv1a64\": \"0x000000000000AAAA\""));
        assert!(json.contains("\"physical_soft_key_count\": 10"));
        assert!(json.contains("\"soft_key_page\": 1"));
        assert!(json.contains("\"payload_fnv1a64\": \"0x000000000000BBBB\""));
        assert!(json.contains("\"expected_accepted_effects\": 1"));
        assert!(json.contains("\"expected_initial_placeholder_pixels\": 0"));
        assert!(json.contains("\"expected_final_placeholder_pixels\": 0"));
        assert!(json.contains("\"report_json\": \"trace-report.json\""));
        assert!(json.contains("\"initial_rgb888\": \"before.rgb\""));
        assert!(json.contains("\"final_rgb888\": \"after.rgb\""));
        assert!(json.contains("\"initial_rgb565_be\": \"before-be.rgb565\""));
        assert!(json.contains("\"initial_rgb565_le\": \"before-le.rgb565\""));
        assert!(json.contains("\"final_rgb565_be\": \"after-be.rgb565\""));
        assert!(json.contains("\"final_rgb565_le\": \"after-le.rgb565\""));
        assert!(json.contains("\"rgb565_be_fnv64\": \"0x0000000000003333\""));
        assert!(json.contains("\"rgb565_le_fnv64\": \"0x0000000000004444\""));
        assert!(json.contains("bad \\\"quote\\\""));
    }
}
