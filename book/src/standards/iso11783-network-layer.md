# ISO 11783-4 — the network layer

Most machines are one CAN segment and can ignore this part. But a big machine — a
tractor with its own bus joined to a large implement with its own bus — is two
segments bridged by a router. Part 4 defines how a message crosses that boundary
safely: which messages forward, in which direction, and how to avoid loops and
address clashes between the two sides.

## Why this exists

One CAN segment has electrical and load limits. When a machine outgrows them, you
split it and join the halves with an interconnect (a network interconnection unit,
NIU). The moment you do, two new problems appear: a broadcast could echo back and
forth forever (a loop), and two devices on opposite segments might claim the same
address. Part 4 is the rulebook that prevents both.

## The mental model

```
   Tractor segment                     Implement segment
   (TECU · VT · GNSS)                   (rate ctrl · sections · sensors)
          │                                      │
          └──────────────►  ┌───────┐  ◄─────────┘
                            │  NIU  │   router: forwards selected PGNs by
                            └───────┘   rule, drops echoes, guards addresses
```

The NIU is not a dumb repeater. It applies **forwarding rules**: a given PGN may be
allowed to cross in one direction, both, or neither, optionally filtered by source or
destination NAME. A **loop guard** ensures a forwarded frame is not forwarded back.

## What machbus models

machbus implements the interconnect in `net::niu`: forwarding policies per PGN,
direction control, NAME-based source/destination filters, and the loop guard. It is
the piece that lets a large machine scale without every ECU having to know it lives
on a multi-segment network.

For single-segment applications — the common case — none of this is in your way; you
build one node on one bus and the network layer is invisible.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Routing PGNs across segments | `net::niu` forwarding rules + loop guard | [Network routing](../tutorials/network-routing.md) |
| Single-segment apps | nothing — it just works | [The session facade](../guide/session-facade.md) |

## See also

- [ISO 11783-3 — data link & transport](iso11783-datalink-transport.md) — what is being routed.
- [ISO 11783-5 — address claiming](iso11783-network-management.md) — the address
  clashes the NIU must guard against.
