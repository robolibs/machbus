//! J1939 / SAE protocol services.
//!
//! Mirrors the C++ `machbus::j1939::*` namespace. Each module ports
//! the wire codec for one PGN family. The C++ "Interface" classes
//! (which embed `IsoNet&` to register callbacks and send replies)
//! are intentionally not ported — users compose the codecs with
//! [`IsoNet::register_pgn_callback`] and [`IsoNet::send`] directly,
//! matching the pump-style architecture used everywhere else.
//!
//! Phase 9 modules: shortcut_button, time_date, language, proprietary,
//! acknowledgment, pgn_request, transmission, speed_distance, request2.
//! Phases 9b–9f add: dm_memory, maintain_power, heartbeat, engine,
//! diagnostic.
//!
//! [`IsoNet::register_pgn_callback`]: crate::net::IsoNet::register_pgn_callback
//! [`IsoNet::send`]: crate::net::IsoNet::send

pub mod acknowledgment;
pub mod diag_monitor;
pub mod diagnostic;
pub mod diagnostic_inventory;
pub mod dm_memory;
pub mod engine;
pub mod heartbeat;
pub mod language;
pub mod maintain_power;
pub mod network_inventory;
pub mod pgn_request;
pub mod powertrain_inventory;
pub mod proprietary;
pub mod request2;
pub mod shortcut_button;
pub mod speed_distance;
pub(crate) mod text;
pub mod time_date;
pub mod transmission;

pub use acknowledgment::{AckControl, Acknowledgment};
pub use diag_monitor::{DiagnosticMonitor, DtcDelta};
pub use diagnostic::{
    DiagProtocol, DiagnosticLamps, DiagnosticProtocolId, Dm3ClearPreviouslyActiveRequest,
    Dm4Message, Dm5Message, Dm6Message, Dm7Command, Dm8TestResult, Dm9VehicleIdentificationRequest,
    Dm10VehicleIdentification, Dm11ClearActiveRequest, Dm12Message, Dm13Command, Dm13Signals,
    Dm13SuspendSignal, Dm20Response, Dm21Readiness, Dm22Control, Dm22Message, Dm22NackReason,
    Dm23Message, Dm25Request, DmClearAllRequest, DmDtcList, Dtc, Fmi, FreezeFrame, LampFlash,
    LampStatus, MonitorPerformanceRatio, PreviouslyActiveDtc, ProductIdentification,
    SoftwareIdentification, SpnSnapshot,
};
pub use diagnostic_inventory::{DIAGNOSTIC_SERVICES, DmLevel, DmService, service};
pub use dm_memory::{
    Dm14Command, Dm14PointerType, Dm14Request, Dm15Response, Dm15Status, Dm16Transfer,
    EcuIdentification,
};
pub use engine::{
    Aftertreatment1, Aftertreatment2, AmbientConditions, ComponentIdentification, DashDisplay,
    Eec1, Eec2, Eec3, EngineFluidLp, EngineHours, EngineTemp1, EngineTemp2, FuelConsumption,
    FuelEconomy, OverrideControlMode, Tsc1, VehicleIdentification, VehiclePosition, Vep1,
};
pub use heartbeat::{
    HB_COMM_ERROR_TIMEOUT_MS, HB_INTERVAL_MS, HB_MAX_JUMP, HB_RECOVERY_COUNT, HbReceiverState,
    HeartbeatReceiver, HeartbeatRequest, HeartbeatSender, HeartbeatTracker, PGN_HEARTBEAT_REQUEST,
};
pub use language::{
    AreaUnit, DateFormat, DecimalSymbol, DistanceUnit, ForceUnit, LanguageData, MassUnit,
    PressureUnit, TemperatureUnit, TimeFormat, UnitSystem, VolumeUnit,
};
pub use maintain_power::{
    KeySwitchState, MaintainPowerData, MaintainPowerRequest, MaintainPowerRequirement,
    MaintainPowerState, PowerManager, PowerRole, PowerState,
};
pub use network_inventory::{CfRecord, NetworkInventory};
pub use pgn_request::{decode_request, encode_request, requested_pgn};
pub use powertrain_inventory::{
    POWERTRAIN_CLAIM, POWERTRAIN_MESSAGES, PowertrainClaim, PowertrainGroup, PowertrainMessage,
    messages_in,
};
pub use proprietary::{ProprietaryMsg, is_proprietary_pgn, proprietary_b_pgn};
pub use request2::{Request2Msg, Request2Reply, Request2Responder, TransferMsg};
pub use shortcut_button::ShortcutButtonState;
pub use speed_distance::SpeedAndDistance;
pub use time_date::TimeDate;
pub use transmission::{CruiseControl, Etc1, TransmissionOilTemp};

