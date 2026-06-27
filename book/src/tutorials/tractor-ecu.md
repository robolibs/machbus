# Tractor ECU

The Tractor ECU — the TECU — is the control function that speaks *for the
tractor* on the implement bus. It is the gateway between the machine's internal
network and the implements hanging off the back (and front), and it is the node
that publishes the tractor's live state: how fast the wheels are turning, how
far the machine has travelled, where the hitch sits, whether the PTO is spinning
and how fast, the key-switch position, and the lighting state. A capable TECU
also *accepts* a bounded set of commands from an implement — move the hitch,
engage the PTO, drive an auxiliary valve — and decides, on its own terms,
whether to act on them.

This page explains what a TECU is, the "classes and facilities" idea that says
which messages a given tractor offers, the message groups `machbus` models, the
difference between publishing state and accepting commands, and how to assemble
the pieces from the real types. It assumes the node has already claimed an
address — see [Address claim](address-claim.md) first.

## Why this exists

An implement cannot do useful work in isolation. A seeder needs ground speed to
hold its seeding rate; a sprayer needs distance and direction to log coverage; a
baler needs to know the PTO is turning before it commits. None of those signals
originate on the implement bus — they live inside the tractor, on its own
network or on directly wired sensors. Something has to translate the tractor's
private telemetry into the standard messages an implement understands, present
the tractor to shared services (the virtual terminal, the task controller) as
just another node, and route the occasional command back the other way.

That something is the TECU. ISO 11783-9 (Tractor ECU) defines it as the gateway
between the tractor bus and the implement bus, and as the node that represents
the tractor to everyone else on the implement bus. ISO 11783-7 (Implement
messages) defines the actual messages it sends and receives.

## Mental model

```
        tractor side                 │   implement bus
   (internal net + sensors)          │
                                     │
  wheel/ground speed ───┐            │
  hitch position    ────┤            │   ┌──────────────┐
  PTO speed/engage  ────┼──► TECU ───┼──►│  implement   │
  key switch        ────┤   (gateway)│   │     CF       │
  lighting          ────┘            │   └──────┬───────┘
                                     │          │
  hitch / PTO / valve ◄──────────────┼──────────┘
  actuators (if it accepts commands) │   commands (hitch move,
                                     │    PTO engage, valve…)
```

Read it in two directions. Most of the traffic flows left to right: the TECU
*publishes* state on a fixed cadence so any implement that cares can subscribe.
A smaller stream flows right to left: an implement *commands* the tractor, and
the tractor chooses whether to honour it. The TECU is never obliged to obey — a
command is a request, and the tractor's own logic and interlocks have the final
say.

## The TECU's role and the facilities idea

A tractor advertises *what it can do* with two related concepts.

