# Anatomy of a CAN frame

Everything on an agricultural bus — J1939 engine data, NMEA 2000 position, ISOBUS
commands — is, at the very bottom, a **CAN frame**. If you understand the frame, you
understand the substrate that all three protocols share. This page builds that picture
from the wire up, and ends with the single detail that confuses every newcomer: the
PDU1/PDU2 split.

You will use `machbus::net` to work at this layer. Its `Identifier` type *is* the
structured form of what this page describes.

## The bus

CAN — Controller Area Network — is a two-wire, multi-drop, broadcast bus. Every node is
wired to the same pair of differential wires (CAN-H and CAN-L), terminated at both ends.

```
        ┌──────┐    ┌──────┐    ┌──────┐    ┌──────┐
        │ ECU  │    │ GNSS │    │ Term │    │ Disp │
        │  A   │    │      │    │ inal │    │ lay  │
        └──┬───┘    └──┬───┘    └──┬───┘    └──┬───┘
   120Ω    │           │           │           │   120Ω
   ──┳━━━━━┷━━━━━━━━━━━┷━━━━━━━━━━━┷━━━━━━━━━━━┷━━━━┳──  CAN-H
     ┃                                              ┃
   ──┻━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┻──  CAN-L
```

Two consequences shape everything else:

- **There are no addresses on the wire.** Every node hears every frame. Filtering by
  who-is-it-for happens in software, *above* CAN. This is why a decoder can simply listen
  and see the whole bus.
- **Frames carry an identifier, not a destination.** The identifier names the *message*,
  and arbitration uses it to decide who transmits when two nodes start at once. A lower
  numeric identifier wins; the loser backs off and retries. This is how "priority" gets
  baked into the identifier — lower value, higher priority.

## Two identifier sizes: 11-bit vs 29-bit

CAN comes in two flavours of identifier:

| Form              | ID width | Common name        | Used by                          |
|-------------------|----------|--------------------|----------------------------------|
| CAN 2.0A          | 11 bits  | Standard / base    | Simple automotive, appliances    |
| CAN 2.0B          | 29 bits  | Extended           | J1939, ISOBUS, NMEA 2000         |

An 11-bit identifier gives you 2048 possible message IDs — fine for a small closed
system. Agriculture and heavy vehicles need far more structure than that: a priority, a
message identity, *and* the address of the sender, all packed into the identifier itself.
Eleven bits cannot hold all of it.

So agriculture uses **29-bit (extended) identifiers**. The extra 18 bits are exactly what
lets J1939 — and therefore ISOBUS and NMEA 2000 — carry priority, a Parameter Group
Number, and a source address in every single frame. When you see a 29-bit ID on a farm
bus, it is almost certainly one of these three protocols, all of which share the same
layout.

## The 29-bit identifier layout

Here is the layout J1939 defined and ISOBUS and NMEA 2000 inherited. Bit 28 is the most
significant.

```
   bit  28      26 25  24 23           16 15            8 7             0
        ┌─────────┬───┬───┬───────────────┬───────────────┬─────────────┐
        │ priority│EDP│ DP│   PF          │   PS          │  source     │
        │ 3 bits  │ 1 │ 1 │  PDU format   │ PDU specific  │  address    │
        │         │   │   │  8 bits       │ 8 bits        │  8 bits      │
        └─────────┴───┴───┴───────────────┴───────────────┴─────────────┘
         \_______/ \_____________________________________/ \___________/
          priority            the PGN lives here              who sent it
```

Field by field:

- **Priority (3 bits).** 0–7. Lower wins arbitration. Safety-critical messages get low
  numbers; routine status gets higher ones. A common default is 6.
- **EDP — Extended Data Page (1 bit).** Selects which PGN page the message lives on.
  J1939/ISOBUS messages use EDP = 0. NMEA 2000 also lives in the standard pages. EDP = 1
  is reserved for other ISO 15765 uses.
- **DP — Data Page (1 bit).** A second page-selector bit. Together EDP and DP extend the
  PGN number space.
- **PF — PDU Format (8 bits).** The high byte of the message identity. Its value decides
  whether the frame is point-to-point or broadcast (see below).
- **PS — PDU Specific (8 bits).** The low byte. Its meaning *depends on PF* — it is
  either a destination address or part of the message identity. This is the trap.
- **Source address (8 bits).** Who sent the frame. 0–253 are real node addresses, 254 is
  the null address (used during address claiming), and 255 is the global/broadcast
  address.

The middle 18 bits — EDP, DP, PF, and (sometimes) PS — together form the **PGN**, the
Parameter Group Number that names the message. The next page is all about PGNs.

`net::Identifier` gives you each of these directly: `priority()`, `pdu_format()`,
`pdu_specific()`, `source()`, `data_page()`, `extended_data_page()`, plus the derived
`pgn()`, `destination()`, and `is_broadcast()`.

## The data field: DLC and up to 8 bytes

After the identifier comes the payload. Classic CAN frames carry a **DLC** (Data Length
Code) and **0 to 8 data bytes**.

```
   ┌──────────────────────┬─────┬───────────────────────────────┐
   │  29-bit identifier   │ DLC │  data: 0–8 bytes              │
   └──────────────────────┴─────┴───────────────────────────────┘
                                  └ byte 0 … byte 7 ┘
```

