# 5. Moving big data with transport

> **Anchor example:** `examples/transport_demo.rs` — run it any time with
> `cargo run --example transport_demo`.

In [chapter 3](sending-receiving.md) and [chapter 4](requests-and-acks.md) every
message fit in a single CAN frame. That is fine for a speed reading or a button
press, but it does not get you far: a CAN frame holds **eight bytes** of payload,
and the messages you actually care about — a Virtual Terminal object pool, a Task
Controller device description, a diagnostic fault list, a GNSS record — are
hundreds or thousands of bytes. Something has to chop the big payload into 8-byte
frames, carry them across the bus, and glue them back together on the far side.

That something is the **Transport Protocol**. By the end of this chapter you will
have sent payloads far too big for one frame and watched machbus split them,
ship them, and reassemble them byte for byte — first with plain TP, then with the
extended ETP for a very large payload, then with NMEA Fast Packet as the
NMEA-2000 equivalent.

This is the build-along version. The
[Transport Protocol tutorial](../standards/iso11783-datalink-transport.md) is the
reference companion: it has the full state machine, every timeout, and every
abort reason. We will point you there for the deep parts instead of repeating
them.

## What we are building

`transport_demo` is unusual for this track: instead of a session, it drives the
**bare transport engines** directly so every frame is visible. You create a
sender engine and a receiver engine, then pass frames between them by hand. That
makes the splitting and reassembly something you can watch happen, step by step.

```
big payload                                   reassembled payload
   │                                                   ▲
   ▼                                                   │
[ sender engine ] ──► 8-byte frames over the bus ──► [ receiver engine ]
```

We will do this three times — TP, ETP, Fast Packet — each as its own self-
contained block in `main`.

## Step 1 — a connection-mode TP round trip (CMDT)

Start with a 40-byte payload. That is five times bigger than one frame can hold,
so it cannot go out as a single message. We use **connection mode** (CMDT), the
flavour with a handshake: the sender asks permission, the receiver paces the
flow, and the receiver confirms when every byte has arrived.

Add this block — it is the whole TP section of the example:

```rust
{{#include ../../../examples/transport_demo.rs:17:56}}
```

Run it:

```sh
cargo run --example transport_demo
```

Expected output (the `[TP CMDT]` section):

```text
[TP CMDT]
  RTS for 40 bytes (≈6 packets)
  CTS num=6 next_seq=1
  TX queued 6 DT frames
  delivered 40 bytes (equal? true)
```

### What just happened

Read the block as a four-message conversation between `tx` and `rx`:

```
tx (0x10)                              rx (0x20)
  │  RTS  "40 bytes, 6 packets" ───────►│   open a receive session,
  │                                     │   allocate the buffer
  │◄────── CTS "send me 6, start at 1"  │   receiver sets the pace
  │  DT seq 1..6  ─────────────────────►│   7 payload bytes per frame
  │◄────── EndOfMsgAck "got all 40"     │   confirm + close
  ▼                                     ▼
Complete                            Complete
```

Line by line in the code:

