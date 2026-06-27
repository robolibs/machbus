# Error handling

This page describes the error model of the `machbus` crate: one `Result`
alias, one `Error` value carrying an `ErrorCode`, and the conventions that codecs,
sessions, and the session facade follows so that failures stay explicit and
inspectable. It is a lookup reference; read it after you have a feel for how the
code is layered (see the [crate map](crate-map.md)).

## Why a single error model

Agricultural bus code runs in places where panics are unwelcome: long-lived
control loops, embedded targets, and bindings that must not unwind across a
language boundary. So `machbus` makes failure a value, not an exception. Every
fallible function returns `Result<T, Error>`, and the `Error` it returns always
has a discrete `ErrorCode` plus an optional human-readable message. Callers can
branch on the code without parsing strings, and the message is there for logs.

```
fallible call ──► Result<T, Error>
                       │
                  Err(Error { code: ErrorCode, message: String })
                       │
                  match on code ─► retry / fail-fast / safe-state
```

`ErrorCode::Ok` exists as the zero value (it mirrors a numeric C++ enum so stored
or wire-adjacent values stay aligned), but a successful Rust call is `Ok(value)`,
not an `Error` with code `Ok`. You will not normally construct or match `Ok` as an
error in Rust.

## The pieces

- `Result<T>` — a crate-local alias for `core::result::Result<T, Error>`. Most
  signatures in the codebase use the alias, so `Result<()>` means "fallible, no
  payload" and `Result<Frame>` means "fallible, returns a frame".
- `Error` — a struct of `code: ErrorCode` and `message: String`. Build it with
  `Error::new(code)` for a bare code, `Error::with_message(code, text)` for
  context, or one of the factory helpers (`Error::timeout()`,
  `Error::invalid_pgn(pgn)`, `Error::invalid_address(addr)`,
  `Error::not_connected()`, `Error::invalid_state(msg)`, and so on). `ErrorCode`
  also `impl From<ErrorCode> for Error`, so `code.into()` yields a message-less
  error.
- `Display` — an `Error` prints as just the code description when the message is
  empty, or `code: message` when it is set. The code's own `as_str` gives a short
  static label suitable for logs.

## The error codes

The variants group by theme. Every name below is a real `ErrorCode` variant; the
crate defines no others.

### Addressing and claim

| Code | Plain meaning | Typical cause |
| --- | --- | --- |
| `AddressClaimFailed` | A node could not secure a source address. | Claim arbitration did not resolve in this node's favour. |
| `AddressConflict` | A requested source address already belongs to another NAME. | Two nodes target the same address; the lower-priority NAME loses. |
| `InvalidAddress` | A supplied address is out of range or reserved. | Passing the null or global address where a real one is required. |

### Transport sessions

| Code | Plain meaning | Typical cause |
| --- | --- | --- |
| `Timeout` | A timed operation did not complete in its window. | No expected response arrived before the deadline elapsed. |
| `TransportTimeout` | A multi-frame transfer stalled. | A TP/ETP peer stopped sending or acknowledging mid-transfer. |
| `TransportAborted` | A multi-frame transfer ended early by abort. | A connection-abort condition was raised by either side. |
| `SessionExists` | A transport session is already active for that key. | A second transfer is started for a PGN/direction/port already in flight. |
| `NoResources` | No session slot or buffer was available. | The session table is full and cannot admit another transfer. |

### Protocol identity and parse

| Code | Plain meaning | Typical cause |
| --- | --- | --- |
| `InvalidPgn` | A PGN value is malformed or not handled here. | A frame's parameter group does not match what the decoder expects. |
| `InvalidData` | A payload failed validation. | Wrong length, an out-of-range field, or a structurally invalid message body. |

### Capacity and buffers

| Code | Plain meaning | Typical cause |
| --- | --- | --- |
| `BufferOverflow` | Data exceeded the space a codec can hold. | Encoding more bytes than the target frame or assembly buffer allows. |

### Object pools

| Code | Plain meaning | Typical cause |
| --- | --- | --- |
| `PoolError` | A generic object-pool failure. | A pool operation failed in a way that is not a specific validation issue. |
| `PoolValidation` | A pool or descriptor object failed validation. | A DDOP or object-pool field is malformed (bad text encoding, bad reference). |

### State and lifecycle

