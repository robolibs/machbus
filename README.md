# machbus

`machbus` is a Rust port of the C++ `machbus` library — an ISO 11783
(ISOBUS) + J1939 + NMEA 2000 networking stack for agricultural and
off-highway control electronics. It targets the same CAN/ISOBUS wire
formats used by tractors, implements, VTs, and task controllers, and it
ships with a virtual-bus simulator so you can develop and test in
software. Local fixture and smoke coverage is documented below; official
AEF/ISOBUS certification or hardware interoperability is not claimed.

## What's in the box

- **ISO 11783 / J1939 transport** — single-frame, BAM, CMDT
  (RTS/CTS/EoMA/Abort), ETP (with DPO), NMEA 2000 fast-packet, address
  claim with NAME arbitration.
- **Surface API** — opinionated facade (`Stack<L>`) that hides PGN
  routing, address-claim sequencing, and event fan-out behind one
  handle.
- **Subsystem handles** — `stack.diag()`, `stack.gnss()`, `stack.vt()`,
  `stack.tc()`, `stack.fs()`, `stack.imp()`, plus matching server-side
  handles `vt_server()`, `fs_server()`, `tc_server()`.
- **Persona builders** — `Tractor`, `Implement`, `VirtualTerminal`,
  `FileServer`, `TaskControllerServer` for the common ECU roles.
- **Helpers** — `AlarmPanel` (priority-stack alarms), `VtBatch`
  (deduplicated VT mutations), `RateThrottle` (period-based coalescer
  with clamp), `SectionRouter` (logical-section ↔ VT indicator
  composer), `TempVisibility` (RAII-style auto-revert).
- **Type-system flourishes** — newtype IDs (`vt::ObjectID`,
  `tc::ObjectID`, `DDI`, `ElementNumber`, `SectionId`, `AlarmId`),
  typestate (`Stack<L, Disconnected>` → `Stack<L, Claimed>`).
- **Async (feature-gated)** — `Stack::events_async()` returns a
  runtime-agnostic, waker-backed `futures_core::Stream<Event>` without
  holding a mutable stack borrow. The stack remains single-threaded and
  `!Send`: run it on a local executor and keep driving `tick()` yourself.
- **Bindings** — C ABI (in `include/machbus.h`) and Python bindings
  (via `pyo3` + `maturin`) over the session/facade subsystems,
  multi-node bus topologies, and helper types. The hosted VT rendering
  runtime/framebuffer remains a Rust-only surface for now.

## Status

- Rust core: `make test` and `cargo test --all-targets --all-features`
  pass locally.
- Embedded core: the `embedded` feature compiles the protocol/session subset as
  `no_std + alloc`; validate with `make no-std-check`,
  `make no-std-target-check`, `make no-std-surface-check`, and
  `make embedded-examples-check`.
- C ABI + Python bindings: smoke-tested via `make c-demo`,
  `make c-full-demo`, and `make python-demo`; the C demos and ABI layout
  probe compile with C warnings denied.
- Cross-implementation byte-exact compatibility with AgIsoStack is
  covered for NAME, Identifier, heartbeat sequencing, and TimeDate
  fixtures.

See also:

- [`book/`](book/) — new mdBook source, starting with conformity boundaries
  and then practical ISOBUS/J1939/NMEA tutorials.
- [`book/src/reference/protocol-coverage.md`](book/src/reference/protocol-coverage.md)
  and [`book/src/reference/assets/protocol_matrix.csv`](book/src/reference/assets/protocol_matrix.csv)
  — protocol coverage evidence.
- [`book/src/reference/hardware-evidence.md`](book/src/reference/hardware-evidence.md)
  and [`book/src/reference/audit/conformance.md`](book/src/reference/audit/conformance.md)
  — hardware evidence path and non-claim boundary.
