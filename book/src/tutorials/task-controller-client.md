# Task Controller client

A Task Controller (TC) client is the **implement side** of documented work. The
implement — a sprayer, a seeder, a spreader — joins the bus, finds the Task
Controller, tells it *what it is and what it can measure or control* by uploading
a **device description**, and then trades **process data** with the TC for as
long as the task runs: it reports measured values up, and it accepts setpoints
coming down. This page explains that conversation end to end and shows how to
drive it with `machbus` through `isobus::tc::TaskControllerClient` and the
[`TcClient`](../guide/session-facade.md) session plugin.

If you have read [Task Controller concepts](../standards/task-controller.md)
and [DDOP and process data](../standards/task-controller.md), this is
where those ideas become code. The TC server side is covered in
[Task Controller server](task-controller-server.md).

## Why this exists

A TC running a prescription needs to know the *shape* of every implement it
controls: how many booms and sections it has, which quantities it can report,
which it can be commanded to change, and the units and resolution of each. There
is no point sending a "set application rate" command to a device that cannot
report or act on a rate. So before any real work happens, the implement publishes
a self-describing model of itself — the **Device Descriptor Object Pool**, or
DDOP — and the TC keeps it. From then on, the two sides speak in compact
**process-data** messages keyed by an element number and a data dictionary
identifier (**DDI**), instead of re-sending the structure each time.

This is the same separation you see throughout ISOBUS: a one-time *description*
upload, then a long stream of small *data* messages against that description.

## Mental model

```
implement (TC client)                          Task Controller (server)
        │                                                │
        │  listen for the TC's periodic status  ◄────────┤  TC status
        │  announce "I am a working set"   ──────────────►
        │  ask "what version do you speak?" ─────────────►
        │                                   ◄──────────── version + capabilities
        │  "do you already have my DDOP?"   ─────────────►  (compare by label)
        │                                   ◄──────────── structure / loc. label
        │     ┌── label matches ──► skip upload, just activate
        │     └── no match / none ──► upload DDOP, then activate
        │  upload DDOP (if needed) ──────────────────────►
        │                                   ◄──────────── pool accepted / rejected
        │  activate pool ────────────────────────────────►
        │                                   ◄──────────── activated  → CONNECTED
        │                                                │
        │  report measured value ────────────────────────►   (task running)
        │                                   ◄──────────── request value / setpoint
        │  acknowledge / respond ────────────────────────►
```

The whole thing is event-driven and pump-style. The client never blocks: you
feed it inbound TC frames, you call `update`, and it hands you the outbound
frames to ship. The session does both halves for you on each `driver.poll()?`.

## Anatomy: the client and its pieces

`TaskControllerClient::new` takes a `TCClientConfig` (its one knob is
`timeout_ms`, the wait budget for each handshake reply; the default is six
seconds). You hand the client a built `DDOP` with `set_ddop`, and you read it
back with `ddop()`.

The client advertises what it can do through `TCClientCapabilities`: a protocol
`version`, a `max_boot_time`, an `options` bitmask, and counts of `booms`,
`sections`, and `channels`. These are answered when the TC asks the client for
its capabilities; the defaults describe a version-4 client with no extra options.

Outbound work leaves the client as `TCClientOutbound` records — a PGN, a data
payload, and an optional destination address (`None` means broadcast). You never
build CAN frames yourself: the client either broadcasts (the working-set
announcement) or addresses the TC it discovered.

Two callbacks connect the client to your application logic:

| Callback | Registered with | Fires when | You return |
| --- | --- | --- | --- |
| Value request | `on_value_request` | the TC asks for a current measured value | the `i32` value, or an `Err` to stay silent |
| Value command | `on_value_command` | the TC sends a setpoint to your device | `Ok(())` if applied, `Err` to signal it could not be |

Both callbacks receive an `ElementNumber` and a `DDI` so you know *which* part of
your device and *which* quantity is being addressed.

## Lifecycle and state machine

The client walks a single FSM, exposed as `TCState` and read with `state()`.
Every transition raises `on_state_change`. The states below are the ones you will
observe in order:

| State | What is happening |
| --- | --- |
| `Disconnected` | Idle. Nothing attempted, or a fault returned here. |
| `WaitForServerStatus` | `connect()` succeeded; waiting to hear a TC announce itself. |
| `SendWorkingSetMaster` | A TC was heard; about to announce this working set. |
| `RequestVersion` / `WaitForVersion` | Asking the TC its version and capabilities. |
| `RequestStructureLabel` / `WaitForStructureLabel` | Asking whether the TC already stores this DDOP's structure. |
| `RequestLocalizationLabel` / `WaitForLocalizationLabel` | Same check for the localization (language/units) label. |
| `TransferDDOP` / `WaitForPoolResponse` | Uploading the DDOP and waiting for accept/reject. |
| `ActivatePool` / `WaitForActivation` | Activating the stored pool. |
| `Connected` | The pool is active; process data flows. |
| `DeactivatePool` / `DeletePool` and their waits | Tearing down the old pool during a re-upload. |

The driving rules:

