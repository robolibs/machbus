# 4. Requests and acknowledgements

> **No dedicated example for this chapter.** This page builds directly on the
> chapter-3 program (two sessions on a simulated bus, one broadcasting a PGN the
> other receives). There is no separate `examples/` binary for a bare PGN
> request, so the request/answer snippets below are **illustrative shape**
> grounded in the real `machbus` API, not compiled includes. Everything still
> validates with `make test`, and the codecs they use are unit-tested in the
> crate.

In [chapter 3](sending-receiving.md) a node *pushed* data: one node broadcast a
PGN and the other picked it up whenever it happened to arrive. That is the right
shape for data that streams on a schedule. But sometimes you need a value *now* —
the current address of a peer, a version string, a one-off status — and you do
not want to wait for the next broadcast that may be seconds away.

This chapter adds the **pull** side of the bus: you will ask another node for a
specific PGN, and handle whatever comes back — including the polite "I cannot
give you that" answer.

## Pull versus wait-for-broadcast

So far every value reached you because someone decided to send it. A **request**
flips that around: *you* name a PGN and ask a node (or everyone) to transmit it.

```
  PULL (this chapter)                 PUSH (chapter 3)
  requester                           sender
     │  Request(PGN = X) ─►              │  PGN X data ─► (on a schedule)
     │                                   │
     │ ◄─ PGN X data  (the answer)       ▼
     │       or                       receiver picks it up
     │ ◄─ Ack (cannot/denied/…)        whenever it arrives
```

A request is a tiny message whose payload **is** the PGN you want. The answer is
either that PGN's normal data, or — when there is nothing to give — an
**Acknowledgement** that explains why.

## Step 1 — start from the chapter-3 program

Keep the exact two-node setup you already have: a simulated `bus0` with seats
`a` and `b`, a session for each, the claim handshake driven by the poll-and-pump
loop, and both nodes claimed. We will call them `node_a` (the requester) and
`node_b` (the responder). The full skeleton, unchanged from chapter 3 — a
`(controls, driver)` pair per node, started, then driven by the poll-and-pump
loop:

```rust
// build NAMEs, build the two sessions, then:
ctrl_a.start()?;
ctrl_b.start()?;
let mut now = Instant::ZERO;
for _ in 0..30 {
    now = now.add_millis(50);
    while drv_a.poll_at(now)?.is_some() {}
    while drv_b.poll_at(now)?.is_some() {}
    built.pump_all().unwrap();
    if ctrl_a.is_claimed() && ctrl_b.is_claimed() {
        break;
    }
}
// both are now Claimed — only now may we send a request
```

**Do not send a request before you are `Claimed`.** A request from a node with no
address is malformed, and a well-behaved responder ignores it. This is the same
rule you met with address claim.

## Step 2 — build a request for a PGN

A request rides on `PGN_REQUEST` (`0xEA00`) and carries a three-byte payload: the
PGN you want, little-endian. The codec lives in `j1939::pgn_request`:

```rust
use machbus::j1939::encode_request;
use machbus::net::pgn_defs::PGN_ADDRESS_CLAIMED;

// Ask for "address-claimed" data — a safe PGN every node answers.
let payload = encode_request(PGN_ADDRESS_CLAIMED).unwrap();
```

`encode_request` returns a `[u8; 3]` and rejects any PGN outside the 18-bit
J1939/ISOBUS range with an error, so you cannot accidentally ask for a malformed
PGN.

## Step 3 — send it like any frame

A request is just a frame. You send it exactly the way you broadcast in chapter
3, but you point it at a destination. Send to `node_b`'s address for a
**destination-specific** request, or to `BROADCAST_ADDRESS` (`0xFF`) to ask
*everyone* who owns the PGN:

```rust
use machbus::net::{BROADCAST_ADDRESS, Priority};
use machbus::net::pgn_defs::PGN_REQUEST;

ctrl_a.send_raw(
    PGN_REQUEST,
    &payload,
    ctrl_b.address(),   // destination: a specific peer (or BROADCAST_ADDRESS)
    Priority::Default,
)?;
```

The source address is filled in for you from the address `node_a` claimed — the
same `send_raw` you used to broadcast in chapter 3, just pointed at a destination
and carrying a request payload.

Then run the **same poll-and-pump loop** so the frame actually crosses the bus
and any answer comes back:

```rust
for _ in 0..20 {
    now = now.add_millis(10);
    while drv_a.poll_at(now)?.is_some() {}
    while drv_b.poll_at(now)?.is_some() {}
    built.pump_all().unwrap();
}
```

That is the whole send path. Nothing here is new machinery — it is the chapter-3
pump loop carrying one more frame.

## Step 4 — receive the answer (data, or an Ack)

On the requester, drain events the way you already do. A request resolves in one
of two broad ways:

**You get the data.** If `node_b` owns the PGN, it transmits it on that PGN. Any
inbound PGN that no subsystem plugin claimed surfaces as an `Event::Custom` when
you poll `node_a`'s driver — exactly like the broadcast you received in chapter 3.
No subscription step is needed:

```rust
// ... after the pump loop ...
while let Some(ev) = drv_a.poll_at(now)? {
    if let Event::Custom { pgn, source, data } = ev {
        println!("answer: pgn=0x{pgn:04X} from 0x{source:02X} = {data:02X?}");
    }
}
```