- [`book/src/reference/validation-history.md`](book/src/reference/validation-history.md),
  [`book/src/reference/release.md`](book/src/reference/release.md),
  [`book/src/reference/audit/bindings.md`](book/src/reference/audit/bindings.md),
  [`book/src/reference/audit/hardening-plan.md`](book/src/reference/audit/hardening-plan.md),
  and [`book/src/reference/behavior-differences.md`](book/src/reference/behavior-differences.md)
  — archived audit/release/binding material integrated into the new book.

Sibling local dependencies:

- `../wirebit` — virtual-bus + `Link` trait
- `../concord` — optional rich WGS / ECEF / ENU geo conversions for
  `geo-concord`

## Install (Rust)

```toml
[dependencies]
machbus = { path = "../machbus" }
```

Features (exactly four):

| feature | what it enables |
|---|---|
| `default` | full hosted stack: `std`, C ABI, Python bindings (`pyo3`), rich geo (`concord`), and the `wirebit` host CAN backend |
| `embedded` | `no_std + alloc` protocol/session core (caller-owned clock and CAN transport) plus fixed-capacity helper primitives |
| `wirebit` | host CAN backend — virtual bus + Linux SocketCAN (included in `default`) |
| `async` | runtime-agnostic async event stream pieces |

The `no_std` switch is keyed off `embedded`, so a normal build is hosted (`std`)
automatically; the C ABI, Python bindings, and geo conversions are always part
of the default hosted build rather than separate features. Embedded users
disable defaults:

```toml
[dependencies]
machbus = { path = "../machbus", default-features = false, features = ["embedded"] }
```

## Embedded `no_std + alloc`

The embedded profile exposes a deterministic, caller-driven core:

- `machbus::session::Session`
- `machbus::session::Transport`
- `machbus::session::Driver::poll_at`
- `machbus::session::{FixedEvent, Session::poll_fixed_event, Driver::poll_fixed_at}` with `embedded`
- `machbus::net::CanTransport` for board/HAL-owned CAN adapters
- `machbus::fixed::{FixedQueue, FixedFrameQueue, FixedSlots, FixedBytes, FixedMessage}` with `embedded`
- `machbus::net::{TpCmdtTx, TpRxFixed, EtpCmdtTx, EtpRxFixed}` with `embedded`
- `machbus::net::{Frame, Identifier, Name, IsoNet, TransportProtocol, ExtendedTransportProtocol, FastPacketProtocol}`
- `machbus::net::{Niu, Router, NiuConfig}` for heap-backed NIU filtering/routing
- `machbus::net::{CanBusConfig, BusPowerSupply, SafetyPolicy, Scheduler, FaultConfinementMonitor}`
- `machbus::net::{parse_iop_data, hash_to_version}` for buffer-owned IOP inspection
- `machbus::vt_storage::StoredPoolVersion` for storage-agnostic VT stored-pool blobs
- `machbus::geo::{Wgs, Geo, Ecf}`
- `machbus::j1939` codecs
- `machbus::isobus` selected application codecs: AUX, legacy file-transfer
  types, functionalities, group-function, guidance, TIM, and implement/tractor
  message codecs, Sequence Control core state/recording/TAN helpers, and
  Task Controller `no_std + alloc` helpers (`tc::{DDOP, TaskSession,
  TCGEOInterface, TreatmentZoneGrid, TaskTotals, ...}`)
- `machbus::isobus::fs` File Server / File Client pump state, wire codecs,
  path validation, in-memory file server, and volume/property helpers
- `machbus::isobus::vt` object pools, VT Client / VT Server pump state,
  storage-agnostic stored-version blobs, and server/client working-set state
- `machbus::nmea` GNSS/NMEA 2000 payload helpers and NMEA 0183 parser
- explicit `machbus::time::Instant`

