//! Rolling-window bus load estimator.
//!
//! Mirrors the C++ `machbus::net::BusLoad`. Counts approximate CAN
//! bits per 100 ms window over a 100-sample (= 10 s) ring buffer, then
//! reports load against a fixed 250 kbit/s ISOBUS bitrate.
//!
//! The bit-count formula tracks classic-CAN frame overhead plus an
//! average 20 % stuff-bit allowance — same constants as the C++.

const WINDOW_SIZE: usize = 100;
const SAMPLE_PERIOD_MS: u32 = 100;
const CAN_BITRATE: u32 = 250_000;

/// Rolling-window CAN bus utilization estimator.
#[derive(Debug, Clone)]
pub struct BusLoad {
    bit_counts: [u32; WINDOW_SIZE],
    write_idx: usize,
    current_bits: u32,
    timer_ms: u32,
    filled: bool,
}

impl Default for BusLoad {
    fn default() -> Self {
        Self::new()
    }
}

impl BusLoad {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bit_counts: [0; WINDOW_SIZE],
            write_idx: 0,
            current_bits: 0,
            timer_ms: 0,
            filled: false,
        }
    }

    /// Account for a single transmitted/received frame with `dlc`
    /// bytes of payload (`dlc ≤ 8`).
    ///
    /// Adds `(67 + dlc * 8) * 1.20` bits — the classic-CAN frame
    /// overhead plus an average stuff-bit estimate.
    pub fn add_frame(&mut self, dlc: u8) {
        let dlc = (dlc as u32).min(8);
        let mut bits = 67 + dlc * 8;
        bits = bits * 120 / 100;
        self.current_bits = self.current_bits.saturating_add(bits);
    }

    /// Advance the sampler by `delta_ms`. Bins the running bit count
    /// into the rolling window every 100 ms (one sample period).
    pub fn update(&mut self, delta_ms: u32) {
        self.timer_ms = self.timer_ms.saturating_add(delta_ms);
        while self.timer_ms >= SAMPLE_PERIOD_MS {
            self.timer_ms -= SAMPLE_PERIOD_MS;
            self.bit_counts[self.write_idx] = self.current_bits;
            self.write_idx = (self.write_idx + 1) % WINDOW_SIZE;
            if self.write_idx == 0 {
                self.filled = true;
            }
            self.current_bits = 0;
        }
    }

    /// Estimated bus load over the rolling window, in percent
    /// (`0.0..=100.0+`). Returns `0.0` until the first sample.
    #[must_use]
    pub fn load_percent(&self) -> f32 {
        let count = if self.filled {
            WINDOW_SIZE
        } else {
            self.write_idx
        };
        if count == 0 {
            return 0.0;
        }
        let total_bits: u32 = self.bit_counts[..count].iter().sum();
        let window_seconds = (count as f32) * (SAMPLE_PERIOD_MS as f32) / 1000.0;
        let bits_per_second = total_bits as f32 / window_seconds;
        (bits_per_second / CAN_BITRATE as f32) * 100.0
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Number of complete samples currently in the window.
    #[must_use]
    pub const fn sample_count(&self) -> usize {
        if self.filled {
            WINDOW_SIZE
        } else {
            self.write_idx
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_load_is_zero() {
        let bl = BusLoad::new();
        assert_eq!(bl.load_percent(), 0.0);
    }

    #[test]
    fn add_frame_alone_does_not_advance_window() {
        let mut bl = BusLoad::new();
        bl.add_frame(8);
        assert_eq!(bl.load_percent(), 0.0); // no sample yet
    }

    #[test]
    fn one_sample_period_yields_one_bin() {
        let mut bl = BusLoad::new();
        bl.add_frame(8);
        bl.update(SAMPLE_PERIOD_MS);
        assert_eq!(bl.sample_count(), 1);
        assert!(bl.load_percent() > 0.0);
    }

    #[test]
    fn idle_bus_reports_zero_load() {
        let mut bl = BusLoad::new();
        // Run several windows with no traffic.
        for _ in 0..(WINDOW_SIZE * 2) {
            bl.update(SAMPLE_PERIOD_MS);
        }
        assert!(bl.filled);
        assert_eq!(bl.load_percent(), 0.0);
    }

    #[test]
    fn known_traffic_yields_expected_load() {
        // 10 frames of 8 bytes each in a single 100 ms window:
        //   bits per frame = (67 + 64) * 120/100 = 157
        //   total bits     = 1570
        //   bits/sec       = 15700
        //   load           = 15700 / 250000 = 6.28 %
        let mut bl = BusLoad::new();
        for _ in 0..10 {
            bl.add_frame(8);
        }
        bl.update(SAMPLE_PERIOD_MS);
        let load = bl.load_percent();
        // The window has only 1 sample so load uses 100 ms only.
        let expected = 1570.0 / (CAN_BITRATE as f32) * 1000.0; // ≈ 6.28
        assert!(
            (load - expected).abs() < 0.01,
            "expected ~{expected}%, got {load}"
        );
    }

    #[test]
    fn ring_buffer_wraps_at_window_size() {
        let mut bl = BusLoad::new();
        for _ in 0..(WINDOW_SIZE + 5) {
            bl.add_frame(8);
            bl.update(SAMPLE_PERIOD_MS);
        }
        assert!(bl.filled);
        assert_eq!(bl.sample_count(), WINDOW_SIZE);
    }

    #[test]
    fn large_delta_emits_multiple_samples() {
        let mut bl = BusLoad::new();
        bl.update(SAMPLE_PERIOD_MS * 5);
        assert_eq!(bl.sample_count(), 5);
    }

    #[test]
    fn reset_clears_state() {
        let mut bl = BusLoad::new();
        bl.add_frame(8);
        bl.update(SAMPLE_PERIOD_MS * (WINDOW_SIZE as u32 + 10));
        bl.reset();
        assert_eq!(bl.sample_count(), 0);
        assert_eq!(bl.load_percent(), 0.0);
    }
}
