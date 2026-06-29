//! High-level **autosteer**: command a tractor's steering by curvature.
//!
//! ISOBUS automatic guidance is curvature-based — you send a desired path
//! **curvature** (1/km), not waypoints and not a steering angle. The tractor's
//! steering ECU closes the loop on the wheels. This demo plugs the high-level
//! [`Guidance`] plugin, claims an address, and commands a turn; the resulting
//! Guidance System Command (PGN 0xAD00) shows up on `poll_transmit`.
//!
//! Run with `cargo run --example guidance_autosteer`.

use machbus::Instant;
use machbus::net::pgn_defs::PGN_GUIDANCE_SYSTEM_CMD;
use machbus::net::{Name, Result};
use machbus::session::Session;
use machbus::session::plugins::Guidance;

fn make_name(identity: u32) -> Name {
    Name::default()
        .with_identity_number(identity)
        .with_function_code(0x80)
        .with_self_configurable(true)
}

fn main() -> Result<()> {
    println!("=== High-level autosteer (machbus::session::plugins::Guidance) ===\n");

    // ANCHOR: build
    // A guidance-controller node. The session core is sans-IO; we drive it.
    let mut session = Session::builder(make_name(0x100), 0x80)
        .plug(Guidance::new())
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
    // Engage (assert "intend to steer"), then command a 50 m-radius turn
    // (curvature = 1000/50 = 20/km). Without engaging, the command is sent with
    // status "not intended to steer" and a conformant steering ECU will not move.
    // `command_curvature(0.0)` is the same as `command_straight`.
    {
        let g = session.get_mut::<Guidance>().expect("guidance plugged");
        g.engage();
        g.command_radius(50.0);
    }

    now = now.add_millis(50);
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
    // A real steering ECU would broadcast Guidance Machine Info (PGN 0xAC00);
    // the plugin decodes it and exposes the tractor's view:
    let g = session.get::<Guidance>().expect("guidance plugged");
    println!(
        "steering ready: {}   estimated curvature: {:?} (1/km)",
        g.is_steering_ready(),
        g.estimated_curvature()
    );
    // ANCHOR_END: feedback

    Ok(())
}
