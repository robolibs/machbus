//! `machbus drive` — ISOBUS guidance + telemetry TUI.
//!
//! Input uses a **continuous intensity** model instead of binary on/off.
//! Each press sets intensity to 1.0; it decays smoothly toward 0 over
//! 0.5 seconds. Physics uses the intensity as a multiplier, so a key
//! at 50% intensity applies 50% of the force. The visual stays lit
//! while intensity > 5%. This eliminates all flicker because there's
//! no binary snap — the terminal's irregular repeat timing just
//! refreshes the intensity back to 1.0.

mod view;

use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use machbus::isobus::implement::tractor_commands::{HitchCommand, PtoCommand};
use machbus::net::Name;
use machbus::session::Session;
use machbus::session::plugins::{Gnss, Guidance, Implement};
use machbus::session::{Hitch, Pto};
use machbus::time::Instant as MbInstant;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::bus::Bus;
use crate::cli::DriveArgs;
use crate::signal;

// How fast key intensity decays (seconds to reach 0 after last press).
// Must be long enough to survive the terminal's repeat pauses (~1s gap
// after ~1s of holding on some systems). 2s gives comfortable margin.
const KEY_DECAY: f64 = 1.0;

// Physics rates (per second, proportional to setpoint).
const R_WITH: f64 = 0.10;
const R_DRAG: f64 = 0.05;

/// A key with continuous intensity (0.0–1.0). Press = 1.0, decays to 0
/// over `KEY_DECAY` seconds. No binary snap = no flicker.
pub struct Key {
    intensity: f64,
}

impl Key {
    pub fn new() -> Self {
        Self { intensity: 0.0 }
    }

    fn press(&mut self) {
        self.intensity = 1.0;
    }

    fn tick(&mut self, dt: f64) {
        self.intensity = (self.intensity - dt / KEY_DECAY).max(0.0);
    }

    /// Visual: lit while there's any intensity left.
    pub fn lit(&self) -> bool {
        self.intensity > 0.05
    }
}

pub struct DriveState {
    pub speed: f64,
    pub speed_limit: f64,
    pub speed_step: f64,
    pub max_curvature: f64,
    pub steer: f64,
    /// Counter-direction multiplier (1x, 2x, 3x, 4x). Cycled by X key.
    pub counter_mult: u8,
    pub status: String,
    pub claimed: bool,
    pub claimed_addr: u8,
    pub kw: Key,
    pub ks: Key,
    pub ka: Key,
    pub kd: Key,
    pub ki: Key,
    pub kk: Key,
    pub kh: Key,
    pub kj: Key,
    pub kp: Key,
    pub ko: Key,
    pub kx: Key,
    pub kenter: Key,
}

impl DriveState {
    fn new(args: &DriveArgs) -> Self {
        Self {
            speed: 0.0,
            speed_limit: args.default_speed,
            speed_step: args.speed_step,
            max_curvature: args.max_curvature,
            steer: 0.0,
            counter_mult: 2,
            status: "press I to set speed, then W".into(),
            claimed: false,
            claimed_addr: 0,
            kw: Key::new(),
            ks: Key::new(),
            ka: Key::new(),
            kd: Key::new(),
            ki: Key::new(),
            kk: Key::new(),
            kh: Key::new(),
            kj: Key::new(),
            kp: Key::new(),
            ko: Key::new(),
            kx: Key::new(),
            kenter: Key::new(),
        }
    }

    pub fn curvature(&self) -> f64 {
        self.steer * self.max_curvature
    }

    fn tick_keys(&mut self, dt: f64) {
        self.kw.tick(dt);
        self.ks.tick(dt);
        self.ka.tick(dt);
        self.kd.tick(dt);
        self.ki.tick(dt);
        self.kk.tick(dt);
        self.kh.tick(dt);
        self.kj.tick(dt);
        self.kp.tick(dt);
        self.ko.tick(dt);
        self.kx.tick(dt);
        self.kenter.tick(dt);
    }

    fn tick_physics(&mut self, dt: f64) {
        let w_eff = self.kw.intensity - self.ks.intensity;
        let s_eff = self.ks.intensity - self.kw.intensity;
        let steer_eff = self.kd.intensity - self.ka.intensity;
        let limit = self.speed_limit.abs().max(0.5);
        let against = R_WITH * self.counter_mult as f64;

        // Speed.
        if w_eff > 0.0 {
            let r = if self.speed >= 0.0 { R_WITH } else { against };
            self.speed =
                (self.speed + r * limit * dt * w_eff).clamp(-self.speed_limit, self.speed_limit);
        }
        if s_eff > 0.0 {
            let r = if self.speed <= 0.0 { R_WITH } else { against };
            self.speed =
                (self.speed - r * limit * dt * s_eff).clamp(-self.speed_limit, self.speed_limit);
        }
        if w_eff <= 0.0 && s_eff <= 0.0 {
            let d2 = R_DRAG * limit * dt;
            if self.speed > 0.0 {
                self.speed = (self.speed - d2).max(0.0);
            } else if self.speed < 0.0 {
                self.speed = (self.speed + d2).min(0.0);
            }
        }

        // Steering.
        if steer_eff > 0.0 {
            let r = if self.steer >= 0.0 { R_WITH } else { against };
            self.steer = (self.steer + r * dt * steer_eff).clamp(-1.0, 1.0);
        } else if steer_eff < 0.0 {
            let r = if self.steer <= 0.0 { R_WITH } else { against };
            self.steer = (self.steer + r * dt * steer_eff).clamp(-1.0, 1.0);
        } else {
            let r = R_DRAG * dt;
            if self.steer.abs() <= r {
                self.steer = 0.0;
            } else {
                self.steer -= self.steer.signum() * r;
            }
        }
    }

