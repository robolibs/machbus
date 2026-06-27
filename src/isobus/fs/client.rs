//! ISO 11783-13 File Client.
//!
//! Mirrors the C++ `machbus::isobus::fs::FileClient` (~680 LOC).
//! Pump-style port:
//!
//! - Operation methods (`open_file`, `read_file`, `write_file`, …)
//!   build the outbound request payload and return it as
//!   [`FSClientOutbound`] when the local request is encodable; the caller ships it on
//!   `PGN_FILE_CLIENT_TO_SERVER`.
//! - [`FileClient::handle_server_response`] decodes inbound
//!   `PGN_FILE_SERVER_TO_CLIENT` messages, updates internal
//!   bookkeeping, and fires the relevant `Event<...>`.
//! - [`FileClient::update`] handles CCM cadence + request expiry +
//!   server status timeout.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("client/client_state_and_requests.rs");
include!("client/client_response_helpers_and_tests.rs");
