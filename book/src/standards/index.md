# The standards, end to end

This section is the *story* of how an ISOBUS machine works — not a clause-by-clause
recital, but the narrative an engineer needs to hold in their head. By the end you
should be able to look at any frame on a tractor bus and know which layer produced
it, why, and what happens next.

We build the picture from the wire up: a single twisted pair, then the language
spoken on it, then the agreement that lets a tractor and a stranger's implement
cooperate the first time they are bolted together.

> Throughout, standards are named only at the part level — "ISO 11783-6 (Virtual
> Terminal)", "SAE J1939", "NMEA 2000". The explanations are written from how
> machbus implements the behavior; consult the official documents for normative
> wording.

## The one-paragraph version

A tractor and an implement are two computers that have never met. They share two
copper wires. **CAN** lets them put bits on those wires without electrocuting each
other's messages. **SAE J1939** turns those bits into *named* messages (PGNs) sent
between *addresses*. **ISO 11783** takes J1939 and adds everything farming needs:
a way to claim an address by *identity* so two strange devices never collide, a way
to move data bigger than 8 bytes, a screen-sharing protocol so an implement can
draw its own controls on the tractor's terminal, a job/data protocol so a field
computer can run a prescription, plus diagnostics, files, guidance, and safety
interlocks. **AEF** is the club that certifies all of this actually interoperates;
**NMEA 2000** is the cousin protocol that carries the GPS fix.

## The layer cake

Everything below sits on the layer above's shoulders. Read it bottom-to-top: each
layer only worries about its own job and trusts the one beneath it.

```
   ┌────────────────────────────────────────────────────────────────────┐
   │ application services   VT · TC · File Server · Sequence Control ·  │
   │                        TIM · diagnostics · guidance · GNSS feed    │
   │                        (ISO 11783-6, -7, -9 … -14)                 │
   ├────────────────────────────────────────────────────────────────────┤
   │ messages & transport   PGN requests/acks · TP / ETP · Fast Packet ·│
   │                        interconnect routing                        │
   │                        (ISO 11783-3, -4 · SAE J1939)               │
   ├────────────────────────────────────────────────────────────────────┤
   │ identity & addressing  NAME · address claim · who-may-talk         │
   │                        (ISO 11783-5)                               │
   ├────────────────────────────────────────────────────────────────────┤
   │ naming                 29-bit ID = priority + PGN + source + dest  │
   │                        (SAE J1939)                                 │
   ├────────────────────────────────────────────────────────────────────┤
   │ wire                   250 kbit/s CAN, 29-bit extended frames      │
   │                        (ISO 11783-2, ISO 11898)                    │
   └────────────────────────────────────────────────────────────────────┘
```

For a one-screen index of every part — what it does, where machbus implements it,
and which chapter covers it — see [Standards capability map](standards-capability-map.md).

The deep-dive chapters in this section follow these bands:

- **[The networking foundation](foundations.md)** — the bottom four bands: CAN,
  J1939 naming, address claiming, and transport. This is the spine; nothing else
  works until a node has claimed an address and can move data.
- **[The Virtual Terminal](virtual-terminal.md)** — ISO 11783-6, the
  screen-sharing protocol and the most elaborate application service.
- **[The Task Controller and the data dictionary](task-controller.md)** —
  ISO 11783-10 and -11: documented work, process data, and the DDI vocabulary.
- **[Implement control, the tractor ECU, and the rest](implement-and-services.md)**
  — ISO 11783-7/-9 plus diagnostics (-12), File Server (-13), Sequence Control
  (-14), and TIM.
- **[Positioning: NMEA and GNSS](positioning.md)** — how the fix gets onto the bus.

## Who is on the bus

A useful mental model before the protocols: an ISOBUS network is a small town of
**control functions** (CFs). Each CF is one participant with one job and one
identity. A physical box (an **ECU**) may host several CFs. The cast of characters
on a typical tractor-plus-implement bus:

```
   The bus — control functions side by side:

   ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
   │ Tractor         │ │ Virtual         │ │ Task            │ │ Implement       │ │ GNSS            │
   │ ECU (TECU)      │ │ Terminal        │ │ Controller      │ │ ECU             │ │ receiver        │
   │ speed · hitch   │ │ the screen,     │ │ runs the job,   │ │ VT + TC client, │ │ position,       │
   │ PTO · GNSS      │ │ soft keys       │ │ logs work       │ │ the actuators   │ │ COG / SOG       │
   └─────────────────┘ └─────────────────┘ └─────────────────┘ └─────────────────┘ └─────────────────┘
```

The tractor side mostly *serves* (it has the screen, the speed, the hitch). The
implement side mostly *requests* (it borrows the screen, asks for the job, reports
what it did). Many roles come in **client/server pairs** — a VT *server* (the
terminal) and VT *clients* (each implement), a TC *server* and TC *clients*. machbus
implements both halves of each pair.

## A machine waking up: the first ten seconds

The single most important sequence to internalize is what happens when you turn the
key. Every protocol above the wire depends on it.

