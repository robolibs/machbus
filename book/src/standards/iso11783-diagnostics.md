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

> **ISOBUS DM1 and DM2 have no lamp bytes.** Annex B.6 and B.7 define bytes 1–2
> as "Reserved, set to FF16" and put SPN/FMI/occurrence in bytes 3–6; the word
> "lamp" does not appear in ISO 11783-12 at all. J1939-73 *does* put lamp status
> there, which is why `DmDtcList` keeps two encoders — `encode()` for the J1939
> form and `encode_iso()` for the ISOBUS one. The diagnostics plugin broadcasts
> the ISOBUS form.

### Advertising functionalities is forward-compatible by design

The Control Function Functionalities message (PGN 0xFC8E) is how a node says
which roles it implements and at which generation. Annex B.9 is unusually
explicit that a reader must tolerate what it does not recognise:

> "Functionality characteristics values reserved for ISO assignment shall be
> parsed without generating an error." … "If the number of option bytes is
> larger than specified in this document for a functionality, the receiving CF
> shall ignore the undefined functionality option bytes and parse the known
> option bytes for this functionality only."

Both halves matter, because A.10 keeps the 0–255 functionality list in the
online database — it grows between revisions. A decoder that rejects the message
over one unknown code throws away every functionality it *did* understand, so
machbus skips unknown blocks using their own declared option length and keeps
the rest.

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
