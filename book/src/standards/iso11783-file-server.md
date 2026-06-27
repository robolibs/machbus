# ISO 11783-13 — the File Server

Part 13 puts a filesystem on the bus. A File Server offers volumes and directories;
clients open, seek, read, write, and close files, manage attributes, and query free
space — each as a request/response exchange. It is how implements persist
configuration, prescriptions, and logs without storage of their own.

## Why this exists

A small implement ECU may have little or no non-volatile memory, yet it needs to load a
prescription, save an as-applied log, or keep settings across power cycles. Rather than
give every ECU a card slot, ISOBUS lets one node be the *server* and everyone else
share it.

## The conversation

Every operation is a request tagged with a **transaction number (TAN)**; the matching
response carries the same TAN, so a client can have several in flight:

```
   CLIENT                                   SERVER
     │ ── open "/presc.bin", read ──► (TAN 7)
     │ ◄── handle 3, ok ────────────
     │ ── read handle 3, 512 bytes ─► (TAN 8)
     │ ◄── 512 bytes ───────────────          (large reads ride Transport Protocol)
     │ ── write handle 3, <data> ──► (TAN 9)
     │ ◄── 512 bytes written ───────
     │ ── close handle 3 ──────────► (TAN 10)
     │ ◄── ok ──────────────────────
```

The full surface: connect (a CCM handshake), open/close, read/write/seek, get/set
attributes, get date-time, current/change directory, move/delete, volume init and
status, plus free-space queries.

## How machbus expresses it

machbus implements both halves with an **async-style request → event** model: each
request method returns its TAN immediately, and the matching response arrives later as
an `FsEvent`. The `FsClient` plugin drives the client; `FsServer` serves in-memory
files and directories.

```
   let tan = ctrl.with_mut::<FsClient, _>(|fs| fs.open("/presc.bin", flags))??;
   // … later …
   // Event::Fs(FsEvent::OpenResponse { tan, result: Ok(handle) })
```

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Reading/writing files (client) | `session::plugins::FsClient` | [File Server](../tutorials/file-server.md) |
| Serving files (server) | `session::plugins::FsServer` | [File Server](../tutorials/file-server.md) |
| The FS codecs directly | `isobus::fs` | [File Server](../tutorials/file-server.md) |

## Failure modes worth knowing

- **TAN confusion** — matching a response to the wrong request if TANs are not tracked.
- **Not connected** — issuing file ops before the CCM handshake completes.
- **Big transfers** — large reads/writes ride the transport protocol and inherit its
  timeout/abort behavior.

## See also

- [ISO 11783-3 — data link & transport](iso11783-datalink-transport.md) — what carries the big
  reads and writes.
- [Implement control, the tractor ECU, and the rest](implement-and-services.md).