The embedded feature does **not** compile the C ABI, Python bindings,
SocketCAN, host file load/save helpers, `wirebit`, or `concord`.
NIU config text serialization is available for embedded callers via
`NiuConfig::to_persisted_string` / `NiuConfig::from_persisted_string`; disk
`save` / `load_from` remains hosted-only.
VT stored working-set versions follow the same storage split:
`machbus::vt_storage::StoredPoolVersion` is embedded-available for blob
encode/decode, while hosted builds additionally wire those blobs into VT
server path/file helpers. VT object-pool, client/server, and working-set state
are embedded-available; the renderer module remains hosted for now.
The embedded `Session` drains decoded network messages through an internal
sans-IO message-capture queue rather than installing a boxed callback listener;
hosted `IsoNet` callback behavior stays opt-in and unchanged.
When `embedded` is enabled, queued TP/ETP transmit requests inside
`IsoNet` use a fixed-capacity pending queue instead of a growable `VecDeque`.
In `no_std` `embedded` builds, TP and ETP active-session tables also use
fixed-capacity slot storage instead of growable session vectors.
Fast Packet receive-session slots are also fixed-capacity in that profile; in
`no_std` `embedded` builds, Fast Packet RX reassembly payloads use a
fixed-capacity inline buffer too.
`FastPacketProtocol::process_frame_fixed::<N>()` returns a bounded
`FixedMessage<N>` completion without allocating the heap-backed `Message`;
`FastPacketProtocol::send_fixed::<N>()` can build transmit frames into inline
storage.
For TP/CMDT and ETP/DPO windows,
`TransportProtocol::get_pending_data_frames_fixed::<N>()` and
`ExtendedTransportProtocol::get_pending_data_frames_fixed::<N>()` drain
pending data windows into fixed-capacity frame storage.
For broadcast TP/BAM, `TransportProtocol::send_bam_fixed::<N>()` builds the
complete ordered BAM + TP.DT frame list into fixed storage without retaining the
payload in a session; the caller owns CAN-port selection and inter-packet
pacing.
For connection-mode TP/CMDT, `TpCmdtTx<'_>` borrows the application payload and
emits RTS / CTS-window DT frames into fixed storage, avoiding the heap-backed
`TransportProtocol` transmit session for embedded callers that can keep the
payload slice alive.
For bounded TP receive paths, `TpRxFixed<N>` reassembles one BAM or CMDT
transfer into a fixed `FixedMessage<N>` and returns optional CTS/EOMA/abort
frames for the caller to transmit, avoiding the heap-backed
`TransportSession.data` receive buffer on that path.
For ETP/CMDT transmit, `EtpCmdtTx<'_>` borrows the application payload and
emits RTS plus DPO/DT CTS-window batches into fixed storage, avoiding the
heap-backed `ExtendedTransportProtocol` transmit session for that path.
For bounded ETP receive paths, `EtpRxFixed<N>` accepts one transfer whose
advertised size fits `N`, reassembles into `FixedMessage<N>`, and returns
optional CTS/EOMA/abort frames.

Minimal loop shape:

```rust
use machbus::net::{Frame, Name};
use machbus::session::Session;
use machbus::time::Instant;

let mut session = Session::builder(Name::default().with_self_configurable(true), 0x80).build()?;
session.start()?;

let mut now = Instant::ZERO;
loop {
    now = now.add_millis(10);

    while let Some((port, frame)) = board_can_receive() {
        session.feed(port, &frame, now);
    }

    session.tick(now);

    while let Some((port, frame)) = session.poll_transmit() {
        board_can_send(port, &frame)?;
    }

    while let Some(event) = session.poll_event() {
        handle_event(event);
    }
}
# fn board_can_receive() -> Option<(u8, Frame)> { None }
# fn board_can_send(_: u8, _: &Frame) -> machbus::net::Result<()> { Ok(()) }
# fn handle_event(_: machbus::session::Event) {}
# Ok::<(), machbus::net::Error>(())
```

