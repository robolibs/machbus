# Tractor-Implement Management (TIM)

Tractor-Implement Management lets an implement ask the tractor for **bounded,
supervised, revocable authority** to command certain tractor functions —
vehicle speed, front and rear hitch, front and rear PTO, and auxiliary valves —
so the implement can coordinate the whole machine instead of just itself. A
baler that senses a heavy windrow can request authority and ask the tractor to
slow down; a planter can ask the rear hitch to lift at a headland. The implement
never seizes control: it requests, the tractor grants within limits it chooses,
and either side can take the authority back at any instant.

This tutorial explains why TIM exists, the authority lifecycle as a state
machine, the interlocks the tractor uses to bound what it accepts, and how to
drive the whole thing through `machbus`'s `isobus::tim` types and the
`Tim` session plugin.

> ## Safety framing — read this first
>
> TIM commands physical motion on a real machine, so it is **safety-critical**.
> Everything below is written assuming these non-negotiable properties:
>
> - **Operator-supervised, never unattended.** TIM is an operator-assistance
>   feature. A present, attentive operator is part of the system, not optional.
> - **Bounded.** The tractor only ever accepts commands inside limits *it*
>   enforces. The implement cannot exceed what the tractor allows.
> - **Instantly revocable.** Either side can withdraw authority at any moment,
>   and the machine must fall back to a safe state on revocation, timeout, or
>   loss of communication.
> - **`machbus` is not certified and is not a safety system.** The types here
>   model the *protocol shape* of TIM authority for development and testing on a
>   virtual bus. They are not a substitute for certified hardware, functional-
>   safety engineering, official AEF automation conformance, or the safety
>   interlocks of a real tractor. Do not use `machbus` to control an actual
>   machine without that engineering in place.

## Why this exists

An implement often knows things the tractor cannot: how heavy the current load
is, where the row ends, whether the bale chamber is full. Without TIM, a human
operator is the only channel for that knowledge to reach the tractor's controls,
and reaction time and attention become the limit. TIM gives the implement a
disciplined way to act on what it knows — but only inside a frame the tractor
and operator define.

The design follows the AEF automation principles and ISO 11783 Tractor-Implement
Management: the implement is a *client* asking for control; the tractor is a
*server* that owns the actuators and decides what it is willing to delegate. The
server is always in charge. The client only ever borrows authority, and only for
the specific functions it negotiated.

## Mental model

```
   implement (client)                         tractor (server)
        │                                           │
        │ 1. request authority (a set of options)   │
        │ ─────────────────────────────────────────►│ checks: supported?
        │                                           │ interlocks clear?
        │ 2a. granted (within tractor limits)       │
        │ ◄─────────────────────────────────────────│
        │                                           │
        │ 3. bounded commands (speed/hitch/PTO/aux) │
        │ ─────────────────────────────────────────►│ clamps to limits,
        │                                           │ actuates
        │ 4. status broadcasts                      │
        │ ◄─────────────────────────────────────────│
        │                                           │
   either side may revoke at any time ──► machine returns to a safe state
        │                                           │
        │ 2b. denied (not supported / blocked)      │
        │ ◄─────────────────────────────────────────│ no authority granted
```

Authority is a lease, not a transfer. The implement holds it only while every
condition stays true: the option was negotiated, the operator is present, no
stop is active, the machine is not in road transport, and messages keep
flowing. The moment any of those fails, the lease ends and the tractor returns
to a safe state.

## Anatomy: options, commands, status

`machbus` splits TIM into three concerns, each with its own types in
`isobus::tim`.

### Options — what may be controlled

A TIM *option* names one controllable capability, such as "rear hitch position"
or "vehicle speed in the forward direction". They are listed in the `TimOption`
enum (for example `TimOption::RearHitchPositionIsSupported`,
`TimOption::VehicleSpeedInForwardDirectionIsSupported`,
`TimOption::GuidanceCurvatureIsSupported`). The currently defined options occupy
a fixed set of bits; `TimOptionSet` packs them into a three-byte bitset
(`TIM_OPTION_BYTES`).