#[cfg(test)]
mod arbitrary_utility_tests {
    use super::*;
    use crate::net::message::Message;
    use crate::net::pgn_defs::{
        PGN_LANGUAGE_COMMAND, PGN_MAINTAIN_POWER, PGN_SHORTCUT_BUTTON, PGN_TIME_DATE,
    };
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_j1939_utility_decoders_accept_or_reject_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=260),
            source in any::<u8>(),
        ) {
            if let Some(pgn) = pgn_request::decode_request(&data) {
                let encoded = pgn_request::encode_request(pgn).expect("valid PGN encodes");
                prop_assert_eq!(pgn_request::decode_request(&encoded), Some(pgn));
            }

            if let Some(ack) = Acknowledgment::decode(&data) {
                let encoded = ack.encode().expect("valid acknowledgment encodes");
                prop_assert_eq!(Acknowledgment::decode(&encoded), Some(ack));
            }

            if let Some(decoded) = TimeDate::decode(&Message::new(PGN_TIME_DATE, data.clone(), source)) {
                let mut expected = decoded;
                expected.timestamp_us = 0;
                let mut decoded_again = TimeDate::decode(&Message::new(
                    PGN_TIME_DATE,
                    decoded.encode().to_vec(),
                    source,
                ))
                .expect("canonical TimeDate re-encode must decode");
                decoded_again.timestamp_us = 0;
                prop_assert_eq!(decoded_again, expected);
            }

            if let Some(decoded) = LanguageData::decode(&Message::new(
                PGN_LANGUAGE_COMMAND,
                data.clone(),
                source,
            )) {
                prop_assert_eq!(
                    LanguageData::decode(&Message::new(
                        PGN_LANGUAGE_COMMAND,
                        decoded.encode().to_vec(),
                        source,
                    )),
                    Some(decoded)
                );
            }

            if let Some(state) = shortcut_button::decode(&Message::new(
                PGN_SHORTCUT_BUTTON,
                data.clone(),
                source,
            )) {
                prop_assert_eq!(
                    shortcut_button::decode(&Message::new(
                        PGN_SHORTCUT_BUTTON,
                        shortcut_button::encode(state).to_vec(),
                        source,
                    )),
                    Some(state)
                );
            }

            if let Some(decoded) = MaintainPowerData::from_message(&Message::new(
                PGN_MAINTAIN_POWER,
                data.clone(),
                source,
            )) {
                let mut expected = decoded;
                expected.timestamp_us = 0;
                let mut decoded_again = MaintainPowerData::decode(&decoded.encode())
                    .expect("canonical MaintainPower re-encode must decode");
                decoded_again.timestamp_us = 0;
                prop_assert_eq!(decoded_again, expected);
            }

            if let Some(decoded) = Request2Msg::decode(&data) {
                let encoded = decoded.encode().expect("decoded Request2 encodes");
                prop_assert_eq!(Request2Msg::decode(&encoded), Some(decoded));
            }

            if let Some(decoded) = TransferMsg::decode(&data) {
                let encoded = decoded.encode().expect("decoded Transfer encodes");
                prop_assert_eq!(TransferMsg::decode(&encoded), Some(decoded));
            }

            if let Some(decoded) = Dm14Request::decode(&data) {
                let encoded = decoded.encode().expect("decoded DM14 encodes");
                prop_assert_eq!(Dm14Request::decode(&encoded), Some(decoded));
            }

            if let Some(decoded) = Dm15Response::decode(&data) {
                let encoded = decoded.encode().expect("decoded DM15 encodes");
                prop_assert_eq!(Dm15Response::decode(&encoded), Some(decoded));
            }

            if let Some(decoded) = Dm16Transfer::decode(&data) {
                prop_assert_eq!(usize::from(decoded.num_bytes), decoded.data.len());
                if decoded.data.len() <= 7 {
                    let encoded = decoded.encode().expect("decoded single-frame DM16 encodes");
                    prop_assert_eq!(Dm16Transfer::decode(&encoded), Some(decoded));
                }
            }

            if let Some(decoded) = EcuIdentification::decode(&data) {
                let encoded = decoded.encode().expect("decoded ECU Identification encodes");
                prop_assert_eq!(EcuIdentification::decode(&encoded), Some(decoded));
            }
        }
    }
}
