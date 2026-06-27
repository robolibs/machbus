# Sequence Control

A **sequence** is a stored series of implement actions that a master can replay
on operator command. Think of a headland turn: lift the hitch, fold the
sections, stop the PTO. Doing that by hand every pass is tedious and
error-prone. Sequence Control (defined in ISO 11783-14) lets one control
function — the **master** — drive that series step by step, while each
participating implement — the **client** — actually performs the actions. This
tutorial explains the master/client relationship, the full lifecycle as a state
machine, the messages involved, what `machbus` supports today versus what it
intentionally rejects, and how to drive it from the session facade.

## Why this exists

On a working machine, the same handful of operator actions repeat at every
headland or waterway. The standard's idea is to let the operator perform those
actions once and have a master replay them afterward — each time the trigger
point is reached — so the machine repeats the operation without the operator
re-doing every input. The actions themselves still live in the implements; the
master only coordinates *when* each one fires and *who has acknowledged* it.

`machbus` implements the **playback** half of that picture: a master that drives
a predefined list of steps through ready, active, pause, resume, and abort, and
a client that follows along and reports what it is doing. The recording half —
capturing a fresh sequence from live operator inputs — is a deliberate gap; see
[What this supports](#what-this-supports-vs-rejects) below.

## Mental model

```
   master (SCM)                         client (SCC)
  ┌──────────────┐                     ┌──────────────┐
  │ list of steps│   master status     │ executes the │
  │  + lifecycle │ ──────────────────► │   client     │
  │   timers     │                     │  function    │
  │              │ ◄────────────────── │              │
  └──────────────┘   client status     └──────────────┘
        │                                     │
        │  "start step 7"                     │ "ready / playing back /
        │  "pause" / "resume" / "abort"       │  aborting" + step echo
        ▼                                     ▼
```

Two control functions, two status messages, one shared cadence. The master
broadcasts its lifecycle state and the current step on `PGN_SC_MASTER_STATUS`.
Each client broadcasts its own state and the step it is acting on via
`PGN_SC_CLIENT_STATUS`. Neither side commands the other with a one-shot request:
the whole exchange is a pair of periodic status messages that each side reads
and reacts to. Clients are independent of one another — they only watch the
master, never each other.

## The master/client relationship

| Role | What it owns | What it broadcasts |
| --- | --- | --- |
| Master (SCM) | The list of steps, the lifecycle, the ready/active timeouts, who has acknowledged. | Its master state, the current step id, and busy flags. |
| Client (SCC) | The actual implement function and whether it is busy. | Its client state, the step it is executing, and a function-error byte. |

Only one master is meant to be active at a time. A client follows whichever
active master it sees and mirrors the sequence state back so the master can tell
that the step landed. The master treats a step as truly active only once enough
clients have acknowledged it — configurable through
`SCMasterConfig::required_client_count` (a `0` is treated as `1`, so a sequence
never starts with no participation).

## Lifecycle as a state machine

`machbus` runs both ends on one unified internal state, `sc::SCState`, with these
variants: `Idle`, `Ready`, `Active`, `Paused`, `Complete`, `Error`. The master
and client map this onto the wire-level master state (`SCMasterState`) and
sequence state (`SCSequenceState`) when they encode a status frame.

```
                 add_step()*            (client ack ≥ required_count)
   ┌──────┐   ┌──────────────┐  start()  ┌───────┐ ─────────────────► ┌────────┐
   │ Idle │──►│  steps loaded│ ────────► │ Ready │                    │ Active │
   └──────┘   └──────────────┘           └───────┘ ◄───── resume() ── └────────┘
      ▲                                      │  │          (Paused)        │ │
      │                                      │  │ ready-timeout            │ │ pause()
      │                                      │  └──────────────┐          │ ▼
      │                                      │                 │      ┌────────┐
      │                                      ▼                 │      │ Paused │
      │  (master Idle/Inactive seen)    ┌───────┐              │      └────────┘
      └─────────────────────────────── │ Error │ ◄────────────┘
                                        └───────┘   abort() / timeout /
   step_completed() advances the step;             client abort
   last step ──► ┌──────────┐
                 │ Complete │
                 └──────────┘
   * steps may only be added while Idle.
```

The transitions, in words:

1. **Load and start.** You add steps while the master is `Idle` (`add_step`
   refuses once you leave `Idle`). `start` requires at least one step and moves
   the master to `Ready`, clearing its ready/active timers and its
   ready/ack client sets.
2. **Ready → Active.** The master sits in `Ready` advertising the sequence as
   ready. As clients report Ready, the master collects their addresses; once it
   has `required_client_count` unique clients it transitions to `Active` and
   emits the first step. A client moves itself to `Ready` the moment it sees an
   active master advertising a ready sequence.
3. **A step advances.** In `Active`, the master broadcasts the current step's
   wire-visible id. The client executes and acknowledges by playing back that
   same step number. When the application calls `step_completed(step_id)` on the
   master, the step is marked done, the index advances, and either the next step
   is emitted or the master transitions to `Complete` after the last one.
4. **Pause / resume.** The master may `pause` only from `Active` (→ `Paused`) and
   `resume` only from `Paused` (→ `Active`, re-arming the active timer and
   clearing the per-step ack set). A client infers pause when an active master
   drops the sequence back to ready while the client was active, and infers
   resume when playback returns.
5. **Abort and errors.** `abort` from any live state forces `Error` and makes the
   very next status carry an Abort sequence state, so the abort is visible on the
   bus, not just locally. Ready and active timeouts also land in `Error`, as does
   a client reporting Abort.

## Anatomy of the messages

Both status messages are classic 8-byte CAN payloads; `machbus` rejects shorter
or overlong reassembled buffers rather than prefix-decoding them.

**Master status** (`PGN_SC_MASTER_STATUS`) carries:

- byte 0 — a fixed message code (`SC_MSG_CODE_MASTER`) that identifies the frame;
- byte 1 — the master state (`SCMasterState`: `Inactive`, `Active`, or the
  reserved `Initialization`);
- byte 2 — the current sequence/step number, or the `0xFF`
  not-applicable sentinel when the sequence is merely ready;
- byte 3 — the sequence state (`SCSequenceState`);
- byte 4 — two busy flags (non-volatile-memory busy, SCD-parsing busy);
- bytes 5–7 — reserved, held at `0xFF`.

**Client status** (`PGN_SC_CLIENT_STATUS`) mirrors it: a client message code in
byte 0, the client state (`SCClientState`: `Disabled`, `Enabled`, or reserved
`Initialization`) in byte 1, the echoed step number in byte 2, the sequence state
in byte 3, and a function-error byte (`SCClientFuncError`: `NoErrors`,
`NoChange`, `Changed`, `NeedsConfirm`) in byte 4, with the same reserved tail.

A few rules the decoders enforce, all from `sc::types`:

- The selected sequence number is a small standard-defined wire range. Library
  step ids therefore run `0..=SC_MAX_SEQUENCE_STEP_ID` (`0x31`); higher byte
  values are reserved, and `0xFF` is the ready / not-applicable sentinel.
  `add_step` rejects a larger id, and rejects duplicate ids in the same
  sequence.
- A Ready sequence state must carry the `0xFF` sentinel; a PlayBack state must
  not. Mismatched combinations are rejected.
- The reserved tail bytes must all be `0xFF`, or the frame is rejected.

## What this supports vs rejects

This is the part to be precise about. `SCSequenceState` *names* all of the
standard's sequence states — `Reserved`, `Ready`, `Recording`,
`RecordingCompletion`, `PlayBack`, `Abort` — so the decoders can recognize them on
the wire. But only **`Ready`, `PlayBack`, and `Abort`** are *supported* as active
behavior. When an active master or an enabled client advertises `Recording` or
`RecordingCompletion`, `machbus` treats it as an unsupported sequence state and
rejects the frame; the local state machine does not move. Likewise, the
`Initialization` master and client states are recognized as names but rejected as
inputs.

The practical consequence: **`machbus` plays back sequences; it does not record
them.** There is no API to capture a fresh sequence from live operator inputs.
You define steps yourself (as `SequenceStep` values) and the master replays them.
This is a current limitation, stated plainly rather than implied — the recording
phase, the SCD object definitions, and the VT object pools that a full
implementation would use are out of scope here.

## Doing it with machbus

There are two layers. The pump-style `sc::SCMaster` / `sc::SCClient` are codecs
plus state machines: you feed them frames and elapsed time and they hand back
`[u8; 8]` payloads to transmit. The session facade wires those into the unified
event loop so you do not dispatch frames by hand.

You plug one or both roles into a `Session` at build time:

```rust
use machbus::session::{Session, EndpointTransport, plugins::ScMaster};

let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(ScMaster::new(SCMasterConfig::default()))
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;

// load steps and start the sequence through fine control:
ctrl.with_mut::<ScMaster, _>(|m| {
    m.add_step(SequenceStep { step_id: 7, ..Default::default() })?;
    m.start()
})?;
```

You reach the master through `ctrl.with_mut::<ScMaster, _>(|m| ...)`, which
returns the master so you can call `add_step`, `start`, `pause`, `resume`,
`abort`, `step_completed`, and `state`. A client plugs
`ScClient::new(...)` and reaches it with `ctrl.with_mut::<ScClient, _>(|c| ...)`,
exposing `state`, `is_busy`, `set_busy`, and `report_step_complete`.

The session owns the I/O: inbound SC status PGNs are routed in, the master/client
update functions run on each `driver.poll()?`, and any emitted payloads are
broadcast for you. You react through typed events with `ctrl.drain::<ScEvent>()`,
described next.

Sequence Control has a natural **integration point** with section state: after a
master announces a wire-visible step id, you feed that id through a
`SectionRouter` to toggle implement sections. You drain the master's
`ScEvent::MasterStepStarted { step_id }`, bind sections to VT indicator objects,
and map step ids such as `7` and `8` onto section changes. The router
deduplicates (setting the same value twice is a no-op) and skips the VT push when
no VT plugin is present.

## Events and responsibilities

The session translates internal master/client events into `ScEvent` variants you
drain from the queue:

| Event | Side | Meaning |
| --- | --- | --- |
| `MasterStateChanged { from, to }` | master | The master's lifecycle state moved. |
| `MasterStepStarted { step_id }` | master | A new step was dispatched. |
| `MasterStepCompleted { step_id }` | master | A step was recorded complete. |
| `MasterSequenceComplete` | master | The last step finished. |
| `MasterTimeout { reason }` | master | A ready or active timeout struck. |
| `MasterClientStatus { source, state }` | master | A valid client status arrived. |
| `ClientStateChanged { from, to }` | client | The client's state moved after a master status or timeout. |
| `ClientSequenceStart` | client | The client observed a new sequence start. |
| `ClientStepRequest { step_id }` | client | The client was asked to execute a step. |
| `ClientPause` / `ClientResume` | client | The client inferred pause or resume. |
| `ClientAbort` | client | The client aborted or entered error. |

Your responsibilities split cleanly by role. As a **master** application you load
steps, call `start`, and — crucially — call `step_completed(step_id)` once your
logic decides a step is done; the master does not advance on its own. As a
**client** application you act on `ClientStepRequest`, set `set_busy(true)` while
the function is in progress, and call `report_step_complete(step_id)` to
acknowledge. The one rule the client enforces for you: `report_step_complete`
only works while the client is `Active` and only for the step it was actually
asked to run.

## Edge cases and failures

- **Abort mid-sequence.** Calling `abort` from `Ready`, `Active`, or `Paused`
  forces `Error` and emits a visible Abort status immediately. Calling it from
  `Idle`, `Complete`, or `Error` is rejected — there is nothing to abort. The
  standard's intent is that an abort halts motion; your application is
  responsible for actually bringing the machine to a safe state when it sees the
  abort.
- **Client not ready.** If a client never reports Ready (or fewer than
  `required_client_count` clients do), the master never leaves `Ready`. The
  ready timeout eventually fires and drops it to `Error`.
- **Client stuck busy.** A client that stays busy past its
  `busy_pause_timeout_ms` while Active or Paused drives itself to `Error`, emits
  an Abort status, and the master picks that up as a sequence-level abort.
- **No ack for a step.** In `Active`, if the required clients do not acknowledge
  the current step within the active timeout, the master times out to `Error`.
  An ack only counts when it echoes the *current* step number.
- **Unsupported state on the wire.** A frame advertising `Recording`,
  `RecordingCompletion`, `Initialization`, reserved state bytes, bad busy bits, a
  wrong message code, a wrong length, or a non-`0xFF` tail is rejected without
  moving the state machine. Use the `try_*` handlers (`try_handle_master_status`
  / `try_handle_client_status`) when you need the explicit validation error
  rather than a silent no-op.
- **Master disappears.** A client that sees the master go Inactive/Idle returns
  to `Idle` and emits an abort event, so a vanished master is not mistaken for a
  paused one.

## Advanced

- **Multi-client coordination.** Set `required_client_count` above 1 to demand
  that several distinct implements participate. The master keys on client source
  address, so a duplicate Ready from one client does not count as a second
  participant, and every required client must echo the *current* step before its
  ack timer is disarmed.
- **Safe state on abort.** The library makes the abort visible; it does not move
  hydraulics. Treat `MasterTimeout`, `ClientAbort`, and any transition into
  `Error` as a cue to put your functions in a safe state. See
  [Shortcut Button and safe-state thinking](../standards/implement-and-services.md).
- **Timing and cadence.** The status messages are paced: clients hold a minimum
  spacing between sends (`min_status_spacing_ms`) and defer a status to the next
  `update` if a send would come too soon. The master emits on its
  `status_interval_ms`. The standard expects a faster cadence in active states
  than in ready; the timeout and spacing constants in `sc::types` encode those
  expectations, and `SCMasterConfig` / `SCClientConfig` let you tune them.
- **Session facade vs the bare codecs.** Use the `ScMaster` / `ScClient` plugins
  for applications — they route frames, run the update loop, and fan out events.
  Drop to `SCMaster` / `SCClient` directly for unit tests or tightly controlled
  embedded loops where you own every elapsed millisecond.

## Validate locally

```sh
make test
```

The session tests run entirely in software: they plug the SC roles into a
`Session`, bind sections, and apply step ids through the router. The unit tests
under `sc::master` and
`sc::client` cover the lifecycle directly — ready-to-active on client ack,
step advance and completion, pause/resume, every timeout path, the abort paths,
the multi-client count, and the rejection of unsupported and malformed frames.

## What this proves / does not prove

Proves: the playback lifecycle — ready, active, step advance, pause, resume,
complete, and the abort/timeout error paths — behaves deterministically in
software, the master/client status messages round-trip, and the malformed and
unsupported-state frames are rejected rather than silently accepted.

Does not prove: recording of live operator sequences (not implemented),
the SCD object and VT object-pool machinery a full system uses, real-hardware
timing, interoperability with a specific third-party master or implement, or any
conformance/certification claim. Those still require official standards, real
hardware, and interoperability evidence.

## See also

- [Sequence Control and TIM](../standards/iso11783-sequence-control.md) — how
  sequence playback relates to TIM (automated control of tractor functions by an
  implement) and to tractor-implement management.
- [Shortcut Button and safe-state thinking](../standards/implement-and-services.md)
  — what to do when a sequence aborts or a function must stop.