**Class** is a coarse capability tier. `machbus` models the base tier as
[`TecuClass`](https://docs.rs/machbus) with three values:

| Class | What it implies |
| --- | --- |
| `Class1` | Basic state: speed, hitch position, PTO, power management. |
| `Class2` | Full measurements: distance, direction, draft, lighting, aux-valve flow. |
| `Class3` | Accepts commands: hitch, PTO, and auxiliary-valve control. |

Each class is a *superset* of the one below it. A class is also a promise: a
tractor may keep a classification even when a physical feature is missing — a
tractor with no rear PTO still answers the PTO messages, but fills the values
with the "not available" sentinel rather than dropping the message.

On top of the base class sit **addenda**, single-letter flags for optional
message families. `machbus` carries them as boolean fields on
[`TecuClassification`](https://docs.rs/machbus): `navigation` (N, GNSS
position), `front_mounted` (F, front hitch/PTO), `guidance` (G, steering),
`powertrain` (P, speed control), and `motion_init` (M, motion initiation). The
`Display` impl renders the combination the way the standard writes it — a
class 2 tractor with navigation and a front hitch prints as `Class 2NF`, and
the addendum order is fixed at N, F, G, P, M.

The fine-grained answer to "which exact messages do you offer?" is the
**tractor facilities response**. `machbus` models it as
[`TractorFacilities`](https://docs.rs/machbus): a flat struct of booleans, one
per feature, that packs into an eight-byte payload via `encode` and parses back
with `decode`. Two PGNs share that payload, distinguished by
[`TractorFacilitiesRole`](https://docs.rs/machbus):

- `Response` — the TECU broadcasting what it actually supports.
- `Required` — an implement telling the TECU which facilities it needs, so the
  TECU can suppress the messages nobody is listening for and save bandwidth.

Convenience builders fill whole tiers at once: `with_class1_all`,
`with_class2_all`, `with_class3_all`, plus `with_class3_v2_all` and
`with_front_v2_all` for the later limit-status and exit-code bits.

## Anatomy of the message groups

The TECU's published state breaks into a handful of families, each with its own
`machbus` codec. Every codec is a plain struct with `encode` → `[u8; 8]` and a
fallible `decode`. The decoders reject the wrong length, bad padding, and
reserved-bit violations rather than guessing — malformed input returns `None`.

**Ground- and wheel-based speed and distance.** A tractor has more than one
notion of speed. *Wheel-based* speed comes from the drivetrain and can slip;
*ground-based* speed (often radar) is closer to true travel. `machbus` keeps
them as separate codecs, [`WheelBasedSpeedDist`](https://docs.rs/machbus) and
[`GroundBasedSpeedDist`](https://docs.rs/machbus). Both carry `speed_mps`,
accumulated `distance_m`, and a `direction` ([`MachineDirection`]). The
wheel-based message additionally carries `max_power_time_min`, `key_switch_state`,
and the implement start/stop and operator-direction-reversed states — it is the
message a maintain-power requester keys off at power-down. There is also
[`MachineSelectedSpeedFull`](https://docs.rs/machbus): the speed the tractor is
actually steering by, tagged with its [`SpeedSource`] (wheel, ground, navigation,
or blended) and a limit status.

**Rear and front hitch position.** [`HitchStatus`](https://docs.rs/machbus)
reports `position_percent`, an `in_work_indication`, a `limit_status`
([`LimitStatus`]) and `exit_code` ([`ExitReasonCode`]), and `draft_force_n` for
class 2 tractors. The same struct serves both ends: the `is_rear` flag picks the
rear or front PGN, and `pgn()` returns the right one.

**PTO state and speed.** [`PtoStatus`](https://docs.rs/machbus) reports
`shaft_speed_rpm`, an `engagement` state, an `economy_mode`, plus the limit and
exit codes. As with the hitch, `is_rear` selects rear or front and `pgn()`
resolves it.

**Operator, key, and lighting.** The key-switch state and maximum power-on time
ride along in the wheel-based speed message. Lighting is carried by its own
state type in the implement modules; the function-instance-0 TECU is the node
responsible for lighting control.

**Key and maintain-power.** Power management is its own concern, covered in the
[lifecycle](#lifecycle-power-and-shutdown) section below.

## Publishing state vs accepting commands

This distinction is the heart of the TECU and worth stating plainly.

**Publishing** is unconditional and periodic. The TECU sends speed, distance,
hitch, and PTO messages on a fixed cadence whether or not anyone is listening
(subject to the bandwidth-saving suppression a *required facilities* message can
request). These are *status* and *measured* values — they report what is, and
carry no expectation of a reply.

**Accepting commands** is conditional and discretionary. A class 3 (or
addendum-P) tractor receives command messages from an implement and *may* act on
them. `machbus` models the inbound commands as separate codecs:
[`HitchCommandMsg`](https://docs.rs/machbus) (with a
[`HitchCommand`] of `NoAction`/`Lower`/`Raise`/`Position`),
[`PtoCommandMsg`](https://docs.rs/machbus) (with a [`PtoCommand`] of
`NoAction`/`Engage`/`Disengage`/`SetSpeed`),
[`AuxValveCommandMsg`](https://docs.rs/machbus) (with a [`ValveCommand`] of
`Extend`/`Retract`/`Float`/`Block`, PGN-routed per valve index by `pgn()` /
`try_pgn()`), [`MachineSpeedCommandMsg`](https://docs.rs/machbus) for a target
travel speed, and [`TractorControlModeMsg`](https://docs.rs/machbus) carrying the
per-axis [`TractorMode`] (manual / automatic). The standard is explicit that the
tractor is not required to execute any command — it applies its own logic and
constraints and may negative-acknowledge. `machbus` gives you the wire codecs;
the accept/reject decision is yours.

## Lifecycle: power and shutdown

A TECU's other major job is power management. `machbus` models the lifecycle as
[`PowerState`](https://docs.rs/machbus):

| State | Meaning |
| --- | --- |
| `PowerOff` | The boot/default state; nothing powered. |
| `IgnitionOn` | Normal operation, key on. |
| `ShutdownInitiated` | Key off; a bounded window of power remains. |
| `FinalShutdown` | Power-down complete. |

The interesting transition is key-off. When the operator turns the key off, the
tractor does not cut power instantly — implements may need a moment to save
settings or move actuators to a safe rest position. So the TECU keeps power up
for a bounded window and broadcasts the remaining time. An implement that needs
more time sends a *maintain power* request asking the TECU to hold ECU power, or
both ECU power and actuator power, a little longer. The timing and current
limits live in [`PowerConfig`](https://docs.rs/machbus) — `shutdown_max_time_ms`
(default three minutes), `maintain_timeout_ms` (default two seconds), and the
ECU/PWR current minimums. The TECU side of a single request is tracked by
[`TecuMaintainPowerRequest`](https://docs.rs/machbus), whose `is_expired` tells
you when a requester's hold has lapsed. [`SafeModeTrigger`](https://docs.rs/machbus)
enumerates the reasons a node might fall back to fail-safe behaviour — power
loss, ECU-power loss, CAN failure, lost communication with the TECU, or a manual
trigger.

`machbus` deliberately ships the building blocks rather than one monolithic
orchestrator: you compose the classification, the facilities response, the
status codecs, and the power state machine into whatever control loop your
product needs. [`TecuConfig`](https://docs.rs/machbus) is a convenience record
bundling the classification, power config, and broadcast intervals.

## Doing it with machbus

The example builds a classification, prints it, and walks the power states. The
classification piece:

```rust
{{#include ../../../examples/tractor_ecu_demo.rs:9:25}}
```

`TecuClassification` is a plain struct of fields, so you set the base class and
flip on the addenda you support. Printing it yields the canonical class string
(`Class 2NF` here). The power-state walk:

```rust
{{#include ../../../examples/tractor_ecu_demo.rs:27:37}}
```

To advertise facilities, build a `TractorFacilities`, fill the tiers you offer,
and encode it onto the response PGN — the shape is:

```rust
// Illustrative shape, not a compiled call.
let facilities = TractorFacilities::default()
    .with_class1_all()
    .with_class2_all();
let payload = facilities.encode();           // [u8; 8]
let pgn = TractorFacilitiesRole::Response.pgn();
// send `payload` on `pgn` …
```

To publish wheel-based speed, fill the struct in SI units and `encode`:

```rust
// Illustrative shape, not a compiled call.
let msg = WheelBasedSpeedDist {
    speed_mps: 3.2,
    distance_m: 1234.5,
    direction: MachineDirection::Forward,
    ..Default::default()
};
let payload = msg.encode();                  // [u8; 8]
```

Inbound commands run the same way in reverse: `HitchCommandMsg::decode(&data)`
returns `Some(cmd)` on a well-formed payload and `None` otherwise. You inspect
`cmd.command`, decide whether your interlocks permit it, and only then drive the
actuator.

## Events and responsibilities

Whatever orchestration you build around these codecs, the responsibilities are
yours:

| Event | Typical TECU responsibility |
| --- | --- |
| Power-up | Claim an address, then broadcast the facilities response. |
| Required-facilities received | Narrow what you broadcast to what implements asked for. |
| Periodic tick | Re-send speed/distance/hitch/PTO status at the right cadence. |
| Command received | Validate against interlocks; act or negative-acknowledge. |
| Key-off | Enter the shutdown window; honour maintain-power requests until they expire. |
| Sensor unavailable | Send the message with the "not available" sentinel, not silence. |

The non-negotiable one: a command is never an obligation. The library hands you
a decoded request; your application decides whether moving that actuator is safe.

## Edge cases and failures

- **Sensor not available.** A feature the tractor lacks (no front PTO, no
  ground-radar) is reported, not omitted. The default constructors seed the
  "not available" sentinels — `0xFF` bytes, the `NotAvailable` enum variants —
  so an unconfigured `PtoStatus` or `HitchStatus` already says "I don't have
  this" on the wire. Keep advertising the message; just mark it unavailable.
- **Command for a facility you don't offer.** If your class or facilities bits
  don't include a command family, an implement should not send it — but if one
  arrives, decline it. There is no requirement to act, and acting on a command
  for hardware you lack is a fault.
- **Malformed payload.** Every `decode` is defensive: wrong length, bad reserved
  bits, or corrupt padding yield `None`. A peer's garbage frame returns `None`
  and must not be allowed to mutate your cached state.
- **Speed-source switching under control.** When the tractor is steering by
  `MachineSelectedSpeedFull` and the underlying source changes, the transition
  is the tractor's problem to smooth — the message reports the source so
  consumers can see it shift.
- **Safe defaults at power loss or comms loss.** On lost power or lost
  communication with the tractor, the implement is expected to assume fail-safe
  operation. `SafeModeTrigger` names the causes; the actual safe state belongs
  in the implement's product logic, not in this library.

## Advanced

- **Required vs optional facilities.** Class 1 is the floor; class 2 adds the
  measurement set; class 3 adds the command set. Addenda (N/F/G/P/M) are
  independent options layered on any base class. Advertise exactly what you
  support — over-claiming invites commands you cannot honour.
- **Version-2 limit and exit reporting.** Newer tractors add limit-status and
  exit-reason reporting so an implement learns *why* a command was constrained
  or aborted. `machbus` carries these as the `*_limit_status` and `*_exit_code`
  facility bits and the `LimitStatus` / `ExitReasonCode` fields on the status
  codecs; `with_class3_v2_all` and `with_front_v2_all` set the facility bits.
- **Multiple TECUs and function instance.** When several TECUs share a bus, the
  one at function instance 0 is primary and owns power, lighting, and language;
  higher instances fill only the gaps. `TecuClassification::instance` records
  which you are.
- **Repetition and update timing.** Status messages are periodic; the speed
  message in particular runs on a tight cadence. `TecuConfig` carries
  `facilities_broadcast_interval_ms` and `status_broadcast_interval_ms` as a
  place to record your chosen rates — the library does not run the timer for
  you, so wire it into your own loop.

## Validate locally

```sh
make run EXAMPLE=tractor_ecu_demo
make test
```

The example constructs a `Class 2NF` classification, prints the class strings
for all three base classes, and walks the four power states from the boot
default. The wide test suite round-trips every codec on this page — speed,
distance, hitch, PTO, facilities, and each command family — and asserts that
malformed payloads decode to `None`. To wire these surfaces together with
address claim and GNSS, plug them into a session — the `presets::tractor()` group
bundles the usual tractor-side subsystems (see
[The session facade](../guide/session-facade.md)).

## What this proves / does not prove

Proves: the classification renders the right class string, the facilities and
status/command codecs round-trip their eight-byte payloads, the decoders reject
malformed input, and the power-state and maintain-power types behave as
specified in software.

Does not prove: real-hardware timing, that a specific third-party implement
interoperates with your TECU, or any conformance or certification claim. A
shipping TECU still needs official standards, real hardware, interoperability
evidence, and — above all — machine-control interlocks that this library does
not provide.

## See also

- [Implement ECU](implement-ecu.md) — the other side of every message here.
- [Powertrain](powertrain.md) — the P-addendum speed-control surface in depth.
- [Guidance](guidance.md) — the G-addendum steering surface.
- [TIM and automation](tim.md) — implement-driven tractor automation.
- [Address claim](address-claim.md) — the prerequisite every TECU runs first.
