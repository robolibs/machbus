# AutoDrive (steering + speed)

`AutoDrive` is the combined autonomous-driving controller: **one engage lifecycle,
one setpoint, one stop latch** across both axes a moving machine has — where it
steers and how fast it goes.

It is the **only** guidance controller in machbus: an earlier `Guidance` plugin
covered the same two messages with a weaker model and was removed. This page
covers the lifecycle, the heartbeat contract, every way it stops, and the
question that trips almost everyone up first: **do I need TIM for this?**

## Safety first

Autonomy moves a machine with a person on it:

- **Operator-supervised, not autonomous.** A human is in the seat.
- **machbus is not a safety system and is not certified.** It moves setpoints on
  the wire. It does not plan paths, close a loop, or supervise anything.

This plugin owns the *protocol* half of that: preconditions, a latching stop and
refusals. The *operator input* half — a held dead-man, a deliberate arm latch,
and treating a lost controller as a release — sits above it, in whatever drives
the plugin. See [`machbus drive`'s safety model](drive-tool.md) for a worked
example you can read and copy.

## Do I need TIM?

**No — not for what `AutoDrive` does.** This is the single most common confusion,
so it is worth being precise.

There are **two unrelated protocols** in ISOBUS that can influence a tractor's
speed and steering. They are not two implementations of one idea; they are
different messages on different PGNs with different rules.

| | ISO 11783-7 native — what `AutoDrive` speaks | AEF TIM — [`src/isobus/tim/`](tim.md) |
| --- | --- | --- |
| Steering | **PGN 0xAD00** Guidance System Command | PGN 0x2400/0x2300, function `ExternalGuidance` (0x46) |
| Speed | **PGN 0xFD43** Machine Selected Speed Command | same PGN pair, function `VehicleSpeed` (0x44) |
| Gate before you may command | none — broadcast | assignment table + authentication + heartbeat counter |
| Also covers | nothing else | PTO, hitch, auxiliary valves |

`AutoDrive` has **no TIM coupling at all**. It never consults a TIM authority,
never waits for an assignment, and never sends a TIM message. It claims an
address and broadcasts.

> **`NoAuthority` is still unused.** `AutodriveRefusal::NoAuthority` sounds like
> TIM, and nothing in the crate constructs it. It is a placeholder for a
> TIM-gated path that does not exist yet — do not read it as evidence that one
> does. (`FacilityNotAdvertised` *is* now produced; see below.)

### The tractor has to advertise the facility first

This is the part that decides whether any of it works, and it is not TIM.

ISO 11783-9:2012 §4.4.2 splits tractor capability into **classes with letter
addenda**, and steering, speed and starting to move are three separate ones:

| Addendum | Clause | What it means |
| --- | --- | --- |
| **G** | §4.4.2.7 | The tractor "shall support the external control of the guidance system" — curvature command, estimated curvature, command status, request reset status, steering input position status, steering system readiness, mechanical lockout. |
| **P** | §4.4.2.8 | The tractor is "capable of accepting speed and/or drive strategy commands from an implement controller". |
| **M** | §4.4.2.9 | The tractor accepts "commands to initiate motion of the vehicle (forward or reverse)". |

So a class **2G** tractor steers on command but need not accept a speed command
at all, and even a class **2GP** tractor need not *start moving* on one — §4.4.2.8
says outright that bringing the tractor to a stop (speed `0.0`) is **optional**
and "can be determined by an implement via the tractor facilities response
message".

That message is the discovery mechanism, and it runs both ways:

- **PGN 0xFE09 Tractor Facilities Response** — what the TECU has installed. The
  TECU sends it at power-up and on request.
- **PGN 0xFE0A Required Tractor Facilities** — what *you* need. §4.4.2 is blunt
  about why this matters: "A facility is not required if its corresponding bits
  are set to 0 in the implement CF required tractor facilities message. **The
  Tractor ECU can then stop the transmission of this implement message to reduce
  bandwidth.**"

In other words, a conforming TECU may simply not broadcast Guidance Machine Info
to a node that never asked for it — and `AutoDrive` refuses to engage without
Machine Info, so the symptom is a permanent `link_down` with a tractor that is
working perfectly.

`AutoDrive` therefore broadcasts Required Tractor Facilities every
`REQUIRED_FACILITIES_INTERVAL_MS` (1 s) once its address is claimed, asking for
guidance + machine selected speed + speed command, and decodes the response:

- Response says **no guidance** → `arm()` and `engage()` refuse with
  `facility_not_advertised`, and a tractor that revokes it mid-drive trips a safe
  stop.
- Response says **no speed command** → a `DriveCommand` carrying a speed is
  refused; a steer-only command still goes through.
- **No response at all** → nothing is refused. Silence is not a denial, and a
  bench, a replay or a retrofit box may never send one.

### So which do you actually need?

- A retrofit guidance system, a test bench, or a machine that implements the
  11783-7 messages directly will act on `AutoDrive`'s broadcasts.
- A tractor that advertises **G** steers on 0xAD00 without any TIM involvement.
  That is the plain reading of §4.4.2.7 and it is why TIM is not required for
  steering.
- An **AEF-certified TIM tractor** may still ignore a bare 0xFD43 from an
  unauthenticated implement. Speed is the function OEMs guard hardest, and TIM
  exists precisely so that handing over speed authority is explicit and
  revocable.

### TIM is where these messages are heading

ISO 11783-7:2022 **Clause 11** says so directly, naming both PGNs `AutoDrive`
uses:

> "In future editions of the ISO 11783 series, the command messages concept will
> be moved to a new concept termed 'Tractor-Implement Management' (TIM). The new
> concept is expected to eventually supersede or, at least, render redundant,
> some of the tractor control related messages… The messaging most likely to be
> impacted, and potentially deprecated, are: … Guidance system command
> (PGN 44288) … Machine selected speed command (PGN 64835) … Drive strategy
> command (PGN 64718)."

So: TIM is **not required today** for a tractor that advertises G (and P), but
the native command messages are explicitly flagged as the path being superseded.
Plan for TIM on new work; do not assume you need it to steer a current machine.

Also worth knowing: machbus's `TimAuthority` currently guards **PTO, hitch and
auxiliary valves only**. There is no plugin wiring TIM speed or TIM steering, so
"use TIM instead" is not currently a thing you can do for these two axes.

## What the design gives you

Four properties are worth knowing about before you write a control loop, because
each replaced something weaker.

### 1. An automation status, not a boolean

`AutoDrive` reports the ISO 11783-7 Table 45 `AutomationStatus`:

```
NotReady ──arm()──► ReadyToEnable ──engage()──► ActiveNotLimited
                                                 ├─► ActiveLimitedHigh
                                                 ├─► ActiveLimitedLow
                                                 └─► Fault  (any safe stop)
```

This is not cosmetic. The plugin **mirrors the machine's own limit status into
its status**: when the steering ECU reports `LimitedHigh` / `LimitedLow`, it
moves to `ActiveLimitedHigh` / `ActiveLimitedLow` and you can read it back with
`status()`. That is the anti-windup signal an outer control loop needs — a
tracker that does not know the steering ECU is saturated will wind up against a
limit it cannot see.

`arm()` before `engage()` also keeps "preconditions are met" and "I am now asking
for the wheel" as distinct states.

### 2. Commands are refused, not clamped

```rust,ignore
d.command(DriveCommand {
    speed_mps: Some(2.0),
    curvature_km_inv: Some(12.5),
})?;
```

`command` returns `Result<(), AutodriveRefusal>` and has **no infallible
sibling**, so the wire codec is never the only range check on a steering
command. An out-of-range curvature would otherwise be clamped by the encoder to
±8031.75 km⁻¹ — a 12 cm turn radius — and transmitted as a perfectly valid
maximum-curvature command. It refuses instead, with `CurvatureOutOfRange`,
`SpeedNotFinite`, `SpeedBelowMinimum`, `StopLatched` or `StatusNotActive`.

`DriveCommand`'s two `Option` fields also let you express *steer only* (leave
speed to whoever owns it) with `DriveCommand::steer(k)`, or drive straight,
without coupling the two axes through a twist.