**You get an Acknowledgement instead.** When a node cannot answer with data, it
may reply on `PGN_ACKNOWLEDGMENT` (`0xE800`). That is the next step.

### Expected output

For a request that the peer can answer, you see the requested PGN come back as a
`Custom` event from `node_b`'s address — the same event shape as a chapter-3
broadcast, except this time *you* triggered it:

```text
answer: pgn=0xEE00 from 0x81 = [..]
```

### What just happened

You named a PGN, sent it to a destination, and the responder transmitted that
PGN back. The request did not say "do something" — it said "send me PGN X." The
answer is ordinary bus traffic; the only special thing was that you asked for it.

## Step 5 — handle the four acknowledgement outcomes

When the honest answer is *not* data, the responder sends an `Acknowledgment`
(`j1939::Acknowledgment`) on `0xE800`. Its `control` byte (`AckControl`) tells you
which of four things happened. On the requester, decode it and branch:

```rust
use machbus::j1939::{AckControl, Acknowledgment};
use machbus::net::pgn_defs::PGN_ACKNOWLEDGMENT;

if msg.pgn == PGN_ACKNOWLEDGMENT {
    if let Some(ack) = Acknowledgment::from_message(&msg) {
        match ack.control {
            AckControl::PositiveAck   => { /* accepted — proceed */ }
            AckControl::NegativeAck   => { /* not supported — stop waiting */ }
            AckControl::AccessDenied  => { /* exists, but not for us right now */ }
            AckControl::CannotRespond => { /* exists, not ready yet — retry later */ }
        }
        // ack.acknowledged_pgn and ack.address let you match it to your request
    }
}
```

What a client should do for each:

| `AckControl` | Meaning | What the requester does |
| --- | --- | --- |
| `PositiveAck` | The request was accepted (used where a request needs confirming, not answering with data). | Treat as success; carry on. |
| `NegativeAck` | Understood, but the node will not / does not supply that PGN. | Give up on this PGN. **Do not** retry — the answer is "no." |
| `AccessDenied` | The node owns the PGN but you may not read it in the current state. | A refusal, not an absence. Do not retry blindly; the data exists but is gated. |
| `CannotRespond` | The node owns the PGN but cannot produce it *yet* (busy, not initialised). | The only outcome where retrying later is reasonable. |

The two fields `acknowledged_pgn` and `address` let you confirm the Ack refers to
the request you actually sent, which matters when several requests are in flight.

### Building an Ack (the responder side)

If you are writing the responder and you do **not** implement a requested PGN,
answer a destination-specific request with a NACK rather than silence:

```rust
let nack = Acknowledgment::nack(requested_pgn, ctrl_b.address());
// send nack.encode().unwrap() on PGN_ACKNOWLEDGMENT, back to the requester
```

`Acknowledgment::ack(pgn, addr)` and `Acknowledgment::nack(pgn, addr)` cover the
two common cases; `encode` produces the eight-byte wire format.

## Global versus specific requests

This is the one policy decision that matters most:

- **Specific (destination = a peer's address):** that peer should give a definite
  answer — the data, or a NACK so you are not left waiting forever.
- **Global (destination = `BROADCAST_ADDRESS`):** only nodes that actually own the
  PGN answer; everyone else stays silent. An unanswered global request is
  **normal**, not an error.

So a request to one node that never answers is suspicious; a global request that
some nodes ignore is expected.

## Gotchas

- **A request is not a command.** It says "send me PGN X," never "do X." If you
  need a node to *act*, that is a different message; a request only pulls data.
- **An unanswered global request is fine.** Treat silence on a global request as a
  valid outcome. Wait a bounded time, then move on.
- **Do not block waiting.** Never spin tightly re-sending the same request while
  you wait. Send once, keep ticking-and-pumping, and only re-ask if the response
  window genuinely closed empty — and even then, only for `CannotRespond`.
- **Malformed requests get no answer.** `decode_request` returns `None` for short,
  wrongly-padded, or out-of-range payloads. An invalid request is not a request —
  a good responder answers nothing.

## Validate locally

```sh
make test
```

The Request and Acknowledgement codecs (`encode_request` / `decode_request` /
`requested_pgn`, and `Acknowledgment` encode/decode with every `AckControl`
value) are covered by unit tests beside their modules and by property tests, so
`make test` exercises the exact calls used above.

## What this proves / does not prove

Proves: you can build a PGN request, send it to a specific node or to everyone,
and handle both a data answer and each of the four acknowledgement outcomes,
using the real `machbus` codecs on the same simulated-bus loop from chapter 3.

Does not prove: how a specific third-party ECU times or answers a request on real
hardware, or any conformance or certification claim. `machbus` is not certified;
a real deployment still needs official standards, real hardware, and
interoperability evidence.

## Next

→ [5. Moving big data with transport](transport.md) — when the answer to a
request is larger than eight bytes, it cannot ride in one frame. The next chapter
shows how the response is segmented and reassembled.

## See also

- [PGN request](../tutorials/request-pgn.md) — the reference page: full anatomy of
  Request, Request2 / Transfer, the responder registry, and the session facade.
- [PGN requests and acknowledgements](../standards/foundations.md) —
  the basics, why the pull mechanism exists at all.
- [Talking diagnostics](diagnostics.md) — diagnostic parameter groups are a common
  target of destination-specific requests.
