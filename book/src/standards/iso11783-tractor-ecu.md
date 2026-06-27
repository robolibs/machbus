# ISO 11783-9 — the tractor ECU

Part 9 defines the **Tractor ECU (TECU)** — the tractor's representative on the bus —
and, crucially, its *classes*. A tractor advertises which **facilities** it offers and
at what class level, so an implement knows up front whether the tractor can do what it
needs before it asks.

## Why this exists

An implement that needs ground speed, rear-hitch control, and guidance readiness must
not blindly send commands and hope. Part 9 makes the tractor publish a *contract of
capability*: "here is what I provide, at this class." The implement reads it and adapts
— or warns the operator that this tractor cannot run this implement fully.

## Facilities and classes

```
   TECU advertisement ──► "I provide:
                            • ground + wheel speed
                            • rear hitch control
                            • rear PTO
                            • guidance readiness"
                              ▲
   implement reads it, then only requests facilities the tractor actually offers
```

Higher tractor classes provide more facilities (more speed signals, more hitch/PTO
control, guidance). machbus models the facility set, the class matrix, and the
`tractor()` preset plus the `Implement` facilities broadcast that advertises them;
maintain-power and guidance-readiness sit here too.

## The TECU's other duties

Beyond the advertisement, a TECU is the source of the part-7 status broadcasts (speed,
hitch, PTO) and the relay for some tractor-side services. In machbus a tractor node is
typically the `Implement` plugin (for the status/command messages) plus the facilities
advertisement, optionally `MaintainPower` and guidance.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Advertising facilities | `session::presets::tractor()` + `Implement` | [Tractor ECU](../tutorials/tractor-ecu.md) |
| The status broadcasts | `session::plugins::Implement` | [Implement ECU](../tutorials/implement-ecu.md) |
| Keeping power after key-off | `session::plugins::MaintainPower` | [The session facade](../guide/session-facade.md) |
| A curated tractor node | `session::presets::tractor()` | [The session facade](../guide/session-facade.md#presets-personas) |

## See also

- [ISO 11783-7 — implement messages](iso11783-implement-messages.md) — the signals the TECU
  produces.
- [TIM (AEF)](tim.md) — how an implement borrows the tractor's facilities under
  authority.
