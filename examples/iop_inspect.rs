//! IOP inspector — load an object-pool byte buffer, run the full VT
//! render pipeline, and print a human-readable report:
//!
//! - pool size and active mask,
//! - per-node placement / kind / visibility,
//! - soft-key cells,
//! - unsupported-object records,
//! - the render coverage ledger (CSV).
//!
//! This is the developer-facing companion to `vt_gtui_server.rs`. It
//! exists so a contributor can drop in any IOP byte buffer (real or
//! synthetic) and immediately see how the renderer classifies every
//! object without writing a test.
//!
//! Run with
//! `cargo run --example iop_inspect -- [--strict] [--active-mask ID] [--physical-soft-keys N] [--navigation-soft-keys N] [--expect-rgb888-fnv64 HEX] [--write-report-json PATH] [--write-rgb888 PATH] [path/to/pool.iop]`.
//! Reviewable `.hex` fixtures are also accepted; pass an optional fixture name
//! after the path for named `label=HEX` fixture files.

use machbus::isobus::vt::render::framebuffer::FramebufferRenderer;
use machbus::isobus::vt::render::gtui::{GtuiRenderer, RenderCommand};
use machbus::isobus::vt::render::scene::Rect;
use machbus::isobus::vt::render::style::Colour;
use machbus::isobus::vt::render::{IopDocument, LayoutConfig, LayoutEngine};
use machbus::isobus::vt::{
    DataMaskBody, FontAttributesBody, InputNumberBody, NumberVariableBody, ObjectID, ObjectPool,
    OutputNumberBody, OutputStringBody, StringVariableBody, WorkingSetBody, create_data_mask,
    create_font_attributes, create_input_number, create_number_variable, create_output_number,
    create_output_string, create_string_variable, create_working_set,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    println!("=== IOP Inspector (VT render pipeline) ===\n");

    let raw_args: Vec<_> = env::args_os().skip(1).collect();
    if raw_args.iter().any(|arg| {
        arg.to_str()
            .is_some_and(|arg| matches!(arg, "-h" | "--help"))
    }) {
        print_usage();
        return;
    }

    let args = match InspectArgs::parse(&raw_args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    };

    let (source, bytes) = match load_input_bytes(&args) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    };
    println!("source     : {source}");
    println!("pool buffer: {} bytes", bytes.len());

    let layout_config = args.layout_config();
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

    let doc = match IopDocument::load(&bytes, layout_config) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("pool rejected: {e}");
            process::exit(1);
        }
    };
    let doc = if let Some(active_mask) = args.active_mask {
        let engine = LayoutEngine::new(layout_config);
        doc.with_scene(&engine, active_mask)
    } else {
        doc
    };

    let mut failures = Vec::new();
    let pool_buffer_fnv64 = fnv1a64(&bytes);
    println!("pool objects: {}", doc.pool().size());
    println!("pool hash   : fnv1a64=0x{pool_buffer_fnv64:016X}");
    println!("active mask : 0x{:04X}", doc.scene().active_mask.raw());
    println!(
        "canvas      : {}x{} px",
        doc.scene().mask_rect.w,
        doc.scene().mask_rect.h
    );

    println!("\n--- scene nodes ---");
    for n in &doc.scene().nodes {
        let vis = if n.visible { "vis" } else { "hid" };
        let en = if n.enabled { "en" } else { "--" };
        println!(
            "  id=0x{:04X} type={:<16} [{}/{}] rect=({:>3},{:>3},{:>3},{:>3})",
            n.id.raw(),
            format!("{:?}", n.object_type),
            vis,
            en,
            n.rect.x,
            n.rect.y,
            n.rect.w,
            n.rect.h
        );
    }

    if !doc.scene().soft_keys.is_empty() {
        println!("\n--- soft keys ---");
        for sk in &doc.scene().soft_keys {
            println!(
                "  id=0x{:04X} rect=({:>3},{:>3},{:>3},{:>3})",
                sk.id.raw(),
                sk.rect.x,
                sk.rect.y,
                sk.rect.w,
                sk.rect.h
            );
        }
    }

    if !doc.scene().unsupported.is_empty() {
        println!("\n--- unsupported (reported, not dropped) ---");
        for r in &doc.scene().unsupported {
            println!(
                "  id=0x{:04X} type={:?} — {}",
                r.id.raw(),
                r.object_type,
                r.reason
            );
        }
    }

    let cov = doc.coverage();
    println!("\n--- coverage ledger ---");
    println!(
        "objects={} drawable={} unsupported={}",
        cov.total_objects, cov.total_drawable, cov.total_unsupported
    );
    if args.strict && cov.total_unsupported != 0 {
        failures.push(format!(
            "strict mode: {} unsupported scene records",
            cov.total_unsupported
        ));
    }
    if let Some(expected) = args.expected_unsupported_records
        && expected != cov.total_unsupported
    {
        failures.push(format!(
            "unsupported scene record count mismatch: expected {expected}, got {}",
            cov.total_unsupported
        ));
    }
    println!();
    print!("{}", cov.to_csv());

    // Also show the first few GTUI draw commands so the consumer can
    // see the retained-mode command list the backend produces.
    let cmds = GtuiRenderer::default().render(doc.scene());
    println!("\n--- GTUI command list ({} commands) ---", cmds.len());
    let preview = cmds.len().min(8);
    for c in cmds.iter().take(preview) {
        print_command(c);
    }
    if cmds.len() > preview {
        println!("  ... ({} more)", cmds.len() - preview);
    }

    println!("\n--- framebuffer snapshot ---");
    let framebuffer_summary = match FramebufferRenderer::default().try_render_scene(doc.scene()) {
        Ok(frame) => {
            let placeholder = Colour::rgb(255, 0, 255);
            let placeholder_pixels = frame.count_colour(placeholder);
            let rgb888 = frame.to_rgb888();
            let rgb565_be = frame.to_rgb565_be();
            let rgb565_le = frame.to_rgb565_le();
            let rgb888_hash = fnv1a64(&rgb888);
            let rgb565_be_hash = fnv1a64(&rgb565_be);
            let rgb565_le_hash = fnv1a64(&rgb565_le);
            let summary = FramebufferSummary {
                width: frame.width(),
                height: frame.height(),
                pixels: frame.pixels().len(),
                rgb888_bytes: frame.rgb888_len(),
                rgb565_bytes: frame.rgb565_len(),
                rgb888_fnv64: Some(rgb888_hash),
                rgb565_be_fnv64: Some(rgb565_be_hash),
                rgb565_le_fnv64: Some(rgb565_le_hash),
                placeholder_pixels: Some(placeholder_pixels),
                error: None,
            };
            println!(
                "  size={}x{} pixels={} rgb888_bytes={} rgb565_bytes={} rgb888_fnv64=0x{rgb888_hash:016X} rgb565_be_fnv64=0x{rgb565_be_hash:016X} rgb565_le_fnv64=0x{rgb565_le_hash:016X} placeholder_pixels={}",
                summary.width,
                summary.height,
                summary.pixels,
                summary.rgb888_bytes,
                summary.rgb565_bytes,
                placeholder_pixels
            );
            if let Some(expected) = args.expected_rgb888_fnv64
                && expected != rgb888_hash
            {
                failures.push(format!(
                    "framebuffer RGB888 FNV-1a hash mismatch: expected 0x{expected:016X}, got 0x{rgb888_hash:016X}"
                ));
            }
            if let Some(expected) = args.expected_rgb565_be_fnv64
                && expected != rgb565_be_hash
            {
                failures.push(format!(
                    "framebuffer RGB565 big-endian FNV-1a hash mismatch: expected 0x{expected:016X}, got 0x{rgb565_be_hash:016X}"
                ));
            }
            if let Some(expected) = args.expected_rgb565_le_fnv64
                && expected != rgb565_le_hash
            {
                failures.push(format!(
                    "framebuffer RGB565 little-endian FNV-1a hash mismatch: expected 0x{expected:016X}, got 0x{rgb565_le_hash:016X}"
                ));
            }
            if let Some(path) = args.write_rgb888_path.as_deref() {
                write_snapshot_file("rgb888", path, &rgb888, &mut failures);
            }
            if let Some(path) = args.write_rgb565_be_path.as_deref() {
                write_snapshot_file("rgb565_be", path, &rgb565_be, &mut failures);
            }
            if let Some(path) = args.write_rgb565_le_path.as_deref() {
                write_snapshot_file("rgb565_le", path, &rgb565_le, &mut failures);
            }
            if args.strict && placeholder_pixels != 0 {
                failures.push(format!(
                    "strict mode: framebuffer contains {placeholder_pixels} placeholder pixels"
                ));
            }
            if let Some(expected) = args.expected_placeholder_pixels
                && expected != placeholder_pixels
            {
                failures.push(format!(
                    "framebuffer placeholder pixel count mismatch: expected {expected}, got {placeholder_pixels}"
                ));
            }
            summary
        }
        Err(error) => {
            let error = format!("{error:?}");
            println!("  not rendered: {error}");
            if args.strict
                || args.writes_framebuffer_snapshot()
                || args.write_report_json_path.is_some()
            {
                failures.push(format!("framebuffer render failed: {error}"));
            }
            FramebufferSummary {
                width: 0,
                height: 0,
                pixels: 0,
                rgb888_bytes: 0,
                rgb565_bytes: 0,
                rgb888_fnv64: None,
                rgb565_be_fnv64: None,
                rgb565_le_fnv64: None,
                placeholder_pixels: None,
                error: Some(error),
            }
        }
    };

    if let Some(path) = args.write_report_json_path.as_deref() {
        let report = evidence_report_json(EvidenceReportInput {
            source: &source,
            pool_buffer_bytes: bytes.len(),
            pool_buffer_fnv64,
            layout_config,
            pool_objects: doc.pool().size(),
            active_mask: doc.scene().active_mask.raw(),
            canvas_width: doc.scene().mask_rect.w,
            canvas_height: doc.scene().mask_rect.h,
            scene_nodes: doc.scene().nodes.len(),
            soft_keys: doc.scene().soft_keys.len(),
            unsupported_records: doc.scene().unsupported.len(),
            coverage_total_objects: cov.total_objects,
            coverage_total_drawable: cov.total_drawable,
            coverage_total_unsupported: cov.total_unsupported,
            gtui_commands: cmds.len(),
            strict: args.strict,
            expected_unsupported_records: args.expected_unsupported_records,
            expected_placeholder_pixels: args.expected_placeholder_pixels,
            expected_rgb888_fnv64: args.expected_rgb888_fnv64,
            expected_rgb565_be_fnv64: args.expected_rgb565_be_fnv64,
            expected_rgb565_le_fnv64: args.expected_rgb565_le_fnv64,
            write_report_json_path: args.write_report_json_path.as_deref(),
            write_rgb888_path: args.write_rgb888_path.as_deref(),
            write_rgb565_be_path: args.write_rgb565_be_path.as_deref(),
            write_rgb565_le_path: args.write_rgb565_le_path.as_deref(),
            framebuffer: &framebuffer_summary,
            failures: &failures,
        });
        write_text_file("report_json", path, &report, &mut failures);
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
    println!("  cargo run --example iop_inspect");
    println!("  cargo run --example iop_inspect -- --strict");
    println!("  cargo run --example iop_inspect -- --active-mask 0x07D0");
    println!(
        "  cargo run --example iop_inspect -- --physical-soft-keys 10 --navigation-soft-keys 2"
    );
    println!("  cargo run --example iop_inspect -- --canvas 480x240 --soft-key-area 480,0,64,240");
    println!("  cargo run --example iop_inspect -- --expect-rgb888-fnv64 0x0123456789ABCDEF");
    println!("  cargo run --example iop_inspect -- --expect-rgb565-be-fnv64 0x0123456789ABCDEF");
    println!("  cargo run --example iop_inspect -- --expect-rgb565-le-fnv64 0x0123456789ABCDEF");
    println!("  cargo run --example iop_inspect -- --expect-unsupported-records 0");
    println!("  cargo run --example iop_inspect -- --expect-placeholder-pixels 0");
    println!("  cargo run --example iop_inspect -- --write-report-json report.json");
    println!("  cargo run --example iop_inspect -- --write-rgb888 snapshot.rgb");
    println!("  cargo run --example iop_inspect -- --write-rgb565-be snapshot.rgb565");
    println!("  cargo run --example iop_inspect -- --write-rgb565-le snapshot-le.rgb565");
    println!("  cargo run --example iop_inspect -- path/to/pool.iop");
    println!(
        "  cargo run --example iop_inspect -- tests/fixtures/isobus/vt_object_pool.hex valid_ws_datamask"
    );
    println!(
        "  cargo run --example iop_inspect -- --strict tests/fixtures/isobus/vt_object_pool.hex valid_ws_datamask"
    );
    println!();
    println!("Raw .iop/.bin files are read as bytes. Text .hex files may contain either");
    println!("one plain hex stream or named fixture lines in the form `name=HEX`.");
    println!("--strict exits non-zero when unsupported scene records, framebuffer render");
    println!("errors, or placeholder pixels are present.");
    println!("--active-mask selects a Data Mask or Alarm Mask by object id instead of");
    println!("the Working Set's default active mask.");
    println!("--expect-unsupported-records and --expect-placeholder-pixels let a");
    println!("candidate fixture keep explicit caveat counts without accepting drift.");
    println!("--expect-rgb888-fnv64, --expect-rgb565-be-fnv64, and");
    println!("--expect-rgb565-le-fnv64 exit non-zero when framebuffer FNV-1a hashes");
    println!("differ, which is useful for repeatable external-pool evidence gates.");
    println!("--write-rgb888, --write-rgb565-be, and --write-rgb565-le write raw packed");
    println!("framebuffer bytes for archiving or hardware-display smoke tests.");
    println!("--write-report-json writes a stable machine-readable summary for fixture");
    println!("provenance notes, CI artifacts, and external-pool pre-promotion review.");
    println!("--canvas, --soft-key-area, --physical-soft-keys, --navigation-soft-keys,");
    println!("and --soft-key-page let candidate external pools be inspected against an");
    println!("explicit target display / soft-key profile instead of the default layout.");
}

