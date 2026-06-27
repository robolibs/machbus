//! Process-data transmit rate limiting (ISO 11783-10).
//!
//! A TC client must not flood a single process-data variable: the standard caps
//! the transmit rate per variable (10 messages/second by default). This is the
//! repo-owned per-variable throttle: the caller drives a clock and asks whether
//! a value for a given `(element, DDI)` may be sent now. It models only the rate
//! cap; request/response pairing is a separate concern.

use alloc::collections::BTreeMap;

/// Default minimum spacing between value messages for one variable (10/s).
pub const DEFAULT_MIN_INTERVAL_MS: u32 = 100;

/// Per-`(element, DDI)` transmit-rate limiter.
#[derive(Debug, Clone)]
pub struct ProcessDataRateLimiter {
    /// Monotonic clock advanced by [`tick`](Self::tick).
    now_ms: u32,
    /// Minimum spacing between sends for one variable.
    min_interval_ms: u32,
    /// Last time a send was allowed for each `(element, ddi)`.
    last_send: BTreeMap<(u16, u16), u32>,
}

impl Default for ProcessDataRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessDataRateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            now_ms: 0,
            min_interval_ms: DEFAULT_MIN_INTERVAL_MS,
            last_send: BTreeMap::new(),
        }
    }

    /// Set the cap as a maximum message rate per second (clamped to ≥ 1/s).
    #[must_use]
    pub fn with_max_rate_per_s(mut self, per_s: u32) -> Self {
        self.min_interval_ms = 1000 / per_s.max(1);
        self
    }

    /// Advance the internal clock.
    pub fn tick(&mut self, elapsed_ms: u32) {
        self.now_ms = self.now_ms.saturating_add(elapsed_ms);
    }

    /// Whether a value for `(element, ddi)` may be sent now. When it returns
    /// `true` it records the send so the next one is gated by the interval.
    pub fn allow(&mut self, element: u16, ddi: u16) -> bool {
        let key = (element, ddi);
        let ok = match self.last_send.get(&key) {
            Some(&last) => self.now_ms.saturating_sub(last) >= self.min_interval_ms,
            None => true,
        };
        if ok {
            self.last_send.insert(key, self.now_ms);
        }
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_variable_rate_is_capped_independently() {
        let mut rl = ProcessDataRateLimiter::new(); // 100 ms / 10 per s
        // First send for a variable is always allowed.
        assert!(rl.allow(1, 0x0041));
        // Immediately again: blocked (within the interval).
        assert!(!rl.allow(1, 0x0041));
        // A different variable is independent.
        assert!(rl.allow(1, 0x0042));
        assert!(rl.allow(2, 0x0041));

        // After less than the interval: still blocked.
        rl.tick(60);
        assert!(!rl.allow(1, 0x0041));
        // Crossing the interval: allowed again.
        rl.tick(60); // total 120 ms since the send
        assert!(rl.allow(1, 0x0041));
    }

    #[test]
    fn custom_rate_changes_interval() {
        let mut rl = ProcessDataRateLimiter::new().with_max_rate_per_s(5); // 200 ms
        assert!(rl.allow(0, 1));
        rl.tick(150);
        assert!(!rl.allow(0, 1));
        rl.tick(60); // 210 ms total
        assert!(rl.allow(0, 1));
    }
}
