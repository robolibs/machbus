# Full scenario example

The full scenario combines multiple roles and subsystems. It is useful after
the small examples make sense.

Look for:

- `examples/session_minimal.rs` extended with several plugins or a preset group
  (see [The session facade](../guide/session-facade.md))
- `examples/c_abi/full_demo.c`

What it proves:

- broad API surfaces still build together
- multiple subsystems can coexist on one node
- event flow can be observed across a combined node
