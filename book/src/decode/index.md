# Decode without ISOBUS

machbus is best known as an ISOBUS stack. But underneath the session facade, the
implement personas, and the Virtual Terminal client, there is a smaller, sharper tool:
a CAN / J1939 / NMEA 2000 *decoder*. You can use it on its own.

This is deliberate. Most of the value of being on an agricultural bus is simply
*understanding what is on it*. A tractor, a GNSS receiver, an engine ECU, and a planter
all chatter constantly. Before you ever claim an address or send a command, you usually
want to listen — to take a raw 29-bit identifier and eight bytes of payload and turn it
into something you can reason about: "engine speed, 1450 rpm, from source 0x00."

machbus exposes exactly that capability as **three standalone public modules**. You do
not need to start a session, claim an address, or run a network manager to use them.

## The three public modules

```
   machbus
   ├── net      ── CAN frames, identifiers, PGNs, the wire layer
   ├── j1939    ── SAE J1939 PGN codecs (engine, diagnostics, …)
   └── nmea     ── NMEA 2000 PGN codecs + an NMEA 0183 serial parser
```

### `machbus::net` — the wire layer

This is the foundation. It knows nothing about the *meaning* of a message — only its
*shape* on the bus. Here you find:

- **`Identifier`** — the 29-bit extended CAN identifier, with structured accessors for
  priority, PGN, source address, and destination. It does the PDU1/PDU2 arithmetic for
  you (more on that trap in the next page).
- **`Message`** — an identifier paired with up to eight data bytes; the unit a node
  actually sends and receives.
- **`Pgn`** and the PGN helpers — the Parameter Group Number, plus utilities to extract
  the PF/PS fields, decide whether a PGN is broadcast, and look up known PGNs.

If all you want is to read a candump line and answer "who sent this, to whom, at what
priority, and which parameter group is it?", `net` alone is enough.

### `machbus::j1939` — named messages

`net` gives you a PGN number. `j1939` tells you what that number *means*. It is a
collection of **wire codecs**, one family per concern: engine controllers, fuel and
temperature, hours, transmission, speed and distance, the whole diagnostic "DM" family,
acknowledgements, requests, and proprietary messages. Each codec takes the raw bytes for
its PGN and produces a typed Rust struct with named, scaled fields.

### `machbus::nmea` — positioning and the marine heritage

NMEA 2000 rides on the *same* 29-bit CAN bus as J1939 and ISOBUS. In agriculture it is
how GNSS receivers, compasses, and weather sensors report position, heading, course, and
environment. The `nmea` module decodes a focused subset of N2K PGNs through its
`NMEAInterface`, handles the N2K network-management PGNs, and — as a bonus — ships an
**NMEA 0183** parser for the older serial-text sentences a GNSS puck might emit over a
UART.

## The decode pipeline as a mental model

Every decode, no matter the protocol, follows the same four steps. Hold this picture in
your head and the rest of these pages will slot into place.

```
   ┌──────────────────────────────────────────────────────────────┐
   │  1. RAW FRAME                                                  │
   │     29-bit identifier  +  0–8 data bytes                       │
   │     e.g.  0x0CF00400   [ FF FF 68 13 FF FF FF FF ]            │
   └───────────────────────────┬──────────────────────────────────┘
                               │  net::Identifier::from_raw(...)
                               ▼
   ┌──────────────────────────────────────────────────────────────┐
   │  2. STRUCTURED IDENTIFIER                                      │
   │     priority = 3   PGN = 0xF004   source = 0x00   dst = global │
   │     "broadcast, electronic engine controller #1"              │
   └───────────────────────────┬──────────────────────────────────┘
                               │  match on the PGN
                               ▼
   ┌──────────────────────────────────────────────────────────────┐
   │  3. PICK THE CODEC FOR THAT PGN                               │
   │     PGN 0xF004 → j1939 EEC1 codec                            │
   │     PGN 129025 → nmea rapid-position codec                   │
   └───────────────────────────┬──────────────────────────────────┘
                               │  decode the data bytes
                               ▼
   ┌──────────────────────────────────────────────────────────────┐
   │  4. TYPED STRUCT                                              │
   │     Eec1 { engine_speed: 620.0 rpm, … }                       │
   └──────────────────────────────────────────────────────────────┘
```

