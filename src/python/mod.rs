//! Python bindings for `machbus` (feature-gated on `python`).
//!
//! Built on the sans-IO [`Session`] facade and exposed
//! as a high-level `machbus.Session` class. Because the core is sans-IO, the
//! Python side drives it explicitly: stamp inputs with a millisecond cursor,
//! advance timers with [`Session::tick`], feed received CAN frames, and drain the
//! outbound frames + application events.
//!
//! ```python
//! import machbus
//!
//! s = machbus.Session(
//!     name=machbus.name(0x100, 0x80, True),
//!     preferred_address=0x80,
//!     enable_diagnostics=True,
//! )
//! s.start()
//! # In a real driver loop you would forward poll_transmit() frames to the bus
//! # and feed() frames received from it. With no contention the claim completes
//! # purely by advancing time:
//! addr = s.run_until_claimed(2000)
//! print(addr, s.claim_state())
//! s.diag_raise(523_312, 0)
//! for (port, can_id, data) in s.poll_transmit_all():
//!     ...  # send on the bus
//! while ev := s.poll_event():
//!     print(ev)
//! ```

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("bindings/python_session_and_core_codecs.rs");
include!("bindings/python_j1939_diagnostics_codecs.rs");
include!("bindings/python_dm_memory_nmea_vt_pool.rs");
include!("bindings/python_vt_ddop_module.rs");
