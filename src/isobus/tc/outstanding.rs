//! Process-data request flow control (ISO 11783-10).
//!
//! A TC may have at most one outstanding (unacknowledged) process-data request
//! per client at a time: a new command must wait until the previous one is
//! answered. This is the repo-owned per-client tracker; pairing it to specific
//! message exchanges is the caller's concern.

use alloc::collections::BTreeSet;

use crate::net::types::Address;

/// Tracks which clients currently have an unanswered request in flight.
#[derive(Debug, Clone, Default)]
pub struct OutstandingRequests {
    pending: BTreeSet<Address>,
}

impl OutstandingRequests {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attempt to start a request for `client`. Returns `true` and marks the
    /// client busy if none was outstanding; returns `false` (request must wait)
    /// if one already is.
    pub fn try_begin(&mut self, client: Address) -> bool {
        self.pending.insert(client)
    }

    /// Mark `client`'s outstanding request answered, freeing it for the next.
    pub fn complete(&mut self, client: Address) {
        self.pending.remove(&client);
    }

    /// Whether `client` currently has a request awaiting a response.
    #[must_use]
    pub fn is_pending(&self, client: Address) -> bool {
        self.pending.contains(&client)
    }

    /// Number of clients with an outstanding request.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_outstanding_request_per_client() {
        let mut o = OutstandingRequests::new();
        // First request for a client is allowed.
        assert!(o.try_begin(0x80));
        assert!(o.is_pending(0x80));
        // A second, before completion, is rejected.
        assert!(!o.try_begin(0x80));
        // A different client is independent.
        assert!(o.try_begin(0x81));

        // After completion the client may issue again.
        o.complete(0x80);
        assert!(!o.is_pending(0x80));
        assert!(o.try_begin(0x80));
        assert_eq!(o.len(), 2);
    }
}
