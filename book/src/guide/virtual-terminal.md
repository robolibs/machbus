# 7. Your first Virtual Terminal client

> **Anchor example:** `examples/vt_client_demo.rs` — run it any time with
> `cargo run --example vt_client_demo`.

So far our node has claimed an address, swapped messages, moved big payloads,
and answered diagnostics. It is a good bus citizen, but it has no way to talk to
the person in the cab. That is the job of a **Virtual Terminal client**.

Here is the key idea: an implement usually has no screen of its own. Instead it
borrows the terminal's. The implement ships the terminal a *description of its
user interface* — a tree of objects called an **object pool** — and the terminal
draws it. From then on the implement sends small commands ("set this number to
42") and receives input events ("soft key 7 was pressed") back over the bus.

By the end of this chapter you will have driven a VT client from
`Disconnected` all the way to `Connected`, watched its connect state machine
advance step by step, and pushed one runtime update to the screen. We will keep
it focused: building a *real* pool is a big topic with its own tutorial, so our
pool here is deliberately tiny.

The deep reference for behavior is the
[Virtual Terminal client tutorial](../tutorials/virtual-terminal-client.md) and
[Virtual Terminal concepts](../standards/virtual-terminal.md). This
chapter is hands-on: add a bit, run it, see what happens.

## What we are building

The example stands in for the whole bus in software. It plays the implement
(the VT client) *and* fakes the terminal at address `0x80` by feeding the client
the frames a real terminal would send. That lets us watch every step of the
connect handshake without any hardware. The program will:

1. build a minimal object pool and a client,
2. `connect()` and pump the connect state machine to `Connected`,
3. react to the terminal's replies along the way, and
4. send one UI command once the pool is live.

## Step 1 — build a pool and a client

A VT client always starts from an **object pool**: the UI it wants drawn. Ours
is the smallest pool that means anything — a **working set** object (the
implement's "name badge" on the terminal) that points at a single **data mask**
(one full-screen layout):

```rust
{{#include ../../../examples/vt_client_demo.rs:16:28}}
```

`VTClient::new` takes a `VTClientConfig` (the default is fine — a 6-second
per-step timeout, VT version 4). `set_object_pool` hands it the UI tree.
`connect()` does **not** block and does **not** put anything on the wire yet; it
arms the state machine and moves the client to "listening for a terminal". Right
after it returns, `client.state()` reports `WaitForVTStatus`.

> **Why a working set plus a mask?** The working set is your implement's whole
> identity on the terminal; a mask is one screen within it. The terminal needs
> both: something to identify the client, and something to draw. The full object
> model is covered in
> [Virtual Terminal concepts](../standards/virtual-terminal.md).

## Step 2 — the terminal announces itself

A real VT periodically broadcasts a **status** frame. The client is waiting for
exactly that: the first valid status binds the session to that terminal's
address and advances the FSM. We fake the broadcast (terminal at `0x80`,
reporting VT version 4) and feed it in with `handle_vt_message`, then call
`update` to get the first outbound frame:

```rust
{{#include ../../../examples/vt_client_demo.rs:30:48}}
```

This is the same tick-and-pump rhythm from
[chapter 2](hello-world-explained.md), specialized for the VT: **inbound frames
drive transitions (`handle_vt_message`); `update` performs the send-side steps**
and hands you back a `Vec` of frames to ship. Each returned frame is a
`ClientOutbound` carrying a `pgn`, `data`, and `dest` — you route it through
your own send path exactly as given.

Two `update` calls happen here. The first emits the **Working Set Master** frame
(announcing our working set to the network). The second emits **Get Memory**,
which asks the terminal to reserve room for the pool — the example asserts that
frame's first byte is `cmd::GET_MEMORY`.

## Step 3 — memory OK, then upload the pool

The terminal answers Get Memory with a verdict: "I have room" or "I do not".
We feed back an OK reply, then pump the pool **transfer** and the **End Of Object
Pool** marker, and finally feed the terminal's activation reply:

```rust
{{#include ../../../examples/vt_client_demo.rs:51:75}}
```

Notice the ordering. `update` ships the pool transfer first (on a real bus this
is the part the transport-protocol layer fragments across many CAN frames), and
only after that does End Of Object Pool go out. The client deliberately lets the
transfer drain before it sends the end marker — sending "that's all" while the
multi-frame transfer is still in flight confuses a real terminal. When the
terminal's End Of Object Pool reply comes back with no error, the client reaches
`VTState::Connected`. The example asserts exactly that.

## Step 4 — the connect state machine, in one picture

What you just pumped through is a fixed sequence of states. It is worth seeing
the whole path at once:

```
Disconnected
   │  connect()
   ▼
WaitForVTStatus      ◄── VT status frame binds the terminal
   │  update()
   ▼
SendWorkingSetMaster ── emits Working Set Master
   │  update()
   ▼
SendGetMemory        ── emits Get Memory (pool size)
   │  update()
   ▼
WaitForMemory        ◄── "memory OK" reply
   │  update()
   ▼
UploadPool           ── emits the pool transfer
   │  update() (after a settle delay)
   ▼
WaitForEndOfPool     ◄── End Of Object Pool reply, no error
   │
   ▼
Connected            ── UI commands now allowed
```

Each "waiting" state is bounded by the config timeout: if the terminal goes
silent, the client falls back to `Disconnected` instead of hanging, and a later
status frame can start a fresh attempt. The full table — including the language
`ReloadPool` path and the memory-not-OK branch — is in the
[client tutorial](../tutorials/virtual-terminal-client.md#lifecycle-and-state-machine).

## Step 5 — send a runtime update

Now that we are `Connected`, the pool is live on the terminal and we may send UI
commands. The simplest is changing a number — the value behind an output-number
object:

```rust
{{#include ../../../examples/vt_client_demo.rs:78:84}}
```

`change_numeric_value(object_id, value)` returns a `ClientOutbound` addressed to
the bound terminal. This is the everyday loop of a running implement: read your
sensors, then push the changed values down with commands like this. You change
the *value* an object references, not the layout — the layout was uploaded once,
back in step 3.

The command surface is broad (`hide_show`, `enable_disable`,
`change_string_value`, `change_active_mask`, and many more); they all share two
rules: they return a `ClientOutbound` addressed to the terminal, and they all
require the `Connected` state, returning an error if you call them too early.
The full set is in [VT updates](../tutorials/vt-updates.md).

## Step 6 — run it

```sh
cargo run --example vt_client_demo
```

You should see the state machine march through its steps, each line printed by
the example as it advances:

```text
=== VT Client Demo ===
[1] connect()    → WaitForVTStatus
[2] VT_STATUS    → SendWorkingSetMaster
[3] update() → 1 frame (SendGetMemory)
[4] update() → GET_MEMORY (WaitForMemory)
[5] mem OK     → UploadPool
[6] update() → ... frames (pool transfer + EOP)
[7] EOP ack    → Connected  ✓

[ui] change_numeric_value(0xCAFE, 42) → pgn=0x..., dest=0x80, len=8
```

Read it top to bottom and you can see the whole lifecycle: connect arms the
machine, the status frame binds the terminal, each `update` advances one step and
emits the right frame, the activation reply flips us to `Connected`, and only
then does the UI command go out — addressed to the terminal at `0x80`.

## Reacting to operator input

In the demo we drive everything by hand, but a real client is event-driven. The
terminal sends input events *down* when the operator acts, and the client fans
them out to handlers you register before connecting. The ones you will reach for
first:

| Event | Fires when | You typically |
| --- | --- | --- |
| `on_state_change` | The FSM transitions. | Log progress; gate UI commands on `Connected`. |
| `on_soft_key` | A soft key is activated. | Map `(ObjectID, ActivationCode)` to an action. |
| `on_button` | A button object is activated. | Same, for on-screen buttons. |
| `on_numeric_value_change` | The operator edits a number. | Update your app model. |
| `on_active_ws_status` | Your working set becomes (in)active. | Show or hide your interface. |

The pattern is always the same: operator presses a soft key → the terminal sends
an input event up → your `on_soft_key` handler reads the object ID and the
`ActivationCode` (`Pressed`, `Released`, `Held`, `Aborted`) → you run your logic
→ you send commands back down to update the screen. The terminal never runs your
logic; every visible change is a command you issued. The full event list is in
the [client tutorial](../tutorials/virtual-terminal-client.md#events-and-responsibilities).

## Things that trip people up

- **Sending updates before `Connected`.** Every command method enforces the
  `Connected` state and returns an error otherwise. Watch `state()` (or
  `on_state_change`) and gate your own logic on it — do not generate UI churn
  into the void.
- **Ending the pool too early.** Do not try to shortcut the settle delay between
  the pool transfer and End Of Object Pool. The client waits on purpose so the
  multi-frame transfer can finish; sending the end marker over a still-draining
  transfer confuses a real terminal.
- **A pool the terminal rejects.** An unknown object type, a duplicate object ID,
  or a missing child reference surfaces as a pool error in the End Of Object Pool
  reply — the client fires `on_pool_error` and drops to `Disconnected`. Fix the
  offending object and reconnect.
- **No active-WS status.** The client only reports whether *your* working set is
  the active one after you tell it your own address with `set_self_address`;
  until then `on_active_ws_status` stays quiet.

## Validate locally

```sh
cargo run --example vt_client_demo
make test
```

`make test` exercises the client's transition, timeout, and pool-validation
paths beyond the happy path the example walks.

## What this proves / does not prove

Proves: machbus can drive a VT client through its full connect-and-upload state
machine — discover, announce, reserve memory, transfer the pool, end it, and go
active — and then issue UI commands, all from a few lines of Rust against a
simulated terminal.

Does not prove: rendering on a real terminal, interoperability with a specific
third-party VT, or any conformance/certification claim. machbus is not
certified; real deployment still needs official standards, real hardware, and
interoperability evidence.

## Next

→ [8. Your first Task Controller client](task-controller.md) — give the
implement a screen's quieter cousin: a controller that logs and commands its
work without an operator watching.

## See also

- [Virtual Terminal client](../tutorials/virtual-terminal-client.md) — the
  connect lifecycle, version/memory negotiation, and event fan-out in depth.
- [VT object pools](../tutorials/vt-object-pools.md) — how to build a real
  interface tree (more than our one-mask demo).
- [VT updates](../tutorials/vt-updates.md) — the full command surface for keeping
  the UI in sync with your application state.
