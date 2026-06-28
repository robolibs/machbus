//! `machbus live` — interactive ratatui TUI ("mechdump live").
//!
//! Architecture:
//! - A background [`capture::CaptureWorker`] thread drains the SocketCAN
//!   socket (or replays a file) and ships decoded [`FrameEntry`]s through
//!   an `mpsc` channel.
//! - The UI loop ([`run`]) draws at ~30 fps, drains the channel, and
//!   dispatches keyboard events to the [`App`].
//! - [`view::render`] splits the screen into a title bar, a tab bar, the
//!   active tab's body, and a status/help footer.

pub mod capture;
pub mod decode;
pub mod filter;
pub mod model;
pub mod view;

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::can::{RawFrame, format_compact};
use crate::cli::LiveArgs;

use capture::{CaptureMsg, CaptureWorker};
use filter::{Filter, FilterField};
use model::{FrameEntry, Stats, Tab};

/// Run the live TUI to completion (until the user quits).
pub fn run(args: LiveArgs) -> Result<(), String> {
    let mut terminal = setup_terminal()?;
    let result = (|| {
        let mut app = App::new(args)?;
        let r = app_loop(&mut terminal, &mut app);
        // Ensure the worker is stopped before we tear down the terminal.
        app.worker.stop();
        r
    })();
    restore_terminal(&mut terminal);
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, String> {
    enable_raw_mode().map_err(|e| format!("enable_raw_mode: {e}"))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| format!("enter alternate screen: {e}"))?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).map_err(|e| format!("terminal init: {e}"))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen,);
    let _ = terminal.show_cursor();
}