#[derive(Debug, Clone)]
struct InspectArgs {
    path: Option<std::ffi::OsString>,
    fixture_name: Option<String>,
    strict: bool,
    active_mask: Option<ObjectID>,
    expected_unsupported_records: Option<usize>,
    expected_placeholder_pixels: Option<usize>,
    expected_rgb888_fnv64: Option<u64>,
    expected_rgb565_be_fnv64: Option<u64>,
    expected_rgb565_le_fnv64: Option<u64>,
    write_report_json_path: Option<PathBuf>,
    write_rgb888_path: Option<PathBuf>,
    write_rgb565_be_path: Option<PathBuf>,
    write_rgb565_le_path: Option<PathBuf>,
    canvas: (u16, u16),
    soft_key_area: Rect,
    physical_soft_key_count: u8,
    navigation_soft_key_count: u8,
    soft_key_page: u16,
}

impl InspectArgs {
    fn parse(raw_args: &[std::ffi::OsString]) -> Result<Self, String> {
        let defaults = LayoutConfig::default();
        let mut strict = false;
        let mut active_mask = None;
        let mut expected_unsupported_records = None;
        let mut expected_placeholder_pixels = None;
        let mut expected_rgb888_fnv64 = None;
        let mut expected_rgb565_be_fnv64 = None;
        let mut expected_rgb565_le_fnv64 = None;
        let mut write_report_json_path = None;
        let mut write_rgb888_path = None;
        let mut write_rgb565_be_path = None;
        let mut write_rgb565_le_path = None;
        let mut canvas = defaults.canvas;
        let mut soft_key_area = defaults.soft_key_area;
        let mut physical_soft_key_count = defaults.physical_soft_key_count;
        let mut navigation_soft_key_count = defaults.navigation_soft_key_count;
        let mut soft_key_page = defaults.soft_key_page;
        let mut positionals = Vec::new();
        let mut iter = raw_args.iter();
        while let Some(arg) = iter.next() {
            match arg.to_str() {
                Some("--strict") => strict = true,
                Some("--active-mask") => {
                    let Some(value) = iter.next() else {
                        return Err("--active-mask requires an object id".to_string());
                    };
                    active_mask = Some(ObjectID::new(parse_u16(value, "--active-mask")?));
                }
                Some("--expect-unsupported-records") => {
                    let Some(value) = iter.next() else {
                        return Err("--expect-unsupported-records requires a count".to_string());
                    };
                    expected_unsupported_records =
                        Some(parse_usize(value, "--expect-unsupported-records")?);
                }
                Some("--expect-placeholder-pixels") => {
                    let Some(value) = iter.next() else {
                        return Err("--expect-placeholder-pixels requires a count".to_string());
                    };
                    expected_placeholder_pixels =
                        Some(parse_usize(value, "--expect-placeholder-pixels")?);
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
                Some("--write-rgb888") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-rgb888 requires an output path".to_string());
                    };
                    set_optional_path(&mut write_rgb888_path, "--write-rgb888", value)?;
                }
                Some("--write-report-json") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-report-json requires an output path".to_string());
                    };
                    set_optional_path(&mut write_report_json_path, "--write-report-json", value)?;
                }
                Some("--write-rgb565-be") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-rgb565-be requires an output path".to_string());
                    };
                    set_optional_path(&mut write_rgb565_be_path, "--write-rgb565-be", value)?;
                }
                Some("--write-rgb565-le") => {
                    let Some(value) = iter.next() else {
                        return Err("--write-rgb565-le requires an output path".to_string());
                    };
                    set_optional_path(&mut write_rgb565_le_path, "--write-rgb565-le", value)?;
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
                    physical_soft_key_count = parse_u8(value, "--physical-soft-keys")?;
                }
                Some("--navigation-soft-keys") => {
                    let Some(value) = iter.next() else {
                        return Err("--navigation-soft-keys requires a count".to_string());
                    };
                    navigation_soft_key_count = parse_u8(value, "--navigation-soft-keys")?;
                }
                Some("--soft-key-page") => {
                    let Some(value) = iter.next() else {
                        return Err("--soft-key-page requires a zero-based page".to_string());
                    };
                    soft_key_page = parse_u16(value, "--soft-key-page")?;
                }
                Some(flag) if flag.starts_with('-') => {
                    return Err(format!("unknown option `{flag}`"));
                }
                _ => positionals.push(arg.clone()),
            }
        }
        if positionals.len() > 2 {
            return Err("expected at most PATH and optional FIXTURE_NAME".to_string());
        }
        let path = positionals.first().cloned();
        let fixture_name = match positionals.get(1) {
            Some(name) => Some(
                name.to_str()
                    .ok_or("fixture name must be valid UTF-8")?
                    .to_owned(),
            ),
            None => None,
        };
        Ok(Self {
            path,
            fixture_name,
            strict,
            active_mask,
            expected_unsupported_records,
            expected_placeholder_pixels,
            expected_rgb888_fnv64,
            expected_rgb565_be_fnv64,
            expected_rgb565_le_fnv64,
            write_report_json_path,
            write_rgb888_path,
            write_rgb565_be_path,
            write_rgb565_le_path,
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

    fn writes_framebuffer_snapshot(&self) -> bool {
        self.write_rgb888_path.is_some()
            || self.write_rgb565_be_path.is_some()
            || self.write_rgb565_le_path.is_some()
    }

    fn expects_framebuffer_hash(&self) -> bool {
        self.expected_rgb888_fnv64.is_some()
            || self.expected_rgb565_be_fnv64.is_some()
            || self.expected_rgb565_le_fnv64.is_some()
    }

    fn requires_final_check(&self) -> bool {
        self.strict
            || self.expected_unsupported_records.is_some()
            || self.expected_placeholder_pixels.is_some()
            || self.expects_framebuffer_hash()
            || self.write_report_json_path.is_some()
            || self.writes_framebuffer_snapshot()
    }
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

fn parse_u8(value: &std::ffi::OsString, flag: &str) -> Result<u8, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{flag} value must be valid UTF-8"))?;
    value
        .parse::<u8>()
        .map_err(|error| format!("invalid {flag} count `{value}`: {error}"))
}

fn parse_u16(value: &std::ffi::OsString, flag: &str) -> Result<u16, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{flag} value must be valid UTF-8"))?;
    parse_u16_str(value, flag, "value")
}