| Code | Plain meaning | Typical cause |
| --- | --- | --- |
| `NotConnected` | An operation needs an established link that is not there. | Calling a client method before its server connection completed. |
| `InvalidState` | An operation was requested in the wrong state. | Driving a state machine through a transition it does not allow yet. |

### Driver and interface

| Code | Plain meaning | Typical cause |
| --- | --- | --- |
| `DriverError` | A lower CAN driver or setup step failed. | Bus construction or a driver call reported a failure. |
| `SocketError` | A socket-backed transport call failed. | A read/write on the underlying socket returned an error. |
| `InterfaceDown` | The network interface is not usable. | The bus interface is down or has not come up. |

## How errors propagate

There are two surfaces, and they treat errors a little differently.

- **Low-level codecs and pumps** (`src/net/`, the J1939 and ISOBUS encoders,
  TP/ETP sessions) return `Result` directly. A decode that sees a wrong length
  returns `Err(Error)` with `InvalidData` rather than panicking; a session that
  is full returns `NoResources`. Because the failure is in the return value, you
  decide what to do at the call site.
- **The session facade** (`src/session/`) drives those pumps on a tick and turns
  most asynchronous protocol outcomes into *events* you drain, not into a thrown
  error. A plugin control method still returns `Result` when the
  caller can recover immediately (for example `Timeout` from a blocking wait), but
  ongoing conditions — a VT object-pool rejection, a connection coming and going —
  reach you through the event stream. Using a subsystem handle that the builder
  never enabled is treated as a programming error, not a recoverable `Error`;
  check setup at construction instead.

## Patterns for handling them

- **Match on the code, not the message.** The `message` is for humans and may
  change; `error.code` is the stable contract.
- **Retry vs fail-fast.** `Timeout`, `TransportTimeout`, and `NoResources` are
  often transient — a bounded retry or back-off is reasonable. `InvalidPgn`,
  `InvalidData`, and `PoolValidation` describe malformed input or a programming
  mistake; retrying the same bytes will fail the same way, so fail fast and fix
  the source.
- **Map to a safe state.** For codes that indicate the link itself is gone or
  unusable — `NotConnected`, `InterfaceDown`, `AddressClaimFailed`,
  `DriverError`, `SocketError` — the right response is usually to stop commanding
  motion and move the application to its defined safe state rather than to retry
  blindly.
- **Add context as you go.** When wrapping a lower call, `Error::with_message`
  lets you attach where it happened without losing the code.

## Across the bindings

The C and Python layers expose the same model, narrowed to each ABI.

- Over the C ABI, opaque handles keep Rust ownership intact and functions report
  outcome through documented status channels rather than by unwinding; a Rust
  `Error` becomes a status the caller checks. See the [C ABI](../bindings/c.md)
  page.
- In Python, a failed call surfaces as a clear exception or an explicit
  empty/false result instead of undefined behaviour, so the `ErrorCode` meaning
  carries across. See the [Python](../bindings/python.md) page.

Neither binding invents new error categories; they re-present the same codes.

## Common confusions

- **`Ok` is not a success return.** It is the zero variant of the enum for
  numeric compatibility. Success in Rust is `Ok(value)` from `Result`.
- **`Timeout` vs `TransportTimeout`.** The first is any timed wait; the second is
  specifically a multi-frame transport transfer that stalled.
- **`TransportTimeout` vs `TransportAborted`.** A timeout means silence past the
  deadline; an abort means an explicit end-of-transfer condition.
- **`PoolError` vs `PoolValidation`.** Validation means a pool object's contents
  failed a check; `PoolError` is the broader catch-all for other pool failures.
- **`InvalidData` vs `InvalidPgn`.** `InvalidPgn` is about the message identity
  being wrong or unhandled; `InvalidData` is about the body of a message that was
  otherwise addressed correctly.
- **`NotConnected` vs `InvalidState`.** `NotConnected` means a required link is
  absent; `InvalidState` means the link may exist but the requested step is not
  allowed from the current state.
- **The code is stable; the message is not.** Branch on `code`. Log the message.

## See also

- [Crate map](crate-map.md) — which layer returns errors and which emits events.
- [Feature flags](feature-flags.md) — how disabled subsystems change the surface.
- [Behavior differences](behavior-differences.md) — where bindings narrow the model.
- [C ABI](../bindings/c.md) and [Python](../bindings/python.md) — error surfacing per binding.
