# ABI stability

The C ABI has an explicit version surface and a generated header. ABI stability
means C callers can rely on ownership and layout rules within the documented
version boundary.

## Current version: 5

The ABI is **version 5**, reported by `machbus_session_abi_version()`.

C examples intentionally fail fast if the runtime reports a different version.
That guard is the only thing standing between a stale header and undefined
behaviour, so it is load-bearing: a caller built against v3 that skipped it
would call the five-argument `machbus_session_fs_client_seek` with four
arguments, reading `out_tan` from an uninitialised stack slot and writing
through it.

### v4 → v5

No signature or layout changed; the **error contract** of two functions did.

- **`machbus_session_autodrive_clear_stop` and
  `machbus_session_guidance_clear_stop` can now return `false`.** Releasing a
  latched safe stop has been a conditional no-op inside the plugins since the
  ISB and GNSS hazard interlocks landed, but both C functions returned `true`
  unconditionally and cleared the last error. A caller that showed the fault as
  cleared on a `true` return re-enabled Engage with the latch still set. They
  now return `false` and set a last-error string naming the refusal
  (`stop_condition_live`) when the operator is still holding the Auxiliary
  Shortcut Button or a GNSS hazard is live.

The Python `autodrive_clear_stop` / `guidance_clear_stop` raise `RuntimeError`
in the same case.

- **`MachbusLanguageData` unit fields now carry `0xFF` for an unrecognised
  code** instead of silently substituting the metric/English default. A
  Language Command whose unit codes this edition does not define is no longer
  discarded whole either, so a caller now receives the operator's language code
  with `0xFF` in the fields it could not interpret. Treat `0xFF` as "not
  specified" rather than as a unit value.

- **`machbus_session_autodrive_stop_reason` and
  `machbus_session_guidance_stop_reason` return `MachbusSafeStopTrigger`**
  instead of a bare `uint32_t`. The values are unchanged — the enum mirrors
  `SafeStopTrigger::as_code`, with `MACHBUS_SAFE_STOP_TRIGGER_NONE = 0` for "no
  stop latched" — but the return type is now named, so a C HMI no longer has to
  hardcode codes read out of the Rust source. **Codes 2 and 3 are permanently
  retired** (they were `TimStatusTimeout` and `FunctionRequestTimeout`, which
  had no producer); they must never be reused, or every value above them shifts
  for callers built against an older header.

- **`MachbusEventKind` gained `TcServerClientDisconnected` (100).** Additive:
  existing discriminants are unchanged. It fires when the TC server drops a
  client after six seconds without a Client Task message (ISO 11783-10 §6.6.3);
  `source` is the client address. A caller with an exhaustive switch over the
  enum must add an arm.

### v3 → v4

Conformance work against ISO 11783-13 and ISO 11783-7 changed four things a C
caller must audit:

- **`machbus_session_fs_client_seek` gained an argument.** It was
  `(handle, position: uint32_t, out_tan)`; it is now
  `(handle, mode: uint8_t, offset: int32_t, out_tan)`. `mode` is the ISO
  11783-13 B.17 Position Mode — 0 from the start, 1 from the current pointer,
  2 from the end — and the offset is signed, so a rewind is a negative value.
- **File Server error codes were renumbered to Annex B.9.** Everything from 5
  upward moved: `InvalidHandle` is now 5 (was 7), `MediaNotPresent` 10 (was
  12), `NotSupported` 12 (was 20). A caller switching on the numeric value must
  be re-read against the table.
- **`MachbusEvent`'s FS seek payload carries the resulting position** rather
  than a unit success, per C.3.3.3.
- **`repr(C)` PODs widened** where a decoder gained a field (notably the GNSS
  position, which now carries DD209 integrity).

### v2 → v3

The rewrite onto the [session facade](../guide/session-facade.md): the entire
symbol set was renamed to the `machbus_session_*` prefix and the model changed
to sans-IO (the caller bridges IO with feed/tick/poll instead of an internal
virtual bus). A deliberate breaking change; stability guarantees start fresh
from v3, and older `machbus.h` headers and pre-v3 symbol names do not apply.

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
