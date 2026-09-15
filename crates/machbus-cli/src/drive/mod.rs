//! `machbus drive` — ISOBUS AutoDrive (steering + speed) + telemetry TUI.
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
use machbus::session::plugins::{AutoDrive, Gnss, Implement, ShortcutButton};
use machbus::session::{AutodriveRefusal, AutomationStatus, DriveCommand, SafeStopTrigger};
use machbus::time::Instant as MbInstant;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::bus::Bus;
use crate::cli::DriveArgs;

// Physics rates (per second, proportional to setpoint).
const R_WITH: f64 = 0.10;
const R_DRAG: f64 = 0.05;

/// Seconds the joystick dead-man (R2) must be held continuously to arm
/// autosteer commanding. Until armed, R2 does nothing — no throttle, no steer.
pub const ARM_HOLD_SECS: f64 = 1.5;

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
    /// Whether we want the steering ECU to actually steer to our commands
    /// (asserts the Curvature Command Status "intend to steer" on PGN 0xAD00).
    /// Driven by the dead-man switch (joystick R2) / engage toggle (keyboard).
    pub engaged: bool,
    /// Safety latch: autosteer commanding is inert until the operator arms it
    /// (holds R2 for [`ARM_HOLD_SECS`]). Prevents instant steer-on at startup.
    pub armed: bool,
    /// Arming hold progress, 0.0..1.0, while R2 is held pre-arm (for the UI).
    pub arm_progress: f64,
    /// After a disarm, block re-arming until the dead-man is fully released, so
    /// hitting stop while still holding R2 cannot silently re-arm.
    pub arm_block: bool,
    /// The last refusal from `arm`/`engage`/`command`, so a rejected request is
    /// visible instead of looking like nothing happened.
    pub refusal: Option<AutodriveRefusal>,
    /// Why AutoDrive latched a safe stop, if it did. Latching: it stays until
    /// the operator clears it.
    pub stop_reason: Option<SafeStopTrigger>,
    /// The ISO 11783-7 Table 45 automation status the plugin is reporting,
    /// including the limit states the steering ECU feeds back.
    pub automation: AutomationStatus,
    /// Operator asked to release a latched safe stop. Routed as a request
    /// because the input handlers have no session; `flush` performs it.
    ///
    /// Deliberately its own key rather than folded into engage: clearing a
    /// fault is not by itself consent to move, and `AutoDrive::clear_stop`
    /// refuses anyway while the shortcut button is held or a GNSS hazard is
    /// live.
    pub clear_requested: bool,
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
            engaged: false,
            armed: false,
            arm_progress: 0.0,
            arm_block: false,
            refusal: None,
            stop_reason: None,
            automation: AutomationStatus::NotReady,
            clear_requested: false,
        }
    }

    /// Disarm autosteer: stop commanding, drop the arm latch, and require the
    /// dead-man to be released before it can be re-armed.
    pub fn disarm(&mut self) {
        self.engaged = false;
        self.armed = false;
        self.arm_progress = 0.0;
        self.arm_block = true;
    }

    /// Advance the arming latch from the dead-man state and return whether
    /// autosteer may command this tick (armed **and** dead-man held). Holding
    /// the dead-man for [`ARM_HOLD_SECS`] arms; releasing it before that resets
    /// the hold. Once armed, it stays armed until [`disarm`](Self::disarm).
    pub fn update_arm(&mut self, deadman: bool, dt: f64) -> bool {
        // After a disarm, ignore the dead-man until it is fully released once.
        if self.arm_block {
            self.arm_progress = 0.0;
            if !deadman {
                self.arm_block = false;
            }
            return false;
        }
        if !self.armed {
            if deadman {
                self.arm_progress = (self.arm_progress + dt / ARM_HOLD_SECS).min(1.0);
                if self.arm_progress >= 1.0 {
                    self.armed = true;
                    // Completing the hold is the joystick's explicit clear
                    // gesture: the operator released the dead-man and held it
                    // again for ARM_HOLD_SECS. Without this a single safe stop
                    // would end the session, because AutoDrive latches and the
                    // pad has no spare button. `clear_stop` still refuses while
                    // the shortcut button is held or a GNSS hazard is live, so
                    // this cannot re-arm against a live condition.
                    self.clear_requested = true;
                }
            } else {
                self.arm_progress = 0.0;
            }
        }
        self.armed && deadman
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
        let Some(d) = session.get_mut::<AutoDrive>() else {
            return;
        };

        if core::mem::take(&mut self.clear_requested) {
            self.refusal = d.clear_stop().err();
        }

        // Match the plugin's engage state to our desired (dead-man) state so
        // commands carry "intend to steer" only while engaged. Transition only
        // on change to avoid re-queueing an extra command every tick.
        //
        // `arm()` is AutoDrive's "preconditions met, not yet asking for the
        // wheel" step; it has to succeed before `engage()` will. Both report
        // the first unmet precondition, which is what we surface to the
        // operator instead of failing silently.
        if self.engaged && !d.is_engaged() {
            self.refusal = d.arm().and_then(|()| d.engage()).err();
        } else if !self.engaged && d.is_engaged() {
            d.disengage(SafeStopTrigger::OperatorOverride);
            self.refusal = None;
        }

        // The command is a heartbeat: AutoDrive stops on `CommandStale` if the
        // setpoint is not refreshed within 300 ms, so this runs every tick even
        // when nothing changed. Curvature is already in km⁻¹ here, so it goes
        // straight through — no twist round-trip.
        let cmd = DriveCommand {
            speed_mps: Some(self.speed),
            curvature_km_inv: Some(self.curvature()),
        };
        if let Err(refusal) = d.command(cmd) {
            // A refused command while engaged is worth showing; while
            // disengaged `StatusNotActive` is the normal resting state.
            if self.engaged {
                self.refusal = Some(refusal);
            }
        } else if self.engaged {
            self.refusal = None;
        }

        self.stop_reason = d.stop_reason();
        self.automation = d.status();
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
        .plug(AutoDrive::new())
        .plug(ShortcutButton::new())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The session builder refuses two authors of one command PGN, and it does
    /// so at **runtime**, not compile time — a bad plugin set would launch and
    /// then fail. Build the real set and assert it assembles.
    #[test]
    fn the_drive_plugin_set_is_a_legal_session() {
        let args = DriveArgs {
            iface: "vcan0".into(),
            addr: "80".into(),
            default_speed: 0.0,
            speed_step: 0.1,
            max_curvature: 40.0,
            daemon: false,
        };
        let name = Name::default()
            .with_self_configurable(true)
            .with_function_code(0x80)
            .with_identity_number(0x0042);
        let session = Session::builder(parse_addr(&args.addr).map(|a| (name, a)).unwrap().0, 0x80)
            .plug(AutoDrive::new())
            .plug(ShortcutButton::new())
            .plug(Implement::new())
            .plug(Gnss::new(
                machbus::nmea::NMEAConfig::default().with_all(true),
            ))
            .build();
        assert!(
            session.is_ok(),
            "drive's plugin set must assemble: {:?}",
            session.err()
        );
    }

    /// A dead-man switch is only a dead-man if losing it reads as *released*.
    /// The joystick path disarms when the pad disconnects; this pins the latch
    /// behaviour that makes re-arming deliberate afterwards.
    #[test]
    fn losing_the_deadman_disarms_and_blocks_silent_rearm() {
        let args = DriveArgs {
            iface: "vcan0".into(),
            addr: "80".into(),
            default_speed: 2.0,
            speed_step: 0.1,
            max_curvature: 40.0,
            daemon: false,
        };
        let mut d = DriveState::new(&args);

        // Hold the dead-man for the full arm period: armed and commanding.
        assert!(!d.update_arm(true, 1.0));
        assert!(d.update_arm(true, 1.0), "held past ARM_HOLD_SECS -> active");
        assert!(d.armed);

        // The pad vanishes. The loop zeroes motion and disarms.
        d.speed = 0.0;
        d.steer = 0.0;
        d.disarm();
        assert!(!d.armed);
        assert!(!d.engaged);

        // A pad that reconnects still asserting R2 must NOT silently re-arm:
        // `arm_block` holds until it is seen fully released once.
        assert!(!d.update_arm(true, 10.0), "must not re-arm while still held");
        assert!(!d.armed);

        // Release, then hold again for the full period.
        assert!(!d.update_arm(false, 0.1));
        assert!(!d.update_arm(true, 1.0));
        assert!(d.update_arm(true, 1.0), "deliberate re-hold re-arms");
    }

    /// The keyboard dead-man is SPACE *held*: the terminal's auto-repeat keeps
    /// the window alive, and letting go disengages. It used to be a toggle with
    /// `armed` forced true at startup, so one press engaged and it stayed
    /// engaged with nothing held at all.
    #[test]
    fn the_keyboard_deadman_must_be_held() {
        use crate::drive::keyboard::{DEADMAN_WINDOW_S, KeyboardState};

        let mut kb = KeyboardState::new();
        // Nothing pressed: the dead-man reads released.
        assert!(!kb.kspace.held_within(DEADMAN_WINDOW_S));

        kb.press_space_for_test();
        assert!(kb.kspace.held_within(DEADMAN_WINDOW_S), "a press holds it");

        // Auto-repeat inside the window keeps it alive.
        kb.tick_for_test(DEADMAN_WINDOW_S * 0.5);
        assert!(kb.kspace.held_within(DEADMAN_WINDOW_S));
        kb.press_space_for_test();
        kb.tick_for_test(DEADMAN_WINDOW_S * 0.5);
        assert!(kb.kspace.held_within(DEADMAN_WINDOW_S));

        // Stop repeating and it goes released — this is the whole point.
        kb.tick_for_test(DEADMAN_WINDOW_S * 1.5);
        assert!(
            !kb.kspace.held_within(DEADMAN_WINDOW_S),
            "releasing SPACE must read as a released dead-man"
        );
    }

    /// The drive TUI cannot be launched here — it needs a tty — so render it
    /// headlessly instead. This is the only automated check that the panes fit
    /// and the widgets are wired: `draw_telemetry` grew a seventh line for the
    /// automation status and both layouts had to grow with it, which a compile
    /// cannot catch.
    #[test]
    fn both_drive_tuis_render_headlessly() {
        use machbus::session::{AutodriveRefusal, AutomationStatus, SafeStopTrigger};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let args = DriveArgs {
            iface: "vcan0".into(),
            addr: "80".into(),
            default_speed: 2.0,
            speed_step: 0.1,
            max_curvature: 40.0,
            daemon: false,
        };
        let name = Name::default()
            .with_self_configurable(true)
            .with_function_code(0x80)
            .with_identity_number(0x42);
        let session = Session::builder(name, 0x80)
            .plug(AutoDrive::new())
            .plug(ShortcutButton::new())
            .plug(Implement::new())
            .plug(Gnss::new(
                machbus::nmea::NMEAConfig::default().with_all(true),
            ))
            .build()
            .expect("the drive plugin set assembles");

        // Every interesting state, including the ones only reachable after a
        // fault: a latched stop and a refused request both have to be legible.
        let states = [
            (AutomationStatus::NotReady, None, None, false, false),
            (AutomationStatus::ReadyToEnable, None, None, true, false),
            (
                AutomationStatus::ActiveLimitedHigh,
                None,
                Some(AutodriveRefusal::StopConditionLive),
                true,
                true,
            ),
            (
                AutomationStatus::Fault,
                Some(SafeStopTrigger::PositionStale),
                Some(AutodriveRefusal::LinkDown),
                false,
                false,
            ),
        ];

        // Hostile sizes on purpose. The keyboard pane draws a fixed 8-row key
        // grid and the gamepad pane fixed stick/trigger art, neither of which
        // shrinks — so anything short or narrow is where a missing bounds check
        // shows up. 80x24 is the classic default; the rest bracket it.
        for (w, h) in [
            (80u16, 24u16),
            (60, 20),
            (40, 12),
            (20, 8),
            (110, 32),
            (200, 60),
        ] {
            for (automation, stop, refusal, armed, engaged) in states {
                let mut state = DriveState::new(&args);
                state.claimed = true;
                state.claimed_addr = 0x80;
                state.speed = 1.5;
                state.steer = 0.4;
                state.automation = automation;
                state.stop_reason = stop;
                state.refusal = refusal;
                state.armed = armed;
                state.engaged = engaged;
                state.arm_progress = 0.5;

                let kb = keyboard::KeyboardState::new();
                let pad = joystick::PadState::new();
                let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                term.draw(|f| view::render_keyboard(f, &state, &kb, &session))
                    .unwrap_or_else(|e| panic!("keyboard TUI at {w}x{h}: {e}"));
                term.draw(|f| view::render_joystick(f, &state, &pad, &session))
                    .unwrap_or_else(|e| panic!("joystick TUI at {w}x{h}: {e}"));
            }
        }
    }

    /// The keyboard TUI cannot be launched here — it needs a CAN interface this
    /// environment cannot create — so drive the real key handler and the real
    /// loop arithmetic instead. This covers what a manual run would: that a key
    /// press moves the machine, that SPACE gates it, and that stop latches.
    #[test]
    fn the_keyboard_drives_the_machine_end_to_end() {
        let args = DriveArgs {
            iface: "vcan0".into(),
            addr: "80".into(),
            default_speed: 2.0,
            speed_step: 0.1,
            max_curvature: 40.0,
            daemon: false,
        };
        let name = Name::default()
            .with_self_configurable(true)
            .with_function_code(0x80)
            .with_identity_number(0x42);
        let mut session = Session::builder(name, 0x80)
            .plug(AutoDrive::new())
            .plug(ShortcutButton::new())
            .plug(Implement::new())
            .plug(Gnss::new(
                machbus::nmea::NMEAConfig::default().with_all(true),
            ))
            .build()
            .expect("plugin set assembles");

        let mut drive = DriveState::new(&args);
        let mut kb = keyboard::KeyboardState::new();
        let dt = 1.0 / 30.0; // a 30 Hz loop, as the real one runs

        // One SPACE press is not consent: the arm latch needs a continuous hold.
        kb.press_for_test(' ', &mut drive, &mut session);
        let active = drive.update_arm(kb.kspace.held_within(keyboard::DEADMAN_WINDOW_S), dt);
        assert!(!active, "a single press must not arm");
        assert!(!drive.armed);

        // Hold it, as the terminal's auto-repeat does, past ARM_HOLD_SECS.
        // Iteration counts are fixed rather than derived from the constants:
        // deriving them means a mutated constant spins the test instead of
        // failing it, which is a hang, not a result.
        let mut active = false;
        for _ in 0..60 {
            kb.press_for_test(' ', &mut drive, &mut session);
            kb.tick_for_test(dt);
            active = drive.update_arm(kb.kspace.held_within(keyboard::DEADMAN_WINDOW_S), dt);
        }
        assert!(drive.armed, "a continuous hold arms");
        assert!(active, "armed and held means commanding");
        drive.engaged = active;

        // W accelerates. The physics is the same call the run loop makes.
        for _ in 0..30 {
            kb.press_for_test('w', &mut drive, &mut session);
            kb.tick_for_test(dt);
            kb.apply_physics_for_test(&mut drive, dt);
        }
        assert!(drive.speed > 0.0, "W must accelerate, got {}", drive.speed);

        // D steers right.
        for _ in 0..30 {
            kb.press_for_test('d', &mut drive, &mut session);
            kb.tick_for_test(dt);
            kb.apply_physics_for_test(&mut drive, dt);
        }
        assert!(drive.steer > 0.0, "D must steer right, got {}", drive.steer);
        assert!(drive.curvature() > 0.0);

        // Let go of SPACE. Past the window the dead-man reads released. Two
        // seconds of silence is well beyond DEADMAN_WINDOW_S (0.9 s).
        for _ in 0..60 {
            kb.tick_for_test(dt);
        }
        let active = drive.update_arm(kb.kspace.held_within(keyboard::DEADMAN_WINDOW_S), dt);
        assert!(!active, "releasing SPACE stops commanding");

        // ENTER is the emergency stop: zeroes motion and disarms.
        kb.press_for_test('\n', &mut drive, &mut session);
        assert_eq!(drive.speed, 0.0);
        assert_eq!(drive.steer, 0.0);
        assert!(!drive.armed, "stop disarms");
        assert!(!drive.engaged);

        // And C asks to clear a latched stop.
        kb.press_for_test('c', &mut drive, &mut session);
        assert!(drive.clear_requested, "C requests the latch release");
    }
}
