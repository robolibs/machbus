# Virtual Terminal server

A Virtual Terminal (VT) is the display that an operator looks at in the cab. The
VT *server* is the terminal side of the relationship: it advertises itself on
the bus as a screen that implements can talk to, accepts the object pools they
upload, keeps each pool as semantic state, and hands operator input — soft keys,
buttons, edited values — back to whichever implement is in control. This page
explains the server's job, the upload and connection lifecycle, how `machbus`
models several connected implements at once, and how to drive the whole thing
with `VTServer` and the session facade.

If you are writing the *implement* side — the ECU that uploads a pool and reacts
to operator input — read [Virtual Terminal client](virtual-terminal-client.md)
instead. For the shared vocabulary (object pools, masks, working sets), read
[Virtual Terminal concepts](../standards/virtual-terminal.md) first.

## Why this exists

An ISOBUS tractor has one terminal but may tow or carry several implements, each
made by a different manufacturer. None of them ship their own screen. Instead
each implement carries a description of the screen it *wants* — an object pool of
masks, buttons, numbers, strings, and pictures — and uploads that description to
the terminal. The terminal renders it and reports back what the operator does.

The VT server is the half of that contract that lives in the terminal. Its job,
in `machbus` terms, is to:

- **advertise** itself as a VT, periodically, so clients know a terminal exists;
- **accept** a client's working set and let it begin an upload;
- **store and validate** the uploaded object pool before activating it;
- **track render state** — which mask is active, what values changed, what is
  hidden — as a semantic cache;
- **deliver operator input** (key and button presses, edited values, selections)
  back to the client as events.

`VTServer` itself does not own a GUI window or pixel framebuffer. It keeps the
*meaning* of an activated pool — enough for protocol tests, auditing, and render
effect replay. Hosted Rust code can feed that state into `VtRenderRuntime`,
`GtuiRenderer`, or `FramebufferRenderer` to produce backend-neutral commands or
deterministic RGB snapshots. The C and Python bindings expose the session/client
and server protocol surfaces; hosted object-pool layout/rendering remains
Rust-only for now.

## Mental model

```
        implement (client)                 terminal (VTServer)
        ──────────────────                 ───────────────────
                                  VT_STATUS (every ~1 s)
                              ◄─────────────────────────────  "a VT is here"
        Get Memory  ─────────────────────────────────────►
                              ◄─────────────  Get Memory Response (upload OK)
        Object Pool Transfer ───────────────────────────►   store + validate
              (sent over the transport protocol, reassembled)
        End of Object Pool  ─────────────────────────────►
                              ◄──────────  End of Pool Response (accept/reject)
                                                            activate pool,
                                                            become active WS
        change-numeric / hide / change-active-mask ─────►   mutate state cache
                              ◄──────────  soft-key / button / value events
```

The exchange is request/response and event-driven. The server announces itself
on a fixed cadence, walks one client at a time through the upload handshake, and
once a pool is activated it both *records* the change commands the client sends
and *emits* the input the operator generates.

## Anatomy: the server pieces

`machbus` splits the server into a small set of types under `isobus::vt`.

| Type | Role |
| --- | --- |
| `VTServerConfig` | The advertised screen: `screen_width`, `screen_height`, `vt_version`. Built with `with_screen`, `with_width`, `with_height`, `with_version`; `validate()` rejects a zero dimension or an out-of-range version. |
| `VTServer` | The server engine. Holds the FSM, the list of connected working sets, the status cadence, and the input events. |
| `VTServerState` | Where the server is in its lifecycle: `Disconnected`, `WaitForClientStatus`, `SendWorkingSetMaster`, `WaitForPoolUpload`, `Connected`. |
| `ServerWorkingSet` | Per-client tracking: the client address, the uploaded `pool`, the upload flags, stored versions, and the `object_state` cache. |
| `ServerObjectState` | The semantic cache for one activated pool — active mask, visibility, numeric/string values, attributes, and more. |
| `OutboundFrame` | One frame the server wants to put on the wire, with `dest: Some(addr)` for a reply or `None` for the broadcast status. |

