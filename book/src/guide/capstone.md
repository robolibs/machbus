# 12. Capstone: a complete implement ECU

> **Anchor example:** `examples/session_minimal.rs` — run it any time with
> `cargo run --example session_minimal`. It is the smallest end-to-end session;
> the capstone below scales the same shape to three nodes.

This is the last chapter of the track, and it is the one where everything you
built in isolation comes together into a single program that looks like a real
machine. So far each chapter taught one thing on its own bus. Here we put a
tractor, a virtual terminal, and an implement on **one** bus, let them claim,
connect, publish, and react, and drive all three from **one** poll-and-pump loop.
No new concept is introduced — this is the assembly chapter.

## What we have learned

Every step below is a capability from an earlier chapter. The capstone uses all of
them at once:

| From chapter | Capability | Where it shows up here |
| --- | --- | --- |
| [1](hello-world.md) / [2](hello-world-explained.md) | Build a NAME and a session; claim an address | All three nodes claim before any traffic |
| [3](sending-receiving.md) | Broadcast and receive a PGN | GNSS position out, DM1 in |
| [4](requests-and-acks.md) | Ask a node for data | The presets request/respond under the hood |
| [5](transport.md) | Move payloads bigger than one frame | The VT object pool and DM1 lists ride transport |
| [6](diagnostics.md) | Publish and read fault codes | The low-fuel alarm becomes a DM1 every peer sees |
| [7](virtual-terminal.md) | Stand up a VT | A VT server emitting status |
| [8](task-controller.md) | Report working values | The implement connects its TC client, the same data a TC consumes |
| [9](tractor-and-implement.md) | Tractor and implement presets | Tractor and implement talking on one wire |
| [10](real-hardware.md) | The same code on real CAN | Swap the endpoint transport for a SocketCAN one, nothing else changes |
| [11](async-events.md) | Drive the event loop | The combined drain at the end of the run |

The session facade — the `presets`, the plugins, and the unified `Event` enum — is
what lets one short `main` hold all of this. Doing the same at the raw
`net`/`isobus` layer would mean wiring each protocol's state machine by hand on
each node.

## What we are building

```
                         bus0
   ┌──────────┐      ┌──────────┐      ┌──────────────┐
   │ Tractor  │      │   VT     │      │  Implement   │
   │  0xF0    │      │  0x26    │      │    0x80      │
   │ GNSS     │──────│ 800x480  │──────│ 6 sections   │
   │ alarm    │      │ status   │      │ reads DM1    │
   └────┬─────┘      └──────────┘      └──────┬───────┘
        │ position, DM1 (low fuel)            │
        └────────────► every peer ◄───────────┘ sections on
```

Three presets, one bus, one loop. The tractor publishes its position and raises a
low-fuel alarm; the VT server comes online and emits status; the implement turns
on three sections and watches the tractor's diagnostics roll in.

## Step 1 — the NAME helper and the topology

Same NAME helper as chapter 1, reused for all three nodes. They differ only by
identity number, which is enough to keep their NAMEs distinct — exactly the `name`
anchor from the session example:

```rust
{{#include ../../../examples/session_minimal.rs:name}}
```

The topology now has **three** seats on `bus0` instead of two. You build it and
take one endpoint per node, then wrap each in a transport for `spawn(...)`.

## Step 2 — build the three sessions from presets

This is the heart of the session facade. Instead of one generic session configured
by hand, each node plugs in the **preset** that turns the right subsystems on for
its role:

```rust
let (tractor, mut tractor_drv) = Session::builder(make_name(1), 0xF0)
    .plug_group(presets::tractor())
    .plug(Gnss::listen())   // the tractor preset does not include GNSS — add it
    .spawn(tractor_transport)?;

let (vt, mut vt_drv) = Session::builder(make_name(2), 0x26)
    .plug(VtServer::new(VTServerConfig::default())?)   // no VT preset — plug the server
    .spawn(vt_transport)?;

let (implement, mut impl_drv) = Session::builder(make_name(3), 0x80)
    .plug_group(presets::implement(pool, ws, ddop))
    .spawn(impl_transport)?;
```

Read what each builder asks for:

