//! Live VT server mode for `machbus term server`.
//!
//! Runs a machbus [`Session`] with the [`VtServer`] plugin over a live
//! SocketCAN socket. The session claims an address, accepts a client's
//! object-pool upload (ETP, reassembled by IsoNet), and once a client's pool
//! is activated we build a [`VtRenderRuntime`] and render it.
//!
//! **Mouse / soft-key interaction**: clicks on the VT screen are mapped to VT
//! pixel coordinates and fed to the runtime as [`OperatorEvent::Tap`].
//! Physical soft keys are shown as a clickable bottom bar.  Activations
//! produced by the runtime are bridged back onto the bus as VT→ECU messages.

use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use machbus::isobus::vt::render::framebuffer::Framebuffer;
use machbus::isobus::vt::render::input::OperatorEvent;
use machbus::isobus::vt::render::runtime::ActivationHoldTiming;
use machbus::isobus::vt::render::{FramebufferRenderer, LayoutConfig, VtRenderRuntime};
use machbus::isobus::vt::{VTServerConfig, VTServerState};
use machbus::net::pgn_defs::PGN_VT_TO_ECU;
use machbus::net::{Name, Priority};
use machbus::session::Session;
use machbus::session::plugins::VtServer;
use machbus::time::Instant as MbInstant;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal};

use crate::bus::Bus;
use crate::cli::TermServerArgs;
use crate::term::fbview;

