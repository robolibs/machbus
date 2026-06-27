//! ISO 11783-10 Task Controller — DDOP, server, client, geo,
//! peer-control.
//!
//! Mirrors the C++ `machbus::isobus::tc::*` namespace. Pump-style port:
//! `*Interface` / `*Server` / `*Client` C++ classes that embed
//! `IsoNet&` are replaced with structs that consume `Message` inputs
//! and return `Vec<Outbound>` payloads. See `book/src/reference/behavior-differences.md`.

pub mod client;
pub mod control;
pub mod ddi_database;
pub mod ddop;
pub mod ddop_helpers;
pub mod geo;
pub mod grid;
pub mod isoxml;
pub mod objects;
pub mod outstanding;
pub mod peer_control;
pub mod rate_limit;
pub mod section_control;
pub mod server;
pub mod server_options;
pub mod task;
pub mod totals;

pub use client::{
    CommandCallback, MAX_TC_PROCESS_DATA_ELEMENT_NUMBER, TCClientCapabilities, TCClientConfig,
    TCClientOutbound, TCClientTaskStatus, TCState, TaskControllerClient,
    ValueCallback as ClientValueCallback, tc_cmd,
};
pub use control::{PrescriptionController, RateCommand};
pub use ddi_database::{
    DDI_DATABASE, DDI_DATABASE_FINGERPRINT_FNV1A64, DDI_DATABASE_SIZE, DDI_DATABASE_VERSION,
    DDIDatabase, DDIDefinition, DDIEntry, DataDictionary, DataDictionaryEntry, DdiClass,
    classify_ddi, ddi, ddi_data_dictionary_entry, ddi_database_fingerprint, ddi_display_range,
    ddi_from_engineering, ddi_is_acceptable, ddi_is_geometry, ddi_is_proprietary, ddi_is_rate,
    ddi_is_section_control, ddi_is_speed_distance, ddi_is_total, ddi_lookup, ddi_name,
    ddi_resolution, ddi_to_engineering, ddi_unit, ddi_unit_description, unknown_ddis,
};
pub use ddop::DDOP;
pub use ddop_helpers::{
    DDOPHelpers, ImplementGeometry, RateInfo, SectionInfo, SubBoomInfo, extract_geometry,
    extract_rates, extract_totals, find_parent_element, section_count,
};
pub use geo::{
    GeoPoint, PrescriptionMap, PrescriptionZone, TCGEOInterface, geo_ddi, point_in_polygon,
    point_in_prescription_zone, prescription_rate_from_engineering,
    prescription_rate_process_data_payload, prescription_rate_to_engineering,
};
pub use grid::TreatmentZoneGrid;
// `isoxml::DeviceElement` is intentionally not re-exported here to avoid
// clashing with `objects::DeviceElement` (the DDOP device element); reach it
// as `tc::isoxml::DeviceElement`.
pub use isoxml::{
    Device, LoggedValue, Partfield, PositionFields, Task, TaskData, TimeLogRecord,
    TimeLogStructure, XmlElement, parse_xml,
};
pub use objects::{
    DDI, DeviceElement, DeviceElementType, DeviceObject, DeviceProcessData, DeviceProperty,
    DeviceValuePresentation, ElementNumber, ObjectID, TCObjectType, TriggerMethod,
};
pub use outstanding::OutstandingRequests;
pub use peer_control::{PeerControlAssignment, PeerControlInterface};
pub use rate_limit::{DEFAULT_MIN_INTERVAL_MS, ProcessDataRateLimiter};
pub use section_control::SectionControl;
pub use server::{
    MeasurementTriggerRuntime, PeerControlCallback, TC_STATUS_INTERVAL_MS, TCClientInfo,
    TCOutbound, TCServerConfig, TaskControllerServer, ValueCallback as ServerValueCallback,
    ValueRequestCallback,
};
pub use server_options::{
    ObjectPoolActivationError, ObjectPoolDeletionErrors, ObjectPoolErrorCodes,
    ProcessDataAcknowledgeErrorCodes, ProcessDataCommands, ServerOptions,
    TC_SERVER_OPTIONS_KNOWN_MASK, TCServerState, tc_options_byte_is_valid,
};
pub use task::{LogEntry, TaskLifecycle, TaskLog, TaskSession};
pub use totals::TaskTotals;
