# Address claim

Address claim is the first thing almost every control function does on an
ISOBUS or J1939 network. Before a node may send normal application traffic it
has to own a **source address** — a single byte, `0x00`–`0xFD`, that uniquely
identifies it as a message source on the bus. This tutorial explains why that
matters, how the negotiation works end to end, and how to drive it with
`machbus` at both the low level and through the session facade.

If you only read one network-management page, read this one: nearly every other
workflow (VT, TC, FS, diagnostics) assumes the node has already claimed an
address.

## Why this exists

A CAN bus has no master and no central address registry. Any node may transmit,
and arbitration on the wire decides which frame wins when two start at once.
That is fine for moving bytes, but an application needs to know *who* sent a
message and *where* to send a reply. ISOBUS solves this with two linked ideas:

- a permanent 64-bit identity called the **NAME**, baked into the product, and
- a temporary 8-bit **source address** that the node claims at power-up.

Address claim is the handshake that binds one NAME to one address on this
particular bus, at this particular time, and resolves the case where two nodes
want the same address. Because the address is only a label and the NAME is the
real identity, a node can lose an address fight and simply move to another
address without changing who it is.

## Mental model

```
power on
   │
   ▼
pick a preferred address  ──►  announce "NAME X wants address A"
   │                                   │
   │                          someone else also wants A?
   │                          ┌────────┴─────────┐
   │                        no│                   │yes → compare NAMEs
   ▼                          ▼                   ▼
 nobody objects          keep A            lower NAME wins A,
 within the window     (Claimed)           higher NAME must move
                                                   │
                                          self-configurable? ─ yes ─► try next free address
                                                   │
                                                   no ─► give up (Cannot claim, address 0xFE)
```

The whole negotiation is event-driven and bounded by a short waiting window: a
node announces its claim, listens for objections for roughly a quarter second,
and is considered the owner of the address if nobody with a better NAME contests
it.

## Anatomy: the NAME

The NAME is a 64-bit value built from a fixed set of fields. `machbus`
represents it as `net::Name`, constructed with consuming `with_*` setters. Every
field has a meaning that other nodes can read, and the *numeric value of the
whole 64-bit word* is what decides arbitration.

| Field | Width | What it expresses |
| --- | --- | --- |
| Identity number | 21 bits | A per-unit serial-like value; keeps otherwise-identical products distinct. |
| Manufacturer code | 11 bits | Who built the ECU. |
| ECU instance | 3 bits | Which of several identical ECUs for the same function this is. |
| Function instance | 5 bits | Which occurrence of the function on this device. |
| Function | 8 bits | What the node does (for example, a terminal, a task controller, a tractor ECU). |
| Device class | 7 bits | The broad equipment category the function belongs to. |
| Device class instance | 4 bits | Which occurrence of that device class on the network. |
| Industry group | 3 bits | The industry the node belongs to (agriculture, for ISOBUS). |
| Self-configurable address | 1 bit | Whether the node may move to a different address if it loses a fight. |

In `machbus` you build a NAME like this (taken from the address-claim example):

```rust
{{#include ../../../examples/address_claim.rs:14:23}}
```

Two rules matter most when you choose these fields:

1. **Uniqueness.** Two control functions on the same bus must not share a NAME.
   The identity number and instance fields exist precisely so that two copies of
   the same product can coexist. If two nodes really do present identical NAMEs,
   the bus cannot tell them apart and behavior becomes undefined — design your
   identity numbers so this never happens.
2. **The self-configurable bit is a policy choice.** Set it when the node is
   allowed to pick a different address after losing arbitration. Clear it when
   the node must own one specific address or nothing.

### How arbitration reads the NAME

When two nodes claim the same address, the one with the **numerically lower
NAME wins**. Because the assigned-by-industry fields sit in the most significant
bytes while the per-unit identity number sits in the least significant bytes,
the comparison is dominated by *what the node is* before it is decided by
*which specific unit it is*. The practical consequence: arbitration is
deterministic and repeatable. The same two NAMEs always produce the same winner,
which is what makes the network predictable across power cycles.

You can inspect the raw value with `name.raw` to reason about who would win a
contest, exactly as the example prints it:

```rust
{{#include ../../../examples/address_claim.rs:25:29}}
```

## Lifecycle and state machine

A control function moves through a small set of claim states. `machbus` exposes
them as `net::ClaimState`:

| State | Meaning | What the node may send |
| --- | --- | --- |
| Not started / idle | No claim attempted yet. | Nothing application-level. |
| Waiting | A claim was announced; the contention window is open. | Only claim-related traffic. |
| Claimed | The node owns the address. | Full application traffic. |
| Cannot claim | No address could be secured. | Only the special "cannot claim" announcement from the null address `0xFE`. |

The transitions:

1. **Start.** The node announces its NAME at its preferred address and opens a
   short waiting window (the contention timeout is roughly a quarter second).
2. **No contest.** If the window closes with no better claim seen, the node
   becomes **Claimed** and may begin normal traffic.
3. **Contest, you win.** If another node claims the same address with a *higher*
   NAME, you keep the address. The other node must yield.
4. **Contest, you lose.** If another node claims the same address with a *lower*
   NAME, you must stop using it. If your NAME is self-configurable, you retry at
   the next candidate address; otherwise you go to **Cannot claim** and announce
   that from the null address.
