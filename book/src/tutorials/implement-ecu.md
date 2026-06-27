# Implement ECU

An implement ECU is the brain of the towed, mounted, or self-propelled machine
behind the tractor: the sprayer, seeder, baler, or mower. On the bus it plays a
very different role from the tractor. It mostly **consumes** tractor state —
ground speed, distance travelled, hitch position, PTO speed — and reacts to it,
while **publishing** its own status and, when the operator drives it through a
UI, asking the tractor to do limited things on its behalf. This tutorial shows
how the implement sees the bus, what message groups it cares about, and how to
drive all of it through `machbus`.

If you have read the [Tractor ECU](tractor-ecu.md) tutorial, this is the mirror
image: the same hitch/PTO/speed/lighting messages, viewed from the consumer
side instead of the producer side. The two pages share one set of `machbus`
codecs under `isobus::implement`, used in opposite directions.

## Why this exists

The implement and the tractor are built by different manufacturers, yet they
have to cooperate in real time in a field. The implement needs to know how fast
the ground is moving to meter product correctly, when the hitch is raised so it
can stop spraying on a headland, and whether the PTO is turning before it
expects power. None of that works if every pairing needs a custom wire harness.
ISO 11783-7 (Implement Messages) gives the implement a fixed vocabulary for
reading tractor state and, where the tractor allows it, for requesting limited
actions — so any compliant implement can ride behind any compliant tractor.

The safety posture is built into the design: automatic control of an implement
from the bus is something you do deliberately and carefully, never as a default.
The implement may *ask*; the tractor decides whether to honour the request.

## Mental model

```
        TRACTOR ECU                          IMPLEMENT ECU
   ┌──────────────────┐                  ┌──────────────────┐
   │ broadcasts state │  ── speed ────►  │ meters product   │
   │  speed/distance  │  ── distance ─►  │ to ground speed  │
   │  hitch / PTO     │  ── hitch ────►  │ lifts on headland│
   │  lighting        │  ── PTO ──────►  │ reacts to power  │
   │                  │                  │                  │
   │ may accept       │  ◄── hitch cmd   │ requests actions │
   │ commands         │  ◄── PTO cmd     │ (if allowed)     │
   │ (Class 3 / TIM)  │  ◄── aux valve   │                  │
   └──────────────────┘                  └────────┬─────────┘
                                                  │
                                      also speaks │ VT (UI)
                                                  │ TC (process data)
                                                  ▼
                                          operator + agronomic record
```

The implement's day job is to listen. It pulls a steady stream of tractor status
frames off the bus, caches the latest of each, and feeds them into its control
logic. Talking back — commanding the hitch, the PTO, or a hydraulic valve — is
the exception, and only works when the tractor advertises that it accepts those
commands.

## The implement's view of the bus

The implement is rarely alone on the wire. It typically holds three
conversations at once:

| Conversation | Direction | Purpose |
| --- | --- | --- |
| Implement messages (this page) | mostly tractor → implement | Read ground speed, distance, hitch, PTO, lighting; optionally command them. |
| [Virtual terminal](virtual-terminal-client.md) | implement ↔ VT | Draw the operator UI and receive button/soft-key input. |
| [Task controller](task-controller-client.md) | implement ↔ TC | Report and log process data (rates, as-applied, section state). |

The implement messages give it *machine reality* — how fast, how far, hitch up
or down. The VT gives it the *operator*. The TC gives it the *agronomic record*.
A real seeding or spraying control loop closes across all three: the speed off
the implement-message stream sets the metering rate; the section state comes
from the operator (VT) or an automation (TC); and the as-applied result goes
back to the TC for logging.

## Anatomy: the implement-side message groups

`machbus` exposes each group as a small codec struct or enum under
`isobus::implement`, re-exported at the module root. None of them embed the
network — you compose them through the `Implement` plugin (below) or with raw
`IsoNet` sends. The names below are the actual `machbus` types.