fn app_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), String> {
    let tick = Duration::from_millis(33); // ~30 fps
    loop {
        app.drain_messages();
        app.tick();
        terminal
            .draw(|f| view::render(f, app))
            .map_err(|e| format!("draw: {e}"))?;

        if event::poll(tick).map_err(|e| format!("poll: {e}"))? {
            loop {
                match event::read() {
                    Ok(ev) => match ev {
                        Event::Key(k) if k.kind == KeyEventKind::Press => app.handle_key(k),
                        Event::Resize(_, _) => {}
                        _ => {}
                    },
                    Err(e) => return Err(format!("read event: {e}")),
                }
                if !event::poll(Duration::ZERO).unwrap_or(false) {
                    break;
                }
            }
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

/// The full live-monitor application state.
pub struct App {
    pub frames: VecDeque<FrameEntry>,
    pub buffer_cap: usize,
    pub filter: Filter,
    pub tab: Tab,
    pub paused: bool,
    pub follow_tail: bool,
    pub detail_expanded: bool,
    pub stats: Stats,
    rx: mpsc::Receiver<CaptureMsg>,
    worker: CaptureWorker,
    log_file: Option<BufWriter<File>>,
    /// Per-tab list selection (index into the tab's visible rows).
    pub selected: usize,
    /// Editable input buffer + cursor (active when `editing` is `Some`).
    pub input: String,
    pub input_cursor: usize,
    pub editing: Option<FilterField>,
    pub filter_field: FilterField,
    /// Transient status line message.
    pub message: Option<(String, Instant)>,
    pub start: Instant,
    pub should_quit: bool,
    pub eof: bool,
    /// Render-computed row keys so key handlers can index them.
    pub pgn_rows: Vec<u32>,
    pub nmea_rows: Vec<u32>,
    pub last_render: Instant,
    /// Rolling frame-rate samples for the Stats sparkline.
    pub rate_history: std::collections::VecDeque<u64>,
    /// Frames ingested since the last tick (per-tick activity).
    pub frames_this_tick: u64,
    /// Sniffer diff-grid state.
    pub sniff: model::SniffTable,
    /// J1939 address-claim table.
    pub nodes: model::NodeTable,
}

impl App {
    pub fn new(args: LiveArgs) -> Result<Self, String> {
        let tab = Tab::from_name(&args.tab).unwrap_or(Tab::Live);
        let (worker, rx) = if let Some(path) = &args.from_file {
            CaptureWorker::start_file(path, args.speed)
        } else {
            CaptureWorker::start_socket(&args.interface)
        };
        let log_file = match args.logfile.as_deref() {
            Some(p) => Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                    .map(BufWriter::new)
                    .map_err(|e| format!("logfile '{p}': {e}"))?,
            ),
            None => None,
        };
        let buffer = args.buffer.clamp(64, 1_000_000);
        Ok(Self {
            frames: VecDeque::with_capacity(buffer),
            buffer_cap: buffer,
            filter: Filter::default(),
            tab,
            paused: false,
            follow_tail: true,
            detail_expanded: true,
            stats: Stats::new(),
            rx,
            worker,
            log_file,
            selected: 0,
            input: String::new(),
            input_cursor: 0,
            editing: None,
            filter_field: FilterField::Interface,
            message: None,
            start: Instant::now(),
            should_quit: false,
            eof: false,
            pgn_rows: Vec::new(),
            nmea_rows: Vec::new(),
            last_render: Instant::now(),
            rate_history: std::collections::VecDeque::with_capacity(120),
            frames_this_tick: 0,
            sniff: model::SniffTable::new(),
            nodes: model::NodeTable::new(),
        })
    }

    /// Ingest everything the worker has buffered since the last tick.
    pub fn drain_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                CaptureMsg::Frame(entry) => {
                    if self.paused {
                        continue; // frozen: drop new frames
                    }
                    let now = Instant::now();
                    self.sniff
                        .observe(entry.raw_id, entry.extended, &entry.data, entry.dlc, now);
                    self.nodes.observe(
                        entry.decoded.pgn.unwrap_or(0),
                        entry.decoded.source.unwrap_or(0),
                        &entry.data,
                        now,
                    );
                    self.log_entry(&entry);
                    self.stats.observe(&entry);
                    self.push_frame(entry);
                    self.frames_this_tick += 1;
                }
                CaptureMsg::Info(m) => self.flash(m),
                CaptureMsg::Error(m) => self.flash(format!("error: {m}")),
                CaptureMsg::Eof => {
                    self.eof = true;
                    self.flash("replay finished".to_string());
                }
            }
        }
        if let Some(w) = &mut self.log_file {
            let _ = w.flush();
        }
    }

    /// Per-tick housekeeping: expire the status message, sample rate history.
    pub fn tick(&mut self) {
        if let Some((_, at)) = self.message
            && at.elapsed() > Duration::from_millis(2800)
        {
            self.message = None;
        }
        // Sample per-tick activity (and a smoothed fps) for the sparkline.
        let sample = self.frames_this_tick.max(self.stats.fps() as u64);
        self.rate_history.push_back(sample);
        if self.rate_history.len() > 120 {
            self.rate_history.pop_front();
        }
        self.frames_this_tick = 0;
        self.last_render = Instant::now();
    }

    fn push_frame(&mut self, e: FrameEntry) {
        self.frames.push_back(e);
        while self.frames.len() > self.buffer_cap {
            self.frames.pop_front();
        }
    }

    fn log_entry(&mut self, e: &FrameEntry) {
        let Some(w) = &mut self.log_file else { return };
        let raw = if e.extended {
            RawFrame::make_ext(e.raw_id, &e.data[..(e.dlc as usize).min(8)])
        } else {
            RawFrame::make_std(e.raw_id, &e.data[..(e.dlc as usize).min(8)])
        };
        let abs_us = self
            .start
            .elapsed()
            .as_micros()
            .saturating_add(e.rel_ms as u128 * 1000) as u64;
        let _ = writeln!(w, "{}", format_compact(&raw, &e.iface, Some(abs_us)));
    }

    fn flash(&mut self, msg: String) {
        self.message = Some((msg, Instant::now()));
    }

    fn clear(&mut self) {
        self.frames.clear();
        self.stats.clear();
        self.sniff.rows.clear();
        self.nodes.rows.clear();
        self.selected = 0;
        self.follow_tail = true;
        self.flash("cleared".into());
    }

    fn switch_tab(&mut self, tab: Tab) {
        if self.tab != tab {
            self.tab = tab;
            self.selected = 0;
            self.follow_tail = tab == Tab::Live;
        }
    }

    fn start_edit(&mut self, field: FilterField) {
        self.editing = Some(field);
        self.input = self.filter.render(field);
        self.input_cursor = self.input.len();
    }

    // ── key handling ────────────────────────────────────────────────────
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C is a hard escape hatch from any state, including text edit
        // mode (where a plain 'q' should still type into the field).
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        if self.editing.is_some() {
            self.handle_edit_key(key);
            return;
        }

        let code = key.code;
        let mods = key.modifiers;

        // 'q' quits when not editing.
        if code == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }

        // A few actions work on every tab.
        match code {
            KeyCode::Char('p') | KeyCode::Char(' ') => {
                self.paused = !self.paused;
                self.flash(if self.paused {
                    "paused".into()
                } else {
                    "resumed".into()
                });
                return;
            }
            KeyCode::Char('c') if mods.is_empty() => {
                self.clear();
                return;
            }
            KeyCode::Char('?') => {
                self.switch_tab(Tab::Help);
                return;
            }
            _ => {}
        }

        // Tab navigation works on EVERY tab so you can never get trapped.
        match code {
            KeyCode::Tab => {
                self.switch_tab(self.tab.next());
                return;
            }
            KeyCode::BackTab => {
                self.switch_tab(self.tab.prev());
                return;
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(n) = c.to_digit(10)
                    && let Some(&t) = Tab::ALL.get((n as usize).saturating_sub(1))
                {
                    self.switch_tab(t);
                    return;
                }
            }
            _ => {}
        }

        // The Filter tab owns its own field navigation + kind toggles.
        if self.tab == Tab::Filter {
            self.handle_filter_tab_key(code, mods);
            return;
        }

        match code {
            KeyCode::Char('f') => self.switch_tab(Tab::Filter),
            KeyCode::Char('/') => {
                self.switch_tab(Tab::Filter);
                self.start_edit(FilterField::Text);
            }
            _ => match self.tab {
                Tab::Live => self.handle_live_key(code, mods),
                Tab::Pgn | Tab::Nmea | Tab::Nodes => self.handle_list_key(code),
                _ => {}
            },
        }
    }

    fn handle_live_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        let _ = mods;
        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.follow_tail = false;
                self.selected = self.selected.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.follow_tail = false;
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.follow_tail = false;
                self.selected = self.selected.saturating_add(10);
            }
            KeyCode::PageUp => {
                self.follow_tail = false;
                self.selected = self.selected.saturating_sub(10);
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.follow_tail = false;
                self.selected = 0;
            }
            KeyCode::Char('G') | KeyCode::End => self.follow_tail = true,
            KeyCode::Char('d') | KeyCode::Enter => self.detail_expanded = !self.detail_expanded,
            _ => {}
        }
    }

    fn handle_list_key(&mut self, code: KeyCode) {
        // Row count for the current tab (for End/G clamping). PGN/NMEA also
        // support Enter to filter Live by the selected PGN.
        let len = match self.tab {
            Tab::Pgn => self.pgn_rows.len(),
            Tab::Nmea => self.nmea_rows.len(),
            Tab::Nodes => self.nodes.rows.len(),
            _ => return,
        };
        match code {
            KeyCode::Down | KeyCode::Char('j') => self.selected = self.selected.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::PageDown => self.selected = self.selected.saturating_add(10),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(10),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.selected = len.saturating_sub(1),
            KeyCode::Enter if matches!(self.tab, Tab::Pgn | Tab::Nmea) => {
                let rows = if self.tab == Tab::Pgn {
                    &self.pgn_rows
                } else {
                    &self.nmea_rows
                };
                if let Some(&pgn) = rows.get(self.selected) {
                    self.filter.pgn = Some(pgn);
                    self.flash(format!("filter PGN={pgn:05X}"));
                    self.switch_tab(Tab::Live);
                    self.follow_tail = true;
                }
            }
            _ => {}
        }
    }

    fn handle_filter_tab_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        let _ = mods;
        use crate::tui::decode::FrameKind;
        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.filter_field = self.filter_field.next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.filter_field = self.filter_field.prev();
            }
            KeyCode::Enter => self.start_edit(self.filter_field),
            KeyCode::Char('x') => {
                let _ = self.filter.set(self.filter_field, "");
                self.flash(format!("cleared {}", self.filter_field.label()));
            }
            KeyCode::Char('e') => {
                self.filter.extended_only = !self.filter.extended_only;
                self.flash(format!("extended-only: {}", self.filter.extended_only));
            }
            KeyCode::Char('s') => {
                self.filter.std_only = !self.filter.std_only;
                self.flash(format!("standard-only: {}", self.filter.std_only));
            }
            KeyCode::Char('J') => {
                self.filter.kind = Some(FrameKind::J1939);
                self.flash("kind: J1939/ISOBUS".into());
            }
            KeyCode::Char('N') => {
                self.filter.kind = Some(FrameKind::Nmea2000);
                self.flash("kind: NMEA 2000".into());
            }
            KeyCode::Char('A') => {
                self.filter.kind = None;
                self.filter.extended_only = false;
                self.filter.std_only = false;
                self.flash("kind/ext filters cleared".into());
            }
            KeyCode::Char('C') => {
                self.filter.clear();
                self.flash("filter cleared".into());
            }
            _ => {}
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.editing = None;
                self.input.clear();
                self.input_cursor = 0;
            }
            KeyCode::Enter => {
                if let Some(field) = self.editing {
                    match self.filter.set(field, &self.input) {
                        Ok(()) => self.flash(format!("{} set", field.label())),
                        Err(e) => self.flash(format!("invalid: {e}")),
                    }
                }
                self.editing = None;
                self.input.clear();
                self.input_cursor = 0;
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input.remove(self.input_cursor);
                }
            }
            KeyCode::Left => self.input_cursor = self.input_cursor.saturating_sub(1),
            KeyCode::Right => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor += 1;
                }
            }
            KeyCode::Home => self.input_cursor = 0,
            KeyCode::End => self.input_cursor = self.input.len(),
            KeyCode::Char(c) => {
                self.input.insert(self.input_cursor, c);
                self.input_cursor += 1;
            }
            _ => {}
        }
    }

    // ── view helpers ────────────────────────────────────────────────────

    /// Indices of frames (into `self.frames`) that pass the current filter,
    /// newest last.
    pub fn filtered_indices(&self) -> Vec<usize> {
        self.frames
            .iter()
            .enumerate()
            .filter(|(_, e)| self.filter.matches(e))
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    //! Deterministic key-handling tests (no terminal / pty required).

    use super::*;
    use crate::cli::LiveArgs;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty())
    }
    fn tab_key() -> KeyEvent {
        KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())
    }

    fn make_app() -> App {
        // Replay /dev/null (empty): the worker just signals EOF. Key handling
        // is pure logic and does not depend on the capture source.
        let args = LiveArgs {
            interface: "can0".into(),
            from_file: Some("/dev/null".into()),
            speed: 1.0,
            buffer: 100,
            tab: "live".into(),
            logfile: None,
        };
        let mut app = App::new(args).expect("app builds");
        app.drain_messages();
        app
    }

    #[test]
    fn digit_switches_from_live() {
        let mut app = make_app();
        assert_eq!(app.tab, Tab::Live);
        // Tab order: 1 Live 2 Sniffer 3 PGN 4 NMEA 5 Nodes 6 Stats 7 Filter 8 Help.
        app.handle_key(key('7'));
        assert_eq!(app.tab, Tab::Filter, "'7' should switch Live -> Filter");
    }

    #[test]
    fn digit_and_tab_escape_from_filter() {
        let mut app = make_app();
        app.handle_key(key('7'));
        assert_eq!(app.tab, Tab::Filter);
        // The bug: pressing a digit while on Filter must STILL switch tabs.
        app.handle_key(key('1'));
        assert_eq!(app.tab, Tab::Live, "'1' must escape Filter -> Live");
        // Tab must also escape from Filter (Filter is second-to-last, so
        // next() wraps to Help).
        app.handle_key(key('7'));
        assert_eq!(app.tab, Tab::Filter);
        app.handle_key(tab_key());
        assert_ne!(app.tab, Tab::Filter, "Tab must leave the Filter tab");
        assert_eq!(app.tab, Tab::Help);
    }

    #[test]
    fn filter_field_cycling_does_not_trap() {
        let mut app = make_app();
        app.handle_key(key('7')); // -> Filter
        app.handle_key(key('j')); // cycle field down
        app.handle_key(key('k')); // cycle field up
        assert_eq!(app.tab, Tab::Filter);
        // Numeric tab-switch still works after cycling fields (6 = Stats).
        app.handle_key(key('6'));
        assert_eq!(app.tab, Tab::Stats);
    }

    #[test]
    fn quit_works_from_every_tab() {
        for target in [Tab::Live, Tab::Filter, Tab::Pgn, Tab::Nmea, Tab::Stats] {
            let mut app = make_app();
            app.tab = target;
            app.handle_key(key('q'));
            assert!(app.should_quit, "q should quit from {:?}", target);
        }
    }

    #[test]
    fn ctrl_c_quits_even_while_editing() {
        let mut app = make_app();
        app.handle_key(key('7')); // Filter
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())); // edit field
        assert!(app.editing.is_some());
        // Ctrl+C must still quit from edit mode.
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit, "Ctrl+C should quit even while editing");
    }

    #[test]
    fn new_tabs_reachable_by_hotkey() {
        let mut app = make_app();
        app.handle_key(key('2'));
        assert_eq!(app.tab, Tab::Sniffer);
        app.handle_key(key('5'));
        assert_eq!(app.tab, Tab::Nodes);
    }

    #[test]
    fn sniffer_records_byte_changes() {
        use crate::tui::model::SniffTable;
        use std::time::Instant;
        let mut t = SniffTable::new();
        let now = Instant::now();
        // First sighting of 0x123 with payload [1,2,3].
        t.observe(0x123, false, &[1, 2, 3], 3, now);
        // Then byte 1 flips 2 -> 9; byte 0 unchanged.
        t.observe(0x123, false, &[1, 9, 3], 3, now);
        let row = &t.rows[&0x123];
        assert_eq!(row.data, [1, 9, 3, 0, 0, 0, 0, 0]);
        assert_eq!(row.dlc, 3);
        assert_eq!(row.count, 2);
        // Both bytes 0 and 1 were "changed" at least once (byte 0 on first
        // sighting, byte 1 on the flip).
        assert!(row.changed_at[0].is_some());
        assert!(row.changed_at[1].is_some());
        // Byte 3 is beyond DLC -> not marked.
        assert!(row.changed_at[3].is_none());
    }

    #[test]
    fn nodes_records_address_claim() {
        use crate::tui::model::{NodeTable, PGN_ADDRESS_CLAIMED};
        use std::time::Instant;
        let mut t = NodeTable::new();
        // PGN 60928 from address 0x80, payload = a NAME (8 bytes).
        let name_bytes = [0x00, 0x80, 0x83, 0x01, 0x00, 0x82, 0x00, 0x20];
        t.observe(PGN_ADDRESS_CLAIMED, 0x80, &name_bytes, Instant::now());
        // A non-claim PGN must be ignored.
        t.observe(0xFEE6, 0x80, &name_bytes, Instant::now());
        assert_eq!(t.rows.len(), 1);
        let e = &t.rows[&0x80];
        assert_eq!(e.address, 0x80);
        assert_eq!(e.count, 1);
    }

    #[test]
    fn renders_every_tab_without_panicking() {
        // TestBackend renders into an in-memory buffer — no terminal needed,
        // and a panic (e.g. an off-by-one in a new tab) fails the test.
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = make_app();
        // Inject one Address-Claimed frame so Live/Sniffer/Nodes are non-empty.
        let raw = crate::can::RawFrame::make_ext(
            0x18EEFF80,
            &[0x00, 0x80, 0x83, 0x01, 0x00, 0x82, 0x00, 0x20],
        );
        let entry = crate::tui::decode::build_entry(1, 0, "vcan0".into(), &raw);
        app.sniff.observe(
            entry.raw_id,
            entry.extended,
            &entry.data,
            entry.dlc,
            Instant::now(),
        );
        app.nodes.observe(
            entry.decoded.pgn.unwrap_or(0),
            entry.decoded.source.unwrap_or(0),
            &entry.data,
            Instant::now(),
        );
        app.frames.push_back(entry);

        let mut term = Terminal::new(TestBackend::new(110, 32)).unwrap();
        for tab in [
            Tab::Live,
            Tab::Sniffer,
            Tab::Pgn,
            Tab::Nmea,
            Tab::Nodes,
            Tab::Stats,
            Tab::Filter,
            Tab::Help,
        ] {
            app.tab = tab;
            app.selected = 0;
            term.draw(|f| view::render(f, &mut app))
                .expect("render must not panic");
        }
    }
}
