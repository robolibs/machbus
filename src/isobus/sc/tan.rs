//! Sequence-control transaction numbers (ISO 11783-14).
//!
//! Each function activation in a sequence is identified by a transaction
//! number (TAN). The originator repeats the activation periodically until the
//! peer acknowledges that TAN. This module is the repo-owned transaction layer
//! the execution message exchange builds on: TAN allocation plus a
//! repeat-until-acknowledged timer. It carries no standard prose.

/// Period at which an unacknowledged transaction is repeated, milliseconds.
pub const SC_TAN_REPEAT_MS: u32 = 100;
/// Lowest usable transaction number.
pub const SC_TAN_MIN: u8 = 1;
/// Highest usable transaction number.
pub const SC_TAN_MAX: u8 = 254;
/// Reserved "no transaction" sentinel.
pub const SC_TAN_NOT_AVAILABLE: u8 = 0xFF;

/// Allocates transaction numbers and drives the repeat-until-acknowledged
/// timer for the single in-flight transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceTanTracker {
    next: u8,
    pending: Option<u8>,
    elapsed_ms: u32,
}

impl Default for SequenceTanTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SequenceTanTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: SC_TAN_MIN,
            pending: None,
            elapsed_ms: 0,
        }
    }

    /// Allocate the next transaction number, wrapping within `SC_TAN_MIN..=MAX`.
    pub fn allocate(&mut self) -> u8 {
        let tan = self.next;
        self.next = if self.next >= SC_TAN_MAX {
            SC_TAN_MIN
        } else {
            self.next + 1
        };
        tan
    }

    /// Begin tracking `tan` as the in-flight transaction (resets the timer).
    pub fn start(&mut self, tan: u8) {
        self.pending = Some(tan);
        self.elapsed_ms = 0;
    }

    /// The transaction currently awaiting acknowledgement, if any.
    #[must_use]
    pub const fn pending(&self) -> Option<u8> {
        self.pending
    }

    /// Acknowledge `tan`. Returns `true` if it matched the in-flight one
    /// (which is then cleared).
    pub fn acknowledge(&mut self, tan: u8) -> bool {
        if self.pending == Some(tan) {
            self.pending = None;
            self.elapsed_ms = 0;
            true
        } else {
            false
        }
    }

    /// Advance the timer. Returns the pending TAN to retransmit when a repeat
    /// period has elapsed without acknowledgement (and resets the period).
    pub fn update(&mut self, elapsed_ms: u32) -> Option<u8> {
        let tan = self.pending?;
        self.elapsed_ms = self.elapsed_ms.saturating_add(elapsed_ms);
        if self.elapsed_ms >= SC_TAN_REPEAT_MS {
            self.elapsed_ms -= SC_TAN_REPEAT_MS;
            Some(tan)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_wraps_within_range() {
        let mut t = SequenceTanTracker::new();
        assert_eq!(t.allocate(), 1);
        assert_eq!(t.allocate(), 2);
        // Jump near the top and confirm the wrap skips the reserved values.
        let mut t2 = SequenceTanTracker {
            next: SC_TAN_MAX,
            pending: None,
            elapsed_ms: 0,
        };
        assert_eq!(t2.allocate(), SC_TAN_MAX);
        assert_eq!(t2.allocate(), SC_TAN_MIN);
    }

    #[test]
    fn repeat_until_acknowledged() {
        let mut t = SequenceTanTracker::new();
        let tan = t.allocate();
        t.start(tan);
        assert_eq!(t.pending(), Some(tan));

        // No repeat before the period elapses.
        assert_eq!(t.update(60), None);
        // Crossing the period yields the TAN to retransmit.
        assert_eq!(t.update(60), Some(tan));
        // It keeps repeating until acknowledged.
        assert_eq!(t.update(100), Some(tan));

        assert!(t.acknowledge(tan));
        assert_eq!(t.pending(), None);
        // After ack, no further repeats.
        assert_eq!(t.update(200), None);
        // Acknowledging an unknown TAN is a no-op.
        assert!(!t.acknowledge(99));
    }
}