### Speed and distance (the implement reads these)

The core of "machine reality". The tractor broadcasts speed and accumulated
distance; the implement decodes and caches them.

| Type | What it carries |
| --- | --- |
| `WheelBasedSpeedDist` | Speed from the driveline (`speed_mps`), accumulated `distance_m`, travel `direction`, key-switch and start/stop state. Basic, available from most tractors. |
| `GroundBasedSpeedDist` | Speed/distance from a ground sensor (radar/GNSS), independent of wheel slip. Richer tractors only. |
| `MachineSelectedSpeedFull` | The tractor's single "this is the speed to use" value plus its source and an exit/limit reason code. |

`speed_mps` and `distance_m` are `f64` in SI units; the codec handles the wire
scaling. `direction` is a `MachineDirection` (`Forward`, `Reverse`, `Error`,
`NotAvailable`). `MachineSelectedSpeedFull` tells you *which* source the tractor
chose via `SpeedSource` (`WheelBased`, `GroundBased`, `NavigationBased`,
`Blended`).

### Hitch and PTO status (the implement reads these)

`HitchStatus` and `PtoStatus` are the feedback frames the tractor broadcasts for
its front and rear three-point hitch and power take-off. The implement uses
hitch position to know when it has been lifted out of work, and PTO speed/engaged
state to know whether driven tools have power.

### Hitch, PTO, and valve commands (the implement may send these)

These are the implement's lever for limited tractor control, defined in the
command portion of ISO 11783-7. They only do anything if the tractor accepts
them.

| Type | Command set |
| --- | --- |
| `HitchCommand` | `NoAction`, `Lower`, `Raise`, `Position` (with a target). |
| `HitchCommandMsg` | The wire message: `command`, `target_position` (0.0025 % per bit), `rate`. |
| `PtoCommand` | `NoAction`, `Engage`, `Disengage`, `SetSpeed`. |
| `PtoCommandMsg` | `command`, `target_speed_rpm`, `ramp_rate`. |
| `ValveCommand` | `NoAction`, `Extend`, `Retract`, `Float`, `Block`. |
| `AuxValveCommandMsg` | A `valve_index` (0–15), a `ValveCommand`, and a `flow_rate`. |

Defaults follow the wire convention: unset numeric fields encode as the
"not available" sentinel (`0xFFFF` / `0xFF`), so a bare command says "do this
action, I'm not specifying a target".

### Lighting

`LightingState` is a snapshot of every standard lighting channel (turn signals,
beams, work lights, beacon, hazards, stop lamps), each a 2-bit `LightState`
(`Off`, `On`, `Error`, `NotAvailable`). The same struct rides two PGNs: the
tractor broadcasts implement-relevant lighting *data*, and a controller can send
a lighting *command* the implement is expected to obey.

### Drive/work strategy and combined commands

For tighter coordination, `DriveStrategyCmd` (with `DriveStrategyMode`:
`MaxPower`, `MaxEconomy`, `MaxSpeed`) lets an implement hint at how the tractor
should manage its powertrain, and `HitchPtoCombinedCmd` packs hitch and PTO into
one request. These are advanced, optional, and tractor-dependent.

### Required vs. available facilities

Before commanding anything, an implement should know what the tractor can do.
`TractorFacilities` is a bit-packed capability set; `TecuClass` summarises it
as `Class1` (basic speed/hitch/PTO), `Class2` (full measurements: distance,
direction, draft, lighting, aux flow), or `Class3` (accepts commands). The same
`TractorFacilities` payload travels in two roles via `TractorFacilitiesRole`:
`Response` is what the tractor advertises, `Required` is what the implement
declares it needs. If the tractor is only Class 1, the implement must not expect
a `Raise` command to do anything.

## How the implement combines this with VT and TC

The implement-message stream is one input to a loop that also touches the VT and
TC:

