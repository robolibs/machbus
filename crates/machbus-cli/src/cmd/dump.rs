//! `machbus dump` — display/filter/log CAN traffic.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::can::{RawFrame, decode_isobus, format_compact, format_readable, parse_candump_line};
use crate::cli::{DumpArgs, TimestampMode};
use crate::signal;
use crate::socket;

/// Entry point for `machbus dump`.
pub fn run(args: DumpArgs) -> Result<(), String> {
    signal::install_cancel_handler();

    let mut log = open_logfile(args.logfile.as_deref())?;
    let mut stats = Stats::default();

    if let Some(path) = args.from_file.as_deref() {
        replay_file(path, &args, &mut log, &mut stats)?;
    } else {
        capture_live(&args, &mut log, &mut stats)?;
    }

    eprintln!("{}", stats.summary());
    Ok(())
}

fn open_logfile(path: Option<&str>) -> Result<Option<BufWriter<File>>, String> {
    path.map(|p| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(Path::new(p))
            .map(BufWriter::new)
            .map_err(|e| format!("cannot open logfile '{p}': {e}"))
    })
    .transpose()
}

fn replay_file(
    path: &str,
    args: &DumpArgs,
    log: &mut Option<BufWriter<File>>,
    stats: &mut Stats,
) -> Result<(), String> {
    let text = std::fs::read_to_string(Path::new(path))
        .map_err(|e| format!("cannot read capture '{path}': {e}"))?;

    let mut clock = Clock::new(args.time);
    for line in text.lines() {
        if signal::cancel_requested() {
            break;
        }
        let Some(parsed) = parse_candump_line(line) else {
            continue;
        };
        let raw = parsed.to_raw();
        clock.advance(parsed.timestamp_us);
        emit(
            &raw,
            parsed.interface.as_deref().unwrap_or(&args.interface),
            args,
            &mut clock,
            log,
            stats,
        );
        if let Some(limit) = args.count
            && stats.frames >= limit
        {
            break;
        }
    }
    flush_log(log);
    Ok(())
}

fn capture_live(
    args: &DumpArgs,
    log: &mut Option<BufWriter<File>>,
    stats: &mut Stats,
) -> Result<(), String> {
    let sock = socket::open(&args.interface).map_err(|e| e.to_string())?;
    let mut clock = Clock::new(args.time);
    let poll = Duration::from_millis(200);

    loop {
        if signal::cancel_requested() {
            break;
        }
        match sock.recv(poll).map_err(|e| e.to_string())? {
            Some((raw, iface)) => {
                let now_us = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_micros() as u64)
                    .unwrap_or(0);
                clock.advance(Some(now_us));
                emit(&raw, &iface, args, &mut clock, log, stats);
                if let Some(limit) = args.count
                    && stats.frames >= limit
                {
                    break;
                }
            }
            None => continue,
        }
    }
    flush_log(log);
    Ok(())
}

fn emit(
    frame: &RawFrame,
    iface: &str,
    args: &DumpArgs,
    clock: &mut Clock,
    log: &mut Option<BufWriter<File>>,
    stats: &mut Stats,
) {
    stats.observe(frame);

    // Compact logfile always carries a timestamp.
    if let Some(writer) = log {
        let ts = clock.log_timestamp();
        let _ = writeln!(writer, "{}", format_compact(frame, iface, ts));
    }

    let mut line = format_readable(frame, iface);
    if let Some(ts) = clock.print_timestamp() {
        line = format!("{ts} {line}");
    }
    if args.decode {
        if let Some(info) = decode_isobus(frame) {
            line.push_str(&info.annotate());
        } else {
            line.push_str("  | (not an ISOBUS/J1939 ID)");
        }
    }
    println!("{line}");
}

fn flush_log(log: &mut Option<BufWriter<File>>) {
    if let Some(w) = log {
        let _ = w.flush();
    }
}

// ── clock ───────────────────────────────────────────────────────────────
struct Clock {
    mode: TimestampMode,
    start: Instant,
    first_us: Option<u64>,
    prev_us: Option<u64>,
}

impl Clock {
    fn new(mode: TimestampMode) -> Self {
        Self {
            mode,
            start: Instant::now(),
            first_us: None,
            prev_us: None,
        }
    }

    /// Advance internal time to `ts` (the frame's own timestamp if present,
    /// otherwise the wall clock captured by the caller).
    fn advance(&mut self, ts: Option<u64>) {
        let us = ts.unwrap_or_else(|| {
            // For live capture the caller passes real wall-clock us.
            self.start.elapsed().as_micros() as u64
        });
        self.first_us.get_or_insert(us);
        self.prev_us = Some(us);
    }

    /// Timestamp string to prepend to the printed line (None in `none` mode).
    fn print_timestamp(&self) -> Option<String> {
        let us = self.prev_us?;
        match self.mode {
            TimestampMode::None => None,
            TimestampMode::Abs => Some(format!("({}.{:06})", us / 1_000_000, us % 1_000_000)),
            TimestampMode::Delta | TimestampMode::Rel => {
                let base = self.first_us?;
                let diff = us.saturating_sub(base);
                Some(format!("({}.{:06})", diff / 1_000_000, diff % 1_000_000))
            }
        }
    }

    /// Timestamp to embed in the compact logfile (always present there).
    fn log_timestamp(&self) -> Option<u64> {
        match self.mode {
            TimestampMode::None => self.prev_us,
            _ => self.prev_us,
        }
    }
}

// ── stats ───────────────────────────────────────────────────────────────
#[derive(Default)]
struct Stats {
    frames: u64,
    bytes: u64,
    ext: u64,
    std: u64,
}

impl Stats {
    fn observe(&mut self, frame: &RawFrame) {
        self.frames += 1;
        self.bytes += u64::from(frame.can_dlc);
        if frame.is_extended() {
            self.ext += 1;
        } else {
            self.std += 1;
        }
    }

    fn summary(&self) -> String {
        format!(
            "captured {} frames ({} extended, {} standard, {} data bytes)",
            self.frames, self.ext, self.std, self.bytes
        )
    }
}