fn parse_usize(value: &std::ffi::OsString, flag: &str) -> Result<usize, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{flag} value must be valid UTF-8"))?;
    value
        .parse::<usize>()
        .map_err(|error| format!("invalid {flag} count `{value}`: {error}"))
}

fn parse_u16_str(value: &str, flag: &str, field: &str) -> Result<u16, String> {
    if value.is_empty() {
        return Err(format!("{flag} missing {field}"));
    }
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return u16::from_str_radix(hex, 16)
            .map_err(|error| format!("invalid {flag} {field} `{value}`: {error}"));
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
        return Err(format!("{flag} output path may not be empty"));
    }
    *slot = Some(PathBuf::from(value));
    Ok(())
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

fn load_input_bytes(args: &InspectArgs) -> Result<(String, Vec<u8>), String> {
    let Some(path) = args.path.as_ref() else {
        return Ok(("synthetic demo pool".to_string(), demo_pool_bytes()));
    };
    let fixture_name = args.fixture_name.as_deref();
    let path = Path::new(path);
    let raw =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
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
        return Err(format!("missing named hex fixture `{name}`"));
    }
    if !plain_hex.is_empty() {
        return parse_hex_bytes(&plain_hex);
    }
    if let Some((name, hex)) = first_named {
        return parse_hex_bytes(hex).map_err(|error| format!("{name}: {error}"));
    }
    Err("hex fixture is empty".to_string())
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
        let byte = u8::from_str_radix(digits, 16)
            .map_err(|error| format!("invalid hex byte `{digits}`: {error}"))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

