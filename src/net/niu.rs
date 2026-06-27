//! Network Interconnect Unit — bridges two CAN networks with
//! filtering, rate limiting, and (optionally) address translation.
//! ISO 11783-4.
//!
//! Mirrors the C++ `machbus::net::NIU` family. **Pump-style API**:
//! [`Niu::process_frame`] returns `Some(frame)` to forward to the
//! *other* side or `None` if blocked. The caller hands the result
//! to the destination `IsoNet`.
//!
//! # Differences from the C++
//!
//! - The C++ `process_frame` takes no time argument — its rate
//!   limiter compares `now_ms` (always `0`) against `last_forward_time`,
//!   which silently blocks every rate-limited PGN forever. The Rust
//!   port takes `now_ms` explicitly.
//! - Only the `Niu` base and `Router` are ported. `RepeaterNIU` (=
//!   default config), `BridgeNIU` (= subset of `Router`), and
//!   `GatewayNIU` (closure-heavy, niche) are deferred.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("niu/niu_router_policy.rs");
include!("niu/niu_router_policy_tests.rs");
