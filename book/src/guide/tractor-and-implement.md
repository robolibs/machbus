# 9. Tractor and implement personas

> **Anchor example:** `examples/session_minimal.rs` — run it any time with
> `cargo run --example session_minimal`. It shows the session build-and-claim
> loop the personas below ride on top of.
>
> The curated roles live in [`presets`](session-facade.md#presets-personas):
> `presets::tractor()` and `presets::implement(pool, ws, ddop)` are plugin groups
> you plug into a session in one call.

Every chapter up to now built a single node and taught it one trick: claim, send,
request, move big data, talk diagnostics, drive a VT, drive a TC. A real machine
is not one node doing one trick. It is at least **two cooperating ECUs on the same
bus** — a tractor that knows how fast the ground is moving and where its hitch
sits, and an implement that needs those numbers to do its job and occasionally
wants the tractor to lift the hitch or spin the PTO.

This chapter stands both of those up. We use two pre-wired **presets** — plugin
groups the session assembles for you — so you can think in machine terms ("this is
the tractor", "this is the implement") instead of wiring every subsystem by hand.
The deep behaviour lives in the [Tractor ECU](../tutorials/tractor-ecu.md) and
[Implement ECU](../tutorials/implement-ecu.md) tutorials; here we build and run.

## What a preset is

A preset is a bundle of plugins with the right subsystems already switched on for
a job. `presets::tractor()` returns the plugin group a tractor ECU needs — the
tractor message groups and its classification. `presets::implement(pool, ws, ddop)`
returns the implement-side group. You feed either to the session builder with
`.plug_group(...)`:

```rust
use machbus::session::{Session, presets};

let (ctrl, mut driver) = Session::builder(name, addr)
    .plug_group(presets::tractor())
    .spawn(transport)?;
ctrl.start()?;
```

Underneath, both presets build the same `Session` you have used since chapter 2 —
the poll-and-pump loop, the event stream, and address claim all work exactly as
before. The preset just turns on a coherent set of subsystems in one call.

There are two directions of traffic, and keeping them straight is the whole point
of this chapter:

```
   TRACTOR preset                           IMPLEMENT preset
  ┌────────────────┐                       ┌────────────────┐
  │ PUBLISH state  │ ── speed/distance ──► │ read latest    │
  │ speed, hitch,  │ ── hitch / PTO ─────► │ values, react  │
  │ PTO, GNSS      │                       │                │
  │                │ ◄── hitch command ─── │ COMMAND tractor│
  │ accept command?│ ◄── PTO command ───── │ (if allowed)   │
  └────────────────┘ ◄── aux-valve cmd ─── └────────────────┘
```

**Publishing** is a one-way broadcast: the tractor states a fact, anyone who cares
subscribes. **Commanding** is a request the other side may refuse. The tractor
publishes; the implement commands. (A capable tractor may also accept commands —
that is the right-to-left arrow.)

## Step 1 — build a tractor preset

A tractor describes itself with a classification: its base class, whether it
offers navigation messages, whether it is front-mounted, and so on. The
`presets::tractor()` group carries sensible tractor defaults and the tractor
message groups; you plug it into the session alongside the usual NAME, preferred
address, and transport:

```rust
let (tractor, mut tractor_drv) = Session::builder(tractor_name, 0xF0)
    .plug_group(presets::tractor())
    .plug(Gnss::listen())   // the tractor preset has no GNSS — add it to publish position
    .spawn(tractor_transport)?;
tractor.start()?;
```

The builder reads like a description of the machine: *this* identity, prefers
address `0xF0` (the conventional TECU address), with the tractor role plugged in.
The `presets::tractor()` group bundles diagnostics, implement messages, and
powertrain — it does **not** include GNSS, so we add `Gnss::listen()` here because
Step 3 publishes a position. `spawn()` returns a `Result` so a contradictory
configuration fails loudly.

## Step 2 — claim

Then drive the same claim handshake from chapter 1 — poll the driver, pump the
bus, repeat until claimed. This is exactly the `claim` anchor in the session
example:

```rust
{{#include ../../../examples/session_minimal.rs:claim}}
```

`ctrl.is_claimed()` and `ctrl.address()` tell you where the node landed.
Everything you learned about the loop still applies; the preset just rides on top
of it.

## Step 3 — publish state

A claimed tractor broadcasts what it knows. Use fine control on the relevant
plugin to publish a GNSS position (latitude, longitude, ground speed) or raise a
diagnostic, then keep polling and pumping so the frames actually move:

```rust
tractor.with_mut::<Gnss, _>(|gnss| {
    gnss.broadcast_position(&pos);
});
```

This is publishing in its purest form — the tractor states a fact and walks away.
It does not wait for an acknowledgement and does not care who reads it; the
poll-and-pump loop puts the position on the wire.

## Step 4 — the events queue

Whatever a node receives surfaces on the **one** unified event stream you drain
with `driver.poll_at(now)?` (or `driver.poll()?` on a real clock). Personas do not
get their own parallel queue — every subsystem feeds the same `Event` enum you
have been draining since chapter 2:

```rust
while let Some(event) = tractor_drv.poll_at(now)? {
    match event {
        Event::Gnss(g) => { /* position fix */ }
        Event::Diag(d) => { /* fault code */ }
        _ => {}
    }
}
```

## Step 5 — two nodes on one bus

The real-machine shape is a tractor and an implement on the same `bus0`, the
tractor built with `presets::tractor()` and the implement with
`presets::implement(pool, ws, ddop)`:

```rust
let (implement, mut impl_drv) = Session::builder(impl_name, 0x80)
    .plug_group(presets::implement(pool, ws, ddop))
    .spawn(impl_transport)?;
implement.start()?;
```

Both must claim before either can say anything else, so the claim loop polls
**both** drivers and pumps the shared bus between them — the same shape as the
single-node `claim` anchor above, with a second `poll_at` for the implement.

This is the gotcha that bites everyone: if only one session claims, the other
stays at the `0xFE` null address and silently drops any application traffic you try
to send through it. Poll both, pump once, every iteration.

## Step 6 — the tractor commands the implement

With both nodes claimed, the tractor issues commands through its implement-message
plugin: a rear-hitch raise, a rear-PTO speed, an auxiliary-valve extend. Use
`with_mut` to reach the plugin and queue each command; the following pump loop
delivers them. Note the raw values — `4320` raw for ~540 rpm, `0x4000` for 50%
flow. The session passes the encoded field through; it does not invent units for
you.

## Step 7 — the implement reads and reacts

On the other side, the implement drains its stream and matches on
`Event::Imp(ImplementEvent::…)`. Then it reads the **cached latest value**
for each through fine control on its implement plugin.

There are two ways to consume implement traffic, and you will use both:

| Style | Source | When |
| --- | --- | --- |
| Event-driven | `ImplementEvent::HitchCommand`, `PtoCommand`, `AuxValveCommand` | react the moment a command arrives |
| Latest-value | the implement plugin's cached getters (last rear hitch, last rear PTO) | read the current state on your own schedule |

The event tells you *something changed*; the cached getter tells you *what the
value is right now*. Control loops usually poll the cached value each tick and only
watch events for edges (a command just landed, a value went stale).

## ImplementEvent at a glance

`ImplementEvent` is the implement subsystem's slice of the unified `Event` enum.
The variants this chapter exercises are `HitchCommand { hitch, msg }` (someone
asked to move a hitch), `PtoCommand { pto, msg }` (drive a PTO at a target speed
and ramp), and `AuxValveCommand(m)` (drive an aux valve at a flow rate). The
`Hitch` and `Pto` enums (`Hitch::Rear`, `Pto::Rear`) pick *which* actuator; the
message body carries the *command*. Match the variant you care about and ignore
the rest, exactly as with any other `Event`. Both `Hitch` and `Pto` are
re-exported from `machbus::session`.

## Gotchas

- **Both sessions must claim first.** On a two-node bus, poll and pump *both* every
  iteration until each reports claimed. A node still at `0xFE` cannot send
  anything.
- **Check facility availability.** A preset only offers what it turned on. Gate on
  the relevant cached getters returning `Some(...)` before you trust a value. A
  `None` from a last-hitch getter means "no command seen yet", not "hitch is down".
- **Plan for stale data.** Published state can stop arriving (cable knocked loose,
  sender reset). The cached getter keeps handing you the last value it saw, which
  can be dangerously old. Real control logic times out a value and falls back to a
  **safe default** — stop metering, hold position — rather than acting on a stale
  number.
- **Commands are requests.** The tractor publishes; the implement commands. A
  command is never a guarantee — the receiving side decides whether to honour it,
  and on a real machine its interlocks have the final say.

## Validate locally

```sh
cargo run --example session_minimal
make test
```

## What this proves / does not prove

Proves: machbus can stand up tractor and implement presets on one simulated bus,
have the tractor publish state and command the implement, and have the implement
consume both event-driven and via cached latest values — all on the same
poll-and-pump loop and unified event stream from chapter 2.

Does not prove: anything about real-hardware timing, signal cadence, or
interoperability with a specific commercial tractor or implement. The simulated
bus is for learning and testing; a real pairing still needs official standards,
hardware, and interoperability evidence.

## Next

→ [10. Onto real hardware with SocketCAN](real-hardware.md) — take the same
session code off the simulated bus and onto a real CAN interface.

## See also

- [Tractor ECU](../tutorials/tractor-ecu.md) — the publish-vs-command split,
  classes and facilities, message groups in depth.
- [Implement ECU](../tutorials/implement-ecu.md) — the consumer side: reading
  state, safe-state thinking, requesting actions.
- [TIM and automation](../tutorials/tim.md) — when an implement is allowed to
  drive the tractor automatically, and the safety machinery around it.
