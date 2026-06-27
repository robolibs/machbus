# NAME management and commanded address

Once a control function owns an address, the network is not done with it. A
configuration tool may need to move that node to a *specific* address, or change
some of its NAME fields so two otherwise-identical units can be told apart. These
are the network-management operations that sit on top of the basic claim. This
tutorial explains the **commanded-address** message, the **NAME-management**
negotiation, and how a correct node behaves when its address is commanded,
contested, or violated.

This page is the sibling of [Address claim](address-claim.md). That page owns the
basic claim — power-up, the contention window, NAME arbitration, and the
self-configure walk. Read it first. This page assumes the node has already
claimed an address and covers what happens *after*.

## Why this exists

The basic claim answers one question: "which address does each NAME get when
everyone powers up at once?" It does not answer the operational questions a real
machine raises later:

- A service tool wants every ECU at a known, documented address so a wiring
  diagram or a diagnostic script can find it. The claim alone gives addresses
  that can drift between power cycles.
- Two physically identical implements hang off the same bus. Their NAMEs collide
  on every field except the per-unit identity number. The integrator needs to set
  an instance field on one of them so the rest of the network can address each
  independently.
- A generic ECU ships with a placeholder function and is configured into its real
  role at install time.

Commanded address and NAME management are the standardized, in-band ways to do
all of this without reflashing firmware. Both are **optional** capabilities in
ISO 11783-5: a node may support neither, one, or both, and may guard them behind
a source check or a proprietary security step. `machbus` gives you the protocol
machinery to *respond* correctly when you choose to support them.

## Mental model

Think of three actors and two messages.

```
 commanding CF (tool/bridge)            target CF (your node)
 ─────────────────────────             ─────────────────────
        │  commanded-address (NAME + new SA)  │
        │ ──────────────────────────────────►│  is the NAME mine?
        │                                     │  yes ─► re-claim at new SA
        │  ◄───────── address-claimed ────────│
        │                                     │
        │  name-mgmt: set pending (fields)    │
        │ ──────────────────────────────────►│  identity unchanged? store it
        │  ◄────────── ACK / NACK ────────────│
        │  name-mgmt: adopt pending           │
        │ ──────────────────────────────────►│  swap NAME ─► re-claim
        │  ◄───────── address-claimed ────────│
```

Two ideas carry the whole topic. First, **address and identity are separable**.
Commanded address changes only the label; the NAME is untouched. NAME management
changes the identity; the address is then re-negotiated from scratch. Second,
**every accepted change ends in a fresh address claim**. Whenever a node moves
address or adopts a new NAME, it must re-run the claim and win an address before
it resumes normal traffic — exactly the handshake from
[Address claim](address-claim.md).

## Anatomy: the two messages

`machbus` carries these on two distinct PGNs, exposed as
`net::pgn_defs::PGN_COMMANDED_ADDRESS` and `net::pgn_defs::PGN_NAME_MANAGEMENT`.

### Commanded address

The commanded-address payload is nine bytes: the eight-byte NAME of the node
being commanded, followed by one byte holding the address it should move to. The
NAME acts as the addressee — a commanding CF must already know *which* NAME it
wants to relocate, because the source address it sends to could have drifted
between the decision and the command.

`NameManager::handle_commanded_address(msg, our_name)` decodes this for you and
applies four guards before it accepts anything:

| Guard | Rejected when | Why |
| --- | --- | --- |
| PGN match | `msg.pgn` is not the commanded-address PGN | Wrong message entirely. |
| Source sanity | source is the null or broadcast address | A command must come from a real claimed node. |
| Length | payload is not exactly nine bytes | Malformed; not a valid command. |
| Target match | the carried NAME is not `our_name` | The command is for some other node. |
| Address range | the new address is above `MAX_ADDRESS` (`0xFD`) | `0xFE`/`0xFF` are reserved and never claimable. |

If all guards pass, the method returns `Some(new_address)` and fires
`on_commanded_address`. It does **not** move you on its own — applying the address
and re-claiming is the caller's job, because only the caller owns the CF state.

### NAME management

The NAME-management message is a fixed wire shape: a one-byte **mode**, eight
NAME bytes, a one-byte NACK reason, and padding, encoded to a canonical 17-byte
payload. `machbus` models it as `net::NameManagementMsg`:

```rust
pub struct NameManagementMsg {
    pub mode: NameMgmtMode,
    pub name_data: [u8; 8],
    pub nack_reason: NameNackReason,
}
```

