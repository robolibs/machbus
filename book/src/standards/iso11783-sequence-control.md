# ISO 11783-14 — sequence control

Part 14 automates *sequences* of actions. Its most familiar face is headland
management: at the end of a pass, the machine raises the hitch, folds a marker,
disengages sections, and lifts the implement — in a defined order, from one operator
action or a position trigger. Sequence Control is the protocol that coordinates that.

## Why this exists

Turning at the headland is a choreography of half a dozen actuator actions that must
happen in the right order every time. Doing it by hand, every pass, is tiring and
error-prone. Sequence Control lets the machine record the choreography once and replay
it reliably, with each participating ECU executing the steps it owns.

## Master and client

```
   MASTER (runs the saved sequence)            CLIENTS (execute their steps)
     │ ── start sequence ──────────────────►  (all)
     │ ── step 3: "raise rear hitch" ──────►  hitch ECU executes
     │ ◄── step 3 complete ─────────────────  hitch ECU reports
     │ ── step 4: "disengage section bank" ►  section ECU executes
     │ ◄── step 4 complete ─────────────────
     │     … pause / resume / abort as needed …
     │ ── sequence complete ───────────────►
```

The master drives ordering and timeouts; each client executes the steps addressed to
it and reports progress. The protocol carries pause, resume, and abort so a sequence
can be interrupted safely.

## How machbus expresses it

machbus implements both roles: `ScMaster` runs and broadcasts the sequence;
`ScClient` executes steps and reports completion. Both surface their lifecycle as
`ScEvent` variants (state changes, step started/completed, sequence complete, timeout,
pause/resume/abort).

```
   ctrl.with_mut::<ScMaster, _>(|m| { m.add_step(step)?; m.start() })?;
   // progress arrives as Event::Sc(ScEvent::MasterStepCompleted { step_id }) …
```

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Running a sequence (master) | `session::plugins::ScMaster` | [Sequence Control](../tutorials/sequence-control.md) |
| Executing steps (client) | `session::plugins::ScClient` | [Sequence Control](../tutorials/sequence-control.md) |

## Failure modes worth knowing

- **Step never completes** — a client that cannot perform a step must report, or the
  master times out; do not hang the sequence.
- **Unsafe abort** — aborting mid-sequence must leave the machine in a safe state, not
  half-folded.
- **Ordering assumptions** — steps run in the master's order; clients should not
  reorder.

## See also

- [TIM (AEF)](tim.md) — the authority model often paired with automated motion.
- [Implement control, the tractor ECU, and the rest](implement-and-services.md).
