# TC-GEO prescription

A prescription map ties a target application rate to a position in the field.
The agronomy decides "apply 200 here, 100 there, nothing in the wet hollow",
and the machine carries that decision out as it drives: it reads its own GNSS
position, looks up the rate for *where it is right now*, and feeds that rate to
the implement as a setpoint. This is variable-rate application, and on ISOBUS it
is the position-based ("TC-GEO") side of task control. This tutorial explains
why position-based control exists, how a prescription map is shaped in
`machbus`, how a position becomes a rate, and how that rate becomes a
process-data value the implement can act on.

It assumes you already understand process data and DDIs from
[DDOP and process data](../standards/task-controller.md), and that a
[Task Controller client](task-controller-client.md) connection is what carries
the setpoints. TC-GEO sits on top of both.

## Why this exists

A field is not uniform. Soil type, yield history, weed pressure, and moisture
all vary across a single block, and the agronomically correct dose of seed,
fertiliser, or spray varies with them. Applying one flat rate everywhere either
over-applies where less is needed (cost, runoff, lodging) or under-applies where
more is needed (lost yield). The fix is to pre-compute a *prescription* — a map
that says what rate belongs at each part of the field — and let the machine
follow it automatically instead of asking the operator to ride a knob.

Position-based control exists so the task controller can do this lookup
continuously. As the machine moves, the controller resolves the current position
to a target rate and pushes it to the implement, which adjusts its metering in
real time. The operator drives; the map does the dosing.

## Mental model

Picture a field split into zones, each tagged with a target rate, plus a default
for anywhere not covered. The machine knows its own position from GNSS and asks
one question over and over: *which zone am I in, and what rate does it want?*

```
        lon →
   +-------------------------------+
   |        zone B  (rate 200)     |
   |   +-----------------------+   |
   |   |                       |   |
   |   |   ● machine here      |   |   position ──► point-in-zone test
   |   |     (GNSS fix)        |   |               │
   |   +-----------------------+   |               ▼
   |        zone A  (rate 100)     |        in zone B → rate 200
   |          +-------+            |               │
   |          | hole  | no-apply  |               ▼
   |          +-------+            |        rate → process-data setpoint
   +-------------------------------+               │
              outside → default rate               ▼
                                              implement meters 200
```

The loop is: **fix → zone → rate → setpoint**. Everything else (freshness
checks, defaults, smoothing) hangs off that loop.

## Anatomy: the pieces in machbus

`machbus` models the map with three plain types in the TC-GEO module, plus the
interface that ties them to a live position.

| Type | What it holds |
| --- | --- |
| `Wgs` | A WGS84 latitude/longitude/altitude triple (re-exported from `concord`). Positions and zone vertices are both `Wgs`. |
| `GeoPoint` | A `Wgs` `position` plus a `timestamp_us`. This is a *timestamped* fix, so you can reason about how fresh it is. |
| `PrescriptionZone` | A `boundary` (a `Vec<Wgs>` polygon), optional `holes` (a `Vec<Vec<Wgs>>` of exclusion polygons), and an `application_rate` (an `i32` in DDI-dependent units). |
| `PrescriptionMap` | A `structure_label` (a `String` name) and its `zones` (a `Vec<PrescriptionZone>`). |
| `TCGEOInterface` | Holds the loaded maps and the current position, performs lookups, and raises events. |

A zone is a polygon, not a rectangle: irregular field shapes are expressed as a
ring of vertices. A `hole` is a polygon *inside* the boundary that must not be
treated — a pond, a power-line apron, a buffer strip. The rate is a bare
integer; its meaning (litres per hectare, kilograms per hectare, and so on) is
fixed by the rate DDI you pair it with when you encode the setpoint, not by the
zone itself.

There is no separate "default rate" field on the map. A position that matches no
zone returns *no rate* (`None`), and your application decides what "no zone"
means — typically a safe default of "apply nothing" or a configured fallback.
Keeping the default in application code, rather than baking it into the map,
makes the safe behaviour explicit at the call site.

## Position → zone → rate

The lookup is a point-in-polygon test. `point_in_prescription_zone(point, zone)`
returns whether a `Wgs` point lies inside a zone's boundary *and* outside every
one of its holes. The outer boundary is inclusive (a point exactly on an edge
counts as inside); hole boundaries are conservative exclusions (a point on a hole
edge counts as *not* applied), which is the safe choice for a no-apply region.

