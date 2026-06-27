# Tractor example

Use the tractor examples when you need a tractor-side role with GNSS,
diagnostics, implement command, or facility behavior.

Look for:

- `examples/tractor_ecu_demo.rs`
- `examples/session_minimal.rs` plus `presets::tractor()` (see
  [The session facade](../guide/session-facade.md))

Expected learning:

- how a tractor node claims an address
- how tractor-side subsystems are plugged in (or pulled in with `presets::tractor()`)
- how events are drained
