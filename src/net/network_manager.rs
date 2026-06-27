//! Top-level orchestrator: [`IsoNet`].
//!
//! Mirrors the C++ `machbus::net::IsoNet`. Owns the address claimers,
//! transport engines, CAN endpoints, and the PGN callback registry,
//! and presents a single polled `update(elapsed_ms)` API.
//!
//! The Rust port is **generic over the link type** (`L: Link`).
//! All ports must use the same concrete link — see `PLAN.md` §2.7.
//! `eth_can.hpp` and the SocketCAN convenience constructor are
//! deferred to a follow-up phase.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("network_manager/iso_network_manager.rs");
include!("network_manager/iso_network_manager_tests.rs");