1. **Speed in.** Decode `WheelBasedSpeedDist` / `MachineSelectedSpeedFull` each
   tick; cache the latest.
2. **Operator in.** The [VT client](virtual-terminal-client.md) delivers
   button/soft-key events — sections on/off, target rate changes.
3. **Compute.** Convert ground speed plus target rate into a metering setpoint
   per section.
4. **Act on the machine.** Drive the implement's own actuators; optionally
   request hitch/PTO action via the command messages if the operation needs it.
5. **Record out.** Push as-applied values and section state to the
   [TC client](task-controller-client.md) as process data.

Keep the three views consistent: the section count you show on the VT, the DDOP
sections you declare to the TC, and the section state your control logic acts on
must all describe the same machine.

## Doing it with machbus

There are two layers. The codec structs above are pure encode/decode. The
session facade wires them to the bus for you through the `Implement` plugin.

### Building an implement-capable session

Plug the `Implement` plugin into a `Session`. Both sides of an
implement/tractor pairing plug it — the codecs are symmetric, so "send a hitch
command" and "receive a hitch command" use the same machinery:

```rust
use machbus::session::{Session, EndpointTransport, plugins::Implement};

let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(Implement::new())
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;
```

After the usual address claim, `ctrl.with_mut::<Implement, _>(|imp| ...)` reaches
the plugin: its outbound methods encode and send in one call, and its cached
getters let you read the latest state without draining events.

### Sending commands

From the tractor (or any node allowed to command), the outbound side is direct:

```rust
ctrl.with_mut::<Implement, _>(|imp| {
    imp.command_hitch(Hitch::Rear, HitchCommand::Raise);
    imp.command_pto_speed(Pto::Rear, 540, 0);
    imp.command_aux_valve(0, ValveCommand::Extend, 100)
})?;
```

`command_hitch(Hitch, HitchCommand)` and `command_hitch_position(...)` send to
the rear or front hitch by PGN; `command_pto` / `command_pto_speed(Pto, rpm, ramp_rate)`
do the same for the PTO; `command_aux_valve(index, ValveCommand, flow_rate)`
addresses one of up to 16 valves (`MAX_AUX_VALVES`) and rejects an out-of-range
index.

### Publishing status

The implement side broadcasts its own feedback with the `broadcast_*` methods:
`broadcast_hitch_status`, `broadcast_pto_status`, `broadcast_wheel_speed`,
`broadcast_ground_speed`, `broadcast_machine_selected_speed`,
`broadcast_lighting_data`, and `command_lighting` for the lighting-command PGN.

### Reading what arrived

Inbound frames are decoded on each `driver.poll()?` and surface two ways. Cached
getters give you the latest of each — `last_front_hitch_status()`,
`last_rear_pto_status()`, and `last_wheel_speed()` — read through
`ctrl.with::<Implement, _>(|imp| ...)`. Or drain the event stream with
`ctrl.drain::<ImplementEvent>()`, matching on `ImplementEvent`.

## Events and responsibilities

Each decoded inbound frame becomes an `ImplementEvent`:

| Event | Fired when | Typical implement action |
| --- | --- | --- |
| `HitchCommand { hitch, msg }` | A hitch command arrives. | Act if you are the actuator; otherwise ignore. |
| `PtoCommand { pto, msg }` | A PTO command arrives. | Same. |
| `AuxValveCommand(msg)` | A valve command arrives. | Drive the addressed valve. |
| `HitchStatus { hitch, msg }` | Tractor reports hitch feedback. | Detect raised/in-work to gate application. |
| `PtoStatus { pto, msg }` | Tractor reports PTO feedback. | Confirm driven tools have power. |
| `WheelSpeed` / `GroundSpeed` | Speed/distance broadcast. | Update the metering setpoint. |
| `MachineSelectedSpeed` | Selected-speed broadcast. | Use as the authoritative ground speed. |
| `Lighting { command, source, state }` | Lighting data or command. | Mirror or obey lighting. |

