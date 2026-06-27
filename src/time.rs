//! Monotonic time for the sans-IO core.
//!
//! The protocol core never reads a system clock. Callers stamp inputs with an
//! [`Instant`] and ask the core when it next needs servicing via `poll_at`. This
//! keeps the core pure (deterministic replay from fixtures) and `no_std`-ready:
//! the driver layer is the only place that touches a real clock and converts it
//! into an [`Instant`] at the edge.
//!
//! [`Instant`] is a monotonic count of microseconds since an arbitrary origin
//! chosen by the driver (commonly process/board start). Only differences between
//! instants are meaningful.

/// A monotonic timestamp in microseconds since a driver-chosen origin.
///
/// This is intentionally a plain `u64` micros newtype with no dependency on
/// `std::time` or any embedded time crate, so the core compiles unchanged on
/// hosted and bare-metal targets. Convert at the driver boundary with
/// [`Instant::from_micros`] / [`Instant::as_micros`] (and the provided `From`
/// impls).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Instant(u64);

impl Instant {
    /// The origin (zero) instant.
    pub const ZERO: Instant = Instant(0);

    /// Construct from a raw microsecond count.
    #[must_use]
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    /// Construct from a millisecond count.
    #[must_use]
    pub const fn from_millis(millis: u64) -> Self {
        Self(millis.saturating_mul(1_000))
    }

    /// Raw microsecond count since the origin.
    #[must_use]
    pub const fn as_micros(self) -> u64 {
        self.0
    }

    /// Whole milliseconds since the origin.
    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0 / 1_000
    }

    /// Microseconds elapsed since `earlier` (saturating; never negative).
    #[must_use]
    pub const fn saturating_duration_since(self, earlier: Instant) -> u64 {
        self.0.saturating_sub(earlier.0)
    }

    /// Whole milliseconds elapsed since `earlier` (saturating).
    #[must_use]
    pub const fn millis_since(self, earlier: Instant) -> u32 {
        let micros = self.0.saturating_sub(earlier.0);
        (micros / 1_000) as u32
    }

    /// This instant advanced by `micros` microseconds (saturating).
    #[must_use]
    pub const fn add_micros(self, micros: u64) -> Self {
        Self(self.0.saturating_add(micros))
    }

    /// This instant advanced by `millis` milliseconds (saturating).
    #[must_use]
    pub const fn add_millis(self, millis: u64) -> Self {
        Self(self.0.saturating_add(millis.saturating_mul(1_000)))
    }
}

impl From<u64> for Instant {
    /// Interpret the value as microseconds since the origin.
    fn from(micros: u64) -> Self {
        Self(micros)
    }
}

impl From<Instant> for u64 {
    fn from(instant: Instant) -> Self {
        instant.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn micros_and_millis_round_trip() {
        let t = Instant::from_millis(5);
        assert_eq!(t.as_micros(), 5_000);
        assert_eq!(t.as_millis(), 5);
        assert_eq!(Instant::from_micros(2_500).as_millis(), 2);
    }

    #[test]
    fn differences_saturate_and_never_go_negative() {
        let a = Instant::from_millis(10);
        let b = Instant::from_millis(25);
        assert_eq!(b.millis_since(a), 15);
        assert_eq!(a.millis_since(b), 0);
        assert_eq!(b.saturating_duration_since(a), 15_000);
    }

    #[test]
    fn advancing_is_saturating() {
        let t = Instant::from_micros(100);
        assert_eq!(t.add_millis(1).as_micros(), 1_100);
        assert_eq!(
            Instant::from_micros(u64::MAX).add_micros(10),
            Instant::from_micros(u64::MAX)
        );
    }

    #[test]
    fn ordering_is_monotonic() {
        assert!(Instant::ZERO < Instant::from_micros(1));
        assert!(Instant::from_millis(2) > Instant::from_millis(1));
    }
}
