# Automatic guidance (autosteer)

The first question everyone asks about ISOBUS autosteer is: *do I send points,
angles, or velocities?* The answer is **none of those** — you send a desired path
**curvature**.

## Points vs. angles vs. curvature

Three things you might imagine sending to make a tractor steer itself, and why
only one of them is right:

- **Not waypoints.** You do not stream a list of latitude/longitude points and let
  the tractor figure out the geometry. The bus has no message for "drive to these
  coordinates."
- **Not a raw steering angle.** You do not command the wheels directly to N degrees.
  The right wheel angle depends on the machine's geometry and changes with speed; an
  angle that holds a line at 4 km/h would oversteer at 12 km/h.
- **Curvature.** You send the desired **curvature** of the path: how tightly the
  machine should be turning, expressed in **1/km** — the inverse of the turn radius.
  Zero curvature is dead straight. A larger magnitude is a tighter turn, and the
  sign tells the steering which way to turn. A 50 m radius is `1000 / 50 = 20` 1/km.

Curvature is the natural interface because it is independent of speed and of the
machine's steering linkage. The tractor's steering ECU takes the commanded
curvature and closes the loop on its own wheels — it owns the actuator, the
mechanical limits, and the speed-dependent geometry. The guidance controller only
has to say *how hard to turn*, not *how to move the wheels*.

**Speed is a separate concern.** A curvature command says nothing about how fast
the machine travels. The tractor owns its speed. If you also want to influence
speed, that is a different facility (see [TIM](tim.md)) and a different message.
Keep the two ideas apart: guidance is geometry, not throttle.

**Turning a path into curvature is the application's job.** Each control cycle,
something has to look at the planned line, the current GNSS position and heading,
and the cross-track error, then compute the single curvature value that steers the
machine back onto the line. That control law — a pure-pursuit or Stanley tracker,
typically — lives in *your* application. machbus does not plan paths or run the
tracker; it carries the resulting curvature command onto the wire and decodes what
comes back.

## Thinking in (v, ω): the robotics twist

If you come from mobile robotics, you command a body with a **twist**: a linear
velocity `v` and an angular (yaw) velocity `ω`. Curvature is not a rejection of that
model — it *is* that model with the speed factored out:

```
κ = ω / v
```

A robot uses `(v, ω)` because one controller owns both steering and throttle.
ISOBUS deliberately splits them: the guidance message carries only the
speed-independent geometry (curvature), because on a tractor the steering authority
and the speed authority are usually different systems — and the operator often keeps
speed. Dividing `ω` by `v` is exactly what removes the speed dependence: a 50 m
circle stays a 50 m circle whether you crawl or fly. (This is also why a raw yaw
rate `ω` alone would be the wrong thing to send — the same `ω` is a different arc at
every speed.)

You can still command in twist terms when it is convenient. machbus's `(v, ω)` call
computes `κ = ω / v` and sends it as the Guidance System Command (PGN 0xAD00), and
**also** sends `v` as a Machine Selected Speed Command (PGN 0xFD43) — so the two
separate authorities each receive the message they expect, from one call. A
near-zero `v` cannot define a forward path curvature, so it commands straight. See
the [Guidance tutorial](../tutorials/guidance.md).

## The two messages

Autosteer is a two-way conversation between the **guidance controller** (the thing
deciding where to go) and the tractor's **steering ECU** (the thing that moves the
wheels). Two messages carry it, each in one direction.

```
   guidance controller                         steering ECU (tractor)
   (your app: path + GNSS → curvature)          (actuator + safety logic)
        │                                                │
        │   Guidance System Command (PGN 0xAD00)         │
        │   commanded curvature + intent to steer  ───►  │  inside limits?
        │                                                │  engaged / allowed?
        │                                                │
        │ ◄─── Agricultural Guidance Machine Info        │
        │      (PGN 0xAC00): estimated curvature,        │
        │      steering readiness, limit status          │
        ▼                                                ▼
   adjust the request next cycle              steer, refuse, or drop out
```

### Guidance System Command — PGN 0xAD00

Direction: **guidance controller → tractor.** This is the command. It carries the
**commanded curvature** plus a small readiness/intent signal that says whether the
controller actually wants to steer right now, or is merely reporting a curvature
without asking to take the wheel. A curvature number on its own is never a request
to steer — the intent flag is the engage signal. The controller resends this
message on a fixed cadence; it is a heartbeat, not a one-shot.

