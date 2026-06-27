//! ISO 11783-10 Task Controller server.
//!
//! Mirrors the C++ `machbus::isobus::tc::TaskControllerServer`.
//! Pump-style:
//!
//! - [`TaskControllerServer::try_handle_client_message`] feeds inbound
//!   `PGN_ECU_TO_TC` messages with explicit envelope validation errors.
//!   [`TaskControllerServer::handle_client_message`] remains the compatibility
//!   wrapper that ignores malformed or unrelated traffic.
//! - [`TaskControllerServer::update`] emits the periodic `Status`
//!   broadcast.
//!
//! Process-data callbacks (`value_request`, `value`, `peer_control`)
//! are stored as boxed `FnMut` closures, so the user can plug in
//! ECU-specific behavior without subclassing.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("server/tc_server_runtime.rs");
include!("server/tc_server_tests.rs");