See [`examples/embedded_session_loop.rs`](examples/embedded_session_loop.rs)
[`examples/embedded_hal_adapter.rs`](examples/embedded_hal_adapter.rs), and
[`examples/embedded_fixed_queue.rs`](examples/embedded_fixed_queue.rs).
Check it with:

```sh
make embedded-examples-check
make no-std-surface-check
```

## Quick start (Rust)

```rust
use std::time::Duration;
use machbus::net::Name;
use machbus::stack::Stack;
use wirebit::ShmLink;
use wirebit::topology::Topology;

// Build a single-node virtual bus.
let mut topo = Topology::new();
let n = topo.add_node("ecu");
topo.can_bus("bus0").members(&[n]);
let mut built = topo.build()?;
let ep = built.can_bus_mut("bus0").unwrap().take_endpoint("ecu").unwrap();

// Build a stack with diagnostics + GNSS + implement-message routing.
let mut stack = Stack::<ShmLink>::builder()
    .name(
        Name::default()
            .with_identity_number(0x100)
            .with_function_code(0x80)
            .with_self_configurable(true),
    )
    .preferred_address(0x80)
    .endpoint(0, ep)
    .with_diagnostics()
    .with_gnss(machbus::nmea::NMEAConfig::default())
    .with_implement()
    .build()?;

stack.start_address_claim()?;
let addr = stack.run_until_claimed(Duration::from_secs(2))?;
println!("claimed 0x{addr:02X}");

// Raise a DTC and broadcast a position.
stack.diag().raise(523_312, machbus::j1939::Fmi::AboveNormal);
stack.gnss().send_position(&machbus::nmea::GNSSPosition {
    wgs: machbus::geo::Wgs::new(52.5200, 13.4050, 0.0),
    ..Default::default()
})?;

loop {
    stack.tick(50);
    built.pump_all()?;
    while let Some(event) = stack.poll_event() {
        match event {
            machbus::stack::Event::Diag(d) => println!("diag: {d:?}"),
            machbus::stack::Event::Gnss(g) => println!("gnss: {g:?}"),
            other => println!("event: {other:?}"),
        }
    }
}
```

Runnable workflows live in [`examples/`](examples):

| file | what it shows |
|---|---|
| `surface_minimal.rs` | minimum-viable Stack — claim + custom PGN |
| `surface_diag.rs` | `stack.diag()` — DM1, DTCs, lamps |
| `surface_gnss.rs` | `stack.gnss()` — position / COG / SOG |
| `surface_tractor.rs` | `Tractor` persona with classification |
| `surface_implement.rs` | `Implement` persona + `AlarmPanel` |
| `surface_dual_role.rs` | two stacks talking on one bus |
| `surface_vt_server.rs` | `VirtualTerminal` persona |
| `surface_fs_server.rs` | `FileServer` persona |
| `surface_tc_server.rs` | `TaskControllerServer` persona |
| `surface_implement_imp.rs` | `stack.imp()` — hitch / PTO / aux valves |
| `surface_alarm_panel.rs` | `AlarmPanel` priority stack |
| `surface_section_router.rs` | `SectionRouter` composer |
| `surface_rate_throttle.rs` | `RateThrottle` coalescer |
| `surface_runtime_typestate.rs` | runtime-checked optional handles + claimed typestate |
| `surface_async.rs` | local-executor `events_async()` stream + manual ticking (`--features async`) |
| `surface_full_scenario.rs` | tractor + implement + VT end-to-end |

Run any of them with `cargo run --example <name>`.

## C ABI

| asset | path |
|---|---|
| header | [`include/machbus.h`](include/machbus.h) |
| Rust implementation | [`src/ffi.rs`](src/ffi.rs) |
| example | [`examples/c_abi/demo.c`](examples/c_abi/demo.c) |
| full-surface example | [`examples/c_abi/full_demo.c`](examples/c_abi/full_demo.c) |
| local makefile | [`examples/c_abi/Makefile`](examples/c_abi/Makefile) |

The C ABI exposes:

- `MachbusHandle` — single-node bundled topology + stack.
- `MachbusBusHandle` + `MachbusStackHandle` — multi-node topology with
  one `Stack` per node.
- `MachbusFullConfig` for enabling any combination of subsystems
  (diagnostics, GNSS, implement messages, VT/TC/FS clients,
  VT/FS/TC servers).
- `machbus_stack_new_full` and `machbus_stack_fs_*` /
  `machbus_stack_fs_server_*` for bus-attached C stacks that need the same
  multi-node File Server client/server workflow as the Rust and Python
  surfaces.
- All subsystem handles wired through `machbus_diag_*`,
  `machbus_gnss_*`, `machbus_imp_*`, `machbus_vt_*`, `machbus_tc_*`,
  `machbus_fs_*`, `machbus_vt_server_*`, `machbus_fs_server_*`,
  `machbus_tc_server_*`.
- The C ABI does not expose the hosted VT render runtime, `IopDocument`,
  `RenderCommand`, or framebuffer APIs yet; C callers can drive VT
  client/server session traffic but not render object pools through this
  facade.
- Helper types: `MachbusAlarmPanel`, `MachbusVtBatch`,
  `MachbusRateThrottle`, `MachbusSectionRouter`,
  `MachbusTempVisibility` — opaque handles with `_new` / `_free` /
  methods.
- Event polling via `machbus_poll_event` + `MachbusEvent` POD.
- Thread-local last-error string via `machbus_last_error_message`.

Build and run:

```sh
cargo build
cd examples/c_abi
make            # builds + runs the basic demo
make run-full   # builds + runs the full-surface demo
```

### Sample (C)

```c
#include <math.h>
#include <stdio.h>
#include "machbus.h"

int main(void) {
    MachbusFullConfig cfg = machbus_default_full_config();
    cfg.base.name_raw           = machbus_name_build(0x100, 0x80, true);
    cfg.base.preferred_address  = 0x80;
    cfg.base.enable_diagnostics = true;
    cfg.base.enable_gnss        = true;
    cfg.base.enable_implement   = true;

    MachbusHandle* h = machbus_new_full(&cfg);
    if (!h) {
        fprintf(stderr, "machbus_new_full: %s\n", machbus_last_error_message());
        return 1;
    }
    machbus_start_address_claim(h);
    machbus_run_until_claimed(h, 2000);
    printf("claimed address = 0x%02X\n", machbus_address(h));

    machbus_diag_raise(h, 523312, 0);   /* SPN, FMI */
    MachbusGnssPosition pos = {
        .latitude = 52.5200, .longitude = 13.4050,
        .altitude_m = NAN, .speed_mps = 3.5, .heading_rad = NAN,
    };
    machbus_gnss_send_position(h, pos);
    machbus_imp_command_pto_speed(h, MACHBUS_PTO_REAR, 4320, 10);

    for (int i = 0; i < 40; i++) machbus_tick(h, 50);

    MachbusEvent ev;
    while (machbus_poll_event(h, &ev)) {
        printf("event kind=%d source=0x%02X\n", (int)ev.kind, ev.source);
    }

    machbus_free(h);
    return 0;
}
```

A multi-node bus is built up explicitly:

```c
MachbusBusHandle* bus = machbus_bus_new();
machbus_bus_add_node(bus, "tractor");
machbus_bus_add_node(bus, "implement");
machbus_bus_build(bus);

MachbusConfig cfg_t = machbus_default_config();
cfg_t.preferred_address  = 0xF0;
cfg_t.enable_implement   = true;

MachbusStackHandle* tractor   = machbus_stack_new(bus, "tractor",   &cfg_t);
MachbusStackHandle* implement = machbus_stack_new(bus, "implement", &cfg_t);

machbus_stack_start_address_claim(tractor);
machbus_stack_start_address_claim(implement);
for (int i = 0; i < 30; i++) {
    machbus_stack_tick(tractor, 50);
    machbus_stack_tick(implement, 50);
    machbus_bus_pump(bus);
}
machbus_stack_imp_command_hitch(tractor, MACHBUS_HITCH_REAR, MACHBUS_HITCH_CMD_RAISE);
```

