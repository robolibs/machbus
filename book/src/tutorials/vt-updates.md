# VT updates

Once your object pool is live on the terminal, you do not re-upload it to change
what the operator sees. You send small **runtime commands** that mutate
individual objects in place: bump a numeric value, replace a label, hide a
widget, switch to a different screen. This tutorial explains that command
surface, how each command targets an object, how the terminal answers, and how
`machbus` lets you batch updates so you do not drown the bus in traffic.

It assumes the pool is already uploaded and the client has reached the connected
state. If you are not there yet, read
[Virtual terminal client](virtual-terminal-client.md) first — uploading is a
one-time event; everything on this page happens afterward, repeatedly, for the
life of the connection.

## Why this exists

A pool upload is expensive: it streams every object across a transport-protocol
session and the terminal validates and lays it out. You do that once. But a
working machine changes constantly — a tank level falls, a speed readout ticks,
an alarm fires, the operator pages to a settings screen. Re-sending the whole
pool for each of those would saturate a shared CAN bus and stall every other
control function on it.

So ISO 11783-6 (Virtual Terminal) splits the job. The pool defines the
*structure and identity* of every object. A separate, compact family of
ECU-to-VT commands carries *runtime changes*, each one naming a single object by
its **Object ID** and saying what to do to it. The terminal already holds the
object; the command just nudges it.

## Mental model

```
   ┌─────────────── your implement ECU (Working Set) ───────────────┐
   │  application logic: "tank is now 73%, show the run mask"        │
   │            │                                                    │
   │            ▼                                                    │
   │   build a command targeting an Object ID                       │
   │   change_numeric_value(level_id, 73)                           │
   │   change_active_mask(ws_id, run_mask_id)                       │
   └────────────┬───────────────────────────────────────────────────┘
                │  one short ECU→VT frame per command
                ▼
        ┌───────────────┐   applies change, redraws the object
        │  VT terminal  │   ───────────────────────────────────►  screen
        └───────┬───────┘
                │  response frame: success, or an error code
                ▼
        your error handler / state tracker
```

The pool is the noun store; commands are the verbs. Each verb references one
noun by ID, the terminal performs it and (for most commands) answers with a
response that is either "done" or a coded rejection.

## Anatomy: the command families

`machbus` names the wire function codes in `cmd` (see
`src/isobus/vt/commands.rs`) and exposes one builder method per command on
`VTClient`. Each builder returns a `ClientOutbound` — a ready-to-send frame on
the ECU-to-VT PGN, addressed to the terminal — that you hand to your transport.
The runtime command set groups into a few intentions:

| Intention | `cmd` code | `VTClient` method | What it changes |
| --- | --- | --- | --- |
| Change numeric value | `CHANGE_NUMERIC_VALUE` | `change_numeric_value(id, value)` | The numeric value held by an output number, meter, bar graph, or similar. |
| Change string value | `CHANGE_STRING_VALUE` | `change_string_value(id, &str)` | The text of an output/input string object. |
| Hide / show | `HIDE_SHOW` | `hide_show(id, visible)` | Whether a container (and its children) is drawn. |
| Enable / disable | `ENABLE_DISABLE` | `enable_disable(id, enabled)` | Whether an input object accepts operator interaction. |
| Change active mask | `CHANGE_ACTIVE_MASK` | `change_active_mask(ws_id, mask_id)` | Which Data or Alarm Mask is the active screen for a working set. |
| Change soft-key mask | `CHANGE_SOFT_KEY_MASK` | `change_soft_key_mask(data_mask_id, sk_mask_id)`, `change_alarm_soft_key_mask(alarm_mask_id, sk_mask_id)` | Which soft-key bank is shown alongside a Data Mask or Alarm Mask. |
| Select colour map / palette | `SELECT_COLOUR_MAP` | `select_colour_map(id)`, `select_colour_palette(id)` | Which Colour Map or Colour Palette remaps VT colour indexes. |
| Change attribute | `CHANGE_ATTRIBUTE` | `change_attribute(id, attribute_id, value)` | A single addressable attribute of any object that exposes one. |
| Change list item | `CHANGE_LIST_ITEM` | `change_list_item(list_id, index, new_item_id)` | Which child object occupies a slot in a list. |
| Change child location | `CHANGE_CHILD_LOCATION` | `change_child_location(parent, child, dx, dy)` | A child's position by a relative offset within its parent. |
| Change child position | `CHANGE_CHILD_POSITION` | `change_child_position(parent, child, x, y)` | A child's absolute position within its parent. |
| Change size | `CHANGE_SIZE` | `change_size(id, width, height)` | An object's width and height. |
| Change background colour | `CHANGE_BACKGROUND_COLOUR` | `change_background_colour(id, colour)` | An object's background colour index. |
| Select input object | `SELECT_INPUT_OBJECT_COMMAND` | `select_input_object(id, option)` | `0xFF` focuses an input field/Button/Key or clears focus for NULL; `0` opens an input field for data input. |
| Lock / unlock mask | `LOCK_UNLOCK_MASK` | `lock_unlock_mask(mask_id, lock, timeout_ms)` | Freeze a mask's display so a burst of updates lands atomically. |

