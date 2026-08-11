//! `machbus drive joystick` — gamepad input via `gilrs`.
//!
//! Works with any controller the kernel recognises: Xbox, PlayStation,
//! Logitech, 8BitDo, etc. Left stick Y = throttle, left stick X = steer.
//! Triggers = accel/brake. Buttons = hitch/PTO/stop.

use std::time::Instant;

use gilrs::{Axis, Button, Event, EventType, Gilrs};
use machbus::isobus::implement::tractor_commands::{HitchCommand, PtoCommand};
use machbus::session::Session;
use machbus::session::plugins::Implement;
use machbus::session::{Hitch, Pto};

use crate::cli::DriveArgs;
use crate::signal;

use super::view;
use super::{DriveState, restore_terminal, setup_session, setup_terminal, shared_tick};

/// Live gamepad state — what we read from gilrs each tick.
pub struct PadState {
    /// Left stick Y (-1.0 = full down, +1.0 = full up).
    pub lstick_y: f64,
    /// Left stick X (-1.0 = full left, +1.0 = full right).
    pub lstick_x: f64,
    /// Right trigger (0.0 = released, 1.0 = full).
    pub rtrigger: f64,
    /// Left trigger (0.0 = released, 1.0 = full).
    pub ltrigger: f64,
    /// D-pad up held.
    pub dpad_up: bool,
    /// D-pad down held.
    pub dpad_down: bool,
    /// Buttons just pressed this tick (for one-shot actions).
    pub a_pressed: bool,
    pub b_pressed: bool,
    pub x_pressed: bool,
    pub y_pressed: bool,
    pub start_pressed: bool,
    /// Button currently held (for visual).
    pub a_held: bool,
    pub b_held: bool,
    pub x_held: bool,
    pub y_held: bool,
    pub start_held: bool,
    /// Connected gamepad name.
    pub pad_name: String,
}

impl PadState {
    fn new() -> Self {
        Self {
            lstick_y: 0.0,
            lstick_x: 0.0,
            rtrigger: 0.0,
            ltrigger: 0.0,
            dpad_up: false,
            dpad_down: false,
            a_pressed: false,
            b_pressed: false,
            x_pressed: false,
            y_pressed: false,
            start_pressed: false,
            a_held: false,
            b_held: false,
            x_held: false,
            y_held: false,
            start_held: false,
            pad_name: "(no pad)".into(),
        }
    }

    fn reset_edge(&mut self) {
        self.a_pressed = false;
        self.b_pressed = false;
        self.x_pressed = false;
        self.y_pressed = false;
        self.start_pressed = false;
    }

    /// Drop every input to its released/centred value.
    ///
    /// A dead-man switch is only a dead-man if losing it reads as *released*.
    /// gilrs sends no button-release events when a pad disconnects, so without
    /// this the last `rtrigger` and stick values persisted: unplugging the pad
    /// (or a flat battery, or a dropped Bluetooth link) while R2 was held left
    /// `deadman` true forever and the machine steering at its last curvature.
    fn release_all(&mut self) {
        self.lstick_x = 0.0;
        self.lstick_y = 0.0;
        self.rtrigger = 0.0;
        self.ltrigger = 0.0;
        self.dpad_up = false;
        self.dpad_down = false;
        self.a_held = false;
        self.b_held = false;
        self.x_held = false;
        self.y_held = false;
        self.start_held = false;
    }
}

