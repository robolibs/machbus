# Task Controller server

A **task controller server** is the node on an ISOBUS that an implement reports
to. It advertises itself as a task controller, accepts implement clients, stores
the device descriptions those clients send, and then exchanges *process data*
with them — logging the values an implement measures and, when the work calls
for it, pushing setpoints back the other way. This tutorial covers why the role
exists, the server's lifecycle, and how to build one with `machbus` at both the
low level (`tc::server`) and through the session facade.

The mirror image of this page is [Task Controller client](task-controller-client.md):
that side is the implement; this side is the controller it talks to. Read both
to see a full exchange. ISO 11783-10 (Task Controller) is the part of the
standard this role comes from.

## Why this exists

An implement — a sprayer, a planter, a spreader — knows a great deal about
itself: how many sections it has, what it can measure, what it can be told to
do. But it has no display, no task plan, and nowhere to write a log. The task
controller fills those gaps. It is the node that holds the job to be done,
records what actually happened, and issues the commands that make the implement
act on a prescription.

For that to work the controller cannot hard-code knowledge of every implement
ever built. Instead each implement describes itself in a structured **device
description** (a DDOP — see [Device descriptions](ddop.md)), uploads it once,
and from then on the two sides speak in terms of *elements* and *DDIs* (data
dictionary identifiers) rather than vendor-specific messages. The server's job
is to receive that description, validate it, keep it, and use it as the shared
vocabulary for everything that follows.

## Mental model

```
implement (client)                    task controller (server)
        │                                       │
        │  <───── TC status / version ──────    │  advertises capabilities
        │                                       │
        │  ──── working set / version req ──>   │  registers the client
        │                                       │
        │  ───────── DDOP upload ──────────>    │  validate, store (inactive)
        │  <──────── pool response ─────────    │
        │                                       │
        │  ───────── activate pool ────────>    │  activate, build lookups
        │  <──────── activate response ─────    │
        │                                       │
        │  ═══════ process data both ways ══    │  request / value / setpoint
        │                                       │
        │  ──────── deactivate / delete ───>    │  drop active state on command
```

The server is a passive responder for almost everything: the client drives the
handshake, and the server answers. The two things the server originates on its
own are the periodic **status broadcast** (so clients know a task controller is
present) and any **value requests or setpoints** the application chooses to send
once a client is active.

## Anatomy: what the server tracks

In `machbus` the low-level type is `tc::TaskControllerServer`. It is configured
with a `TCServerConfig` and keeps a small amount of state per client.

| Piece | machbus type / field | What it holds |
| --- | --- | --- |
| Identity | `TCServerConfig::tc_number`, `tc_version` | The TC number that distinguishes this controller, and the version it speaks. |
| Capacity | `num_booms`, `num_sections`, `num_channels` | The boom/section/channel dimensions advertised to clients. |
| Feature flags | `server_options` | A bitfield of supported features (documentation, section control, peer control, geo). |
| Per-client record | `TCClientInfo` | Address, stored `DDOP`, whether the pool is activated, last transfer, tracked client version. |
| Stored labels | `structure_label`, `localization_label` | Seven-byte labels that let a returning implement skip re-uploading. |
| Trigger runtime | `MeasurementTriggerRuntime` | Per-value logging triggers (time, distance, threshold, on-change). |

You build the configuration with consuming `with_*` setters and validate it
before the server goes live:

```rust
{{#include ../../../examples/tc_server_demo.rs:13:24}}
```

`TCServerConfig::validate` rejects topologies the wire format cannot express — a
zero boom count, or a section count outside the representable range. Reject
invalid topology *before* you start advertising it, never after.

### Capabilities and options

The `server_options` byte tells clients which features this controller actually
supports. `machbus` names the bits in `tc::ServerOptions`:

| Flag | Meaning |
| --- | --- |
| `SupportsDocumentation` | The controller can log values for record-keeping (the documentation TC). |
| `SupportsTCGEOWithoutPositionBasedControl` | Geo features without position-based section/rate control. |
| `SupportsTCGEOWithPositionBasedControl` | Geo features *with* position-based control (prescription maps). |
| `SupportsPeerControlAssignment` | One client may be wired to control another's value. |
| `SupportsImplementSectionControl` | The controller can drive implement section on/off. |

These OR together into the single byte you pass to `with_options`. Advertise
only what your application truly implements: a client that sees a flag set will
expect the behaviour behind it.

## Lifecycle and state machine

The server moves through three states, exposed as `tc::TCServerState`:

| State | Meaning | What the server does |
| --- | --- | --- |
| `Disconnected` | Not running. | Nothing. `update` returns no status. |
| `WaitForClients` | Started, advertising, no client yet. | Emits periodic TC status; waits for a first client. |
| `Active` | At least one client registered. | Full process-data exchange; still broadcasts status. |

The transitions:

1. **Start.** `start()` moves the server from `Disconnected` to `WaitForClients`.
   From here `update(dt)` returns a status payload roughly every two seconds
   (`TC_STATUS_INTERVAL_MS`), which the caller broadcasts so clients discover a
   task controller is present.
