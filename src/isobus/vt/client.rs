//! ISO 11783-6 Virtual Terminal client (FSM + command codecs).
//!
//! Mirrors the C++ `machbus::isobus::vt::VTClient` (~990 LOC).
//!
//! ## Pump-style API
//!
//! - [`VTClient::handle_vt_message`] feeds an inbound `PGN_VT_TO_ECU`.
//! - [`VTClient::handle_language_command`] feeds inbound
//!   `PGN_LANGUAGE_COMMAND`.
//! - [`VTClient::update`] advances the connect FSM and returns the
//!   [`ClientOutbound`] frames the user must ship.
//! - User-triggered commands (`hide_show`, `change_numeric_value`,
//!   `store_version`, …) return a [`ClientOutbound`] payload (or an
//!   error) so the user can dispatch via `IsoNet::send`.
//!
//! No `IsoNet&` is held; the C++ `ControlFunction&` source/dest
//! parameters are not needed because the caller routes the bytes.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("client/vt_client_types_and_session.rs");
include!("client/vt_client_wire_helpers_and_tests.rs");
