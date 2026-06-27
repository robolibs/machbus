//! J1939 engine/transmission (powertrain) as a [`Plugin`]. Decodes the engine,
//! temperature, fuel, and transmission PGNs into a cached snapshot + events, and
//! broadcasts common frames on command. Reuses the stack's decode logic so there
//! is one source of truth for the PGN set.

use crate::j1939::{Eec1, Etc1, VehicleIdentification};
use crate::net::pgn_defs::{
    PGN_AMBIENT_CONDITIONS, PGN_AT1, PGN_AT2, PGN_COMPONENT_ID, PGN_CRUISE_CONTROL,
    PGN_DASH_DISPLAY, PGN_EEC1, PGN_EEC2, PGN_EEC3, PGN_EFLP, PGN_ENGINE_HOURS, PGN_ET1, PGN_ET2,
    PGN_ETC1, PGN_FUEL_CONSUMPTION, PGN_FUEL_ECONOMY, PGN_TSC1, PGN_VEHICLE_ID,
    PGN_VEHICLE_POSITION, PGN_VEP1,
};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::Event;
use crate::session::sys::powertrain::{PowertrainSnapshot, decode_and_cache};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[
    PGN_EEC1,
    PGN_EEC2,
    PGN_EEC3,
    PGN_ET1,
    PGN_ET2,
    PGN_EFLP,
    PGN_ENGINE_HOURS,
    PGN_FUEL_ECONOMY,
    PGN_FUEL_CONSUMPTION,
    PGN_TSC1,
    PGN_VEP1,
    PGN_AMBIENT_CONDITIONS,
    PGN_AT1,
    PGN_AT2,
    PGN_DASH_DISPLAY,
    PGN_VEHICLE_POSITION,
    PGN_COMPONENT_ID,
    PGN_VEHICLE_ID,
    PGN_ETC1,
    PGN_CRUISE_CONTROL,
];

/// Powertrain (J1939 engine/transmission) plugin.
#[derive(Default)]
pub struct Powertrain {
    snapshot: PowertrainSnapshot,
    pending: Vec<(Pgn, Vec<u8>)>,
}

impl Powertrain {
    /// Create a powertrain plugin.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Latest decoded snapshot of all supported PGNs.
    #[must_use]
    pub fn snapshot(&self) -> &PowertrainSnapshot {
        &self.snapshot
    }

    /// Queue an EEC1 broadcast (flushed on tick).
    pub fn broadcast_eec1(&mut self, data: &Eec1) {
        self.pending.push((PGN_EEC1, data.encode().to_vec()));
    }

    /// Queue an ETC1 broadcast (flushed on tick).
    pub fn broadcast_etc1(&mut self, data: &Etc1) {
        self.pending.push((PGN_ETC1, data.encode().to_vec()));
    }

    /// Queue a vehicle-identification broadcast (flushed on tick).
    pub fn broadcast_vehicle_identification(&mut self, data: &VehicleIdentification) {
        self.pending.push((PGN_VEHICLE_ID, data.encode().to_vec()));
    }
}

impl Plugin for Powertrain {
    fn name(&self) -> &'static str {
        "powertrain"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        for event in decode_and_cache(&mut self.snapshot, msg) {
            ctx.emit(Event::Powertrain(event));
        }
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        for (pgn, payload) in self.pending.drain(..) {
            ctx.send(pgn, payload, BROADCAST_ADDRESS, Priority::Default);
        }
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
