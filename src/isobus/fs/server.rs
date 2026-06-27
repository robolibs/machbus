//! ISO 11783-13 enhanced File Server with TAN-based idempotency.
//!
//! Mirrors the C++ `machbus::isobus::fs::FileServerEnhanced` (~1050
//! LOC). Pump-style port:
//!
//! - [`FileServer::handle_client_message`] feeds inbound
//!   `PGN_FILE_CLIENT_TO_SERVER` messages and returns the response.
//! - [`FileServer::update`] advances timers/volume FSM and returns
//!   the periodic status broadcasts.
//!
//! Notable simplifications versus the C++:
//!
//! - `OpenFile` stores the file *path* instead of a raw pointer back
//!   into the server's `files_` map. This sidesteps the `&mut`
//!   aliasing issue and keeps the borrow checker happy.
//! - The server's per-client `ClientConnection` struct is renamed
//!   [`ServerClientConnection`] to disambiguate from
//!   [`super::connection::ClientConnection`].

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("server/server_state_and_models.rs");
include!("server/server_file_operations.rs");
include!("server/server_wire_helpers_and_tests.rs");
