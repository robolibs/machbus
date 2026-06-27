# First node

> This page is the shortest path to a running node. For the full tour of plugins,
> the sans-IO core, and the driver/handle split, read
> [The session facade](../guide/session-facade.md).

A node owns one local role on a bus. The exact plugins you add depend on the
role, but the pattern is always the same:

1. choose a NAME
2. choose a preferred source address
3. plug only the subsystems you need
4. spawn over a transport — this gives you `(Controls, Driver)`
5. start, then claim an address before normal traffic
6. drive `driver.poll()?` and handle events

Conceptual Rust shape:

```rust
{{#include ../../../examples/session_minimal.rs:build}}
```

The snippet above is copied from `examples/session_minimal.rs`. Use runnable
examples for the exact current API calls.

```sh
make run EXAMPLE=session_minimal
```

## What to check

- `ctrl.is_claimed()` becomes true once the handshake completes, and
  `ctrl.address()` reports the address the node owns.
- The node does not send application traffic before address claim.
- Optional fine control such as `ctrl.with_mut::<Diagnostics, _>(...)` is only
  available when you plugged that subsystem (it returns `None` otherwise).
- Events are drained regularly enough for your application queue policy.

## Common mistakes

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| address claim times out | no transport or peer traffic not being pumped | drive the virtual bus and call `driver.poll()?` |
| `with_mut::<Diagnostics>` returns `None` | diagnostics not plugged | add `.plug(Diagnostics::every(1000))` |
| no GNSS events | GNSS not plugged or no GNSS PGN/sentence was sent | add `.plug(Gnss::listen())` and send input |
| normal traffic ignored by peers | node has not claimed an address | wait for `ctrl.is_claimed()` before sending |