`NameManagementMsg::encode` and `NameManagementMsg::decode` move between this
struct and the wire bytes. `decode` is strict: it rejects non-canonical lengths,
unknown modes, invalid NACK reason codes, and any padding byte that is not `0xFF`.
`NameManagementMsg::for_name(mode, name)` is the convenient constructor, and
`msg.name()` extracts the carried NAME.

## The NAME-management modes

The mode byte selects which of nine operations a frame represents. `machbus`
enumerates them as `net::NameMgmtMode`:

| Mode | Direction | Meaning |
| --- | --- | --- |
| `RequestCurrent` | tool → node | "Tell me the NAME you are using right now." |
| `RequestCurrentResponse` | node → tool | The current NAME, answering the above. |
| `SetPending` | tool → node | "Stage this NAME; do not adopt it yet." |
| `RequestPending` | tool → node | "Tell me the NAME you have staged." |
| `RequestPendingResponse` | node → tool | The staged NAME, answering the above. |
| `AdoptPending` | tool → node | "Make the staged NAME your current NAME and re-claim." |
| `Acknowledge` | node → tool | A set or adopt succeeded. |
| `NegativeAcknowledge` | node → tool | A request was refused; carries a reason. |
| `RequestAddressClaim` | tool → bus | "If your NAME matches, send your address claim." |

The shape of a configuration session is: query the current NAME, stage one or
more field changes with `SetPending`, optionally read back what was staged with
`RequestPending`, then trigger the swap with `AdoptPending`. The node keeps
running under its current NAME the whole time it has a pending one staged; nothing
changes on the bus until adoption.

### How machbus answers each mode

`NameManager::handle_name_management(msg, current_name)` is the responder. It
ignores frames from the null or broadcast source, decodes the payload, and
dispatches by mode. It always emits the `on_name_management` event for
observability, and returns the reply to send (if any):

| Incoming mode | machbus reply |
| --- | --- |
| `RequestCurrent` | `RequestCurrentResponse` carrying `current_name`. |
| `SetPending` | `Acknowledge` if accepted, else `NegativeAcknowledge`. |
| `RequestPending` | `RequestPendingResponse` if one is staged, else NACK `PendingNotSet`. |
| `AdoptPending` | `Acknowledge` if a pending NAME existed, else NACK `PendingNotSet`. |
| all response/ack modes, `RequestAddressClaim` | no reply — observed only. |

The last row is deliberate. Acknowledgements and responses are **observed but
never answered**, so two `machbus` nodes can never fall into a response-to-response
loop where each one replies to the other's reply forever. Those frames also never
mutate the pending NAME.

### The one rule that constrains a SetPending

`NameManager::set_pending(current_identity, new_name)` enforces the single
invariant the standard places on NAME changes: **the identity number must not
change.** Every other field — instances, function, device class, industry group,
even the self-configurable bit — can be re-commanded, but the per-unit identity
number is fixed at manufacture and stays put. If a `SetPending` carries a NAME
whose identity number differs from the node's current one, `machbus` rejects it
with `NegativeAcknowledge` and reason `InvalidItems`, and stages nothing.

### NACK reasons as behavior

When `machbus` refuses an operation it answers with `NegativeAcknowledge` and a
`net::NameNackReason`. Read each one as "what the node is telling the tool to do
differently":

| Reason | What it means in practice |
| --- | --- |
| `Security` | The node will not accept this change from this source. The tool must authenticate or come from an allowed CF (a bridge or service tool). |
| `InvalidItems` | One or more commanded fields are not allowed to change — in `machbus`, attempting to change the identity number. |
| `Conflict` | The node cannot take on what the change implies (it cannot perform the requested function, or cannot be self-configurable as asked). |
| `Checksum` | The integrity check that guards against an addressee mismatch did not match. |
| `PendingNotSet` | A `RequestPending` or `AdoptPending` arrived but nothing is staged. machbus returns this for both. |
| `Other` | A catch-all when none of the above fit. |

## Self-configurable address ranges

When a self-configurable node loses arbitration, it must pick another address to
try. ISO 11783-5 narrows the choice: a self-configurable node draws its retry
candidates from the dynamic range (the upper region of the address space), and
should prefer the lower part of that range before reaching into addresses that
double as preferred slots for other functions. A node with an assigned preferred
address may also fall back to that preferred address.