/// Run the live VT server TUI (`machbus term server`).
pub fn run_server(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    args: TermServerArgs,
) -> Result<(), String> {
    let iface = args.iface.clone();
    let addr = parse_addr(&args.addr)?;

    let mut vt_config = VTServerConfig::default()
        .with_screen(480, 240)
        .with_version(4);
    vt_config.physical_soft_keys = 10;
    let mut vt = VtServer::new(vt_config).map_err(|e| format!("vt server: {e}"))?;
    vt.start().map_err(|e| format!("vt start: {e}"))?;

    let name = vt_name();
    let mut session = Session::builder(name, addr)
        .plug(vt)
        .build()
        .map_err(|e| format!("session build: {e}"))?;
    session.start().map_err(|e| format!("session start: {e}"))?;

    let bus = Bus::open(&iface).map_err(|e| format!("open {iface}: {e}"))?;

    let mut state = LiveState::default();
    let tick = Duration::from_millis(3);
    let mut should_quit = false;
    let start = std::time::Instant::now();
    let mut screen_rect = Rect::default();
    let mut key_slots: Vec<KeySlot> = Vec::new();

    while !should_quit {
        let now = MbInstant::ZERO.add_millis(start.elapsed().as_millis() as u64);
        bus.pump(&mut session, now);
        session.tick(now);
        refresh(&mut state, &mut session);

        terminal
            .draw(|f| {
                let (sr, ks) = draw(f, &state, &iface, addr, &session);
                screen_rect = sr;
                key_slots = ks;
            })
            .map_err(|e| format!("draw: {e}"))?;

        if event::poll(tick).map_err(|e| format!("poll: {e}"))? {
            loop {
                match event::read() {
                    Ok(ev) => match ev {
                        Event::Key(k) if k.kind == KeyEventKind::Press => {
                            if (k.code == KeyCode::Char('c')
                                && k.modifiers.contains(KeyModifiers::CONTROL))
                                || k.code == KeyCode::Char('q')
                                || k.code == KeyCode::Esc
                            {
                                should_quit = true;
                            }
                        }
                        Event::Mouse(m) => {
                            if m.kind == MouseEventKind::Down(MouseButton::Left) {
                                handle_click(&mut state, screen_rect, &key_slots, m.column, m.row);
                            }
                        }
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
    }
    Ok(())
}

/// Mutable live-render state.
#[derive(Default)]
struct LiveState {
    runtime: Option<VtRenderRuntime>,
    frame: Option<Framebuffer>,
    status: String,
    client_addr: Option<u8>,
}

/// Build (or skip) the runtime + framebuffer from the active client.
fn refresh(state: &mut LiveState, session: &mut Session) {
    // Once we have a runtime, re-render the framebuffer from it each tick
    // (the scene may change due to ECU commands or operator interaction).
    if let Some(rt) = &mut state.runtime {
        if let Ok(fb) = FramebufferRenderer::default().try_render_scene(rt.scene()) {
            state.frame = Some(fb);
        }
        // Bridge pending activations → bus messages → send to ECU.
        let dst = state.client_addr;
        if let Some(dst) = dst
            && let Ok((_evts, msgs)) = rt
                .advance_activation_hold_time_with_bus_messages(3, ActivationHoldTiming::default())
        {
            for msg in msgs {
                let _ = session.send_raw(PGN_VT_TO_ECU, &msg.data, dst, Priority::Default);
            }
        }
        return;
    }

    // No runtime yet — try to build one from an active client.
    let Some(vt) = session.get_mut::<VtServer>() else {
        return;
    };
    let srv = vt.server_mut();
    if let Some(client) = srv.clients().iter().find(|c| c.pool_activated) {
        match VtRenderRuntime::from_server_working_set(client, LayoutConfig::default()) {
            Ok(rt) => {
                state.client_addr = Some(client.client_address);
                state.status = format!(
                    "connected — rendering (client 0x{:02X})",
                    client.client_address
                );
                state.runtime = Some(rt);
            }
            Err(e) => state.status = format!("runtime error: {e}"),
        }
    } else {
        state.status = format!("waiting for a client…  (VT state: {:?})", srv.state());
    }
}

/// Handle a left-click: on a key button (→ PhysicalSoftKey) or on the VT
/// screen (→ Tap).
fn handle_click(state: &mut LiveState, screen: Rect, key_slots: &[KeySlot], mx: u16, my: u16) {
    // Soft-key buttons?
    for &(rect, idx) in key_slots {
        if mx >= rect.x
            && mx < rect.x + rect.width
            && my >= rect.y
            && my < rect.y + rect.height
            && let Some(rt) = &mut state.runtime
        {
            let _ = rt.handle_operator_event(OperatorEvent::PhysicalSoftKey(idx));
            state.status = format!("soft key {idx} pressed");
            return;
        }
    }

    // VT screen?
    if mx >= screen.x
        && mx < screen.x + screen.width
        && my >= screen.y
        && my < screen.y + screen.height
        && let Some(fb) = &state.frame
        && let Some((fx, fy)) = fbview::screen_to_pixel(screen, fb, mx - screen.x, my - screen.y)
        && let Some(rt) = &mut state.runtime
    {
        let _ = rt.handle_operator_event(OperatorEvent::Tap(fx, fy));
        state.status = format!("tap at ({fx}, {fy})");
    }
}

/// One physical soft-key slot: rect + key index.
type KeySlot = (Rect, u8);

/// Draw the full UI.  Returns the VT-screen rect and all 10 key-button rects
/// (for mouse hit-testing).
fn draw(
    f: &mut Frame,
    state: &LiveState,
    iface: &str,
    addr: u8,
    session: &Session,
) -> (Rect, Vec<KeySlot>) {
    let area = f.area();
    // Portrait (tall) → keys on left + right;  landscape (wide) → top + bottom.
    let portrait = area.height > area.width;

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    draw_title(f, iface, addr, session, outer[0]);

    let body = outer[1];
    let (screen_rect, key_slots) = if portrait {
        // left_keys(10) | screen | right_keys(10)
        let key_w = 10u16;
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(key_w),
                Constraint::Min(0),
                Constraint::Length(key_w),
            ])
            .split(body);
        let left = split_keys_vertical(cols[0], 0);
        let right = split_keys_vertical(cols[2], 5);
        let inner = draw_screen(f, state, cols[1]);
        (inner, left.into_iter().chain(right).collect())
    } else {
        // top_keys(5) | screen | bottom_keys(5)
        let key_h = 5u16;
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(key_h),
                Constraint::Min(0),
                Constraint::Length(key_h),
            ])
            .split(body);
        let top = split_keys_horizontal(rows[0], 0);
        let bot = split_keys_horizontal(rows[2], 5);
        let inner = draw_screen(f, state, rows[1]);
        (inner, top.into_iter().chain(bot).collect())
    };

    for &(rect, idx) in &key_slots {
        let label = soft_key_label(state, idx);
        render_button(f, rect, &label);
    }

    draw_status(f, state, outer[2]);
    (screen_rect, key_slots)
}

