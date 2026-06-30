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

## What each signal means, in plain terms

Two messages, a handful of fields. Here is what each field is actually telling you —
no jargon.

**On the command (PGN 0xAD00), controller → tractor:**

| Field | What it means |
| --- | --- |
| Commanded curvature | How hard to turn, in 1/km (0 = straight, sign = direction). |
| Curvature Command Status | The **engage request**. *Intended to steer* = "take the wheel"; *not intended to steer* = "I'm only reporting a number, don't steer." This single flag is the difference between a suggestion and a request — it is exactly what `engage()` and `disengage()` flip. |

**On the feedback (PGN 0xAC00), tractor → bus.** The steering ECU broadcasts this
every 100 ms the whole time it is powered. *Seeing this message on the bus at all is
the first signal that the ECU can be steered.* Inside it:

| Field | What it means | Value that means "good to steer" |
| --- | --- | --- |
| Steering System Readiness State | The headline "am I ready?" flag. | **On / active** = ready and engaged. Off/passive = not ready. |
| Mechanical Lockout | A physical safety cut-out (e.g. a lockout switch). | **Not active.** If it is Active, you cannot engage at all. |
| Remote Engage Switch Status | The operator's **arm switch** — most systems need the person in the seat to flip a switch or hold a button before autosteer is allowed to take the wheel. | **On / active** (operator has armed it). |
| Steering Input Position Status | Whether the operator's steering wheel is being moved — the basis for override detection. | _(informational)_ |
| Guidance Limit Status | Whether your command is being clamped, the system is at a limit, or has a non-recoverable fault. | **Not limited.** |
| Exit / reason code | *Why* the system is refusing or last dropped out (a diagnostic — see below). | **No reason / all clear.** |
| Estimated curvature | What the wheels are actually producing right now — not necessarily what you asked for. | _(always read it back; never assume)_ |

`is_steering_ready()` is exactly the "Steering System Readiness State == on/active"
check. For everything else, read the full record with `latest_machine_info()`.

### When it refuses: the exit / reason code

When the steering ECU will not — or will no longer — accept your commands, it says
why in the exit/reason code. The reasons a real system reports (from the AEF
automation guideline's external-guidance table) include:

- required level of operator presence/awareness not detected
- **operator override of function** — someone touched the wheel
- operator control not in a valid position
- **remote command timeout** — your command heartbeat stalled
- remote command out of range / invalid
- system not calibrated
- alternate guidance system active
- vehicle speed too high / too low
- transmission gear does not allow remote commands (park, etc.)

Treat any non-clear reason as "stop asserting intent and tell the operator," not as
something to retry blindly.

## The lifecycle: when each thing happens

Read the two messages in order and a normal engage → steer → release cycle looks
like this:

1. **Power on.** The steering ECU starts broadcasting Agricultural Guidance Machine
   Info (0xAC00) at 100 ms with readiness = *not ready*. Its mere presence tells your
   controller the machine is steerable. A deeper, up-front capability check is the
   **ISO 11783-9** tractor-facilities advertisement, which you can read *before* any
   guidance traffic to know the tractor supports being steered at all.
2. **Operator arms it.** The person in the seat flips the remote-engage switch (or
   holds the button). Remote Engage Switch Status goes on, and the system moves toward
   ready. Until this happens, *nothing your code sends will steer.*
3. **Controller asks for the wheel.** Your app calls `engage()` and starts sending
   the Guidance System Command (0xAD00) carrying the curvature **and** Curvature
   Command Status = *intended to steer*, on a fixed ~100 ms heartbeat.
4. **System engages.** With the operator armed, no lockout, speed in range, and the
   command stream alive, the steering ECU engages: readiness reports *on/active* and
   the estimated curvature begins tracking your command.
5. **Steady state.** Every cycle you recompute a curvature from your path + GNSS and
   resend it, and you read back readiness, limit status, and estimated curvature to
   confirm the machine is actually following.
6. **Drop out.** The instant the operator touches the wheel — or speed leaves the
   window, or your heartbeat stalls — the system leaves automatic mode, readiness
   drops to *not ready*, and the exit/reason code says why. Your controller must
   `disengage()` and stop asserting intent at once. It does not fight the operator.
7. **Release.** When you are done, call `disengage()`: the next command goes out with
   *not intended to steer*, and the operator has the wheel back.

The rule under all of it: **the steering ECU is the authority on its own safety. Your
controller asks, the tractor decides, and a dropout is honoured immediately** — every
cycle.

## Two layers of "is it allowed?"

The fields above are the **raw ISO 11783-7** handshake. On top of them sits the
**AEF TIM / automation layer**, which treats steering as an
**authority-controlled automated function**: authority to steer must be granted, and
it comes with the same operator-override interlocks formalised as part of the
automation contract. Both layers say the same thing — the operator and the tractor
stay in charge — at different levels of formality. See [TIM](tim.md) for how that
authority is granted and revoked.

## machbus is not a safety system

machbus encodes and decodes these messages. It does not plan paths, does not close
the steering loop, does not supervise the operator, and carries no ISO/SAE/AEF
certification. Nothing here makes a machine safe for unattended steering. Real
deployment needs official standards, functional-safety engineering, qualified
hardware, and interoperability evidence this crate does not provide.

## From concept to code

The high-level `session::plugins::Guidance` plugin is the whole surface: **engage**
(assert intent to steer) and **disengage**, command a curvature / radius / straight,
and read back the steering system's estimated curvature, readiness, and limit
status. It is also exposed in the C and Python bindings (session-level `guidance_*`
functions/methods, behind an `enable_guidance` flag).

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Asking for / releasing the wheel (intent to steer) | `Guidance::engage` / `Guidance::disengage` (sets the Curvature Command Status on 0xAD00) | [Guidance tutorial](../tutorials/guidance.md) |
| Commanding a path by curvature | `session::plugins::Guidance` (`command_curvature`, `command_radius`, `command_straight`) | [Guidance tutorial](../tutorials/guidance.md) |
| Commanding in robotics (v, ω) terms | `Guidance::command_velocity` (sends curvature on 0xAD00 + speed on 0xFD43) | [Guidance tutorial](../tutorials/guidance.md) |
| Reading the steering ECU's feedback | `Guidance::estimated_curvature`, `is_steering_ready`, `latest_machine_info`, `Event::Guidance` | [Guidance tutorial](../tutorials/guidance.md) |
| The tractor advertising it can steer | ISO 11783-9 facilities | [ISO 11783-9 — the tractor ECU](iso11783-tractor-ecu.md) |
| Steering as a granted, revocable authority | `session::plugins::Tim` | [TIM (AEF)](tim.md) |

## See also

- [Guidance tutorial](../tutorials/guidance.md) — the `Guidance` plugin, end to end.
- [ISO 11783-9 — the tractor ECU](iso11783-tractor-ecu.md) — what the tractor advertises.
- [TIM (AEF)](tim.md) — steering under granted authority.
- [Positioning: NMEA and GNSS](positioning.md) — where the position and heading that
  feed your path tracker come from.
