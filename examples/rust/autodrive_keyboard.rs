//! **AutoDrive**: steering and speed behind one engage lifecycle.
//!
//! This is the loop `machbus drive keyboard` runs, with the terminal taken out
//! so it is readable and runnable offline. It shows the four things that are
//! easy to get wrong:
//!
//! 1. **arm → engage**, and what a refusal looks like;
//! 2. the **command heartbeat** — the setpoint must be refreshed or the
//!    controller stops on its own;
//! 3. the **dead-man**: releasing it disengages, and *losing* it must read the
//!    same as releasing it;
//! 4. a **latched** safe stop, and the deliberate clear that releases it.
//!
//! `AutoDrive` needs a steering ECU answering before it will engage, so this
//! example feeds itself Agricultural Guidance Machine Info (PGN 0xAC00) the way
//! a real tractor would. Nothing here is a safety system.
//!
//! Run with `cargo run --example autodrive_keyboard`.

use machbus::Instant;
use machbus::isobus::implement::guidance::{
    GenericSaeBs02SlotValue, GuidanceLimitStatus, GuidanceMachineInfo, MechanicalLockout,
};
use machbus::isobus::implement::{RequestResetCommandStatus, Signal};
use machbus::net::pgn_defs::{
    PGN_GUIDANCE_MACHINE_INFO, PGN_GUIDANCE_SYSTEM_CMD, PGN_MACHINE_SELECTED_SPEED_CMD,
};
use machbus::net::{BROADCAST_ADDRESS, Frame, Identifier, Name, Priority, Result};
use machbus::session::plugins::AutoDrive;
use machbus::session::{DriveCommand, SafeStopTrigger, Session};

/// The steering ECU's broadcast: healthy, unlocked, operator switch armed.
/// Without this `AutoDrive` refuses to engage with `link_down`.
fn machine_info_frame() -> Frame {
    let info = GuidanceMachineInfo {
        estimated_curvature: Signal::Value(0.0),
        lockout: MechanicalLockout::NotActive,
        steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
        steering_input_position_status: GenericSaeBs02SlotValue::EnabledOnActive,
        request_reset_status: RequestResetCommandStatus::ResetNotRequired,
        guidance_limit_status: GuidanceLimitStatus::NotLimited,
        guidance_system_command_exit_reason_code: 0,
        remote_engage_switch_status: GenericSaeBs02SlotValue::EnabledOnActive,
    };
    Frame::new(
        Identifier::encode(
            Priority::Default,
            PGN_GUIDANCE_MACHINE_INFO,
            0x1C,
            BROADCAST_ADDRESS,
        ),
        info.encode(),
        8,
    )
}

