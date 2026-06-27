# ISOBUS in plain words

ISOBUS is the agricultural-machine family of CAN-based communication built on
SAE J1939 ideas and ISO 11783 application layers. A tractor, implement, Virtual
Terminal, Task Controller, GNSS receiver, file server, service tool, and
specialized controllers may all share the same physical network.

If you are new to it, do not start with the whole protocol surface. Start with
the mental model:

1. every communicating role is a **control function**;
2. every control function has a stable **NAME**;
3. the NAME is used to claim a temporary **source address**;
4. normal traffic is grouped by **PGN**;
5. short payloads fit in one CAN frame;
6. long payloads use TP, ETP, or Fast Packet;
7. application services such as VT, TC, FS, SC, TIM, diagnostics, and GNSS build
   workflows on top of those lower layers.

machbus groups those layers into modules and stack helpers so application code
can work with typed messages instead of raw bytes most of the time.

## Mental model

```text
Application role
  └─ machbus session/plugins
       └─ ISOBUS/J1939/NMEA services
            └─ transport protocols
                 └─ CAN frames
```

## Two ways to look at the stack

From the bus upward:

| Layer | Question it answers |
|---|---|
| CAN | Which frame won arbitration, and what bytes arrived? |
| J1939 identifier | What priority, PGN, source, and destination are encoded in the identifier? |
| Address claim | Which NAME currently owns which source address? |
| Transport | Is this one payload or a reassembled multi-frame payload? |
| Application protocol | Is this diagnostics, VT, TC, FS, GNSS, TIM, or another service? |
| Your application | What should the machine or UI do with that information? |

From an application downward:

| Application thought | Protocol reality |
|---|---|
| "I am an implement." | Build an implement node (plug `Implement` / `presets::implement(...)`) with a NAME and preferred address. |
| "I need a terminal." | Find a VT partner by NAME/function and connect the VT client. |
| "I need a task controller." | Find a TC partner, upload a DDOP, then exchange process data. |
| "I need to send 2 kB." | Let TP/ETP split and reassemble the payload. |
| "I need to react to an ISB." | Subscribe to the Shortcut Button state and move to the application safe state. |

## Where to go next

This page is the gentle on-ramp. For the full story — every part explained with
diagrams and a path from concept to code — read
[The standards, end to end](../standards/index.md), which builds the picture from
the wire up:

1. [The networking foundation](../standards/foundations.md) — CAN, J1939, NAME and
   address claim, and transport (the spine everything else stands on).
2. [The Virtual Terminal](../standards/virtual-terminal.md) — how an implement
   borrows the cab's screen.
3. [The Task Controller](../standards/task-controller.md) — documented work and the
   shared DDI vocabulary.
4. [Application services](../standards/implement-and-services.md) — implement
   control, the tractor ECU, diagnostics, File Server, sequence control, and TIM.
5. [Positioning: NMEA and GNSS](../standards/positioning.md) — getting the fix onto
   the bus.

When you want to *see* this on a real bus, jump to
[Reading candump traces](reading-candump-traces.md); for the source documents, see
[Further reading](further-reading.md). When you are ready to build, start with
[The session facade](../guide/session-facade.md).

## What this section is not

This section is a practical orientation guide. It is not a copy of ISO 11783,
and it does not replace official protocol documents or external certification
work. The goal is to give you enough shape to understand the machbus API and to
debug traces without drowning in every part of the standard on day one.
