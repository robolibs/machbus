//! Command-line argument definitions (clap derive).

use clap::{Args, Parser, Subcommand};

/// `machbus` — SocketCAN command-line tools built on the machbus stack.
///
/// Provides candump-style frame capture, single-frame sending, and
/// synthetic traffic generation, with optional ISOBUS/J1939 decoding.
#[derive(Parser, Debug)]
#[command(name = "machbus", version, propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Display, filter and log CAN traffic (like `candump`).
    Dump(DumpArgs),
    /// Send a single CAN frame (like `cansend`).
    Send(SendArgs),
    /// Generate (random) CAN traffic (like `cangen`).
    #[command(name = "gen")]
    Generate(GenArgs),
    /// Interactive live CAN monitor ("mechdump live") with a ratatui UI.
    Live(LiveArgs),
    /// ISOBUS Virtual Terminal (render a pool, or run a live VT server/client).
    Term {
        #[command(subcommand)]
        command: TermSub,
    },
    /// ISOBUS drive: WASD guidance + telemetry over CAN.
    Drive(DriveArgs),
}

// ── drive ───────────────────────────────────────────────────────────────
#[derive(Args, Debug)]
pub struct DriveArgs {
    /// SocketCAN interface.
    #[arg(short = 'i', long = "iface", default_value = "vcan0")]
    pub iface: String,
    /// Preferred ECU source address (hex).
    #[arg(long = "addr", default_value = "80")]
    pub addr: String,
    /// Default forward speed limit in m/s (W accelerates toward this). Start
    /// at 0 and use I/K to raise it.
    #[arg(long = "default-speed", default_value = "0")]
    pub default_speed: f64,
    /// Speed step per I/K keypress in m/s.
    #[arg(long = "speed-step", default_value = "0.5")]
    pub speed_step: f64,
    /// Maximum curvature in 1/km for full A/D deflection.
    #[arg(long = "max-curvature", default_value = "40")]
    pub max_curvature: f64,
}

// ── dump ────────────────────────────────────────────────────────────────
#[derive(Args, Debug)]
pub struct DumpArgs {
    /// CAN interface to listen on (e.g. `can0`). Use `any` to receive from
    /// every interface. Ignored when `--from-file` is given.
    #[arg(default_value = "can0")]
    pub interface: String,

    /// Replay a `candump` capture file instead of opening a live socket.
    /// Accepts both compact (`ID#DATA`) and bracketed forms.
    #[arg(short = 'f', long = "from-file", value_name = "FILE")]
    pub from_file: Option<String>,

    /// Timestamp mode for printed lines.
    ///   `none`  — no timestamp (default)
    ///   `abs`   — wall-clock seconds.microseconds since epoch
    ///   `delta` — seconds since the first frame
    ///   `rel`   — seconds since the previous frame
    #[arg(short = 't', long = "time", default_value = "none")]
    pub time: TimestampMode,

    /// Also append an ISOBUS/J1939 PGN/source/destination annotation.
    #[arg(short = 'd', long = "decode")]
    pub decode: bool,

    /// Stop after receiving `count` frames.
    #[arg(short = 'n', long = "count", value_name = "N")]
    pub count: Option<u64>,

    /// Write a compact `candump -L` log of every frame to `FILE` in
    /// addition to printing to stdout.
    #[arg(short = 'L', long = "logfile", value_name = "FILE")]
    pub logfile: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum TimestampMode {
    None,
    Abs,
    Delta,
    Rel,
}

// ── send ────────────────────────────────────────────────────────────────
#[derive(Args, Debug)]
pub struct SendArgs {
    /// CAN interface to send on (e.g. `can0`).
    pub interface: String,

    /// Compact CAN frame: `<ID>#<DATA>`.
    ///
    /// 3 hex digits = standard 11-bit ID, 8 hex digits = extended 29-bit.
    /// Append `#R` (or `#R<n>`) for a remote frame.
    /// Examples: `123#DEADBEEF`, `18FEE680#A43116081C267D78`, `7A1#R`.
    pub frame: String,

    /// Print the decoded ISOBUS/J1939 fields of the sent frame.
    #[arg(short = 'd', long = "decode")]
    pub decode: bool,
}

// ── gen ─────────────────────────────────────────────────────────────────
#[derive(Args, Debug)]
pub struct GenArgs {
    /// CAN interface to generate traffic on (e.g. `can0`).
    pub interface: String,

    /// Number of frames to send. Without this, runs until Ctrl-C.
    #[arg(short = 'n', long = "count", value_name = "N")]
    pub count: Option<u64>,

    /// Fixed CAN ID (hex) instead of random.
    #[arg(short = 'I', long = "id", value_name = "ID")]
    pub id: Option<String>,

    /// Generate extended (29-bit) frames. By default standard 11-bit IDs
    /// are used (unless `--id` is given with 8 hex digits).
    #[arg(short = 'e', long = "extended")]
    pub extended: bool,