The version the server advertises is bounded: `VT_SERVER_MIN_VERSION` (3) through
`VT_SERVER_MAX_VERSION` (6). The status broadcast cadence is
`VT_STATUS_INTERVAL_MS` (one second). These are the only "what kind of terminal"
knobs you set; everything else is driven by what clients upload. The server
follows ISO 11783-6 (Virtual Terminal) for the message shapes.

`VTServer` is **pump-style**: it does not own a network handle. You feed it
inbound `PGN_ECU_TO_VT` messages and it returns the `OutboundFrame`s it wants to
send; you advance its clock with `update`. The session facade wires that pump to
a real bus for you (covered below).

## Lifecycle and state machine

A client connects by walking the server through one upload handshake. The server
tracks each client independently with `ServerWorkingSet` and moves its own
top-level `VTServerState` as the first client progresses.

```
              Disconnected
                   │ start()  (validates the advertised config)
                   ▼
           WaitForClientStatus ──────► update() begins broadcasting VT_STATUS
                   │ Get Memory from a client
                   │ → reply Get Memory Response, allow upload
                   ▼
            WaitForPoolUpload
                   │ Object Pool Transfer (reassembled by the transport)
                   │ → deserialize, validate graph, stash as pending
                   │ End of Object Pool
                   ├── pool valid & pending ──► activate, become active WS
                   │                            emit on_client_connected
                   ▼
               Connected  ◄──── more clients upload, change commands flow
                   │ stop()
                   ▼
              Disconnected  (saves stored versions, clears clients)
```

Step by step:

1. **Advertise.** After `start()`, every call to `update(elapsed_ms)` advances a
   timer; once a second's worth of time has accumulated it returns the eight-byte
   `VT_STATUS` payload to broadcast. That frame names the active working set and
   the advertised VT version, and it is how a client first learns a terminal
   exists.
2. **Get Memory.** A client asks whether the terminal has room for its pool. The
   server registers the client, marks `pool_upload_allowed`, and replies with a
   Get Memory Response. From `WaitForClientStatus` this also moves the server to
   `WaitForPoolUpload`.
3. **Transfer.** The client sends its serialized pool. For anything but a tiny
   pool this arrives over the transport protocol (BAM or RTS/CTS) and is
   reassembled into one message before the server sees it. The server
   deserializes it, runs `pool.validate()`, and — if it parses and is non-empty —
   stores it as *pending activation*.
4. **End of Object Pool.** The client signals it is done. The server re-checks
   that a pending, uploaded, non-empty, valid pool exists. If so it marks the
   pool *activated*, replies with a success End of Pool response, transitions to
   `Connected`, makes this client the active working set if none was set, and
   emits `on_client_connected`. If not, it replies with an error response and
   clears the pending flags. There is no separate "activate" command — a
   successful End of Object Pool response *is* activation.
5. **Run.** With the pool activated, the client streams change commands and the
   server records them; the operator generates input and the server emits it.

The optional **version storage** steps (Store / Load / Get / Delete Version) let
a client save an uploaded pool under a short label so a later session can reload
it without re-uploading. A successful Load Version activates the restored pool
just like an End of Object Pool would.

## Managing multiple working sets

Several implements can be connected to one terminal at the same time. The server
keeps a `ServerWorkingSet` per client in `clients()`, each with its own pool,
upload state, and object-state cache, keyed by the client's source address. They
do not interfere: a change command from `0x42` only ever mutates `0x42`'s cache.

Exactly one of them is the **active working set** at a time — the one whose
screen the operator currently sees. `active_working_set()` returns its address
(or the null address when none is active). The active selection moves in three
ways:

- the **first** client to finish a successful upload becomes active if no working
  set was active yet;
- a client can ask to become active with the Select Active Working Set command,
  which the server honours only if that client has an activated pool;
