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
