# The Task Controller and the data dictionary

If the Virtual Terminal is how a machine *talks to the operator*, the Task
Controller is how it *does documented work*. ISO 11783-10 (Task Controller) defines
the conversation between a field computer and an implement so a prescription can be
executed, section by section, and an accurate as-applied record produced. It only
works because ISO 11783-11 gives everyone a shared vocabulary — the data dictionary
(DDIs). This chapter covers both.

## Why this exists

Precision agriculture is bookkeeping at speed. A sprayer must apply the *right rate*
at the *right place*, turn individual booms off over already-covered ground, and log
exactly what it did for compliance and analytics. None of that is possible unless
the field computer and the implement agree, to the litre and the centimetre, on:

- *what the implement is* — how many booms, sections, and tanks; what it can measure
  and control;
- *what each number means* — that "application rate" is this DDI, in these units,
  at this resolution;
- *where the implement is* — so coverage and section control line up with the map.

The TC protocol plus the DDI dictionary plus a geometry description make that
agreement machine-readable.

## The two halves

```
   TASK CONTROLLER (server)                 IMPLEMENT (TC client)
   the field computer / job log             the sprayer / seeder

        ◄──── DDOP upload ─────────────────  "here is what I am" (device desc.)
        ───── activate ───────────────────►
        ───── setpoint: rate = 200 L/ha ──►  process data DOWN (commands)
        ◄──── measured: actual = 198 ─────   process data UP (as-applied)
        ◄──── section 3 worked ───────────
```

machbus implements both: a TC **client** (the implement side) and a TC **server**
(the controller side), plus the DDOP builder, the process-data helpers, peer
control, and TC-GEO geometry/prescription conversion.

## The device description: the DDOP

Before any work, the implement uploads a **Device Object Pool** (DDOP) — a tree that
describes the machine. It is conceptually similar to a VT object pool, but instead
of describing a *screen* it describes a *machine's structure and capabilities*:

```
   Device "ACME Sprayer"
     └─ DeviceElement "boom"                (a functional part)
          ├─ DeviceProcessData  DDI=ActualRate     (a runtime value)
          ├─ DeviceProcessData  DDI=SetpointRate
          ├─ DeviceProperty     DDI=WorkingWidth    (a fixed attribute)
          └─ DeviceElement "section 1"      (offset X/Y/Z, width)
             DeviceElement "section 2"
             …
```

- **DeviceElement** — a part of the machine (the device, a boom, a section), with a
  geometry (offsets from a reference point).
- **DeviceProcessData** — a *runtime* value the TC can read or write (rate, speed,
  section state), tagged with a DDI.
- **DeviceProperty** — a *definition-time* constant (working width, capacity).

machbus builds DDOPs fluently and serializes them to the wire layout. The upload
itself rides the Transport Protocol — a DDOP is far bigger than one frame.

## The shared vocabulary: DDIs (ISO 11783-11)

A DDI — Data Dictionary Identifier — is a number that means a specific quantity, in
specific units, at a specific resolution, agreed across the whole industry. "Setpoint
volume per area application rate" is one DDI; "actual working width" is another. The
dictionary is what stops one vendor's "rate" being another vendor's "rate ÷ 10."

machbus ships the dictionary as a generated, sorted, fingerprinted table with:

- lookup by DDI number, distinguishing a *known* entry from the *unknown* sentinel;
- resolution-aware engineering conversion (raw integer ↔ real units), saturating on
  invalid input;
- explicit handling of the proprietary DDI range so custom values never collide with
  the unknown sentinel.

Crucially, the data dictionary is a *public* vocabulary; machbus references named DDI
constants (e.g. `ddi::ACTUAL_WORKING_WIDTH`) rather than magic numbers, and its
helpers carry the named references through so geometry, rate, and total semantics
stay correct. The dictionary has its own deep-dive:
[ISO 11783-11 — the data dictionary](iso11783-data-dictionary.md).

## The lifecycle

```
  DISCONNECTED
     │  who is a TC?  (TC status broadcast)
     ▼
  DISCOVERED ── learns TC address + version
     │  working-set master + version handshake
     ▼
  UPLOADING  ── transfer the DDOP (Transport Protocol)
     │  activate
     ▼
  CONNECTED  ── trade process data for the life of the task
     │           setpoints down, measurements up, section reports
     ▼
  (task ends / disconnect)
```

