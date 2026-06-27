# File Server

A **File Server** (FS) is a control function that owns a file-based storage
device and lets any other control function on the implement bus store and
retrieve data. The data plates of a tractor, the screenshots of a virtual
terminal, a stored object pool, a harvest log a task controller wants to keep
between power cycles — all of it can live on one shared server that speaks a
small request/response protocol over the bus. This tutorial covers both roles:
the **client** that browses and reads/writes files, and the **server** that
exposes a namespace and enforces the rules. It explains the operation set,
the file-handle lifecycle, the status and error model, and how to drive each
side with `machbus` at the low level and through the session facade.

The protocol is defined in ISO 11783-13 (File Server). `machbus` ports it as a
pump-style client and an enhanced, TAN-idempotent server.

## Why a File Server exists

A CAN bus moves short frames, not files. Implements still need durable,
shareable storage: a place to drop logs, configuration, calibration tables,
and large blobs that outlive a single message. Building a flash chip into every
ECU is wasteful and fragments the data across the machine. The FS model instead
puts one storage device behind a network service. Any client can open a file,
read or write at byte offsets, and close it again, exactly as if it had a local
disk — except the disk is shared, the access is mediated, and the server
decides what is visible and what is writable.

Because the namespace is server-owned, the FS is also a safety boundary. A
client only sees the paths the server chooses to expose, cannot escape that
namespace with `..`, and cannot write to files the server marks read-only. The
server is the single point that mounts media, names volumes, and reports when
removable media comes and goes.

## Mental model

```
   client                                   server
   ──────                                    ──────
   connect ──────── GetProperties ─────────►  reply: version, caps, max files
        ◄──────────  (Connected) ───────────  (status broadcasts begin)
   open "\\logs\\a.txt" ───────────────────►  validate path, allocate handle
        ◄──────────  handle = 7  ────────────
   write @pos / read @pos (by handle) ─────►  advance file position, reply
        ◄──────────  bytes + status ─────────
   close 7 ────────────────────────────────►  release handle
   ... every 2 s: CCM keepalive ───────────►  refresh connection liveness
```

Every transaction is **client-initiated and server-terminated**: the client
sends a request tagged with a transaction number (TAN), the server does the
work and answers with the same TAN. The client keeps a keepalive (CCM) flowing
so the server knows it is still there; the server broadcasts its status so
clients learn whether it is busy and how many files are open.

## The operation set

`machbus` models each operation as an `FSFunction` code. The client builds the
request payload; the server decodes it, executes, and encodes a response. The
function set, in your own words:

| Operation | `FSFunction` | What it does |
| --- | --- | --- |
| Get file-server properties | `GetFileServerProperties` | Read the server version, the max number of simultaneous files, and capability bits (directories, volume management, attributes, move, delete). This is the first request the client sends on connect. |
| Get file-server status | `FileServerStatus` | Read the busy flag and the count of currently open files. Also broadcast periodically. |
| Get current directory | `GetCurrentDirectory` | Ask which directory this client's session is currently in. |
| Change current directory | `ChangeDirectory` | Move the client's session to another directory, including `.` (stay), `..` (up), and `\` (root). |
| Open file | `OpenFile` | Open or create a file (or open a directory for listing) and get back a handle. Carries the access mode and the create/append/exclusive flags. |
| Seek | `SeekFile` | Set the absolute byte position of an open handle. |
| Read | `ReadFile` | Read up to N bytes from an open handle at its current position; the position advances by the bytes returned. For a directory handle, returns listing entries instead. |
| Write | `WriteFile` | Write a payload to an open handle at its current position; the file grows if needed and the position advances. |
| Close | `CloseFile` | Release a handle. |
| Move | `MoveFile` | Rename/relocate a file within the namespace. |
| Delete | `DeleteFile` | Remove a file. |
| Get attributes | `GetFileAttributes` | Read a file's attribute byte (read-only, hidden, system, directory, archive, volume). |
| Set attributes | `SetFileAttributes` | Set a file's attribute byte. |
| Get date/time | `GetFileDateTime` | Read the packed filesystem date and time for a path. |
| Initialize volume | `InitializeVolume` | Reset the volume to an empty namespace (a service-tool operation). |
| Volume status | `VolumeStatus` | Server-to-client broadcast of volume presence/removal state. |

### Access modes and open flags

`OpenFile` carries an `OpenFlags` byte. The low two bits are the access mode;
the upper bits are independent flags you OR in:

| Flag | Bits | Meaning |
| --- | --- | --- |
| `Read` | mode `0x00` | Open for reading. |
| `Write` | mode `0x01` | Open for writing. |
| `ReadWrite` | mode `0x02` | Open for both. |
| `OpenDir` | mode `0x03` | Open a directory for listing (read its entries with `ReadFile`). |
| `Create` | `0x04` | Create the file if it does not exist. |
| `Append` | `0x08` | Start the position at end-of-file. Invalid for read-only or directory opens. |
| `Exclusive` | `0x10` | With `Create`, fail if the file already exists. |

`OpenFlags::Write | OpenFlags::Create` is the idiom for "create or open a file
to write into"; adding `Exclusive` turns it into "create, but only if new".
The server rejects reserved bits with `InvalidAccess` and rejects an
unsupported directory open with `NotSupported`.

## The file-handle lifecycle

A handle is a one-byte token (`FileHandle`) the server assigns on a successful
`OpenFile`. `0x00` and `0xFF` are reserved sentinels (`RESERVED_FILE_HANDLE_0`,
`INVALID_FILE_HANDLE`), so live handles run `0x01..=0xFE`. The handle is the
only thing read/write/seek/close refer to — paths are resolved exactly once, at
open time.

```
        open(path, flags)
              │  server validates path, checks caps,
              │  allocates handle (or errors out)
              ▼
        ┌───────────┐   seek(pos)        ┌───────────┐
        │   OPEN    │ ─────────────────► │   OPEN    │   position moved
        │ (handle)  │ ◄───────────────── │ (handle)  │
        └───────────┘   read/write       └───────────┘
              │         (position advances by bytes moved)
              │
              │  close(handle)  ──►  handle released, slot freed
              ▼
        ┌───────────┐
        │  CLOSED   │   handle is now stale; reuse → InvalidHandle
        └───────────┘
```

The handle is **owner-scoped**: the server records the owning client address on
each `OpenFile` and only honours read/write/seek/close from that same client.
A handle from one client is meaningless to another. The handle also dies if the
client times out (no CCM) or the volume is removed — in both cases the server
drops the open file and the next use of that handle returns `InvalidHandle`.

On the client side, `FileClient` mirrors the server's bookkeeping in an
`OpenFileInfo` per handle (path, flags, local position) so the application can
track where each file cursor sits without a round trip.

## The status and error model

Every server response carries an `FSError` byte. `machbus` exposes the standard
set as the `FSError` enum; `Success` is `0`. The categories, and how a client
should react:

| `FSError` | Category | Client reaction |
| --- | --- | --- |
| `Success` | OK | Proceed; use the returned data. |
| `NotFound` | Path | The file or directory is not there. Create it (if you meant to) or correct the path. |
| `WrongType` | Path | You asked for a file but the path is a directory, or vice versa. Pick the right operation. |
| `InvalidSourceName` / `InvalidDestName` | Path | The name is illegal (bad characters, traversal, host-absolute). Fix the path before retrying. |
| `AccessDenied` | Permission | The file is read-only, is open elsewhere, or the target already exists for an exclusive create. Do not retry blindly. |
| `InvalidAccess` | Permission | The requested access mode or flag combination is not allowed. Fix the flags. |
| `TooManyOpen` | Resource (per client) | You hit your own open-file cap. Close something and retry. Retryable. |
| `MaxHandles` | Resource (server-wide) | The server is out of handle slots. Back off and retry later. Retryable. |
| `InvalidHandle` | Handle | The handle is unknown, stale, or not yours. Re-open the file. |
| `NoSpace` | Storage | The volume is full. Free space or stop. |
| `WriteFail` | Storage / I/O | A write failed at the media. Retryable. |
| `MediaNotPresent` | Volume | Removable media is gone. Fatal for the in-flight transfer; wait for the volume to return. |
| `NotInitialized` | Volume | The file system did not mount. Fatal. |
| `NotSupported` | Capability | The server does not implement this operation (check the properties first). |
| `InvalidLength` | Framing | A length field in the request or response is wrong. |
| `OutOfMemory` | Resource | The server could not allocate. Fatal. |
| `EndOfFile` | Read | The read started at or past end-of-file. `machbus` surfaces this on the client as `Ok(empty)` so a read loop ends cleanly. |
| `TANError` | Protocol | The transaction number was the reserved sentinel or otherwise invalid. |
| `MalformedRequest` | Framing | The request could not be parsed. Fix the payload shape. |
| `OtherError` | Catch-all | Unspecified failure. |

`FSError::is_fatal()` flags `OutOfMemory`, `NotInitialized`, and
`MediaNotPresent` — conditions a retry will not fix. `FSError::is_retryable()`
flags `TooManyOpen`, `MaxHandles`, and `WriteFail` — transient conditions where
a backoff-and-retry is the right move.

### Idempotency: the TAN

