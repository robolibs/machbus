//! ISO 11783-6 Virtual Terminal server.
//!
//! Mirrors the C++ `machbus::isobus::vt::VTServer` (555 LOC).
//! Pump-style — no `IsoNet&`, no `ControlFunction*`. Outbound frames
//! come back as [`OutboundFrame`] entries from the inbound dispatch
//! and from [`VTServer::update`]. The caller routes them through
//! their own `IsoNet::send`.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("server/server_types_and_config.rs");
include!("server/server_lifecycle_queries_upload.rs");
include!("server/server_runtime_command_handlers.rs");
include!("server/server_client_pool_validation.rs");
include!("server/server_attribute_helpers.rs");
include!("server/server_tests.rs");