1. `tx.send(0xEF00, &payload, 0x10, 0x20, 0, Priority::Lowest)` validates the
   payload, opens a transmit session, and hands back the **RTS** frame ("Request
   To Send"). The `0x10`/`0x20` are the sender and receiver addresses.
2. `rx.process_frame(&rts, 0)` opens the matching receive session and returns a
   **CTS** ("Clear To Send") granting a window of packets. The `num=6` is how
   many DT frames it is ready for; `next_seq=1` is where to start.
3. `tx.process_frame(&cts, 0)` accepts the window; `tx.get_pending_data_frames()`
   drains it into the **DT** (data-transfer) frames — one sequence byte plus
   seven payload bytes each. Six frames carry the 40 bytes (`ceil(40 / 7) = 6`).
4. Feeding each DT frame back into `rx.process_frame` reassembles the bytes, and
   when the last one lands the receiver returns an **EndOfMsgAck**.
5. `tx.process_frame(&eoma)` confirms delivery, which fires the `on_complete`
   event we subscribed to. The completed `TransportSession` carries the
   reassembled `data`, and `got.data == payload` proves it came back intact.

The key idea: **the receiver is in charge of pace.** It never gets more frames
than the CTS it just issued asked for, so a slow receiver cannot be flooded.
That handshake-and-flow-control is exactly what makes CMDT the right choice for
the big, must-not-drop uploads (object pools, device descriptions) you will meet
in later chapters.

## Step 2 — the same idea, much bigger (ETP)

TP has a ceiling. Its on-wire counters cap a single transfer at **1785 bytes**.
When your payload is bigger than that, you reach for the **Extended Transport
Protocol (ETP)**, which widens the counters to carry payloads into the megabytes.
ETP is **connection-mode only** — there is no broadcast form.

Here we push **2.5 KiB** (2500 bytes), well past TP's limit, so ETP is required.
The handshake is the same shape, with one extra step folded in (a packet-offset
message that lets ETP's small sequence numbers address a huge payload — the
tutorial covers it). Because that means several CTS windows, the example pumps in
a loop until reassembly completes:

```rust
{{#include ../../../examples/transport_demo.rs:60:96}}
```

Expected output (the `[ETP]` section):

```text
[ETP]
  converged in 23 turns, delivered 2500 bytes (equal? true)
```

The exact turn count is not important — what matters is that the loop keeps
pumping frames back and forth until `received` is filled, then checks that 2500
bytes came back byte-for-byte equal. The pump loop here is the same heartbeat you
have used since chapter 1, just with the bus stood in by hand: **keep feeding
frames between the two engines until the transfer converges.** Stop pumping early
and the transfer never finishes.

### The size ladder

The three mechanisms form a ladder, and the choice is decided purely by how many
bytes you are sending:

| Payload size | Mechanism |
| --- | --- |
| up to 8 bytes | one ordinary CAN frame — no transport at all |
| 9 to 1785 bytes | TP (this chapter's step 1) |
| 1786 bytes and up | ETP (step 2) |

machbus enforces these bounds for you: `TransportProtocol::send` rejects a
payload that is 8 bytes or smaller ("use a single frame") and one that is too big
for TP ("use ETP"), and `ExtendedTransportProtocol::send` rejects anything small
enough for plain TP. You generally do not pick the protocol yourself at the
session layer — it looks at the size and routes to the right engine.

## Step 3 — Fast Packet, the NMEA side

ISOBUS rides on J1939, where TP/ETP is the transport. NMEA 2000 — the marine and
GNSS world — shares the same CAN physical layer but uses its own lighter
multi-frame scheme called **Fast Packet** for payloads up to a couple hundred
bytes. machbus speaks it too, and the API mirrors TP closely.

Here we send a 30-byte GNSS-style payload as Fast Packet:

```rust
{{#include ../../../examples/transport_demo.rs:100:119}}
```

Expected output (the `[Fast Packet]` section):

```text
[Fast Packet]
  5 frames for 30 bytes
  reassembled 30 bytes (equal? true)
```

### What happened

Fast Packet has no handshake at all. `tx.send(PGN_GNSS_POSITION, &payload, 0x10)`
returns the **complete set of frames** in one go — the first frame carries a
length and the leading bytes, each following frame carries a sequence counter and
more payload. The receiver glues them back together as they arrive and returns
the finished message once the last frame lands. Thirty bytes split into five
frames; `msg.data == payload` confirms the round trip.

Fast Packet is to NMEA what BAM (the broadcast form of TP) is to ISOBUS: a
fire-and-forget, no-flow-control way to push a moderately sized payload to
listeners. It trades the safety of a handshake for simplicity and speed.

## Broadcast vs connection-mode

You have now seen both postures, and the difference is the one thing to carry
forward:

- **Connection-mode (CMDT, ETP)** — there is exactly one receiver, it paces the
  flow with CTS windows, it can refuse the transfer, and it confirms completion.
  Reliable. Use it for uploads that must not drop a byte.
- **Broadcast (BAM, Fast Packet)** — the sender pushes to everyone with no
  acknowledgement and no flow control. A listener that misses a frame just fails
  to reassemble and drops the message; nobody retransmits. Use it when many nodes
  want the same data and occasional loss is acceptable.

machbus handles the framing, sequencing, and reassembly in both cases. Your job
is to hand it a payload and pump.

## Things that trip people up

- **Stopping the pump too early.** Reassembly only completes when you keep
  feeding frames between the engines. In the ETP step that is the whole point of
  the loop — break out before `received` is filled and you get nothing. With a
  real session this is the usual poll-and-pump loop; keep it running.
- **Pumping only one side.** A transfer needs frames flowing **both** ways
  (RTS→CTS→DT→ack). On a virtual bus, if you only pump the sender, the receiver
  never gets a chance to issue its CTS and the transfer stalls until it times
  out.
- **Expecting flow control on broadcast.** BAM and Fast Packet have no CTS and no
  ack. There is no back-pressure and no retry — by design.
- **Aborts and timeouts exist.** Out-of-order frames, duplicate sequences,
  exhausted buffers, and silent peers all end a session with an abort or a
  timeout. The full list of abort reasons and the timeout windows lives in the
  [Transport Protocol tutorial](../standards/iso11783-datalink-transport.md) — reach for
  it when a transfer misbehaves.

## Validate locally

```sh
cargo run --example transport_demo
make test
```

The example runs all three round trips and prints `equal? true` for each,
proving every payload was split and reassembled byte for byte.

## What this proves / does not prove

Proves: machbus can split a payload too large for one CAN frame, carry it as a
stream of 8-byte frames, and reassemble it intact — over plain TP, over ETP for
very large payloads, and over NMEA Fast Packet — and that it picks the right
mechanism by payload size.

Does not prove: anything about real-hardware timing, interoperability with a
specific third-party ECU, or any conformance or certification claim. machbus is
not certified; real deployment still needs official standards, hardware, and
interoperability evidence.

## Next

→ [6. Talking diagnostics](diagnostics.md) — fault codes and the kind of fault
lists that ride on top of the transport you just built.

## See also

- [Transport Protocol tutorial](../standards/iso11783-datalink-transport.md) — the full
  reference: state machine, timeouts, every abort reason.
- [Fast Packet tutorial](../tutorials/fast-packet.md) — the NMEA multi-frame
  mechanism in depth.
