# `no_std` on microcontrollers

`machbus` can be built for firmware-style applications where there is no
operating-system standard library. In that mode the crate is a caller-driven
protocol engine: your board code owns the clock, CAN peripheral, storage, task
scheduling, allocator, and panic behavior.

Use this page when you are targeting a microcontroller, an RTOS task, or a
small embedded Linux component that wants the same explicit ownership model.

## Why this exists

Desktop examples can lean on files, host clocks, virtual buses, SocketCAN, C
bindings, Python bindings, and rich geo libraries. A microcontroller usually
cannot. It has a CAN peripheral, a timer, maybe flash or an SD card, and a main
loop or RTOS executor.

The embedded feature split keeps those worlds separate:

- hosted/default mode keeps the convenient OS integrations;
- `embedded` compiles the protocol/session surface as `no_std + alloc`;
- `embedded` adds fixed-capacity helpers for bounded buffers and selected
  transport paths.

The important boundary is simple: `machbus` owns protocol state, while the
application owns hardware and resources.

## Mental model

```text
        board / RTOS / firmware application
┌────────────────────────────────────────────────────┐
│ monotonic timer                                    │
│ CAN driver / interrupt queues                      │
│ flash, SD, EEPROM, LittleFS, or no persistent store │
│ allocator + panic handler                          │
│ main loop, RTOS task, or executor                  │
└───────────────┬──────────────────────┬─────────────┘
                │ Instant              │ Frame
                ▼                      ▼
        ┌────────────────────────────────────┐
        │ machbus no_std + alloc core        │
        │                                    │
        │ Session / Driver::poll_at          │
        │ J1939 / ISOBUS codecs              │
        │ address claim, TP, ETP, Fast Packet │
        │ VT / TC / FS pump state            │
        └────────────────────────────────────┘
                │
                ▼
        events and outbound CAN frames
```

There is no hidden host clock and no hidden CAN backend in the embedded path.
Each poll step receives explicit time and drains or emits explicit frames.

## Dependency setup

Disable default features and enable the embedded profile:

```toml
[dependencies]
machbus = { path = "../machbus", default-features = false, features = ["embedded"] }
```

For fixed-capacity helper APIs:

```toml
[dependencies]
machbus = { path = "../machbus", default-features = false, features = ["embedded"] }
```

The embedded profile intentionally does not compile:

| Hosted surface | Why it is not in `embedded` |
| --- | --- |
| `ffi` / C ABI | Uses hosted ABI and `std` facilities. |
| Python bindings | Uses `pyo3` and requires `std`. |
| SocketCAN | Linux-specific host interface. |
| `wirebit` | Host virtual-bus and simulation adapter. |
| `concord` | Rich hosted geo conversion stack. |
| file load/save helpers | Firmware decides how flash, SD, EEPROM, or LittleFS are used. |
| host-clock `Driver::poll()` | Firmware must pass board time explicitly through `poll_at`. |

## What `no_std + alloc` means

`embedded` is not a zero-heap profile. It means:

- the crate itself does not require Rust `std`;
- heap-backed structures can still be used through `alloc`;
- the final firmware binary must provide any global allocator it needs;
- the final firmware binary must provide its panic strategy or panic handler;
- `machbus` is checked as a library, not as a complete firmware image.

That shape is intentional for the first embedded target. It gets the host APIs
out of the protocol core without forcing every high-level ISOBUS service into
fixed storage immediately.

If your MCU project forbids heap allocation entirely, start from
`embedded` and use its fixed-capacity helpers at the CAN/session boundary,
but treat the full no-alloc migration as still in progress.

## The embedded loop

The loop shape is:

1. get board monotonic time;
2. drain received CAN frames from your driver or interrupt queue;
3. feed frames into `Session`;
4. tick protocol timers;
5. transmit every queued frame through your CAN driver;
6. handle session events.

Conceptually:

```rust
let mut now = board_monotonic_instant();

while let Some((port, frame)) = can_recv() {
    session.feed(port, &frame, now);
}

session.tick(now);

while let Some((port, frame)) = session.poll_transmit() {
    can_send(port, &frame)?;
}

while let Some(event) = session.poll_event() {
    handle_event(event);
}
```

The compiled example is `examples/embedded_session_loop.rs`. It runs as a host
example for convenience, but it compiles `machbus` with `--no-default-features
--features embedded` and uses the same board-owned clock/CAN/storage shape an
MCU application would use.

## CAN adapter boundary

The embedded CAN boundary is `machbus::net::CanTransport`, re-exported by the
session module as `machbus::session::Transport`.

Your adapter receives concrete frames from the MCU HAL and converts them into
`machbus::net::Frame`. Outbound frames go the other way.

