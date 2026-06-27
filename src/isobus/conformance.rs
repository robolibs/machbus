//! ISO 11783-1 role conformance profile.
//!
//! GAP.md (ISO 11783-1) asks for "a formal conformance profile for exactly
//! which standard roles the crate claims" plus "a role-to-module ledger"
//! that states, per role, whether it is a product surface, an internal
//! helper, or unsupported. This module is that ledger expressed as typed,
//! queryable code so docs and a release gate can consult one source of
//! truth rather than prose that drifts.
//!
//! The statuses are deliberately conservative: a role is only
//! [`RoleStatus::Implemented`] when a client/server/runtime product
//! surface exists, [`RoleStatus::PartialHelper`] when only codecs/state
//! helpers exist, and [`RoleStatus::Unsupported`] when the crate does not
//! provide it. This keeps the crate honest about what "supported" means.

/// A standard ISO 11783 / ISOBUS participant role the crate may claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Iso11783Role {
    /// Generic control function: address claim / network management.
    ControlFunction,
    VtClient,
    VtServer,
    TcClient,
    TcServer,
    FsClient,
    FsServer,
    TractorEcu,
    ImplementEcu,
    SequenceControlMaster,
    SequenceControlClient,
    NetworkInterconnectUnit,
    TimParticipant,
    NmeaBridge,
}

/// How complete the crate's support for a role is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoleStatus {
    /// A product-level surface exists (client / server / runtime).
    Implemented,
    /// Codecs and/or state helpers exist, but not a complete product role.
    PartialHelper,
    /// Deliberately not provided by the crate.
    Unsupported,
}

/// One row of the role-to-module ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleProfile {
    pub role: Iso11783Role,
    pub status: RoleStatus,
    /// Primary module path backing the role.
    pub module: &'static str,
    /// Short, honest note on the support level.
    pub note: &'static str,
}

/// The conformance profile: one row per role. Source of truth for the
/// role-to-module ledger.
pub const ROLE_PROFILES: [RoleProfile; 14] = [
    RoleProfile {
        role: Iso11783Role::ControlFunction,
        status: RoleStatus::Implemented,
        module: "net::address_claimer",
        note: "NAME / address claim / commanded address",
    },
    RoleProfile {
        role: Iso11783Role::VtClient,
        status: RoleStatus::Implemented,
        module: "isobus::vt::client",
        note: "VT client protocol FSM + object pool upload",
    },
    RoleProfile {
        role: Iso11783Role::VtServer,
        status: RoleStatus::Implemented,
        module: "isobus::vt::server + isobus::vt::render",
        note: "server FSM + object-pool codec + render runtime",
    },
    RoleProfile {
        role: Iso11783Role::TcClient,
        status: RoleStatus::Implemented,
        module: "isobus::tc::client + isobus::tc::task",
        note: "TC client + task lifecycle/log/session runtime",
    },
    RoleProfile {
        role: Iso11783Role::TcServer,
        status: RoleStatus::Implemented,
        module: "isobus::tc::server",
        note: "TC server / process data / DDOP",
    },
    RoleProfile {
        role: Iso11783Role::FsClient,
        status: RoleStatus::Implemented,
        module: "isobus::fs::client",
        note: "file-server client operations",
    },
    RoleProfile {
        role: Iso11783Role::FsServer,
        status: RoleStatus::Implemented,
        module: "isobus::fs::server",
        note: "file-server server + operation matrix",
    },
    RoleProfile {
        role: Iso11783Role::TractorEcu,
        status: RoleStatus::Implemented,
        module: "isobus::tractor_ecu",
        note: "TECU facilities / speed-distance / hitch-PTO",
    },
    RoleProfile {
        role: Iso11783Role::ImplementEcu,
        status: RoleStatus::PartialHelper,
        module: "isobus::implement",
        note: "implement-message codecs, not a full ECU application",
    },
    RoleProfile {
        role: Iso11783Role::SequenceControlMaster,
        status: RoleStatus::Implemented,
        module: "isobus::sc::master",
        note: "SC master + sequence recording authoring",
    },
    RoleProfile {
        role: Iso11783Role::SequenceControlClient,
        status: RoleStatus::Implemented,
        module: "isobus::sc::client",
        note: "SC client playback flow",
    },
    RoleProfile {
        role: Iso11783Role::NetworkInterconnectUnit,
        status: RoleStatus::PartialHelper,
        module: "net (router/filter)",
        note: "router/filter helpers, not a managed-gateway product",
    },
    RoleProfile {
        role: Iso11783Role::TimParticipant,
        status: RoleStatus::PartialHelper,
        module: "isobus::tim",
        note: "authority/interlock/consent state guard, not a full TIM peer",
    },
    RoleProfile {
        role: Iso11783Role::NmeaBridge,
        status: RoleStatus::PartialHelper,
        module: "nmea",
        note: "selected PGN subset, not the full Appendix A/B catalog",
    },
];

/// The ledger row for a given role.
#[must_use]
pub fn profile_for(role: Iso11783Role) -> RoleProfile {
    // ROLE_PROFILES has exactly one row per role (asserted in tests).
    ROLE_PROFILES
        .into_iter()
        .find(|p| p.role == role)
        .expect("every role has a profile row")
}

/// `true` if the crate claims a complete product surface for `role`.
#[must_use]
pub fn is_implemented(role: Iso11783Role) -> bool {
    profile_for(role).status == RoleStatus::Implemented
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_ROLES: [Iso11783Role; 14] = [
        Iso11783Role::ControlFunction,
        Iso11783Role::VtClient,
        Iso11783Role::VtServer,
        Iso11783Role::TcClient,
        Iso11783Role::TcServer,
        Iso11783Role::FsClient,
        Iso11783Role::FsServer,
        Iso11783Role::TractorEcu,
        Iso11783Role::ImplementEcu,
        Iso11783Role::SequenceControlMaster,
        Iso11783Role::SequenceControlClient,
        Iso11783Role::NetworkInterconnectUnit,
        Iso11783Role::TimParticipant,
        Iso11783Role::NmeaBridge,
    ];

    #[test]
    fn every_role_has_exactly_one_profile_row() {
        assert_eq!(ROLE_PROFILES.len(), ALL_ROLES.len());
        for role in ALL_ROLES {
            let rows = ROLE_PROFILES.iter().filter(|p| p.role == role).count();
            assert_eq!(rows, 1, "role {role:?} must have exactly one profile row");
        }
    }

    #[test]
    fn profiles_carry_a_module_and_note() {
        for p in ROLE_PROFILES {
            assert!(!p.module.is_empty(), "{:?} missing module", p.role);
            assert!(!p.note.is_empty(), "{:?} missing note", p.role);
        }
    }

    #[test]
    fn honest_partials_are_not_claimed_implemented() {
        // The roles GAP.md flags as not-yet-complete must not over-claim.
        assert!(!is_implemented(Iso11783Role::ImplementEcu));
        assert!(!is_implemented(Iso11783Role::NetworkInterconnectUnit));
        assert!(!is_implemented(Iso11783Role::TimParticipant));
        assert!(!is_implemented(Iso11783Role::NmeaBridge));
        // The built-out product surfaces are claimed.
        assert!(is_implemented(Iso11783Role::VtServer));
        assert!(is_implemented(Iso11783Role::TcClient));
    }
}