5. **Request to claim.** Any node may ask everyone to re-announce. A claimed
   node responds by re-sending its current claim; this is how a newcomer learns
   who owns what.

The two addresses you never claim as a source are the **global/broadcast
address** `0xFF` (a destination, not a source) and the **null address** `0xFE`
(used as the source only for the "cannot claim" message). `machbus` exposes
these as `BROADCAST_ADDRESS` and `NULL_ADDRESS`.

## Doing it with machbus

There are two ways to drive address claim, and they suit different needs.

### The session facade (recommended for applications)

The [`Session`](../getting-started/first-node.md) facade hides the sequencing.
You give it a NAME, a preferred address, and a transport; it runs the claim for
you and reports the result. The shape is:

```rust
let (ctrl, mut driver) = Session::builder(my_name, 0x80)
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;

while !ctrl.is_claimed() {
    driver.poll()?;
}
println!("owned address: 0x{:02X}", ctrl.address());
```

`ctrl.start()?` kicks off the claim; pumping `driver.poll()?` advances the bus
and the claim state machine until the node is Claimed. Once `ctrl.is_claimed()`
returns true, `ctrl.address()` reflects the owned address and you may start
sending — application sends made before the claim confirms are rejected, so the
ordering is enforced for you. See [Getting started](../getting-started/first-node.md)
for the full builder.

### The low-level claimer (for tests and embedded control)

Underneath, `net::AddressClaimer` drives a single `net::InternalCf`. You own the
timing: you call `start`, feed in observed claims with `handle_claim`, and
advance time with `update`. The address-claim example shows a full contention
between two nodes that both prefer `0x80`:

```rust
{{#include ../../../examples/address_claim.rs:31:56}}
```

The loser, being self-configurable, shifts from `0x80` to `0x81`; the winner
keeps `0x80`. The claimer raises `on_address_claimed` and `on_address_lost`
events so your application can react to both outcomes.

## Events and responsibilities

Whichever API you use, your application is responsible for reacting to the
outcome:

| Event | Meaning | Typical action |
| --- | --- | --- |
| Claimed | The node owns a usable address. | Enable normal application traffic. |
| Lost | A lower NAME took the address. | Stop normal traffic; move or stop per policy. |
| Cannot claim | No address was available. | Stay silent except the allowed claim traffic. |
| Request for claim | A node asked everyone to re-announce. | Re-send the current claim if Claimed. |

The one rule you must never break: **do not transmit application traffic before
you are Claimed.** A node that talks during the waiting window, or from an
address it does not own, corrupts other nodes' view of the bus.

## Edge cases and failure modes

- **Preferred address already taken by a lower NAME.** A self-configurable node
  walks to the next free address; a fixed-address node fails to claim. Decide
  which behavior your product needs before shipping.
- **Two nodes with identical NAMEs.** This is a configuration bug, not a
  scenario the protocol can resolve. Arbitration cannot pick a winner between
  equal NAMEs. Ensure identity numbers differ.
- **Claiming too fast after power-up.** Other nodes need time to answer. The
  waiting window exists for a reason; do not shorten it below what the timeout
  constants allow.
- **Address violation.** A node that keeps using an address it lost will be seen
  contesting against the rightful owner. The correct response to losing is to
  stop, not to fight again with the same NAME.
- **No transport / no bus traffic.** If nothing is pumping the bus, the claim
  never completes and `ctrl.is_claimed()` never turns true. On a virtual bus you
  must drive both sides; see [Virtual bus](../getting-started/virtual-bus.md).

## Advanced

- **Several control functions in one ECU.** A physical ECU may host more than
  one control function — each one has its own NAME and claims its own address
  independently. Model each as its own CF; do not try to share an address.
- **Self-configurable address ranges.** A self-configurable node should retry
  within the address range appropriate for its function rather than walking the
  entire space. Choose a preferred address inside that range so the first
  attempt usually succeeds.
- **Commanded address.** A configuration tool can tell a node to move to a
  specific address. This is covered together with NAME management in
  [NAME management](name-management.md).
- **Fine control vs the bare codecs.** The session facade is right for
  applications: it sequences the claim, retries, and event fan-out for you. The
  `AddressClaimer` / `InternalCf` pair is right for unit tests and tightly
  controlled embedded loops where you own every millisecond of timing.

## Validate locally

```sh
make run EXAMPLE=address_claim
make test
```

The example runs a two-node contention entirely in software and asserts that the
lower NAME keeps `0x80` while the higher NAME self-configures to `0x81`. To run
the same claim against a real or virtual `vcan` interface, build a `Session` over
a SocketCAN transport and pump `driver.poll()?` until `ctrl.is_claimed()` is
true.

## What this proves / does not prove

Proves: the NAME comparison, the contention window, and the self-configure path
behave deterministically in software, and the machbus API drives them correctly.

Does not prove: real-hardware timing, interoperability with a specific
third-party ECU, or any conformance/certification claim. Those still require
official standards, real hardware, and interoperability evidence.

## See also

- [NAME and address claim](../standards/iso11783-network-management.md) — the
  conceptual primer.
- [NAME management](name-management.md) — commanded address and NAME-level
  negotiation.
- [PGN request](request-pgn.md) — the next building block after claiming.
- [Address conflicts](../troubleshooting/address-conflicts.md) — when claims go
  wrong.
