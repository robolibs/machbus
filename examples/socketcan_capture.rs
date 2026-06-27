//! SocketCAN/vcan capture: record live frames off a CAN interface into a
//! `candump` evidence file using `machbus::net::CaptureRecorder`.
//!
//! This is the capture side of the hardware-evidence harness. It needs a
//! Linux SocketCAN interface; for a non-hardware loopback create `vcan0`:
//!
//! ```text
//! sudo modprobe vcan
//! sudo ip link add dev vcan0 type vcan
//! sudo ip link set vcan0 up
//! ```
//!
//! ```text
//! MACHBUS_SOCKETCAN_IFACE=vcan0 MACHBUS_CAPTURE_OUT=cap.candump \
//!   cargo run --features wirebit --example socketcan_capture
//! ```
//!
//! It transmits a few ISOBUS-shaped frames on the bus and captures them
//! back on a second socket, proving the recorder works end-to-end against a
//! real interface, then writes + prints the `candump` capture.

#[cfg(all(feature = "wirebit", target_os = "linux"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Instant;

    use machbus::net::CaptureRecorder;
    use wirebit::can::{CanConfig, CanEndpoint, CanFrame, SocketCanConfig, SocketCanLink};

    let iface = std::env::var("MACHBUS_SOCKETCAN_IFACE").unwrap_or_else(|_| "vcan0".to_string());
    let out = std::env::var("MACHBUS_CAPTURE_OUT")
        .unwrap_or_else(|_| "/tmp/machbus_vcan_capture.candump".to_string());

    let mk = |listen_only: bool| -> Result<CanEndpoint<SocketCanLink>, Box<dyn std::error::Error>> {
        let link = SocketCanLink::create(SocketCanConfig {
            interface_name: iface.clone(),
            create_if_missing: false,
            destroy_on_close: false,
        })?;
        Ok(CanEndpoint::new(
            link,
            CanConfig {
                bitrate: 250_000,
                loopback: false,
                listen_only,
                rx_buffer_size: 256,
            },
            0,
        ))
    };

    // Listen-only mode (MACHBUS_CAPTURE_LISTEN=1): capture whatever another
    // emitter (e.g. socketcan_address_claim) puts on the bus, recording
    // `MACHBUS_CAPTURE_COUNT` frames. Otherwise: self-test by transmitting a
    // few ISOBUS-shaped frames and capturing them on a second socket.
    let listen = std::env::var("MACHBUS_CAPTURE_LISTEN").is_ok_and(|v| v == "1");
    let count: usize = std::env::var("MACHBUS_CAPTURE_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    let mut rx = mk(true)?;
    if !listen {
        let mut tx = mk(false)?;
        for f in [
            CanFrame::make_ext(
                0x18EE_FF80,
                &[0x00, 0x80, 0x83, 0x01, 0x00, 0x82, 0x00, 0x20],
            ),
            CanFrame::make_ext(
                0x18FE_CA80,
                &[0x01, 0xFF, 0x64, 0x00, 0x01, 0x01, 0xFF, 0xFF],
            ),
            CanFrame::make_ext(
                0x18FE_E680,
                &[0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78],
            ),
        ] {
            tx.send_can(&f)?;
        }
    }

    let seconds: u64 = std::env::var("MACHBUS_CAPTURE_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15);
    let start = Instant::now();
    let deadline = start + std::time::Duration::from_secs(seconds);
    let mut recorder = CaptureRecorder::new(&iface);
    while recorder.len() < count && Instant::now() < deadline {
        match rx.recv_can() {
            Ok(f) => {
                let ts_us = start.elapsed().as_micros() as u64;
                let len = f.can_dlc as usize;
                recorder.record(ts_us, f.id(), f.data[..len].to_vec());
            }
            // In listen mode a receive timeout just means "nothing yet" —
            // keep listening until the deadline. In self-test mode the
            // frames are already buffered, so surface a real error.
            Err(_) if listen => continue,
            Err(e) => return Err(e.into()),
        }
    }

    recorder.save(&out)?;
    println!("captured {} frames from {iface} -> {out}", recorder.len());
    print!("{}", recorder.to_candump());
    Ok(())
}

#[cfg(not(all(feature = "wirebit", target_os = "linux")))]
fn main() {
    eprintln!(
        "socketcan_capture requires Linux and `cargo run --features wirebit --example socketcan_capture`"
    );
}