    fn flush(&mut self, session: &mut Session) {
        if !self.claimed {
            return;
        }
        if let Some(g) = session.get_mut::<Guidance>() {
            let v = self.speed;
            g.command_velocity(v, v * self.curvature() / 1000.0);
        }
    }

    fn press(&mut self, c: char, session: &mut Session) {
        match c {
            'w' => self.kw.press(),
            's' => self.ks.press(),
            'a' => self.ka.press(),
            'd' => self.kd.press(),
            'i' => {
                self.ki.press();
                self.speed_limit += self.speed_step;
            }
            'k' => {
                self.kk.press();
                self.speed_limit = (self.speed_limit - self.speed_step).max(0.0);
            }
            'h' => {
                self.kh.press();
                if let Some(imp) = session.get_mut::<Implement>() {
                    imp.command_hitch(Hitch::Rear, HitchCommand::Raise);
                }
            }
            'j' => {
                self.kj.press();
                if let Some(imp) = session.get_mut::<Implement>() {
                    imp.command_hitch(Hitch::Rear, HitchCommand::Lower);
                }
            }
            'p' => {
                self.kp.press();
                if let Some(imp) = session.get_mut::<Implement>() {
                    imp.command_pto(Pto::Rear, PtoCommand::Engage);
                }
            }
            'o' => {
                self.ko.press();
                if let Some(imp) = session.get_mut::<Implement>() {
                    imp.command_pto(Pto::Rear, PtoCommand::Disengage);
                }
            }
            'x' => {
                self.kx.press();
                self.counter_mult = (self.counter_mult % 4) + 1; // 1→2→3→4→1
            }
            '\n' => {
                self.kenter.press();
                self.speed = 0.0;
                self.steer = 0.0;
            }
            _ => {}
        }
    }
}

pub fn run(args: DriveArgs) -> Result<(), String> {
    let addr = parse_addr(&args.addr)?;
    let name = Name::default()
        .with_self_configurable(true)
        .with_function_code(0x80)
        .with_identity_number(0x0042);
    let mut session = Session::builder(name, addr)
        .plug(Guidance::new())
        .plug(Implement::new())
        .plug(Gnss::new(
            machbus::nmea::NMEAConfig::default().with_all(true),
        ))
        .build()
        .map_err(|e| format!("session: {e}"))?;
    session.start().map_err(|e| format!("start: {e}"))?;
    let bus = Bus::open(&args.iface).map_err(|e| format!("open: {e}"))?;

    signal::install_cancel_handler();
    let mut terminal = setup_terminal()?;
    let result = drive_loop(&mut terminal, &mut session, &bus, &args);
    restore_terminal(&mut terminal);
    result
}

fn drive_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    session: &mut Session,
    bus: &Bus,
    args: &DriveArgs,
) -> Result<(), String> {
    let mut state = DriveState::new(args);
    let start = Instant::now();
    let mut last = start;
    let mut should_quit = false;

    while !should_quit {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(0.1);
        last = now;

        // 1. Drain all input FIRST (before key decay).
        while event::poll(Duration::from_millis(3)).map_err(|e| format!("poll: {e}"))? {
            if let Ok(Event::Key(k)) = event::read()
                && k.kind == KeyEventKind::Press
            {
                if (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
                    || k.code == KeyCode::Char('q')
                {
                    should_quit = true;
                    break;
                }
                let c = match k.code {
                    KeyCode::Char(ch) => ch.to_ascii_lowercase(),
                    KeyCode::Enter => '\n',
                    _ => continue,
                };
                state.press(c, session);
            }
        }

        // 2. Bus + session.
        let mb = MbInstant::ZERO.add_millis(start.elapsed().as_millis() as u64);
        bus.pump(session, mb);
        session.tick(mb);

        // 3. Claim.
        let was = state.claimed;
        state.claimed = session.is_claimed();
        if state.claimed && !was {
            state.claimed_addr = session.address();
        }

        // 4. Decay keys + physics.
        state.tick_keys(dt);
        state.tick_physics(dt);
        state.flush(session);

        // 5. Status + render.
        if state.claimed {
            state.status = format!(
                "v={:.2}  κ={:.1}  steer={:+.2}  limit={:.1}",
                state.speed,
                state.curvature(),
                state.steer,
                state.speed_limit,
            );
        }

        terminal
            .draw(|f| view::render(f, &state, session))
            .map_err(|e| format!("draw: {e}"))?;

        if signal::cancel_requested() {
            should_quit = true;
        }
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, String> {
    crossterm::terminal::enable_raw_mode().map_err(|e| format!("raw mode: {e}"))?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )
    .map_err(|e| format!("alt screen: {e}"))?;
    Terminal::new(CrosstermBackend::new(std::io::stdout())).map_err(|e| format!("terminal: {e}"))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) {
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    );
    let _ = terminal.show_cursor();
}

fn parse_addr(spec: &str) -> Result<u8, String> {
    u8::from_str_radix(spec.trim_start_matches("0x"), 16)
        .map_err(|_| format!("--addr '{spec}': expected hex byte"))
}
