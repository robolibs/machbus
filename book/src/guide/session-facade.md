# The session facade

`machbus::session` is how you build an application. You compose a node from
**plugins** (one per subsystem), drive a pure **`Session`** core, and — for the
common case — let a **`Driver`** own the CAN interface while a cheap
**`Controls`** handle issues commands. It covers every protocol machbus speaks,
with a composable and testable shape. Hosted/default builds expose the full
plugin/controls facade. Embedded builds use the same sans-IO loop shape through
the `no_std + alloc` `Session`/`Driver::poll_at` surface, but not every hosted
convenience or plugin wrapper is part of that embedded profile.

## Why this shape

The session facade is built around three ideas:

- **Sans-IO core.** `Session` is a pure state machine: you feed it frames and a
  timestamp and drain its outputs. No socket, no system clock. That makes it
  deterministically testable and is the basis of the current `embedded`
  `no_std + alloc` build.
- **Plugin composition.** Each subsystem is a `Plugin` you `.plug(...)`. The set
  is explicit, and a plugin instance *is* the fine-control object for that
  subsystem.
- **Handle / driver split.** `spawn(transport)` returns `(Controls, Driver)`: the
  driver runs the loop you own; the `Controls` is a cheap handle for commands and
  status.

## Mental model

```
   ┌── Driver + Controls ── owns the CAN transport + clock, runs the loop ┐
   │   let (ctrl, mut driver) = Session::builder(name, addr)              │
   │       .plug(...).spawn(transport)?;                                  │
   │   ctrl.start()?;  loop { driver.poll()? ... }                        │
   ├── Session ── pure sans-IO state machine (no IO, no clock) ───────────┤
   │   s.feed(port, &frame, now);  s.poll_transmit();  s.poll_event();    │
   ├── Plugins ── one per subsystem; reuse the pure codecs ───────────────┤
   │   Diagnostics · Gnss · VtClient · TcClient · FsClient · Implement · …│
   └── Codecs ── encode/decode primitives (net / j1939 / isobus) ─────────┘
```

A received frame flows **up** (transport → `feed` → routed to interested plugins →
events). Commands and cadenced broadcasts flow **down** (plugin → outbound buffer
→ `poll_transmit` → transport).

## The pieces

| Type | Role |
| --- | --- |
| `Session` | The sans-IO core. `feed` / `tick` / `poll_transmit` / `poll_event` / `drain::<E>`. |
| `SessionBuilder` | `Session::builder(name, address)`, `.plug(p)`, `.plug_group(g)`, `.build()` or `.spawn(transport)`. |
| `Plugin` | A composable subsystem (see `session::plugins`). One instance per type. |
| `PluginCtx` | A plugin's keyhole during a callback: `send`, `emit`, `now`, `address`, `set_name`. |
| `Transport` | The CAN boundary (`recv`/`send`). `EndpointTransport` adapts a `wirebit::CanEndpoint`. |
| `Driver<T>` | Owns the transport + clock; `poll` / `poll_at` / `pump` run the loop. |
| `Controls` | Cheap cloneable handle: `start`, `address`, `is_claimed`, `with`/`with_mut`, `send_raw`, `drain`. |
| `Subscription` | RAII handle from `Driver::on(...)`; drop it to unsubscribe. |

## Building and driving

Build a node, plug the subsystems you want, and split into controls + driver:

```rust
{{#include ../../../examples/session_minimal.rs:build}}
```

Then drive the loop. `Driver::poll_at(now)` does one cycle — read the transport,
feed frames, advance timers, flush outbound — and returns the next event:

```rust
{{#include ../../../examples/session_minimal.rs:claim}}
```

In a hosted real-time program you would call `driver.poll()` (which reads the
host monotonic clock) in a loop instead of advancing `now` by hand. In
microcontroller or deterministic embedded code, keep using `driver.poll_at(now)`
and pass time from the board timer.

## Available plugins

Everything the old facade covered, as plugins in `machbus::session::plugins`:

| Area | Plugin |
| --- | --- |
| Diagnostics (DM1) | `Diagnostics` |
| GNSS / NMEA 2000 | `Gnss` |
| Virtual Terminal | `VtClient`, `VtServer` |
| Task Controller | `TcClient`, `TcServer` |
| File Server | `FsClient`, `FsServer` |
| Implement messages | `Implement` (hitch / PTO / aux / speed / lighting) |
| Sequence Control | `ScMaster`, `ScClient` |
| TIM | `Tim` |
| Powertrain (J1939) | `Powertrain` |
| Heartbeat | `Heartbeat` |
| Maintain Power | `MaintainPower` |
| Shortcut Button | `ShortcutButton` |
| Language Command | `LanguageCommand` |
| Auxiliary (AUX-O/N) | `Auxiliary` |
| DM14/15/16 + IDs | `DmMemory` |
| CF Functionalities | `ControlFunctionalities` |
| Group Function | `GroupFunction` |
| Request2 | `Request2` |
| NAME Management | `NameManagement` |

