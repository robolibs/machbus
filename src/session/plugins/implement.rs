//! ISO 11783-7 implement messages (hitch / PTO / aux-valve / speed / lighting /
//! tractor-facilities) as a [`Plugin`]. Decodes inbound command/status frames
//! into a cache + [`ImplementEvent`]s, and sends commands/status on command.

use crate::isobus::implement::tractor_commands::{
    AuxValveCommandMsg, HitchCommand, HitchCommandMsg, PtoCommand, PtoCommandMsg, ValveCommand,
};
use crate::isobus::implement::{
    GroundBasedSpeedDist, HitchStatus, LightingState, MAX_AUX_VALVES, MachineSelectedSpeedFull,
    PtoStatus, TractorFacilities, TractorFacilitiesRole, WheelBasedSpeedDist,
};
use crate::net::pgn_defs::{
    PGN_AUX_VALVE_CMD, PGN_FRONT_HITCH, PGN_FRONT_HITCH_CMD, PGN_FRONT_PTO, PGN_FRONT_PTO_CMD,
    PGN_GROUND_BASED_SPEED_DIST, PGN_LIGHTING_COMMAND, PGN_LIGHTING_DATA,
    PGN_MACHINE_SELECTED_SPEED, PGN_REAR_HITCH, PGN_REAR_HITCH_CMD, PGN_REAR_PTO, PGN_REAR_PTO_CMD,
    PGN_REQUIRED_TRACTOR_FACILITIES, PGN_TRACTOR_FACILITIES_RESPONSE, PGN_WHEEL_BASED_SPEED_DIST,
};
use crate::net::{Address, BROADCAST_ADDRESS, Error, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, Hitch, ImplementEvent, Pto};
use crate::time::Instant;
use core::any::Any;

const A: Pgn = PGN_AUX_VALVE_CMD;
const INTERESTS: &[Pgn] = &[
    PGN_FRONT_HITCH_CMD,
    PGN_REAR_HITCH_CMD,
    PGN_FRONT_PTO_CMD,
    PGN_REAR_PTO_CMD,
    PGN_FRONT_HITCH,
    PGN_REAR_HITCH,
    PGN_FRONT_PTO,
    PGN_REAR_PTO,
    PGN_WHEEL_BASED_SPEED_DIST,
    PGN_GROUND_BASED_SPEED_DIST,
    PGN_MACHINE_SELECTED_SPEED,
    PGN_LIGHTING_DATA,
    PGN_LIGHTING_COMMAND,
    PGN_TRACTOR_FACILITIES_RESPONSE,
    PGN_REQUIRED_TRACTOR_FACILITIES,
    A,
    A + 1,
    A + 2,
    A + 3,
    A + 4,
    A + 5,
    A + 6,
    A + 7,
    A + 8,
    A + 9,
    A + 10,
    A + 11,
    A + 12,
    A + 13,
    A + 14,
    A + 15,
];

/// Implement-message plugin.
#[derive(Default)]
pub struct Implement {
    front_hitch: Option<HitchCommandMsg>,
    rear_hitch: Option<HitchCommandMsg>,
    front_pto: Option<PtoCommandMsg>,
    rear_pto: Option<PtoCommandMsg>,
    front_hitch_status: Option<HitchStatus>,
    rear_hitch_status: Option<HitchStatus>,
    front_pto_status: Option<PtoStatus>,
    rear_pto_status: Option<PtoStatus>,
    wheel_speed: Option<WheelBasedSpeedDist>,
    ground_speed: Option<GroundBasedSpeedDist>,
    machine_selected_speed: Option<MachineSelectedSpeedFull>,
    lighting_data: Option<LightingState>,
    lighting_command: Option<LightingState>,
    pending: Vec<(Pgn, Vec<u8>, Address)>,
}

impl Implement {
    /// Create an implement-message plugin.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Send a hitch command (broadcast).
    pub fn command_hitch(&mut self, hitch: Hitch, command: HitchCommand) {
        let msg = HitchCommandMsg {
            command,
            ..HitchCommandMsg::default()
        };
        self.queue(hitch.cmd_pgn(), msg.encode().to_vec(), BROADCAST_ADDRESS);
    }

    /// Send a hitch position command (raw target + rate).
    pub fn command_hitch_position(&mut self, hitch: Hitch, target_position: u16, rate: u8) {
        let msg = HitchCommandMsg {
            command: HitchCommand::Position,
            target_position,
            rate,
        };
        self.queue(hitch.cmd_pgn(), msg.encode().to_vec(), BROADCAST_ADDRESS);
    }

    /// Send a PTO command (broadcast).
    pub fn command_pto(&mut self, pto: Pto, command: PtoCommand) {
        let msg = PtoCommandMsg {
            command,
            ..PtoCommandMsg::default()
        };
        self.queue(pto.cmd_pgn(), msg.encode().to_vec(), BROADCAST_ADDRESS);
    }