### 3. The stop latch is fed only from inside

There is no public way to trip the latch. Every producer is inside the plugin,
and a source-scanning test enforces that each `SafeStopTrigger` variant has a
real producer — so a trigger cannot be declared and then quietly never fire.

### 4. GNSS latches a stop

`GnssHazards` blocks `engage()` and `clear_stop()`, and additionally trips a
**latching** stop on `PositionStale` / `FixDegraded`, so a receiver that stops
reporting halts the machine rather than only preventing re-engagement.

## The lifecycle

Plug it and claim, exactly like any other plugin:

```rust,ignore
{{#include ../../../examples/autodrive_keyboard.rs:build}}
```

Then arm, then engage. Both report the **first unmet precondition** rather than
failing silently, because an autonomy client that asks to steer and is ignored
cannot tell "commanded" from "declined":

```rust,ignore
{{#include ../../../examples/autodrive_keyboard.rs:arm}}
```

`arm()` and `engage()` check, in order: no latched stop, no held shortcut button,
no live GNSS hazard, link alive, machine info present, then the machine's own
report — mechanical lockout clear and operator engage switch active.

Those machine conditions are **re-checked on every Machine Info broadcast**, not
just at engage. The operator dropping the engage switch or asserting the lockout
mid-drive stops this node asking for the wheel.

