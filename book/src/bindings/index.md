# Bindings overview

machbus is Rust-first, and both the C and Python bindings are now built on the
[session facade](../guide/session-facade.md). They wrap the sans-IO `Session`
core: you drive the node explicitly by feeding received CAN frames, ticking a
millisecond clock, and draining the frames and events it wants to emit. There is
no hidden bus or internal IO; the binding hands you bytes and you move them.

Reach for a binding when you have:

- a C application that needs machbus through an ABI-stable boundary,
- Python tooling that needs demos, tests, or integration scripts,
- a product that uses Rust internally but exposes a C ABI to another runtime.

Both surfaces expose the same model:

- one node behind a single object (`MachbusSession*` in C, `machbus.Session` in
  Python),
- subsystems plugged in at construction (diagnostics, GNSS, implement, VT
  client, TC client),
- a four-step drive loop: feed inbound frames, tick the clock, drain
  `poll_transmit`, drain `poll_event`.

Read [C ABI](./c.md) for the `machbus_session_*` surface, [Python](./python.md)
for the `machbus.Session` class, and [ABI stability](./abi-stability.md) for the
versioning contract.