- the application can force it with `set_active_working_set(addr)`.

Whenever the active working set changes, the server emits
`on_active_ws_changed` carrying `(old, new)`, and the next `VT_STATUS` broadcast
advertises the new active address. If the active client deletes its pool (Delete
Object Pool), the server clears the active selection back to the null address.

## Doing it with machbus

There are two ways to drive the server: the `VtServer` plugin on a `Session` for
applications, and the bare `VTServer` pump for tests and tight control loops.

### The session facade (recommended for applications)

Plug the `VtServer` plugin into a `Session`. It claims an address, runs the FSM,
routes inbound `PGN_ECU_TO_VT` traffic into the server, and ships the periodic
`VT_STATUS` for you on every poll. You give it a screen size and a version:

```rust
use machbus::session::{Session, EndpointTransport, plugins::VtServer};

let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(VtServer::new(VTServerConfig::default())?)
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;
```

After the address is claimed you start the server FSM through fine control on the
plugin:

```rust
ctrl.with_mut::<VtServer, _>(|vt| vt.start())?;

loop {
    driver.poll()?;
    for ev in ctrl.drain::<VtEvent>() {
        // state changes, client connect/disconnect, active-working-set
        // changes, soft keys, buttons, numeric/string value changes,
        // input-object selections
    }
}
```

`ctrl.drain::<VtEvent>()` yields the server events as they happen, and
`ctrl.with_mut::<VtServer, _>(|vt| ...)` reaches the underlying `VTServer` for
state queries, version management, and callbacks not surfaced as events. Driving
`driver.poll()?` claims, starts the server, and ships the `VT_STATUS` frames a
watching node receives.

### The low-level pump (for tests and embedded control)

Underneath, `VTServer` is a self-contained engine you drive by hand. Construct it
from a `VTServerConfig`, then `start()`:

```rust
{{#include ../../../examples/vt_server_demo.rs:14:25}}
```

Feed each inbound message to `handle_ecu_message(&msg)`, which returns the
`OutboundFrame`s to send and applies side effects (state transitions, events).
The demo walks a single client at `0x42` through Get Memory, pool transfer, and
End of Object Pool. The upload step:

```rust
{{#include ../../../examples/vt_server_demo.rs:39:49}}
```

And the activation step, where a success response makes the pool active:

```rust
{{#include ../../../examples/vt_server_demo.rs:58:67}}
```

To advertise, call `update(elapsed_ms)` each loop; when it returns `Some(bytes)`
you broadcast those bytes. `clients()` and `active_working_set()` let you inspect
the connection table at any time.

## Events and responsibilities

Whichever API you use, the server raises events your application reacts to. On
the raw `VTServer` these are `Event` fields; the `VtServer` plugin bridges them
to `VtEvent`s you drain from the session.

| Event | Meaning | Typical action |
| --- | --- | --- |
| `on_state_change` | The server FSM moved. | Track connection progress / UI state. |
| `on_client_connected` | A client's pool activated. | Add it to the rendered set; pick it if it should be foreground. |
| `on_client_disconnected` | A client dropped. | Remove its surface; reassign the active working set. |
| `on_active_ws_changed` | The foreground client changed. | Repaint with the new client's pool. |
| `on_soft_key_activation` / `on_button_activation` | The operator pressed a key/button. | Route the key number to the active client. |
| `on_numeric_value_change` / `on_string_value_change` | The operator edited a value. | Reflect and forward the new value. |
| `on_input_object_selected` | An input object was selected / opened for edit. | Move focus on the rendered screen. |

The server's own responsibilities are: never accept change commands for a pool
that is not activated; only mutate the cache of the client that sent the command;
and keep advertising `VT_STATUS` for as long as it is running so clients do not
time the terminal out.

## Edge cases and failures

- **Bad or truncated pool.** If the transferred bytes fail to deserialize, are
  empty, or fail graph validation (missing root, bad child reference, unknown
  object), the server silently drops the transfer — the pending pool never
  becomes activated and End of Object Pool then returns an error response.
