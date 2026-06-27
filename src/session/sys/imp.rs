//! `stack.imp()` — implement-message handle (hitch / PTO / aux / status).
//!
//! Wraps the codec structs in [`crate::isobus::implement::tractor_commands`]
//! plus the speed/distance and lighting status codecs. Outbound: convenience
//! `command_*` / `broadcast_*` methods that encode + ship via `IsoNet::send`.
//! Inbound: incoming hitch/PTO/aux-valve commands plus hitch/PTO/speed/
//! lighting status frames are decoded, cached, and re-emitted as
//! [`ImplementEvent`] entries on the unified queue.

use crate::isobus::implement::tractor_commands::{
    AuxValveCommandMsg, HitchCommandMsg, PtoCommandMsg,
};
use crate::isobus::implement::{
    GroundBasedSpeedDist, HitchStatus, LightingState, MachineSelectedSpeedFull, PtoStatus,
    TractorFacilities, TractorFacilitiesRole, WheelBasedSpeedDist,
};
use crate::net::pgn_defs::{
    PGN_FRONT_HITCH_CMD, PGN_FRONT_PTO_CMD, PGN_REAR_HITCH_CMD, PGN_REAR_PTO_CMD,
};
use crate::net::types::{Address, Pgn};

/// Which hitch (front or rear).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hitch {
    Front,
    Rear,
}

impl Hitch {
    #[must_use]
    pub const fn cmd_pgn(self) -> Pgn {
        match self {
            Self::Front => PGN_FRONT_HITCH_CMD,
            Self::Rear => PGN_REAR_HITCH_CMD,
        }
    }
}

/// Which PTO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pto {
    Front,
    Rear,
}

impl Pto {
    #[must_use]
    pub const fn cmd_pgn(self) -> Pgn {
        match self {
            Self::Front => PGN_FRONT_PTO_CMD,
            Self::Rear => PGN_REAR_PTO_CMD,
        }
    }
}

/// Implement-side events on the unified queue. Fired when the
/// implement *receives* a command/status frame from a tractor (or vice versa,
/// depending on perspective — the codecs are symmetric).
#[derive(Debug, Clone, PartialEq)]
pub enum ImplementEvent {
    /// Hitch command received. `hitch` indicates front or rear based
    /// on which PGN delivered it.
    HitchCommand { hitch: Hitch, msg: HitchCommandMsg },
    /// PTO command received.
    PtoCommand { pto: Pto, msg: PtoCommandMsg },
    /// Aux-valve command received.
    AuxValveCommand(AuxValveCommandMsg),
    /// Hitch status feedback received.
    HitchStatus { hitch: Hitch, msg: HitchStatus },
    /// PTO status feedback received.
    PtoStatus { pto: Pto, msg: PtoStatus },
    /// Wheel-based speed/distance status received.
    WheelSpeed(WheelBasedSpeedDist),
    /// Ground-based speed/distance status received.
    GroundSpeed(GroundBasedSpeedDist),
    /// Full machine-selected-speed status received.
    MachineSelectedSpeed(MachineSelectedSpeedFull),
    /// Lighting data or command received.
    Lighting {
        command: bool,
        source: Address,
        state: LightingState,
    },
    /// Tractor-facilities response or implement requirement payload.
    TractorFacilities {
        role: TractorFacilitiesRole,
        source: Address,
        facilities: TractorFacilities,
    },
}
