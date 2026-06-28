//! `machbus` — SocketCAN command-line tools built on the machbus stack.
//!
//! Provides `dump`, `send`, and `gen` subcommands analogous to the
//! upstream `can-utils` (`candump`, `cansend`, `cangen`), with optional
//! ISOBUS/J1939 PGN/source/destination decoding supplied by the `machbus`
//! library.

mod can;
mod cli;
mod cmd;
mod rng;
mod signal;
mod socket;
mod tui;

use clap::Parser;
use cli::{Cli, Command};

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Dump(args) => cmd::run("dump", || cmd::dump::run(args)),
        Command::Send(args) => cmd::run("send", || cmd::send::run(args)),
        Command::Generate(args) => cmd::run("gen", || cmd::generate::run(args)),
        Command::Live(args) => cmd::run("live", || cmd::live::run(args)),
    }
}
