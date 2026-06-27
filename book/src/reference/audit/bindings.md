# Binding contracts

This page explains how the Rust, C, and Python surfaces relate. It is written
for maintainers who need to change a binding without accidentally promising
more than the repository tests.

Both the C and Python bindings are built on the
[session facade](../../guide/session-facade.md): they wrap the sans-IO `Session`
core behind one object per node, driven with feed/tick/poll.

## The three surfaces

| Surface | Role | Main files | Main gate |
|---|---|---|---|
| Rust | Canonical implementation API. New behavior should land here first. | `src/lib.rs`, `src/session/`, `src/isobus/`, `src/j1939/`, `src/nmea/`, `src/net/` | `make check`, `make test`, `make check-all`, `make test-all`, `make clippy`, `make rustdoc` |
| C | Opaque-handle ABI (`machbus_session_*`) for C callers and downstream generated bindings. | `src/ffi.rs`, `include/machbus.h`, `examples/c_abi/` | `make bind-c-check`, `make c-demo`, `make c-full-demo` |
| Python | Ergonomic pyo3 facade for examples and scripting. | `src/python/mod.rs`, `examples/python_binding/`, `pyproject.toml` | `make python-demo` |

Rust is the design surface. C and Python are facades. Do not expose a Rust
internal type directly just because it exists; add a stable facade operation
and a test first.

## VT rendering exposure

The Virtual Terminal client/server session traffic is visible through the C and
Python session facades where the corresponding VT subsystems are enabled. The
hosted VT rendering stack is different: `IopDocument`, `VtRenderRuntime`,
`RenderCommand`, `GtuiRenderer`, and `FramebufferRenderer` are currently
Rust-only APIs.

That split is intentional until a stable binding contract exists for object
pool ownership, render-command snapshots, framebuffer byte buffers, and error
reporting. C and Python callers should not be documented as rendering VT
object pools today; they can drive the protocol facade and consume session
events, while hosted object-pool layout/rendering remains in Rust.

## C ABI versioning

`machbus_session_abi_version()` is the C caller's fast compatibility check.
Increase the ABI contract deliberately when exported layouts or functions change
in a way downstream bindings need to know about.

The current public C ABI contract is version `3`, which marks the rewrite onto
the session facade. The C examples intentionally fail fast if the runtime
reports a different version.

The generated `include/machbus.h` comes from `src/ffi.rs`. Regenerate it with
`make bind-c`, then prove it is stable with `make bind-c-check`.

## Opaque ownership table

The session ABI exposes a single caller-owned handle, released by its matching
function.

| Handle | Constructor | Free function | Lifetime note |
|---|---|---|---|
| `MachbusSession*` | `machbus_session_new` | `machbus_session_free` | One node wrapping the sans-IO `Session` core; the caller drives it with feed/tick/poll. |

Ownership rules:

- `machbus_session_free(NULL)` is a no-op.
- Double-freeing a handle is outside the C ABI contract.
- C callers should set caller variables to `NULL` immediately after freeing.

## Error shape

Rust returns typed `Result` values. C uses boolean/sentinel returns plus the
thread-local `machbus_session_last_error()`. Python raises exceptions or returns
ergonomic objects/dicts depending on the operation.

When adding a new binding call, make the failure shape boring:

1. validate pointers, lengths, enum ranges, and subsystem state before mutating
   the session;
2. report a clear error through the surface's normal error channel;
3. add a negative test for the bad input;
4. add a happy-path test that proves the event or state is visible to callers.

## What the facades intentionally hide

The facades should not leak Rust layout, borrowed Rust storage, or internal
state machines. In C, use POD structs, fixed output buffers, explicit lengths,
and opaque handles. In Python, prefer small classes and dict-like event
payloads that are stable for examples.

If C or Python needs a richer event later, add a new accessor or event shape
instead of changing the meaning of an existing field silently.