The "change attribute" command is the general escape hatch: any object attribute
that carries an attribute ID and is not read-only can be set with it, which
covers font, line, fill, visibility flags, and colours beyond the dedicated
commands above. The dedicated commands exist because the standard groups
commonly-changed attributes into one compact message for efficiency — for
example, font properties travel together rather than as several separate
attribute writes.

### How a command references its target

Every command carries the Object ID of the thing it acts on, encoded
little-endian. In `machbus` that ID is `ObjectID`, and the builder methods accept
anything that converts into one (`impl Into<ObjectID>`), so a bare integer
literal works:

```rust
// illustrative shape, not a compiled call
let frame = client.change_numeric_value(0xCAFE, 73)?;   // 0xCAFE → ObjectID
let frame = client.hide_show(detail_panel_id, false)?;  // hide the panel
```

A handful of commands name *two* objects. `change_active_mask` names the
**working set** and the **mask** to make active. The standard
`CHANGE_SOFT_KEY_MASK` frame names a **Data Mask** or **Alarm Mask**, selected by
its Mask Type byte, plus the **soft-key mask** to pair with it; the
`VTClient::change_soft_key_mask` convenience method emits the Data Mask variant
and `VTClient::change_alarm_soft_key_mask` emits the Alarm Mask variant.
`change_list_item`,
`change_child_location`, and `change_child_position` name a **parent** plus the
**child** inside it. The terminal already knows the object owner from the source
address of your frame, so commands never repeat that.

## The response and error model

Most commands have a matching VT response. The pattern is uniform: you send the
command, the terminal applies it (or refuses), and answers on the VT-to-ECU PGN
with the same function code plus an error indication. A zero error means the
change took effect; a non-zero code is a rejection that tells you *why*. Typical
rejections are an unknown Object ID, a value outside the object's allowed range,
a type mismatch, or a command sent against an object whose mask is not in a
state to accept it.

Two consequences shape how you write the client:

- **A sent command is not a confirmed change.** Until the response arrives the
  terminal's view and your internal value may differ. This is why the update
  helper only updates its cached state once you confirm a send succeeded, rather
  than optimistically the moment you build the frame.
- **Operator input races your commands.** The operator may be editing the very
  object you are updating. The standard makes the working set responsible for
  validating what comes back and for avoiding changes to an object that is open
  for input where doing so would disrupt the interaction. Newer terminals accept
  attribute changes even on an in-use object and may cache them until the edit
  finishes; older deprecated behavior returned an "object in use" rejection. Do
  not assume either — handle the response code you actually get.

## Doing it with machbus

### Sending a single command

After the client reports connected, every command builder is callable. They all
guard on connection state and return `Err` if you call them too early. The VT
client demo connects a client and then sends one update:

```rust
{{#include ../../../examples/vt_client_demo.rs:77:85}}
```

`change_numeric_value` returns a `ClientOutbound { pgn, dest, data }` — the
terminal's address in `dest`, the encoded command in `data`. You transmit that
through whatever link the rest of your stack uses. The same shape applies to
every builder in the table above.

### Switching screens

Changing the active mask is the command behind "go to the run screen" or "open
settings". It names the working set whose screen changes and the mask to show:

```rust
// illustrative shape
let frame = client.change_active_mask(working_set_id, run_mask_id)?;
```

The terminal answers with a change-active-mask response. Note that the active
mask the terminal reports back is the authoritative one — the helper below does
*not* pre-cache an active-mask change, precisely because the terminal echoes the
real active mask in its own status, and that is what your state tracker should
believe.

## The VtBatch helper: coalescing and deduplicating

If your control loop runs at, say, 50 Hz and naively sends every value it touches
each tick, you will flood the bus with redundant frames — many of them setting a
value to what it already is. `machbus` gives you `VTClientUpdateHelper` (in
`src/isobus/vt/update_helper.rs`) to stop that at the source. It does two things:

1. **Deduplicates against cached state.** The helper borrows a
   `VTClientStateTracker`. When you call `set_numeric_value(id, v)` and the
   tracker already has `v` for that object, it returns `None` — no frame is
   produced. The same short-circuit applies to strings, visibility, enable
   state, and the active mask.
2. **Coalesces a batch to last-write-wins.** Between `begin_batch()` and
   `end_batch()`, setters queue ops keyed by *slot* — the (kind, Object ID)
   pair. Writing the same slot twice keeps only the last value. Writing a value
   and then reverting it to what the tracker already holds removes the pending
   op entirely. `end_batch()` drains the deduplicated `Vec<UpdateOp>` for you to
   send.

The setters return an `UpdateOp` (or queue it, in batch mode). To turn one into a
frame, call `op.to_client_outbound(&client)`, which maps it to the canonical
command for that kind. After a send succeeds, call `helper.confirm(&op)` so the
tracker's cache reflects the new value and future identical writes short-circuit.

```rust
// illustrative shape of a batch
helper.begin_batch();
helper.set_numeric_value(speed_id, 1200);   // queued
helper.set_numeric_value(speed_id, 1250);   // coalesces: last value wins
helper.set_string_value(label_id, "READY"); // queued
helper.hide(spinner_id);                     // queued
for op in helper.end_batch() {               // drains 3 ops, not 4
    let frame = op.to_client_outbound(&client)?;
    // ...transmit frame...
    helper.confirm(&op);
}
```

Convenience wrappers sit on top of the bare setters: `show`/`hide`,
`enable`/`disable`, `set_numeric_clamped` to pin a value into a range, and
`set_numeric_scaled` / `try_set_numeric_scaled` to apply a `(value + offset) *
scale` conversion. The fallible `try_*` forms reject non-finite input or a
scaled result outside the `u32` wire domain instead of silently saturating; the
plain forms drop such input as `None` for compatibility. `try_set_string_value`
likewise rejects a string longer than the two-byte length field before it can
enter a batch.

If you attach a pool with `with_pool(&pool)`, `change_active_mask` validates that
the target ID exists and is a Data or Alarm Mask, returning `Err` otherwise — a
cheap guard against switching to a non-mask object.

## Rate and throttling considerations

A CAN bus and the terminal both have finite bandwidth, shared across every
working set using the terminal. ISO 11783-6 recommends sending a command only
when the visible data actually changed, and reducing or stopping updates for a
working set whose mask is not currently shown. Practical rules:

- Update only what is on screen. There is no point streaming a gauge that lives
  on a mask the operator is not looking at — but you *may* keep an off-screen
  mask's values current so it is ready when activated.