fn main() -> Result<()> {
    println!("=== AutoDrive: steering + speed behind one lifecycle ===\n");

    // ANCHOR: build
    // A driving node. `AutoDrive` supersedes `Guidance` and is mutually
    // exclusive with it — both author PGN 0xAD00, and the builder refuses the
    // combination rather than let one overwrite the other's safe stop.
    let name = Name::default()
        .with_identity_number(0x100)
        .with_function_code(0x80)
        .with_self_configurable(true);
    let mut session = Session::builder(name, 0x80)
        .plug(AutoDrive::new())
        .build()?;
    session.start()?;

    let mut now = Instant::ZERO;
    for _ in 0..40 {
        now = now.add_millis(50);
        session.tick(now);
        while session.poll_transmit().is_some() {} // discard claim traffic
        if session.is_claimed() {
            break;
        }
    }
    // ANCHOR_END: build

    // ANCHOR: arm
    // Engaging before a steering ECU answers is refused, and the refusal says
    // which precondition failed — an autonomy client that asks to steer and is
    // silently ignored cannot tell "commanded" from "declined".
    {
        let d = session.get_mut::<AutoDrive>().expect("autodrive plugged");
        match d.arm() {
            Ok(()) => println!("[armed] with no machine info — unexpected"),
            Err(refusal) => println!("[refused] arm before any ECU answered: {}", refusal.as_str()),
        }
    }

    // The tractor starts broadcasting. Now the preconditions are met.
    session.feed(0, &machine_info_frame(), now);
    now = now.add_millis(50);
    session.tick(now);

    {
        let d = session.get_mut::<AutoDrive>().expect("autodrive plugged");
        // Both report the first unmet precondition rather than failing silently.
        d.arm().expect("preconditions met once the ECU answers");
        d.engage().expect("engage follows a successful arm");
        println!("[engaged] status = {:?}", d.status());
    }
    // ANCHOR_END: arm

    // ANCHOR: heartbeat
    // The command is a heartbeat. While engaged the plugin re-transmits every
    // 100 ms even when the setpoint has not changed, because that stream *is*
    // what the steering ECU times out on. The flip side: an application that
    // stops refreshing must not leave the machine steering forever, so
    // `AutoDrive` stops itself after COMMAND_STALE_MS (300 ms) without a fresh
    // command. Refresh every cycle, exactly as the drive tool does.
    let mut steps = 0;
    for _ in 0..10 {
        now = now.add_millis(50);
        session.feed(0, &machine_info_frame(), now); // the ECU keeps answering

        // A real controller computes this from the path and the GNSS pose; see
        // `geo::guidance` for the pure-pursuit maths.
        let cmd = DriveCommand {
            speed_mps: Some(2.0),
            curvature_km_inv: Some(20.0), // a 50 m-radius turn
        };
        if let Some(d) = session.get_mut::<AutoDrive>()
            && let Err(refusal) = d.command(cmd)
        {
            println!("[refused] command: {}", refusal.as_str());
        }

        session.tick(now);
        while let Some((_, frame)) = session.poll_transmit() {
            match frame.id.pgn() {
                PGN_GUIDANCE_SYSTEM_CMD => steps += 1,
                PGN_MACHINE_SELECTED_SPEED_CMD => {}
                _ => {}
            }
        }
    }
    println!("[heartbeat] {steps} guidance commands in 500 ms of driving");
    // ANCHOR_END: heartbeat

    // ANCHOR: deadman
    // The operator releases the dead-man. `disengage` is infallible and
    // idempotent — a disengage must never be refused — and it falls back to
    // `DriveCommand::halt()`: zero speed, zero curvature.
    //
    // Losing the dead-man has to read the same as releasing it. In the drive
    // tool that means a disconnected gamepad, or a SPACE key that stopped
    // repeating, both land here rather than leaving the last setpoint running.
    {
        let d = session.get_mut::<AutoDrive>().expect("autodrive plugged");
        d.disengage(SafeStopTrigger::OperatorOverride);
        println!(
            "[released] status = {:?}   stop = {:?}",
            d.status(),
            d.stop_reason().map(SafeStopTrigger::as_str)
        );
    }
    // ANCHOR_END: deadman

    // ANCHOR: clear
    // The stop is *latching*: re-engaging is refused until it is explicitly
    // cleared. Clearing a fault is not by itself consent to move, so it is a
    // separate deliberate act — the `C` key in the drive tool, or a fresh
    // 1.5 s arm hold on the gamepad.
    {
        let d = session.get_mut::<AutoDrive>().expect("autodrive plugged");
        match d.engage() {
            Ok(()) => println!("[engaged] while latched — unexpected"),
            Err(refusal) => println!("[refused] engage while latched: {}", refusal.as_str()),
        }

        // `clear_stop` itself refuses while the condition is still live, so it
        // cannot re-arm autonomy against a held stop button or a stale fix.
        match d.clear_stop() {
            Ok(()) => println!("[cleared] latch released; re-engage is allowed again"),
            Err(refusal) => println!("[refused] clear_stop: {}", refusal.as_str()),
        }
    }
    // ANCHOR_END: clear

    Ok(())
}