`machbus` walks candidates in `AddressClaimer`. After a loss it calls an internal
`find_next_address` that steps linearly from the contested address, skips the
node's own preferred address (it was just contested), and skips any address it has
already *seen claimed by someone else*. The claimer remembers occupied addresses
from every peer claim it observes, so a saturated network does not make it cycle
onto slots it already knows are taken. If the walk exhausts every claimable
address, the node transitions to the failed state and announces cannot-claim from
the null address — the same dead end the claim page describes, reached through a
fuller search.

Two practical consequences:

- Pick a preferred address *inside* the range appropriate for your function so the
  very first attempt usually succeeds and the walk never runs.
- Do not configure the null or broadcast address as a local preferred source
  address. machbus treats that as unclaimable at startup and takes the
  cannot-claim path instead of briefly advertising an unusable address.
- After a node settles on a new address, that address should become its initial
  address for the next power-up. Persisting it (see Advanced) is what keeps the
  network stable across restarts instead of re-shuffling every boot.

## Contention and address-violation handling

There are two distinct "someone wants my address" situations, and they are not the
same.

**Contention during a claim.** Another node sends an address-claimed for an
address you are claiming. This is the normal arbitration path: lower NAME wins,
the loser self-configures or fails. `AddressClaimer::handle_claim` owns this and
it is covered in [Address claim](address-claim.md).

**Address violation after a claim.** You are already the owner, online and
sending, and you observe *some other message* using your source address — not an
address claim, just ordinary traffic from a node that thinks the address is its.
A correct node does not stay quiet. It re-announces its claim to the whole bus so
the network re-converges on the true owner, and it raises the corresponding
diagnostic condition so a tool can see that two nodes are fighting over one
address. The wrong response is to keep transmitting as if nothing happened, which
leaves the bus with an ambiguous owner.

The closely related failure is a **duplicate NAME**: two distinct devices present
the *same* NAME on different source addresses. Arbitration has no tie-breaker for
equal NAMEs, so it cannot resolve this. `AddressClaimer::handle_duplicate_name`
treats it as fatal for the local node: it goes offline, transitions to the failed
state, drops to the null address, and emits a single cannot-claim announcement
rather than continuing under an identity the bus cannot distinguish. The cure is
upstream — make the identity numbers differ — not on the wire.

## Doing it with machbus

There are two layers, mirroring the claim page.

### The session facade (recommended)

The session facade owns the NAME-management responder when you plug the
`NameManagement` plugin. It registers a callback on the NAME-management PGN,
drains incoming frames each pump, and runs `NameManager::handle_name_management`
for you. When a reply is produced it sends it; when an `AdoptPending` is accepted
it applies the new NAME to your control function and *restarts address claiming
automatically* — you do not have to re-sequence the claim by hand. The shape is:

```rust
// Illustrative shape — reacting to a tool adopting a new NAME for this node.
ctrl.with_mut::<NameManagement, _>(|nm| {
    nm.manager_mut().on_name_changed.subscribe(|new_name| {
        // The session will re-claim under `new_name`; persist it if you want
        // it to survive the next power cycle.
    });
});
```

The plugin is a responder: it answers a commanding CF. Driving the *commanding*
side (sending `SetPending`/`AdoptPending` to other nodes) is an application
concern you build on top of the raw send API.

### The low-level manager (for tests and embedded control)

`net::NameManager` is the pure, stateless-by-design helper. You feed it decoded
`Message`s and route its replies yourself:

```rust
// Illustrative shape — not a compiled example.
let mut mgr = NameManager::new();

// A tool commands us to move to 0x42.
if let Some(new_addr) = mgr.handle_commanded_address(&msg, our_name) {
    cf.set_address(new_addr);
    // re-run the claim at new_addr (see the address-claim tutorial)
}

// A tool runs a NAME-management exchange.
if let Some(reply) = mgr.handle_name_management(&msg, current_name) {
    net.send(PGN_NAME_MANAGEMENT, &reply.msg.encode(),
             self_cf, reply.destination, Priority::Default)?;
}
```

The manager tracks only the pending NAME. `set_pending`, `adopt_pending`,
`has_pending`, and `pending_name` give you direct control for unit tests, and
`adopt_pending` returns the adopted NAME *and* fires `on_name_changed` so the
caller knows it now owes a re-claim.

## Events and responsibilities