The protocol cannot tell a lost request from a lost response, so every request
carries a **transaction number**. The client increments its TAN for each new
request (wrapping `0..=0xFE`; `0xFF` is the `INVALID_TAN` sentinel). The server
caches the last response per TAN per client: if the same TAN arrives again it
**replays the cached response instead of re-executing**. That is what makes a
retry safe — re-sending a `ReadFile` with the same TAN cannot accidentally
advance the file twice. In `machbus` the server keeps a `TANResponse` cache
that expires on a timer; the client tracks each outstanding request and matches
the reply by TAN before firing the event.

## Large files over the transport

A single CAN frame holds eight data bytes. Anything longer — a read of more than
a few bytes, a directory listing, a write payload — is segmented by the
transport layer. The FS messages ride `PGN_FILE_CLIENT_TO_SERVER` and
`PGN_FILE_SERVER_TO_CLIENT`; when a payload exceeds eight bytes the stack uses
the transport or extended transport protocol automatically. Two consequences
for your code: large transfers take time (so the per-request timeout matters,
and the server sends a busy status if it cannot answer promptly), and you
should size each `ReadFile`/`WriteFile` to the chunk your buffer can hold rather
than trying to move a whole file in one call. See
[Transport protocol](../standards/iso11783-datalink-transport.md) for the segmentation details and
[File Server and large data](../standards/iso11783-file-server.md)
for the conceptual primer.

## Doing it with machbus

### The client (low level)

`fs::FileClient` is pump-style. Operation methods build the outbound payload and
return it as `FSClientOutbound`; you ship it on the bus. Responses arrive
through `handle_server_response`, which decodes the frame and fires a
per-operation `Event` carrying `Result<T, FSError>`. `update` paces the CCM
keepalive, retries expired requests, and disconnects on a server-status
timeout. The fallible `try_*` variants return a precise local error instead of
a silent `None` when a request cannot even be built (not connected, bad path,
unknown handle, oversized payload).

The handshake is: `connect_to_server` (which emits the initial
`GetFileServerProperties` request), then a properties response transitions the
client from `WaitingForStatus` to `Connected` and fires `on_connected`. Only
then will `open_file`, directory operations, and the rest produce frames.

### The server (low level)

`fs::FileServer` is the enhanced, TAN-idempotent server. You pre-load files and
directories with `add_file` / `add_directory`, name the volume with
`set_volume_name`, and tune limits through `FileServerConfig` (per-client cap,
server-wide cap, CCM timeout, status cadence). Feed inbound frames to
`handle_client_message`, which returns the response frame(s) to ship; call
`update` to advance timers, run the volume state machine, prune timed-out
clients, and emit periodic status broadcasts. The server also exposes volume
management — `prepare_volume_for_removal`, `set_volume_removed`,
`reinsert_volume` — and fires events (`on_client_connected`, `on_file_opened`,
`on_volume_removed`, and so on).

The `file_server_demo` example drives a full open/write/seek/read round trip
against a `FileServer` directly. The client (`0x42`) creates a file with
`Write | Create`, gets a handle back, and writes into it:

```rust
{{#include ../../../examples/file_server_demo.rs:15:24}}
```

It then writes five bytes, seeks back to the start, and reads them again:

```rust
{{#include ../../../examples/file_server_demo.rs:26:35}}
```

```rust
{{#include ../../../examples/file_server_demo.rs:42:56}}
```

### The session facade (recommended for applications)

The [session facade](../getting-started/first-node.md) exposes both roles
through the `FsClient` and `FsServer` plugins. Inbound frames are routed and
keepalives / status broadcasts are shipped automatically on each
`driver.poll()?`. The client plugin turns each operation into an async-style
request: `open`, `read`, `write`, `seek`, `close`, `current_directory`, and
`change_directory` each return the TAN immediately, and the matching reply
arrives later as an `FsEvent` (`OpenResponse`, `ReadResponse`, `WriteResponse`, …)
you drain with `ctrl.drain::<FsEvent>()`. The shape on the client side:

```rust
use machbus::session::{Session, EndpointTransport, plugins::FsClient};

let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(FsClient::new(FileClientConfig::default()))
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;

ctrl.with_mut::<FsClient, _>(|fs| fs.connect_to(server_addr))?;
// ... poll until FsEvent::Connected ...
let tan = ctrl.with_mut::<FsClient, _>(|fs| fs.open("\\logs\\a.txt", OpenFlags::Read.bit()))?;
// ... poll; then match FsEvent::OpenResponse { tan, result } ...
```

The server side plugs the `FsServer` plugin, configured with a `root`, a
`volume_name`, and a `max_clients`, then populated through fine control. A session
with `FsServer` plugged pre-loads two files and a directory, claims an address,
and polls the server idle. `ctrl.with_mut::<FsClient, _>(|fs| ...)` and
`ctrl.with_mut::<FsServer, _>(|fs| ...)` reach the underlying `FileClient` /
`FileServer` for the methods not surfaced directly.