    /// Send a PTO speed command.
    pub fn command_pto_speed(&mut self, pto: Pto, rpm: u16, ramp_rate: u8) {
        let msg = PtoCommandMsg {
            command: PtoCommand::SetSpeed,
            target_speed_rpm: rpm,
            ramp_rate,
        };
        self.queue(pto.cmd_pgn(), msg.encode().to_vec(), BROADCAST_ADDRESS);
    }

    /// Send an auxiliary-valve command.
    ///
    /// # Errors
    /// Returns an error if `valve_index` is out of range.
    pub fn command_aux_valve(
        &mut self,
        valve_index: u8,
        command: ValveCommand,
        flow_rate: u16,
    ) -> Result<()> {
        if valve_index >= MAX_AUX_VALVES {
            return Err(Error::invalid_data(format!(
                "aux valve index {valve_index} out of range 0..={}",
                MAX_AUX_VALVES - 1
            )));
        }
        let msg = AuxValveCommandMsg {
            valve_index,
            command,
            flow_rate,
        };
        let pgn = msg
            .try_pgn()
            .expect("valve_index validated before PGN construction");
        self.queue(pgn, msg.encode().to_vec(), BROADCAST_ADDRESS);
        Ok(())
    }

    /// Broadcast hitch status feedback.
    pub fn broadcast_hitch_status(&mut self, hitch: Hitch, mut status: HitchStatus) {
        status.is_rear = matches!(hitch, Hitch::Rear);
        self.queue(status.pgn(), status.encode().to_vec(), BROADCAST_ADDRESS);
    }

    /// Broadcast PTO status feedback.
    pub fn broadcast_pto_status(&mut self, pto: Pto, mut status: PtoStatus) {
        status.is_rear = matches!(pto, Pto::Rear);
        self.queue(status.pgn(), status.encode().to_vec(), BROADCAST_ADDRESS);
    }

    /// Broadcast wheel-based speed/distance.
    pub fn broadcast_wheel_speed(&mut self, msg: WheelBasedSpeedDist) {
        self.queue(
            PGN_WHEEL_BASED_SPEED_DIST,
            msg.encode().to_vec(),
            BROADCAST_ADDRESS,
        );
    }

    /// Broadcast ground-based speed/distance.
    pub fn broadcast_ground_speed(&mut self, msg: GroundBasedSpeedDist) {
        self.queue(
            PGN_GROUND_BASED_SPEED_DIST,
            msg.encode().to_vec(),
            BROADCAST_ADDRESS,
        );
    }

    /// Broadcast full machine-selected-speed.
    pub fn broadcast_machine_selected_speed(&mut self, msg: MachineSelectedSpeedFull) {
        self.queue(
            PGN_MACHINE_SELECTED_SPEED,
            msg.encode().to_vec(),
            BROADCAST_ADDRESS,
        );
    }

    /// Broadcast lighting data.
    pub fn broadcast_lighting_data(&mut self, state: LightingState) {
        self.queue(
            PGN_LIGHTING_DATA,
            state.encode().to_vec(),
            BROADCAST_ADDRESS,
        );
    }

    /// Broadcast lighting command.
    pub fn command_lighting(&mut self, state: LightingState) {
        self.queue(
            PGN_LIGHTING_COMMAND,
            state.encode().to_vec(),
            BROADCAST_ADDRESS,
        );
    }

    /// Broadcast the TECU-supported tractor facilities payload.
    pub fn broadcast_tractor_facilities_response(&mut self, facilities: TractorFacilities) {
        self.queue(
            TractorFacilitiesRole::Response.pgn(),
            facilities.encode().to_vec(),
            BROADCAST_ADDRESS,
        );
    }

    /// Broadcast the implement-required tractor facilities payload.
    pub fn request_tractor_facilities(&mut self, facilities: TractorFacilities) {
        self.queue(
            TractorFacilitiesRole::Required.pgn(),
            facilities.encode().to_vec(),
            BROADCAST_ADDRESS,
        );
    }

    /// Most recent front-hitch status observed.
    #[must_use]
    pub fn last_front_hitch_status(&self) -> Option<HitchStatus> {
        self.front_hitch_status
    }

    /// Most recent rear-PTO status observed.
    #[must_use]
    pub fn last_rear_pto_status(&self) -> Option<PtoStatus> {
        self.rear_pto_status
    }

    /// Most recent wheel-based speed observed.
    #[must_use]
    pub fn last_wheel_speed(&self) -> Option<WheelBasedSpeedDist> {
        self.wheel_speed
    }

    fn queue(&mut self, pgn: Pgn, data: Vec<u8>, dst: Address) {
        self.pending.push((pgn, data, dst));
    }
}