/// Split `area` into `n` equal horizontal slices, returning rects with key
/// indices starting at `start`.
fn split_keys_horizontal(area: Rect, start: u8) -> Vec<KeySlot> {
    const N: u16 = 5;
    let w = area.width / N;
    (0..N)
        .map(|i| {
            (
                Rect {
                    x: area.x + i * w,
                    y: area.y,
                    width: w,
                    height: area.height,
                },
                start + i as u8,
            )
        })
        .collect()
}

/// Split `area` into `n` equal vertical slices.
fn split_keys_vertical(area: Rect, start: u8) -> Vec<KeySlot> {
    const N: u16 = 5;
    let h = area.height / N;
    (0..N)
        .map(|i| {
            (
                Rect {
                    x: area.x,
                    y: area.y + i * h,
                    width: area.width,
                    height: h,
                },
                start + i as u8,
            )
        })
        .collect()
}

/// Render a physical soft-key button: a bordered box with the label centred
/// *inside* (not as a title).
fn render_button(f: &mut Frame, area: Rect, label: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let max = inner.width as usize;
    let display: String = label.chars().take(max).collect();
    // Vertically centre the text in taller buttons for a cleaner look.
    let text_area = if inner.height > 1 {
        Rect {
            y: inner.y + inner.height / 2,
            height: 1,
            ..inner
        }
    } else {
        inner
    };
    f.render_widget(
        Paragraph::new(display).alignment(Alignment::Center).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        text_area,
    );
}

/// Label for physical key `idx`: application label if available, else "SK N".
fn soft_key_label(state: &LiveState, idx: u8) -> String {
    if let Some(rt) = &state.runtime
        && let Some(sk) = rt.scene().soft_keys.get(idx as usize)
        && !sk.label.is_empty()
    {
        return sk.label.clone();
    }
    format!("SK {idx}")
}

fn draw_title(f: &mut Frame, iface: &str, addr: u8, session: &Session, area: Rect) {
    let brand = Span::styled(
        " ◆ machbus term ",
        Style::default().fg(Color::Black).bg(Color::Cyan),
    );
    let sub = Span::styled(" VT (live) ", Style::default().fg(Color::DarkGray));
    let vt_state = session
        .get::<VtServer>()
        .map(|v| v.state())
        .unwrap_or(VTServerState::Disconnected);
    let right = Line::from(vec![
        Span::styled(format!(" {iface} "), Style::default().fg(Color::White)),
        Span::styled("·", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(" VT 0x{addr:02X} "),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("·", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {vt_state:?} "), Style::default().fg(Color::White)),
    ])
    .alignment(Alignment::Right);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(48)])
        .split(area);
    f.render_widget(Paragraph::new(Line::from(vec![brand, sub])), cols[0]);
    f.render_widget(Paragraph::new(right), cols[1]);
    f.render_widget(
        Paragraph::new(Span::styled(
            "━".repeat(area.width as usize),
            Style::default().fg(Color::DarkGray),
        )),
        Rect {
            y: area.y + 1,
            height: 1,
            ..area
        },
    );
}

fn draw_screen(f: &mut Frame, state: &LiveState, area: Rect) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(
            Line::from(" VT screen (click to interact) ").style(Style::default().fg(Color::Cyan)),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    match &state.frame {
        Some(fb) => fbview::paint(f.buffer_mut(), inner, fb),
        None => f.render_widget(
            Paragraph::new(state.status.clone())
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center),
            inner,
        ),
    }
    inner
}

fn draw_status(f: &mut Frame, state: &LiveState, area: Rect) {
    let left = Line::from(vec![
        Span::styled(
            " q",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("quit", Style::default().fg(Color::DarkGray)),
        Span::raw("   "),
        Span::styled(
            "🖱 click",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "VT screen / soft keys",
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("   "),
        Span::styled(
            format!("❯ {}", state.status),
            Style::default().fg(Color::Yellow),
        ),
    ]);
    f.render_widget(Paragraph::new(left), area);
}

fn vt_name() -> Name {
    Name::default()
        .with_self_configurable(true)
        .with_function_code(0x1C)
        .with_identity_number(0x0001)
}

fn parse_addr(spec: &str) -> Result<u8, String> {
    u8::from_str_radix(spec.trim_start_matches("0x"), 16)
        .map_err(|_| format!("--addr '{spec}': expected hex byte, e.g. 26"))
}
