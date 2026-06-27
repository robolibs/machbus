# 10. Onto real hardware with SocketCAN

> **Anchor example:** `examples/socketcan_capture.rs` — a real SocketCAN program.
> With the `socketcan` feature on and a Linux CAN interface available, run it with
> `cargo run --features wirebit --example socketcan_capture`. It is a frame
> **capture** tool: it transmits a few ISOBUS-shaped frames on the interface and
> records them back into a `candump` file, proving the wire path end-to-end. It
> does **not** build a `Session` — the session-on-SocketCAN snippets below are
> *illustrative shape* showing how you would swap the transport, grounded in the
> real `spawn(...)` API.

Up to [chapter 9](tractor-and-implement.md) every node lived on a *simulated*
bus: a `wirebit` topology whose frames you moved by hand with `pump_all()`. That
is perfect for learning and testing, but it is not a wire. This chapter is where
you leave the simulator. You will run the same kind of session code from
[chapter 1](hello-world.md), unchanged in spirit, against a real Linux CAN
interface — using a **virtual** `vcan0` so you need no adapter and no machine.

The headline you should carry out of this chapter: **the session code does not
change. Only the transport changes.** You build the session exactly as you have
all along; you just hand `spawn(...)` a SocketCAN transport instead of an
endpoint transport. Everything you already know still applies.

If you only want the setup reference, the short version lives in
[Getting started → SocketCAN](../getting-started/socketcan.md). This chapter is
the build-along.

## What we are building

The anchor example puts real ISOBUS-shaped frames on a real `vcan0` interface and
records them with a `CaptureRecorder`, while `candump` lets you watch the same raw
bytes cross the wire — in the same format you read about in
[Reading candump traces](../isobus-basics/reading-candump-traces.md). Alongside it,
this chapter shows *illustratively* how a full `Session` would run over the same
SocketCAN transport: the only thing that changes versus the simulated chapters is
the argument you hand to `spawn(...)`.

## Step 1 — turn on the `socketcan` feature

SocketCAN support is behind a Cargo feature. The hosted default build includes
the virtual-bus adapter, but not the Linux-only SocketCAN backend, and the
embedded profile excludes both. Everything in this chapter needs the hosted
`socketcan` feature. The exact feature name is `socketcan`, and you pass it on
the command line:

```sh
cargo run --features wirebit --example socketcan_capture
```

If you just want to confirm the examples type-check on your machine without a live
interface, there is a make target for exactly that:

```sh
make socketcan-examples-check
```

That runs `cargo check --features wirebit --examples`. It does not open any
interface, so it works anywhere — it only proves the code compiles.

## Step 2 — bring up a virtual CAN interface

You do not need a physical adapter to exercise the wire format. Linux ships a
*virtual* CAN driver, `vcan`, that behaves like a real interface but loops frames
back in the kernel. Create one called `vcan0`:

```sh
sudo modprobe vcan
sudo ip link add dev vcan0 type vcan
sudo ip link set vcan0 up
```

These three commands load the `vcan` kernel module, create the interface, and
bring it up. They touch kernel networking state, so they need privileges — hence
`sudo`. Once `vcan0` is up it persists until you reboot or remove it
(`sudo ip link delete vcan0`).

Sanity-check that it exists and is up:

```sh
ip link show vcan0
```

You should see `vcan0` listed with state `UP` (vcan often reports `UNKNOWN`/`UP`
flags — that is normal for a virtual link).

> The same `vcan0` shows up in the replay tutorial too; see
> [SocketCAN replay](../tutorials/socketcan-replay.md) for turning captures into
> repeatable fixtures.

## Step 3 — build a session on a SocketCAN transport

Here is the only real difference from chapter 1. Instead of taking an endpoint
from a simulated topology, you open a SocketCAN transport on the named interface
and hand it to `spawn(...)`. A `SocketCanConfig` names the interface;
`create_if_missing: false` means "the interface must already exist" (you made it
in step 2). The interface name commonly comes from an environment variable like
`MACHBUS_SOCKETCAN_IFACE`, defaulting to `vcan0`.

From there, the session builder is *identical* to the simulated case — same NAME,
same preferred address, same plugins. (The anchor example is a capture tool and
does not build a session; this snippet is illustrative shape for the transport
swap.)

```rust
let (ctrl, mut driver) = Session::builder(name, addr)
    .plug(Plugin::...)
    .spawn(socketcan_transport)?;
ctrl.start()?;
```

