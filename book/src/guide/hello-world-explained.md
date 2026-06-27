# 2. The Hello World, line by line

> **Anchor example:** `examples/session_minimal.rs`.

In [chapter 1](hello-world.md) we got a node onto the bus. It worked, but a few
things were waved past. This chapter slows down and explains every part of that
program, because almost everything you do later is a variation on it. Nothing
new is built here — this is the "read the manual for the thing you just used"
chapter.

## The imports

```rust
use machbus::Instant;
use machbus::j1939::diagnostic::{Dtc, Fmi};
use machbus::net::{Name, Result};
use machbus::session::plugins::Diagnostics;
use machbus::session::{ClaimEvent, DiagEvent, Event};
use machbus::session::{EndpointTransport, Session};
use wirebit::topology::Topology;
```

Two layers show up here, and it is worth knowing which is which:

- `machbus::net` is the **low-level protocol layer** — raw frames, NAMEs,
  addresses, priorities. You reach into it when you want fine control.
- `machbus::session` is the **surface layer** — the `Session` facade, its
  `Controls` / `Driver` pair, plugins, and the unified `Event` type. This is what
  most application code uses.
- `wirebit` is the **bus transport** crate. `Topology` builds simulated buses and
  hands out endpoints. On real hardware you would build a transport over a
  SocketCAN link instead (see [chapter 10](real-hardware.md)).

A session is generic over its transport. That is how the exact same session code
runs on a simulated bus today and a real one later: only the transport changes.

## The NAME helper

```rust
{{#include ../../../examples/session_minimal.rs:name}}
```

A `Name` is built with consuming `with_*` setters. This helper sets only three
things and leaves the rest at default, which is fine for a demo. In a real
product every field carries meaning:

| Field | What it says | In the helper |
| --- | --- | --- |
| Identity number | Which specific unit this is | the `identity` argument |
| Function code | What the node does | `0x80` |
| Self-configurable | May I move addresses if I lose? | `true` |
| Manufacturer, device class, instances, … | The rest of the identity | left at default |

The two nodes differ only by identity number (`0x100` vs `0x999`), which is
enough to make their NAMEs distinct. Distinct NAMEs are mandatory — two nodes
that present the same NAME break the bus's ability to tell them apart. The full
field-by-field meaning is in
[NAME and address claim](../standards/iso11783-network-management.md).

## Building the simulated bus

We describe a network with `wirebit`, build it, and take an endpoint per seat:

```rust
let mut topo = Topology::new();
let n1 = topo.add_node("a");
let n2 = topo.add_node("b");
topo.can_bus("bus0").members(&[n1, n2]);
let mut built = topo.build().unwrap();
let bus = built.can_bus_mut("bus0").unwrap();
let ep_a = bus.take_endpoint("a").unwrap();
let ep_b = bus.take_endpoint("b").unwrap();
```

Line by line:

- `Topology::new()` starts an empty description of a network.
- `add_node("a")` / `add_node("b")` declare two devices.
- `can_bus("bus0").members(&[n1, n2])` wires both onto one CAN segment.
- `build()` turns the description into a live, runnable bus.
- `take_endpoint("a")` removes seat `a`'s endpoint and hands it to us, so we can
  wrap it in a transport. Each endpoint can only be taken once.

The object `built` still owns the bus itself. We keep it around because we have
to *pump* it later to actually move frames between the seats.

## Building the session

```rust
{{#include ../../../examples/session_minimal.rs:build}}
```

The builder reads like a sentence: this session has *this* NAME, prefers *this*
address, has *this* plugin, and is spawned on *this* transport. The
`EndpointTransport::new(0, ep_a)` channel index (`0`) matters once a node sits on
more than one bus (a router/NIU node does); for a single-bus node it is always
`0`.

`spawn()` returns a `Result`, so a misconfigured session fails loudly rather
than limping along. It also splits the session into two halves:

- `ctrl_a` — the `Controls`: command and inspect the node (`start()`,
  `is_claimed()`, `address()`, `with_mut::<Plugin, _>(...)`).
- `drv_a` — the `Driver`: advance the node and read events (`poll()` /
  `poll_at()`).

After `spawn()` the session exists but has **not** claimed anything, and it will
not begin until you call `ctrl.start()`. If you query it before the loop runs,
`ctrl_a.is_claimed()` is `false` and `ctrl_a.address()` is the null/no-address
placeholder.

## The poll-and-pump loop

This is the single most important pattern in machbus, so let us read it
carefully:

```rust
{{#include ../../../examples/session_minimal.rs:claim}}
```

- `start()` (called just above, in the build snippet) tells each session to begin
  announcing its claim. This only queues the work; nothing is on the wire yet.
- Inside the loop, `now.add_millis(50)` advances the simulated clock by 50
  milliseconds, and `drv.poll_at(now)` lets the driver do any work that became
  due — including sending its claim and, later, deciding the contention window
  has closed — handing you each resulting event in turn.
- `built.pump_all()` is the bus's job: it carries frames that one endpoint sent
  over to the other endpoint's inbox.
- The loop exits early once both `ctrl_a.is_claimed()` and `ctrl_b.is_claimed()`
  report success.

The mental model: **sessions produce and consume frames when you poll their
drivers; the bus moves frames when you pump it.** Neither happens on its own. A
real application runs this loop forever using `drv.poll()` (host clock, no
simulated bus to pump), interleaving it with reading sensors and updating
displays.

After enough iterations the contention window has elapsed with no conflict, and
each node is claimed at its preferred address. That is the line:

```text
[claim] node A → 0x80, node B → 0x81
```

## Where claim results show up as events

The driver does not only let the node change its state — `poll_at` (and `poll`)
also hand you an **event** per piece of progress:

```rust
let mut a_claims: Vec<ClaimEvent> = Vec::new();
while let Some(event) = drv_a.poll_at(now)? {
    if let Event::AddressClaim(claim) = event {
        a_claims.push(claim);
    }
}
```

The unified `Event` enum has a variant per subsystem — `AddressClaim`, `Diag`,
`Vt`, `Tc`, and so on. A claim success arrives as
`Event::AddressClaim(ClaimEvent::Claimed { address })`.

You will spend a lot of time in later chapters matching on these events. The
rule to remember: **poll the driver regularly.** Events are produced as you
poll; if you stop polling, the node stops making progress and you stop seeing
what it did.

## Fine control through the plugin

The `Diagnostics` plugin we attached is reachable through the controls. To raise
a fault code on node A:

```rust
{{#include ../../../examples/session_minimal.rs:finecontrol}}
```

`ctrl.with_mut::<Diagnostics, _>(|diag| ...)` borrows the named plugin so you can
call its methods. This is the general escape hatch for poking at a specific
capability without leaving the session facade. Diagnostics get a full chapter
later; here it is just a taste of how fine control works.

## The shape of every machbus program

Strip away the demo specifics and every program in this track has the same
skeleton:

```
build a NAME
build a Session (name + preferred address + plugins) → (controls, driver)
controls.start()
loop:
    driver.poll() → react to each Event
    do your application work (controls.send_*, controls.with_mut, ...)
```

The chapters from here on only change what happens inside "react" and "do your
application work". The plumbing stays identical.

## Validate locally

```sh
cargo run --example session_minimal
make test
```

## What this proves / does not prove

Proves: you understand the machbus lifecycle and event model well enough to read
any later chapter.

Does not prove: anything about hardware or certification — the same caveats from
chapter 1 apply.

## Next

→ [3. Sending and receiving messages](sending-receiving.md) — now that the node
is on the bus, make it talk.
