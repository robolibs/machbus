//! `machbus term` — ISOBUS Virtual Terminal (file / server / client).

use crate::cli::TermSub;
use crate::term;

/// Dispatch a `machbus term <sub>` subcommand.
pub fn run(sub: TermSub) -> Result<(), String> {
    match sub {
        TermSub::File(args) => term::run_file(args),
        TermSub::Server(args) => term::run_server(args),
        TermSub::Client(args) => term::run_client(args),
    }
}