| Type | Role |
| --- | --- |
| `TimOption` | One named controllable function (PTO, hitch, speed, guidance). |
| `TimOptionSet` | A bitset of options — what a node *supports*, *requests*, or is *granted*. |

You build a set from options and reason about it set-wise:

```rust
// Illustrative shape — verify exact calls against isobus::tim.
let available = TimOptionSet::from_options(&[
    TimOption::RearHitchPositionIsSupported,
    TimOption::VehicleSpeedInForwardDirectionIsSupported,
]);
let requested = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
assert!(requested.is_subset_of(&available));        // can only request what is supported
let missing = requested.missing_from(&available);   // what would be refused
```

`TimOptionSet::try_from_bytes` rejects reserved bits via
`TimValidationError::ReservedOptionBits`, so a malformed capability payload
cannot be smuggled in as granted authority.

### Commands — the bounded actions

A `TimCommand` is a concrete action the authority guard reasons about, such as
`TimCommand::RearHitchPosition` or `TimCommand::RearPtoSpeedCw`. Every command
maps to exactly one required option through `TimCommand::required_option()`, so
the guard can check "is this command covered by what was granted?" without
guesswork.

### Status — what the tractor reports

The state of each actuator travels as a small fixed-layout payload. `machbus`
encodes and decodes these:

- `PtoState` — engaged flag, direction, shaft speed in RPM.
- `HitchState` — motion-enabled flag plus a position `0..=MAX_HITCH_POSITION`
  (`10_000`, i.e. `0.00%`–`100.00%` at `0.01%`/bit); out-of-range positions are
  rejected by `validate()` / `try_encode()` with
  `TimValidationError::HitchPositionOutOfRange`.
- `AuxValveCommand` — a valve index `0..MAX_AUX_VALVES` (`32`), state, and flow;
  an out-of-range index yields `TimValidationError::AuxValveIndexOutOfRange`.

These are deliberately strict: the decoders reject wrong lengths, bad padding,
and non-boolean flag bytes, so a corrupt frame becomes "no value" rather than a
plausible-looking wrong value.

## Lifecycle and state machine

The local authority guard is `TimAuthority`, and its lifecycle is the heart of
TIM. Its states are `TimAuthorityState`:

| State | Meaning |
| --- | --- |
| `Idle` | No request outstanding. Nothing may be commanded. |
| `Requested` | The client asked for a set of options; awaiting a grant. |
| `Granted` | Authority is active. Covered commands may be issued. |
| `Denied` | The request was refused. |
| `Revoked` | A previously granted authority was withdrawn. |

The transitions and their triggers:

```
              request(set)            grant()
   Idle ───────────────────► Requested ───────► Granted
     ▲                          │  │               │
     │                          │  │ deny()        │ revoke()
     │                          │  └──────────────►│ or interlock trips
     │                          │                  ▼
     │                          └─────────────► Denied / Revoked
     └──────────────────── request(set) again ◄────┘
```

1. **request.** `TimAuthority::request(set)` moves `Idle → Requested`. It fails
   with `UnsupportedOptions` if the set is not a subset of what is available, or
   `ReservedOptionBits` if either set has undefined bits set. You cannot request
   what you do not support.
2. **grant.** `grant()` moves `Requested → Granted` — but only if no interlock
   is blocking. If a stop, road mode, or missing-operator condition is active,
   the grant is refused with `InterlockActive` and the machine stays safe. A
   `grant()` from any state other than `Requested` fails with
   `AuthorityNotRequested`.
3. **deny.** `deny()` records `Denied`. The client must not command anything.
4. **command.** While `Granted`, `ensure_command(cmd)` (or `ensure_option`)
   checks four things in order: the option is supported, it was part of the
   request, no interlock is blocking, and the state is still `Granted`. Only if
   all four hold does the command proceed.
5. **revoke / forced safe state.** `revoke()` moves to `Revoked`. Crucially,
   `set_interlocks(...)` also forces `Granted → Revoked` automatically the
   instant a blocking interlock appears — the client does not have to notice and
   react; the guard revokes for it. A revoked authority blocks every command
   until a fresh `request` + `grant` cycle re-establishes it.

