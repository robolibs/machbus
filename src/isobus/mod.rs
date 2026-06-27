//! ISO 11783 application-layer modules.
//!
//! Mirrors the C++ `machbus::isobus::*` namespace. Phase 10 covers
//! the seven top-level files: auxiliary, file_transfer (legacy),
//! functionalities, group_function, guidance, tim, tractor_ecu.
//! Phases 11–15 add the subnamespaces: `implement/`, `sc/`, `vt/`,
//! `tc/`, `fs/`.
//!
//! The C++ "Interface" / "Server" / "Client" / "Protocol" classes
//! that embed `IsoNet&` are intentionally not ported (see
//! `book/src/reference/behavior-differences.md`) — users compose the wire codecs with
//! `IsoNet::register_pgn_callback` / `IsoNet::send` directly.

pub mod auxiliary;
#[cfg(feature = "default")]
pub mod conformance;
pub mod file_transfer;
pub mod fs;
pub mod functionalities;
pub mod group_function;
pub mod guidance;
pub mod implement;
pub mod sc;
pub mod tc;
pub mod tim;
#[cfg(feature = "default")]
pub mod tractor_ecu;
pub mod vt;

pub use auxiliary::{AuxFunctionState, AuxFunctionType, AuxNFunction, AuxOFunction};
pub use file_transfer::{
    FILE_SERVER_BUSY_STATUS_INTERVAL_MS, FILE_SERVER_STATUS_INTERVAL_MS, FS_REQUEST_TIMEOUT_MS,
    FileAttribute, FileClientState, FileOperation, FileProperties, FileServerConfig,
    FileServerState, FileTransferError, OpenFileState, VolumeInfo,
};
pub use functionalities::{
    AuxNOptions, AuxOOptions, BasicTractorEcuOptions, Functionalities, Functionality,
    FunctionalityData, MinimumControlFunctionOptions, TaskControllerGeoServerOptions,
    TractorImplementManagementOptions,
};
pub use group_function::{
    GroupFunctionError, GroupFunctionMsg, GroupFunctionResponder, GroupFunctionSupport,
    GroupFunctionType,
};
pub use guidance::GuidanceData;
pub use implement::{
    AuxValveCommandMsg, AuxValveFlowMsg, CURVATURE_MAX_PER_KM, CURVATURE_MIN_PER_KM,
    CurvatureCommand, CurvatureCommandStatus, DriveStrategyCmd, DriveStrategyMode, ExitReasonCode,
    FacilityGroup, FamilyLevel, GenericSaeBs02SlotValue, GroundBasedSpeedDist, GuidanceLimitStatus,
    GuidanceMachineInfo, GuidanceSystemCmd, GuidanceSystemStatus, HitchCommand, HitchCommandMsg,
    HitchPtoCombinedCmd, HitchRollPitchCmd, HitchStatus, IMPLEMENT_FAMILIES, ImplementFamilyInfo,
    ImplementMessageFamily, LightState, LightingController, LightingState, LimitStatus,
    MachineDirection, MachineSelectedSpeedFull, MachineSelectedSpeedMsg, MachineSpeedCommandMsg,
    MechanicalLockout, PtoCommand, PtoCommandMsg, PtoStatus, RequestResetCommandStatus,
    RequiredFacilitiesAggregator, SpeedExitCode, SpeedSource, SteeringReadiness,
    TECU_FACILITY_MATRIX, TecuClass, TractorControlModeMsg, TractorFacilities,
    TractorFacilitiesRole, TractorMode, ValveCommand, ValveFailSafe, ValveLimitStatus, ValveState,
    WheelBasedSpeedDist, curvature_within_range, estimated_flow_pgn, facilities_in, family_info,
    measured_flow_pgn, wheel_slip_percent,
};
pub use sc::{
    SC_MAX_SEQUENCE_STEP_ID, SC_MSG_CODE_CLIENT, SC_MSG_CODE_MASTER, SC_STATUS_ACTIVE_RATE_MS,
    SC_STATUS_MIN_SPACING_MS, SC_STATUS_TIMEOUT_ACTIVE_MS, SC_STATUS_TIMEOUT_READY_MS, SC_TAN_MAX,
    SC_TAN_MIN, SC_TAN_NOT_AVAILABLE, SC_TAN_REPEAT_MS, SCClient, SCClientConfig,
    SCClientFuncError, SCClientState, SCCommand, SCD_LABEL_NONE, SCMaster, SCMasterConfig,
    SCMasterState, SCSequenceState, SCState, ScdAction, ScdLabel, SequenceRecorder, SequenceStep,
    SequenceTanTracker, scd_action,
};
pub use tim::{
    AuxValve, AuxValveCommand, HitchState, MAX_AUX_VALVES, MAX_HITCH_POSITION, PtoState,
    TIM_OPTION_BYTES, TIM_OPTION_DEFINED_MASK, TIM_UPDATE_INTERVAL_MS, TimArbitration,
    TimAuthority, TimAuthorityArbiter, TimAuthorityState, TimCommand, TimInterlock, TimInterlocks,
    TimOption, TimOptionSet, TimValidationError,
};
#[cfg(feature = "default")]
pub use tractor_ecu::{
    PowerConfig, PowerState, SafeModeTrigger, TecuClassification, TecuCommandKind, TecuConfig,
    TecuExclusiveMessage, TecuMaintainPowerRequest, TecuSafeMode,
};

