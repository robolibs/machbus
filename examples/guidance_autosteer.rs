//! **Automatic guidance**: command a tractor's steering by curvature.
//!
//! ISOBUS guidance is curvature-based — you send a desired path **curvature**
//! (1/km), not waypoints and not a steering angle. The tractor's steering ECU
//! closes the loop on the wheels. This demo plugs [`AutoDrive`], claims an
//! address, commands a 50 m-radius turn, and reads back what the machine says
//! it is actually producing.
//!
//! The focus here is the **curvature conversation**: converting a radius to the
//! wire unit, commanding steering without touching speed, and comparing the
//! command against the feedback. For the full driving lifecycle — arm, engage,
//! the command heartbeat, the dead-man and a latched stop — see
//! `autodrive_keyboard`.
//!
//! Run with `cargo run --example guidance_autosteer`.

use machbus::Instant;
use machbus::geo::guidance::curvature_per_km_from_radius;
use machbus::isobus::implement::guidance::{
    GenericSaeBs02SlotValue, GuidanceLimitStatus, GuidanceMachineInfo, MechanicalLockout,
};
use machbus::isobus::implement::{RequestResetCommandStatus, Signal};
use machbus::net::pgn_defs::{PGN_GUIDANCE_MACHINE_INFO, PGN_GUIDANCE_SYSTEM_CMD};
use machbus::net::{BROADCAST_ADDRESS, Frame, Identifier, Name, Priority, Result};
use machbus::session::plugins::AutoDrive;
use machbus::session::{DriveCommand, Session};

/// What a steering ECU broadcasts (PGN 0xAC00). `AutoDrive` will not engage
/// until one is answering, and it re-checks the machine's own report — lockout
/// clear, operator engage switch active — on every one of these.
///
/// `estimated` is what the wheels are really producing, which is deliberately
/// not the same as what we command below.
fn machine_info_frame(estimated: f64) -> Frame {
    let info = GuidanceMachineInfo {
        estimated_curvature: Signal::Value(estimated),
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
    println!("=== Automatic guidance by curvature (session::plugins::AutoDrive) ===\n");

    // ANCHOR: build
    // A guidance-controller node. The session core is sans-IO; we drive it.
    let name = Name::default()
        .with_identity_number(0x100)
        .with_function_code(0x80)
        .with_self_configurable(true);
    let mut session = Session::builder(name, 0x80)
        .plug(AutoDrive::new())
        .build()?;
    session.start()?;

    // Drive the address claim (no contention → claims by advancing time).
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

    // ANCHOR: command
    // Curvature is the inverse of the turn radius, in 1/km: a 50 m radius is
    // 1000/50 = 20 1/km. `geo::guidance` does the conversion so the magic
    // number stays out of application code.
    let curvature = curvature_per_km_from_radius(50.0);
    println!("50 m radius  ->  {curvature:.1} 1/km");

    // The steering ECU has to be answering before autonomy may engage.
    session.feed(0, &machine_info_frame(0.0), now);
    now = now.add_millis(50);
    session.tick(now);

    {
        let d = session.get_mut::<AutoDrive>().expect("autodrive plugged");
        // Arm, then engage. Both return the first unmet precondition — a dead
        // link, a mechanical lockout, an inactive operator switch, a latched
        // stop — rather than being silently ignored.
        match d.arm().and_then(|()| d.engage()) {
            Ok(()) => {
                // `DriveCommand::steer` commands curvature and leaves speed to
                // whoever already owns it. Use the two-field form when this node
                // should own both axes.
                if let Err(refusal) = d.command(DriveCommand::steer(curvature)) {
                    println!("[autodrive] command refused: {}", refusal.as_str());
                }
            }
            Err(refusal) => println!("[autodrive] engage refused: {}", refusal.as_str()),
        }
    }

    now = now.add_millis(100);
    session.tick(now); // flushes the queued command to the transmit buffer

    while let Some((port, frame)) = session.poll_transmit() {
        if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD {
            println!(
                "TX port{port}  Guidance System Command (PGN 0x{:04X})  data={:02X?}",
                frame.id.pgn(),
                frame.data
            );
        }
    }
    // ANCHOR_END: command

    // ANCHOR: feedback
    // You command through 0xAD00 and you verify through 0xAC00. Never assume
    // the machine reached the curvature you asked for — the wheels lag, and the
    // ECU may be at a limit. Here it reports 18.5 against our commanded 20.
    session.feed(0, &machine_info_frame(18.5), now);
    now = now.add_millis(50);
    session.tick(now);

    let d = session.get::<AutoDrive>().expect("autodrive plugged");
    println!(
        "status: {:?}   readiness: {:?}",
        d.status(),
        d.steering_readiness_state()
    );
    match d.estimated_curvature() {
        Signal::Value(estimated) => println!(
            "commanded {curvature:.1} 1/km   estimated {estimated:.1} 1/km   \
             tracking error {:+.1}",
            curvature - estimated
        ),
        // A missing signal is not a zero: an error or an absent reading has to
        // stay distinguishable from "the wheels are straight".
        other => println!("estimated curvature unavailable: {other:?}"),
    }
    // ANCHOR_END: feedback

    Ok(())
}
