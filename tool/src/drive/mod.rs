//! `machbus drive` — ISOBUS guidance + telemetry TUI.
//!
//! Input uses a **continuous intensity** model instead of binary on/off.
//! Each press sets intensity to 1.0; it decays smoothly toward 0 over
//! 0.5 seconds. Physics uses the intensity as a multiplier, so a key
//! at 50% intensity applies 50% of the force. The visual stays lit
//! while intensity > 5%. This eliminates all flicker because there's
//! no binary snap — the terminal's irregular repeat timing just
//! refreshes the intensity back to 1.0.

//! `machbus drive` — shared physics + state. Input is in `keyboard.rs` or
//! `joystick.rs`; rendering is in `view.rs`.

pub mod joystick;
pub mod keyboard;
mod view;

use std::time::Instant;

use machbus::net::Name;
use machbus::session::Session;
use machbus::session::plugins::{Gnss, Guidance, Implement};
use machbus::time::Instant as MbInstant;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::bus::Bus;
use crate::cli::DriveArgs;

// Physics rates (per second, proportional to setpoint).
const R_WITH: f64 = 0.10;
const R_DRAG: f64 = 0.05;

pub struct DriveState {
    pub speed: f64,
    pub speed_limit: f64,
    pub speed_step: f64,
    pub max_curvature: f64,
    pub steer: f64,
    pub counter_mult: u8,
    pub status: String,
    pub claimed: bool,
    pub claimed_addr: u8,
}

impl DriveState {
    pub fn new(args: &DriveArgs) -> Self {
        Self {
            speed: 0.0,
            speed_limit: args.default_speed,
            speed_step: args.speed_step,
            max_curvature: args.max_curvature,
            steer: 0.0,
            counter_mult: 2,
            status: String::new(),
            claimed: false,
            claimed_addr: 0,
        }
    }

    pub fn curvature(&self) -> f64 {
        self.steer * self.max_curvature
    }

    /// Apply physics from an analog input (-1..+1 for each axis).
    /// `throttle`: +1 = full forward, -1 = full reverse/brake.
    /// `steer_input`: +1 = full right, -1 = full left.
    /// Values are applied directly (the stick IS the gradual control).
    pub fn apply_analog(&mut self, throttle: f64, steer_input: f64, dt: f64) {
        let limit = self.speed_limit.abs().max(0.5);
        let against = R_WITH * self.counter_mult as f64;

        // Speed: throttle directly sets target speed as fraction of limit.
        let target = throttle * limit;
        let rate =
            if (target - self.speed).signum() == self.speed.signum() || self.speed.abs() < 0.01 {
                R_WITH * 0.3 // moving toward target direction: moderate
            } else {
                against * 0.3 // countering: faster
            };
        let max_delta = rate * limit * dt;
        let diff = target - self.speed;
        if diff.abs() <= max_delta {
            self.speed = target;
        } else {
            self.speed += diff.signum() * max_delta;
        }
        self.speed = self.speed.clamp(-limit, limit);

        // Steer: analog input directly sets target steer.
        let steer_target = steer_input.clamp(-1.0, 1.0);
        let steer_rate = if steer_target.signum() == self.steer.signum() || self.steer.abs() < 0.01
        {
            R_WITH * 0.3
        } else {
            against * 0.3
        };
        let max_s = steer_rate * dt;
        let s_diff = steer_target - self.steer;
        if s_diff.abs() <= max_s {
            self.steer = steer_target;
        } else {
            self.steer += s_diff.signum() * max_s;
        }

        // If no input, drift toward 0.
        if throttle.abs() < 0.05 {
            let d2 = R_DRAG * limit * dt;
            if self.speed > 0.0 {
                self.speed = (self.speed - d2).max(0.0);
            } else if self.speed < 0.0 {
                self.speed = (self.speed + d2).min(0.0);
            }
        }
        if steer_input.abs() < 0.05 {
            let r = R_DRAG * dt;
            if self.steer.abs() <= r {
                self.steer = 0.0;
            } else {
                self.steer -= self.steer.signum() * r;
            }
        }
    }

    pub fn flush(&mut self, session: &mut Session) {
        if !self.claimed {
            return;
        }
        if let Some(g) = session.get_mut::<Guidance>() {
            let v = self.speed;
            g.command_velocity(v, v * self.curvature() / 1000.0);
        }
    }

    pub fn update_status(&mut self) {
        if self.claimed {
            self.status = format!(
                "v={:.2}  κ={:.1}  steer={:+.2}  limit={:.1}",
                self.speed,
                self.curvature(),
                self.steer,
                self.speed_limit,
            );
        }
    }
}

/// Shared session setup for both keyboard and joystick modes.
pub fn setup_session(args: &DriveArgs) -> Result<(Session, Bus, DriveState), String> {
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
    let state = DriveState::new(args);
    Ok((session, bus, state))
}

/// Shared pump + claim + flush tick.
pub fn shared_tick(session: &mut Session, bus: &Bus, state: &mut DriveState, start: Instant) {
    let mb = MbInstant::ZERO.add_millis(start.elapsed().as_millis() as u64);
    bus.pump(session, mb);
    session.tick(mb);

    let was = state.claimed;
    state.claimed = session.is_claimed();
    if state.claimed && !was {
        state.claimed_addr = session.address();
    }
    state.flush(session);
    state.update_status();
}

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, String> {
    crossterm::terminal::enable_raw_mode().map_err(|e| format!("raw mode: {e}"))?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )
    .map_err(|e| format!("alt screen: {e}"))?;
    Terminal::new(CrosstermBackend::new(std::io::stdout())).map_err(|e| format!("terminal: {e}"))
}

pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) {
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    );
    let _ = terminal.show_cursor();
}

pub fn parse_addr(spec: &str) -> Result<u8, String> {
    u8::from_str_radix(spec.trim_start_matches("0x"), 16)
        .map_err(|_| format!("--addr '{spec}': expected hex byte"))
}
