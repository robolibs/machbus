# J1939 messages and PGNs

The previous page took a raw 29-bit frame and cracked it into priority, PGN, source, and
destination. The PGN is the *routing key*. This page is about what that key unlocks:
turning a PGN into a **named message** with named, scaled signals — the job of the
`machbus::j1939` module.

SAE J1939 is the heavy-vehicle networking standard that ISOBUS was built on. The engine,
transmission, and diagnostic messages an agricultural bus carries are, overwhelmingly,
J1939 messages. Decode them and you can read a tractor's powertrain without ever joining
the network.

## PGNs name the message; SPNs name the signal

Two acronyms carry the whole model.

- **PGN — Parameter Group Number.** A number that identifies a *group of related
  parameters* — one message type. "Electronic Engine Controller 1" is a PGN. "Engine
  Hours" is a PGN. The PGN is what you matched on in the identifier.
- **SPN — Suspect Parameter Number.** A number that identifies a *single signal* within
  (or referenced by) a PGN. "Engine speed" is an SPN. "Engine coolant temperature" is an
  SPN. SPNs are how individual values get named, and they matter most in diagnostics,
  where a fault points at a specific SPN.

```
   PGN  0xF004  "Electronic Engine Controller 1"   ← one message
     ├── SPN 190  engine speed          (2 bytes, 0.125 rpm/bit)
     ├── SPN 513  actual torque         (1 byte,  1 %/bit, -125 offset)
     ├── SPN 512  driver demand torque  (1 byte,  …)
     └── …
```

A J1939 codec's whole job is this mapping: given the PGN's data bytes, pull out each SPN
at its byte/bit position, apply its **scale** and **offset**, honour the **not-available**
sentinel, and hand you a typed value in real units. machbus does this inside each codec
in `j1939`, so you receive a struct like `Eec1 { engine_speed, … }` rather than raw
bytes.

### Scaling, offset, and "not available"

Every SPN has three things you must respect:

- **Resolution (scale).** Raw bits times a factor. Engine speed is `raw × 0.125 rpm`.
- **Offset.** Added after scaling. A percent-torque signal might be `raw × 1 − 125`,
  letting one byte cover −125% to +130%.
- **Not-available sentinel.** The top of the raw range (all-ones) means "no reading."
  A two-byte signal of `0xFFFF` is *missing*, not 8191.875 rpm.

Get any of these wrong and your decode is silently, plausibly wrong — which is worse than
crashing. The codecs in `j1939` encode them once, correctly, so you do not re-derive them
per project.

## Single-frame vs multi-frame messages

Most J1939 messages fit in the eight-byte CAN payload, so the pipeline is direct: one
frame in, one decoded struct out. But some messages are larger than eight bytes — a long
list of active faults, a block of vehicle-identification text. Those need a **transport
protocol** to break the payload into numbered packets and reassemble it on the far side.

J1939 defines three transport mechanisms. Conceptually:

```
   ┌──────────────────────────────────────────────────────────────┐
   │  TP — Transport Protocol  (up to 1785 bytes)                  │
   │                                                              │
   │   BAM  Broadcast Announce Message                            │
   │        "I'm about to send N bytes of PGN X to everyone"      │
   │        then a stream of numbered data packets. No handshake. │
   │        Best-effort, one-to-all.                              │
   │                                                              │
   │   RTS/CTS  Request-To-Send / Clear-To-Send                   │
   │        A point-to-point handshake: sender asks, receiver     │
   │        grants a window, data flows, receiver acknowledges.   │
   │        Flow-controlled, one-to-one.                          │
   └──────────────────────────────────────────────────────────────┘

   ┌──────────────────────────────────────────────────────────────┐
   │  ETP — Extended Transport Protocol  (very large transfers)   │
   │        Same idea as RTS/CTS but with a wider length field,    │
   │        for payloads beyond the TP ceiling.                    │
   └──────────────────────────────────────────────────────────────┘
```

How to think about it as a decoder:

- **BAM** is the one you will see most when *listening*, because it is broadcast — you can
  reassemble it without participating. Watch for the announce frame, collect the numbered
  data frames, concatenate, then decode the resulting payload as if it had arrived in one
  piece.
- **RTS/CTS** and **ETP** are *directed* and *handshaked*. To receive them you generally
  have to be a claimed participant sending CTS/acknowledgement frames — which is when you
  graduate from the decode modules to the session facade.

For the basics, picture transport as a black box between "many frames" and "one payload."
Once the payload is whole, the PGN-to-struct decode is identical to the single-frame case.
The full handshake rules live in the standards section; see
[SAE J1939: the heritage](../standards/j1939.md).

## The "DM" diagnostic family

Diagnostics deserve their own mention because they are a whole sub-language built on
PGNs, and ISOBUS reuses them almost verbatim. The "DM" (Diagnostic Message) family
reports faults as **DTCs** — Diagnostic Trouble Codes — each a bundle of an SPN (which
parameter), an **FMI** (Failure Mode Identifier — *how* it failed: short, open,
out-of-range, …), and an occurrence count, alongside lamp status (the warning lights).

