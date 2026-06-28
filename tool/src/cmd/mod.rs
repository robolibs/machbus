//! Subcommand implementations.

pub mod dump;
pub mod generate;
pub mod live;
pub mod send;

use std::process::ExitCode;

/// Run a command, mapping any error into a non-zero exit with a message.
pub fn run<F>(label: &str, f: F) -> ExitCode
where
    F: FnOnce() -> Result<(), String>,
{
    match f() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("machbus {label}: {msg}");
            ExitCode::from(1)
        }
    }
}