```text
HAL frame ──► board adapter ──► machbus Frame ──► Session
HAL frame ◄── board adapter ◄── machbus Frame ◄── Session
```

Keep the adapter thin. It should know about CAN IDs, DLC, data bytes, and the
local CAN port number. It should not contain ISOBUS state machines. The
protocol state belongs in `Session`, `IsoNet`, or the lower-level protocol
helpers.

See `examples/embedded_hal_adapter.rs` for the compiled adapter shape without
adding a dependency on a specific HAL crate.

## Time and scheduling

Use `machbus::time::Instant` as the protocol timestamp. Convert your board
timer ticks into that type at the application boundary.

Use `Driver::poll_at(now)` rather than hosted `Driver::poll()`. The `_at`
method is deterministic because it only sees the time value you pass in. That
makes it suitable for:

- bare-metal superloops;
- RTOS periodic tasks;
- cooperative executors;
- deterministic simulation tests.

## Storage ownership

Embedded storage is buffer-oriented:

| Data | Embedded shape |
| --- | --- |
| NIU config | parse/format text buffers; application persists them. |
| IOP/object-pool bytes | parse bytes already supplied by the application. |
| VT stored pools | encode/decode storage blobs; application writes blobs to flash/SD/etc. |
| File Server data | in-memory protocol model; real media integration is application-owned. |
| candump traces | file save/load is hosted; line parse/format is the reusable part. |

This avoids baking one flash layout or filesystem into the protocol crate.

## Protocol surface available today

The embedded feature currently covers the useful protocol core:

- CAN frame, identifier, priority, PGN, and address-claim primitives;
- J1939 request, acknowledgment, diagnostics, heartbeat, engine, powertrain,
  maintain-power, and related codecs;
- TP, ETP, and Fast Packet helpers;
- NMEA/GNSS encode/decode paths;
- NIU filtering/routing and safety/physical helper data;
- selected ISO 11783 application codecs;
- Sequence Control core state and recording helpers;
- Task Controller heap-backed pump state, DDOP/object codecs, TC-GEO, grids,
  task logging, rate limiting, outstanding requests, ISOXML parsing, and totals;
- File Client / File Server pump state, codecs, path validation, in-memory file
  storage, and volume status helpers;
- Virtual Terminal object pools, VT Client / VT Server pump state, update
  helpers, storage-agnostic stored-version blobs, and working-set state.

The VT renderer/GTUI layer remains hosted because it is a UI/filesystem
integration surface, not an MCU protocol primitive.

## Fixed-capacity helpers

`embedded` is the first checked bounded-memory layer. It adds types such
as:

- `FixedQueue<T, N>`;
- `FixedFrameQueue<N>`;
- `FixedSlots<T, N>`;
- `FixedBytes<N>`;
- `FixedMessage<N>`;
- fixed event polling with `Session::poll_fixed_event::<N>()`;
- bounded TP/ETP/Fast Packet helper paths.

Use this mode when you want fixed RX/TX queues around the session loop or when
you are hardening a specific transport path. Do not claim the whole crate is
no-alloc yet: the session core is still the `no_std + alloc` profile.

See `examples/embedded_fixed_queue.rs`.

## Validate locally

Use Makefile targets, not ad-hoc Cargo commands:

```sh
make no-std-check
make no-std-target-check
make no-std-surface-check
make embedded-examples-check
```

`make no-std-target-check` uses the documented embedded target. If your local
Rust toolchain does not have it yet:

```sh
rustup target add thumbv7em-none-eabihf
```

For dependency audits:

```sh
cargo tree --no-default-features --features embedded -e normal
```

The embedded dependency graph should stay free of `wirebit`, `concord`, `pyo3`,
and SocketCAN dependencies.

## What this proves / does not prove

A green embedded check proves that the selected Rust surface compiles without
`std` and without the hosted adapter dependencies. It also proves the examples
and public embedded imports still type-check.

It does not prove:

- your MCU has enough RAM for your chosen feature set;
- your allocator strategy is real-time safe;
- CAN interrupt buffering is correctly sized;
- physical bus timing and wiring are correct;
- the product is ISO 11783, SAE J1939, NMEA, or AEF certified.

For deployment, combine these checks with board-level tests, trace captures,
interoperability tests, and the hardware evidence process.

## See also

- [Feature flags](../reference/feature-flags.md)
- [Validation gates](../reference/validation-gates.md)
- [The session facade](../guide/session-facade.md)
- [Onto real hardware with SocketCAN](../guide/real-hardware.md)
- [Hardware evidence](../reference/hardware-evidence.md)