### Agricultural Guidance Machine Info — PGN 0xAC00

Direction: **steering ECU → bus.** This is the feedback. The steering system
broadcasts:

- its **estimated actual curvature** — what the wheels are really producing now,
  which is not always what you asked for;
- its **steering-system readiness** — whether the system is engaged and in a state
  that allows an external command to steer it;
- a **guidance limit status** — whether the command is being clamped, the system
  is at a limit, or there is a fault.

You command through 0xAD00 and you always verify through 0xAC00. Never assume the
machine reached the curvature you requested — read back the estimated curvature.

## Readiness, limits, and operator override

Autosteer is **operator-supervised, not autonomous.** A person is in the seat and
is expected to stay ready to take over. Several gates sit between your command and
the wheels actually moving:

- **Readiness.** The steering system reports whether it is ready and engaged. If it
  is not ready, your command will not move anything no matter how often you send it.
  Gate on readiness before you treat a command as effective.
- **Limit status.** The steering enforces its own curvature and rate limits. A
  request beyond what the machine can do is clamped, and the limit status tells you
  so. A non-recoverable fault in that status means the system has given up — stop
  asking it to steer.
- **Operator override.** The operator can always take the wheel. Touching the
  steering input drops the system out of automatic mode immediately. This is a hard
  rule of the design, not an optional courtesy: when the operator intervenes, your
  command no longer governs the machine, and you must stop asserting intent.

The discipline that overrides every optimisation: **honour a dropout at once and
respect the steering system's limits.** The steering ECU is the authority on its
own safety. Your guidance controller defers to it, every cycle.

## Where the tractor advertises that it can steer

A tractor does not silently accept guidance commands. Under **ISO 11783-9** (the
tractor ECU), a tractor advertises the facilities it offers, and a
guidance/steering-readiness facility is one of them. A guidance controller can read
that advertisement to know, before it commands anything, whether this tractor is
even capable of being steered over the bus.

On top of the raw messages sits the **AEF TIM / automation layer**, which treats
steering as an **authority-controlled automated function**. Authority to steer must
be granted, and it comes with operator-override interlocks: the same principle as
the wheel-touch dropout, formalised as part of the automation contract. See
[TIM](tim.md) for how the tractor grants and revokes that authority.

## machbus is not a safety system

machbus encodes and decodes these messages. It does not plan paths, does not close
the steering loop, does not supervise the operator, and carries no ISO/SAE/AEF
certification. Nothing here makes a machine safe for unattended steering. Real
deployment needs official standards, functional-safety engineering, qualified
hardware, and interoperability evidence this crate does not provide.

## From concept to code

The high-level `session::plugins::Guidance` plugin is the whole surface: command a
curvature, command by radius, command straight, and read back the steering system's
estimated curvature, readiness, and limit status. It is also exposed in the C and
Python bindings (session-level `guidance_*` functions/methods, behind an
`enable_guidance` flag).

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Commanding a path by curvature | `session::plugins::Guidance` (`command_curvature`, `command_radius`, `command_straight`) | [Guidance tutorial](../tutorials/guidance.md) |
| Commanding in robotics (v, ω) terms | `Guidance::command_velocity` (sends curvature on 0xAD00 + speed on 0xFD43) | [Guidance tutorial](../tutorials/guidance.md) |
| Reading the steering ECU's feedback | `Guidance::estimated_curvature`, `is_steering_ready`, `Event::Guidance` | [Guidance tutorial](../tutorials/guidance.md) |
| The tractor advertising it can steer | ISO 11783-9 facilities | [ISO 11783-9 — the tractor ECU](iso11783-tractor-ecu.md) |
| Steering as a granted, revocable authority | `session::plugins::Tim` | [TIM (AEF)](tim.md) |

## See also

- [Guidance tutorial](../tutorials/guidance.md) — the `Guidance` plugin, end to end.
- [ISO 11783-9 — the tractor ECU](iso11783-tractor-ecu.md) — what the tractor advertises.
- [TIM (AEF)](tim.md) — steering under granted authority.
- [Positioning: NMEA and GNSS](positioning.md) — where the position and heading that
  feed your path tracker come from.