/// Read all pending gilrs events, update pad state.
/// Returns `true` if the active pad went away this poll, so the caller can
/// disarm — losing the controller is losing the dead-man.
fn poll_gamepad(
    gilrs: &mut Gilrs,
    pad: &mut PadState,
    active_id: &mut Option<gilrs::GamepadId>,
) -> bool {
    pad.reset_edge();
    let mut disconnected = false;

    // Pick the first connected pad if we don't have one.
    if active_id.is_none()
        && let Some((id, gp)) = gilrs.gamepads().next()
    {
        *active_id = Some(id);
        pad.pad_name = gp.name().to_string();
    }

    while let Some(Event { id, event, .. }) = gilrs.next_event() {
        if Some(id) != *active_id {
            continue;
        }
        match event {
            EventType::AxisChanged(axis, value, _) => {
                let v = value as f64;
                match axis {
                    Axis::LeftStickY => pad.lstick_y = deadzone(v),
                    Axis::LeftStickX => pad.lstick_x = deadzone(v),
                    _ => {}
                }
            }
            EventType::ButtonPressed(button, _) | EventType::ButtonReleased(button, _) => {
                let pressed = matches!(event, EventType::ButtonPressed(_, _));
                match button {
                    Button::LeftTrigger | Button::LeftTrigger2 => {
                        pad.ltrigger = if pressed { 1.0 } else { 0.0 };
                    }
                    Button::RightTrigger | Button::RightTrigger2 => {
                        pad.rtrigger = if pressed { 1.0 } else { 0.0 };
                    }
                    Button::DPadUp => pad.dpad_up = pressed,
                    Button::DPadDown => pad.dpad_down = pressed,
                    Button::South => {
                        pad.a_held = pressed;
                        if pressed {
                            pad.a_pressed = true;
                        }
                    }
                    Button::East => {
                        pad.b_held = pressed;
                        if pressed {
                            pad.b_pressed = true;
                        }
                    }
                    Button::West => {
                        pad.x_held = pressed;
                        if pressed {
                            pad.x_pressed = true;
                        }
                    }
                    Button::North => {
                        pad.y_held = pressed;
                        if pressed {
                            pad.y_pressed = true;
                        }
                    }
                    Button::Start => {
                        pad.start_held = pressed;
                        if pressed {
                            pad.start_pressed = true;
                        }
                    }
                    _ => {}
                }
            }
            EventType::Connected => {
                pad.pad_name = gilrs.gamepad(id).name().to_string();
                *active_id = Some(id);
            }
            EventType::Disconnected => {
                pad.pad_name = "(disconnected)".into();
                pad.release_all();
                *active_id = None;
                disconnected = true;
            }
            _ => {}
        }
    }

    // Belt and braces: an unplug that produced no event still has to read as
    // released, so check liveness rather than trusting the event stream.
    if let Some(id) = *active_id
        && !gilrs.gamepad(id).is_connected()
    {
        pad.pad_name = "(disconnected)".into();
        pad.release_all();
        *active_id = None;
        disconnected = true;
    }

    disconnected
}

/// Ignore tiny stick movements near centre.
fn deadzone(v: f64) -> f64 {
    if v.abs() < 0.08 { 0.0 } else { v }
}

/// Handle one-shot button presses (actions that fire once per press).
fn handle_buttons(pad: &PadState, drive: &mut DriveState, session: &mut Session) {
    // A / Cross = emergency stop + disarm. Zeroes motion and drops the arm
    // latch; you must release R2 and hold it again for 1.5 s to re-arm.
    if pad.a_pressed {
        drive.speed = 0.0;
        drive.steer = 0.0;
        drive.disarm();
    }
    // B / Circle = hitch raise.
    if pad.b_pressed
        && let Some(imp) = session.get_mut::<Implement>()
    {
        imp.command_hitch(Hitch::Rear, HitchCommand::Raise);
    }
    // X / Square = hitch lower.
    if pad.x_pressed
        && let Some(imp) = session.get_mut::<Implement>()
    {
        imp.command_hitch(Hitch::Rear, HitchCommand::Lower);
    }
    // Y / Triangle = PTO engage.
    if pad.y_pressed
        && let Some(imp) = session.get_mut::<Implement>()
    {
        imp.command_pto(Pto::Rear, PtoCommand::Engage);
    }
    // Start = cycle counter multiplier.
    if pad.start_pressed {
        drive.counter_mult = (drive.counter_mult % 4) + 1;
    }
    // D-pad up = speed limit +. Held every frame, so scale down 10× to tame sensitivity.
    if pad.dpad_up {
        drive.speed_limit += drive.speed_step * 0.1;
    }
    // D-pad down = speed limit −.
    if pad.dpad_down {
        drive.speed_limit = (drive.speed_limit - drive.speed_step * 0.1).max(0.0);
    }
}