`disengage()` is deliberately infallible and idempotent: a disengage must never be
refused.

## The command is a heartbeat

This is the contract most likely to surprise you.

While engaged, `AutoDrive` re-transmits at `MIN_TX_INTERVAL_MS` (100 ms, the ISO
11783-7 §5.2.7.2 minimum for the guidance group) **even when the setpoint has not
changed**, because the command *is* the heartbeat the steering ECU times out on.

That turns a fail-silent path into a fail-active one. An application that dies
used to emit no frames and let the ECU time out; with a heartbeat it would instead
keep the machine steering at the last curvature forever. So:

> **You must call `command()` at least every `COMMAND_STALE_MS` (300 ms) while
> engaged**, even with an unchanged setpoint. Miss it and `AutoDrive` trips
> `SafeStopTrigger::CommandStale`, falls to `DriveCommand::halt()` and latches.

Tune with `with_command_stale_ms(ms)`; `0` disables the watchdog, which is only
appropriate when something else guarantees liveness.

The driving loop, then, refreshes the setpoint every cycle:

```rust,ignore
{{#include ../../../examples/autodrive_keyboard.rs:heartbeat}}
```

Releasing the dead-man disengages. `disengage()` is infallible and falls back to
`DriveCommand::halt()` — and **losing** the dead-man has to land here too, not
leave the last setpoint running:

```rust,ignore
{{#include ../../../examples/autodrive_keyboard.rs:deadman}}
```

The stop is latching, so re-engaging is refused until it is explicitly cleared:

```rust,ignore
{{#include ../../../examples/autodrive_keyboard.rs:clear}}
```

## Cadence and tuning

| Constant | Default | Meaning |
| --- | --- | --- |
| `MIN_TX_INTERVAL_MS` | 100 ms | Conformance minimum; `with_cadence` clamps to it |
| `MAX_TX_INTERVAL_MS` | 2000 ms | Idle re-broadcast when not active |
| `COMMAND_STALE_MS` | 300 ms | Unrefreshed setpoint → `CommandStale` |
| `LINK_TIMEOUT_MS` | 300 ms | Three missed 100 ms Machine Info → `GuidanceLinkTimeout` |
| `DEFAULT_MIN_SPEED_MPS` | 0.05 m/s | Below this a yaw rate does not define a curvature |

```rust,ignore
AutoDrive::new()
    .with_cadence(100, 2000)      // min is clamped up to MIN_TX_INTERVAL_MS
    .with_command_stale_ms(300)
    .with_min_speed(0.05)
```