1. **Discover.** `connect()` first validates the DDOP, then moves to
   `WaitForServerStatus`. The TC broadcasts a periodic status; the first one
   received binds `tc_address()` and advances to `SendWorkingSetMaster`.
2. **Announce.** `update` ships the **Working Set Master** announcement (member
   count one) so the TC knows which working set owns the upcoming pool, then asks
   for the TC's version.
3. **Version handshake.** The version reply carries the TC's protocol version and
   its boom/section counts. The client records `tc_version()` and proceeds.
4. **Label check (the "do I need to upload?" decision).** The client asks the TC
   for the **structure label** of any DDOP it already stores for this client.
   An all-`0xFF` label means the TC has nothing → go straight to upload. A label
   that **matches** this DDOP's structure → check the localization label next, and
   if that also matches, jump straight to activation. A label that does **not**
   match → delete the stale pool first, then upload.
5. **Upload, then activate.** `TransferDDOP` serializes the DDOP and sends it as
   an object-pool transfer. A success response advances to activation; a failure
   returns to `Disconnected`. The activate command then lands the client in
   `Connected`, and only now should process data be emitted.

Every `WaitFor*` state is bounded by `timeout_ms`. If a reply does not arrive in
time, `update` drops the client back to `Disconnected` rather than hanging.

## Process-data exchange in practice

Once `Connected`, the conversation is entirely process data. Each message names
an element (a part of your device) and a DDI (a quantity from the data
dictionary), plus a 32-bit value where one applies.

**Reporting measured values.** Build an ECU→TC value frame with
`TaskControllerClient::build_value_command(element, ddi, value)` and ship it. The
element number is carried in a 12-bit field, so values above
`MAX_TC_PROCESS_DATA_ELEMENT_NUMBER` are rejected at build time rather than
silently truncated.

**Answering value requests.** When the TC asks for a current reading, the client
calls your `on_value_request` callback with the element and DDI and packages
whatever `i32` you return into the reply automatically. Return an `Err` and the
client stays silent for that request.

**Receiving setpoints.** When the TC sends a value to your device, the client
calls `on_value_command` with the element, DDI, and value. If the TC used the
"set and acknowledge" form, the client emits a process-data acknowledge for you,
reporting success or, on a callback `Err`, an error code such as
"no processing resources available." If your device does not register a command
callback at all, the acknowledge reports that the element is not supported.

**Triggers.** A TC does not poll blindly; it tells the client *when* to report a
variable. ISO 11783-10 defines trigger kinds — on a fixed time interval, on a
travelled-distance interval, on crossing a value threshold, on every change, and
on explicit request. The guidance for fast control data (such as section work
state) is to send on change as the primary trigger with a slow time interval as a
fall-back, and to respect the per-variable message-rate ceiling so you do not
flood the bus. Your job as the client is to honour the triggers the TC set and to
keep each request/response pair ordered.

## Peer control (TC-controlled section/rate)

Beyond plain measure-and-command, a TC can wire one control function's output
directly to another's input — for example, letting a guidance or rate source
drive an implement's sections. `machbus` models these routes with
`PeerControlAssignment` and a `PeerControlInterface` registry. An assignment
records a source (`from(element, ddi)` at a source address) and a destination
(`to(element, ddi)` at a destination address), and you `add_assignment`,
`activate_assignment`, and `remove_assignment` against the registry. Each route
encodes to an 8-byte process-data payload with `try_encode` and decodes inbound
ones with `PeerControlAssignment::decode`. Think of it as the TC delegating a
slice of control without routing every value through itself.

## Doing it with machbus

For applications, use the session facade; drop to the codec when you need to own
the pump.

### The session facade (recommended)

Plug the [`TcClient`](../guide/session-facade.md) plugin with your DDOP. It runs
the discover/announce/upload/activate FSM on each tick, ships the frames, and
emits `Event::Tc(TcEvent::StateChanged(..))` as the connection advances:

```rust
// illustrative shape — the API mirrors the tested `session::plugins::TcClient`
use machbus::session::{Session, EndpointTransport, plugins::TcClient};

let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(TcClient::new(TCClientConfig::default(), ddop))
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;
ctrl.with_mut::<TcClient, _>(|tc| tc.connect())?;     // arm the handshake

loop {
    if let Some(Event::Tc(TcEvent::StateChanged(state))) = driver.poll()? {
        // when `state` reaches TCState::Connected the DDOP is active
    }
}
```

Read status with `ctrl.with::<TcClient, _>(|tc| tc.is_connected())`, and reach the
underlying `TaskControllerClient` for value/command callbacks via
`with_mut::<TcClient>(|tc| tc.client_mut())`.

### Driving the codec directly

The whole connect handshake runs in `examples/tc_client_demo.rs`. Start by
building a small DDOP that names the device and one element:

```rust
{{#include ../../../examples/tc_client_demo.rs:22:34}}
```

Then create the client, hand it the pool, and begin the connection. The first TC
status the client hears binds the server address:

```rust
{{#include ../../../examples/tc_client_demo.rs:36:51}}
```

