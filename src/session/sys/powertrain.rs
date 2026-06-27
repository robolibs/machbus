//! `stack.powertrain()` / `stack.engine()` / `stack.transmission()` —
//! J1939 engine and powertrain convenience routing.
//!
//! The low-level J1939 engine/transmission modules remain pump-style codecs.
//! This subsystem wires their PGNs into the high-level [`Stack`] event queue,
//! caches the latest decoded values, and provides broadcast helpers for common
//! engine/transmission frames.

use crate::j1939::{
    Aftertreatment1, Aftertreatment2, AmbientConditions, ComponentIdentification, CruiseControl,
    DashDisplay, Eec1, Eec2, Eec3, EngineFluidLp, EngineHours, EngineTemp1, EngineTemp2, Etc1,
    FuelConsumption, FuelEconomy, TransmissionOilTemp, Tsc1, VehicleIdentification,
    VehiclePosition, Vep1,
};
use crate::net::message::Message;
use crate::net::pgn_defs::{
    PGN_AMBIENT_CONDITIONS, PGN_AT1, PGN_AT2, PGN_COMPONENT_ID, PGN_CRUISE_CONTROL,
    PGN_DASH_DISPLAY, PGN_EEC1, PGN_EEC2, PGN_EEC3, PGN_EFLP, PGN_ENGINE_HOURS, PGN_ET1, PGN_ET2,
    PGN_ETC1, PGN_FUEL_CONSUMPTION, PGN_FUEL_ECONOMY, PGN_TSC1, PGN_VEHICLE_ID,
    PGN_VEHICLE_POSITION, PGN_VEP1,
};
use crate::net::types::Address;