The one-way rule that makes this safe: **anything that can go wrong drops you
out of `Granted`, and nothing climbs back into `Granted` except an explicit new
grant under clear interlocks.**

## Interlocks and limit enforcement

Two independent layers bound what TIM can do.

**Local interlocks** live in `TimInterlocks`, a snapshot the guard consults
before granting and before every command. The blocking conditions, expressed as
`TimInterlock`, are:

| Condition | `TimInterlock` | Why it blocks |
| --- | --- | --- |
| Operator absent | `OperatorNotPresent` | TIM requires a supervising operator. |
| Road transport mode | `RoadTransportMode` | Implement control on the road is unsafe. |
| External stop active | `ExternalStop` | A stop request (e.g. a safe-state input) overrides everything. |
| Implement not ready | `ImplementNotReady` | The implement itself is not in a state to be controlled. |

`TimInterlocks::all_clear()` is the default "everything permits TIM" snapshot;
you opt into a blocking condition explicitly
(`all_clear().with_external_stop(true)`), and `blocking_reason()` returns the
first active block, or `None` when clear. Because `set_interlocks` revokes a
live grant the moment a block appears, an interlock change *is* the safe-state
trigger.

**Tractor-side limits** are the second layer and they live on the tractor, not
in the client. The implement can only ever ask for an option the tractor
published as available, and the tractor clamps each command to the range it is
willing to actuate. The client's `TimAuthority` proves a command is *allowed to
be sent*; the tractor still decides what it is *willing to do*. Never assume a
sent command was accepted at full value — read the status broadcasts back.

## Doing it with machbus

The pure guard (`TimAuthority`, `TimOptionSet`, the codecs) has no networking.
The `Tim` plugin wires it to real PGNs when you plug it into a `Session` with the
options the node supports.

A typical client flow — request, grant, command, observe — looks like this:

```rust
use machbus::session::{Session, EndpointTransport, plugins::Tim};

let rear_hitch = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);

let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(Tim::new(TimAuthority::new(rear_hitch)))
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;

// ... after address claim ...

ctrl.with_mut::<Tim, _>(|tim| {
    // Before authority, a command is refused locally and never hits the bus:
    assert!(tim.command_hitch_position(Hitch::Rear, 20_000, 12).is_err()); // OptionNotRequested

    // Negotiate, then command:
    tim.request_authority(rear_hitch)?;
    tim.grant_authority()?;
    tim.command_hitch_position(Hitch::Rear, 20_000, 12)   // now crosses the bus
})?;
```

The guarded command helpers on the `Tim` plugin are `command_hitch_position`,
`command_pto_engage`, and `command_pto_disengage`. Each one calls the guard
first: if `ensure_command` fails,
the helper emits a local `TimEvent::CommandBlocked` and returns an error
**without sending any CAN frame**. Authority changes are surfaced as
`TimEvent::AuthorityStateChanged`. Status the node observes from peers arrives as
`TimEvent::PtoStatus`, `HitchStatus`, `PtoCommand`, `HitchCommand`, and
`AuxValveCommand`, cached on the plugin (`last_front_pto_status`,
`last_rear_hitch_status`, and so on). Drain them with `ctrl.drain::<TimEvent>()`.

A tractor (server) side plugs TIM the same way and uses the broadcast helpers
`broadcast_pto_status`, `broadcast_hitch_status`, and
`broadcast_aux_valve_command` to publish actuator state; invalid payloads are
rejected before send, so a bad value never leaves the node.

## Events and responsibilities

Whatever role you play, these duties are not optional:

| Responsibility | What you must do |
| --- | --- |
| Request only what you support | Build the requested `TimOptionSet` as a subset of available options. |
| Honor revocation immediately | Stop commanding the instant authority leaves `Granted`. |
| Go to a safe state on loss | On timeout, revocation, or interlock trip, command the machine to a defined safe state, not "last value". |
| Keep messages flowing | TIM is periodic; the tractor expects fresh authority/command/status traffic (around `TIM_UPDATE_INTERVAL_MS`). Silence must be treated as loss. |
| Never command from `Denied`/`Revoked` | Re-run `request` + `grant` before commanding again. |