- **End of Object Pool with nothing pending.** If a client signals end-of-pool
  without a valid pending upload, the server clears the upload flags and replies
  with an error response rather than activating stale state.
- **Unsupported function.** An ECU-to-VT function the server does not implement
  is answered with an Unsupported VT Function reply naming the function byte,
  built by `build_unsupported_function`, rather than being silently ignored.
- **Commands before activation.** Change commands (hide/show, change active mask,
  numeric value, and the rest) only mutate state once the client's pool is
  `pool_activated`; before that they are no-ops. Many also re-check that the
  referenced object exists and is of the expected type, so a command naming a
  non-existent or wrong-typed object is dropped.
- **Malformed change command.** Commands with the wrong length, a non-canonical
  boolean, or a non-`0xFF` reserved tail are rejected without mutating state, so
  a single bad frame cannot corrupt the cache.
- **Client disappears.** A client that stops talking leaves its `ServerWorkingSet`
  in place; deciding when to evict it and reassign the active working set is the
  application's call. A Delete Object Pool from the active client clears the
  active selection.
- **Multiple clients racing for foreground.** Only one working set is active.
  Honour Select Active Working Set, or arbitrate in the application with
  `set_active_working_set`; do not assume the last uploader wins.

## Advanced

- **Soft-key and aux routing.** Soft-key and button activations come back as
  events carrying the object ID, the parent ID, and the physical key number, so
  the application can map a press to the right client and object. For auxiliary
  inputs (joysticks, encoders), the server advertises channel capabilities set
  with `set_aux_capabilities`, binds an uploaded AUX input object to an AUX
  function with `assign_aux_input`, and folds incoming AUX status frames into the
  cache with `handle_aux_input_status` — rejecting cross-family (AUX-O vs AUX-N)
  assignments. See [VT auxiliary capabilities](vt-auxiliary-capabilities.md).
- **Language and units.** Operator language and unit preferences are broadcast
  separately from the VT exchange; a client adapts its pool to them. That
  broadcast is covered in the language-command material rather than here.
- **Version storage.** `set_storage_path`, `load_all_versions`,
  `save_all_versions`, and `cleanup_expired_versions` persist uploaded pools per
  client under short labels, so a returning implement can reload a stored pool
  instead of re-uploading it. Stored versions are capped per client and validated
  on read.
- **Session facade vs the bare codec.** The `VtServer` plugin is right for
  applications: it sequences the claim, the FSM, the inbound routing, and the
  status broadcast. The bare `VTServer` pump is right for unit tests and tightly
  controlled loops where you own every message and every millisecond.

## Validate locally

```sh
make run EXAMPLE=vt_server_demo
make test
```

`vt_server_demo` drives one client through the full handshake in software and
asserts that End of Object Pool returns success and the server reaches
`Connected`. The session tests build the `VtServer` plugin, claim an address,
start the server, and check the `VT_STATUS` broadcasts a second node receives.

## What this proves / does not prove

Proves: the upload handshake, pool validation, activation, the active-working-set
rules, and the change-command state cache behave as described in software, and
the `machbus` API drives them correctly.

Does not prove: pixel-accurate rendering on a specific commercial VT,
real-hardware timing, interoperability with a specific third-party terminal or
implement, or any conformance/certification claim. The hosted Rust renderer and
software framebuffer are regression/evidence tools, not certification. `machbus`
is not certified; real deployment still needs official standards, hardware, and
interoperability evidence.

## See also

- [Virtual Terminal client](virtual-terminal-client.md) — the implement side that
  uploads a pool and consumes the input events.
- [Virtual Terminal concepts](../standards/virtual-terminal.md) — object pools,
  masks, and working sets.
- [VT auxiliary capabilities](vt-auxiliary-capabilities.md) — advertising and
  routing auxiliary inputs.
- [Address claim](address-claim.md) — the claim every VT server completes before
  it advertises.
