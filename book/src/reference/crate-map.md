# Crate map

| Area | Path | Purpose |
| --- | --- | --- |
| Low-level CAN/J1939 | `src/net/` | identifiers, address claim, TP/ETP, sessions |
| CAN transport seam | `src/net/can_transport.rs`, `src/net/can_adapter.rs` | crate-owned `CanTransport` boundary plus hosted adapter isolation |
| J1939 diagnostics | `src/j1939/` | DM messages and diagnostic helpers |
| ISOBUS services | `src/isobus/` | VT, TC, FS, SC, AUX, guidance, implement data |
| NMEA | `src/nmea/` | NMEA 2000 and NMEA 0183 GNSS/navigation helpers |
| Hosted session facade | `src/session/` | sans-IO `Session` core, `Plugin`s, `Driver`/`Controls`, presets, typed events in hosted builds |
| Embedded session facade | `src/embedded_session.rs` | `no_std + alloc` `Session`, `Driver::poll_at`, caller-owned transport loop |
| Fixed-capacity helpers | `src/fixed.rs` | `embedded` queues, slots, byte buffers, and bounded message helpers |
| Time | `src/time.rs` | `Instant` — the monotonic timestamp injected into the sans-IO core |
| Lightweight geo | `src/geo.rs` | protocol-facing WGS/ECEF/local-frame types, with optional hosted `concord` conversions |
| VT storage blobs | `src/vt_storage.rs` | storage-agnostic VT stored-pool encode/decode for embedded-owned persistence |
| C ABI | `src/ffi.rs` | exported C surface |
| Python | `src/python/` | Python extension bindings |
| Examples | `examples/` | runnable API demonstrations |

## How to choose the right layer

| Need | Start here | Why |
| --- | --- | --- |
| Decode or encode a single frame/payload | `src/net/`, `src/j1939/`, `src/isobus/`, `src/nmea/` | lowest surface with byte-level types |
| Build an ECU-like application | `src/session/` | plugin-composed, sans-IO core + driver/handle split — see [The session facade](../guide/session-facade.md) |
| Build MCU firmware | `src/embedded_session.rs`, `src/net/can_transport.rs`, `src/fixed.rs` | board-owned clock/CAN/storage with `no_std + alloc`; see [`no_std` on microcontrollers](../getting-started/no-std-microcontrollers.md) |
| Build a common tractor/implement/VT/TC role | `src/session/presets.rs` | curated plugin groups for a role |
| Expose to C | `src/ffi.rs` and `include/machbus.h` | stable opaque-handle API |
| Expose to Python | `src/python/` | Pythonic wrappers and event dictionaries |
| Prove behavior with executable samples | `examples/` | commands are documented in the examples chapters |

## Session plugins

In hosted/default `machbus::session`, each subsystem is a `Plugin` you
`.plug(...)` and reach by type with `session.get_mut::<P>()` /
`controls.with_mut::<P>(...)`:

| Subsystem | Plugin (`session::plugins`) |
| --- | --- |
| Diagnostics (DM1) | `Diagnostics` |
| GNSS / NMEA 2000 | `Gnss` |
| Virtual Terminal | `VtClient`, `VtServer` |
| Task Controller | `TcClient`, `TcServer` |
| File Server | `FsClient`, `FsServer` |
| Implement messages | `Implement` |
| Sequence Control | `ScMaster`, `ScClient` |
| TIM | `Tim` |
| Powertrain | `Powertrain` |
| Heartbeat / Maintain Power | `Heartbeat`, `MaintainPower` |
| Shortcut Button / Language | `ShortcutButton`, `LanguageCommand` |
| Auxiliary / DM memory | `Auxiliary`, `DmMemory` |
| Functionalities / Group fn / Request2 / NAME mgmt | `ControlFunctionalities`, `GroupFunction`, `Request2`, `NameManagement` |

See [The session facade](../guide/session-facade.md) for the full surface.

Embedded builds do not expose every hosted plugin wrapper. They expose the
caller-driven `Session` loop plus protocol components that can compile without
`std`, including core network/J1939/NMEA helpers and heap-backed VT/TC/FS pump
state. Use [Feature flags](feature-flags.md) and
[`no_std` on microcontrollers](../getting-started/no-std-microcontrollers.md)
for the current embedded boundary.

## Reaching a plugged subsystem

You reach any plugged subsystem by type and call its own methods:
`controls.with_mut::<P>(...)` (or `session.get_mut::<P>()`). Both return `None`
when that plugin was not plugged, so check the result rather than assuming the
subsystem is present.
