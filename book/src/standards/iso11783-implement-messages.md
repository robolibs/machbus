# ISO 11783-7 — implement messages

Part 7 is the vocabulary of physical machine control: the frames that report and
command hitches, power take-offs, auxiliary valves, ground/wheel speed, distance, and
lighting. These are the network's steady heartbeat of "what the iron is doing" and
"what I want it to do."

## Why this exists

A tractor and an implement must continuously agree on the physical situation: how fast
the ground is moving, where the hitch is, whether the PTO is turning, which booms are
on. Part 7 standardizes those signals so a rate controller, a guidance system, or a
section controller from any vendor reads the same numbers.

## The message families

```
   TRACTOR  ── wheel-based speed + distance ──►  bus      (Class 1+)
   TRACTOR  ── ground-based speed (radar) ────►  bus      (Class 2+)
   TRACTOR  ── machine-selected speed ────────►  bus
   TRACTOR  ── front/rear hitch status ───────►  bus
   TRACTOR  ── front/rear PTO status ─────────►  bus
   IMPLEMENT ── aux-valve command ────────────►  tractor
   either   ── lighting command / data ───────►  bus
```

## Two ideas to hold

**Speed has provenance.** Wheel-based, ground-based (radar), and machine-selected
speed are *different* signals with different trust. A controller must choose the right
one — radar for true ground speed, wheel for driveline. machbus decodes each with
`0xFF`-tail "not available" handling, and offers a wheel-slip helper derived from
wheel vs ground speed.

```
   wheel speed 10.0 km/h ─┐
   ground speed 9.2 km/h ─┴─► slip = (10.0 − 9.2)/10.0 = 8%
```

**Status and command are symmetric.** The same structures describe "the hitch is at
62%" and "move the hitch to 80%"; front vs rear is carried by which PGN delivered it.
machbus's `Implement` plugin decodes the status/command families into a cache plus
events, and offers `broadcast_*` / `command_*` helpers.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Hitch / PTO / aux / speed / lighting | `session::plugins::Implement` (`isobus::implement`) | [Implement ECU](../tutorials/implement-ecu.md) |
| Engine/transmission alongside | `session::plugins::Powertrain` | [Powertrain](../tutorials/powertrain.md) |
| Commanding the tractor under authority | `session::plugins::Tim` | [TIM (AEF)](tim.md) |

## Failure modes worth knowing

- **Wrong speed source** — using wheel speed where radar ground speed is meant skews
  rate and coverage.
- **Front/rear mix-up** — acting on the wrong hitch/PTO because the PGN was misread.
- **Ignoring "not available"** — treating a `0xFF`-filled signal as a real zero.

## See also

- [ISO 11783-9 — the tractor ECU](iso11783-tractor-ecu.md) — what the tractor advertises
  it can do.
- [TIM (AEF)](tim.md) — guarded command of the tractor by the implement.