`TCGEOInterface::get_rate_at_position(pos)` walks every loaded map and every zone
in order and returns the `application_rate` of the **first** zone that contains
the point, or `None` if none do. "First match wins" is what makes overlapping
zones resolve deterministically — there is no averaging or precedence beyond
declaration order, so order your zones intentionally.

Degenerate input is rejected rather than guessed at: a polygon with fewer than
three vertices, a zero-area polygon, or any non-finite coordinate makes the
point-in-polygon test return `false`. That keeps a malformed map from silently
matching everywhere.

## Rate → process-data setpoint

A looked-up rate is just a number until it is wrapped as a process-data value the
implement understands. TC-GEO speaks the same language as the rest of task
control: an ECU-to-TC process-data payload keyed by a DDI.

Two paths take you from position to wire:

- `get_rate_at_position_engineering(pos, ddi)` looks up the raw rate and converts
  it to the engineering unit defined by a rate DDI (applying that DDI's
  resolution/scale), returning `Ok(Some(value))`, `Ok(None)` for no match, or an
  error if the DDI is unknown, is not a rate DDI, or the value is out of range.
- `rate_process_data_payload_at_position(pos, ddi)` does the same lookup and
  hands back an 8-byte `PGN_ECU_TO_TC` Process Data Value payload ready to ship,
  again `Some`/`None`/error.

The free functions `prescription_rate_from_engineering`,
`prescription_rate_to_engineering`, and `prescription_rate_process_data_payload`
expose the conversion on its own, so you can validate a rate against a DDI's
range before it ever touches a zone. All of them refuse a DDI that is not an
application-rate DDI, which stops you from, say, shipping a fertiliser quantity
under a latitude DDI.

The position itself is also reportable. `position_process_data_payloads()`
returns the two 8-byte payloads (actual latitude, then actual longitude) for the
machine to send back to the TC, so the controller knows where the reported rate
was applied. It errors if no position has been recorded yet.

## GNSS freshness and stale positions

Position-based control is only as trustworthy as the fix behind it. A
`GeoPoint` carries a `timestamp_us` precisely so your loop can ask "how old is
this fix?" before acting on it. If the fix is stale — the receiver dropped to a
degraded mode, lost lock, or the feed stalled — the rate you would compute is for
where the machine *was*, not where it *is*.

The decode path is already defensive. `try_handle_gnss_position` (and its
ignore-errors wrapper `handle_gnss_position`) rejects the not-available sentinel,
out-of-range coordinates, wrong PGNs, invalid source addresses, and
non-canonical lengths, and on rejection it leaves the previously cached position
**unchanged** rather than overwriting it with garbage. That guards against a
single bad frame, but it does not by itself notice a fix that is simply *old*.
That freshness check is yours: compare `current_position().timestamp_us` against
your clock, and when the gap exceeds your tolerance, fall back to a safe default
rate (usually "apply nothing") rather than dosing against a stale position.

## Doing it with machbus

The example builds a two-zone map, drives a short trajectory across it, and
prints the rate at each step. First, the map — two square zones, one at rate 100
to the south and one at rate 200 to the north:

```rust
{{#include ../../../examples/tc_geo_demo.rs:17:41}}
```

Then it subscribes to rate changes and walks four positions — into the south
zone, onto the shared boundary, into the north zone, and off the map entirely —
looking up both the raw rate and the engineering value at each:

```rust
{{#include ../../../examples/tc_geo_demo.rs:53:77}}
```

The "outside" point returns `None` from `get_rate_at_position`, which is the
no-zone case your default policy handles. Finally it records a position and emits
the two position process-data payloads for the controller:

```rust
{{#include ../../../examples/tc_geo_demo.rs:79:90}}
```

The whole flow runs in software with no bus: you load a map, set a position,
call `update`, and read the rate.

## Events and responsibilities

`TCGEOInterface` raises three events you can subscribe to:

| Event | Fires when | Typical action |
| --- | --- | --- |
| `on_position_update` | A new position is recorded (via `set_position` or a decoded GNSS frame). | Note freshness; trigger a re-evaluation. |
| `on_application_rate_changed` | `update` runs and the looked-up rate differs from the last one. | Send the new setpoint to the implement. |
| `on_prescription_map_received` | A map is added with `add_prescription_map`. | Log/validate the new map; reset cached rate state. |