The members you meet most:

| DM   | Role                                                                 |
|------|---------------------------------------------------------------------|
| DM1  | **Active** diagnostic trouble codes — faults happening *now*         |
| DM2  | **Previously active** DTCs — the fault *history*                     |
| DM3  | Clear previously-active DTCs                                         |
| DM11 | Clear active DTCs                                                    |
| DM13 | Stop/start broadcast — quiet the bus during service                 |
| DM14/15/16 | Memory access — read/write ECU memory (the `dm_memory` codec)  |

The mental model: **DM1 is the present, DM2 is the past.** A diagnostics viewer listens
for DM1 to show what is wrong right now, and can request DM2 to show what *has been*
wrong. Each carries one or more DTCs you then expand into SPN + FMI + count + lamp state.

## What `machbus::j1939` decodes

The `j1939` module ships a codec per PGN family. You match the PGN from the identifier,
pick the matching codec, and get a typed struct. The table below maps the families the
module covers to the kinds of PGN they decode.

| Family / module        | Covers (representative PGNs)                                   | What you get                                            |
|------------------------|---------------------------------------------------------------|---------------------------------------------------------|
| `engine` — EEC1/2/3    | 0xF004 EEC1, 0xF003 EEC2, 0xFEC0 EEC3                          | Engine speed, torque, demand, retarder, friction torque |
| `engine` — fuel/econ   | 0xFEF2 fuel economy, 0xFEE9 fuel consumption                  | Fuel rate, instantaneous & average economy              |
| `engine` — temps       | engine temperature 1 & 2                                       | Coolant, oil, fuel, intercooler temperatures            |
| `engine` — hours       | 0xFEE5 engine hours                                            | Total engine hours and revolutions                      |
| `engine` — other       | ambient conditions, fluid levels, dash display, aftertreatment | Pressures, levels, ambient air, DEF/SCR data            |
| `speed_distance`       | speed & distance PGNs                                          | Wheel/ground speed, trip & total distance               |
| `transmission`         | ETC1 and transmission parameters                              | Selected/current gear, output shaft speed               |
| `diagnostic`           | 0xFECA DM1, 0xFECB DM2, DM3–DM12, DM20–DM25                    | DTC lists (SPN+FMI+count), lamps, freeze frames, IDs    |
| `dm_memory`            | 0xD900 DM14, 0xD800 DM15, 0xD700 DM16                          | Memory-access request/response/transfer                 |
| `diag_monitor`         | DTC delta tracking over DM1                                    | Appeared/cleared fault deltas                           |
| `heartbeat`            | the J1939 heartbeat PGN                                        | Liveness sequence, jump/loss detection                  |
| `language`             | 0xFE0F language command                                        | Units, date/time/decimal format, unit system            |
| `maintain_power`       | 0xFE47 maintain power                                          | Key-switch state, power-down hold requests              |
| `acknowledgment`       | 0xE800 ACK/NACK                                                | Positive/negative acknowledgement and reason            |
| `pgn_request`          | 0xEA00 Request                                                 | Which PGN is being asked for                             |
| `request2`             | 0xC900 Request2 / transfer                                     | Request-2 query, reply, and transfer                    |
| `proprietary`          | proprietary A / proprietary B ranges                          | Raw manufacturer-specific payloads + helpers            |

A few notes on reading this table:

- **EEC1 (0xF004)** is the workhorse — engine speed lives here, broadcast continuously.
  If you decode exactly one J1939 PGN, decode this.
- The **diagnostic** family is the largest single codec because the DM messages are
  numerous and structured; it expands DTC lists into typed `Dtc { spn, fmi, count }`
  records with lamp status.
- **`pgn_request` (0xEA00)** is a PDU1 message — recall from the previous page that its PS
  byte is a *destination*, not part of the PGN. The payload then carries the PGN being
  requested.
- **Proprietary** PGNs are, by definition, manufacturer-defined; the codec gives you the
  raw payload and the addressing helpers so you can route them, but it cannot name the
  signals for you.

## The decode loop in words

Putting the whole J1939 picture together, the listener's loop is:

1. Receive a frame; build a `net::Identifier`.
2. If it is a transport control/data frame, feed it to reassembly; continue until a full
   payload is ready.
3. Take the PGN. Match it against the families above.
4. Hand the payload to the matching `j1939` codec.
5. Receive a typed struct with real units; act on it (log, display, alarm).

Nothing here requires a claimed address as long as you only *listen* and only reassemble
*broadcast* (BAM) transports. The moment you must request directed data or acknowledge
RTS/CTS, move up to the session facade.

## Cross-links

- [SAE J1939: the heritage](../standards/j1939.md) — the identifier, PGN/SPN model, the
  transport handshakes, and the DM family in depth.
- [Anatomy of a CAN frame](can-frames.md) — the PDU1/PDU2 split you need before matching
  any PGN.
- [NMEA 2000 on the bus](nmea2000.md) — the same machinery, applied to positioning.
