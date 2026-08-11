# `machbus drive` — the operator safety model

`machbus drive` is the CLI's driving mode: it owns an [`AutoDrive`](autodrive.md)
plugin and turns operator input into curvature and speed commands on a real bus.

```sh
machbus drive keyboard            # WASD, TUI
machbus drive keyboard --daemon   # headless
machbus drive joystick            # gamepad, TUI
machbus drive joystick --daemon   # headless
```

Both modes share the same physics, the same arm latch and the same single gate
to the bus. They differ only in **what the operator holds**.

> **This is a development and bench tool.** machbus is not a safety system and
> is not certified. The model below is defence in depth against the obvious
> failure modes, not a substitute for a rated interlock.

## Three layers

Nothing reaches the wire unless all three agree.

```
   operator input        │  dead-man held?  arm latch satisfied?
   ───────────────────── │
   AutoDrive plugin      │  preconditions met?  (link, lockout, engage switch,
                         │                       no latched stop, no GNSS hazard)
   ───────────────────── │
   stop latch            │  nothing tripped?
   ───────────────────── ▼
                      PGN 0xAD00 + 0xFD43
```

Layers 2 and 3 are documented on the [AutoDrive page](autodrive.md#every-way-it-stops).
This page is about layer 1 — the part the tool owns.

## The dead-man and the arm latch

A dead-man switch has one job: **losing it must read as released.** Both modes
implement the same two-stage latch.

| | Joystick | Keyboard |
| --- | --- | --- |
| Dead-man | **R2 held** (analog, > 0.3) | **SPACE held** (auto-repeat) |
| Arm | hold R2 for `ARM_HOLD_SECS` (1.5 s) | hold SPACE for 1.5 s |
| Release | let go of R2 | stop pressing SPACE |
| Emergency stop | **A / Cross** — zero motion, disarm | **ENTER** — zero motion, disarm |
| Clear a latched stop | complete a fresh 1.5 s arm hold | **C** |
| Re-arm after a disarm | must release R2 fully first | must release SPACE fully first |

**Arming is deliberate.** Until the dead-man has been held continuously for
1.5 s, nothing is commanded — no throttle, no steer. The UI shows a fill bar
(`⚠ HOLD R2 TO ARM [███░░░]`) so the operator can see the hold accumulating.
This exists so that a controller left with a trigger pressed, or a stuck key,
cannot steer the moment the tool starts.

**Re-arming is deliberate too.** `disarm()` sets a block that is only released
once the dead-man is seen *fully released*. Hitting emergency stop while still
holding R2 cannot silently re-arm the instant the latch clears — the UI says
`⚠ RELEASE R2 TO RE-ARM` until you let go.

## Losing the controller counts as releasing it

This is the failure that motivated most of the model.

**Joystick.** gilrs emits no button-release events when a pad disconnects. The
tool therefore treats a disconnect — unplugged cable, flat battery, dropped
Bluetooth link — as a full release: it clears the active pad, zeroes every axis
and button, zeroes motion, and **disarms**. There is also a liveness check
(`gamepad.is_connected()`) each poll, so an unplug that produces no event at all
is still caught.

Before that, the last trigger value persisted: unplugging the pad while R2 was
held left the dead-man reading *pressed* indefinitely, and the machine kept
steering at its last curvature.

> Note that `AutoDrive`'s `CommandStale` watchdog does **not** cover this. That
> watchdog fires when the application stops refreshing the setpoint — but the
> tool's loop was still running and still refreshing, so the watchdog stayed
> fed. A dead-man that cannot be released is not protected by a liveness timer
> on the thing holding it.

**Keyboard.** SPACE is a *held* key, not a toggle. Each press refreshes a
window; if no press arrives within `DEADMAN_WINDOW_S`, the dead-man reads
released and the tool disengages. A released dead-man also zeroes the W/A/S/D
key intensities, so a still-decaying keypress cannot keep feeding the physics —
the setpoint decays to a stop rather than freezing at its last value.

### The keyboard window is coarser, on purpose

`DEADMAN_WINDOW_S` is **0.9 s**, and that is a compromise you should understand
before driving anything real from a keyboard.

A terminal reports key **repeats**, not holds. X11 defaults to roughly a 660 ms
delay before the first repeat, then a fast rate. The window has to outlast that
initial gap or holding SPACE would drop out immediately after the first press.

So the keyboard dead-man can take up to ~0.9 s to notice you let go, where the
joystick's analog trigger is effectively instant. It is **bounded and correct** —
not "forever", which is what a toggle gives you — but it is not equivalent.

**Use the joystick on a real machine.** The keyboard mode is for a bench, a
virtual CAN, or a replay.

(The fix for this is the kitty keyboard protocol, which reports true release
events. It is terminal-dependent, so it would need a runtime probe and a
fallback to the window above.)

## Refusals are shown, not swallowed

`AutoDrive` refuses rather than silently ignoring, so the tool surfaces the
refusal. The telemetry pane carries a line with the ISO 11783-7 Table 45
automation status, any latched stop, and the last refusal:

```
aut  ACTIVE:LIM-HI   ■ STOP:position_stale   refused:stop_condition_live
```

- **status** — `ACTIVE`, `ACTIVE:LIM-HI` / `LIM-LO` (the steering ECU reporting
  saturation, which is the anti-windup signal), `READY`, `FAULT`, `not-ready`.
- **STOP** — the latched trigger. Latching: it stays until explicitly cleared.
- **refused** — why the last arm/engage/command was rejected, e.g.
  `link_down`, `mechanical_lockout`, `operator_not_engaged`,
  `stop_condition_live`, `curvature_out_of_range`.

An operator pressing engage and seeing nothing happen is the failure mode this
line exists to prevent.

## Clearing a latched stop

`AutoDrive` **latches**: once a stop trips, `engage()` refuses until the latch is
explicitly cleared. Without an affordance for that, the first stop would end the
session's ability to drive.

- **Keyboard — `C`.** A dedicated key, deliberately not folded into engage:
  clearing a fault is not by itself consent to move.
- **Joystick — a fresh arm hold.** The pad has no spare button, so completing the
  1.5 s hold *is* the gesture: you already released the dead-man and deliberately
  held it again.

Either way `AutoDrive::clear_stop` still refuses with `stop_condition_live` while
the Auxiliary Shortcut Button is held or a GNSS hazard is live, so neither path
can re-arm against a condition that is still asserted.

## Key and button map

**Keyboard**

| Key | Action |
| --- | --- |
| `SPACE` | **dead-man — hold to drive** |
| `W` / `S` | accelerate / brake |
| `A` / `D` | steer left / right |
| `ENTER` | emergency stop + disarm |
| `C` | clear a latched safe stop |
| `I` / `K` | speed limit ± |
| `H` / `J` | hitch raise / lower |
| `P` / `O` | PTO on / off |
| `X` | cycle counter multiplier |
| `Q`, `Ctrl+C` | quit |

**Joystick**

| Control | Action |
| --- | --- |
| `R2` | **dead-man — hold to drive** (hold 1.5 s to arm) |
| Left stick | throttle (Y) and steer (X) |
| `A` / Cross | emergency stop + disarm |
| `B` / Circle | hitch raise |
| `X` / Square | hitch lower |
| `Y` / Triangle | PTO engage |
| D-pad ↑ / ↓ | speed limit ± |
| `Start` | cycle counter multiplier |

## Watching without driving

`machbus live` has an **AutoDrive tab** (hotkey `6`, or `-T autodrive`) that
decodes the same conversation read-only — commanded vs estimated curvature, the
intent-to-steer flag, readiness, lockout, limit status and speed, each with a
staleness age. It transmits nothing. Use it to see what another controller is
doing, or to check your own commands from a second terminal.

## What this proves / does not prove

Proves: the tool gates commands behind a held dead-man and a deliberate arm
latch, treats a lost controller as a release, and surfaces every refusal and
latched stop.

Does not prove: any safety rating, any certification, real-machine timing, that
0.9 s is an adequate reaction window for your application, or that the layers
above it behave as documented on a specific tractor. This is a development tool.

## See also

- [AutoDrive (steering + speed)](autodrive.md) — the plugin, its stop triggers
  and the heartbeat contract.
- [Automatic guidance](../standards/automatic-guidance.md) — the curvature model.