`update(elapsed_ms)` is the heartbeat: it re-evaluates the current position
against the maps and fires `on_application_rate_changed` *only* when the rate
actually changes, so identical re-evaluations do not spam setpoints. Your
responsibilities are the parts the interface deliberately leaves to you: decide
the default for no-zone, decide the staleness tolerance, and gate setpoints on a
valid, fresh fix.

## Edge cases and failures

- **Point outside all zones.** `get_rate_at_position` returns `None`. Treat this
  as "apply the safe default", not "keep the last rate" — driving off the map
  should not silently freeze the dose.
- **Overlapping zones.** Resolution is deterministic by declaration order: the
  first zone whose polygon contains the point wins. If two zones overlap, the one
  you listed first decides the rate. Order zones with this in mind.
- **Holes and buffer strips.** A point inside a zone but inside one of its holes
  is *not applied* (`None` from that zone). Points on a hole edge are excluded
  too, which is the conservative choice for no-spray regions.
- **Stale or lost position.** The decoder keeps the last good fix on a bad frame,
  but a fix can still go old. Check `timestamp_us` and fall back to a safe rate
  when it ages past tolerance.
- **Coordinate precision.** Positions are decoded at roughly 1e-7 degree
  resolution and zone vertices are `f64`. Near a boundary, two nearby fixes can
  land on opposite sides — see boundary jitter below.
- **Zone boundary jitter.** Around a shared edge, small position noise can flip
  the matched zone back and forth, causing rapid rate toggling. Because the
  boundary is inclusive and overlaps resolve by order, you can damp this in
  application code (hysteresis, or a small dwell before acting on a change)
  rather than expecting the lookup to smooth it for you.

## Advanced

- **Large maps and performance.** The lookup is linear: every map, every zone,
  every edge, until a match. For a field with many fine zones, pre-filter by a
  bounding box per zone before the full polygon test, or index zones spatially,
  so each fix touches only nearby candidates rather than the whole map.
- **Smoothing and look-ahead.** Real implements have application latency — the
  metering reacts a moment after the command. Production systems look the rate up
  slightly *ahead* of the current position along the travel direction so the
  change lands at the right ground point, and smooth transitions across a
  boundary instead of stepping instantly.
- **Integration with section control.** TC-GEO answers "what rate here"; section
  control answers "which sections are on here". The two compose: a section that
  is switched off in an exclusion area should not apply even where the
  prescription rate is non-zero, and a hole in a prescription zone is one way to
  express that no-apply region. Keep the rate decision and the on/off decision as
  separate inputs to the implement.
- **Surface vs low-level.** `TCGEOInterface` is the pump-style core: you feed it
  positions and maps and pull payloads and events out. It does not open a bus or
  run a TC connection on its own — pair it with a
  [Task Controller client](task-controller-client.md) to actually carry the
  setpoints.

## Validate locally

```sh
make run EXAMPLE=tc_geo_demo
make test
```

The example loads a two-zone map and prints the raw and engineering rates at
four positions, including the no-zone case off the map, and the rate-change
events observed along the way. `make test` exercises the point-in-polygon test,
zone walking, hole exclusion, overlap ordering, the GNSS decode guards, and the
rate/DDI conversions.

## What this proves / does not prove

Proves: in software, a position resolves to the rate of the first containing
zone (holes excluded), the decoder rejects malformed or out-of-range fixes
without corrupting the cached position, and rates convert to and from
process-data payloads under their rate DDIs deterministically.

Does not prove: real-hardware GNSS accuracy or latency, interoperability with a
specific third-party task controller or implement ECU, correct application
look-ahead on a moving machine, or any conformance/certification claim. Those
still require official standards, real hardware, and interoperability evidence.

## See also

- [Task Controller client](task-controller-client.md) — the connection that
  carries the setpoints TC-GEO produces.
- [DDOP and process data](../standards/task-controller.md) — DDIs,
  value presentation, and the process-data form the rate is encoded into.
- [NMEA 2000](nmea-2000.md) and [Serial GNSS](serial-gnss.md) — where the
  position fixes come from.
