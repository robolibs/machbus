# ABI stability

The C ABI has an explicit version surface and a generated header. ABI stability
means C callers can rely on ownership and layout rules within the documented
version boundary.

## Current version: 3

The ABI is now **version 3**, reported by `machbus_session_abi_version()`. This
version marks the rewrite onto the [session facade](../guide/session-facade.md):
the entire symbol set was renamed to the `machbus_session_*` prefix and the model
changed to sans-IO (the caller bridges IO with feed/tick/poll instead of an
internal virtual bus). That was a deliberate, breaking change, so stability
guarantees start fresh from v3 — older `machbus.h` headers and any pre-v3 symbol
names do not apply.

C examples intentionally fail fast if the runtime reports a different version.

## What the version covers

Bump the ABI version whenever any of these change in a way C callers must audit:

- exported function signatures,
- `#[repr(C)]` POD struct layouts (`MachbusConfig`, `MachbusEvent`, …),
- enum discriminants (`MachbusClaimState`, `MachbusEventKind`, the command
  enums),
- ownership or error contracts.

## What is checked

- generated header drift (`include/machbus.h` against `src/ffi.rs`),
- exported function compile surface,
- C POD layout assertions,
- Rust-side FFI contract tests,
- C demo workflows.

## When changing ABI

1. Update the Rust FFI code in `src/ffi.rs`.
2. Bump `MACHBUS_C_ABI_VERSION` if the change is caller-visible.
3. Regenerate and check the header (`make bind-c`, `make bind-c-check`).
4. Update examples.
5. Run `make verify`.
6. Document the change in release notes.
