//! NMEA 2000 high-level interface: encode / decode the most common
//! PGNs and emit events on inbound traffic.
//!
//! Mirrors the C++ `machbus::nmea::NMEAInterface` (~870 LOC).
//! Pump-style port:
//!
//! - Per-PGN `send_*` builders return the 8-byte (or
//!   variable-length) payload — caller dispatches via
//!   `IsoNet::send`.
//! - [`NMEAInterface::handle_message`] decodes inbound based on the
//!   message's PGN and fires the relevant `Event<T>`.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("interface/nmea_interface_codecs.rs");
include!("interface/nmea_interface_tests.rs");
