# 8. Your first Task Controller client

> **Anchor example:** `examples/tc_client_demo.rs` — run it any time with
> `cargo run --example tc_client_demo`.

In [chapter 7](virtual-terminal.md) we gave the implement a screen to talk to the
operator. Now we give it the quieter partner that needs no screen at all: a
**Task Controller**. Where the Virtual Terminal is about *showing* things to a
person, the Task Controller is about *logging and controlling* the actual field
work — how much product to apply, which sections are on, and what was really
done. A documentation TC mostly listens (it records measured values for the
field log); a control TC also talks back (it sends setpoints down).

Here is the key idea, and it mirrors the VT one: before the implement can trade
any work data, it must **describe itself** first. With the VT it uploaded an
object pool of screens. With the TC it uploads a **device description** — a small
tree called the **DDOP** (Device Descriptor Object Pool) that says "this is what
I am and these are the values I can talk about." Only after the TC has that
description, and has activated it, do the two sides start exchanging
**process data**.

By the end of this chapter you will have driven a TC client from `Disconnected`
all the way to `Connected`, watching its connect state machine advance one step
at a time as you feed it the frames a real Task Controller would send.

The deep reference for behavior is the
[Task Controller client tutorial](../tutorials/task-controller-client.md) and
[Task Controller concepts](../standards/task-controller.md). This
chapter is hands-on: add a bit, run it, see what happens.

## What we are building

The example stands in for the whole bus in software. It plays the implement (the
TC client) *and* fakes the Task Controller at address `0x33` by feeding the
client the frames a real TC would send. That lets us watch every step of the
connect handshake without any hardware. The program will:

1. build a tiny DDOP and a client,
2. `connect()`, then hear the TC announce itself,
3. announce a working set and negotiate the TC's version,
4. upload the DDOP and activate it, landing on `Connected`.

## Step 1 — build a DDOP and a client

A TC client always starts from a **DDOP**: the description of the machine the TC
will log and control. Ours is the smallest pool that means anything — one
**device object** (the implement as a whole, with a designator and software
version) that owns one **device element** (the `Root` of the device tree):

```rust
{{#include ../../../examples/tc_client_demo.rs:22:34}}
```

`TaskControllerClient::new` takes a `TCClientConfig` (the default is fine — a
six-second per-step timeout, a version-4 client). `set_ddop` hands it the device
description. `connect()` does **not** block and does **not** put anything on the
wire yet; it validates the DDOP, arms the state machine, and moves the client to
"listening for a TC". Right after it returns, `client.state()` reports it is
waiting for the server's status.

```rust
{{#include ../../../examples/tc_client_demo.rs:36:39}}
```

> **Why so small?** A real DDOP describes every boom, section, bin, and reportable
> quantity, each with a DDI and units. That is a topic of its own — see the
> [DDOP tutorial](../tutorials/ddop.md). Here we keep it to one device and one
> element so the *handshake* is the only thing on screen.

## Step 2 — the TC announces itself

A real Task Controller periodically broadcasts a process-data **status** frame.
The client is waiting for exactly that: the first valid status binds the session
to that TC's address and advances the FSM. We fake the broadcast (TC at `0x33`)
and feed it in with `handle_tc_message`:

```rust
{{#include ../../../examples/tc_client_demo.rs:41:51}}
```

This is the same tick-and-pump rhythm from
[chapter 2](hello-world-explained.md), specialized for the TC: **inbound frames
drive transitions (`handle_tc_message`); `update` performs the send-side steps**
and hands you back the frames to ship. After this status, `tc_address()` is bound
to `0x33` — every reply from here on must come from that source, and frames from
any other address are refused.

## Step 3 — announce the working set, then ask the TC's version

With a TC in sight, the client announces which working set owns the upcoming pool
and then asks the TC what protocol version it speaks. Each `update` performs one
send-side step:

```rust
{{#include ../../../examples/tc_client_demo.rs:53:60}}
```

The first `update` ships the **Working Set Master** announcement (member count
one). The second ships the **version request**. Version negotiation comes before
anything substantial on purpose: a connection only works at the level *both* ends
support, so "what version is this TC" is the first fact the client establishes.

## Step 4 — the version reply

The TC answers with its protocol version and its technical capabilities (how many
booms and sections it can handle). We feed that reply back and read the recorded
version:

```rust
{{#include ../../../examples/tc_client_demo.rs:63:72}}
```

`tc_version()` now reports what the TC told us. A malformed version reply is
ignored and the client keeps waiting until the timeout, rather than proceeding on
bad data.

## Step 5 — upload the DDOP, then activate it

Now the description goes up. `update` serializes the DDOP and emits it as an
object-pool transfer (on a real bus, large pools are fragmented across many CAN
frames by the transport-protocol layer):

```rust
{{#include ../../../examples/tc_client_demo.rs:74:80}}
```

The TC replies with a verdict on the pool — accepted or rejected — and the client
then sends the **activate** command and waits for its acknowledgement. A zero
status means success:

```rust
{{#include ../../../examples/tc_client_demo.rs:82:95}}
```

When the activation reply comes back clean, the client reaches
`TCState::Connected`. The example asserts exactly that. Only now — pool uploaded
*and* activated — is it meaningful to emit process data.

## Step 6 — the connect handshake, in one picture

What you just pumped through is a fixed sequence of states. It is worth seeing
the whole path at once:

```
Disconnected
   │  connect()  (validates the DDOP)
   ▼
WaitForServerStatus    ◄── TC status frame binds the server address
   │  update()
   ▼
SendWorkingSetMaster   ── emits Working Set Master
   │  update()
   ▼
RequestVersion         ── emits the version request
   │
   ▼
WaitForVersion         ◄── version + capabilities reply
   │  update()
   ▼
TransferDDOP           ── emits the device-description upload
   │
   ▼
WaitForPoolResponse    ◄── pool accepted
   │  update()
   ▼
ActivatePool           ── emits the activate command
   │
   ▼
WaitForActivation      ◄── activation reply, no error
   │
   ▼
Connected              ── process data now allowed
```

Each "waiting" state is bounded by the config timeout: if the TC goes silent, the
client falls back to `Disconnected` instead of hanging, and a later status frame
can start a fresh attempt. There is also a branch the demo skips for brevity — the
**label check** — described next.

## Label-based caching: the upload you can skip

A real client does not always re-upload its DDOP. Between announcing its version
and transferring the pool, it can ask the TC two questions: *do you already hold
my structure?* and *do you already hold my localization (language/units)?* The
device object carries a **structure label** and a **localization label** for
exactly this. If the TC answers that both already match, the client skips the
whole transfer and jumps straight to activation; if they do not match, it deletes
the stale pool first and then uploads. Set stable labels on your device object so
a TC that has met your implement before does not re-download the pool every
session. The full label-driven decision is in the
[client tutorial](../tutorials/task-controller-client.md#lifecycle-and-state-machine).

## Process data: the live conversation

Once `Connected`, the handshake is over and the running exchange begins. Every
process-data message names an **element** (which part of your device, from the
DDOP) and a **DDI** (which quantity, from the shared data dictionary), plus a
32-bit value where one applies. It flows both ways:

- **Up — measured values.** The implement reports what it is actually doing: real
  rates, flows, counts, section states. A documentation TC logs these. You build
  an ECU→TC value frame and ship it; the client also calls your value-request
  handler when the TC asks for a current reading.
- **Down — setpoints and commands.** A control TC sends what it *wants* done:
  target rates, section on/off. The client calls your value-command handler with
  the element, DDI, and value. A downward command is a *request* — your
  application validates it against the real machine state and decides whether
  acting on it is safe. The implement always owns its own behaviour.

A DDI is only meaningful *with* its owning element: the same "actual rate" DDI can
appear once per section, so "DDI X changed" is ambiguous until you know which
element reported it. The DDOP is the map that removes that ambiguity. The
message-level detail is in
[DDOP and process data](../standards/task-controller.md).

## Step 7 — run it

```sh
cargo run --example tc_client_demo
```

You should see the handshake march through its steps, each line printed by the
example as it advances:

```text
=== TC Client Demo ===
[1] connect → WaitForServerStatus
[2] TC_STATUS → SendWorkingSetMaster, tc_addr=0x33
[3] sent WS Master + VERSION_REQUEST → WaitForVersion
[4] TC version=4, → TransferDDOP
[5] DDOP frame size=8 bytes → WaitForPoolResponse
[6] activated → Connected  ✓
```

Read it top to bottom and you can see the whole lifecycle: connect arms the
machine, the TC status binds the server at `0x33`, the working-set announcement
and version request go out, the version reply advances us, the DDOP uploads, and
the activation reply flips us to `Connected`. Only after that last line would real
process data start to flow.

## Things that trip people up

- **Process data before `Connected`.** Reporting values against a pool the TC has
  not activated is meaningless, and the client's state guards exist to stop it.
  Watch `state()` (or the state-change event) and gate your own logic on
  `Connected` — do not emit work data into the void.
- **Skipping the version handshake.** Both ends cap themselves to the version and
  capabilities the other actually supports. Establish "what version is this TC"
  and "how many sections does it support" first, not as an afterthought.
- **A DDOP the TC rejects.** A non-zero pool response means the upload was
  refused; the client returns to `Disconnected` rather than pretending the pool is
  live. Re-check the DDOP for duplicate object IDs or dangling references before
  retrying.
- **A value out of range.** Element numbers are carried in a 12-bit wire field, so
  a number that does not fit is rejected when you build the payload — a coding
  error surfaces as an error, not a corrupted frame on the bus.
- **A reply from the wrong TC.** Once a TC is bound, frames from any other source
  address are refused. Make sure the TC you target is the one whose status you
  first heard.

## Validate locally

```sh
cargo run --example tc_client_demo
make test
```

`make test` exercises the client's label-driven upload/skip/delete decision, the
timeout-to-`Disconnected` path, the re-upload sequence, and the value-request and
setpoint callbacks — well beyond the happy path the example walks.

## What this proves / does not prove

Proves: machbus can drive a TC client through its full connect handshake —
discover the TC, announce a working set, negotiate the version, upload the device
description, and activate it — landing on `Connected`, all from a few lines of
Rust against a simulated Task Controller.

Does not prove: interoperability with a specific third-party Task Controller,
real-hardware timing or bandwidth behaviour, or any conformance/certification
claim. machbus is not certified; real deployment still needs official standards,
real hardware, and interoperability evidence.

## Next

→ [9. Tractor and implement personas](tractor-and-implement.md) — step back from a
single service and wire the whole machine together: a tractor ECU and an implement
ECU talking across the bus.

## See also

- [Task Controller client](../tutorials/task-controller-client.md) — the connect
  FSM, the label-based upload decision, and the process-data callbacks in depth.
- [DDOP](../tutorials/ddop.md) — how to build and validate the device description
  you upload (more than our one-element demo).
- [DDOP and process data](../standards/task-controller.md) — the
  message-level view of the device description and the live values.