This mirrors the VT lifecycle deliberately — discover, upload a big description,
activate, then run — because both are "describe yourself, then cooperate" services.
machbus drives the FSM and ships the frames; you supply the DDOP and react to value
requests and commands.

## Process data in practice

Once connected, process data flows continuously:

- **Down (TC → implement):** setpoints and triggers — "set rate to 200 L/ha",
  "enable section 4".
- **Up (implement → TC):** measured values and state — "actual rate 198", "section 4
  on", "12.4 ha worked" — sometimes on change, sometimes on a time/distance trigger
  the TC requests.

The TC can ask for values on a **trigger** (every N ms, every N cm, on change, on
threshold) so the network is not flooded. machbus exposes the process-data value
helpers and the request/command path.

## Peer control and TC-GEO

Two higher-order capabilities sit on the TC foundation:

- **Peer control** — the TC can hand control of a specific element/DDI from one CF to
  another (for example, letting a guidance or rate controller drive a section
  directly). machbus models the peer-control assignment messages and surfaces them as
  events.
- **TC-GEO** — geographic/prescription work: turning a position fix plus a
  prescription map into per-section setpoints, with DDI-aware engineering conversion
  for the rates. This is where the GNSS feed (see
  [Positioning](positioning.md)) meets the DDOP geometry.

```
   prescription map  +  GNSS fix  +  DDOP geometry (section offsets/width)
            │               │                 │
            └───────────────┴─────────────────┘
                            ▼
              per-section setpoint rates (TC-GEO)
                            ▼
              process data DOWN to the implement
```

## Doing it with machbus

On the session facade, plug `TcClient` (implement) or `TcServer` (controller):

```
   Session::builder(name, addr)
       .plug(TcClient::new(config, ddop))
       .spawn(transport)
              │
              ▼  driver.poll() each cycle
       FSM: discover → announce → upload DDOP → activate → trade data
       state changes → Event::Tc(TcEvent::StateChanged(..))
```

The full hands-on path is in the
[Task Controller client tutorial](../tutorials/task-controller-client.md) and
[Task Controller server tutorial](../tutorials/task-controller-server.md); DDOP
construction in [DDOP](../tutorials/ddop.md); the geographic side in
[TC-GEO prescription](../tutorials/tc-geo-prescription.md).

## Failure modes worth knowing

- **DDOP rejected** — a structural or DDI error makes the TC refuse activation;
  validate the DDOP and watch the state machine.
- **Magic DDI numbers** — using a raw number instead of a named, dictionary-checked
  DDI is how units quietly drift; machbus's helpers guard against it.
- **Trigger storms vs starvation** — too-frequent triggers flood the bus; too-rare
  ones lose resolution in the as-applied log. Match the trigger to the work.
- **Geometry mistakes** — wrong section offsets/widths make section control and
  coverage misalign with reality even when every message is "valid."
- **Version gaps** — older TCs support fewer features; negotiate on the advertised
  version.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| TC client (implement side) | `session::plugins::TcClient` | [Task Controller client](../tutorials/task-controller-client.md) |
| TC server (controller side) | `session::plugins::TcServer` | [Task Controller server](../tutorials/task-controller-server.md) |
| Building a device description | `isobus::tc` DDOP builder | [DDOP](../tutorials/ddop.md) |
| The DDI vocabulary | `isobus::tc::ddi_database` | [ISO 11783-11](iso11783-data-dictionary.md) |
| Position → setpoint | TC-GEO helpers + `Gnss` | [TC-GEO prescription](../tutorials/tc-geo-prescription.md) |

## What this proves / does not prove

machbus implements the TC client/server, DDOP, process data, peer control, TC-GEO
conversion, and the DDI dictionary, tested locally. That proves protocol and
conversion behavior; it does **not** prove interoperability with a specific
commercial TC or AEF TC certification — see [Conformity first](../conformity/index.md).

## See also

- [The Virtual Terminal](virtual-terminal.md) — usually running alongside the TC.
- [Positioning: NMEA and GNSS](positioning.md) — the fix that feeds TC-GEO.
- [ISO 11783-11 — the data dictionary](iso11783-data-dictionary.md) — the shared DDI vocabulary.
