# ISO 11783-12 — diagnostics

When a sensor reads out of range, a valve stops responding, or a supply voltage sags,
the ECU that noticed needs to tell the network, and a service technician needs to read
that fault back later. Part 12 is the shared language for that. It reuses the SAE J1939
"DM" diagnostic family almost verbatim and adds a few ISOBUS-specific wrinkles.

## Why this exists

A fault only one ECU knows about is useless to everyone else. Network diagnostics make
a fault *visible* (other nodes and the terminal can show it) and *durable* (a service
tool can read and clear it long after it occurred).

## The anatomy of a fault

```
   a DTC = SPN (which parameter is wrong)
         + FMI (how it is wrong: too high, too low, open circuit, …)
         + occurrence count (how many times)
   plus a lamp panel: malfunction / warning / protect / amber-red status
```

## The DM family

```
   active faults   ──► DM1   broadcast, periodic
   previous faults ──► DM2   on request
   clear           ──► DM3 / DM11 (clear all) · DM22 (clear one, with ack/nack)
   freeze frame    ──► DM25  captured conditions at fault time
   memory access   ──► DM14 / DM15 / DM16   service-tool read/write
   identity        ──► ECU / software / product identification strings
```

The DM1 broadcast is the heartbeat of health: a node periodically announces its active
faults; clearing one moves it to the "previously active" list. ISOBUS adds a sixth
ECU-identification field and the control-function functionalities advertisement on top
of the J1939 base.

## How machbus expresses it

Two plugins cover the family:

- `Diagnostics` — owns the active/previous lists, the periodic DM1 broadcast, and
  request handling (DM1/DM2 requests, DM3/DM11 clears, DM22 individual clears). You
  `raise`/`clear` faults through fine control; inbound peer faults arrive as
  `Event::Diag(DiagEvent::Dm1Received { .. })`.
- `DmMemory` — the service-tool messages (DM14/15/16) and the identity strings, with
  automatic answers to identification requests.

```
   ctrl.with_mut::<Diagnostics, _>(|d| d.raise(dtc));   // active → goes out on DM1
   // peer DM1 → Event::Diag(DiagEvent::Dm1Received { source, active, lamps })
```

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Active/previous faults, DM1 | `session::plugins::Diagnostics` | [Diagnostics](../tutorials/diagnostics.md) |
| Service-tool memory + identity | `session::plugins::DmMemory` | [Diagnostics](../tutorials/diagnostics.md) |
| The DM codecs directly | `j1939::diagnostic` | [Diagnostics](../tutorials/diagnostics.md) |

## Failure modes worth knowing

- **Silent faults** — forgetting to enable diagnostics means a real fault never reaches
  the bus.
- **Stale active list** — clearing must move a DTC to previously-active, not just drop
  it.
- **Identity gaps** — service tools expect identification responses; missing them looks
  like a dead ECU.

## See also

- [SAE J1939 — the heritage](j1939.md) — where the DM family comes from.
- [Implement control, the tractor ECU, and the rest](implement-and-services.md) — the
  services overview.