- the tractor prefers address `0xF0` (the tractor's conventional range) and plugs
  in the tractor preset (diagnostics, implement messages, powertrain). The preset
  does **not** include GNSS, so we add `Gnss::listen()` to publish position.
- the VT prefers `0x26` (the first VT address). There is no VT preset, so we plug a
  `VtServer` directly; `VtServer::new` validates the config and returns a `Result`.
- the implement prefers `0x80` and plugs the implement preset, which takes the
  object pool, working set, and DDOP it advertises.

Each `spawn()` returns a `Result`. Every session is the same `Session` underneath;
the preset just bundles the plugins, and fine control with
`ctrl.with_mut::<Plugin, _>(...)` reaches any one of them when you need
role-specific behaviour.

## Step 3 — claim, all three at once

Nothing may talk before it has an address, so the first loop is pure address claim.
You call `start()` on every session, then run the familiar poll-pump-settle loop —
only now it polls **three** drivers per iteration. This is the same `claim` anchor
from the session example, scaled up:

```rust
{{#include ../../../examples/session_minimal.rs:claim}}
```

After about a simulated second all three are claimed and the program prints their
addresses:

```text
[claim] tractor=0xF0, vt=0x26, implement=0x80
```

**The ordering matters:** claim comes first, application traffic second. If you
publish before a node is claimed, the send is rejected — there is no address to
send from.

## Step 4 — bring the subsystems online

Now that everyone owns an address, each session does its job through fine control
on its plugins. The VT server starts emitting status, the tractor publishes a
position fix and raises a low-fuel alarm, and the implement connects to the task
controller so it can report its working values:

```rust
// VtServer::start returns a Result — handle it.
vt.with_mut::<VtServer, _>(|srv| srv.start())
    .transpose()?;

tractor.with_mut::<Gnss, _>(|g| g.broadcast_position(&pos));

tractor.with_mut::<Diagnostics, _>(|diag| {
    // SPN 96 is the fuel-level parameter; FMI BelowNormal is the low-fuel alarm
    diag.raise(Dtc { spn: 96, fmi: Fmi::BelowNormal, occurrence_count: 1 });
});

implement.with_mut::<TcClient, _>(|tc| tc.connect())
    .transpose()?;
```

These are independent actions on independent sessions. Because they all share one
bus, what one node sends, the others can receive. The DM1 the tractor raises is
the diagnostic message every node on the bus can read; connecting the TC client is
how the implement begins reporting the working-set values a task controller logs.

`with_mut` returns `Option<R>` (`None` if the plugin is not plugged); when the
inner call returns a `Result`, `.transpose()?` turns `Option<Result<…>>` into
`Result<Option<…>>` so you can propagate any error.

## Step 5 — the single combined run loop

You drain the claim events first (you already saw the claim print), then run the
**same loop again** for two simulated seconds. This time it is not driving a
handshake — it is letting the periodic broadcasts (VT status, the tractor's DM1,
GNSS) fire repeatedly and flow to every peer:

```rust
for _ in 0..40 {
    now = now.add_millis(50);
    while tractor_drv.poll_at(now)?.is_some() {}
    while vt_drv.poll_at(now)?.is_some() {}
    while impl_drv.poll_at(now)?.is_some() {}
    built.pump_all().unwrap();
}
```

This is the whole point of the chapter. One loop body — poll every driver, pump
the bus — drives an entire machine. A real application would interleave reading
sensors and updating displays inside this loop, but the plumbing is exactly what
you wrote in chapter 1.

## Step 6 — drain the unified event stream

Each node's driver hands back events it received from its peers. The unified
`Event` enum carries a variant per subsystem, so one `match` reaches across
diagnostics, GNSS, VT, and the rest. The implement counts the DM1 broadcasts it
saw, and the tractor counts the echoes it received:

```rust
while let Some(event) = impl_drv.poll_at(now)? {
    if let Event::Diag(DiagEvent::Dm1Received { source, active, .. }) = event {
        // the implement seeing the tractor's low-fuel alarm
    }
}
```

`Event::Diag(DiagEvent::Dm1Received { source, active, .. })` is the implement
seeing the tractor's low-fuel alarm — the same SPN/FMI the tractor raised in step
4, now decoded on the other side of the bus. The tractor's drain matches
`DiagEvent::Dm1Received` and `GnssEvent::Position` from the same unified enum.

**Drain or drop:** the event stream is bounded. Poll it to empty every iteration;
if you never read it, the oldest events are dropped to make room. In a
long-running machine you drain every loop iteration, not once at the end. Typed
draining is also available — `ctrl.drain::<DiagEvent>()` pulls just the
diagnostics events.

## Putting it together — expected output

Run the minimal session example to see the build-and-claim shape end to end:

```sh
cargo run --example session_minimal
```

```text
=== Session facade — minimal demo ===

[claim] node A → 0x80, node B → 0x81
[events] node A saw 1 address-claim event(s)
[rx] node B received DM1 from 0x80 with 1 active DTC(s)

Done.
```

The capstone scales that same shape to three nodes: three nodes boot, all three
claim, the VT comes online, the tractor publishes and alarms, the implement acts,
and over two seconds the periodic diagnostics flow across the bus and land as
events. The exact DM1 count depends on broadcast cadence and the run window; the
shape is what matters.

## How the pieces compose

The three sessions never call each other directly. They compose through the bus:
one node broadcasts, the bus carries the frame, every other node decodes it into an
event. That is why the implement sees the tractor's low-fuel alarm without any
wiring between them — they only share `bus0` and the same poll loop.

Where the session facade saved you work versus the low-level layer:

- **Presets and plugins instead of hand-wired subsystems.** `presets::tractor()`
  and `presets::implement(...)` bundle a role's plugins, and a single plug such as
  `VtServer::new(...)` turns one subsystem on. At the `net`/`isobus` layer you would
  assemble each protocol's state machine yourself on each node.
- **One `Event` enum instead of N inboxes.** Diagnostics, GNSS, VT, and TC events
  all arrive on one stream you drain with one `match`. You do not poll each
  subsystem separately.
- **Fine control for the rest.** `ctrl.with_mut::<Plugin, _>(...)` reaches any
  plugin when you need a role-specific call — raise a fault, send a position, flip
  a section — without leaving the facade.

When you need to own every byte — tests, tight control loops, an unusual PGN — the
low-level layer is still there underneath. The presets are built on it.

## Gotchas

- **Claim before app traffic.** Every node must be claimed before it publishes. The
  example runs a full claim loop first for exactly this reason.
- **Drain events or they drop.** The event stream is bounded. Poll it regularly.
- **Poll every driver, every iteration.** A session that stops being polled stops
  sending its periodic broadcasts and stops decoding inbound frames. In a
  multi-node loop it is easy to forget one — poll all three here.
- **Pump after polling.** Frames a session produced sit in its outbox until you
  pump the bus. Poll *and* pump.

## Validate locally

```sh
cargo run --example session_minimal
make test
```

## What this proves / does not prove

Proves: machbus can host a realistic multi-node machine — tractor, VT, and
implement presets — on one bus, drive address claim, subsystem startup,
publishing, and cross-node event reaction from a single loop, entirely in
software. It is a working software integration of everything in this track.

Does not prove: that this is a certified or hardware-proven machine. `machbus` is
not certified. The bus here is simulated; the timing, the terminals, and the ECUs
are not the real ones you would ship against. A real deployment still needs the
official standards, real hardware, and interoperability testing with the actual
devices you intend to work with.

## Where to go next

You have finished the build-along track. To go deeper, leave the lab course and
open the reference manual:

- [Tutorials](../tutorials/index.md) — one page per subsystem, in depth: the
  [Implement ECU](../tutorials/implement-ecu.md) and
  [Tractor ECU](../tutorials/tractor-ecu.md) tutorials expand the presets you just
  used, and there are dedicated pages for the
  [VT client](../tutorials/virtual-terminal-client.md),
  [TC client](../tutorials/task-controller-client.md),
  [diagnostics](../tutorials/diagnostics.md), and the rest.
- [Reference](../reference/index.md) — the crate map, protocol coverage, feature
  flags, and error handling, for when you are building a real product and need
  exact behavior.
- [Language bindings](../bindings/index.md) — driving machbus from C or Python when
  your application is not pure Rust.
- [Conformity](../conformity/index.md) — exactly what is and is not tested, and
  where the evidence boundary sits. Read this before you make any claim about a
  machine built on machbus.

That last page is the honest edge of everything you have built: the track teaches
the API and the behavior, and conformity tells you what still stands between a
passing demo and a certified machine.
