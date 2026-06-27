# 1. The ISOBUS Hello World

> **Anchor example:** `examples/session_minimal.rs` — run it any time with
> `cargo run --example session_minimal`.

Every networking tutorial has a "hello world", and on ISOBUS that hello world is
not "print some text" — it is **claim an address on the bus**. Until a node has
claimed an address it is not allowed to say anything else, so this is genuinely
the first thing every real ECU does. By the end of this chapter you will have a
program that brings a node onto a (simulated) ISOBUS network and watches it take
ownership of an address.

We will go fast here and just get it running. The next chapter,
[The Hello World, line by line](hello-world-explained.md), pulls the same program
apart and explains every piece.

## What we are building

A tiny program that:

1. creates a simulated CAN bus with two seats on it,
2. builds an machbus `Session` for our node,
3. drives the address-claim handshake, and
4. prints the address our node ended up owning.

There are two nodes on the bus because a node is never really alone on a real
machine — there is always at least a terminal or a tractor ECU present. We will
call our node `node_a`; `node_b` stands in for "some other ECU that is already
there".

## Step 1 — a node needs a NAME

Before anything else, a node needs an identity: its **NAME**. The NAME is a
64-bit value that says what the node is and which specific unit it is. We will
cover every field in the [next chapter](hello-world-explained.md) and in depth
in the [Address claim tutorial](../tutorials/address-claim.md); for now, a tiny
helper that stamps out a NAME from an identity number is enough:

```rust
{{#include ../../../examples/session_minimal.rs:name}}
```

The important part is `with_self_configurable(true)`: it tells the bus "if I
lose a fight over an address, I am allowed to move to another one." That keeps
our hello world from getting stuck.

## Step 2 — a bus to plug into

On real hardware the bus is a pair of wires and a CAN controller. In software,
machbus talks to a bus through a *transport*, and the companion `wirebit` crate
can build a simulated bus with named seats. We make a bus called `bus0` with two
members and take an endpoint for each, then wrap each endpoint in an
`EndpointTransport`. Think of `take_endpoint("a")` as "plug a cable from seat
`a` into our node".

## Step 3 — build the sessions

Now we build a `Session` for each node. The session is the machbus facade that
hides all the protocol bookkeeping. `Session::builder(name, addr)` takes a NAME
and a preferred address; `.plug(...)` adds optional capabilities (here a
`Diagnostics` plugin); and `.spawn(transport)` returns a `(Controls, Driver)`
pair. We then call `start()` on the controls to begin the claim:

```rust
{{#include ../../../examples/session_minimal.rs:build}}
```

`node_a` would like address `0x80`; `node_b` would like `0x81`. They do not
conflict, so both should get their wish — but the bus does not know that yet.
Right after `spawn()`, neither node has actually claimed anything. The `Controls`
half (`ctrl_a`) is how you command and inspect the node; the `Driver` half
(`drv_a`) is how you advance it and read events.

## Step 4 — drive the claim

Claiming is not instant. A node announces its NAME at its preferred address and
then waits a short window to see if anyone objects. We make that happen by
*polling* each driver (advancing its sense of time and draining the events it
produces) and *pumping* the bus (delivering frames between the two endpoints) in
a loop:

```rust
{{#include ../../../examples/session_minimal.rs:claim}}
```

`drv.poll_at(now)` hands you one event at a time until there is nothing left for
that instant; `built.pump_all()` carries the frames between endpoints. In a real
program you would call `drv.poll()` instead, which uses the host clock, and you
would not need to drive a simulated bus. That loop is the heartbeat of every
machbus program: **poll the drivers, pump the bus, repeat.**

## Step 5 — run it

Run the finished example:

```sh
cargo run --example session_minimal
```

You should see output that includes something like:

```text
[claim] node A → 0x80, node B → 0x81
[events] node A saw 1 address-claim event(s)
```

That is the whole game. After polling and pumping, `node_a` owns `0x80` and
`node_b` owns `0x81`. You can confirm with `ctrl_a.is_claimed()` and
`ctrl_a.address()`. Our node is now a full citizen of the bus and may start
sending real traffic — which is exactly what the
[next chapters](sending-receiving.md) do.

## What just happened

```
node_a: "NAME 0x.... wants 0x80"  ─┐
                                    ├─► bus carries both claims
node_b: "NAME 0x.... wants 0x81"  ─┘
        no conflict → both keep their preferred address → Claimed
```

Because the two preferred addresses were different, there was nothing to fight
over. If both had asked for `0x80`, the node with the lower NAME would have kept
it and the other — being self-configurable — would have slid to the next free
address. You can see that contest play out in the
[Address claim tutorial](../tutorials/address-claim.md).

## Things that trip people up

- **Forgetting to pump.** On a simulated bus, if you poll the drivers but never
  call `pump_all()`, the claim announcements never reach the other node and
  nobody ever becomes claimed. Poll *and* pump.
- **Forgetting to `start()`.** A session does not begin claiming until you call
  `ctrl.start()`. Build, start, then poll.
- **Sending too early.** A node must be claimed before it sends application
  traffic. We will rely on that in the next chapter.
- **Reusing a NAME.** Two nodes with identical NAMEs cannot be told apart. Give
  each node a distinct identity number.

## Validate locally

```sh
cargo run --example session_minimal
make test
```

## What this proves / does not prove

Proves: machbus can bring a node onto a simulated bus and complete address claim,
and you can drive that from a few lines of Rust.

Does not prove: anything about real-hardware timing or interoperability with a
specific ECU. The simulated bus is for learning and testing.

## Next

→ [2. The Hello World, line by line](hello-world-explained.md) — now that it
runs, understand every line.
