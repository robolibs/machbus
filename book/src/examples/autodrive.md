# AutoDrive example

The driving loop `machbus drive keyboard` runs, with the terminal taken out so
it is readable and runnable offline.

```sh
cargo run --example autodrive_keyboard
```

Look for:

- `examples/autodrive_keyboard.rs`

## Expected output shape

```text
=== AutoDrive: steering + speed behind one lifecycle ===

[refused] arm before any ECU answered: link_down
[engaged] status = ActiveNotLimited
[heartbeat] 5 guidance commands in 500 ms of driving
[released] status = Fault   stop = Some("operator_override")
[refused] engage while latched: stop_latched
[cleared] latch released; re-engage is allowed again
```

Each line is one of the four things that are easy to get wrong:

1. **`link_down`** — arming before any steering ECU answers is refused, and the
   refusal names the precondition. `AutoDrive` returns refusals rather than
   silently ignoring, because a client that asks to steer and is ignored cannot
   tell "commanded" from "declined".
2. **5 commands in 500 ms** — the 100 ms cadence (`MIN_TX_INTERVAL_MS`, the ISO
   11783-7 §5.2.7.2 minimum). The command *is* the heartbeat the steering ECU
   times out on, so it goes out even when the setpoint has not changed.
3. **`Fault` + `operator_override`** — releasing the dead-man disengages and
   falls back to `DriveCommand::halt()`. Losing the dead-man must land in the
   same place; see the [drive tool safety model](../tutorials/drive-tool.md).
4. **`stop_latched` then cleared** — the stop latches, so re-engaging is refused
   until an explicit, separate `clear_stop()`. Clearing a fault is not by itself
   consent to move.

The example feeds itself Agricultural Guidance Machine Info (PGN 0xAC00), because
`AutoDrive` will not engage without a steering ECU answering — that refusal is
the first line of output.

## What this proves

- `AutoDrive` gates arm/engage on preconditions and reports which one failed.
- The command cadence is a heartbeat at the conformance minimum.
- A safe stop latches and needs a deliberate clear.

## What this does not prove

Anything about closed-loop steering or speed control, path planning, operator
supervision, actuator safety, real-machine timing, interoperability with a
specific tractor, or whether a given tractor acts on an unauthenticated speed
command at all — see
[AutoDrive → Do I need TIM?](../tutorials/autodrive.md#do-i-need-tim). machbus
is not a safety system and is not certified.

## See also

- [AutoDrive (steering + speed)](../tutorials/autodrive.md) — the plugin.
- [`machbus drive` safety model](../tutorials/drive-tool.md) — the operator-input
  layer around it: dead-man, arm latch, and losing the controller.