Eight bytes is a hard ceiling for classic CAN. That single constraint explains a huge
amount of how J1939 and NMEA 2000 are designed:

- Signals are packed tightly — a field might be one byte, or two bytes little-endian, or
  even a handful of bits — to fit the budget.
- Anything larger than eight bytes must be **split across multiple frames** and
  reassembled. That is what transport protocols (J1939 TP/BAM/ETP) and Fast Packet (NMEA
  2000) exist to do. Each gets covered on its protocol's page.

Two more details worth knowing as a decoder:

- **Byte order is little-endian.** Multi-byte signals put the least-significant byte
  first. A two-byte value in bytes 3–4 is `byte3 + (byte4 << 8)`.
- **0xFF means "not available."** A byte (or all bytes of a signal) set to all-ones is
  the standard "no data" sentinel. A scaled signal reading its maximum raw value almost
  always means *the sensor has nothing to report*, not a real reading. Treat these as
  missing, never as data.

## Bit timing and bus speed (briefly)

You rarely touch this as a decoder, but it is worth one paragraph.

The ISOBUS / NMEA 2000 baseline runs at **250 kbit/s**. Every node on the segment must
agree on this rate, or nothing communicates. The bit rate is built from a *nominal bit
time* divided into time quanta, with a sample point typically placed around 75–87% of the
bit. The sample point and synchronization-jump-width let nodes stay in lock-step despite
clock drift and propagation delay along the cable. J1939 buses also commonly run at 250
kbit/s, with some segments at 500 kbit/s.

The practical takeaway: if you are capturing a farm bus, configure your interface for
**250 kbit/s, extended (29-bit) identifiers**, and you will see the traffic. The deeper
electrical and timing rules live in
[ISO 11783-2: the physical layer](../standards/iso11783-physical-layer.md).

## The PDU1 vs PDU2 split — read this twice

This is the one rule that catches everyone. The meaning of the **PS byte** — and
therefore how you compute the PGN and the destination — depends entirely on the **PF
byte**.

```
   ┌─────────────────────────────────────────────────────────────┐
   │  if  PF < 240   →   PDU1   (point-to-point)                  │
   │                                                              │
   │      PS = DESTINATION ADDRESS                                │
   │      "send PGN <PF·00> to node <PS>"                         │
   │      the PGN's low byte is forced to 0                       │
   │                                                              │
   │  if  PF ≥ 240   →   PDU2   (broadcast)                       │
   │                                                              │
   │      PS = GROUP EXTENSION (part of the PGN)                  │
   │      "broadcast PGN <PF·PS> to everyone"                     │
   │      there is no destination — it is for all                 │
   └─────────────────────────────────────────────────────────────┘
```

Why it matters: the *same* 8-bit PS field is a destination in one case and message
identity in the other. If you blindly fold PS into the PGN, you will compute nonsense
PGNs for every peer-to-peer message. If you blindly read PS as a destination, you will
think broadcast messages are addressed to phantom nodes.

### PDU1 example — a request to one node

```
   identifier bytes (PF=0xEA, the Request PGN):
        prio  EDP/DP   PF=0xEA   PS=0x26   src=0x80
                                  └ destination! ─┘

   → PGN  = 0xEA00   (PS is NOT part of it; low byte is 0)
   → dest = 0x26     (the PS byte)
   → "address 0x80 is requesting something from address 0x26"
```

### PDU2 example — a broadcast signal

```
   identifier bytes (PF=0xF0, an engine controller PGN):
        prio  EDP/DP   PF=0xF0   PS=0x04   src=0x00
                                  └ group extension ┘

   → PGN  = 0xF004   (PS IS part of it)
   → dest = global   (broadcast, no specific node)
   → "address 0x00 is broadcasting EEC1 to everyone"
```

### The mechanics in one table

| Condition | PDU type   | PS byte means        | PGN low byte | Destination     |
|-----------|------------|----------------------|--------------|-----------------|
| PF < 240  | PDU1       | destination address  | forced to 0  | the PS value    |
| PF ≥ 240  | PDU2       | group extension      | equals PS    | global (all)    |

`net::Identifier` implements all of this for you. `is_pdu2()` returns true when PF ≥ 240;
`pgn()` folds PS in only for PDU2; `destination()` returns the PS value for PDU1 and the
broadcast address for PDU2. You never have to do the masking by hand — but you must
understand *why* the same bits decode two different ways, because the trap reappears the
moment you read a raw trace without the helper.

## Putting it together: reading one frame

Take a single candump-style line: an identifier and some bytes. To decode it as a human:

1. Split the 29 bits into priority, EDP/DP, PF, PS, source.
2. Look at PF. Is it `< 240` (PDU1, point-to-point) or `≥ 240` (PDU2, broadcast)?
3. Compute the PGN accordingly, and the destination accordingly.
4. Now you have the routing key (PGN) and can hand off to a J1939 or NMEA codec.

That hand-off — PGN to *named message* — is the subject of the next two pages.

## Cross-links

- [ISO 11783-2: the physical layer](../standards/iso11783-physical-layer.md) — the electrical
  bus, termination, bit timing, and 250 kbit/s in depth.
- [SAE J1939: the heritage](../standards/j1939.md) — where the 29-bit identifier and the
  PDU1/PDU2 split came from, and what ISOBUS added.
