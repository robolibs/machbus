# 3. Sending and receiving messages

> **Anchor example:** `examples/session_minimal.rs` — run it any time with
> `cargo run --example session_minimal`.

In [chapter 1](hello-world.md) we got a node onto the bus, and in
[chapter 2](hello-world-explained.md) we read the program line by line. Both
nodes are now claimed, which is the moment a node earns the right to say anything
beyond "this address is mine". This chapter spends that right: we make one node
**broadcast** a message and the other node **receive** it.

We keep the same `(controls, driver)` pair from the previous chapters and the
same poll-and-pump loop. Nothing new gets built; we just add traffic.

## What we are adding

1. `node_b` **builds and broadcasts** a raw frame through its controls.
2. We **poll and pump** until the message arrives.
3. `node_a` **reads the inbound event** out of its driver.

Sender and receiver are two different nodes on the same simulated bus, exactly
like a tractor ECU talking to an implement.

## Step 1 — send a frame

A node sends an arbitrary frame through its controls handle. The session exposes
`send_raw` for hand-crafted traffic:

```rust
ctrl_b.send_raw(0xFECA, &[0xDE, 0xAD, 0xBE, 0xEF], BROADCAST_ADDRESS, Priority::Default)?;
```

Reading `send_raw` argument by argument:

| Argument | Value here | Meaning |
| --- | --- | --- |
| PGN | `0xFECA` | which message this is |
| payload | `&[0xDE, 0xAD, 0xBE, 0xEF]` | the data bytes |
| destination | `BROADCAST_ADDRESS` | who it is for — everyone |
| priority | `Priority::Default` | how urgent the frame is on the bus |

The source address is filled in for you: the session uses the address `node_b`
actually claimed. That is why we had to wait until the node was claimed before
sending — a node with no address has nothing valid to put there.

Why `0xFECA` specifically? It is a **PDU2** PGN — its PDU Format byte is `0xFE`,
which is `>= 0xF0`. PDU2 PGNs are *broadcast-only*: the message has no single
addressee, so the destination field is not part of the PGN and the address we
pass as "destination" will not overwrite it. That keeps this first example
simple — we can broadcast to everyone without worrying about who is listening.
PGNs below `0xF000` (PDU1) are addressed to one node and behave differently; the
difference is covered in
[PGNs, priority, source and destination](../standards/j1939.md).

`BROADCAST_ADDRESS` (the value `0xFF`) means "no specific recipient". Because we
chose a PDU2 PGN, this is the natural and only sensible choice: the message goes
out and any node on the bus will pick it up.

## Step 2 — poll and pump until it lands

Sending only queues the frame in `node_b`'s transport. As always in machbus,
**nothing moves until you pump the bus** (on a simulated bus), and nothing is
processed until you poll the drivers. So we run the same heartbeat from the
previous chapters, watching `node_a`'s inbound events:

```rust
for _ in 0..10 {
    now = now.add_millis(50);
    while drv_b.poll_at(now)?.is_some() {}   // let node_b flush the send
    built.pump_all().unwrap();               // bus carries the frame
    while let Some(event) = drv_a.poll_at(now)? {
        // node_a turns the inbound frame into an Event here
    }
    built.pump_all().unwrap();               // settle
}
```

This is the same loop from chapter 1, just with traffic flowing. We poll
`node_b` (flush the send), pump (carry the frame from `node_b`'s outbox to
`node_a`'s inbox), poll `node_a` (let it recognise the frame and produce an
event), and pump again to settle. A real application would call `drv.poll()` and
run this forever, with no simulated bus to pump.

## Step 3 — read the inbound event

Polling `node_a`'s driver hands you each inbound message as an `Event`. You match
on it the same way you matched on claim events:

```rust
while let Some(event) = drv_a.poll_at(now)? {
    match event {
        Event::Custom { pgn, source, data } => {
            println!("got PGN {pgn:#06X} from {source:#04X}: {data:02X?}");
        }
        other => println!("{other:?}"),
    }
}
```

A raw inbound PGN that no subsystem claimed arrives as a `Custom` event carrying:

- `pgn` — which PGN this is (`0xFECA`), so one handler can serve several message
  kinds.
- `source` — the address of the node that sent it (`node_b`'s claimed address),
  so you know who is talking.
- `data` — the payload bytes, exactly what `node_b` put in (`DE AD BE EF`).

Anything that is *not* the variant you care about falls through to the `other`
arm. That catch-all is a habit worth keeping: the event stream is shared across
every subsystem, so a handler should always have a default branch.

If you prefer to collect one subsystem's events without matching the whole
stream, `ctrl.drain::<E>()` pulls the buffered events of a single typed kind. The
poll-and-match form above is the general path; `drain` is the convenience when
you only want one event family.

## What just happened

```
node_b: ctrl_b.send_raw(0xFECA, ..)   ── build + queue a frame on node_b's transport
                                          │
pump_all()                             ── bus carries it to node_a's inbox
                                          │
drv_a.poll(..)                          ── session recognises the frame,
                                          makes an Event
                                          │
match event { Event::Custom { .. } => . } ── you read { pgn, source, data }
```

Broadcasting (a PDU2 PGN to `BROADCAST_ADDRESS`) means anyone on the bus hears
it; an *addressed* message (a PDU1 PGN to a specific node's address) is delivered
to just that one recipient. We use the broadcast form here because it is the
simplest thing that demonstrably works; addressed exchanges show up when we send
a [request and wait for an answer](requests-and-acks.md).

## Things that trip people up

- **Forgetting to pump.** On a simulated bus, the frame never reaches the other
  node until you call `pump_all()`. Poll *and* pump.
- **PDU1 vs PDU2 confusion.** With a PDU2 PGN (`>= 0xF000`) the destination you
  pass is harmless — it does not change the PGN. With a PDU1 PGN it is part of
  the addressing and absolutely does matter. Get this wrong and your message goes
  to the wrong place or nowhere. When in doubt, re-read
  [PGNs, priority, source and destination](../standards/j1939.md).
- **Not polling the driver.** Inbound frames become events only when you poll. If
  you stop polling, your "missing" message simply never surfaced. Poll on every
  loop iteration.
- **Sending before claimed.** The source address is only valid once the node has
  claimed. Sending earlier puts a bogus source on the bus.

## Validate locally

```sh
cargo run --example session_minimal
make test
```

## What this proves / does not prove

Proves: machbus can broadcast a frame from one node, move it across a simulated
bus, and surface it on another node as an event with its PGN, source, and payload
intact.

Does not prove: anything about real-hardware timing, electrical behaviour, or
interoperability with a specific ECU. The simulated bus is for learning and
testing; a real deployment still needs hardware and the official standards.

## Next

→ [4. Requests and acknowledgements](requests-and-acks.md) — broadcasting is
one-way. Next we send a request *to* a specific node and wait for its reply.

## See also

- [Request a PGN](../tutorials/request-pgn.md) — the addressed request/response
  pattern in depth.
- [Receiving and routing](../standards/iso11783-network-layer.md) — how the
  session decides which frames become events and where they go.
- [PGNs, priority, source and destination](../standards/j1939.md)
  — the anatomy of a message identifier and the PDU1/PDU2 split.