## Every way it stops

On any of these the plugin latches, sets `AutomationStatus::Fault`, replaces the
setpoint with `DriveCommand::halt()` (speed 0, curvature 0) and emits
`AutodriveEvent::SafeStop { trigger }`.

| Trigger | Cause |
| --- | --- |
| `GuidanceLinkTimeout` | 300 ms without Machine Info |
| `IsbStop` | Operator holds the shortcut button, a seen ISB source goes silent, or the machine reports a mechanical lockout |
| `OperatorOverride` | Engage switch dropped, or the ECU reports `OperatorLimitedControlled` / `NonRecoverableFault` |
| `CommandStale` | You stopped refreshing the setpoint |
| `SendFailed(pgn)` | The network layer refused one of the two command PGNs |
| `PositionStale`, `FixDegraded` | GNSS events, via `GnssHazards` |
| `BusOff`, `AddressClaimLost`, `HeartbeatError`, `ClockWentBackwards`, `KeySwitchOff` | Session events |

The latch is **latching, not momentary** — autonomy must not resume by itself
because a button was released or a link came back.

`clear_stop()` is the only way out, and it is deliberately explicit: clearing a
fault is not consent to move. It returns `Err(StopConditionLive)` while the
shortcut button is still held or a GNSS hazard is still live, so an HMI cannot
show "cleared" against a stop that is still asserted.

## What you need plugged

| Plugin | Why |
| --- | --- |
| `AutoDrive` | required |
| `Gnss` | **strongly recommended** — without it `GnssHazards` never fires, so `clear_stop()` can re-arm autonomy against a receiver that stopped reporting |
| `ShortcutButton` | the ISO stop-all path |
| `Diagnostics` | DTCs for the faults above |

## Path → curvature is still your job

The plugin moves a curvature value; it does not produce one. Your pure-pursuit or Stanley tracker turns the planned line, the GNSS pose
and the cross-track error into the single curvature that steers back onto the
line, and it lives in your code. See [Serial GNSS](serial-gnss.md) and
[NMEA 2000](nmea-2000.md) for pose, and [TC geo](tc-geo-prescription.md) for
where planned lines come from.

## Other languages

Both bindings expose the same surface behind an `enable_autodrive` flag, as
session-level `autodrive_*` functions and methods: arm, engage, disengage,
command, clear stop, and read back status / engaged / stop reason.

In C, `machbus_session_autodrive_stop_reason` returns a
`MachbusSafeStopTrigger` enum. Note that **codes 2 and 3 are permanently
retired** — reusing them would shift every value above for callers built against
an older header. See [ABI stability](../bindings/abi-stability.md).

## Validate locally

```sh
cargo run --example autodrive_keyboard   # the full driving lifecycle
cargo run --example guidance_autosteer   # the curvature conversation alone
make test
```

The example claims an address, shows an engage refused before any ECU answers,
drives for 500 ms at the 100 ms cadence, releases the dead-man into a latched
stop, and clears it. See the [AutoDrive example](../examples/autodrive.md) for a
line-by-line reading of the output.

## What this proves / does not prove

Proves: `AutoDrive` commands curvature on PGN 0xAD00 and speed on PGN 0xFD43
behind one engage lifecycle, refuses out-of-range and out-of-state requests
before encoding, and latches a safe stop on each trigger above.

Does not prove: anything about closed-loop steering or speed control, path
planning, operator supervision, actuator safety, real-machine timing,
interoperability with any specific tractor, whether a given tractor will act on an
unauthenticated speed command, or any certification. machbus is not a safety
system and is not certified.

## See also

- [`machbus drive` safety model](drive-tool.md) — the operator-input layer:
  dead-man, arm latch, and what happens when the controller is lost.
- [Automatic guidance](../standards/automatic-guidance.md) — the curvature
  model and the two PGNs.
- [TIM and automation](tim.md) — the authority-gated path, for PTO/hitch/aux.
- [TIM (AEF)](../standards/tim.md) — why authority exists at all.