/// Decoded J1939 engine/transmission event.
#[derive(Debug, Clone, PartialEq)]
pub enum PowertrainEvent {
    Eec1 {
        source: Address,
        data: Eec1,
    },
    Eec2 {
        source: Address,
        data: Eec2,
    },
    Eec3 {
        source: Address,
        data: Eec3,
    },
    EngineTemp1 {
        source: Address,
        data: EngineTemp1,
    },
    EngineTemp2 {
        source: Address,
        data: EngineTemp2,
    },
    EngineFluidLp {
        source: Address,
        data: EngineFluidLp,
    },
    EngineHours {
        source: Address,
        data: EngineHours,
    },
    FuelEconomy {
        source: Address,
        data: FuelEconomy,
    },
    FuelConsumption {
        source: Address,
        data: FuelConsumption,
    },
    Tsc1 {
        source: Address,
        data: Tsc1,
    },
    Vep1 {
        source: Address,
        data: Vep1,
    },
    AmbientConditions {
        source: Address,
        data: AmbientConditions,
    },
    DashDisplay {
        source: Address,
        data: DashDisplay,
    },
    VehiclePosition {
        source: Address,
        data: VehiclePosition,
    },
    Aftertreatment1 {
        source: Address,
        data: Aftertreatment1,
    },
    Aftertreatment2 {
        source: Address,
        data: Aftertreatment2,
    },
    ComponentIdentification {
        source: Address,
        data: ComponentIdentification,
    },
    VehicleIdentification {
        source: Address,
        data: VehicleIdentification,
    },
    Etc1 {
        source: Address,
        data: Etc1,
    },
    TransmissionOilTemp {
        source: Address,
        data: TransmissionOilTemp,
    },
    CruiseControl {
        source: Address,
        data: CruiseControl,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PowertrainSnapshot {
    pub eec1: Option<Eec1>,
    pub eec2: Option<Eec2>,
    pub eec3: Option<Eec3>,
    pub engine_temp1: Option<EngineTemp1>,
    pub engine_temp2: Option<EngineTemp2>,
    pub engine_fluid_lp: Option<EngineFluidLp>,
    pub engine_hours: Option<EngineHours>,
    pub fuel_economy: Option<FuelEconomy>,
    pub fuel_consumption: Option<FuelConsumption>,
    pub tsc1: Option<Tsc1>,
    pub vep1: Option<Vep1>,
    pub ambient_conditions: Option<AmbientConditions>,
    pub dash_display: Option<DashDisplay>,
    pub vehicle_position: Option<VehiclePosition>,
    pub aftertreatment1: Option<Aftertreatment1>,
    pub aftertreatment2: Option<Aftertreatment2>,
    pub component_identification: Option<ComponentIdentification>,
    pub vehicle_identification: Option<VehicleIdentification>,
    pub etc1: Option<Etc1>,
    pub transmission_oil_temp: Option<TransmissionOilTemp>,
    pub cruise_control: Option<CruiseControl>,
}

pub(crate) fn decode_and_cache(
    snapshot: &mut PowertrainSnapshot,
    msg: &Message,
) -> Vec<PowertrainEvent> {
    let mut events = Vec::new();
    let source = msg.source;
    if !msg.has_usable_envelope_for_pgn(msg.pgn) {
        return events;
    }
    match msg.pgn {
        PGN_EEC1 => push_copy(
            &mut snapshot.eec1,
            Eec1::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::Eec1 { source, data },
        ),
        PGN_EEC2 => push_copy(
            &mut snapshot.eec2,
            Eec2::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::Eec2 { source, data },
        ),
        PGN_EEC3 => push_copy(
            &mut snapshot.eec3,
            Eec3::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::Eec3 { source, data },
        ),
        PGN_ET1 => push_copy(
            &mut snapshot.engine_temp1,
            EngineTemp1::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::EngineTemp1 { source, data },
        ),
        PGN_ET2 => {
            push_copy(
                &mut snapshot.engine_temp2,
                EngineTemp2::decode(&msg.data),
                source,
                &mut events,
                |source, data| PowertrainEvent::EngineTemp2 { source, data },
            );
            push_copy(
                &mut snapshot.transmission_oil_temp,
                TransmissionOilTemp::decode(&msg.data),
                source,
                &mut events,
                |source, data| PowertrainEvent::TransmissionOilTemp { source, data },
            );
        }
        PGN_EFLP => push_copy(
            &mut snapshot.engine_fluid_lp,
            EngineFluidLp::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::EngineFluidLp { source, data },
        ),
        PGN_ENGINE_HOURS => push_copy(
            &mut snapshot.engine_hours,
            EngineHours::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::EngineHours { source, data },
        ),
        PGN_FUEL_ECONOMY => push_copy(
            &mut snapshot.fuel_economy,
            FuelEconomy::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::FuelEconomy { source, data },
        ),
        PGN_FUEL_CONSUMPTION => push_copy(
            &mut snapshot.fuel_consumption,
            FuelConsumption::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::FuelConsumption { source, data },
        ),
        PGN_TSC1 => push_copy(
            &mut snapshot.tsc1,
            Tsc1::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::Tsc1 { source, data },
        ),
        PGN_VEP1 => push_copy(
            &mut snapshot.vep1,
            Vep1::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::Vep1 { source, data },
        ),
        PGN_AMBIENT_CONDITIONS => push_copy(
            &mut snapshot.ambient_conditions,
            AmbientConditions::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::AmbientConditions { source, data },
        ),
        PGN_AT1 => push_copy(
            &mut snapshot.aftertreatment1,
            Aftertreatment1::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::Aftertreatment1 { source, data },
        ),
        PGN_AT2 => push_copy(
            &mut snapshot.aftertreatment2,
            Aftertreatment2::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::Aftertreatment2 { source, data },
        ),
        PGN_DASH_DISPLAY => push_copy(
            &mut snapshot.dash_display,
            DashDisplay::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::DashDisplay { source, data },
        ),
        PGN_VEHICLE_POSITION => push_copy(
            &mut snapshot.vehicle_position,
            VehiclePosition::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::VehiclePosition { source, data },
        ),
        PGN_COMPONENT_ID => push_clone(
            &mut snapshot.component_identification,
            ComponentIdentification::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::ComponentIdentification { source, data },
        ),
        PGN_VEHICLE_ID => push_clone(
            &mut snapshot.vehicle_identification,
            VehicleIdentification::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::VehicleIdentification { source, data },
        ),
        PGN_ETC1 => push_copy(
            &mut snapshot.etc1,
            Etc1::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::Etc1 { source, data },
        ),
        PGN_CRUISE_CONTROL => push_copy(
            &mut snapshot.cruise_control,
            CruiseControl::decode(&msg.data),
            source,
            &mut events,
            |source, data| PowertrainEvent::CruiseControl { source, data },
        ),
        _ => {}
    }
    events
}

fn push_copy<T: Copy>(
    slot: &mut Option<T>,
    decoded: Option<T>,
    source: Address,
    events: &mut Vec<PowertrainEvent>,
    event: impl FnOnce(Address, T) -> PowertrainEvent,
) {
    if let Some(data) = decoded {
        *slot = Some(data);
        events.push(event(source, data));
    }
}

fn push_clone<T: Clone>(
    slot: &mut Option<T>,
    decoded: Option<T>,
    source: Address,
    events: &mut Vec<PowertrainEvent>,
    event: impl FnOnce(Address, T) -> PowertrainEvent,
) {
    if let Some(data) = decoded {
        *slot = Some(data.clone());
        events.push(event(source, data));
    }
}