Your responsibilities: decide which side of each message you are (consumer or
actuator), keep your cached view fresh by ticking, and never act on a stale
value as if it were current.

## Edge cases and failures

- **Tractor facility unavailable.** If the tractor is Class 1, it does not accept
  hitch/PTO/valve commands. Read `TractorFacilities` / `TecuClass` first; if the
  capability is absent, do not command it and tell the operator why.
- **Command rejected or ignored.** A command message is a request, not a
  guarantee. The tractor may decline. Confirm intent by watching the
  corresponding `HitchStatus` / `PtoStatus` feedback, not by assuming the command
  took effect.
- **Sentinel / "not available" values.** Speed, distance, position, and rate
  fields all have a not-available encoding (`0xFFFF` / `0xFF`, and enum
  `NotAvailable` / `Error` variants). Treat these as "no data", never as zero.
  A `direction` of `NotAvailable` is not "stopped".
- **Stale tractor data.** Broadcasts are periodic. If the stream stops — bus
  fault, tractor reset, disconnect — your last cached speed goes stale fast.
  Time-stamp what you cache and fall back to a safe default (stop metering, hold
  sections off) when data ages out, rather than metering to a value frozen
  minutes ago.
- **Data loss safe defaults.** On loss of speed, the only safe assumption is
  that you do not know the speed: stop applying product. On loss of hitch
  status, assume nothing about whether you are in work.
- **Index out of range.** `command_aux_valve` validates `valve_index` against
  `MAX_AUX_VALVES` and returns an error rather than sending a bad PGN.

## Advanced

- **Coordinating VT + TC + implement messages.** These three streams update at
  different rates and arrive interleaved. Drive them from one `driver.poll()?`
  loop, cache the latest of each, and compute on a fixed cadence rather than
  reacting frame by frame — that keeps your control loop stable when one stream
  stutters.
- **Update timing.** Status broadcasts are periodic (commonly around 100 ms for
  speed and lighting). Send your own status at the expected cadence; consumers
  rely on it being fresh, and a slow or bursty publisher looks like a fault.
- **TIM authority for active control.** Reading tractor state needs no special
  authority. *Actively controlling* the tractor — steering, speed, sustained
  hitch/PTO automation — is the domain of [TIM](tim.md), which adds the
  authority handshake and timeouts that make automatic control safe. Use the
  implement command messages for the limited, operator-initiated actions ISO
  11783-7 covers; reach for TIM when the implement is genuinely driving the
  tractor. See [TIM](tim.md) for that boundary.
- **Session facade vs. the bare codecs.** The `Implement` plugin is right for
  applications: it sends, caches, and fans out events. The bare codecs are right
  for tests and tightly controlled loops where you own every byte and every send.

## Validate locally

```sh
make test
```

The session tests build an 8-section sprayer from the `Implement` plugin, toggle
section state, and exercise the alarm panel and diagnostics. They also put a
tractor and an implement on one virtual bus, have the tractor command a rear
hitch raise, a rear PTO speed, and an aux-valve extend, and show the implement
receiving each as an `ImplementEvent` plus reading it back from the cached
getters.

## What this proves / does not prove

Proves: the implement-side codecs encode and decode the speed/distance, hitch,
PTO, valve, and lighting messages correctly, and the `Implement` plugin ships and
receives them across a virtual bus with the right caching and event fan-out.

Does not prove: real-hardware timing, interoperability with a specific tractor or
implement, or any conformance/certification claim. `machbus` is not certified;
real deployment still needs official standards, real hardware, and
interoperability evidence.

## See also

- [Tractor ECU](tractor-ecu.md) — the producer side of the same messages.
- [Virtual terminal client](virtual-terminal-client.md) — the operator UI.
- [Task controller client](task-controller-client.md) — process-data logging.
- [TIM](tim.md) — authority and timeouts for actively controlling the tractor.
