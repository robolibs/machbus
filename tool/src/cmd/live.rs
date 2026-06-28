//! `machbus live` — launches the interactive ratatui TUI.

use crate::cli::LiveArgs;
use crate::tui;

/// Entry point for `machbus live`.
pub fn run(args: LiveArgs) -> Result<(), String> {
    tui::run(args)
}
