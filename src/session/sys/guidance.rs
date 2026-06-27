//! Automatic-guidance (autosteer) events — ISO 11783-7 agricultural guidance.
//!
//! The high-level [`Guidance`](crate::session::plugins::Guidance) plugin commands
//! a steering system by *curvature* (Guidance System Command, PGN 0xAD00) and
//! decodes the steering ECU's Agricultural Guidance Machine Info (PGN 0xAC00)
//! into the events below.

use crate::net::types::Address;

/// Events emitted by the guidance/autosteer subsystem.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuidanceEvent {
    /// Agricultural Guidance Machine Info received from a steering ECU
    /// (PGN 0xAC00). Reports the steering system's own view of the world.
    MachineInfo {
        /// Source address of the steering ECU that sent it.
        source: Address,
        /// The steering system's estimated path curvature, in 1/km
        /// (positive and negative follow the wire convention).
        estimated_curvature: f64,
        /// `true` when the steering system reports it is engaged / in a state
        /// that allows an external guidance command to steer.
        steering_ready: bool,
        /// Raw guidance limit status (0 = not limited; non-zero = at a limit
        /// or fault — see ISO 11783-7 agricultural guidance).
        limit_status: u8,
    },
}