    /// Fixed data length 0–8 (default 8, or `--random-dlc`).
    #[arg(short = 'L', long = "dlc", value_name = "LEN")]
    pub dlc: Option<u8>,

    /// Randomize the data length each frame (0–8).
    #[arg(long = "random-dlc")]
    pub random_dlc: bool,

    /// Fixed payload (hex) instead of random data.
    #[arg(short = 'D', long = "data", value_name = "DATA")]
    pub data: Option<String>,

    /// Increment the CAN ID (and fixed payload bytes) each frame.
    #[arg(short = 'i', long = "increment")]
    pub increment: bool,

    /// Gap between frames in milliseconds (default 1).
    #[arg(short = 'g', long = "gap", value_name = "MS", default_value = "1")]
    pub gap: u64,

    /// Decode and print each sent frame.
    #[arg(short = 'd', long = "decode")]
    pub decode: bool,

    /// Run quietly (do not print each frame).
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

// ── live ────────────────────────────────────────────────────────────────
#[derive(Args, Debug)]
pub struct LiveArgs {
    /// CAN interface to monitor (e.g. `can0`). Use `any` to receive from
    /// every interface. Ignored when `--from-file` is given.
    #[arg(default_value = "can0")]
    pub interface: String,

    /// Replay a `candump` capture file (with timing) instead of a live
    /// socket. Useful for demos without hardware.
    #[arg(short = 'f', long = "from-file", value_name = "FILE")]
    pub from_file: Option<String>,

    /// Replay speed multiplier for `--from-file` (1.0 = real time).
    #[arg(short = 's', long = "speed", default_value = "1", value_name = "MULT")]
    pub speed: f64,

    /// Ring-buffer capacity: number of frames kept in memory.
    #[arg(
        short = 'b',
        long = "buffer",
        default_value = "10000",
        value_name = "N"
    )]
    pub buffer: usize,

    /// Initial tab: live | sniffer | pgn | nmea | nodes | stats | filter | help.
    #[arg(short = 'T', long = "tab", default_value = "live")]
    pub tab: String,

    /// Log every received frame to this `candump -L` file in the background.
    #[arg(short = 'L', long = "logfile", value_name = "FILE")]
    pub logfile: Option<String>,
}

// ── term (Virtual Terminal) ─────────────────────────────────────────────
//
// `machbus term` is a group with three subcommands:
//   machbus term file   <pool.iop>      — render offline
//   machbus term server --iface vcan0   — live VT server
//   machbus term client <pool.iop> --iface vcan0 — upload a pool (test)

#[derive(Subcommand, Debug)]
pub enum TermSub {
    /// Render an object pool from a `.iop` file (offline).
    File(TermFileArgs),
    /// Live VT server: receive a pool over CAN and render it.
    Server(TermServerArgs),
    /// Live VT client: upload a pool to a server (the test counterpart).
    Client(TermClientArgs),
}

#[derive(Args, Debug)]
pub struct TermFileArgs {
    /// Path to the ISOBUS object pool (`.iop`) file to render.
    pub iop: String,
    /// Initial mask object ID (hex), e.g. `1F`.
    #[arg(short = 'm', long = "mask", value_name = "ID")]
    pub mask: Option<String>,
    /// VT canvas size in pixels, e.g. `480x240`.
    #[arg(long = "canvas", value_name = "WxH")]
    pub canvas: Option<String>,
    /// Physical soft-key count (0 = legacy unlimited).
    #[arg(long = "physical-soft-keys", value_name = "N")]
    pub physical_soft_keys: Option<u8>,
    /// Navigation soft-key count.
    #[arg(long = "navigation-soft-keys", value_name = "N")]
    pub navigation_soft_keys: Option<u8>,
}

#[derive(Args, Debug)]
pub struct TermServerArgs {
    /// SocketCAN interface.
    #[arg(short = 'i', long = "iface", default_value = "vcan0")]
    pub iface: String,
    /// Preferred VT source address (hex).
    #[arg(long = "addr", default_value = "26")]
    pub addr: String,
}

#[derive(Args, Debug)]
pub struct TermClientArgs {
    /// Path to the ISOBUS object pool (`.iop`) file to upload. Omit with
    /// `--demo` to upload a built-in small pool.
    pub iop: Option<String>,
    /// SocketCAN interface.
    #[arg(short = 'i', long = "iface", default_value = "vcan0")]
    pub iface: String,
    /// Preferred client (ECU) source address (hex).
    #[arg(long = "addr", default_value = "80")]
    pub addr: String,
    /// Upload a built-in small demo pool (fast connect; good for testing).
    #[arg(long = "demo")]
    pub demo: bool,
    /// VT protocol version to request (2, 3, 4, 5, 6). Default 4. Try a
    /// lower version if the real VT rejects the connection.
    #[arg(long = "vt-version", default_value = "4")]
    pub vt_version: u8,
}
