# Feature flags

`machbus` separates hosted integration surfaces from embedded protocol surfaces.
The default build is intentionally convenient for Linux/desktop development,
while embedded users opt into a smaller `no_std + alloc` surface by disabling
default features.

## Current feature model

```toml
[features]
default = ["wirebit", "dep:pyo3", "dep:concord", "tracing/std"]
embedded = []
wirebit = ["dep:wirebit", "wirebit/socketcan"]
async = ["dep:futures-core"]
```

There are exactly four features: `default`, `embedded`, `wirebit`, and `async`.
There is no separate `std` or `alloc` knob — the `no_std` switch is keyed off
`embedded`, so a normal build is `std` (with an allocator) automatically, and an
`embedded` build is `no_std + alloc`. An allocator is always available.

Unless you select `embedded`, the C ABI, the Python bindings (`pyo3`), and the
rich geo conversions (`concord`) are **always** compiled — they are part of the
default hosted build, not separate features.

## The feature table

| Feature | What it enables | What it pulls in | When to use it |
| --- | --- | --- | --- |
| `default` | Full hosted stack: `std`, C ABI, Python bindings, rich geo conversions, and the `wirebit` host CAN backend | `wirebit`, `pyo3`, `concord`, `tracing/std` | Desktop/Linux development, bindings, simulator workflows |
| `embedded` | `no_std + alloc` protocol/session core plus allocation-free fixed-capacity helper primitives | (nothing; `no_std`) | Microcontrollers or embedded Linux code that owns time, CAN IO, and storage |
| `wirebit` | Host CAN backend: virtual bus / simulation adapter and Linux SocketCAN | `wirebit`, `wirebit/socketcan` | Real or virtual Linux CAN interfaces and host-adapter examples |
| `async` | Runtime-agnostic async event stream pieces | `futures-core` | Local-executor event consumption |

## Hosted/default mode

With normal dependency syntax:

```toml
[dependencies]
machbus = { path = "../machbus" }
```

you get the hosted default surface. This is the right mode for examples that use
the virtual bus, C ABI work, Python binding work, rich `concord` conversions, and
ordinary Linux/desktop development.

## Embedded `no_std + alloc` mode

Embedded users should disable defaults:

```toml
[dependencies]
machbus = { path = "../machbus", default-features = false, features = ["embedded"] }
```

This compiles the embedded public surface as `no_std + alloc`. The embedded loop
owns:

- monotonic time (`machbus::time::Instant` values supplied by the caller);
- CAN receive/transmit through `machbus::net::CanTransport`;
- storage for NIU config text, IOP bytes, and VT stored-pool blobs;
- scheduling and task wakeups.

The embedded feature does **not** compile:

- the C ABI;
- the Python bindings;
- SocketCAN / `wirebit`;
- `concord`;
- host file load/save helpers;
- host-clock `Driver::poll()` convenience.

Use `Driver::poll_at(now)` or the embedded `Session` loop shape instead.
The embedded `Session` enables `IsoNet`'s direct message-capture queue, so
decoded messages are drained without a boxed callback listener at the session
boundary. Hosted callback dispatch remains explicit and opt-in.
Selected ISO 11783 application codecs are also embedded-available, including
AUX, legacy file-transfer types, Functionalities, Group Function, Guidance,
TIM, implement/tractor message codecs, and Sequence Control core
state/recording/TAN helpers. `machbus::isobus::tc` also builds in embedded
mode for heap-backed DDOP/object codecs, TC client/server pump state, TC-GEO,
grids, task lifecycle/logging, rate limiting, outstanding-request tracking,
ISOXML parsing, and task totals. `machbus::isobus::fs` builds in embedded mode
for File Client / File Server pump state, ISO 11783-13 operation/type/property
codecs, path validation, in-memory file storage, and volume helpers; real media
persistence stays application-owned. `machbus::isobus::vt` builds in embedded
mode for object pools, VT Client / VT Server pump state, update helpers,
storage-agnostic stored-version blobs, and working-set state; VT render/GTUI
remains hosted.

### Fixed-capacity helpers (embedded)

The `embedded` profile also ships allocation-free fixed-capacity helper
primitives for bounded-memory critical paths:

- `machbus::fixed::FixedQueue<T, N>`, `FixedFrameQueue<N>`, `FixedSlots<T, N>`,
  `FixedBytes<N>`, `FixedMessage<N>`;
- `machbus::net::TpCmdtTx<'_>`, `TpRxFixed<N>`, `EtpCmdtTx<'_>`, `EtpRxFixed<N>`;
- `machbus::session::FixedEvent<N>`;
- `Session::poll_fixed_event::<N>()` / `Driver::poll_fixed_at::<N>()`;
- `examples/embedded_fixed_queue.rs`, which uses fixed RX/TX transport buffers.

In `no_std` builds `IsoNet` uses a fixed-capacity pending TP/ETP transmit queue
for the deferred multi-frame send path; TP and ETP active-session tables use
fixed-capacity slot storage; Fast Packet receive-session slots and reassembly
payloads are fixed-capacity. `FastPacketProtocol::process_frame_fixed::<N>()`
returns a bounded `FixedMessage<N>` without allocating, and
`FastPacketProtocol::send_fixed::<N>()` avoids a growable frame vector. TP/CMDT
and ETP/DPO pending transmit windows have fixed variants
(`get_pending_data_frames_fixed::<N>()`), broadcast BAM transfers can use
`send_bam_fixed::<N>()`, and the borrowing transmit/receive helpers
(`TpCmdtTx`, `TpRxFixed`, `EtpCmdtTx`, `EtpRxFixed`) reassemble or emit
fixed-capacity batches without copying into the heap-backed session stores.
Oversized fixed events are reported explicitly instead of being truncated.

This is **not** yet a full no-alloc profile: the session core still uses the
heap-backed `embedded` profile. The fixed-capacity helpers are the
allocation-free critical-path building blocks introduced ahead of the larger
internal queue/cache/reassembly migration.

## Validation targets

Use the Makefile targets rather than ad-hoc Cargo commands:

```sh
make no-std-check
make no-std-target-check
make no-std-surface-check
make embedded-examples-check
make wirebit-examples-check
```

`make no-std-target-check` checks the `embedded` feature on the documented
embedded target. `make no-std-surface-check` compiles the embedded public API
imports and minimal loop shape in a dedicated test.

For dependency audits:

```sh
cargo tree --no-default-features --features embedded -e normal
```

The embedded graph should stay free of `wirebit`, `pyo3`, and `concord`.

## Combining features

Hosted features combine freely when they make sense. `wirebit` implies the
hosted transport path and is not part of the embedded profile. `async` is
runtime-agnostic, but it does not make the session `Send` or thread-safe; it
remains a local, explicitly pumped model.

## What this proves / does not prove

Feature flags describe compile-time surface area and dependencies. They do not
claim official ISO 11783, SAE J1939, NMEA, or AEF certification. A real
deployment still needs official standards access, hardware evidence, and
interoperability testing.

## See also

- [`no_std` on microcontrollers](../getting-started/no-std-microcontrollers.md)
- [Crate map](crate-map.md)
- [Validation gates](validation-gates.md)
- [Hardware evidence](hardware-evidence.md)
- [Bindings overview](../bindings/index.md)