pub fn run(args: DriveArgs) -> Result<(), String> {
    if args.daemon {
        return run_daemon(args);
    }
    signal::install_cancel_handler();
    let (mut session, bus, mut drive) = setup_session(&args)?;
    let mut terminal = setup_terminal()?;

    let mut gilrs = Gilrs::new().map_err(|e| format!("gilrs: {e}"))?;
    let mut pad = PadState::new();
    let mut active_id: Option<gilrs::GamepadId> = None;

    let start = Instant::now();
    let mut last = start;
    let mut should_quit = false;

    while !should_quit {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(0.1);
        last = now;

        // 1. Poll gamepad events. Losing the pad is losing the dead-man, so
        //    it disarms: the operator must reconnect, release R2 and hold it
        //    again for ARM_HOLD_SECS.
        if poll_gamepad(&mut gilrs, &mut pad, &mut active_id) {
            drive.speed = 0.0;
            drive.steer = 0.0;
            drive.disarm();
        }

        // Read live axis state directly.
        if let Some(id) = active_id {
            let gp = gilrs.gamepad(id);
            pad.lstick_y = deadzone(gp.value(Axis::LeftStickY) as f64);
            pad.lstick_x = deadzone(gp.value(Axis::LeftStickX) as f64);
            // Analog triggers via button_data.
            pad.rtrigger = gp
                .button_data(Button::RightTrigger)
                .map_or(0.0, |d| d.value() as f64);
            pad.ltrigger = gp
                .button_data(Button::LeftTrigger)
                .map_or(0.0, |d| d.value() as f64);
        }

        // 2. Handle one-shot buttons.
        handle_buttons(&pad, &mut drive, &mut session);

        // 3. R2 (right trigger) is the dead-man switch. It must first be held
        //    for ARM_HOLD_SECS to arm; after that it engages autosteer and
        //    enables throttle only while held. Nothing moves until armed+held.
        let deadman = pad.rtrigger > 0.3;
        let active = drive.update_arm(deadman, dt);
        let throttle = if active {
            pad.lstick_y // +1 = full forward, -1 = full reverse
        } else {
            0.0 // not armed / dead-man released: no throttle
        };
        // Command "intend to steer" only while armed and held.
        drive.engaged = active;

        // 4. Apply analog physics.
        drive.apply_analog(throttle, pad.lstick_x, dt);

        // 5. Shared bus + session tick.
        shared_tick(&mut session, &bus, &mut drive, start);

        // 6. Render.
        terminal
            .draw(|f| view::render_joystick(f, &drive, &pad, &session))
            .map_err(|e| format!("draw: {e}"))?;

        // 7. Check keyboard quit (q / Ctrl+C).
        if crossterm::event::poll(std::time::Duration::ZERO).unwrap_or(false)
            && let Ok(crossterm::event::Event::Key(k)) = crossterm::event::read()
            && k.kind == crossterm::event::KeyEventKind::Press
            && (k.code == crossterm::event::KeyCode::Char('q')
                || (k.code == crossterm::event::KeyCode::Char('c')
                    && k.modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)))
        {
            should_quit = true;
        }

        if signal::cancel_requested() {
            should_quit = true;
        }
    }

    restore_terminal(&mut terminal);
    Ok(())
}

/// Headless daemon mode — no TUI, just gamepad + CAN.
fn run_daemon(args: DriveArgs) -> Result<(), String> {
    signal::install_cancel_handler();
    let (mut session, bus, mut drive) = setup_session(&args)?;

    let mut gilrs = Gilrs::new().map_err(|e| format!("gilrs: {e}"))?;
    let mut pad = PadState::new();
    let mut active_id: Option<gilrs::GamepadId> = None;

    let start = Instant::now();
    let mut last = start;
    let mut should_quit = false;

    println!("machbus drive joystick --daemon  (Ctrl+C to quit)");

    while !should_quit {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(0.1);
        last = now;

        // Same rule headless: losing the pad disarms.
        if poll_gamepad(&mut gilrs, &mut pad, &mut active_id) {
            drive.speed = 0.0;
            drive.steer = 0.0;
            drive.disarm();
        }
        if let Some(id) = active_id {
            let gp = gilrs.gamepad(id);
            pad.lstick_y = deadzone(gp.value(Axis::LeftStickY) as f64);
            pad.lstick_x = deadzone(gp.value(Axis::LeftStickX) as f64);
            pad.rtrigger = gp
                .button_data(Button::RightTrigger)
                .map_or(0.0, |d| d.value() as f64);
            pad.ltrigger = gp
                .button_data(Button::LeftTrigger)
                .map_or(0.0, |d| d.value() as f64);
        }

        handle_buttons(&pad, &mut drive, &mut session);

        let deadman = pad.rtrigger > 0.3;
        let active = drive.update_arm(deadman, dt);
        let throttle = if active { pad.lstick_y } else { 0.0 };
        drive.engaged = active;
        drive.apply_analog(throttle, pad.lstick_x, dt);
        shared_tick(&mut session, &bus, &mut drive, start);

        if start.elapsed().as_millis() % 500 < 3 {
            drive.update_status();
            println!("\r{}", drive.status);
        }

        if signal::cancel_requested() {
            should_quit = true;
        }
    }
    Ok(())
}
