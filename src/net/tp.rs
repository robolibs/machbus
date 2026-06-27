//! ISO 11783-3 / J1939-21 Transport Protocol (`8..=1785` bytes).
//!
//! Mirrors the C++ `machbus::net::TransportProtocol`. Implements both
//! BAM (broadcast) and CMDT (RTS/CTS/EoMA) modes.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("tp/tp_protocol_types.rs");
include!("tp/tp_protocol_engine.rs");
include!("tp/tp_tests.rs");