#[cfg(test)]
mod arbitrary_decode_tests {
    use super::*;
    use crate::isobus::implement::{
        AuxValveCommandMsg, AuxValveFlowMsg, CurvatureCommand, DriveStrategyCmd,
        GroundBasedSpeedDist, GuidanceMachineInfo, GuidanceSystemCmd, GuidanceSystemStatus,
        HitchCommandMsg, HitchPtoCombinedCmd, HitchRollPitchCmd, HitchStatus, LightingState,
        MachineSelectedSpeedFull, MachineSelectedSpeedMsg, MachineSpeedCommandMsg, PtoCommandMsg,
        PtoStatus, TractorControlModeMsg, TractorFacilities, WheelBasedSpeedDist,
    };
    use crate::net::Message;
    use crate::net::pgn_defs::{
        PGN_AUX_INPUT_STATUS, PGN_AUX_INPUT_TYPE2, PGN_AUX_VALVE_0_7, PGN_FRONT_HITCH,
        PGN_FRONT_PTO, PGN_GUIDANCE_MACHINE, PGN_REAR_HITCH, PGN_REAR_PTO,
    };
    use proptest::prelude::*;

    fn msg(pgn: u32, data: &[u8]) -> Message {
        Message::new(pgn, data.to_vec(), 0x80)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn isobus_application_decoders_accept_or_reject_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=96),
            valve_index in any::<u8>(),
        ) {
            let aux_o = msg(PGN_AUX_INPUT_STATUS, &data);
            let aux_n = msg(PGN_AUX_INPUT_TYPE2, &data);
            let tim_pto = msg(PGN_FRONT_PTO, &data);
            let tim_hitch = msg(PGN_FRONT_HITCH, &data);
            let tim_aux = msg(PGN_AUX_VALVE_0_7, &data);
            let guidance = msg(PGN_GUIDANCE_MACHINE, &data);

            let _ = AuxOFunction::decode(&aux_o);
            let _ = AuxNFunction::decode(&aux_n);
            let _ = GroupFunctionMsg::decode(&data);
            let _ = GuidanceData::decode(&guidance);

            let _ = PtoState::decode(&tim_pto);
            let _ = HitchState::decode(&tim_hitch);
            let _ = AuxValveCommand::decode(&tim_aux);

            let _ = HitchCommandMsg::decode(&data);
            let _ = PtoCommandMsg::decode(&data);
            let _ = AuxValveCommandMsg::decode(&data);
            let _ = TractorControlModeMsg::decode(&data);
            let _ = WheelBasedSpeedDist::decode(&data);
            let _ = GroundBasedSpeedDist::decode(&data);
            let _ = MachineSelectedSpeedFull::decode(&data);
            let _ = HitchStatus::decode(&data, false);
            let _ = HitchStatus::decode(&data, true);
            let _ = PtoStatus::decode(&data, false);
            let _ = PtoStatus::decode(&data, true);
            let _ = LightingState::decode(&data);
            let _ = AuxValveFlowMsg::decode(&data, valve_index);
            let _ = MachineSelectedSpeedMsg::decode(&data);
            let _ = MachineSpeedCommandMsg::decode(&data);
            let _ = DriveStrategyCmd::decode(&data);
            let _ = GuidanceSystemCmd::decode(&data);
            let _ = HitchPtoCombinedCmd::decode(&data);
            let _ = HitchRollPitchCmd::decode(&data, false);
            let _ = HitchRollPitchCmd::decode(&data, true);
            let _ = CurvatureCommand::decode(&data);
            let _ = GuidanceMachineInfo::decode(&data);
            let _ = GuidanceSystemStatus::decode(&data);
            let _ = TractorFacilities::decode(&data);
        }

        #[test]
        fn successful_fixed_size_isobus_decodes_are_canonical_on_reencode(
            data in any::<[u8; 8]>(),
            valve_index in 0u8..16,
        ) {
            if let Some(decoded) = GroupFunctionMsg::decode(&data) {
                prop_assert_eq!(
                    GroupFunctionMsg::decode(&decoded.encode().expect("decoded Group Function encodes")),
                    Some(decoded)
                );
            }

            let aux_o_msg = msg(PGN_AUX_INPUT_STATUS, &data);
            if let Some(decoded) = AuxOFunction::decode(&aux_o_msg) {
                let encoded = decoded.encode();
                prop_assert_eq!(
                    AuxOFunction::decode(&msg(PGN_AUX_INPUT_STATUS, &encoded)),
                    Some(decoded)
                );
            }
            let aux_n_msg = msg(PGN_AUX_INPUT_TYPE2, &data);
            if let Some(decoded) = AuxNFunction::decode(&aux_n_msg) {
                let encoded = decoded.encode();
                prop_assert_eq!(
                    AuxNFunction::decode(&msg(PGN_AUX_INPUT_TYPE2, &encoded)),
                    Some(decoded)
                );
            }

            if let Some(decoded) = PtoState::decode(&msg(PGN_FRONT_PTO, &data)) {
                prop_assert_eq!(
                    PtoState::decode(&msg(PGN_REAR_PTO, &decoded.encode())),
                    Some(decoded)
                );
            }
            if let Some(decoded) = HitchState::decode(&msg(PGN_FRONT_HITCH, &data)) {
                prop_assert_eq!(
                    HitchState::decode(&msg(PGN_REAR_HITCH, &decoded.encode())),
                    Some(decoded)
                );
            }
            if let Some(decoded) = AuxValveCommand::decode(&msg(PGN_AUX_VALVE_0_7, &data)) {
                prop_assert_eq!(
                    AuxValveCommand::decode(&msg(PGN_AUX_VALVE_0_7, &decoded.encode())),
                    Some(decoded)
                );
            }

            if let Some(decoded) = HitchCommandMsg::decode(&data) {
                prop_assert_eq!(HitchCommandMsg::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = PtoCommandMsg::decode(&data) {
                prop_assert_eq!(PtoCommandMsg::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = AuxValveCommandMsg::decode(&data) {
                prop_assert_eq!(AuxValveCommandMsg::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = TractorControlModeMsg::decode(&data) {
                prop_assert_eq!(TractorControlModeMsg::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = LightingState::decode(&data) {
                prop_assert_eq!(LightingState::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = AuxValveFlowMsg::decode(&data, valve_index) {
                prop_assert_eq!(
                    AuxValveFlowMsg::decode(&decoded.encode(), valve_index),
                    Some(decoded)
                );
            }
            if let Some(decoded) = MachineSelectedSpeedMsg::decode(&data) {
                prop_assert_eq!(MachineSelectedSpeedMsg::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = MachineSpeedCommandMsg::decode(&data) {
                prop_assert_eq!(MachineSpeedCommandMsg::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = DriveStrategyCmd::decode(&data) {
                prop_assert_eq!(DriveStrategyCmd::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = GuidanceSystemCmd::decode(&data) {
                prop_assert_eq!(GuidanceSystemCmd::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = HitchPtoCombinedCmd::decode(&data) {
                prop_assert_eq!(HitchPtoCombinedCmd::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = CurvatureCommand::decode(&data) {
                prop_assert_eq!(CurvatureCommand::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = GuidanceMachineInfo::decode(&data) {
                prop_assert_eq!(GuidanceMachineInfo::decode(&decoded.encode()), Some(decoded));
            }
            if let Some(decoded) = GuidanceSystemStatus::decode(&data) {
                prop_assert_eq!(GuidanceSystemStatus::decode(&decoded.encode()), Some(decoded));
            }
        }
    }
}
