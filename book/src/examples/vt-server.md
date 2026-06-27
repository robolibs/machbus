# VT server example

Use the VT server example to understand status advertisement, upload handling,
and the protocol/state boundary.

Look for:

- `examples/vt_server_demo.rs`
- the `VtServer` plugin on the session facade (see
  [The session facade](../guide/session-facade.md))

Remember: `VTServer` is the protocol/state machine. Hosted Rust code can replay
its accepted render effects through `VtRenderRuntime` and the framebuffer/GTUI
backends, but the server itself is not a GUI window or product UI.
