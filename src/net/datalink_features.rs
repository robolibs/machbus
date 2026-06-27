//! Data-link feature checklist (ISO 11783-3).
//!
//! GAP.md (ISO 11783-3) asks to "generate a data-link feature checklist"
//! and "mark each TP/ETP/session case implemented, rejected, or missing."
//! This is that checklist as typed, queryable code over the transport
//! features the crate provides.
//!
//! It contains no standard prose — only the repo-owned feature→module→status
//! classification.

/// A data-link transport feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataLinkFeature {
    /// Single-frame (≤8 byte) messages.
    SingleFrame,
    /// Transport Protocol BAM (broadcast) sessions.
    TpBroadcast,
    /// Transport Protocol connection mode (RTS/CTS/EoMA).
    TpConnection,
    /// Transport Protocol abort handling (both directions).
    TpAbort,
    /// Transport Protocol T1/T2/T3/T4 timeouts.
    TpTimeouts,
    /// Extended Transport Protocol (>1785-byte payloads).
    Etp,
    /// ETP CTS hold/resume (receiver-paused flow control).
    EtpCtsHoldResume,
    /// NMEA 2000 Fast Packet multi-frame assembly.
    FastPacket,
    /// Distinct T2 (post-CTS data-wait) timeout path, separate from T1.
    TpT2DataTimeout,
    /// CSMA/CR arbitration-loss back-off / priority transmit ordering.
    CsmaCrArbitration,
    /// CAN error-state handling (error-passive / bus-off / error frames).
    BusFaultHandling,
}

/// Status of a data-link feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DlStatus {
    /// Implemented with in-repo tests.
    Implemented,
    /// Deliberately rejected / unsupported.
    Rejected,
    /// Not yet implemented.
    Missing,
}

/// One checklist row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataLinkFeatureRow {
    pub feature: DataLinkFeature,
    pub module: &'static str,
    pub status: DlStatus,
}

use DataLinkFeature as F;
use DlStatus::{Implemented, Missing};

/// The ISO 11783-3 data-link feature checklist.
pub const DATALINK_FEATURES: [DataLinkFeatureRow; 11] = [
    DataLinkFeatureRow {
        feature: F::SingleFrame,
        module: "net::message",
        status: Implemented,
    },
    DataLinkFeatureRow {
        feature: F::TpBroadcast,
        module: "net::tp",
        status: Implemented,
    },
    DataLinkFeatureRow {
        feature: F::TpConnection,
        module: "net::tp",
        status: Implemented,
    },
    DataLinkFeatureRow {
        feature: F::TpAbort,
        module: "net::tp + net::session",
        status: Implemented,
    },
    DataLinkFeatureRow {
        feature: F::TpTimeouts,
        module: "net::tp + net::timer",
        status: Implemented,
    },
    DataLinkFeatureRow {
        feature: F::Etp,
        module: "net::etp",
        status: Implemented,
    },
    DataLinkFeatureRow {
        feature: F::EtpCtsHoldResume,
        module: "net::etp",
        status: Implemented,
    },
    DataLinkFeatureRow {
        feature: F::FastPacket,
        module: "net::fast_packet",
        status: Implemented,
    },
    // Honest gaps (see GAP.md ISO 11783-3): present in the standard but not yet
    // implemented as distinct behaviour.
    DataLinkFeatureRow {
        feature: F::TpT2DataTimeout,
        module: "net::tp",
        status: Missing,
    },
    DataLinkFeatureRow {
        feature: F::CsmaCrArbitration,
        module: "net::bus_load",
        status: Missing,
    },
    DataLinkFeatureRow {
        feature: F::BusFaultHandling,
        module: "net::policy",
        status: Missing,
    },
];

/// The checklist row for a feature.
#[must_use]
pub fn datalink_feature(feature: DataLinkFeature) -> DataLinkFeatureRow {
    DATALINK_FEATURES
        .into_iter()
        .find(|r| r.feature == feature)
        .expect("every data-link feature has a row")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [DataLinkFeature; 11] = [
        F::SingleFrame,
        F::TpBroadcast,
        F::TpConnection,
        F::TpAbort,
        F::TpTimeouts,
        F::Etp,
        F::EtpCtsHoldResume,
        F::FastPacket,
        F::TpT2DataTimeout,
        F::CsmaCrArbitration,
        F::BusFaultHandling,
    ];

    const IMPLEMENTED: [DataLinkFeature; 8] = [
        F::SingleFrame,
        F::TpBroadcast,
        F::TpConnection,
        F::TpAbort,
        F::TpTimeouts,
        F::Etp,
        F::EtpCtsHoldResume,
        F::FastPacket,
    ];

    #[test]
    fn every_feature_has_one_row_with_a_module_and_honest_status() {
        assert_eq!(DATALINK_FEATURES.len(), ALL.len());
        for f in ALL {
            let rows: Vec<_> = DATALINK_FEATURES
                .iter()
                .filter(|r| r.feature == f)
                .collect();
            assert_eq!(rows.len(), 1, "{f:?} must have one row");
            assert!(rows[0].module.starts_with("net::"));
            // Status is honest: implemented features are Implemented; the three
            // tracked gaps are Missing (not silently claimed as done).
            let expected = if IMPLEMENTED.contains(&f) {
                DlStatus::Implemented
            } else {
                DlStatus::Missing
            };
            assert_eq!(rows[0].status, expected, "{f:?} status");
        }
        assert_eq!(datalink_feature(F::Etp).module, "net::etp");
        assert_eq!(
            datalink_feature(F::BusFaultHandling).status,
            DlStatus::Missing
        );
    }
}
