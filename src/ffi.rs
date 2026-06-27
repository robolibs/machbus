//! C ABI for machbus, built on the [`session`](crate::session) facade.
//!
//! This is a clean, coherent `machbus_session_*` C ABI over the sans-IO
//! [`Session`] core. A node is a heap-allocated
//! `Session` behind one opaque handle; the caller drives it explicitly:
//!
//! 1. feed received CAN frames with [`machbus_session_feed`],
//! 2. advance time with [`machbus_session_tick`],
//! 3. drain outbound frames with [`machbus_session_poll_transmit`] and write
//!    them to the bus,
//! 4. drain application events with [`machbus_session_poll_event`].
//!
//! Conventions: opaque `Box`-backed handles (free with the matching `*_free`);
//! fallible calls return `bool`/int with the reason in the thread-local
//! [`machbus_session_last_error`]; borrowed byte/string views stay owned by the
//! handle they came from. `include/machbus.h` is generated from this file by
//! cbindgen.

// extern "C" fns take raw pointers from C and deref them by design.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("ffi/c_abi_session_core.rs");
include!("ffi/c_abi_session_vt_tc.rs");
include!("ffi/c_abi_j1939_diagnostics.rs");
include!("ffi/c_abi_transfer_nmea.rs");
include!("ffi/c_abi_vt_pool_builder.rs");