fn write_snapshot_file(format: &str, path: &Path, bytes: &[u8], failures: &mut Vec<String>) {
    match fs::write(path, bytes) {
        Ok(()) => println!(
            "  wrote_{format}: {} bytes -> {}",
            bytes.len(),
            path.display()
        ),
        Err(error) => failures.push(format!(
            "failed to write {format} snapshot {}: {error}",
            path.display()
        )),
    }
}

fn write_text_file(format: &str, path: &Path, text: &str, failures: &mut Vec<String>) {
    match fs::write(path, text) {
        Ok(()) => println!(
            "  wrote_{format}: {} bytes -> {}",
            text.len(),
            path.display()
        ),
        Err(error) => failures.push(format!(
            "failed to write {format} {}: {error}",
            path.display()
        )),
    }
}

#[derive(Debug, Clone)]
struct FramebufferSummary {
    width: u16,
    height: u16,
    pixels: usize,
    rgb888_bytes: usize,
    rgb565_bytes: usize,
    rgb888_fnv64: Option<u64>,
    rgb565_be_fnv64: Option<u64>,
    rgb565_le_fnv64: Option<u64>,
    placeholder_pixels: Option<usize>,
    error: Option<String>,
}

struct EvidenceReportInput<'a> {
    source: &'a str,
    pool_buffer_bytes: usize,
    pool_buffer_fnv64: u64,
    layout_config: LayoutConfig,
    pool_objects: usize,
    active_mask: u16,
    canvas_width: u16,
    canvas_height: u16,
    scene_nodes: usize,
    soft_keys: usize,
    unsupported_records: usize,
    coverage_total_objects: usize,
    coverage_total_drawable: usize,
    coverage_total_unsupported: usize,
    gtui_commands: usize,
    strict: bool,
    expected_unsupported_records: Option<usize>,
    expected_placeholder_pixels: Option<usize>,
    expected_rgb888_fnv64: Option<u64>,
    expected_rgb565_be_fnv64: Option<u64>,
    expected_rgb565_le_fnv64: Option<u64>,
    write_report_json_path: Option<&'a Path>,
    write_rgb888_path: Option<&'a Path>,
    write_rgb565_be_path: Option<&'a Path>,
    write_rgb565_le_path: Option<&'a Path>,
    framebuffer: &'a FramebufferSummary,
    failures: &'a [String],
}