2. **A client appears.** When the first client sends a working-set master frame
   or a technical-capabilities request, the server registers it and moves to
   `Active`. Registration creates a `TCClientInfo` and raises
   `on_client_connected`.
3. **Capability exchange.** The client asks what the controller supports; the
   server answers with version, options, and boom/section/channel counts. The
   server may also ask the client for *its* version with
   `request_client_version`, and records the reply (raising
   `on_client_version_received`).
4. **DDOP upload.** The client transfers its device description. The server
   deserializes and validates it, stores it on the client's record as an
   *inactive* pool, and replies with an object-pool response carrying an
   error code (`tc::ObjectPoolErrorCodes`).
5. **Activation.** The client requests activation; `activate_pool` flips the
   stored pool to active if it holds at least one device, otherwise it answers
   with an activation error (`tc::ObjectPoolActivationError`) and raises
   `on_pool_activation_error`.
6. **Process data.** With an active pool the two sides exchange values: the
   server answers `RequestValue`, receives `Value` / `SetValueAndAcknowledge`,
   and may originate its own requests and setpoints.
7. **Teardown.** The client may deactivate or delete its pool; the server drops
   the active state *only* on a well-formed command. `stop()` returns the server
   to `Disconnected` and clears all client records.

## Doing it with machbus

There are two ways to run a TC server, and they suit different needs.

### The session facade (recommended for applications)

Plug the `TcServer` plugin into a `Session`: it claims an address, ships the
periodic status for you, and routes inbound implement traffic into the server on
every `driver.poll()?`. You build it with the topology you want to advertise:

```rust
use machbus::session::{Session, EndpointTransport, plugins::TcServer};

let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(TcServer::new(
        TCServerConfig::default()
            .with_booms(2)
            .with_sections(16)
            .with_channels(4),
    )?)
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;
```

After the address claim settles, you start the server subsystem and let the
session pump it:

```rust
ctrl.with_mut::<TcServer, _>(|tc| tc.server_mut().start())?;
// ... each loop iteration:
driver.poll()?;
```

The session with the `TcServer` plugin runs the claim, starts the server, and
ships the `TC_STATUS` frames a passive peer observes on `PGN_TC_TO_ECU`. You reach
the underlying server through `ctrl.with_mut::<TcServer, _>(|tc| ...)`. The
plugin itself exposes `server_mut()`, which hands you the full
`TaskControllerServer` for `start`, `stop`, `state`, callbacks, and measurement
triggers.

### The low-level server (for tests and embedded control)

`tc::TaskControllerServer` is a pump: you feed it inbound frames and ship what it
returns. The standalone `tc_server_demo` example uses it directly. The two entry
points are:

- `handle_client_message(&msg)` — feed an inbound `PGN_ECU_TO_TC` frame and get
  back a `Vec<TCOutbound>` to send. Malformed or unrelated frames are ignored.
- `try_handle_client_message(&msg)` — the same, but returns explicit errors for
  the wrong PGN, an invalid source address, an empty message, or a malformed
  fixed-size request. Use this when your dispatch needs to know *why* a frame was
  rejected.
- `handle_working_set_master(&msg)` / `try_handle_working_set_master(&msg)` —
  register a client from its working-set announcement and request its version.
- `update(dt)` — returns the periodic status payload when the cadence elapses,
  otherwise `None`.

Each `TCOutbound` carries the payload plus an optional destination: `dest: None`
means broadcast, `Some(addr)` means a directed reply. You install behaviour with
three callbacks:

| Callback | Fires on | Your job |
| --- | --- | --- |
| `on_value_request(cb)` | Client `RequestValue` | Return the current `i32` value for `(element, DDI)`. |
| `on_value_received(cb)` | Client `Value` / `SetValueAndAcknowledge` | Accept the value; return a `ProcessDataAcknowledgeErrorCodes`. |
| `on_peer_control_assignment(cb)` | Peer-control assignment | Accept or reject one value driving another. |

To originate traffic, the server exposes builders that return an 8-byte payload:
`build_request_value`, `build_set_value`, `build_set_value_and_acknowledge`, and
the measurement-command builders (`build_time_interval_measurement_command` and
its distance/threshold/change siblings). These all validate the element number
against the 12-bit wire field and return a `Result`.

## Events and responsibilities

Whichever API you use, the server enforces protocol shape but leaves the meaning
to you. Your responsibilities:

- **Validate the DDOP.** The server deserializes and validates on upload; you
  decide whether the described device fits the task at hand.
- **Answer value requests truthfully.** `on_value_request` must return the real
  current value, or the controller logs nonsense.
- **Issue setpoints deliberately.** A setpoint makes the implement act. Send one
  only when the task and the implement's active pool support it.
- **Log measured values.** Wire measurement triggers to your application's time
  base and odometry so the right values get sampled at the right moments.

