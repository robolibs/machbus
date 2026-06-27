//! ISO 11783-14 Sequence Control (SC) — master, client, types.
//!
//! Mirrors the C++ `machbus::isobus::sc::*` namespace. Three files,
//! ~650 LOC. Pump-style port: master / client expose a
//! `handle_*(msg)` inbound and `update(elapsed_ms) -> Option<[u8; 8]>`
//! outbound — no `IsoNet&` coupling. Users wire the bytes through
//! `IsoNet::register_pgn_callback` / `IsoNet::send` directly. See
//! `book/src/reference/behavior-differences.md`.

pub mod client;
pub mod master;
pub mod recording;
pub mod scd;
pub mod tan;
pub mod types;

pub use client::SCClient;
pub use master::SCMaster;
pub use recording::SequenceRecorder;
pub use scd::{SCD_LABEL_NONE, ScdAction, ScdLabel, scd_action};
pub use tan::{SC_TAN_MAX, SC_TAN_MIN, SC_TAN_NOT_AVAILABLE, SC_TAN_REPEAT_MS, SequenceTanTracker};
pub use types::{
    SC_MAX_SEQUENCE_STEP_ID, SC_MSG_CODE_CLIENT, SC_MSG_CODE_MASTER, SC_STATUS_ACTIVE_RATE_MS,
    SC_STATUS_MIN_SPACING_MS, SC_STATUS_TIMEOUT_ACTIVE_MS, SC_STATUS_TIMEOUT_READY_MS,
    SCClientConfig, SCClientFuncError, SCClientState, SCCommand, SCMasterConfig, SCMasterState,
    SCSequenceState, SCState, SequenceStep,
};
