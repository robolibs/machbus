//! Extended Transport Protocol (`>1785` bytes, up to ~117 MB).
//!
//! Mirrors the C++ `machbus::net::ExtendedTransportProtocol`. Connection-mode
//! only — ETP does not support broadcast.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("etp/etp_protocol_engine.rs");
include!("etp/etp_tests.rs");