## Events and responsibilities

**Client responsibilities.** Connect before doing anything else and wait for the
properties response. Keep ticking so CCM keepalives flow — if they stop for the
timeout window the server drops you and your handles. Match every response to
its request by TAN, and on a timeout re-send with the *same* TAN so the server's
idempotency cache protects you. Close handles you open; on disconnect the client
emits close-file frames for anything still open, but you should not rely on that
as your only cleanup.

**Server responsibilities.** Validate every path and reject traversal,
host-absolute paths, and illegal characters. Scope each handle to its owner and
never honour a cross-client handle. Enforce both the per-client and the
server-wide open-file caps, and keep the advertised properties consistent with
the real limits. Cache responses by TAN for idempotent retries. Broadcast status
on cadence (slower when idle, faster when busy) and announce volume state
changes so clients can react to removal.

## Edge cases and failures

- **Handle exhaustion.** When a client hits its per-client cap the open returns
  `TooManyOpen`; when the whole server is out of slots it returns `MaxHandles`.
  Both are retryable after closing files or waiting.
- **Path not found.** A non-existent path returns `NotFound` unless the open
  carries `Create`. A `..` or host-absolute path is rejected as an invalid name
  before it ever reaches the filesystem.
- **Permission denied.** Writing or deleting a read-only file, moving onto an
  existing name, or an exclusive create over an existing file all return
  `AccessDenied`. So does touching a file that is currently open elsewhere.
- **Volume removed mid-transfer.** Once the volume goes to the removed state the
  server clears all open handles and rejects file operations with
  `MediaNotPresent`. In-flight handles are gone; the client must wait for the
  volume to return and re-open.
- **Busy server.** If a request takes long enough, the server flips to busy and
  broadcasts that status faster, so the client knows the delay is the server
  working, not a lost message — and should keep waiting rather than retry early.
- **Stale handle.** Using a handle after close, after a client timeout, or after
  a volume removal returns `InvalidHandle`. Re-open to get a fresh one.
- **Lost request or response.** Indistinguishable to the client; the cure is the
  same — retry with the same TAN and let the server replay or execute exactly
  once.

## Advanced

- **Multiple clients.** The server tracks each client independently: separate
  current directory, separate handle set, separate TAN cache. There is no
  interference between sessions, and the connection manager caps the number of
  simultaneous clients.
- **Handle ownership scope.** Handles never cross client boundaries. Two clients
  can hold handles to the *same* file, but each has its own position; the server
  blocks a move/delete/attribute change while a file is open by anyone.
- **Concurrency.** Both the client and the server are single-threaded pump
  state machines — you advance them with `update`/`tick` and they never block.
  Concurrency between nodes is mediated entirely by the request/response and TAN
  rules, not by locks.
- **Persistence.** `machbus`'s server keeps files in memory (pre-loaded with
  `add_file`); a real deployment backs the namespace with actual media and is
  responsible for mounting, naming volumes, and reporting removal. The protocol
  surface is identical either way.
- **Session facade vs the bare codecs.** Use the `FsClient` / `FsServer` plugins
  for applications — they route frames, ship keepalives, and fan out events for
  you. Use the raw `FileClient` / `FileServer` for tests and tightly controlled
  loops where you own every frame and every millisecond.

## Validate locally

```sh
make run EXAMPLE=file_server_demo
make test
```

`file_server_demo` runs an open/write/seek/read round trip against a
`FileServer` and asserts the bytes come back intact. The session tests build the
`FsServer` plugin on a virtual bus, pre-load files, claim an address, and poll
idle without a client. `make test` runs the unit and protocol-fixture
suites, including the path-safety, owner-scoping, TAN-idempotency, and
malformed-request rejection tests in the FS modules.

## What this proves / does not prove

Proves: the FS operation set, the handle lifecycle, the TAN-idempotency cache,
the path-safety rules, and the error mapping behave correctly in software, and
the `machbus` client and server drive each other to a clean round trip.

Does not prove: real-hardware media behavior, real transport timing across a
loaded bus, interoperability with a specific third-party file server or client,
or any conformance/certification claim. Those still require official standards,
real hardware, and interoperability evidence.

## See also

- [File Server and large data](../standards/iso11783-file-server.md)
  — the conceptual primer on why files (not frames) need their own service.
- [Transport protocol](../standards/iso11783-datalink-transport.md) — how multi-frame FS payloads are
  segmented and reassembled.
- [VT object pools](vt-object-pools.md) — a common use of the FS: a stored
  object pool a virtual terminal can load.
- [Address claim](address-claim.md) — the claim every FS node does before it
  may send or answer.
