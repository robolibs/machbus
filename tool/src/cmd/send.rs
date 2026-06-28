//! `machbus send` — send a single CAN frame.

use std::time::Duration;

use crate::can::{decode_isobus, format_readable, parse_candump_line};
use crate::cli::SendArgs;
use crate::socket;

/// Entry point for `machbus send`.
pub fn run(args: SendArgs) -> Result<(), String> {
    let parsed = parse_candump_line(&args.frame).ok_or_else(|| {
        format!(
            "invalid frame '{}': expected <ID>#<DATA> (e.g. 18FEE680#A43116081C267D78)",
            args.frame
        )
    })?;
    let raw = parsed.to_raw();

    let sock = socket::open(&args.interface).map_err(|e| e.to_string())?;

    // A best-effort short wait so that, on a vcan/loopback setup, a
    // concurrent `machbus dump` is already polling when we transmit.
    sock.send(&raw).map_err(|e| e.to_string())?;

    if args.decode {
        if let Some(info) = decode_isobus(&raw) {
            println!(
                "{}{}",
                format_readable(&raw, &args.interface),
                info.annotate()
            );
        } else {
            println!("{}", format_readable(&raw, &args.interface));
            eprintln!("(not an ISOBUS/J1939 ID — sent as raw CAN)");
        }
    } else {
        println!("{}", format_readable(&raw, &args.interface));
    }

    // Give the kernel a moment to flush the frame out the door before exit.
    std::thread::sleep(Duration::from_millis(1));
    Ok(())
}
