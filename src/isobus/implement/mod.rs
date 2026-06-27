//! ISO 11783-7/9 implement-side message codecs.
//!
//! Mirrors the C++ `machbus::isobus::implement::*` namespace. The C++
//! `*Interface` classes (each embedding `IsoNet&`) are intentionally
//! not ported — users compose codecs with
//! `IsoNet::register_pgn_callback` / `IsoNet::send` directly. See
//! `book/src/reference/behavior-differences.md`.

pub mod aux_valve_status;
pub mod drive_strategy;
pub mod guidance;
pub mod inventory;
pub mod lighting;
pub mod machine_speed_cmd;
pub mod speed_distance;
pub mod tractor_commands;
pub mod tractor_facilities;

pub use aux_valve_status::{
    AuxValveFlowMsg, MAX_AUX_VALVES, ValveFailSafe, ValveLimitStatus, ValveState,
    estimated_flow_pgn, measured_flow_pgn,
};
pub use drive_strategy::{
    CurvatureCommandStatus, DriveStrategyCmd, DriveStrategyMode, GuidanceSystemCmd,
    HitchPtoCombinedCmd, HitchRollPitchCmd,
};
pub use guidance::{
    CURVATURE_MAX_PER_KM, CURVATURE_MIN_PER_KM, CurvatureCommand, GenericSaeBs02SlotValue,
    GuidanceLimitStatus, GuidanceMachineInfo, GuidanceSystemStatus, MechanicalLockout,
    RequestResetCommandStatus, SteeringReadiness, curvature_within_range,
};
pub use inventory::{
    FamilyLevel, IMPLEMENT_FAMILIES, ImplementFamilyInfo, ImplementMessageFamily, family_info,
};
pub use lighting::{LightState, LightingController, LightingState};
pub use machine_speed_cmd::{
    MachineDirection, MachineSelectedSpeedMsg, MachineSpeedCommandMsg, SpeedExitCode, SpeedSource,
};
pub use speed_distance::{
    ExitReasonCode, GroundBasedSpeedDist, HitchStatus, LimitStatus, MachineSelectedSpeedFull,
    PtoStatus, WheelBasedSpeedDist, wheel_slip_percent,
};
pub use tractor_commands::{
    AuxValveCommandMsg, HitchCommand, HitchCommandMsg, PtoCommand, PtoCommandMsg,
    TractorControlModeMsg, TractorMode, ValveCommand,
};
pub use tractor_facilities::{
    FacilityGroup, RequiredFacilitiesAggregator, TECU_FACILITY_MATRIX, TecuClass,
    TractorFacilities, TractorFacilitiesRole, facilities_in,
};
