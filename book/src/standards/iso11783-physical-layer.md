# ISO 11783-2 — the physical layer

Part 2 is the copper. It fixes the bus that every higher layer assumes exists: a
single twisted pair running at a defined speed, terminated correctly, with bit timing
chosen so dozens of ECUs spread across a long machine all agree on where each bit
starts and stops.

## Why this exists

A field machine is an electrically brutal place — long harnesses, big motors,
welding repairs in a shed. The physical layer is the agreement that makes a robust,
deterministic bus out of that environment, and that lets a stranger's ECU plug in and
*just work* at the same bitrate and timing as everyone else.

## The profile

```
   CAN_H ──┬───────┬───────┬───────┬──
   CAN_L ──┴───────┴───────┴───────┴──
           │       │       │       │
         ECU 1   ECU 2   ECU 3    GNSS

   • 250 kbit/s   • terminated at both ends
   • twisted pair • defined bit timing + sample point
```

machbus does not toggle pins — a [transport/driver](../guide/session-facade.md)
owns the hardware — but it **validates the profile**: a wrong bitrate, an
out-of-range sample point, or inconsistent explicit bit-timing segments are rejected
before they can be mistaken for a compliant bus.

## The one idea that matters upward: arbitration

CAN's defining trick is **non-destructive, priority-based arbitration**. When two
nodes transmit simultaneously, the bus ANDs their bits: a dominant `0` overwrites a
recessive `1`. The node sending the lower identifier wins and continues; the loser
detects the mismatch and backs off — *no frame is destroyed*.

```
   ECU X  transmits  1 0 1 1 0 …
   ECU Y  transmits  1 0 1 0 …        ← differs at bit 4
   bus    carries    1 0 1 0 …        ← Y's dominant 0 wins
                            ▲
                     X sees bus ≠ what it sent → X yields, retries later
```

This "lowest number wins, losers retry cleanly" behavior is the hardware foundation
for two things higher up: **message priority** (low priority value = wins the bus)
and **NAME-based address claiming** (low NAME = wins the address). The same rule, all
the way up the stack.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Validating the bus profile | `net` CAN-config checks | [CAN interface problems](../troubleshooting/can-interface.md) |
| Putting frames on real copper | a `Transport` (e.g. `EndpointTransport` over SocketCAN) | [SocketCAN](../getting-started/socketcan.md) |
| Watching real frames | `candump` + replay | [Reading candump traces](../isobus-basics/reading-candump-traces.md) |

## See also

- [SAE J1939 heritage](j1939.md) — what those arbitrated bits *mean*.
- [ISO 11783-3 — data link & transport](iso11783-datalink-transport.md) — moving more than 8
  bytes over this wire.
