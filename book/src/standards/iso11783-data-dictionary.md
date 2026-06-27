# ISO 11783-11 — the data dictionary

The Task Controller can only trade numbers if everyone agrees what each number *means*.
Part 11 is that agreement: a public dictionary of **DDIs** — Data Dictionary
Identifiers — each pinning a quantity to specific units and resolution. It is the
vocabulary that makes one vendor's "application rate" equal another vendor's.

## Why this exists

"Rate = 200" is meaningless without units and scale. Is that litres per hectare, or
kilograms, or millilitres per square metre? At what resolution? Part 11 removes the
ambiguity: a DDI number *is* the definition. Pick the DDI for "setpoint volume per area
application rate" and both ends know the units and the integer-to-real conversion.

## What a DDI carries

```
   DDI 0x0001 ─► "Setpoint Volume per Area Application Rate"
                  unit: mm³/m²   ·   resolution: 0.01   ·   range: …
   DDI 0x0043 ─► "Actual Working Width"  ·  unit: mm  ·  resolution: 1
   …                                      (a public, industry-wide table)
```

Two ranges matter:

- the **standard range** — the published dictionary everyone shares;
- the **proprietary range** — vendor-private DDIs, which must never collide with the
  *unknown* sentinel the lookup returns for unrecognized numbers.

## How machbus expresses it

machbus ships the dictionary as a generated, **sorted, fingerprinted** table with:

- **lookup** by number, distinguishing a known entry from the unknown sentinel;
- **engineering conversion** — raw integer ↔ real units using the entry's resolution,
  saturating cleanly on invalid input;
- **proprietary-range handling** so custom DDIs never alias the unknown sentinel.

Crucially, the code refers to **named DDI constants** (e.g. `ddi::ACTUAL_WORKING_WIDTH`)
rather than magic numbers, and the DDOP/process-data helpers carry those named
references through so geometry, rate, and total semantics stay correct end to end.

```
   raw 12_345  ── ddi_to_engineering(DDI, raw) ──►  123.45 (real units)
   123.45      ── ddi_from_engineering(DDI, x) ──►  12_345 (raw, saturating)
```

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| DDI lookup + conversion | `isobus::tc::ddi_database` (`ddi::*` constants) | [DDOP](../tutorials/ddop.md) |
| Using DDIs in a DDOP | `isobus::tc` DDOP builder | [The Task Controller](task-controller.md) |
| Geographic rate conversion | TC-GEO helpers | [TC-GEO prescription](../tutorials/tc-geo-prescription.md) |

## Failure modes worth knowing

- **Magic numbers** — a raw DDI literal instead of a named, dictionary-checked constant
  is how units silently drift.
- **Resolution mistakes** — applying the wrong scale turns 200 L/ha into 20 or 2000.
- **Proprietary collisions** — a custom DDI that overlaps the unknown sentinel becomes
  invisible.

## See also

- [The Task Controller](task-controller.md) — the protocol that uses this vocabulary.