fn evidence_report_json(input: EvidenceReportInput<'_>) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"schema\": \"machbus-iop-inspect-report-v1\",\n");
    out.push_str("  \"source\": ");
    push_json_string(&mut out, input.source);
    out.push_str(",\n");
    out.push_str(&format!(
        "  \"pool_buffer_bytes\": {},\n  \"pool_buffer_fnv64\": \"0x{:016X}\",\n  \"pool_objects\": {},\n  \"active_mask\": \"0x{:04X}\",\n",
        input.pool_buffer_bytes, input.pool_buffer_fnv64, input.pool_objects, input.active_mask
    ));
    out.push_str(&format!(
        "  \"canvas\": {{ \"width\": {}, \"height\": {} }},\n",
        input.canvas_width, input.canvas_height
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
    out.push_str(&format!(
        "  \"scene\": {{ \"nodes\": {}, \"soft_keys\": {}, \"unsupported_records\": {} }},\n",
        input.scene_nodes, input.soft_keys, input.unsupported_records
    ));
    out.push_str(&format!(
        "  \"coverage\": {{ \"objects\": {}, \"drawable\": {}, \"unsupported\": {} }},\n",
        input.coverage_total_objects,
        input.coverage_total_drawable,
        input.coverage_total_unsupported
    ));
    out.push_str(&format!("  \"gtui_commands\": {},\n", input.gtui_commands));
    out.push_str("  \"framebuffer\": {\n");
    out.push_str(&format!(
        "    \"rendered\": {},\n",
        input.framebuffer.error.is_none()
    ));
    out.push_str(&format!(
        "    \"width\": {},\n    \"height\": {},\n    \"pixels\": {},\n    \"rgb888_bytes\": {},\n    \"rgb565_bytes\": {},\n",
        input.framebuffer.width,
        input.framebuffer.height,
        input.framebuffer.pixels,
        input.framebuffer.rgb888_bytes,
        input.framebuffer.rgb565_bytes
    ));
    out.push_str("    \"rgb888_fnv64\": ");
    push_json_hash_or_null(&mut out, input.framebuffer.rgb888_fnv64);
    out.push_str(",\n    \"rgb565_be_fnv64\": ");
    push_json_hash_or_null(&mut out, input.framebuffer.rgb565_be_fnv64);
    out.push_str(",\n    \"rgb565_le_fnv64\": ");
    push_json_hash_or_null(&mut out, input.framebuffer.rgb565_le_fnv64);
    out.push_str(",\n    \"placeholder_pixels\": ");
    push_json_usize_or_null(&mut out, input.framebuffer.placeholder_pixels);
    out.push_str(",\n    \"error\": ");
    push_json_str_or_null(&mut out, input.framebuffer.error.as_deref());
    out.push_str("\n  },\n");
    out.push_str("  \"artifacts\": {\n");
    out.push_str("    \"report_json\": ");
    push_json_path_or_null(&mut out, input.write_report_json_path);
    out.push_str(",\n    \"rgb888\": ");
    push_json_path_or_null(&mut out, input.write_rgb888_path);
    out.push_str(",\n    \"rgb565_be\": ");
    push_json_path_or_null(&mut out, input.write_rgb565_be_path);
    out.push_str(",\n    \"rgb565_le\": ");
    push_json_path_or_null(&mut out, input.write_rgb565_le_path);
    out.push_str("\n  },\n");
    out.push_str("  \"checks\": {\n");
    out.push_str(&format!("    \"strict_requested\": {},\n", input.strict));
    out.push_str("    \"expected_unsupported_records\": ");
    push_json_usize_or_null(&mut out, input.expected_unsupported_records);
    out.push_str(",\n    \"expected_placeholder_pixels\": ");
    push_json_usize_or_null(&mut out, input.expected_placeholder_pixels);
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

fn print_command(c: &RenderCommand) {
    match c {
        RenderCommand::FillRect { rect, colour } => {
            println!("  FillRect   rect=({rect:?}) rgb={:?}", colour.to_array());
        }
        RenderCommand::StrokeRect {
            rect,
            colour,
            suppress,
            ..
        } => {
            println!(
                "  StrokeRect rect=({rect:?}) rgb={:?} suppress=0x{suppress:X}",
                colour.to_array()
            );
        }
        RenderCommand::Line {
            x0,
            y0,
            x1,
            y1,
            colour,
            ..
        } => {
            println!(
                "  Line       ({x0},{y0})->({x1},{y1}) rgb={:?}",
                colour.to_array()
            );
        }
        RenderCommand::Ellipse { rect, filled, .. } => {
            println!("  Ellipse    rect=({rect:?}) filled={filled}");
        }
        RenderCommand::EllipseArc {
            rect,
            filled,
            ellipse_type,
            start_angle,
            end_angle,
            ..
        } => {
            println!(
                "  EllipseArc rect=({rect:?}) filled={filled} type={ellipse_type} start={start_angle} end={end_angle}"
            );
        }
        RenderCommand::DrawText { rect, text, .. } => {
            println!("  DrawText   rect=({rect:?}) text={text:?}");
        }
        RenderCommand::Meter {
            rect,
            value,
            min,
            max,
            start_angle,
            end_angle,
            ..
        } => {
            println!(
                "  Meter      rect=({rect:?}) value={value} min={min} max={max} start={start_angle} end={end_angle}"
            );
        }
        RenderCommand::BarGraph {
            rect,
            value,
            target_value,
            arched,
            horizontal,
            clockwise,
            show_target_line,
            line_only,
            start_angle,
            end_angle,
            bar_width,
            ..
        } => {
            println!(
                "  BarGraph   rect=({rect:?}) value={value} target={target_value} show_target={show_target_line} line_only={line_only} arched={arched} horizontal={horizontal} clockwise={clockwise} start={start_angle} end={end_angle} width={bar_width}"
            );
        }
        RenderCommand::IndexedImage {
            rect,
            width,
            height,
            format,
            data,
            ..
        } => {
            println!(
                "  Image      rect=({rect:?}) size={width}x{height} fmt={format} bytes={}",
                data.len()
            );
        }
        RenderCommand::RgbaImage {
            rect,
            width,
            height,
            data,
            ..
        } => {
            println!(
                "  RgbaImage  rect=({rect:?}) size={width}x{height} rgba_bytes={}",
                data.len()
            );
        }
        RenderCommand::GraphicsContextViewport {
            object_id,
            viewport,
            zoom_raw,
        } => {
            println!(
                "  GCtxView   object={object_id:?} viewport=({viewport:?}) zoom_raw={zoom_raw:?}"
            );
        }
        RenderCommand::GraphicsContextCopyToPicture {
            object_id,
            picture_id,
            source,
            viewport,
            zoom_raw,
        } => {
            println!(
                "  GCtxCopy   object={object_id:?} picture={picture_id:?} source={source:?} viewport=({viewport:?}) zoom_raw={zoom_raw:?}"
            );
        }
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            source,
            width,
            height,
            format,
            transparent_index,
            data,
        } => {
            println!(
                "  GCtxPixels object={object_id:?} picture={picture_id:?} source={source:?} size={width}x{height} fmt={format} transparent={transparent_index} bytes={}",
                data.len()
            );
        }
        RenderCommand::GraphicsContextCanvas {
            object_id,
            rect,
            canvas_width,
            canvas_height,
            background,
            transparency_colour,
            transparent,
        } => {
            println!(
                "  GCtxCanvas object={object_id:?} rect=({rect:?}) canvas={canvas_width}x{canvas_height} background={background} transparency={transparency_colour} transparent={transparent}"
            );
        }
        RenderCommand::GraphicsContextReplay {
            object_id,
            subcommand,
            payload,
        } => {
            println!(
                "  GCtxReplay object={object_id:?} subcommand=0x{subcommand:02X} bytes={}",
                payload.len()
            );
        }
        RenderCommand::Placeholder {
            object_type,
            reason,
            ..
        } => {
            println!("  Placeholder type={object_type:?} reason={reason}");
        }
        RenderCommand::SoftKey {
            rect,
            kind,
            label,
            latched,
            ..
        } => {
            println!(
                "  SoftKey    rect=({rect:?}) kind={kind:?} label={label:?} latched={latched}"
            );
        }
        RenderCommand::Clip(rect) => println!("  Clip       rect=({rect:?})"),
        RenderCommand::Polygon { origin, filled, .. } => {
            println!("  Polygon    origin={origin:?} filled={filled}");
        }
        RenderCommand::PatternFillRect {
            rect,
            anchor,
            pattern,
        } => {
            println!(
                "  PatternRect rect=({rect:?}) anchor={anchor:?} pattern={:?} size={}x{} fmt={}",
                pattern.object_id, pattern.width, pattern.height, pattern.format
            );
        }
        RenderCommand::PatternFillEllipse {
            rect,
            anchor,
            pattern,
            ..
        } => {
            println!(
                "  PatternEllipse rect=({rect:?}) anchor={anchor:?} pattern={:?} size={}x{} fmt={}",
                pattern.object_id, pattern.width, pattern.height, pattern.format
            );
        }
        RenderCommand::PatternFillPolygon {
            origin,
            anchor,
            pattern,
            ..
        } => {
            println!(
                "  PatternPolygon origin={origin:?} anchor={anchor:?} pattern={:?} size={}x{} fmt={}",
                pattern.object_id, pattern.width, pattern.height, pattern.format
            );
        }
    }
}

/// Build a small synthetic object pool that exercises output string /
/// output number / input number paths so the inspector has something
/// interesting to print by default.
fn demo_pool_bytes() -> Vec<u8> {
    let font = create_font_attributes(
        10,
        &FontAttributesBody {
            font_color: 1,
            font_size: 8,
            ..Default::default()
        },
    );
    let label_var = create_string_variable(
        11,
        &StringVariableBody {
            length: 10,
            value: b"SPRAY RATE".to_vec(),
        },
    );
    let rate_var = create_number_variable(12, &NumberVariableBody { value: 138 });
    let target_var = create_number_variable(13, &NumberVariableBody { value: 150 });

    let label = create_output_string(
        4,
        &OutputStringBody {
            width: 200,
            height: 24,
            font_attributes: font.id,
            variable_reference: label_var.id,
            ..Default::default()
        },
    )
    .unwrap();
    let value = create_output_number(
        5,
        &OutputNumberBody {
            width: 80,
            height: 24,
            font_attributes: font.id,
            variable_reference: rate_var.id,
            number_of_decimals: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let target = create_input_number(
        6,
        &InputNumberBody {
            width: 80,
            height: 24,
            font_attributes: font.id,
            variable_reference: target_var.id,
            options: 0x01,
            min_value: 0,
            max_value: 1000,
            ..Default::default()
        },
    )
    .unwrap();

    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([4u16, 5u16, 6u16]),
        )
        .with_object(font)
        .with_object(label_var)
        .with_object(rate_var)
        .with_object(target_var)
        .with_object(label)
        .with_object(value)
        .with_object(target);
    pool.serialize().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_hex_fixture_ignores_comments_and_whitespace() {
        let bytes = parse_hex_fixture(
            "
            # comment
            01 02
            03_04
            ",
            None,
        )
        .unwrap();

        assert_eq!(bytes, vec![1, 2, 3, 4]);
    }

    #[test]
    fn parse_named_hex_fixture_chooses_requested_entry() {
        let bytes = parse_hex_fixture(
            "
            first=0102
            second=0A0B0C
            ",
            Some("second"),
        )
        .unwrap();

        assert_eq!(bytes, vec![0x0A, 0x0B, 0x0C]);
    }

    #[test]
    fn parse_hex_fixture_rejects_odd_or_invalid_hex() {
        assert!(parse_hex_fixture("0", None).is_err());
        assert!(parse_hex_fixture("not-hex", None).is_err());
        assert!(parse_hex_fixture("first=0102", Some("missing")).is_err());
    }

    #[test]
    fn parse_inspector_args_handles_strict_path_and_fixture_name() {
        let args = InspectArgs::parse(&[
            "--strict".into(),
            "--active-mask".into(),
            "0x07D0".into(),
            "--expect-rgb888-fnv64".into(),
            "0x0123456789ABCDEF".into(),
            "--expect-rgb565-be-fnv64".into(),
            "0x1111222233334444".into(),
            "--expect-rgb565-le-fnv64".into(),
            "0x5555666677778888".into(),
            "--expect-unsupported-records".into(),
            "0".into(),
            "--expect-placeholder-pixels".into(),
            "0".into(),
            "--write-report-json".into(),
            "report.json".into(),
            "--write-rgb888".into(),
            "snapshot.rgb".into(),
            "--write-rgb565-be".into(),
            "snapshot-be.rgb565".into(),
            "--write-rgb565-le".into(),
            "snapshot-le.rgb565".into(),
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
            "fixtures.hex".into(),
            "valid_pool".into(),
        ])
        .unwrap();

        assert!(args.strict);
        assert_eq!(args.active_mask, Some(ObjectID::new(0x07D0)));
        assert_eq!(args.expected_rgb888_fnv64, Some(0x0123_4567_89AB_CDEF));
        assert_eq!(args.expected_rgb565_be_fnv64, Some(0x1111_2222_3333_4444));
        assert_eq!(args.expected_rgb565_le_fnv64, Some(0x5555_6666_7777_8888));
        assert_eq!(args.expected_unsupported_records, Some(0));
        assert_eq!(args.expected_placeholder_pixels, Some(0));
        assert_eq!(
            args.write_report_json_path.as_deref(),
            Some(Path::new("report.json"))
        );
        assert_eq!(
            args.write_rgb888_path.as_deref(),
            Some(Path::new("snapshot.rgb"))
        );
        assert_eq!(
            args.write_rgb565_be_path.as_deref(),
            Some(Path::new("snapshot-be.rgb565"))
        );
        assert_eq!(
            args.write_rgb565_le_path.as_deref(),
            Some(Path::new("snapshot-le.rgb565"))
        );
        assert!(args.writes_framebuffer_snapshot());
        assert!(args.expects_framebuffer_hash());
        assert!(args.requires_final_check());
        assert_eq!(args.layout_config().canvas, (320, 240));
        assert_eq!(
            args.layout_config().soft_key_area,
            Rect::new(320, 0, 64, 240)
        );
        assert_eq!(args.layout_config().physical_soft_key_count, 10);
        assert_eq!(args.layout_config().navigation_soft_key_count, 2);
        assert_eq!(args.layout_config().soft_key_page, 1);
        assert_eq!(
            args.path.as_deref(),
            Some(std::ffi::OsStr::new("fixtures.hex"))
        );
        assert_eq!(args.fixture_name.as_deref(), Some("valid_pool"));
    }

    #[test]
    fn parse_inspector_args_rejects_unknown_flags_or_extra_positionals() {
        assert!(InspectArgs::parse(&["--nope".into()]).is_err());
        assert!(InspectArgs::parse(&["--active-mask".into()]).is_err());
        assert!(InspectArgs::parse(&["--active-mask".into(), "nope".into()]).is_err());
        assert!(InspectArgs::parse(&["--expect-rgb888-fnv64".into()]).is_err());
        assert!(InspectArgs::parse(&["--expect-rgb888-fnv64".into(), "not-hex".into()]).is_err());
        assert!(InspectArgs::parse(&["--expect-rgb565-be-fnv64".into()]).is_err());
        assert!(
            InspectArgs::parse(&["--expect-rgb565-le-fnv64".into(), "not-hex".into()]).is_err()
        );
        assert!(InspectArgs::parse(&["--expect-unsupported-records".into()]).is_err());
        assert!(
            InspectArgs::parse(&["--expect-placeholder-pixels".into(), "not-a-count".into()])
                .is_err()
        );
        assert!(InspectArgs::parse(&["--write-rgb888".into()]).is_err());
        assert!(InspectArgs::parse(&["--write-report-json".into()]).is_err());
        assert!(InspectArgs::parse(&["--canvas".into(), "320".into()]).is_err());
        assert!(InspectArgs::parse(&["--canvas".into(), "0x240".into()]).is_err());
        assert!(InspectArgs::parse(&["--soft-key-area".into(), "0,0,0,1".into()]).is_err());
        assert!(InspectArgs::parse(&["--physical-soft-keys".into(), "999".into()]).is_err());
        assert!(
            InspectArgs::parse(&[
                "--write-rgb565-be".into(),
                "a.rgb565".into(),
                "--write-rgb565-be".into(),
                "b.rgb565".into()
            ])
            .is_err()
        );
        assert!(InspectArgs::parse(&["a".into(), "b".into(), "c".into()]).is_err());
    }

    #[test]
    fn fnv1a64_is_stable_for_framebuffer_evidence() {
        assert_eq!(fnv1a64(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(fnv1a64(b"machbus"), 0xFB16_22CB_75FD_FEEE);
        assert_eq!(
            parse_u64_hash("0xfb1622cb75fdfeee").unwrap(),
            fnv1a64(b"machbus")
        );
    }

    #[test]
    fn evidence_report_json_escapes_strings_and_keeps_stable_fields() {
        let failures = vec!["bad \"quote\"\nline".to_string()];
        let framebuffer = FramebufferSummary {
            width: 2,
            height: 1,
            pixels: 2,
            rgb888_bytes: 6,
            rgb565_bytes: 4,
            rgb888_fnv64: Some(0x0123_4567_89AB_CDEF),
            rgb565_be_fnv64: Some(0x1111_2222_3333_4444),
            rgb565_le_fnv64: Some(0x5555_6666_7777_8888),
            placeholder_pixels: Some(0),
            error: None,
        };
        let json = evidence_report_json(EvidenceReportInput {
            source: "fixtures/pool.iop",
            pool_buffer_bytes: 42,
            pool_buffer_fnv64: 0xCAFE_BABE_CAFE_BABE,
            layout_config: LayoutConfig {
                canvas: (320, 240),
                soft_key_area: Rect::new(320, 0, 64, 240),
                physical_soft_key_count: 10,
                navigation_soft_key_count: 2,
                soft_key_page: 1,
                auto_layout_gap: 4,
            },
            pool_objects: 3,
            active_mask: 2,
            canvas_width: 480,
            canvas_height: 240,
            scene_nodes: 1,
            soft_keys: 0,
            unsupported_records: 0,
            coverage_total_objects: 3,
            coverage_total_drawable: 1,
            coverage_total_unsupported: 0,
            gtui_commands: 4,
            strict: true,
            expected_unsupported_records: Some(0),
            expected_placeholder_pixels: Some(0),
            expected_rgb888_fnv64: Some(0x0123_4567_89AB_CDEF),
            expected_rgb565_be_fnv64: Some(0x1111_2222_3333_4444),
            expected_rgb565_le_fnv64: Some(0x5555_6666_7777_8888),
            write_report_json_path: Some(Path::new("report.json")),
            write_rgb888_path: Some(Path::new("snapshot.rgb")),
            write_rgb565_be_path: Some(Path::new("snapshot-be.rgb565")),
            write_rgb565_le_path: Some(Path::new("snapshot-le.rgb565")),
            framebuffer: &framebuffer,
            failures: &failures,
        });

        assert!(json.contains("\"schema\": \"machbus-iop-inspect-report-v1\""));
        assert!(json.contains("\"pool_buffer_fnv64\": \"0xCAFEBABECAFEBABE\""));
        assert!(json.contains("\"physical_soft_key_count\": 10"));
        assert!(json.contains("\"soft_key_page\": 1"));
        assert!(json.contains("\"active_mask\": \"0x0002\""));
        assert!(json.contains("\"expected_unsupported_records\": 0"));
        assert!(json.contains("\"expected_placeholder_pixels\": 0"));
        assert!(json.contains("\"report_json\": \"report.json\""));
        assert!(json.contains("\"rgb888\": \"snapshot.rgb\""));
        assert!(json.contains("\"rgb565_be\": \"snapshot-be.rgb565\""));
        assert!(json.contains("\"rgb565_le\": \"snapshot-le.rgb565\""));
        assert!(json.contains("\"rgb888_fnv64\": \"0x0123456789ABCDEF\""));
        assert!(json.contains("\"rgb565_be_fnv64\": \"0x1111222233334444\""));
        assert!(json.contains("\"rgb565_le_fnv64\": \"0x5555666677778888\""));
        assert!(json.contains("bad \\\"quote\\\"\\nline"));
    }
}
