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
    /// No Machine Info has arrived within the link timeout, so the steering ECU
    /// is presumed gone. The controller has been forced to the safe state:
    /// curvature zeroed and the command status set to *not intended to steer*.
    ///
    /// This is the ISO 11783-7 §8.2 loss-of-communication reaction. Without it
    /// a controller keeps streaming *intended to steer* at the last commanded
    /// curvature indefinitely after the ECU stops answering.
    LinkLost {
        /// Milliseconds since the last Machine Info was received.
        silent_for_ms: u32,
        /// `true` when the controller was actually engaged at the moment the
        /// link dropped, i.e. this interrupted live steering.
        was_engaged: bool,
    },
    /// A Machine Info arrived after the link had been declared lost.
    LinkRestored { source: Address },
    /// An operator pressed the Auxiliary Shortcut Button (stop all implement
    /// operations). The controller has latched the stop and entered the safe
    /// state; it will not steer again until the latch is explicitly cleared.
    StopRequested {
        /// `true` when this interrupted live steering.
        was_engaged: bool,
    },
}