impl Plugin for Implement {
    fn name(&self) -> &'static str {
        "implement"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    #[allow(clippy::too_many_lines)]
    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        let p = msg.pgn;
        if !msg.has_usable_envelope_for_pgn(p) {
            return;
        }
        match p {
            _ if p == PGN_FRONT_HITCH_CMD || p == PGN_REAR_HITCH_CMD => {
                if let Some(decoded) = HitchCommandMsg::decode(&msg.data) {
                    let hitch = if p == PGN_FRONT_HITCH_CMD {
                        self.front_hitch = Some(decoded);
                        Hitch::Front
                    } else {
                        self.rear_hitch = Some(decoded);
                        Hitch::Rear
                    };
                    ctx.emit(Event::Imp(ImplementEvent::HitchCommand {
                        hitch,
                        msg: decoded,
                    }));
                }
            }
            _ if p == PGN_FRONT_PTO_CMD || p == PGN_REAR_PTO_CMD => {
                if let Some(decoded) = PtoCommandMsg::decode(&msg.data) {
                    let pto = if p == PGN_FRONT_PTO_CMD {
                        self.front_pto = Some(decoded);
                        Pto::Front
                    } else {
                        self.rear_pto = Some(decoded);
                        Pto::Rear
                    };
                    ctx.emit(Event::Imp(ImplementEvent::PtoCommand { pto, msg: decoded }));
                }
            }
            _ if (A..=A + 15).contains(&p) => {
                if let Some(mut decoded) = AuxValveCommandMsg::decode(&msg.data) {
                    decoded.valve_index = (p - A) as u8;
                    ctx.emit(Event::Imp(ImplementEvent::AuxValveCommand(decoded)));
                }
            }
            _ if p == PGN_FRONT_HITCH || p == PGN_REAR_HITCH => {
                let hitch = if p == PGN_FRONT_HITCH {
                    Hitch::Front
                } else {
                    Hitch::Rear
                };
                if let Some(decoded) = HitchStatus::decode(&msg.data, hitch == Hitch::Rear) {
                    if hitch == Hitch::Front {
                        self.front_hitch_status = Some(decoded);
                    } else {
                        self.rear_hitch_status = Some(decoded);
                    }
                    ctx.emit(Event::Imp(ImplementEvent::HitchStatus {
                        hitch,
                        msg: decoded,
                    }));
                }
            }
            _ if p == PGN_FRONT_PTO || p == PGN_REAR_PTO => {
                let pto = if p == PGN_FRONT_PTO {
                    Pto::Front
                } else {
                    Pto::Rear
                };
                if let Some(decoded) = PtoStatus::decode(&msg.data, pto == Pto::Rear) {
                    if pto == Pto::Front {
                        self.front_pto_status = Some(decoded);
                    } else {
                        self.rear_pto_status = Some(decoded);
                    }
                    ctx.emit(Event::Imp(ImplementEvent::PtoStatus { pto, msg: decoded }));
                }
            }
            _ if p == PGN_WHEEL_BASED_SPEED_DIST => {
                if let Some(decoded) = WheelBasedSpeedDist::decode(&msg.data) {
                    self.wheel_speed = Some(decoded);
                    ctx.emit(Event::Imp(ImplementEvent::WheelSpeed(decoded)));
                }
            }
            _ if p == PGN_GROUND_BASED_SPEED_DIST => {
                if let Some(decoded) = GroundBasedSpeedDist::decode(&msg.data) {
                    self.ground_speed = Some(decoded);
                    ctx.emit(Event::Imp(ImplementEvent::GroundSpeed(decoded)));
                }
            }
            _ if p == PGN_MACHINE_SELECTED_SPEED => {
                if let Some(decoded) = MachineSelectedSpeedFull::decode(&msg.data) {
                    self.machine_selected_speed = Some(decoded);
                    ctx.emit(Event::Imp(ImplementEvent::MachineSelectedSpeed(decoded)));
                }
            }
            _ if p == PGN_LIGHTING_DATA || p == PGN_LIGHTING_COMMAND => {
                if let Some(decoded) = LightingState::decode(&msg.data) {
                    let command = p == PGN_LIGHTING_COMMAND;
                    if command {
                        self.lighting_command = Some(decoded);
                    } else {
                        self.lighting_data = Some(decoded);
                    }
                    ctx.emit(Event::Imp(ImplementEvent::Lighting {
                        command,
                        source: msg.source,
                        state: decoded,
                    }));
                }
            }
            _ if p == PGN_TRACTOR_FACILITIES_RESPONSE || p == PGN_REQUIRED_TRACTOR_FACILITIES => {
                if let Some(decoded) = TractorFacilities::decode(&msg.data) {
                    let role = if p == PGN_TRACTOR_FACILITIES_RESPONSE {
                        TractorFacilitiesRole::Response
                    } else {
                        TractorFacilitiesRole::Required
                    };
                    ctx.emit(Event::Imp(ImplementEvent::TractorFacilities {
                        role,
                        source: msg.source,
                        facilities: decoded,
                    }));
                }
            }
            _ => {}
        }
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        for (pgn, data, dst) in self.pending.drain(..) {
            ctx.send(pgn, data, dst, Priority::Default);
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