## Python bindings

| asset | path |
|---|---|
| Rust module implementation | [`src/python/mod.rs`](src/python/mod.rs) |
| packaging config | [`pyproject.toml`](pyproject.toml) |
| example | [`examples/python_binding/basic.py`](examples/python_binding/basic.py) |
| regression smoke | [`examples/python_binding/regression.py`](examples/python_binding/regression.py) |
| local makefile | [`examples/python_binding/Makefile`](examples/python_binding/Makefile) |

The Python surface exposes (every class is `unsendable` because the
underlying `Stack<L>` uses `Rc<RefCell<…>>`):

- `machbus.Machbus(name, preferred_address, enable_diagnostics, …)` —
  single-node stack. The same constructor also accepts the full subsystem
  flags for VT/TC/FS clients and VT/FS/TC servers.
- `machbus.Bus()` + `machbus.BusStack(bus, node, …)` — multi-node bus
  with one stack per node.
- Personas: `machbus.Tractor(...)`, `machbus.Implement(...)`.
- Helpers: `machbus.AlarmPanel()`, `machbus.VtBatch()`,
  `machbus.TempVisibility.show(stack, id, duration_ms)`,
  `machbus.TempVisibility.hide(stack, id, duration_ms)`,
  `machbus.RateThrottle(period_ms, clamp=(min,max))`,
  `machbus.SectionRouter(section_count)`. `RateThrottle` rejects zero periods
  and inverted clamps; `SectionRouter` accepts section counts in `1..=254`.
- Module-level helpers: `machbus.name(identity, function_code,
  self_configurable)` for building NAME values, `machbus.__version__`.
- The hosted VT render runtime/framebuffer is not currently bound in Python.
  Python callers can exercise VT client/server session traffic through the
  facade, but object-pool layout/rendering remains Rust-only.

Build and install with `maturin`:

```sh
nix develop
cd examples/python_binding
make test
```

The example makefile uses `PYO3_PYTHON` from the flake shell, creates a
local `.venv`, and installs the extension with `maturin develop` so
`pyo3`, `maturin`, and the runtime interpreter stay aligned. `make test`
runs both the readable `basic.py` smoke and a pytest-shaped dependency-free
`regression.py` runner, which asserts single-node, multi-node, persona, helper,
event-queue, full-flag, disabled-client guard, error-message, and event-schema
behavior without requiring pytest.

### Sample (Python)

```python
import math
import machbus

a = machbus.Machbus(
    name=machbus.name(0x100, 0x80, True),
    preferred_address=0x80,
    enable_diagnostics=True,
    enable_gnss=True,
    enable_implement=True,
)

a.start_address_claim()
print("address =", hex(a.run_until_claimed(2_000)))

a.diag_raise(523_312, 0)   # SPN, FMI=AboveNormal
a.gnss_send_position(
    latitude=52.5200,
    longitude=13.4050,
    speed_mps=3.5,
    heading_rad=math.pi / 4,
)
a.imp_command_pto_speed("rear", 4320, 10)

for _ in range(40):
    a.tick(50)

for ev in a.drain_events():
    print(ev["kind"], ev.get("sub", ""), {k: v for k, v in ev.items() if k not in ("kind", "sub")})
```

Multi-node bus from Python:

```python
bus = machbus.Bus()
bus.add_node("tractor")
bus.add_node("implement")
bus.build()

tractor = machbus.BusStack(
    bus, "tractor",
    name=machbus.name(0x100, 0x80, True),
    preferred_address=0xF0,
    enable_implement=True,
)
implement = machbus.BusStack(
    bus, "implement",
    name=machbus.name(0x42, 0x80, True),
    preferred_address=0x80,
    enable_implement=True,
)

tractor.start_address_claim()
implement.start_address_claim()
for _ in range(30):
    tractor.tick(50)
    implement.tick(50)
    bus.pump()

tractor.imp_command_hitch("rear", "raise")
for _ in range(10):
    tractor.tick(50); implement.tick(50); bus.pump()

for ev in implement.drain_events():
    print("implement saw:", ev)
```

## Common patterns

### Driving a tractor persona

```rust
use std::time::Duration;
use machbus::net::Name;
use machbus::stack::Tractor;

let mut tractor = Tractor::<wirebit::ShmLink>::builder()
    .name(Name::default().with_identity_number(0x100).with_function_code(0x80))
    .preferred_address(0xF0)
    .endpoint(0, ep)
    .with_navigation()
    .with_front_hitch()
    .with_gnss()
    .build()?;

tractor.run_until_claimed(Duration::from_secs(2))?;
println!("{}", tractor.classification());  // "Class 1NF"
```

### Typestate-typed claim transition

```rust
use std::time::Duration;
use machbus::stack::{Claimed, Stack};

let stack = Stack::<wirebit::ShmLink>::builder()/* … */.build()?;
let stack: Stack<_, Claimed> = stack.run_until_claimed_typed(Duration::from_secs(2))?;
// `claimed_address()` is infallible at the type level.
println!("claimed: 0x{:02X}", stack.claimed_address());
```

### `AlarmPanel` priority stack

```rust
use machbus::j1939::Fmi;
use machbus::stack::{AlarmPanel, AlarmPriority};

let mut panel = AlarmPanel::new();
panel.raise(&mut stack, "LOW_FUEL",  96,  Fmi::BelowNormal,    AlarmPriority::Warning,  "Refill needed");
panel.raise(&mut stack, "TANK_EMPTY", 96, Fmi::ConditionExists, AlarmPriority::Critical, "Stop");
if let Some((key, prio, msg)) = panel.current() {
    println!("[{prio:?}] {key} — {msg}");  // [Critical] TANK_EMPTY — Stop
}
```

## Layout

```
machbus/
├── src/
│   ├── ffi.rs           ← C ABI surface
│   ├── python/mod.rs    ← pyo3 module (hosted, in `default`)
│   ├── stack/           ← surface API (Stack, handles, personas, helpers)
│   ├── isobus/          ← VT, TC, FS, implement messages, etc.
│   ├── j1939/           ← J1939 codec layer
│   ├── nmea/            ← NMEA 2000 + NMEA 0183
│   ├── net/             ← IsoNet orchestrator + transports
│   └── lib.rs
├── include/machbus.h    ← C header
├── pyproject.toml       ← maturin config
├── Cargo.toml
├── Makefile             ← top-level convenience targets
├── flake.nix            ← nix dev shell with maturin + python
├── examples/
│   ├── c_abi/           ← C demos
│   ├── python_binding/  ← Python demos
│   └── *.rs             ← Rust examples
```

## Convenience targets

```sh
make build         # cargo build --lib
make test          # cargo test --all-targets
make check         # cargo check --all-targets
make c-demo        # build + run examples/c_abi/demo.c
make c-full-demo   # build + run examples/c_abi/full_demo.c
make python-demo   # build + run Python basic/regression + wheel install smoke
make bind-c-check  # verify include/machbus.h matches cbindgen output
make trace-replay-demo       # replay checked-in candump fixtures
make fuzz-smoke              # run arbitrary-input decoder fuzz smoke
make socketcan-examples-check # typecheck SocketCAN/vcan examples
make standard-suite-check    # run the ISO 11783/AEF/NMEA standard test suite
make whitespace-check        # run git diff --check
make book          # build the mdBook in ./book
make verify        # full local hardening gate
make clean         # cargo clean
```

## License

MIT.