Plug one per node:

```rust
// illustrative — the API mirrors the tested types in `src/session`
let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(VtClient::new(vt_config, pool, working_set))
    .plug(TcClient::new(tc_config, ddop))
    .plug(Diagnostics::every(1000))
    .spawn(transport)?;
```

Plugging two instances of the same plugin type is a build error (one per type).

## Fine control

There are two first-class ways to drop below the facade — both work for every
subsystem:

**Own the subsystem component.** Reach a plugged subsystem by type and call its
own methods:

```rust
{{#include ../../../examples/session_minimal.rs:finecontrol}}
```

`ctrl.with::<P>(...)` / `ctrl.with_mut::<P>(...)` (or `session.get::<P>()` /
`get_mut::<P>()`) return `None` if that plugin was not plugged.

**Drive the pure core directly.** Skip the driver entirely and run `Session`
yourself — feed frames with an injected timestamp, drain outbound and events.
This is what the tests and embedded loops use:

```rust
// illustrative shape
let mut s = Session::builder(name, 0x80).plug(Diagnostics::every(1000)).build()?;
s.start()?;
s.feed(0, &frame, now);                       // a received frame + the time
s.tick(now);                                  // advance timers
while let Some((port, frame)) = s.poll_transmit() { bus.send(port, &frame); }
while let Some(event) = s.poll_event() { /* handle */ }
```

For arbitrary PGNs there is a raw escape hatch: `ctrl.send_raw(pgn, &data, dst,
priority)` / `session.send_raw(...)`.

## Events, three ways

The core produces one unified `Event` enum. Consume it however suits you:

1. **Unified enum + poll** — `driver.poll()? ` / `session.poll_event()`. One match
   site for everything.
2. **Typed per-subsystem stream** — `session.drain::<VtEvent>()` /
   `controls.drain::<VtEvent>()` returns just that subsystem's events and leaves
   the rest queued. No matching a 20-variant enum for one concern.
3. **Callbacks (RAII)** — register typed callbacks and pump:

   ```rust
   // illustrative shape
   let sub = driver.on::<VtEvent>(|e| handle_vt(e));   // returns a Subscription
   loop { driver.pump()?; }                            // dispatches to callbacks
   drop(sub);                                           // unsubscribes
   ```

## Presets (personas)

`session::presets` bundles curated plugin groups — one call wires up a role's
usual subsystems — for `plug_group`:

```rust
// illustrative shape
use machbus::session::presets;
let (ctrl, mut driver) = Session::builder(name, 0xF0)
    .plug_group(presets::tractor())     // diagnostics + implement + powertrain
    .plug(Heartbeat::every(100))        // add or drop pieces freely
    .spawn(transport)?;
```

Available: `presets::tractor()`, `presets::implement(pool, ws, ddop)`,
`presets::diagnostic_node()`.

## Hosted versus embedded session use

The same conceptual loop exists in both modes, but the available conveniences
are different:

| Build mode | Session shape | What owns time/CAN/storage |
| --- | --- | --- |
| Hosted/default | `Session::builder(...).plug(...).spawn(transport)?`, `Driver::poll()`, `Controls`, callbacks, presets, host adapters | `Driver` can read the host clock; adapters can use host transports and files. |
| Embedded | `Session::builder(...).build()?`, `Driver::new(session, transport)`, `Driver::poll_at(now)`, `feed`/`tick`/`poll_transmit`/`poll_event` | Your firmware owns the monotonic timer, CAN HAL, allocator, panic behavior, and persistence. |
| Embedded fixed helpers | Same embedded session plus fixed-capacity boundary helpers such as `FixedFrameQueue`, `FixedMessage`, and `poll_fixed_event::<N>()` | Your firmware chooses queue sizes and handles overflow explicitly. |

For the MCU-focused version of this loop, see
[`no_std` on microcontrollers](../getting-started/no-std-microcontrollers.md).

## Validate locally

```sh
make run EXAMPLE=session_minimal
make standard-suite-check
make verify
```

If you change feature gates or embedded session behavior, also run:

```sh
make no-std-check
make no-std-target-check
make no-std-surface-check
make embedded-examples-check
```

## What this proves / does not prove

Running `session_minimal` proves the facade builds, claims an address, drives a
plugin, and routes events across a virtual bus. It does **not** prove vendor
interoperability or physical-bus timing — see
[Hardware evidence](../reference/hardware-evidence.md) and the
[Claim boundary](../conformity/claim-boundary.md).

## See also

- [First node](../getting-started/first-node.md) — the shortest path to a running node.
- [`no_std` on microcontrollers](../getting-started/no-std-microcontrollers.md) — the embedded loop and HAL boundary.
- [Crate map](../reference/crate-map.md) — where `session` sits in the crate.
- [Receiving and routing](../standards/iso11783-network-layer.md) — the
  feed/route/event model in plain words.