```
 t0  Power on. The CF has a NAME (its 64-bit identity) and a *preferred* address,
     but is not yet allowed to send application traffic.

 t1  It broadcasts an Address Claim for its preferred address, putting its NAME
     on the wire as the tie-breaker.

 t2  Silence for the contention window?  → the address is ours. Claimed.
     Someone else claims the same address? → the *lower NAME wins*. The loser
     either grabs another address (if self-configurable) or goes quiet.

 t3  Now addressed, the CF starts its periodic chores: a TECU broadcasts wheel
     speed and hitch state; a CF may begin its heartbeat.

 t4  An implement's VT client goes looking for a terminal: "any VT out there?"
     It discovers the VT's address and version.

 t5  The VT client uploads its *object pool* — the entire description of its UI —
     in one big transport-protocol transfer, then asks the VT to activate it.

 t6  In parallel, the TC client uploads its *device description* (the DDOP) to the
     Task Controller and the two begin trading process data.

 t7  Steady state: status broadcasts tick along, the operator presses soft keys,
     setpoints flow down, as-applied values flow up, and the GNSS receiver feeds
     position the whole time.
```

machbus mirrors exactly this order. In the session facade you `plug` the subsystems
you need, `start()` the claim, and drive `poll()`; address claim runs first and the
application plugins only act once an address is held. See
[The session facade](../guide/session-facade.md).

## Why identity-based addressing is the clever bit

J1939 alone assigns addresses more or less by convention. That is fine for a truck
built by one manufacturer. It falls apart the moment a *random* implement from a
*different* manufacturer is hitched to a tractor it has never seen — both might want
the same address.

ISO 11783-5 (network management) solves this with **NAME-based arbitration**. Every
CF carries a 64-bit NAME encoding what it *is* (manufacturer, device class,
function, instance, and a self-configurable flag), not where it sits. When two CFs
want the same address, the numerically lower NAME wins the address and the other
moves. Because NAMEs are unique by construction, the network always converges — no
human, no DIP switches, no central authority. This is the property that makes
"hitch any implement to any tractor" actually work, and it is why address claim is
the first thing every node does.

```
   Conflict on address 0x80:

   CF-A  NAME = 0x00A0_0000_0000_1234   ─┐
   CF-B  NAME = 0x00C0_0000_0000_5678   ─┤  compare NAMEs
                                          ▼
                 0x00A0… < 0x00C0…  →  CF-A keeps 0x80
                 CF-B is self-configurable → claims 0x81 instead
```

## Two sizes of message, and why transport exists

A raw CAN frame carries at most 8 bytes. Plenty for "engine speed = 1500 rpm";
hopeless for "here is a 40 KB object pool describing my user interface." ISO 11783
inherits J1939's answer and extends it:

- **Single frame** — ≤ 8 bytes, one shot.
- **Transport Protocol (TP)** — up to 1785 bytes, broken into 7-byte packets with
  flow control (a destination can say "send me packets 1–16, then pause").
- **Extended Transport Protocol (ETP)** — megabyte-scale transfers for big object
  pools and files.
- **Fast Packet** — NMEA 2000's lighter multi-frame scheme for things like a GNSS
  position record.

The deep dive covers the handshakes. The key idea: every layer above transport gets
to pretend messages are arbitrarily large; transport quietly chops and reassembles.

## The application services, in one breath

Once a node can claim an address and move data, the farming-specific services are
just well-defined conversations on top:

| Service (part) | The conversation, in plain words |
| --- | --- |
| Virtual Terminal (-6) | "Here is my whole UI. Draw it. Tell me what the operator touches; I'll tell you what to change." |
| Implement messages (-7) | "Here is my hitch/PTO/section/speed state" — and the commands to change it. |
| Tractor ECU (-9) | The tractor's promise of which facilities (speed, hitch, PTO, guidance) it offers. |
| Task Controller (-10) | "Here is what I am and what I can measure/control (my DDOP). Now let's trade process data for this job." |
| Data dictionary (-11) | The shared vocabulary (DDIs) so "application rate" means the same number to everyone. |
| Diagnostics (-12) | "Here are my active faults" — and the service-tool requests to read and clear them. |
| File Server (-13) | A shared filesystem on the bus: open/read/write/close, volumes and directories. |
| Sequence Control (-14) | "Run this saved sequence of steps" — headland automation and the like. |
| TIM (AEF) | "May I, the implement, command the tractor's hitch/PTO?" — authority with safety interlocks. |

Each has a dedicated codec and a session plugin in machbus. The deep-dive chapters
tell each story properly.

## Where AEF and NMEA fit

- **AEF** (Agricultural Industry Electronics Foundation) does not invent wire
  formats; it defines *functionalities* and a certification process that proves two
  vendors' boxes actually interoperate. TIM (Tractor Implement Management) is an
  AEF-driven capability layered on the ISO messages. machbus models the
  functionality advertisement and the TIM authority guard, but **ships no
  certification** — see [Conformity first](../conformity/index.md).
- **NMEA 2000** is a separate but CAN-compatible standard. On an ISOBUS machine it
  is how the GNSS receiver publishes position, course, speed, and attitude. machbus
  decodes the relevant PGNs so guidance and TC-GEO have a fix to work with.

## How to read the rest of this section

The four deep-dive chapters are written to be read in order but stand alone. Each
follows the same arc: the field problem, the mental model, the message anatomy, the
lifecycle (with diagrams), how machbus expresses it, and the failure modes that bite
in practice.

- [The networking foundation](foundations.md)
- [The Virtual Terminal](virtual-terminal.md)
- [The Task Controller and the data dictionary](task-controller.md)
- [Implement control, the tractor ECU, and the rest](implement-and-services.md)
- [Positioning: NMEA and GNSS](positioning.md)

If you would rather learn by building, jump to [The session facade](../guide/session-facade.md)
and come back here when a frame surprises you.
