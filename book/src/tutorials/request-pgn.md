# PGN request

Most traffic on an ISOBUS or J1939 network is sent on a schedule, but sometimes
a node needs a value *now* — the current address of a peer, a software version,
a time-and-date stamp. The **PGN Request** is the pull mechanism for exactly
that: one control function asks another (or asks everyone) to transmit a named
parameter group. This tutorial covers the plain Request, its richer
**Request2 / Transfer** sibling, and the **Acknowledgement** message that a node
sends when it answers a request with a yes, a no, or a refusal instead of data.

If you have read [address claim](address-claim.md), you have already met the
Request indirectly: "request for address-claimed" is how a newcomer asks every
node to re-announce who it is. The same envelope carries every other pull on the
bus.

## Why this exists

A scheduled broadcast is fine for data that changes often, but it wastes bus
bandwidth for data that rarely changes or that only one node ever cares about.
The Request gives the network a request/response shape on top of an otherwise
broadcast medium:

- A node that just powered on can ask for state it missed instead of waiting for
  the next periodic broadcast.
- A diagnostic tool can poll a specific ECU for a specific parameter group.
- A configuration step can confirm that a peer is present and answering.

Because a request names a PGN rather than a node's internal API, any node that
owns that PGN can answer in a uniform way, and the requester does not need to
know how the responder is built.

## Mental model

```
requester                              responder(s)
    │                                        │
    │  Request(PGN = X)  ──► destination     │
    │     (specific DA, or global 0xFF)      │
    │                                        │
    │                              do I own PGN X?
    │                          ┌─────────────┴─────────────┐
    │                       yes│                           │no
    │                          ▼                           ▼
    │                   send PGN X data            (specific request)
    │ ◄────────────────  on PGN X                  NACK on PGN 0xE800
    │                          or                          │
    │                   Ack(positive/                      │
    │ ◄───────────────  denied/cannot)  ◄──────────────────┘
```

A request is just a tiny message whose payload *is* the PGN being asked for. The
response is whatever that PGN normally looks like — or, when there is no data to
give, an Acknowledgement that explains why.

## Anatomy: the Request message

The plain Request rides on `PGN_REQUEST` (`0xEA00`) and carries a three-byte
payload: the 18-bit PGN being requested, little-endian. `machbus` keeps this as
a thin codec in `j1939::pgn_request`:

| Symbol | Role |
| --- | --- |
| `encode_request(pgn)` | Build the 3-byte payload, rejecting any PGN outside the 18-bit range. |
| `decode_request(data)` | Parse a payload back to a `Pgn`, or `None` if it is malformed. |
| `requested_pgn(msg)` | Pull the requested PGN straight out of a received `Message`. |

The decoder is deliberately strict but tolerant of one real-world variation. The
canonical payload is exactly three bytes, but some stacks pad the same content
into a full eight-byte classic-CAN frame with `0xFF` filler. `decode_request`
accepts that padded shape and rejects anything else — a four-byte payload, or
eight bytes whose tail is not all `0xFF`, returns `None` rather than guessing.

The destination of the request decides its scope. A request sent to a specific
source address is **destination-specific**; a request sent to the global address
`0xFF` (`BROADCAST_ADDRESS`) asks every node that owns the PGN to answer.

## Anatomy: Request2 and Transfer

Request2 (`PGN_REQUEST2`, `0xC900`) is an extended form defined in ISO 11783-3.
It does everything the plain Request does and adds two things: up to three bytes
of **extended identifier** that let the requester narrow what it wants, and a
**use-transfer flag** that asks the responder to reply through the Transfer PGN
rather than on the requested PGN directly. `machbus` models the message as
`j1939::Request2Msg`:

| Field | Meaning |
| --- | --- |
| `requested_pgn` | The PGN being asked for, same as the plain Request. |
| `extended_id` | Up to three optional bytes that qualify the request. |
| `use_transfer` | When set, the answer comes back wrapped in a Transfer message. |

`Request2Msg::encode` produces a fixed eight-byte payload; `Request2Msg::decode`
reverses it and rejects reserved control bits, stray padding, or an out-of-range
PGN. When the transfer flag is set, the reply is carried by a `TransferMsg` on
`PGN_TRANSFER` (`0xCA00`): the original PGN as a three-byte prefix, followed by
the actual response bytes. This lets a requester correlate a Transfer reply with
the request that triggered it even when several are in flight.

## Anatomy: the Acknowledgement

Sometimes the honest answer to a request is not data. The Acknowledgement
message (`PGN_ACKNOWLEDGMENT`, `0xE800`) is how a node says *something other than
here-is-your-data*. `machbus` models it as `j1939::Acknowledgment`, with the
outcome carried in the `AckControl` control byte:

| `AckControl` | Wire value | What it tells the requester |
| --- | --- | --- |
| `PositiveAck` | 0 | The request was accepted (used where a request needs confirming rather than answering with data). |
| `NegativeAck` | 1 | The request was understood but the node will not or cannot supply that PGN. |
| `AccessDenied` | 2 | The node owns the PGN but the requester is not permitted to have it right now. |
| `CannotRespond` | 3 | The node owns the PGN but is temporarily unable to produce it (busy, not yet initialised). |

An `Acknowledgment` also records the `acknowledged_pgn` it refers to and the
`address` it was acknowledged for, so the requester can match the response to the
request it sent. The constructors `Acknowledgment::ack(pgn, addr)` and
`Acknowledgment::nack(pgn, addr)` cover the two common cases; `encode` and
`decode` (or `from_message`) handle the eight-byte wire format, validating the
control byte and the reserved padding on the way in.

## The request to response lifecycle

A single request resolves in one of a few ways. None of these are timed states
inside `machbus`; they are *behaviours* the responder chooses and the requester
must be ready to observe.

1. **Data answer.** The responder owns the PGN and has a current value. It
   transmits that PGN — directly, or through Transfer if the requester asked for
   it. This is the normal, happy path and there is no separate acknowledgement.
2. **Positive acknowledgement.** For requests that are commands rather than
   data pulls, the responder confirms acceptance with `PositiveAck`.
3. **Negative acknowledgement.** The responder understood the request but will
   not supply that PGN — it does not implement it, or policy forbids it. It
   sends a `NegativeAck` on `0xE800`. A NACK is the correct answer to a
   *destination-specific* request for an unsupported PGN; it tells the requester
   to stop waiting.
4. **Access denied.** The responder has the PGN but the requester may not read
   it in the current state. This is a refusal, not an absence — the data exists.
5. **Cannot respond.** The responder has the PGN but cannot produce it *yet*.
   The requester may reasonably try again later, unlike a NACK or Access-Denied,
   which are answers in their own right.

A key asymmetry: a node that cannot satisfy a **global** request usually stays
silent rather than flooding the bus with NACKs, while a node that cannot satisfy
a **destination-specific** request is expected to answer so the single requester
is not left waiting. Build your responder policy around that distinction.

## Doing it with machbus

There is no dedicated `examples/` binary for the bare Request codec, so the
snippets below show the real API shape rather than a compiled include. Each call
is grounded in the types above.

### Sending and parsing a plain Request

```rust
use machbus::j1939::{encode_request, requested_pgn};
use machbus::net::pgn_defs::{PGN_REQUEST, PGN_ADDRESS_CLAIMED};

// Ask for address-claimed data. Send to a specific DA, or 0xFF for everyone.
let payload = encode_request(PGN_ADDRESS_CLAIMED)?;
// ... transmit `payload` on PGN_REQUEST after you have claimed an address ...

// On the receiving side, recover the requested PGN from the inbound message:
if msg.pgn == PGN_REQUEST {
    if let Some(pgn) = requested_pgn(&msg) {
        // decide whether you own `pgn` and how to answer
    }
}
```

### Answering Request2 with the responder registry

`j1939::Request2Responder` is a small registry that turns an incoming Request2
into reply metadata without owning a bus. You register a deterministic payload
per PGN, then hand it each inbound message:

```rust
use machbus::j1939::Request2Responder;

let responder = Request2Responder::new()
    .with_response(PGN_TIME_DATE, time_date_bytes)?;

if let Some(reply) = responder.handle_message(&msg) {
    // reply.pgn is PGN_TIME_DATE for a direct answer,
    // or PGN_TRANSFER when the request set use_transfer.
    // reply.destination is the original requester's address.
    net.send(reply.pgn, &reply.data, self_cf, reply.destination, Priority::Default)?;
}
```

The registry refuses to answer a request whose source is the null or broadcast
address, and returns `None` for an unregistered PGN — leaving you free to
synthesise a NACK instead.

### Building an Acknowledgement

```rust
use machbus::j1939::Acknowledgment;
use machbus::net::pgn_defs::PGN_ACKNOWLEDGMENT;

// We do not implement the requested PGN: answer a destination-specific
// request with a NACK that names the PGN and our address.
let nack = Acknowledgment::nack(requested, my_address);
net.send(PGN_ACKNOWLEDGMENT, &nack.encode()?, self_cf, requester, Priority::Default)?;
```

### The session facade

For applications, the [`Session`](../getting-started/first-node.md) facade wires
a Request2 responder into its pump so you never touch the inbound queue by hand.
Plug the `Request2` plugin at build time, and the session will listen on
`0xC900`, filter out frames not addressed to it, pick direct-vs-Transfer
automatically, and send the reply for you:

```rust
let (ctrl, mut driver) = Session::builder(my_name, 0x80)
    .plug(Request2::new(Request2Responder::new().with_response(pgn, data)?))
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;
```

