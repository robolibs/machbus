//! Background capture worker: drains a SocketCAN socket or replays a
//! `candump` file, decoding every frame off the render thread and shipping
//! it through an `mpsc` channel to the UI loop.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::can::parse_candump_line;
use crate::socket;
use crate::tui::decode::build_entry;
use crate::tui::model::FrameEntry;

/// Messages sent from the worker to the UI loop.
pub enum CaptureMsg {
    Frame(FrameEntry),
    Info(String),
    Error(String),
    /// File replay finished.
    Eof,
}

/// Handle to a running capture thread. [`CaptureWorker::stop`] signals it
/// to exit and joins.
pub struct CaptureWorker {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CaptureWorker {
    /// Start a live SocketCAN capture on `iface`.
    pub fn start_socket(iface: &str) -> (Self, mpsc::Receiver<CaptureMsg>) {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let iface_owned = iface.to_string();
        let tx_clone = tx.clone();
        let handle = thread::Builder::new()
            .name("machbus-capture".into())
            .spawn(move || run_socket(&iface_owned, tx_clone, &stop_clone))
            .ok();
        (Self { stop, handle }, rx)
    }

    /// Start a timed replay of a `candump` file at the given speed.
    pub fn start_file(path: &str, speed: f64) -> (Self, mpsc::Receiver<CaptureMsg>) {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let path_owned = path.to_string();
        let tx_clone = tx.clone();
        let handle = thread::Builder::new()
            .name("machbus-replay".into())
            .spawn(move || run_file(&path_owned, speed, tx_clone, &stop_clone))
            .ok();
        (Self { stop, handle }, rx)
    }

    /// Signal the worker to stop and wait for it.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for CaptureWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_socket(iface: &str, tx: mpsc::Sender<CaptureMsg>, stop: &AtomicBool) {
    let sock = match socket::open(iface) {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(CaptureMsg::Error(format!("open {iface}: {e}")));
            return;
        }
    };
    let _ = tx.send(CaptureMsg::Info(format!("listening on {iface}")));
    let start = Instant::now();
    let mut seq: u64 = 0;
    let poll = Duration::from_millis(200);

    while !stop.load(Ordering::Acquire) {
        match sock.recv(poll) {
            Ok(Some((raw, src_iface))) => {
                seq += 1;
                let rel_ms = start.elapsed().as_millis() as u64;
                let entry = build_entry(seq, rel_ms, src_iface, &raw);
                if tx.send(CaptureMsg::Frame(entry)).is_err() {
                    break; // UI gone
                }
            }
            Ok(None) => continue,
            Err(e) => {
                let _ = tx.send(CaptureMsg::Error(format!("recv: {e}")));
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn run_file(path: &str, speed: f64, tx: mpsc::Sender<CaptureMsg>, stop: &AtomicBool) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            let _ = tx.send(CaptureMsg::Error(format!("read {path}: {e}")));
            return;
        }
    };
    let speed = if speed > 0.0 { speed } else { 1.0 };
    let _ = tx.send(CaptureMsg::Info(format!("replaying {path} @ {speed}x")));

    let mut seq: u64 = 0;
    let mut first_us: Option<u64> = None;
    let mut played_us: u64 = 0;
    let replay_start = Instant::now();
    let default_gap = Duration::from_millis(40);

    for line in text.lines() {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let Some(parsed) = parse_candump_line(line) else {
            continue;
        };

        // Pace by the capture timestamps if present, otherwise a fixed gap.
        if let Some(ts) = parsed.timestamp_us {
            let base = *first_us.get_or_insert(ts);
            let target_us = ts.saturating_sub(base);
            let want = Duration::from_micros(((target_us as f64) / speed) as u64);
            let elapsed = replay_start.elapsed();
            if want > elapsed {
                let to_sleep = want - elapsed;
                sleep_chunks(to_sleep, stop);
            }
            played_us = target_us;
        } else {
            sleep_chunks(default_gap, stop);
        }
        let _ = played_us; // tracked for completeness

        seq += 1;
        let rel_ms = (played_us / 1000).max(replay_start.elapsed().as_millis() as u64);
        let raw = parsed.to_raw();
        let entry = build_entry(
            seq,
            rel_ms,
            parsed.interface.unwrap_or_else(|| "replay".into()),
            &raw,
        );
        if tx.send(CaptureMsg::Frame(entry)).is_err() {
            return;
        }
    }
    let _ = tx.send(CaptureMsg::Eof);
}

/// Sleep in small increments so a stop signal is noticed promptly.
fn sleep_chunks(total: Duration, stop: &AtomicBool) {
    let mut remaining = total;
    let step = Duration::from_millis(50);
    while !remaining.is_zero() && !stop.load(Ordering::Acquire) {
        let s = remaining.min(step);
        thread::sleep(s);
        remaining = remaining.saturating_sub(s);
    }
}
