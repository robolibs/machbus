//! Fixture-backed protocol golden tests.
//!
//! These tests make the first H1 fixture corpus executable: golden bytes live
//! under `tests/fixtures/` and the tests prove that current codecs still decode
//! and encode those bytes exactly.

use machbus::isobus::implement::Signal;

/// A measured value, for building expected J1939 structs concisely.
#[allow(dead_code)]
const fn sig(v: f64) -> Signal<f64> {
    Signal::Value(v)
}

/// Assert a `Signal` carries a measurement close to `expected`.
#[allow(dead_code)]
#[track_caller]
fn assert_sig(actual: Signal<f64>, expected: f64, tolerance: f64) {
    let value = actual
        .value()
        .unwrap_or_else(|| panic!("expected ~{expected}, got {actual:?}"));
    assert!(
        (value - expected).abs() <= tolerance,
        "expected ~{expected}, got {value}"
    );
}

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("protocol_fixtures/fixture_constants_and_core_protocols.rs");
include!("protocol_fixtures/j1939_diagnostics_powertrain_fixtures.rs");
include!("protocol_fixtures/j1939_powertrain_niu_router_fixtures.rs");
include!("protocol_fixtures/isobus_vt_tc_fixtures.rs");
include!("protocol_fixtures/isobus_tc_fs_fixtures.rs");
include!("protocol_fixtures/isobus_implement_control_fixtures.rs");
include!("protocol_fixtures/isobus_tim_sc_nmea_gnss_fixtures.rs");
include!("protocol_fixtures/nmea_environment_tp_fixtures.rs");
include!("protocol_fixtures/etp_socketcan_trace_fixtures.rs");
