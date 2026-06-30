//! `machbus drive keyboard` — WASD input mode.

use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use machbus::isobus::implement::tractor_commands::{HitchCommand, PtoCommand};
use machbus::session::Session;
use machbus::session::{Hitch, Pto};

use crate::cli::DriveArgs;
use crate::signal;

use super::view;
use super::{restore_terminal, setup_session, setup_terminal, shared_tick};

// Key decay: intensity reaches 0 over this many seconds after last press.
const KEY_DECAY: f64 = 1.0;

pub struct Key {
    pub intensity: f64,
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
    pub fn lit(&self) -> bool {
        self.intensity > 0.05
    }
}

pub struct KeyboardState {
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

impl KeyboardState {
    pub fn new() -> Self {
        Self {
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

    fn tick(&mut self, dt: f64) {
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

    fn apply_physics(&self, drive: &mut super::DriveState, dt: f64) {
        let w_eff = self.kw.intensity - self.ks.intensity;
        let s_eff = self.ks.intensity - self.kw.intensity;
        let steer_eff = self.kd.intensity - self.ka.intensity;
        let limit = drive.speed_limit.abs().max(0.5);
        let r_with = super::R_WITH;
        let against = r_with * drive.counter_mult as f64;

        if w_eff > 0.0 {
            let r = if drive.speed >= 0.0 { r_with } else { against };
            drive.speed =
                (drive.speed + r * limit * dt * w_eff).clamp(-drive.speed_limit, drive.speed_limit);
        }
        if s_eff > 0.0 {
            let r = if drive.speed <= 0.0 { r_with } else { against };
            drive.speed =
                (drive.speed - r * limit * dt * s_eff).clamp(-drive.speed_limit, drive.speed_limit);
        }
        if w_eff <= 0.0 && s_eff <= 0.0 {
            let d2 = super::R_DRAG * limit * dt;
            if drive.speed > 0.0 {
                drive.speed = (drive.speed - d2).max(0.0);
            } else if drive.speed < 0.0 {
                drive.speed = (drive.speed + d2).min(0.0);
            }
        }

        if steer_eff > 0.0 {
            let r = if drive.steer >= 0.0 { r_with } else { against };
            drive.steer = (drive.steer + r * dt * steer_eff).clamp(-1.0, 1.0);
        } else if steer_eff < 0.0 {
            let r = if drive.steer <= 0.0 { r_with } else { against };
            drive.steer = (drive.steer + r * dt * steer_eff).clamp(-1.0, 1.0);
        } else {
            let r = super::R_DRAG * dt;
            if drive.steer.abs() <= r {
                drive.steer = 0.0;
            } else {
                drive.steer -= drive.steer.signum() * r;
            }
        }
    }

    fn handle_press(&mut self, c: char, drive: &mut super::DriveState, session: &mut Session) {
        match c {
            'w' => self.kw.press(),
            's' => self.ks.press(),
            'a' => self.ka.press(),
            'd' => self.kd.press(),
            'i' => {
                self.ki.press();
                drive.speed_limit += drive.speed_step;
            }
            'k' => {
                self.kk.press();
                drive.speed_limit = (drive.speed_limit - drive.speed_step).max(0.0);
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
                drive.counter_mult = (drive.counter_mult % 4) + 1;
            }
            '\n' => {
                self.kenter.press();
                drive.speed = 0.0;
                drive.steer = 0.0;
            }
            _ => {}
        }
    }
}

use machbus::session::plugins::Implement;

pub fn run(args: DriveArgs) -> Result<(), String> {
    signal::install_cancel_handler();
    let (mut session, bus, mut drive) = setup_session(&args)?;
    let mut kb = KeyboardState::new();
    let mut terminal = setup_terminal()?;
    let start = Instant::now();
    let mut last = start;
    let mut should_quit = false;

    while !should_quit {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(0.1);
        last = now;

        // Drain input first.
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
                kb.handle_press(c, &mut drive, &mut session);
            }
        }

        shared_tick(&mut session, &bus, &mut drive, start);
        kb.tick(dt);
        kb.apply_physics(&mut drive, dt);
        drive.flush(&mut session);
        drive.update_status();

        terminal
            .draw(|f| view::render_keyboard(f, &drive, &kb, &session))
            .map_err(|e| format!("draw: {e}"))?;

        if signal::cancel_requested() {
            should_quit = true;
        }
    }
    restore_terminal(&mut terminal);
    Ok(())
}