This is the payoff of the transport being a pluggable parameter. Compare it to the
hello-world builder in
[chapter 2](hello-world-explained.md#building-the-session): the *only* thing that
moved is the argument to `spawn(...)` — an endpoint transport became a SocketCAN
transport. Everything above the transport is unchanged.

## Step 4 — claim, then drive

The driving loop is the same rhythm from chapter 1, with one important change:
there is no `pump_all()`.

On the simulated bus, *you* carried frames between endpoints by pumping. On
SocketCAN there is no software bus to pump — **the kernel moves the frames.** You
still drive the session with `driver.poll()?` to let it produce and consume frames
and advance, but delivery is the operating system's job now. Because you are on a
real host clock, use `driver.poll()` rather than the simulated `driver.poll_at(now)`;
let real wall-clock time pass between polls so the claim's contention window can
elapse and the OS has a moment to ferry frames.

So the loop slogan from chapter 1 — *poll the driver, pump the bus* — becomes
**poll the driver, and let the OS move frames.** Everything above the transport is
unchanged: `ctrl.start()`, `driver.poll()`, `ctrl.is_claimed()`, `ctrl.address()`,
and the plugin event handling all behave exactly as they did on the simulator.

## Step 5 — run it, watching the wire

Open two terminals.

**Terminal 1 — watch the raw wire:**

```sh
candump -td -L vcan0
```

`-td` prints the delta time between frames; `-L` prints in a log format that
includes the interface. Leave it running.

**Terminal 2 — run the example:**

```sh
cargo run --features wirebit --example socketcan_capture
```

`candump` (terminal 1) then shows the actual frames the capture example puts on
`vcan0` — a handful of ISOBUS-shaped frames (an Address Claimed-style frame and a
couple of broadcasts). Those are real CAN frames in the standard 29-bit
extended-ID encoding, identical in layout to what a physical ECU would put on the
wire. A session running over the same transport would emit its own Address Claimed
frame and broadcasts in exactly this format.

## What just happened

```
  example process               vcan0 (kernel)             candump
  ───────────────               ─────────────              ───────
  tx → ISOBUS frame       ──►   frame on the wire   ──►    raw bytes
  tx → ISOBUS frame       ──►   frame on the wire   ──►    raw bytes
```

No `pump_all()` appears anywhere. The kernel is the bus. Independent processes —
the example and `candump` — both attach to the same interface and see the same
traffic, exactly as separate ECUs and a logging tool would on a shared CAN
segment. A session running over a SocketCAN transport works the same way: you
`poll` the driver and the kernel moves the frames.

## Bit-rate and timing realities

On `vcan` the bitrate field is cosmetic: a virtual interface has no physical
layer, so frames appear effectively instantly and a `bitrate: 250_000` in the
config is just carried metadata. On a *real* adapter that number matters a great
deal. Every node on a CAN segment must agree on the bitrate (ISOBUS commonly runs
at 250 kbit/s), and the sample point and termination have to be right or you get
errors and bus-off conditions instead of frames. Real adapters also impose real
arbitration and queueing delays that `vcan` does not model. The concepts behind
all of this — arbitration, priority, the J1939 ID structure — are covered in
[CAN and J1939](../standards/j1939.md). Treat `vcan` as a faithful model of the
*wire format*, not of *wire timing*.

## Things that trip people up

- **Feature not enabled.** Without `--features wirebit` the SocketCAN code is
  not compiled in. If you see a usage hint instead of "opening SocketCAN
  interface", rerun the example with
  `cargo run --features wirebit --example socketcan_capture`.
- **Interface down or missing.** With `create_if_missing: false`, if `vcan0` does
  not exist or is not `UP`, the example errors at startup. Re-run the step 2
  commands and check `ip link show vcan0`.
- **Permissions.** Creating and bringing up an interface needs `sudo`. Opening an
  *existing* `vcan0` from the examples usually does not — but on a locked-down
  host, opening raw CAN sockets may still require elevated capabilities.
- **Nothing observed.** If `candump` shows no frames, the session probably ran on
  a different interface. Use the same `MACHBUS_SOCKETCAN_IFACE` everywhere.
- **PDU2 vs PDU1 on the wire.** What you see in `candump` is the assembled 29-bit
  CAN identifier, not the friendly PGN names the API uses. A broadcast PDU2 message
  carries its PGN directly in the ID; a destination-specific PDU1 message packs the
  destination address into the same bits. If a frame's ID does not match the PGN
  you expected, that mapping is usually why — see
  [Reading candump traces](../isobus-basics/reading-candump-traces.md) for decoding
  identifiers by hand.

## Validate locally

The live example needs a Linux CAN interface, so it is not part of `make verify`.
What you can always run is the type-check:

```sh
make socketcan-examples-check
```

With `vcan0` up (step 2), the full run is:

```sh
cargo run --features wirebit --example socketcan_capture
```

## What this proves / does not prove

Proves: machbus produces correct, standard-format CAN frames on a real Linux
interface, and those frames round-trip to a separate listener (the capture
recorder and `candump`) — confirming the on-the-wire encoding end to end. The same
transport swap carries a full `Session` unchanged.

Does not prove: anything about a *physical* adapter's timing, bitrate negotiation,
electrical termination, or interoperability with a specific real-world ECU. `vcan`
is a kernel loopback; it models the wire format, not the physics. Real deployment
still needs proper hardware, configuration, and interoperability evidence, and
`machbus` ships no certification.

## Next

→ [11. Async event streams](async-events.md) — instead of a hand-rolled
poll-and-react loop, bridge the session into async event streams.