| Event | Fired by | Your responsibility |
| --- | --- | --- |
| `on_commanded_address` | `handle_commanded_address` | Apply the new address and re-claim. The session does this for you. |
| `on_name_changed` | `adopt_pending` | The current NAME is now the adopted one; re-claim under it and persist it. |
| `on_name_management` | every received NM frame | Observe/log. Do not reply from here — the manager already decides replies. |

The rule that never bends, same as the claim page: **after any commanded move or
NAME adoption, do not resume normal traffic until you have successfully claimed an
address again.** The address you were commanded to, or the identity you adopted,
is not yours until the claim confirms it.

## Edge cases and failures

- **Commanded to an occupied address.** The command is accepted and you re-claim
  at the new address — but the claim can still *lose* if a lower NAME already owns
  it. The correct end state is whatever the claim yields, not blind occupation.
  A node that cannot take the commanded address re-announces its current address
  instead.
- **Commanded to a reserved address.** `handle_commanded_address` returns `None`
  for any address above `MAX_ADDRESS`, so `0xFE`/`0xFF` are silently ignored.
- **NACKed name change.** A `SetPending` that touches the identity number, or that
  the node will not accept from this source, comes back as a `NegativeAcknowledge`.
  The pending state is left untouched; nothing was staged.
- **Adopt with nothing staged.** `AdoptPending` when no pending NAME exists NACKs
  with `PendingNotSet` and changes nothing. `adopt_pending` consumes the pending
  NAME, so a second adopt also fails — adoption is one-shot.
- **Response/violation storms.** Because `machbus` never replies to ack or
  response modes, a bus full of NM responses cannot drag your node into an
  ever-escalating reply loop. Likewise a real violation triggers exactly one
  re-announce per detection, not a continuous stream.
- **Stale source address.** A node can claim a new address between a tool deciding
  to command it and the command arriving. Commanded address sidesteps this by
  addressing the NAME, not the source; NAME management guards the same race with
  an integrity check that surfaces as the `Checksum` NACK reason.

## Advanced

- **Multi-CF ECUs.** One physical ECU may host several control functions, each
  with its own NAME and its own address. Commanded address and NAME management are
  per-CF: model each control function separately and run a `NameManager` per CF.
  Never try to share one address between two of them.
- **Configuration tools and security.** Supporting these messages is optional, and
  the standard explicitly lets a manufacturer accept them only from a bridge or
  service tool, or behind a proprietary security check. If your product is a
  responder, decide *who* you accept commands from before shipping, and answer
  unauthorized requests with the `Security` NACK reason rather than silently
  obeying.
- **Persistence across power cycles.** When a node moves address — by losing
  arbitration, by self-configuring, or by command — that new address is meant to
  become the initial address it tries next boot. `machbus` does not own your
  non-volatile storage; persist the address (and an adopted NAME) yourself from
  the `on_address_claimed` and `on_name_changed` events so the network comes back
  up in the same shape it settled into.
- **Fine control vs the bare codecs.** The `NameManagement` plugin is right for
  applications: it pumps, replies, applies adopted NAMEs, and re-claims for you.
  The bare `NameManager` is right for tests and tightly controlled embedded loops
  where you route every frame and own every state transition.

## Validate locally

```sh
make test
```

The library tests exercise the responder end to end: request-current,
set-pending with matching and mismatched identity, request-pending when set and
unset, adoption and double-adoption, malformed and overlong payloads, response
modes that must not loop, and commanded-address targeting us, targeting someone
else, with a reserved address, and with short or overlong payloads. For the basic
claim and self-configure walk that these operations re-trigger, run the
address-claim example:

```sh
make run EXAMPLE=address_claim
```

## What this proves / does not prove

Proves: in software, `machbus` decodes and answers NAME-management and
commanded-address traffic by the rules above, refuses identity changes and
unstaged adoptions, never loops on responses, and re-claims after any accepted
move or NAME change.

Does not prove: real-hardware timing, interoperability with a specific
third-party tool or ECU, or any conformance or certification claim. Supporting
these optional messages on a shipping product still needs official standards, real
hardware, and interoperability evidence. `machbus` ships no certification.

## See also

- [Address claim](address-claim.md) — the basic claim, contention window, and
  self-configure walk this page builds on.
- [Control functions and partners](../standards/iso11783-general-device-classes.md)
  — what a CF is and how multi-CF ECUs are modeled.
- [Network routing](network-routing.md) — how addressed traffic moves once every
  node owns an address.
- [Address conflicts](../troubleshooting/address-conflicts.md) — diagnosing claims
  and violations when they go wrong.