Read it left to right:

1. **Raw frame.** Whatever your CAN driver hands you: an identifier and a byte slice.
   This is the same shape whether the frame is J1939, NMEA 2000, or plain ISOBUS — they
   all share the 29-bit CAN format.
2. **Structured identifier.** `net::Identifier` cracks the 29 bits into priority, PGN,
   source, and destination. This step is protocol-agnostic; it is pure bit arithmetic.
3. **Pick the codec.** The PGN is the routing key. You look at it and decide which
   decoder applies — a J1939 engine codec, an NMEA navigation codec, or none (an unknown
   or proprietary PGN you choose to ignore).
4. **Typed struct.** The chosen codec interprets the data bytes — applying scale factors,
   offsets, bit masks, and "not available" sentinels — and gives you a struct with real
   units.

The crucial insight: **steps 1–2 are universal, steps 3–4 are protocol-specific.** That
is exactly why machbus splits `net` (universal) from `j1939` and `nmea` (specific).

## A note on multi-frame messages

Step 1 above assumes one frame carries one whole message. Often it does — most J1939 and
N2K signals fit in eight bytes. But some messages are larger: a long diagnostic list, a
detailed GNSS fix with every satellite. Those are split across many frames and
reassembled by a *transport protocol* (J1939 TP/BAM/ETP) or *Fast Packet* (NMEA 2000).

For the basics, treat reassembly as a black box that sits between steps 1 and 2: many
frames go in, one logical payload comes out, and from there the pipeline is unchanged.
The J1939 and NMEA 2000 pages explain how those mechanisms work conceptually.

## When to use this vs the full session facade

Reach for the **decode modules** when you want to *observe* the bus:

- You have a candump trace, a log file, or a live socket and you want to know what is on
  it.
- You are building a dashboard, a logger, a black-box recorder, or a diagnostics viewer.
- You only care about *reading* engine, GNSS, or diagnostic data — you are not a
  participant, just a listener.
- You want a tiny dependency: no address claiming, no network manager, no timers.

Reach for the **session facade** when you want to *participate*:

- You need a claimed address and a NAME on the network.
- You must respond to requests, send acknowledgements, or run a Virtual Terminal or Task
  Controller client.
- You need the network manager to track partners, handle address conflicts, and run
  transport sessions for you.

A good rule of thumb: **if you are read-only, stay in `net` / `j1939` / `nmea`.** The
moment you need to *talk back* in a way the network must track, move up to the session
facade.

## Where to go next

Three conceptual pages build the picture from the bottom up:

- [Anatomy of a CAN frame](can-frames.md) — the bus, the 29-bit identifier, and the
  PDU1/PDU2 split that trips up everyone.
- [J1939 messages and PGNs](j1939-messages.md) — from frames to named messages, the
  transport protocols, the DM diagnostic family, and what `j1939` decodes.
- [NMEA 2000 on the bus](nmea2000.md) — how N2K reuses J1939, Fast Packet, the GNSS
  PGNs, and what `nmea` decodes.

Then three hands-on tutorials show the actual code:

- [Tutorial: inspect CAN identifiers](tut-inspect-can.md)
- [Tutorial: decode J1939 PGNs](tut-j1939.md)
- [Tutorial: decode NMEA 2000](tut-nmea2000.md)

For the deep theory behind the wire — why the identifier looks the way it does, how
address claiming works, how transport is specified — see the
[standards section](../standards/index.md), in particular
[SAE J1939: the heritage](../standards/j1939.md) and
[Positioning: NMEA and GNSS](../standards/positioning.md).
