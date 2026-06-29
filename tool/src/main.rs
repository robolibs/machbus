mod bus;
mod can;
mod cli;
mod cmd;
mod drive;
mod rng;
mod signal;
mod socket;
mod term;
mod tui;

use clap::Parser;
use cli::{Cli, Command, TermSub};

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let label = command_label(&cli.command);
    let result = match cli.command {
        Command::Dump(args) => cmd::dump::run(args),
        Command::Send(args) => cmd::send::run(args),
        Command::Generate(args) => cmd::generate::run(args),
        Command::Live(args) => cmd::live::run(args),
        Command::Term { command } => cmd::term::run(command),
        Command::Drive(args) => drive::run(args),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("machbus {label}: {msg}");
            std::process::ExitCode::from(1)
        }
    }
}

fn command_label(command: &Command) -> &'static str {
    match command {
        Command::Dump(_) => "dump",
        Command::Send(_) => "send",
        Command::Generate(_) => "gen",
        Command::Live(_) => "live",
        Command::Term { command } => match command {
            TermSub::File(_) => "term file",
            TermSub::Server(_) => "term server",
            TermSub::Client(_) => "term client",
        },
        Command::Drive(_) => "drive",
    }
}
