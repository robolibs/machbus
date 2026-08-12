//! Full autosteer example: connect to a real machine over CAN, claim an
//! address, then steer an **S-shaped path** by streaming curvature commands.
//!
//! This is the whole pipeline end to end:
//!   1. open a SocketCAN interface and wrap it as a session transport,
//!   2. build a `Session` with the `AutoDrive` plugin and `spawn` its `Driver`,
//!   3. drive the ISO 11783-5 address-claim handshake (NAME → address),
//!   4. stream a serpentine "S" as a sweep of curvature commands (PGN 0xAD00),
//!      reading the steering ECU's machine info (PGN 0xAC00) back as it arrives.
//!
//! The "S" is just a sine sweep of curvature: κ(t) = κmax · sin(2π·t/T). Over one
//! period the curvature goes 0 → +κmax → 0 → −κmax → 0 — i.e. the machine bends
//! one way, then the other: an S. Remember autosteer carries the *path shape*
//! (curvature), not waypoints or wheel angles — see the guidance docs.
//!
//! Set up a loopback bus with no hardware:
//! ```text
//! sudo modprobe vcan
//! sudo ip link add dev vcan0 type vcan
//! sudo ip link set vcan0 up
//! ```
//!
//! Run (interface defaults to vcan0):
//! ```text
//! MACHBUS_SOCKETCAN_IFACE=can0 \
//!   cargo run --features wirebit --example guidance_s_path
//! ```

#[cfg(all(feature = "wirebit", target_os = "linux"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::f64::consts::PI;
    use std::thread::sleep;
    use std::time::{Duration, Instant as StdInstant};

    use machbus::Instant;
    use machbus::isobus::implement::Signal;
    use machbus::net::Name;
    use machbus::session::plugins::AutoDrive;
    use machbus::session::{
        DriveCommand, EndpointTransport, Event, GuidanceEvent, SafeStopTrigger, Session,
    };
    use wirebit::can::{CanConfig, CanEndpoint, SocketCanConfig, SocketCanLink};

    // ── Tunables (env-overridable) ───────────────────────────────────────────
    let iface = std::env::var("MACHBUS_SOCKETCAN_IFACE").unwrap_or_else(|_| "vcan0".into());
    let min_radius_m: f64 = env_f64("MACHBUS_S_RADIUS", 50.0); // tightest part of the S
    let period_s: f64 = env_f64("MACHBUS_S_PERIOD", 12.0); // seconds for one full S
    let rate_hz: f64 = env_f64("MACHBUS_S_RATE", 10.0); // command cadence
    let kappa_max = 1000.0 / min_radius_m; // 1/km  (50 m → 20 /km)

    println!("=== Autosteer S-path over {iface} ===");
    println!("min radius {min_radius_m} m (κmax {kappa_max:.1} /km), S period {period_s}s\n");

    // ── 1. Open the CAN interface as a session transport ─────────────────────
    let link = SocketCanLink::create(SocketCanConfig {
        interface_name: iface.clone(),
        create_if_missing: false,
        destroy_on_close: false,
    })?;
    let endpoint = CanEndpoint::new(
        link,
        CanConfig {
            bitrate: 250_000,
            loopback: false,
            listen_only: false,
            rx_buffer_size: 256,
        },
        0,
    );

    // ── 2. Build the guidance-controller session + its driver ────────────────
    let name = Name::default()
        .with_identity_number(0x515) // your unique serial
        .with_manufacturer_code(0)
        .with_function_code(0x80) // generic; pick your real function
        .with_self_configurable(true);
    let (ctrl, mut drv) = Session::builder(name, 0x80)
        .plug(AutoDrive::new())
        .spawn(EndpointTransport::new(0, endpoint))?;
    ctrl.start()?; // begin address claiming

    let wall = StdInstant::now();
    let now = |w: &StdInstant| Instant::ZERO.add_millis(w.elapsed().as_millis() as u64);

    // ── 3. Drive the address-claim handshake ─────────────────────────────────
    print!("claiming address… ");
    for _ in 0..400 {
        while drv.poll_at(now(&wall))?.is_some() {}
        if ctrl.is_claimed() {
            break;
        }
        sleep(Duration::from_millis(10));
    }
    if !ctrl.is_claimed() {
        return Err("address claim did not complete (is anyone else on the bus?)".into());
    }
    println!("claimed 0x{:02X}", ctrl.address());

    // ── 4. Stream the S path: sweep curvature κ(t) = κmax·sin(2π·t/T) ─────────
    // Engage first: assert "intend to steer" so the steering ECU acts on the
    // curvature stream instead of treating each command as advisory.
    // Arm, then engage. Both report the first unmet precondition rather than
    // failing silently — with no steering ECU on the bus this refuses with
    // `link_down`, which is the honest answer.
    if let Some(Err(refusal)) =
        ctrl.with_mut::<AutoDrive, _>(|d| d.arm().and_then(|()| d.engage()))
    {
        println!("engage refused: {} — commands stay advisory", refusal.as_str());
    }
    println!("steering the S (Ctrl-C to stop)…");
    let dt = Duration::from_secs_f64(1.0 / rate_hz);
    let s_start = wall.elapsed().as_secs_f64();
    loop {
        let t = wall.elapsed().as_secs_f64() - s_start;
        if t > period_s {
            break;
        }

        // The whole abstraction: one curvature number for "how hard to bend now".
        let kappa = kappa_max * (2.0 * PI * t / period_s).sin();
        // Steering only: speed stays with whoever already owns it. Refreshing
        // every cycle is required — `AutoDrive` stops itself after 300 ms
        // without a fresh setpoint rather than steering on forever.
        let _ = ctrl.with_mut::<AutoDrive, _>(|d| d.command(DriveCommand::steer(kappa)));

        // Pump the driver (flushes the command, reads inbound frames as events).
        while let Some(event) = drv.poll_at(now(&wall))? {
            if let Event::Guidance(GuidanceEvent::MachineInfo {
                estimated_curvature,
                steering_ready,
                limit_status,
                ..
            }) = event
            {
                println!(
                    "  t={t:4.1}s  cmd κ={kappa:7.2}/km  est κ={est:>9}  \
                     ready={steering_ready}  limit={limit_status}",
                    // Three outcomes, not two: "err" is the steering ECU
                    // declaring a sensor fault, "n/a" is an ECU that simply
                    // does not report it.
                    est = match estimated_curvature {
                        Signal::Value(k) => format!("{k:7.2}/km"),
                        Signal::Error => "err".to_owned(),
                        Signal::NotAvailable => "n/a".to_owned(),
                    }
                );
            }
        }
        // Fine-control read-back is also available any time, without an event:
        //   let est = ctrl.with::<AutoDrive, _>(|d| d.estimated_curvature());
        sleep(dt);
    }

    // ── 5. Settle straight, disengage, and finish ────────────────────────────
    let _ = ctrl.with_mut::<AutoDrive, _>(|d| d.command(DriveCommand::steer(0.0)));
    ctrl.with_mut::<AutoDrive, _>(|d| d.disengage(SafeStopTrigger::OperatorOverride));
    for _ in 0..5 {
        while drv.poll_at(now(&wall))?.is_some() {}
        sleep(Duration::from_millis(20));
    }
    println!(
        "done — commanded straight, address 0x{:02X}",
        ctrl.address()
    );
    Ok(())
}

#[cfg(all(feature = "wirebit", target_os = "linux"))]
fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(not(all(feature = "wirebit", target_os = "linux")))]
fn main() {
    eprintln!(
        "guidance_s_path needs Linux + SocketCAN:\n  \
         cargo run --features wirebit --example guidance_s_path"
    );
}
