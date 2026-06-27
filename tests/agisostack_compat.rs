//! Cross-implementation wire-compat tests adopted from
//! [AgIsoStack-plus-plus][1] — the reference open-source ISOBUS C++
//! library by Open-Agriculture.
//!
//! Each test below mirrors a specific test case in AgIsoStack's
//! `test/*.cpp` files and uses the same byte-exact wire payloads /
//! magic constants. If we round-trip the same values, our `Name` /
//! `Identifier` / `TimeDate` / heartbeat sequencing is interoperable
//! with AgIsoStack and any device that talks to it.
//!
//! Source: <https://github.com/Open-Agriculture/AgIsoStack-plus-plus>
//! (`test/can_name_tests.cpp`, `test/address_claim_tests.cpp`,
//! `test/core_network_management_tests.cpp`, `test/identifier_tests.cpp`,
//! `test/can_message_tests.cpp`, `test/heartbeat_tests.cpp`,
//! `test/time_date_tests.cpp`, `test/diagnostic_protocol_tests.cpp`,
//! `test/isobus_data_dictionary_tests.cpp`,
//! `test/cf_functionalities_tests.cpp`,
//! `test/tc_client_tests.cpp`,
//! `test/tc_server_tests.cpp`,
//! `test/guidance_tests.cpp`,
//! `test/nmea2000_message_tests.cpp`).
//!
//! [1]: https://github.com/Open-Agriculture/AgIsoStack-plus-plus

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("agisostack_compat/agisostack_network_vt_tc_compat.rs");
include!("agisostack_compat/agisostack_ddop_nmea_diagnostics_compat.rs");
