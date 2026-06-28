//! `machbus term` — an ISOBUS Virtual Terminal renderer.
//!
//! Loads an object pool (`.iop`), renders the active mask to a machbus
//! [`Framebuffer`], and displays it in ratatui using half-block pixels
//! (see [`fbview`]). A side panel lists the pool's masks so you can switch
//! between them interactively. This mirrors the *display* side of the
//! upstream AgIsoVirtualTerminal; the heavy lifting (object-pool parse +
//! render into pixels) is done by machbus's own VT pipeline.

pub mod client;
pub mod fbview;
pub mod live;
mod view;

use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use machbus::isobus::vt::render::framebuffer::Framebuffer;
use machbus::isobus::vt::render::{FramebufferRenderer, IopDocument, LayoutConfig, LayoutEngine};
use machbus::isobus::vt::{ObjectID, ObjectType};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::cli::{TermClientArgs, TermFileArgs, TermServerArgs};

/// `machbus term file` — render a pool from disk.
pub fn run_file(args: TermFileArgs) -> Result<(), String> {
    let mut terminal = setup_terminal()?;
    let result = (|| {
        let mut app = TermApp::load(args)?;
        app_loop(&mut terminal, &mut app)
    })();
    restore_terminal(&mut terminal);
    result
}

/// `machbus term server` — live VT server on a CAN interface.
pub fn run_server(args: TermServerArgs) -> Result<(), String> {
    let mut terminal = setup_terminal()?;
    let result = live::run_server(&mut terminal, args);
    restore_terminal(&mut terminal);
    result
}

/// `machbus term client` — upload a pool to a live VT (test counterpart).
pub fn run_client(args: TermClientArgs) -> Result<(), String> {
    client::run(args)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, String> {
    enable_raw_mode().map_err(|e| format!("enable_raw_mode: {e}"))?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| format!("enter alternate screen: {e}"))?;
    Terminal::new(CrosstermBackend::new(std::io::stdout())).map_err(|e| format!("terminal: {e}"))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();
}

