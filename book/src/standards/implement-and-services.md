# Application services: implement, tractor, and the rest

The Virtual Terminal and Task Controller get the spotlight, but a working machine runs
on a handful of quieter services: the messages that move a hitch and spin a PTO, the
tractor's promise of what it offers, diagnostics, the on-bus filesystem, saved
automation sequences, and the AEF authority that lets an implement command the tractor.
This page is the map; each service has its own chapter.

## The services at a glance

```
   ISO 11783-7   implement messages   hitch · PTO · aux valves · speed · lighting
   ISO 11783-9   tractor ECU          facilities advertisement + status source
   ISO 11783-12  diagnostics          DM1 faults, clears, freeze frames, memory, IDs
   ISO 11783-13  File Server          a shared filesystem on the bus
   ISO 11783-14  sequence control     headland automation and saved step sequences
   AEF / TIM     authority            implement commands the tractor, under interlocks
```

## Read each service

- [ISO 11783-7 — implement messages](iso11783-implement-messages.md) — the physical-control
  vocabulary (hitch, PTO, aux valves, speed/distance, lighting).
- [ISO 11783-9 — the tractor ECU](iso11783-tractor-ecu.md) — facilities, classes, and
  the capability contract.
- [ISO 11783-12 — diagnostics](iso11783-diagnostics.md) — the DM fault family and
  service-tool access.
- [ISO 11783-13 — the File Server](iso11783-file-server.md) — files, volumes, and the
  TAN-matched request/response model.
- [ISO 11783-14 — sequence control](iso11783-sequence-control.md) — master/client step
  automation.
- [TIM (AEF)](tim.md) — authority with safety interlocks.

## The supporting cast

A few more services round out a real node, each an machbus plugin:

| Service | One line | Plugin |
| --- | --- | --- |
| Heartbeat | Periodic "I'm alive" for liveness detection. | `Heartbeat` |
| Maintain Power | Keep tractor power after key-off to finish safely. | `MaintainPower` |
| Shortcut Button / ISB | The cab "stop everything" safe-state signal. | `ShortcutButton` |
| Language Command | Broadcast locale and unit preferences. | `LanguageCommand` |
| Auxiliary (AUX-O / AUX-N) | Joystick / switch-bank inputs assigned to functions. | `Auxiliary` |
| Functionalities / Group fn / Request2 / NAME mgmt | Advertisement and request/response plumbing that keeps the network self-describing. | `ControlFunctionalities`, `GroupFunction`, `Request2`, `NameManagement` |

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| A curated tractor node | `session::presets::tractor()` | [Tractor ECU](../tutorials/tractor-ecu.md) |
| A curated implement node | `session::presets::implement(pool, ws, ddop)` | [Implement ECU](../tutorials/implement-ecu.md) |
| The small responders | the matching `session::plugins` | [The session facade](../guide/session-facade.md) |

## See also

- [The networking foundation](foundations.md) — what these services stand on.
- [The Task Controller](task-controller.md) and [The Virtual Terminal](virtual-terminal.md)
  — the two big implement-side services.
- [Standards capability map](standards-capability-map.md) — the one-screen index.
