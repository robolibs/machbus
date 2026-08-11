# AutoDrive (steering + speed)

`AutoDrive` is the combined autonomous-driving controller: **one engage lifecycle,
one setpoint, one stop latch** across both axes a moving machine has — where it
steers and how fast it goes.

If you have read [Guidance](guidance.md) you already know most of the
wire story, because both plugins speak the same two messages. This page is about
what `AutoDrive` adds, when to pick it over `Guidance`, and the question that
trips almost everyone up first: **do I need TIM for this?**

## Safety first

Everything on the [Guidance safety note](guidance.md#safety-first) applies here
without change, and more so, because this plugin also commands speed:

- **Operator-supervised, not autonomous.** A human is in the seat.
- **machbus is not a safety system and is not certified.** It moves setpoints on
  the wire. It does not plan paths, close a loop, or supervise anything.

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

> **A note on two unused refusals.** `AutodriveRefusal` declares `NoAuthority`
> and `FacilityNotAdvertised`, which sound like TIM. Nothing in the crate ever
> constructs them. They are placeholders for a TIM-gated path that does not
> exist yet — do not read them as evidence that one does.

### So which do you actually need?

That depends on the tractor, and the honest answer is that **machbus cannot tell
you**:

- A retrofit guidance system, a test bench, or a machine that implements the
  11783-7 messages directly will act on `AutoDrive`'s broadcasts.
- An **AEF-certified TIM tractor** will very likely ignore a bare 0xFD43 from an
  unauthenticated implement. Speed is the function OEMs guard hardest, and TIM
  exists precisely so that handing over speed authority is explicit and
  revocable.

This book does not vendor the ISO 11783-7 or AEF 023 text, so it does not assert
what a conformant TECU is *obliged* to do with an unauthenticated speed command.
Treat it as an empirical question about your target machine. What is documented
here is what machbus sends.

Also worth knowing: machbus's `TimAuthority` currently guards **PTO, hitch and
auxiliary valves only**. There is no plugin wiring TIM speed or TIM steering, so
"use TIM instead" is not currently a thing you can do for these two axes.

## `AutoDrive` vs `Guidance`

Both plugins send **the same two PGNs**. `Guidance` is not steering-only — its
`command_velocity(v, ω)` sets a speed setpoint too. Both carry the shortcut-button
guard, the GNSS hazard guard, a stop latch, a link watchdog and a stale-command
watchdog. The trigger sets are nearly identical.

The differences are these four.

### 1. Automation status, not a boolean

`Guidance` has `engaged: bool`. `AutoDrive` has the ISO 11783-7 Table 45
`AutomationStatus`:

```
NotReady ──arm()──► ReadyToEnable ──engage()──► ActiveNotLimited
                                                 ├─► ActiveLimitedHigh
                                                 ├─► ActiveLimitedLow
                                                 └─► Fault  (any safe stop)
```

This is not cosmetic. `AutoDrive` **mirrors the machine's own limit status into
its status**: when the steering ECU reports `LimitedHigh` / `LimitedLow`, the
plugin moves to `ActiveLimitedHigh` / `ActiveLimitedLow` and you can read it back
with `status()`. That is the anti-windup signal an outer control loop needs — a
tracker that does not know the steering ECU is saturated will wind up against a
limit it cannot see. `Guidance` decodes the same field and discards it.

There is also a real `arm()` step before `engage()`, so "preconditions are met"
and "I am now asking for the wheel" are distinct states.

### 2. Commands are refused, not clamped

```rust,ignore
// Guidance — infallible; out-of-range input is clamped by the codec
g.command_curvature(1e9);      // → clamped to ±8031.75 km⁻¹, a 12 cm turn radius

// AutoDrive — one call, both axes, checked before anything is encoded
d.command(DriveCommand {
    speed_mps: Some(2.0),
    curvature_km_inv: Some(12.5),
})?;
```

`AutoDrive::command` returns `Result<(), AutodriveRefusal>` and has no infallible
sibling, so the wire encoder is never the only range check on a steering command.
It refuses with `CurvatureOutOfRange`, `SpeedNotFinite`, `SpeedBelowMinimum`,
`StopLatched` or `StatusNotActive`.

`DriveCommand`'s two `Option` fields also let you express *steer only* (leave
speed to whoever owns it) or *drive straight* cleanly. `Guidance` couples the two
through the twist: it computes `κ = ω / v`.

### 3. The stop latch is fed only from inside

`Guidance::request_stop` is public — anything holding `&mut Guidance` can trip its
latch. `AutoDrive`'s latch has no public trip; every producer is inside the
plugin, and a source-scanning test enforces that each `SafeStopTrigger` variant
has a real producer.

### 4. GNSS latches a stop

Both use `GnssHazards` to block `engage()` and `clear_stop()`. `AutoDrive`
additionally trips a **latching** stop on `PositionStale` / `FixDegraded`, so a
receiver that stops reporting halts the machine rather than only preventing
re-engagement.

### Picking one

Use **`AutoDrive`** for new work: it supersedes `Guidance`, and the status model
and refusals are what an autonomy client actually needs. Use `Guidance` if you
want steering under a simple boolean and are driving speed yourself.

They are **mutually exclusive** — both author PGN 0xAD00 from this address, so a
session that plugs both is refused at build rather than letting one silently
overwrite the other's safe stop.

## The lifecycle

```rust,ignore
let mut d = AutoDrive::new();

d.arm()?;                   // preconditions met, commanding nothing yet
d.engage()?;                // setpoint reaches the bus on the next tick

loop {
    // Every cycle. See "the command is a heartbeat" below.
    d.command(DriveCommand {
        speed_mps: Some(2.0),
        curvature_km_inv: Some(tracker.curvature()),
    })?;
}

d.disengage(SafeStopTrigger::OperatorOverride);   // infallible, idempotent
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

Not `Guidance` — see the mutual exclusion above.

## Path → curvature is still your job

Unchanged from `Guidance`: the plugin moves a curvature value, it does not produce
one. Your pure-pursuit or Stanley tracker turns the planned line, the GNSS pose
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

- [Guidance](guidance.md) — the older, simpler boolean-engage plugin.
- [Automatic guidance](../standards/automatic-guidance.md) — the curvature
  model and the two PGNs.
- [TIM and automation](tim.md) — the authority-gated path, for PTO/hitch/aux.
- [TIM (AEF)](../standards/tim.md) — why authority exists at all.
