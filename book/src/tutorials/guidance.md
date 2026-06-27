# Guidance (autosteer)

ISOBUS automatic guidance steers a tractor by **curvature**. You do not stream
waypoints and you do not command a raw steering angle — you send a desired path
curvature in **1/km** (the inverse of the turn radius; 0 = straight, the sign
chooses the turn direction). The tractor's steering ECU closes the loop on its own
wheels. This tutorial drives that conversation through the high-level
`session::plugins::Guidance` plugin.

If you have not read the concept page yet, start with
[Automatic guidance (autosteer)](../standards/autosteer.md) — it explains the
points-vs-angles-vs-curvature answer and the two PGNs in full. Here we build.

## Safety first

Autosteer moves a machine with a person on it. Two things stay true no matter what
your code does:

- **It is operator-supervised, not autonomous.** A human is in the seat and ready
  to take over. Touching the wheel drops the system out of automatic mode at once.
- **machbus is not a safety system and is not certified.** It moves curvature
  commands on the wire. It does not plan paths, close the steering loop, or
  supervise the operator. Treat everything below as protocol plumbing.

## The plugin in one minute

`Guidance` is a session plugin. Plug it, claim an address, and you can:

- **command** a curvature — `command_curvature(1/km)`, or the convenience
  `command_radius(metres)` and `command_straight()`;
- **command** in robotics `(v, ω)` terms — `command_velocity(linear_mps, angular_rad_s)`,
  which sends both the steering curvature and a speed command;
- **read back** the steering ECU's own view — `is_steering_ready()` and
  `estimated_curvature() -> Option<f64>`;
- **react** to inbound Agricultural Guidance Machine Info as an
  `Event::Guidance(GuidanceEvent::MachineInfo { .. })`.

The plugin sends the Guidance System Command (PGN 0xAD00) when you command, and
decodes Agricultural Guidance Machine Info (PGN 0xAC00) when the steering ECU
broadcasts it.

## 1. Plug Guidance and claim

A guidance-controller node is a session with the `Guidance` plugin plugged in. The
session core is sans-IO, so you drive it: tick it forward and let it claim an
address before you command anything.

```rust,ignore
{{#include ../../../examples/guidance_autosteer.rs:build}}
```

## 2. Command a curvature

Commanding is the whole point. Pick how you want to express the turn:

- `command_curvature(c)` — the native form, `c` in 1/km.
- `command_radius(r)` — `r` in metres, converted to `1000 / r` 1/km for you.
- `command_straight()` — the same as `command_curvature(0.0)`.

The command is queued on the plugin and flushed to the transmit buffer on the next
`tick`, where it becomes a Guidance System Command frame (PGN 0xAD00).

```rust,ignore
{{#include ../../../examples/guidance_autosteer.rs:command}}
```

That command also carries the controller's **intent to steer**. A curvature number
on its own is never a request to take the wheel — the intent is what arms it. The
controller is expected to keep sending a fresh command on a fixed cadence; it is a
heartbeat, and a stalled stream is a reason for the steering system to drop out.

## Commanding like a robot: (v, ω)

If you think in mobile-robotics terms — a linear velocity `v` and an angular (yaw)
velocity `ω` — use `command_velocity(v, ω)`. Because curvature is just `κ = ω / v`,
the plugin takes a twist directly:

```rust,ignore
// Drive at 2 m/s while yawing left at 0.04 rad/s.
// → κ = ω / v = 0.02 /m = 20 /km (a 50 m-radius turn).
session
    .get_mut::<Guidance>()
    .unwrap()
    .command_velocity(2.0, 0.04);
```

That single call sends **two** messages, because in ISOBUS steering and speed are
separate authorities:

- the steering **curvature** (`κ = ω / v`) as a Guidance System Command (PGN 0xAD00);
- the target **speed** (`v`) as a Machine Selected Speed Command (PGN 0xFD43), with
  reverse encoded as the command's direction.

A few things to keep in mind:

- A near-zero `v` cannot define a forward path curvature, so it commands straight
  (`κ = 0`) while still sending the (near-zero) speed.
- The curvature sign follows the ISO wire convention; if your platform's left/right
  sign differs, negate `ω`.
- `v` here is *both* the speed setpoint and the denominator of the curvature. If you
  do not want machbus to command speed at all, use `command_curvature` /
  `command_radius` and leave speed to the operator.

## 3. Read the steering ECU's feedback

You command through 0xAD00 and you verify through 0xAC00. The steering ECU
broadcasts Agricultural Guidance Machine Info; the plugin decodes it and exposes
the tractor's own view:

- `is_steering_ready()` — `true` when the steering system reports it is engaged and
  in a state that allows an external command to steer. Gate on this: if it is not
  ready, your command moves nothing.
- `estimated_curvature()` — `Some(c)` with the curvature the wheels are actually
  producing now, or `None` if the steering reports none. Never assume the machine
  reached the curvature you asked for — read it back here.

```rust,ignore
{{#include ../../../examples/guidance_autosteer.rs:feedback}}
```

## 4. React to the machine-info event

If you run an async event loop, each inbound Agricultural Guidance Machine Info
surfaces as an event rather than something you poll for. Match on it to see who
sent it, the estimated curvature, the readiness flag, and the raw limit status (0 =
not limited; non-zero = at a limit or fault):

```rust,ignore
use machbus::session::{Event, GuidanceEvent};

match event {
    Event::Guidance(GuidanceEvent::MachineInfo {
        source,
        estimated_curvature,
        steering_ready,
        limit_status,
    }) => {
        if !steering_ready || limit_status != 0 {
            // not in a state to follow — stop asserting intent.
        }
        // otherwise: compare estimated_curvature against what you commanded.
    }
    _ => {}
}
```

## Path → curvature is your job

The plugin moves a curvature value; it does not produce one. Each control cycle,
your application looks at the planned line, the current GNSS position and heading,
and the cross-track error, and computes the single curvature that steers the
machine back onto the line. That is a pure-pursuit or Stanley tracker, and it lives
in your code, not in machbus. See [Serial GNSS](serial-gnss.md) and
[NMEA 2000](nmea-2000.md) for getting position and heading into the stack, and
[TC geo / prescriptions](tc-geo-prescription.md) for where planned lines come from.

## Other languages

The same surface is exposed in the C and Python bindings as session-level
`guidance_*` functions/methods, enabled with an `enable_guidance` flag when the
session is built. The shape mirrors the Rust plugin: command by curvature, radius,
or `(v, ω)` twist (`guidance_command_velocity`), command straight, and read back
estimated curvature and readiness.

## Validate locally

```sh
cargo run --example guidance_autosteer
make test
```

The example claims an address, commands a 50 m-radius turn, prints the resulting
Guidance System Command frame, and reads back the steering readiness and estimated
curvature.

## What this proves / does not prove

Proves: the `Guidance` plugin commands curvature on PGN 0xAD00 and decodes
Agricultural Guidance Machine Info from PGN 0xAC00 into typed feedback.

Does not prove: anything about closed-loop steering, path planning, operator
supervision, actuator safety, real-machine timing, interoperability with a specific
steering system, or any certification. machbus is not a safety system and is not
certified for steering of any kind.

## See also

- [Automatic guidance (autosteer)](../standards/autosteer.md) — the curvature model
  and the two PGNs, explained.
- [TIM and automation](tim.md) — steering under granted, revocable authority.
- [Tractor ECU](tractor-ecu.md) — how a tractor advertises it can be steered.
- [Serial GNSS](serial-gnss.md) — getting position and heading into the stack.
