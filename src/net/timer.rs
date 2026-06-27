//! Periodic [`Timer`] and one-shot [`Timeout`] driven by polled
//! `update(delta_ms)` calls.
//!
//! Mirrors the C++ `machbus::net::Timer` and `Timeout`.

/// Auto-resetting (or single-shot) periodic interval timer.
#[derive(Debug, Clone, Copy)]
pub struct Timer {
    interval_ms: u32,
    elapsed_ms: u32,
    running: bool,
    auto_reset: bool,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new(0, true)
    }
}

impl Timer {
    /// Build a timer with the given interval. `auto_reset = true`
    /// produces periodic ticks; `false` makes it one-shot.
    #[must_use]
    pub const fn new(interval_ms: u32, auto_reset: bool) -> Self {
        Self {
            interval_ms,
            elapsed_ms: 0,
            running: false,
            auto_reset,
        }
    }

    pub fn set_interval(&mut self, ms: u32) {
        self.interval_ms = ms;
    }

    #[inline]
    #[must_use]
    pub const fn interval(&self) -> u32 {
        self.interval_ms
    }

    /// Start (or restart) the timer; clears elapsed.
    pub fn start(&mut self) {
        self.running = true;
        self.elapsed_ms = 0;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Reset elapsed without changing the running state.
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
    }

    #[inline]
    #[must_use]
    pub const fn running(&self) -> bool {
        self.running
    }

    /// Advance the timer by `delta_ms`. Returns `true` exactly once
    /// per interval that has elapsed during this update.
    ///
    /// On expiration, an auto-reset timer subtracts the interval from
    /// `elapsed`, preserving any overshoot; a one-shot timer stops.
    pub fn update(&mut self, delta_ms: u32) -> bool {
        if !self.running || self.interval_ms == 0 {
            return false;
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        if self.elapsed_ms >= self.interval_ms {
            if self.auto_reset {
                self.elapsed_ms -= self.interval_ms;
            } else {
                self.running = false;
            }
            return true;
        }
        false
    }

    #[inline]
    #[must_use]
    pub const fn elapsed(&self) -> u32 {
        self.elapsed_ms
    }

    #[must_use]
    pub const fn remaining(&self) -> u32 {
        if !self.running || self.elapsed_ms >= self.interval_ms {
            0
        } else {
            self.interval_ms - self.elapsed_ms
        }
    }

    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.interval_ms == 0 {
            0.0
        } else {
            self.elapsed_ms as f32 / self.interval_ms as f32
        }
    }

    #[inline]
    #[must_use]
    pub const fn expired(&self) -> bool {
        self.running && self.elapsed_ms >= self.interval_ms
    }
}

/// One-shot timeout. Fires `true` once when the elapsed window passes
/// the configured threshold, then deactivates.
#[derive(Debug, Clone, Copy, Default)]
pub struct Timeout {
    timeout_ms: u32,
    elapsed_ms: u32,
    active: bool,
}

impl Timeout {
    #[must_use]
    pub const fn new(timeout_ms: u32) -> Self {
        Self {
            timeout_ms,
            elapsed_ms: 0,
            active: false,
        }
    }

    /// Start with a new timeout value.
    pub fn start_with(&mut self, timeout_ms: u32) {
        self.timeout_ms = timeout_ms;
        self.elapsed_ms = 0;
        self.active = true;
    }

    /// Restart with the existing timeout value.
    pub fn start(&mut self) {
        self.elapsed_ms = 0;
        self.active = true;
    }

    pub fn cancel(&mut self) {
        self.active = false;
    }

    /// Advance the timeout. Returns `true` on the call that crosses
    /// the threshold; returns `false` thereafter.
    pub fn update(&mut self, delta_ms: u32) -> bool {
        if !self.active {
            return false;
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        if self.elapsed_ms >= self.timeout_ms {
            self.active = false;
            return true;
        }
        false
    }

    #[inline]
    #[must_use]
    pub const fn active(&self) -> bool {
        self.active
    }

    #[inline]
    #[must_use]
    pub const fn timed_out(&self) -> bool {
        !self.active && self.elapsed_ms >= self.timeout_ms
    }

    #[inline]
    #[must_use]
    pub const fn elapsed(&self) -> u32 {
        self.elapsed_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_does_not_fire_when_stopped() {
        let mut t = Timer::new(100, true);
        for _ in 0..5 {
            assert!(!t.update(50));
        }
    }

    #[test]
    fn timer_fires_when_interval_reached() {
        let mut t = Timer::new(100, true);
        t.start();
        assert!(!t.update(50));
        assert!(t.update(50));
    }

    #[test]
    fn auto_reset_preserves_overshoot() {
        let mut t = Timer::new(100, true);
        t.start();
        // 150 ms of one update: should fire once and leave 50 ms elapsed.
        assert!(t.update(150));
        assert_eq!(t.elapsed(), 50);
        assert!(t.running());
    }

    #[test]
    fn one_shot_stops_after_firing() {
        let mut t = Timer::new(100, false);
        t.start();
        assert!(t.update(120));
        assert!(!t.running());
        assert!(!t.update(100));
    }

    #[test]
    fn zero_interval_never_fires() {
        let mut t = Timer::new(0, true);
        t.start();
        for _ in 0..10 {
            assert!(!t.update(1000));
        }
    }

    #[test]
    fn remaining_and_progress() {
        let mut t = Timer::new(200, true);
        t.start();
        t.update(50);
        assert_eq!(t.remaining(), 150);
        assert!((t.progress() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn timeout_fires_once_then_inactive() {
        let mut to = Timeout::new(100);
        to.start();
        assert!(to.active());
        assert!(!to.update(50));
        assert!(to.update(60));
        assert!(!to.active());
        assert!(to.timed_out());
        assert!(!to.update(100));
    }

    #[test]
    fn timeout_start_with_replaces_value() {
        let mut to = Timeout::new(100);
        to.start_with(20);
        assert!(to.update(25));
    }

    #[test]
    fn timeout_cancel_prevents_firing() {
        let mut to = Timeout::new(100);
        to.start();
        to.cancel();
        assert!(!to.update(200));
        assert!(!to.timed_out());
    }
}
