//! `machbus gen` — generate (random) CAN traffic.

use std::thread;
use std::time::Duration;

use crate::can::{CAN_MAX_DLEN, RawFrame, decode_isobus, format_readable};
use crate::cli::GenArgs;
use crate::rng::Rng;
use crate::signal;
use crate::socket;

/// Entry point for `machbus gen`.
pub fn run(args: GenArgs) -> Result<(), String> {
    signal::install_cancel_handler();

    let spec = FrameSpec::from_args(&args)?;
    let gap = Duration::from_millis(args.gap);
    let sock = socket::open(&args.interface).map_err(|e| e.to_string())?;

    let mut rng = Rng::new_seeded();
    let mut id_step: u32 = 0;
    let mut data_step: u32 = 0;
    let mut sent: u64 = 0;

    loop {
        if signal::cancel_requested() {
            break;
        }
        if let Some(limit) = args.count
            && sent >= limit
        {
            break;
        }

        let frame = spec.build(&mut rng, id_step, data_step);
        if let Err(e) = sock.send(&frame) {
            // A transient EAGAIN on a busy bus should not abort generation.
            if e.kind() == std::io::ErrorKind::WouldBlock {
                thread::sleep(gap);
                continue;
            }
            return Err(e.to_string());
        }
        sent += 1;

        if !args.quiet {
            if args.decode {
                let mut line = format_readable(&frame, &args.interface);
                if let Some(info) = decode_isobus(&frame) {
                    line.push_str(&info.annotate());
                }
                println!("{line}");
            } else {
                println!("{}", format_readable(&frame, &args.interface));
            }
        }

        if args.increment {
            id_step = id_step.wrapping_add(1);
            data_step = data_step.wrapping_add(1);
        }

        if !gap.is_zero() {
            thread::sleep(gap);
        }
    }

    eprintln!("sent {} frames on {}", sent, args.interface);
    Ok(())
}

/// Resolved fixed/random shape of generated frames.
struct FrameSpec {
    fixed_id: Option<u32>,
    extended: bool,
    fixed_data: Option<Vec<u8>>,
    dlc: DlcMode,
}

enum DlcMode {
    Fixed(u8),
    Random,
}

impl FrameSpec {
    fn from_args(args: &GenArgs) -> Result<Self, String> {
        // Resolve fixed CAN ID (hex). An 8-digit ID forces extended mode.
        let (fixed_id, id_forces_ext) = match args.id.as_deref() {
            Some(id) => {
                let v = u32::from_str_radix(id, 16)
                    .map_err(|_| format!("invalid --id '{id}' (expected hex)"))?;
                (Some(v), id.len() >= 4)
            }
            None => (None, false),
        };
        let extended = args.extended || id_forces_ext;

        if let Some(id) = fixed_id {
            let max = if extended { 0x1FFF_FFFF } else { 0x07FF };
            if id > max {
                return Err(format!(
                    "--id {id:X} out of range for {} IDs (max {max:X})",
                    if extended { "extended" } else { "standard" }
                ));
            }
        }

        let fixed_data = match args.data.as_deref() {
            Some(d) => {
                if d.len() % 2 != 0 {
                    return Err(format!("invalid --data '{d}': odd hex length"));
                }
                let mut out = Vec::with_capacity(d.len() / 2);
                for chunk in d.as_bytes().chunks(2) {
                    let s = std::str::from_utf8(chunk).map_err(|_| "bad hex")?;
                    out.push(
                        u8::from_str_radix(s, 16)
                            .map_err(|_| format!("invalid --data byte '{s}'"))?,
                    );
                }
                if out.len() > CAN_MAX_DLEN {
                    return Err(format!(
                        "--data payload {} bytes exceeds CAN max {}",
                        out.len(),
                        CAN_MAX_DLEN
                    ));
                }
                Some(out)
            }
            None => None,
        };

        let dlc = match (args.dlc, args.random_dlc) {
            (Some(_), true) => {
                return Err("--dlc and --random-dlc are mutually exclusive".into());
            }
            (Some(len), false) => {
                if len > 8 {
                    return Err(format!("--dlc {len} out of range (0..=8)"));
                }
                DlcMode::Fixed(len)
            }
            (None, true) => DlcMode::Random,
            (None, false) => DlcMode::Fixed(8),
        };

        Ok(Self {
            fixed_id,
            extended,
            fixed_data,
            dlc,
        })
    }

    fn build(&self, rng: &mut Rng, id_step: u32, data_step: u32) -> RawFrame {
        let id = match self.fixed_id {
            Some(base) => {
                let span = if self.extended { 0x1FFF_FFFF } else { 0x07FF };
                (base.wrapping_add(id_step)) & span
            }
            None => {
                if self.extended {
                    (rng.next_u64() as u32) & 0x1FFF_FFFF
                } else {
                    rng.below(0x07FF)
                }
            }
        };

        let dlc = match self.dlc {
            DlcMode::Fixed(n) => n as usize,
            DlcMode::Random => rng.below(8) as usize,
        };

        let mut data = [0u8; CAN_MAX_DLEN];
        match &self.fixed_data {
            Some(fixed) => {
                let n = fixed.len().min(dlc);
                data[..n].copy_from_slice(&fixed[..n]);
                // For incremental mode, bump each fixed byte by data_step.
                if data_step != 0 {
                    for b in &mut data[..n] {
                        *b = b.wrapping_add(data_step as u8);
                    }
                }
                // Pad the unused tail with 0xFF (ISOBUS "not available").
                for b in &mut data[n..dlc.min(CAN_MAX_DLEN)] {
                    *b = 0xFF;
                }
            }
            None => rng.fill(&mut data[..dlc]),
        }

        if self.extended {
            RawFrame::make_ext(id, &data[..dlc])
        } else {
            RawFrame::make_std(id, &data[..dlc])
        }
    }
}
