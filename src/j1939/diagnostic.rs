//! J1939-73 / ISO 11783-12 diagnostic message codecs.
//!
//! Mirrors the C++ `machbus::j1939::diagnostic.hpp`. Ports all wire
//! types — DTC, lamp status, FMI, every DM message family. The C++
//! `DiagnosticProtocol` class (IsoNet-coupled DTC database, freeze-frame
//! storage, DM13 suspend timer) is intentionally not ported; users
//! compose the codecs into their own state machine.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("diagnostic/diagnostic_messages.rs");
include!("diagnostic/diagnostic_tests.rs");
