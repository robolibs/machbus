# Virtual Terminal client

A Virtual Terminal (VT) is the display-and-input device in the cab — the screen,
soft keys, and dial that the operator uses to drive an implement. The implement
itself has no screen. Instead it ships a *description of its user interface* to
the VT and then talks to that interface over the bus. The piece of software on
the implement side that does this is the **VT client**, and this tutorial shows
how to drive one with `machbus` through `isobus::vt::VTClient`.

If you are new to the VT model, read
[Virtual Terminal concepts](../standards/virtual-terminal.md) first; this page
assumes you know what a working set, an object pool, and a data mask are, and
focuses on the *client lifecycle*: how to find a VT, hand it the interface, and
then keep that interface in sync with your application state.

## Why this exists

The cab terminal is a shared, general-purpose display. Many different
implements — a seeder today, a sprayer tomorrow — must each present their own
controls on the same screen without the terminal knowing anything about them in
advance. ISO 11783-6 (Virtual Terminal) solves this by making the implement
*describe* its interface as a tree of objects (the **object pool**) and upload
that tree to the terminal at connection time. The terminal renders the pool;
the implement then sends small commands ("set this number to 42", "hide this
object") and receives input events ("soft key 3 was pressed") back.

The client's job is three things, in order:

1. **Find a VT partner** on the bus and bind to it.
2. **Upload the object pool** so the terminal has something to draw.
3. **Drive the UI** — push value updates out, take operator input in — for as
   long as the connection lasts.

Get the first two wrong and there is nothing on screen; this tutorial spends
most of its length on getting them right.

## Mental model

```
   your app state                 the bus                  the terminal
   ─────────────                  ───────                  ────────────
   build ObjectPool
        │
   VTClient::connect() ──── listen for VT status ────────►  VT broadcasts
        │                                                    its status
        ▼                  ◄───────────────────────────────  (found it)
   announce working set
   ask "got memory?"  ──────── GetMemory(size) ───────────►  reserve room
        │                  ◄──── memory OK / not OK ────────
        ▼
   stream the pool    ──────── pool transfer (TP/ETP) ─────►  store objects
   say "that's all"   ──────── EndOfObjectPool ────────────►  parse + validate
        │                  ◄──── parsed OK / error ──────────
        ▼
   CONNECTED
        │
   change_numeric_value() ──── ECU→VT command ─────────────►  redraw
   on_soft_key / on_button ◄── VT→ECU activation ──────────  operator presses
```

The whole thing is a pump: you feed inbound frames into the client, you call
`update`, and the client hands you the frames it wants put on the wire. The
client never holds a network handle and never sends on its own — you route every
byte. That makes it testable in pure software and easy to slot under whatever
transport you use.

## Anatomy: the pieces tied to the API

| Piece | machbus type | Role |
| --- | --- | --- |
| Connection state | `vt::VTState` | Where the client is in the connect/upload lifecycle. |
| Configuration | `vt::VTClientConfig` | Per-session timeout and preferred VT version. |
| The interface | `vt::ObjectPool` + `vt::WorkingSet` | The tree of objects you upload. |
| An outbound frame | `vt::ClientOutbound` | A `{ pgn, data, dest }` triple the caller ships. |
| Version preference | `vt::VTVersion` | Which VT generation (3–6) you target. |
| Language | `vt::LanguageCode` | The two-letter language the pool was built for. |

`VTClientConfig` defaults to a 6-second per-step timeout and VT version 4. You
can adjust either with the consuming setters `with_timeout` and `with_version`,
and you can change the version preference later with `set_vt_version_preference`.

Every command method returns a `ClientOutbound` (or an error). `ClientOutbound`
carries the PGN, the payload bytes, and an optional destination: `None` means
broadcast, `Some(addr)` means addressed to the bound VT. You take that value
and dispatch it through your own send path.

## Lifecycle and state machine

`VTState` enumerates the connect-and-upload path. The client advances through it
as a pump: inbound frames drive transitions in `handle_vt_message`, and
`update` performs the time-based, send-side steps.

| State | What it means | What `update` does here |
| --- | --- | --- |
| `Disconnected` | No session. | Nothing. |
| `WaitForVTStatus` | Listening for a VT to announce itself. | Times out to `Disconnected`. |
| `SendWorkingSetMaster` | A VT was found; announce our working set. | Emits the Working Set Master frame, advances. |
| `SendGetMemory` | Ask the VT to reserve room for the pool. | Emits Get Memory with the serialized size, advances. |
| `WaitForMemory` | Waiting for the VT's memory verdict. | Times out to `Disconnected`. |
| `UploadPool` | Stream the serialized pool. | Emits the object-pool transfer, advances. |
| `WaitForPoolStore` | Let the transfer drain before ending. | After a settle delay, emits End Of Object Pool. |
| `WaitForEndOfPool` | Waiting for parse/activate result. | Times out to `Disconnected`. |
| `ReloadPool` | Language changed; re-upload. | Loops back to `SendGetMemory`. |
| `Connected` | Pool is live; UI commands allowed. | Nothing time-based. |

The transitions, end to end:

1. **Discover.** `connect` serializes the pool once as a sanity check, clears any
   stale VT binding, and moves to `WaitForVTStatus`. The client now waits for a
   VT to broadcast its status. The first valid status frame binds the session to
   that VT's address and advances to `SendWorkingSetMaster`.
2. **Announce.** The next `update` broadcasts the Working Set Master frame so the
   network knows this working set exists, then moves to `SendGetMemory`.
3. **Reserve memory.** The following `update` serializes the pool, sends Get
   Memory carrying the byte size, and waits in `WaitForMemory`. The VT replies
   either "I have room" or "I do not". The hosted server accepts only the
   canonical fixed request shape: command byte, four-byte requested size, then
   `0xFF` reserved tail bytes. On OK the client moves to `UploadPool`;
   otherwise it drops to `Disconnected`.
4. **Transfer.** `update` ships the pool transfer command with the serialized
   bytes (this is what the transport-protocol layer fragments across many CAN
   frames) and moves to `WaitForPoolStore`.
5. **Drain, then end.** The client does **not** send End Of Object Pool
   immediately. It computes a settle delay sized to the transfer, waits that
   long in `WaitForPoolStore` so the multi-frame transfer can finish on the wire,
   then emits End Of Object Pool and waits in `WaitForEndOfPool`.
6. **Activate.** The VT parses the pool and responds. A clean response (no error
   code, no pool-error bitmask) moves the client to `Connected`. Any error fires
   `on_pool_error` and drops the client to `Disconnected`.

Every waiting state is bounded by `config.timeout_ms`; if the partner goes
silent, the client falls back to `Disconnected` rather than hanging. A drop to
`Disconnected` always clears the VT session binding, so a later VT status frame
can start a fresh attempt.

## Version and pool management

Two distinct things get negotiated here, and it helps to keep them apart.

**VT version.** A terminal reports the VT generation it implements in its status
frame. The client records that value (`vt_version_value`), and you state your
own preference with `set_vt_version_preference` or `VTClientConfig::with_version`.
Targeting a lower version keeps your pool compatible with older terminals at the
cost of newer object types.

**Stored pools.** Re-uploading a full pool on every power cycle is slow. The VT
can keep a pool in non-volatile memory under a short label, so the client has a
choice: upload fresh, or ask the VT to reload what it already has.

| Operation | Method | Effect |
| --- | --- | --- |
| List stored labels | `get_versions` | VT replies with its stored labels (`on_versions_received`). |
| Save current pool | `store_version(label)` | Ask the VT to persist the active pool under `label`. |
| Reload a stored pool | `load_version(label)` | Skip the upload; the VT restores `label`. |
| Forget a stored pool | `delete_version(label)` | Remove a stored label. |

`load_version` is the fast path: instead of streaming the whole pool again, the
client sends the label and jumps to `WaitForEndOfPool`, expecting the VT to
restore the stored objects and respond. On success it reaches `Connected`
directly. Newer terminals also support *extended* version labels (longer,
file-style names); the client exposes the parallel
`request_extended_version_label`, `send_extended_store_version`,
`send_extended_load_version`, and `send_extended_delete_version`, and reports
whether the VT supports them via `vt_supports_extended_versions`. A typical
boot sequence is therefore: request versions, and if your label is present call
`load_version`; otherwise upload fresh and `store_version` it for next time.

## Doing it with machbus

For applications, use the session facade; drop to the codec when you need to own
the pump.

### The session facade (recommended)

Plug the [`VtClient`](../guide/session-facade.md) plugin with your object pool and
working set. The plugin drives the whole connect/upload/activate FSM on each tick,
ships the frames for you, and surfaces VT activity as `Event::Vt(VtEvent::…)`. You
point it at a server, then push UI updates through fine control:

```rust
// illustrative shape — the API mirrors the tested `session::plugins::VtClient`
use machbus::session::{Session, EndpointTransport, plugins::VtClient};

let (ctrl, mut driver) = Session::builder(name, 0x80)
    .plug(VtClient::new(VTClientConfig::default(), pool, working_set))
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;

// once an address is claimed, target a VT and let the FSM upload + activate:
ctrl.with_mut::<VtClient, _>(|vt| vt.connect_to(0x26));

loop {
    match driver.poll()? {
        Some(Event::Vt(VtEvent::SoftKey { id, .. })) => { /* react */ }
        Some(_) | None => {}
    }
    // push a UI update when your app state changes:
    ctrl.with_mut::<VtClient, _>(|vt| vt.set_value(numeric_object, 42))?;
}
```

`with_mut::<VtClient>` exposes the same command surface as the codec
(`show`/`hide`, `enable`/`disable`, `set_value`, `set_string`,
`change_active_mask`, …); each is buffered and shipped on the next tick. Soft-key,
button, and value-change activations arrive as `VtEvent` — match them on
`driver.poll()` or filter with `controls.drain::<VtEvent>()`.

### Driving the codec directly

The `vt_client_demo` example walks the whole connect FSM in software, standing in
for a VT server at address `0x80`. Start by building a minimal pool — a working
set object that points at one data mask — and calling `connect`:

```rust
{{#include ../../../examples/vt_client_demo.rs:16:28}}
```

`connect` only arms the state machine; it does not block. From here you pump.
When the (simulated) VT broadcasts its status, you feed it in, then call `update`
to get the next outbound frame:

```rust
{{#include ../../../examples/vt_client_demo.rs:30:48}}
```

Each `update` returns a `Vec<ClientOutbound>` in emission order — usually one
frame, but the upload step can produce the pool transfer and then, a tick later,
the End Of Object Pool. You ship each `ClientOutbound` exactly as given: its
`pgn`, `data`, and `dest`. The example continues by feeding a memory-OK reply,
pumping the transfer and end-of-pool, feeding the activation reply, and then —
once `state()` is `VTState::Connected` — calling `change_numeric_value` to push a
UI update.

The command surface (all of which require the `Connected` state) covers the
common UI operations: `hide_show`, `enable_disable`, `change_numeric_value`,
`change_string_value`, `change_active_mask`, `change_soft_key_mask`,
`change_alarm_soft_key_mask`, `change_attribute`, `change_size`,
`change_background_colour`, `change_child_location`, `change_child_position`,
`change_list_item`, `select_colour_map`, `select_colour_palette`,
`select_input_object`, `lock_unlock_mask`, `control_audio_signal`,
`set_audio_volume`, and `execute_macro`. Each returns a `ClientOutbound`
addressed to the bound VT, or `Error::not_connected` if you call it too early.
Technical-data helpers follow the same rule. Most are fixed `[code][FF×7]`
requests, but WideChar discovery is a real query: use
`get_supported_widechars()` for code plane 0 over the full range, or
`get_supported_widechars_range(code_plane, first, last)` when you need the
standard clipped range response. The hosted server validates the same reserved
bytes for parameterless technical-data requests, so requests such as Get
Hardware, Get Number of Soft Keys, Get Text Font Data, and Get Window Mask Data
must keep bytes 1 through 7 as `0xFF`.

## Events and responsibilities

The client is event-driven. Subscribe to the `Event` fields before you connect,
and the client will emit into them as inbound frames arrive:

| Event | Fires when | You typically |
| --- | --- | --- |
| `on_state_change` | The FSM transitions. | Log progress; gate UI commands on `Connected`. |
| `on_soft_key` | A soft key is activated. | Map `(ObjectID, ActivationCode)` to an action. |
| `on_button` | A button object is activated. | Same, for button objects. |
| `on_numeric_value_change` | The operator edits a number. | Update your app model from `(ObjectID, u32)`. |
| `on_string_value_change` | The operator edits a string. | Update your app model from `(ObjectID, String)`. |
| `on_pool_error` | The VT rejects the pool. | Inspect the error byte; fix the pool. |
| `on_active_ws_status` | This working set becomes (in)active. | Show/hide your interface accordingly. |
| `on_language_change` | The VT's language differs from yours. | Reload a localized pool (see below). |
| `on_unsupported_function` | The VT can't do a function you used. | Degrade gracefully; check `unsupported_functions`. |
| `on_versions_received` | A version list arrives. | Decide upload-fresh vs `load_version`. |
| `on_store_version_response` / `on_load_version_response` | A store/load completes. | Read `(success, error_code)`. |

Two responsibilities are non-negotiable. First, **only send UI commands while
`Connected`** — every command method enforces this and returns an error
otherwise, but you should also gate your own logic so you are not generating
churn into the void. Second, to detect whether *your* working set is the active
one on the terminal, call `set_self_address` with your control function's
address; until you do, the client never claims active-WS status and
`on_active_ws_status` stays quiet.

## Edge cases and failures

- **Pool won't serialize.** `connect` serializes the pool up front and fails
  immediately if it cannot — for example a working set with too many children to
  encode. You get an error before any frame leaves, and the state stays
  `Disconnected`. The send-side steps also re-check this and bail to
  `Disconnected` rather than emit a malformed transfer.
- **Empty pool.** `connect` rejects an empty pool with an invalid-state error.
  There is nothing to draw, so there is nothing to connect with.
- **VT reports no memory.** If the Get Memory reply says "not enough room", the
  client drops to `Disconnected`. The pool is too large for that terminal;
  shrink it or target a different VT.
- **Pool parse error.** A non-zero error code or pool-error bitmask in the End Of
  Object Pool response fires `on_pool_error` with the reported byte and drops the
  session. Unknown object type, duplicate object ID, and a missing child
  reference all surface here — fix the offending object and re-upload.
- **End sent before the transfer drains.** Sending End Of Object Pool while the
  multi-frame transfer is still on the wire confuses the terminal. The client
  avoids this by waiting a settle delay sized to the transfer length before it
  emits End Of Object Pool; do not shortcut that step.
- **VT busy or silent.** Every waiting state is timeout-bounded by
  `config.timeout_ms`. A partner that stops responding lands you back in
  `Disconnected`, where a fresh VT status frame can begin a new attempt.
- **Frames from the wrong VT.** Once bound, the client ignores VT frames whose
  source is not the bound VT, so a second terminal on the bus cannot hijack the
  session mid-flight.

## Advanced

- **Multiple VTs on the bus.** Several terminals may broadcast status. The client
  binds to the first valid status it sees and ignores the rest for the rest of
  the session (`vt_address` reports which one). If you need to target a specific
  terminal, drive `connect`/`disconnect` so you bind during the window the
  intended VT is announcing.
- **Language and units changes.** The operator can switch the cab's language at
  any time, broadcast over the language command PGN. Feed those frames to
  `handle_language_command`. If `auto_reload_on_language_change` is on (the
  default) and the VT's language differs from yours, the client fires
  `on_language_change` and, while `Connected`, moves to `ReloadPool` to re-upload
  a pool built for the new language. Toggle this with
  `set_auto_reload_on_language_change` if your application manages localization
  itself.
- **Swapping pools at runtime.** While `Connected`, `swap_pool` re-uploads a new
  pool (optionally storing the old one first), and `quick_swap_to_version` reloads
  a previously stored pool by label without a full transfer.
- **Reconnect.** Because a drop to `Disconnected` clears the session binding, the
  reconnect story is simply: keep feeding inbound frames. The next VT status frame
  restarts the lifecycle from `WaitForVTStatus` with no special handling on your
  part.
- **Macros.** Register reusable command sequences with `register_macro` and fire
  them by ID with `execute_macro`; the client emits `on_macro_executed` when it
  ships one.

## Validate locally

```sh
make run EXAMPLE=vt_client_demo
make test
```

The example runs the full connect FSM against a simulated VT entirely in
software: it builds a pool, connects, walks Disconnected → WaitForVTStatus →
SendWorkingSetMaster → SendGetMemory → WaitForMemory → UploadPool →
WaitForEndOfPool → Connected, asserts the state reaches `Connected`, and then
sends one `change_numeric_value` command. `make test` exercises the client's
transition, timeout, and validation paths.

## What this proves / does not prove

Proves: the connect-and-upload state machine, the memory/version negotiation,
the transfer-then-end ordering with its settle delay, and the inbound-event
fan-out behave correctly in software, and the `machbus` API drives them as
described.

Does not prove: rendering on a real terminal, interoperability with a specific
third-party VT, or any conformance/certification claim. `machbus` is not
certified; real deployment still needs official standards, real hardware, and
interoperability evidence.

## See also

- [Virtual Terminal concepts](../standards/virtual-terminal.md) — the model behind
  working sets, masks, and object pools.
- [VT object pools](vt-object-pools.md) — how the interface tree is built and
  serialized.
- [VT updates](vt-updates.md) — the command surface for keeping the UI in sync.
- [Virtual Terminal server](virtual-terminal-server.md) — the other side of this
  handshake.
