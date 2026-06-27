//! ISO 11783-5 network-management role matrix.
//!
//! GAP.md (ISO 11783-5) asks for "a complete management role matrix for
//! internal CFs, partner CFs, self-configurable addresses, commanded
//! address, NAME changes, and restart" that tags each behaviour as
//! product-ready or an internal helper. This is that matrix as typed,
//! queryable code — one honest source of truth, no drifting prose.

/// A network-management behaviour the crate may provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManagementBehavior {
    /// Internal CF address claim (NAME + claim/contend).
    InternalCfAddressClaim,
    /// Self-configurable-address reclaim onto a free address.
    SelfConfigurableReclaim,
    /// Address contest / duplicate-NAME resolution.
    AddressContest,
    /// Responding to a Request For Address Claim.
    RequestForAddressClaim,
    /// Accepting a Commanded Address targeted at our NAME.
    CommandedAddress,
    /// NAME change (pending + adopt + re-claim).
    NameChange,
    /// Partner CF tracking / discovery by NAME filter.
    PartnerCfDiscovery,
    /// Retaining the claimed address across a process restart.
    AddressRetentionAcrossRestart,
}

/// Support level for a management behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManagementSupport {
    /// A product-level runtime behaviour exists.
    Implemented,
    /// Partial / helper-level only — not a finished product behaviour.
    PartialHelper,
}

/// One row of the management role matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagementRole {
    pub behavior: ManagementBehavior,
    pub support: ManagementSupport,
    pub module: &'static str,
    pub note: &'static str,
}

use ManagementBehavior as B;
use ManagementSupport::{Implemented, PartialHelper};

/// The ISO 11783-5 management role matrix.
pub const MANAGEMENT_ROLES: [ManagementRole; 8] = [
    ManagementRole {
        behavior: B::InternalCfAddressClaim,
        support: Implemented,
        module: "net::address_claimer",
        note: "NAME-based claim and contention",
    },
    ManagementRole {
        behavior: B::SelfConfigurableReclaim,
        support: Implemented,
        module: "net::address_claimer",
        note: "reclaim onto a free address, skipping observed-occupied",
    },
    ManagementRole {
        behavior: B::AddressContest,
        support: Implemented,
        module: "net::address_claimer",
        note: "lower-NAME wins; non-self-configurable loser fails",
    },
    ManagementRole {
        behavior: B::RequestForAddressClaim,
        support: Implemented,
        module: "net::address_claimer",
        note: "re-announces the current claim on request",
    },
    ManagementRole {
        behavior: B::CommandedAddress,
        support: Implemented,
        module: "net::name_manager",
        note: "accepts a commanded address targeted at our NAME",
    },
    ManagementRole {
        behavior: B::NameChange,
        support: Implemented,
        module: "net::name_manager",
        note: "pending NAME, adopt, and re-claim",
    },
    ManagementRole {
        behavior: B::PartnerCfDiscovery,
        support: Implemented,
        module: "net::partner_cf",
        note: "partner CF tracking by NAME filter",
    },
    ManagementRole {
        behavior: B::AddressRetentionAcrossRestart,
        support: PartialHelper,
        module: "net::address_claimer",
        note: "no persistence layer; address is not retained across a process restart",
    },
];

/// The matrix row for a behaviour.
#[must_use]
pub fn role_for(behavior: ManagementBehavior) -> ManagementRole {
    MANAGEMENT_ROLES
        .into_iter()
        .find(|r| r.behavior == behavior)
        .expect("every behaviour has a matrix row")
}

/// `true` if the behaviour is a finished product-level behaviour.
#[must_use]
pub fn is_implemented(behavior: ManagementBehavior) -> bool {
    role_for(behavior).support == ManagementSupport::Implemented
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ManagementBehavior; 8] = [
        B::InternalCfAddressClaim,
        B::SelfConfigurableReclaim,
        B::AddressContest,
        B::RequestForAddressClaim,
        B::CommandedAddress,
        B::NameChange,
        B::PartnerCfDiscovery,
        B::AddressRetentionAcrossRestart,
    ];

    #[test]
    fn every_behavior_has_exactly_one_row_with_module_and_note() {
        assert_eq!(MANAGEMENT_ROLES.len(), ALL.len());
        for b in ALL {
            let rows: Vec<_> = MANAGEMENT_ROLES
                .iter()
                .filter(|r| r.behavior == b)
                .collect();
            assert_eq!(rows.len(), 1, "{b:?} must have one row");
            assert!(!rows[0].module.is_empty());
            assert!(!rows[0].note.is_empty());
        }
    }

    #[test]
    fn restart_retention_is_honestly_partial() {
        // GAP.md: restart/address-retention is not a finished product policy.
        assert!(!is_implemented(B::AddressRetentionAcrossRestart));
        // The core claim behaviours are implemented.
        assert!(is_implemented(B::InternalCfAddressClaim));
        assert!(is_implemented(B::CommandedAddress));
        assert!(is_implemented(B::NameChange));
    }
}