The native events you can subscribe to are `on_state_change`,
`on_client_connected`, `on_client_disconnected`, `on_client_version_received`,
`on_pool_activation_error`, and `on_peer_control_assignment_received`. Through
the session the same information arrives as `TcEvent` values from
`ctrl.drain::<TcEvent>()`.

## Managing multiple clients and stored descriptions

A real bus can carry several implements. The server keeps one `TCClientInfo` per
source address in `clients()`, each with its own stored DDOP and activation
state, so two implements never share a description. A directed reply always goes
back to the address that asked.

The structure and localization labels are how a returning implement avoids
re-uploading. An implement first asks for the controller's stored labels; if they
match the DDOP it already has, it can skip the upload entirely. The server hands
back whatever you configured with `set_structure_label` / `set_localization_label`,
and the all-`0xFF` label means "nothing stored", which prompts the client to
upload. The server also treats a byte-for-byte repeat of an already-accepted
transfer as idempotent — an identical re-upload does not deactivate a pool that
is already live.

## Measurement triggers

The documentation side of a task controller logs values over time. Rather than
polling, the server keeps a local **trigger runtime** per value — built with
`MeasurementTriggerRuntime::new(dest, element, ddi)` and the `with_*` setters for
time, distance, threshold, and on-change conditions. Register one with
`configure_measurement_trigger`. Then:

- `update_measurements(dt)` advances time-based triggers and returns any due
  `RequestValue` frames — drive it from the same tick that pumps the stack.
- `record_measurement_distance(dest, mm)` adds travelled distance and returns due
  distance-triggered requests — call it when your application has odometry or a
  GNSS-derived distance.

Threshold and on-change triggers fire as values arrive through `on_value_received`.
A malformed inbound value must never overwrite the last accepted value, so the
log stays trustworthy.

## Edge cases and failures

- **Unknown DDI.** A request or value for a DDI the application does not handle
  should be answered with the appropriate acknowledge error
  (`ElementNotSupportedByThisDevice`), not silently. The default when no value
  callback is installed is exactly that error.
- **Malformed DDOP.** A pool that fails to deserialize or validate is rejected
  with an object-pool error and is *not* stored; the previous state is left
  untouched.
- **Activation with no device.** Activating an empty or never-uploaded pool
  yields `ThereAreErrorsInTheDDOP` and raises `on_pool_activation_error`.
- **Unsupported version or feature.** A client that asks for a feature whose flag
  you did not advertise must not get it. Keep `server_options` honest.
- **Client disconnects mid-task.** If a client falls off the bus, its
  `TCClientInfo` still holds its last DDOP and activation state. Decide whether to
  retain it (so the implement can resume) or drop it; the protocol does not force
  the choice.
- **Wrong PGN or bad source address.** `try_handle_client_message` returns an
  error for traffic that is not `PGN_ECU_TO_TC`, or that arrives from the null or
  broadcast address. The lenient wrapper drops these instead.

## Advanced

- **Documentation vs control TC.** A *documentation* controller only logs; a
  *control* controller also issues setpoints and section commands. The
  `server_options` flags you set decide which one your node presents as. Many
  controllers do both.
- **Peer control.** With `SupportsPeerControlAssignment` advertised, one client's
  value can be wired to drive another's. The server validates the assignment and
  hands it to your `on_peer_control_assignment` callback, which accepts or rejects
  it. See [Task Controller concepts](../standards/task-controller.md)
  for the model.
- **Geo and prescriptions.** Position-based control — applying a prescription map
  as the machine moves — sits on top of the geo option flags and is covered in
  [TC-GEO prescription](tc-geo-prescription.md).
- **Persistence.** The in-memory store lives only as long as the server. If you
  want a returning implement to skip re-upload across power cycles, persist the
  labels and DDOPs yourself and restore them before clients connect.
- **Session facade vs the bare codec.** The `TcServer` plugin is right for
  applications: it claims, routes, and broadcasts for you. The bare
  `TaskControllerServer` is right for unit tests and tight embedded loops where
  you own every frame and millisecond.

## Validate locally

```sh
make run EXAMPLE=tc_server_demo
make test
```

`tc_server_demo` registers a client, answers a value request, and prints a status
broadcast entirely in software. The session tests build the `TcServer` plugin on
a virtual bus, run the address claim, start the server, and count the status
frames a passive watcher observes.

## What this proves / does not prove

Proves: the server's handshake, DDOP storage and validation, activation logic,
process-data callbacks, and measurement triggers behave as described in software,
and the machbus API drives them correctly.

Does not prove: real-hardware timing, interoperability with a specific
third-party implement or FMIS, or any conformance or certification claim. Those
still require official standards, real hardware, and interoperability evidence.
`machbus` ships no certification.

## See also

- [Task Controller client](task-controller-client.md) — the implement side of
  this exchange.
- [Device descriptions](ddop.md) — the DDOP the client uploads and the server
  stores.
- [Task Controller concepts](../standards/task-controller.md) — elements, DDIs,
  process data, and peer control.
- [TC-GEO prescription](tc-geo-prescription.md) — position-based control on top
  of the geo options.