fn app_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut TermApp,
) -> Result<(), String> {
    crate::signal::install_cancel_handler();
    let tick = Duration::from_millis(50);
    loop {
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

/// The VT terminal application state.
pub struct TermApp {
    pub source: String,
    engine: LayoutEngine,
    pub doc: IopDocument,
    pub frame: Framebuffer,
    pub masks: Vec<ObjectID>,
    pub mask_sel: usize,
    pub panel_open: bool,
    pub should_quit: bool,
    pub message: Option<(String, Instant)>,
}

impl TermApp {
    /// Load the pool, build the layout engine, enumerate masks, and render
    /// the initial mask. (`machbus term file`.)
    pub fn load(args: TermFileArgs) -> Result<Self, String> {
        let iop_path = &args.iop;
        let bytes = std::fs::read(iop_path).map_err(|e| format!("read '{iop_path}': {e}"))?;
        let mut config = LayoutConfig::default();
        if let Some(c) = &args.canvas {
            config.canvas = parse_canvas(c)?;
        }
        if let Some(n) = args.physical_soft_keys {
            config.physical_soft_key_count = n;
        }
        if let Some(n) = args.navigation_soft_keys {
            config.navigation_soft_key_count = n;
        }

        let engine = LayoutEngine::new(config);
        let doc = IopDocument::load(&bytes, config).map_err(|e| format!("pool rejected: {e}"))?;

        // Enumerate every mask object (Data / Alarm / Window) for the selector.
        let masks: Vec<ObjectID> = doc
            .pool()
            .objects()
            .iter()
            .filter(|o| {
                matches!(
                    o.r#type,
                    ObjectType::DataMask | ObjectType::AlarmMask | ObjectType::WindowMask
                )
            })
            .map(|o| o.id)
            .collect();

        let doc = match &args.mask {
            Some(spec) => doc.with_scene(&engine, parse_object_id(spec)?),
            None => doc,
        };

        let active = doc.scene().active_mask;
        let mask_sel = masks.iter().position(|m| *m == active).unwrap_or(0);
        let frame = render_scene(&doc)?;
        let mut app = Self {
            source: std::path::Path::new(iop_path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| iop_path.to_string()),
            engine,
            doc,
            frame,
            masks,
            mask_sel,
            panel_open: true,
            should_quit: false,
            message: None,
        };
        app.flash(format!(
            "loaded '{}' ({} objects)",
            app.source,
            app.doc.pool().size()
        ));
        Ok(app)
    }

    /// Re-render the pool with `mask` as the active mask.
    fn show_mask(&mut self, mask: ObjectID) {
        // `with_scene` consumes the document; clone first so we can keep the
        // field assignable through `&mut self`.
        let new_doc = self.doc.clone().with_scene(&self.engine, mask);
        self.doc = new_doc;
        match render_scene(&self.doc) {
            Ok(f) => {
                self.frame = f;
                self.mask_sel = self
                    .masks
                    .iter()
                    .position(|m| *m == mask)
                    .unwrap_or(self.mask_sel);
                self.flash(format!("mask 0x{:04X}", mask.raw()));
            }
            Err(e) => self.flash(format!("render: {e}")),
        }
    }

    fn next_mask(&mut self) {
        if self.masks.len() < 2 {
            return;
        }
        let idx = (self.mask_sel + 1) % self.masks.len();
        self.show_mask(self.masks[idx]);
    }

    fn prev_mask(&mut self) {
        if self.masks.len() < 2 {
            return;
        }
        let idx = (self.mask_sel + self.masks.len() - 1) % self.masks.len();
        self.show_mask(self.masks[idx]);
    }

    fn re_render(&mut self) {
        match render_scene(&self.doc) {
            Ok(f) => {
                self.frame = f;
                self.flash("re-rendered".into());
            }
            Err(e) => self.flash(format!("render: {e}")),
        }
    }

    fn flash(&mut self, msg: String) {
        self.message = Some((msg, Instant::now()));
    }

    fn message_expired(&mut self) {
        if let Some((_, at)) = self.message
            && at.elapsed() > Duration::from_millis(2800)
        {
            self.message = None;
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C always quits.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        let code = key.code;
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.panel_open = !self.panel_open,
            KeyCode::Char('n') => self.next_mask(),
            KeyCode::Char('p') => self.prev_mask(),
            KeyCode::Char('r') => self.re_render(),
            // Panel navigation.
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.masks.is_empty() {
                    let i = (self.mask_sel + 1) % self.masks.len();
                    self.show_mask(self.masks[i]);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.masks.is_empty() {
                    let i = (self.mask_sel + self.masks.len() - 1) % self.masks.len();
                    self.show_mask(self.masks[i]);
                }
            }
            _ => {}
        }
    }
}

/// Render the document's current scene to a framebuffer.
fn render_scene(doc: &IopDocument) -> Result<Framebuffer, String> {
    FramebufferRenderer::default()
        .try_render_scene(doc.scene())
        .map_err(|e| format!("{e:?}"))
}

fn parse_canvas(spec: &str) -> Result<(u16, u16), String> {
    let (w, h) = spec
        .split_once(['x', 'X', ','])
        .ok_or_else(|| format!("--canvas '{spec}': expected WxH, e.g. 480x240"))?;
    let w: u16 = w.trim().parse().map_err(|_| format!("bad width '{w}'"))?;
    let h: u16 = h.trim().parse().map_err(|_| format!("bad height '{h}'"))?;
    Ok((w, h))
}

fn parse_object_id(spec: &str) -> Result<ObjectID, String> {
    let v = u16::from_str_radix(spec.trim_start_matches("0x"), 16)
        .map_err(|_| format!("mask id '{spec}': expected hex, e.g. 1F"))?;
    Ok(ObjectID::new(v))
}

#[cfg(test)]
mod tests {
    //! Deterministic load/render tests against the checked-in sample pool.

    use super::*;

    const SAMPLE_POOL: &str = "../tests/fixtures/isobus/VT3TestPool.iop";

    fn sample_args() -> TermFileArgs {
        TermFileArgs {
            iop: SAMPLE_POOL.into(),
            mask: None,
            canvas: None,
            physical_soft_keys: None,
            navigation_soft_keys: None,
        }
    }

    #[test]
    fn loads_and_renders_sample_pool() {
        let app = TermApp::load(sample_args()).expect("sample pool must load + render");
        assert!(app.frame.width() > 0, "framebuffer has width");
        assert!(app.frame.height() > 0, "framebuffer has height");
        assert!(
            !app.masks.is_empty(),
            "pool should expose at least one mask"
        );
    }

    #[test]
    fn switching_masks_re_renders() {
        let mut app = TermApp::load(sample_args()).expect("load");
        let first_w = app.frame.width();
        if app.masks.len() > 1 {
            let other = app.masks[(app.mask_sel + 1) % app.masks.len()];
            app.show_mask(other);
            assert_eq!(app.doc.scene().active_mask, other);
        }
        // Stays a valid framebuffer regardless of mask.
        assert!(app.frame.width() >= 1);
        let _ = first_w;
    }

    #[test]
    fn renders_term_view_without_panicking() {
        // TestBackend renders into an in-memory buffer (no terminal); catches
        // any panic in the layout or framebuffer painter.
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut app = TermApp::load(sample_args()).expect("load");
        app.panel_open = true;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| view::render(f, &mut app))
            .expect("render must not panic");
        app.panel_open = false;
        term.draw(|f| view::render(f, &mut app))
            .expect("render (no panel) must not panic");
    }
}