You can inspect or adjust the live registry through
`ctrl.with_mut::<Request2, _>(|r| ...)`, which returns `None` when the plugin is
not installed.

## Events and responsibilities

| Event | Meaning | Your responsibility |
| --- | --- | --- |
| Inbound Request for a PGN you own | A peer wants that data. | Send the PGN, or an Ack explaining why not. |
| Inbound Request for a PGN you do not own | Not your concern, usually. | NACK only a destination-specific request; stay silent on a global one. |
| Inbound Request2 with `use_transfer` | Peer wants the answer wrapped. | Reply on `PGN_TRANSFER` with the original PGN prefixed (the responder does this for you). |
| Inbound Acknowledgement | A peer answered your request with a status. | Match it to the request and stop waiting; retry only on `CannotRespond`. |

The one rule that mirrors address claim: **do not send requests before you are
Claimed.** A request from an unclaimed or null source address is malformed from
the responder's point of view, and a well-behaved responder ignores it.

## Edge cases and failure modes

- **No responder.** A global request for a PGN nobody owns produces no answer at
  all. Treat silence as a valid outcome — wait a bounded time, then move on. Do
  not retry in a tight loop and create a request storm.
- **Global versus specific destination.** This is the single most important
  policy decision. A specific request deserves a definite answer (data or a
  NACK); a global request should only be answered by nodes that actually own the
  PGN, and unanswered otherwise.
- **Repeated requests.** Re-issuing the same request before the first answer
  arrives multiplies bus load for no benefit. Send once, wait, and only re-ask if
  the response window genuinely closed empty.
- **Request for a PGN you do not support.** Answer a destination-specific request
  with `Acknowledgment::nack`; never fabricate empty data on the requested PGN,
  which the requester would read as a real (wrong) value.
- **Malformed payloads.** `decode_request` and `Request2Msg::decode` return
  `None` for short, padded-wrong, or out-of-range payloads. A responder that
  decodes to `None` should answer nothing — an invalid request is not a request.
- **Mistaking a response for a request.** The responder must never treat an
  inbound Acknowledgement or a data PGN as a new request to answer. The session
  responder only reacts to `PGN_REQUEST2`, which avoids this entirely.

## Advanced

- **Request scheduling and response windows.** ISO 11783-3 frames requests as
  something to issue sparingly, with the requester giving peers reasonable time
  to answer before concluding nothing is coming. `machbus` does not impose a
  timer on the bare codec; you own the wait window. Pick one long enough to
  cover a multi-packet response (see below) and short enough that your
  application does not stall.
- **Multi-packet responses.** When the requested PGN carries more than eight
  bytes, the answer cannot fit in a single classic frame and travels over the
  transport protocol. The requester's response window must allow for the extra
  round-trips of a segmented transfer. See
  [transport protocol](../standards/iso11783-datalink-transport.md).
- **PDU1 versus PDU2 scope.** Whether a response may go to a specific or global
  destination depends on the format of the requested PGN and its data length.
  In practice the session handles addressing for you; the rule to remember is
  that a destination-specific request should yield a destination-specific answer.
- **Fine control versus the bare codecs.** The codecs in `j1939::pgn_request`,
  `j1939::request2`, and `j1939::acknowledgment` are pure encode/decode with no
  bus coupling — ideal for tests and embedded loops where you own every send.
  The `Request2` plugin is the right choice for applications: it installs the
  inbound callback, filters by destination, and sends replies on your behalf.

## Validate locally

```sh
make test
make check
```

The codec round-trips, the responder registry, and the Acknowledgement
encode/decode paths are covered by unit tests beside each module
(`src/j1939/pgn_request.rs`, `src/j1939/request2.rs`,
`src/j1939/acknowledgment.rs`) and by the property tests in `src/j1939/mod.rs`.
To exercise a request and response across a real virtual bus, drive two sessions
over a shared link and watch the responder answer; the transport demo shows the
two-node pump shape:

```sh
make run EXAMPLE=transport_demo
```

## What this proves / does not prove

Proves: the Request, Request2, Transfer, and Acknowledgement codecs round-trip
correctly, reject malformed and out-of-range input, and that the responder
registry answers registered PGNs while ignoring unknown ones, invalid sources,
and non-request traffic.

Does not prove: real-hardware timing, the response behaviour of a specific
third-party ECU, or any conformance or certification claim. `machbus` is not
certified; a real deployment still needs official standards, real hardware, and
interoperability evidence.

## See also

- [Address claim](address-claim.md) — claim an address before you ever send a
  request; "request for address-claimed" uses this same envelope.
- [Transport protocol](../standards/iso11783-datalink-transport.md) — how a response larger than eight
  bytes is segmented and reassembled.
- [Diagnostics](diagnostics.md) — diagnostic parameter groups are a common
  target of destination-specific requests.
