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

### Steering, speed and motion are three separate classes

Base class 1/2/3 says nothing about whether a tractor can be *driven* over the
bus. That is carried by letter addenda, and §4.4.2 gives them separately:

| Addendum | Clause | Meaning |
| --- | --- | --- |
| **G** | §4.4.2.7 | "shall support the external control of the guidance system" — curvature command, estimated curvature, readiness, lockout |
| **P** | §4.4.2.8 | "capable of accepting speed and/or drive strategy commands from an implement controller" |
| **M** | §4.4.2.9 | accepts "commands to initiate motion of the vehicle (forward or reverse)" |

A class 2G tractor steers on command and need not accept a speed command at all.
Even with P, §4.4.2.8 says outright that bringing the tractor to a stop
(speed `0.0`) is **optional** and "can be determined by an implement via the
tractor facilities response message".

### The handshake is not optional for the implement either

Two PGNs, and the second one is the part that surprises people:

- **0xFE09 Tractor Facilities Response** — what the TECU has installed.
- **0xFE0A Required Tractor Facilities** — what the implement needs.

> §4.4.2: "An implement CF can send the required tractor facilities message to
> the Tractor ECU **to enable the transmission of the messages that provide the
> required facilities**. A facility is not required if its corresponding bits are
> set to 0 in the implement CF required tractor facilities message. The Tractor
> ECU can then **stop the transmission** of this implement message to reduce
> bandwidth."

So a node that never declares what it needs may simply never receive it. That is
why [`AutoDrive`](../tutorials/autodrive.md#the-tractor-has-to-advertise-the-facility-first)
broadcasts the request on a cycle rather than only listening.

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