- Let the helper's deduplication do the throttling. If a value has not changed,
  no frame is sent, which naturally collapses a fast loop to the rate of real
  change.
- The numeric-value command in particular is rate-limited by the standard;
  do not exceed the permitted update frequency for a single object.
- Batch related updates and, when several must land together visually, consider
  `lock_unlock_mask` so the operator does not see a half-updated screen.

## Events and responsibilities

Your application owns both directions of this exchange:

| Event | Source | Your responsibility |
| --- | --- | --- |
| Command response (success) | VT | Treat the change as applied; `confirm` the op so the tracker caches it. |
| Command response (error code) | VT | Decode the rejection; correct the value, ID, or timing and retry as appropriate. |
| Operator input (key, button, numeric/string change) | VT | Update your internal model; you may have raced a command you sent. |
| Active-mask change notification | VT | Trust the terminal's reported active mask over any local guess. |

The one discipline that prevents most surprises: do not assume a command
succeeded. Wait for the response, and key your cached state off confirmed sends,
which is exactly the contract `confirm` encodes.

## Edge cases and failures

- **Updating before connected.** Every command builder returns `Err` if the
  client is not connected. Reaching the connected state is a precondition, not a
  best effort.
- **Unknown Object ID.** The terminal rejects a command naming an object that is
  not in its copy of the pool. With a pool attached, the helper catches the
  active-mask case locally; other commands surface it as a response error.
- **Value out of range or wrong type.** A numeric value the object cannot hold,
  or a string longer than the length field, is rejected. The `try_*` helpers
  catch the encodable-range problems before they ever reach the bus.
- **Flooding the bus.** Naive per-tick sends starve other nodes. Deduplicate,
  batch, and gate on real change.
- **Racing the operator.** A command and an operator edit can cross in flight.
  Validate what comes back; avoid mutating an object that is open for input
  unless you are sure the change is harmless.

## Advanced

- **Batching strategy.** Group a frame's worth of related changes per control
  cycle, drain once, send, confirm. The slot-keyed coalescing guarantees one
  frame per object per batch regardless of how many times your code touched it.
- **Partial updates.** Keep an off-screen mask's values current with cheap
  numeric/string updates so activating it is instant, while suppressing redraw
  churn for objects nobody is viewing.
- **Animation and metering.** Drive a bar graph or meter by repeatedly changing
  its numeric value; the helper's dedup means a held value costs nothing, so you
  only pay for actual motion. Respect the per-object numeric update-rate limit.
- **Atomic screens.** When several objects must change together without an
  intermediate flicker, lock the mask, push the batch, then unlock — the
  timeout argument bounds how long the freeze can last if you never unlock.
- **Surface vs low-level.** The helper centralizes the `UpdateOp` → wire-command
  mapping and the dedup logic; the raw `VTClient` builders give you every command
  directly when you need one the helper does not wrap.

## Validate locally

```sh
make run EXAMPLE=vt_client_demo
make test
```

The demo walks a client to connected and then sends a single
`change_numeric_value`, printing the resulting frame's PGN, destination, and
length. The update-helper module's own tests assert the dedup-and-coalesce
behavior end to end, including last-write-wins, revert-to-cached removal, and the
canonical command bytes each `UpdateOp` produces.

## What this proves / does not prove

Proves: the command builders encode the documented ECU-to-VT layouts, the helper
deduplicates and coalesces updates as specified, and the connection guard rejects
commands sent too early — all in software.

Does not prove: how a specific real terminal renders or rate-limits these
commands, interoperability with any particular VT, or any conformance or
certification claim. `machbus` is not certified; real deployment still needs the
official standards, real hardware, and interoperability evidence.

## See also

- [Virtual terminal client](virtual-terminal-client.md) — getting to the
  connected state these commands require.
- [VT object pools](vt-object-pools.md) — the objects and IDs your commands
  target.
- [Virtual terminal concepts](../standards/virtual-terminal.md) —
  the conceptual primer on masks, working sets, and the terminal model.