From there the example pumps `update` to ship the working-set announcement and
version request, feeds the TC's replies back with `handle_tc_message`, lets the
DDOP upload, and finishes on an activate response with the client in
`TCState::Connected`.

### More on the session facade

The `TcClient` plugin wraps the same client. You arm the handshake with
`ctrl.with_mut::<TcClient, _>(|tc| tc.connect())`, read `state()` or
`is_connected()`, and read `tc_address()` after the handshake (the negotiated
`tc_version()` lives on the underlying client via `client_mut()`).
Inbound TC frames are routed and the FSM's outbound frames are shipped
automatically on every `driver.poll()?`, so you never hand-pump `update`. To
register the value and command callbacks, reach the underlying client with
`ctrl.with_mut::<TcClient, _>(|tc| tc.client_mut())`. Drain TC state changes from
the session with `ctrl.drain::<TcEvent>()`, which yields `TcEvent::StateChanged`
as the FSM advances toward `Connected`.

## Events and responsibilities

| Situation | What the client does | What you must do |
| --- | --- | --- |
| TC asks for a value | Calls `on_value_request` | Return the current reading, or `Err` to decline |
| TC sends a setpoint | Calls `on_value_command` | Apply it; return `Err` if you cannot |
| TC requests "set and acknowledge" | Builds the acknowledge for you | Make the callback's result truthful |
| FSM transitions | Raises `on_state_change` / `TcEvent::StateChanged` | Gate application logic on reaching `Connected` |
| TC stops announcing | Times out a `WaitFor*` state → `Disconnected` | Stop emitting process data; reconnect when it returns |

The one hard rule: **do not emit process data until `Connected`.** Reporting
values against a pool the TC has not activated is meaningless and the client's
state guards exist precisely to stop it.

## Edge cases and failure modes

- **TC rejects the DDOP.** A non-zero pool response means the upload was refused.
  The client returns to `Disconnected` instead of pretending the pool is live.
  Re-check the DDOP for duplicate object IDs or dangling references before
  retrying.
- **Version or capability mismatch.** A malformed version reply (wrong fixed
  layout) is ignored and the client keeps waiting until the timeout. Make sure the
  TC you target speaks a version your client supports.
- **Value out of range.** Element numbers wider than the 12-bit wire field are
  rejected when you build the payload, so a coding error surfaces as an `Err`
  rather than a corrupted frame on the bus.
- **Frame from the wrong TC.** Once a TC is bound, frames from any other source
  address are refused. Use `try_handle_tc_message` if you want the explicit
  envelope error (`InvalidPgn`, `InvalidAddress`, `InvalidState`, `InvalidData`)
  rather than the silent-ignore wrapper.
- **Connection loss.** If the TC goes quiet, the active `WaitFor*` state times out
  to `Disconnected`. Treat that as "stop work" and re-run `connect()` when the TC
  reappears.

## Advanced

- **Caching the DDOP by label.** The structure- and localization-label check is
  the bandwidth optimisation that matters most. When the TC already stores a pool
  whose labels match the one you are about to send, the client skips the entire
  transfer and jumps to activation. Set stable labels on your device object so a
  TC that has seen your implement before does not re-download megabytes of pool.
- **Re-uploading while connected.** `reupload_ddop(pool)` from `Connected`
  validates the new pool, then deactivates, deletes, re-uploads, and reactivates
  without re-discovering the TC. Use it when the implement's description changes
  mid-session.
- **Multiple TCs and large pools.** A client connects to a single TC at a time;
  the binding follows the first/preferred TC it locks onto, so do not fan one
  client across servers. A wide implement reports many element/DDI pairs — lean on
  the triggers the TC assigns rather than polling everything yourself, and keep
  request/response pairs ordered so you stay under the per-variable rate ceiling.
- **Session facade vs the bare codec.** The `TcClient` plugin is right for
  applications: it routes inbound frames and ships outbound ones on each poll. The
  raw `TaskControllerClient` is right for tests and embedded loops where you own
  the `update`/`handle_tc_message` cadence.

## Validate locally

```sh
make run EXAMPLE=tc_client_demo
make test
```

The example drives the full connect handshake — status discovery, working-set
announcement, version exchange, DDOP upload, and activation — entirely in
software, and asserts the client lands in `TCState::Connected`. The test suite
covers the label-driven upload/skip/delete decision, the timeout-to-`Disconnected`
path, the re-upload sequence, and the value-request and setpoint callbacks.

## What this proves / does not prove

Proves: the client's connect FSM, the label-based upload decision, the
process-data callbacks, and the re-upload teardown behave correctly in software,
and the `machbus` API drives them as documented.

Does not prove: interoperability with a specific third-party Task Controller,
real-hardware timing and bandwidth behaviour, or any conformance or certification
claim. Those still require official standards, real hardware, and interoperability
evidence.

## See also

- [Task Controller concepts](../standards/task-controller.md) — the
  conceptual primer for tasks, DDOP, and process data.
- [DDOP](ddop.md) — building and validating the device description you upload.
- [Task Controller server](task-controller-server.md) — the other side of this
  conversation.
- [TC geo prescription](tc-geo-prescription.md) — how a TC turns a map into the
  setpoints this client receives.