The guard helps with the first and the last by construction, but **timeout and
safe-state behavior are your application's responsibility** — the pure guard does
not run a clock for you.

## Edge cases and failures

- **Authority denied.** The request was not granted (`deny()`), or `grant()`
  failed because an interlock was active. The client must treat the function as
  unavailable and command nothing.
- **Revoked mid-command.** The operator, the tractor, or an interlock withdrew
  authority while you were commanding. The next guarded helper returns an error
  and emits `CommandBlocked`; you must immediately drive the machine to a safe
  state rather than repeating the last command.
- **Timeout / loss of communication.** If authority, command, or status traffic
  stops, both sides must assume the worst and fall back to safe — the tractor
  releases delegated control, the client stops asserting it.
- **Out-of-limit command.** A position past `MAX_HITCH_POSITION` or a valve
  index past `MAX_AUX_VALVES` is rejected before encoding; even a well-formed
  command is still clamped by the tractor to its own limits, so observed state
  may differ from what you asked for.
- **Operator override.** The operator can always take back control. An operator
  action that clears `operator_present` or asserts an external stop revokes the
  grant through `set_interlocks`, and the machine returns to a safe state.

## Advanced

- **Combining with guidance and speed.** `TimOption::GuidanceCurvatureIsSupported`
  and the vehicle-speed options let an implement coordinate steering and travel
  speed together — for example holding a guidance line while slowing for load.
  Each function is still negotiated and bounded independently; granting hitch
  control says nothing about speed control. See [Guidance](guidance.md).
- **Who is liable.** Because the implement is borrowing authority over a machine
  it does not own, the boundaries matter legally as well as technically. The
  tractor's published limits and interlocks define what the implement is *able*
  to do; the operator's supervision defines what is *permitted* to happen. The
  AEF automation framework and the official conformance process exist precisely
  to pin down these responsibilities — `machbus` carries none of that
  certification.
- **Why bounded authority instead of full control.** A lease that the server
  clamps and either party can revoke means no single failure (a buggy implement,
  a dropped message, an inattentive operator) can drive the machine outside the
  envelope the tractor and operator agreed to. Bounded, revocable authority is
  what makes implement-driven automation tolerable on a safety-critical machine.
- **Pure guard vs. the session facade.** Use `TimAuthority` directly in unit
  tests and embedded loops where you own timing and wiring. Use the `Tim` plugin
  in applications: it couples the same guard to the real PGNs and the unified
  event queue for you.

## Validate locally

```sh
make test
```

The `tests/standard/aef_tim_automation.rs` and `tests/standard/session_harness.rs`
checks drive the workflow over a two-node virtual bus: a command blocked before authority stays local and emits no frame, a
granted command crosses the bus and is decoded by the peer, an external stop
revokes authority and blocks the next command, and status/aux payloads round-
trip and update the peer's caches. The pure-guard unit tests live alongside the
code in `src/isobus/tim.rs`. There is no dedicated `examples/` binary for TIM;
the tests are the runnable reference.

## What this proves / does not prove

Proves: the option negotiation, the request → grant → command → revoke
lifecycle, interlock-driven revocation, and the strict status/command codecs
behave deterministically in software, and that `machbus` refuses to emit a TIM
command frame unless authority is granted and interlocks are clear.

Does **not** prove — and this matters most for TIM — that any of this is **safe
for real, unattended, or production control**. It proves nothing about real-
hardware timing, functional-safety integrity, interoperability with a specific
tractor, operator-presence enforcement on a real machine, or AEF automation
certification. Controlling an actual tractor requires certified hardware, a
functional-safety case, the official AEF conformance process, and a supervising
operator — none of which `machbus` provides.

## See also

- [Sequence control](sequence-control.md) — ordered, conditional command
  sequences that often drive TIM functions.
- [Guidance](guidance.md) — curvature and steering, frequently combined with TIM
  speed control.
- Shortcut button and safe state — the operator's always-available path to stop
  automation and return the machine to safe.
