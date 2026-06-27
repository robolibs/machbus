# Fast Packet

NMEA 2000 Fast Packet is the multi-frame transport that NMEA 2000 uses to send a
payload that is too big for one CAN frame but still modest in size — things like
a GNSS position fix, a wind report, or an engine parameter group. It is *not*
the ISOBUS/J1939 Transport Protocol (TP). It rides the same 29-bit CAN bus and
the same PGN addressing, but it has its own framing, no handshake, and no flow
control. This tutorial explains why a second transport exists, how the frames are
laid out, how reassembly works, where it can silently fail, and how to drive it
with `machbus`'s `FastPacketProtocol`.

If you are coming from ISOBUS, the one-line summary is: Fast Packet is what TP
would look like if you stripped out the connection setup and the acknowledgements
and just streamed the data. Read [Transport Protocol](../standards/iso11783-datalink-transport.md)
first if you want the contrast in full.

## Why this exists

A classic CAN frame carries at most eight bytes. Many NMEA 2000 parameter groups
need a few dozen. ISOBUS solves the same problem with TP and ETP, but those
protocols pay for reliability with round trips: a request to send, a clear to
send, sequenced data, and an end-of-message acknowledgement. On a marine or
mixed network where the same parameter group is broadcast many times a second to
every listener at once, that handshake is the wrong trade. There is no single
receiver to acknowledge, and a dropped frame is cheaper to ignore than to
re-negotiate, because a fresh copy of the data is already on its way.

Fast Packet is the answer to "I have 9 to 223 bytes, I am broadcasting to
everyone, and the data refreshes faster than I could retransmit it." It chops the
payload into a short burst of back-to-back frames carrying a tiny counter, and
the receiver stitches them back together. No setup, no acknowledgement, no
retry — if a frame is lost, that copy of the message is lost and the next periodic
broadcast takes its place.

`machbus` is not certified for any standard; treat this as a faithful software
model of the framing, not a conformance statement.

## Mental model

A Fast Packet transfer is one *first frame* followed by zero or more
*continuation frames*, all sharing the same source address, PGN, and a small
3-bit **sequence counter** that tags the whole burst. Inside each frame the low
five bits are a **frame counter** that orders the pieces.

```
 sender broadcasts a 30-byte payload  (sequence = 3)
 ───────────────────────────────────────────────────────────────
   byte0           byte1        bytes 2..8
 ┌──────────────┬───────────┬──────────────────────────────┐
 │ seq=3 │ fc=0 │ total=30  │ payload[0..6]                 │   first frame
 └──────────────┴───────────┴──────────────────────────────┘
   byte0                 bytes 1..8
 ┌──────────────┬───────────────────────────────────────────┐
 │ seq=3 │ fc=1 │ payload[6..13]                             │   continuation
 ├──────────────┼───────────────────────────────────────────┤
 │ seq=3 │ fc=2 │ payload[13..20]                            │   continuation
 ├──────────────┼───────────────────────────────────────────┤
 │ seq=3 │ fc=3 │ payload[20..27]                            │   continuation
 ├──────────────┼───────────────────────────────────────────┤
 │ seq=3 │ fc=4 │ payload[27..30]  + 0xFF padding            │   continuation
 └──────────────┴───────────────────────────────────────────┘
 ───────────────────────────────────────────────────────────────
 receiver keys the session on (source, PGN, seq), counts frames
 0,1,2,3,4 in order, and emits the reassembled 30-byte message.
```

The receiver never speaks. It just listens, accumulates, and either completes a
message or drops the partial one. Because the sequence counter distinguishes
bursts, the *same* sender can have more than one Fast Packet transfer for the
*same* PGN in flight at once without the pieces getting mixed up.

## Anatomy: first frame vs continuation frame

Every Fast Packet frame spends its first byte on two packed counters:

- The high three bits are the **sequence counter** (`0..=7`). All frames of one
  transfer carry the same value; the sender bumps it for the next transfer so a
  late frame from an old burst cannot poison a new one.
- The low five bits are the **frame counter** (`0..=31`). The first frame of a
  transfer is always frame counter `0`; continuation frames count up `1, 2, 3,
  …` in transmission order.

That split is why a transfer is capped: five bits of frame counter and a few
bytes per frame is all you get.

| Frame | Byte 0 | Byte 1 | Bytes 2..8 | Payload bytes carried |
| --- | --- | --- | --- | --- |
| First (frame counter 0) | seq + counter | total payload length | first slice of data | `FIRST_FRAME_DATA` = 6 |
| Continuation (1, 2, …) | seq + counter | data | data | `SUBSEQUENT_FRAME_DATA` = 7 |

Only the first frame spends a byte on the **total length**, so the receiver knows
up front how big the final message is and how many continuation frames to expect.
Every continuation frame gives up one extra payload byte (seven instead of six)
because it does not repeat that length. Unused trailing bytes in the last frame
are padded with `0xFF`. `machbus` exposes these slice widths as
`net::fast_packet::FIRST_FRAME_DATA` and `SUBSEQUENT_FRAME_DATA`.

### The maximum payload

With six bytes in the first frame, seven in each of up to 31 continuation frames,
and an eight-bit length field, the ceiling is **223 bytes**. `machbus` pins that
to a single constant, `net::constants::FAST_PACKET_MAX_DATA = 223`, mirrored as
`FastPacketProtocol::MAX_DATA_LENGTH`. Anything larger is not a Fast Packet at
all — it belongs to TP or ETP. Anything eight bytes or smaller is not a Fast
Packet either: it fits in one ordinary CAN frame, so `send` refuses it.

## Reassembly and what happens when a frame is lost

The reassembler is deliberately simple, and the simplicity is the point. For each
incoming frame `machbus`:

1. Reads byte 0 and splits it into sequence and frame counters.
2. If the frame counter is `0`, starts a **new session**: it reads the total
   length, allocates a buffer, copies the first six bytes, and records that it
   now expects frame counter `1`.
3. Otherwise it looks for an in-flight session matching `(source, PGN,
   sequence)`. If none exists, the frame is an orphan and is dropped.
4. It checks that the frame counter equals the **expected** next value. If it
   does, the seven bytes land at the right offset and the expected counter
   advances. When the accumulated byte count reaches the declared total, the
   session completes and a reassembled `Message` is returned.

There is no retransmission, and this is the sharp difference from TP. If a
continuation frame is dropped or arrives out of order, the next frame's counter
will not match what the receiver expects, so `machbus` **discards the whole
in-flight session** and waits for a fresh first frame. The partially built
message is thrown away, not patched. TP would request the missing packet; Fast
Packet cannot, because there is no back channel and the sender already moved on.

A few more behaviors fall out of this design:

- **Orphan continuation frame.** A continuation frame with no matching first
  frame (you joined mid-burst, or the first frame was lost) is dropped and
  counted as a dropped frame.
- **Malformed first frame.** A first frame whose declared length is `≤ 8` or
  `> 223` is rejected outright; nothing is allocated.
- **Short frame.** A frame shorter than eight bytes is dropped before it can
  touch any session.
- **Timeout.** A session that stops making progress is dropped after the
  transport idle window (`TP_TIMEOUT_T1_MS`) once you call `update`. This is the
  cleanup that stops a half-finished broadcast from lingering forever.

All of these increment counters on a `TransportStats` snapshot you can read back,
so a stalled reassembly is observable rather than mysterious.

## Single-frame vs TP vs ETP vs Fast Packet

| Transport | Used for | Handshake | Flow control | Max payload |
| --- | --- | --- | --- | --- |
| Single CAN frame | Up to 8 bytes; the common case | No | No | 8 bytes |
| Fast Packet (NMEA 2000) | 9–223 byte broadcast parameter groups | No | No | `FAST_PACKET_MAX_DATA` = 223 |
| TP (ISOBUS / J1939) | 9–1785 bytes, point-to-point or BAM | Yes (RTS/CTS), BAM is open-loop | Yes (CTS windows) | `TP_MAX_DATA_LENGTH` = 1785 |
| ETP | Very large transfers | Yes | Yes | `ETP_MAX_DATA_LENGTH` ≈ 117 MB |

The shape of the decision: pick the smallest transport that fits the payload, and
pick Fast Packet over TP when the data is a repeating broadcast where a lost copy
is cheaper than a retransmit. TP earns its handshake when you need a reliable
delivery to a specific node; Fast Packet earns its silence when you are
firehosing the same value to the whole bus.

## Doing it with machbus

The whole protocol is one type, `net::FastPacketProtocol`. One side calls `send`
to chop a payload into frames; the other side feeds each frame to `process_frame`
and watches for the completed `Message`. The `transport_demo` example wires both
ends together for a GNSS-position payload:

```rust
{{#include ../../../examples/transport_demo.rs:100:119}}
```

What the calls mean:

- `FastPacketProtocol::new()` builds a sender/receiver with the default cap on
  simultaneous receive sessions (`FAST_PACKET_DEFAULT_MAX_RX_SESSIONS = 32`).
  Use `with_max_rx_sessions(n)` to bound memory more tightly; a cap of `0`
  refuses all new multi-frame sessions.
- `send(pgn, data, source)` returns the `Vec<Frame>` for the transfer and
  advances the internal transmit sequence counter so the next call uses a fresh
  sequence value. It errors with a buffer-overflow code for payloads over 223
  bytes and an invalid-state code for payloads of eight bytes or fewer (those
  belong in a single frame). It also rejects an invalid PGN or a null/broadcast
  source address before allocating anything.
- `process_frame(&frame)` returns `Some(Message)` only when the frame that
  completes a session arrives, and `None` for every frame before it (and for
  every frame it drops). The reassembled `Message` carries the PGN, the source,
  a broadcast destination, and the timestamp of the *last* frame that completed
  it.
- `update(elapsed_ms)` ages in-flight sessions and drops the ones that have gone
  quiet past `TP_TIMEOUT_T1_MS`. Call it from your bus loop so stalled
  reassemblies are reclaimed.

For diagnostics, `rx_session_count()` reports how many reassemblies are in flight
and `stats()` returns a `TransportStats` snapshot (dropped frames, dropped
sessions, timeouts, resource rejections); `clear_stats()` resets them without
disturbing live sessions.

## Events and responsibilities

Fast Packet has no events to subscribe to and no acknowledgements to send. Your
responsibilities are narrow but real:

- **Pump `process_frame` for every received frame** whose PGN you handle, and act
  on the `Message` when one completes.
- **Call `update` on a regular tick** so timed-out sessions are reclaimed; if you
  never call it, a partial broadcast holds a buffer until the sender happens to
  restart that exact sequence.
- **Do not expect delivery guarantees.** Treat a missing or late message as
  normal, because the protocol gives you no way to ask for a retransmission.
- **Respect the size envelope.** Build payloads of 9–223 bytes for Fast Packet;
  route anything larger through TP or ETP.

## Edge cases and failure modes

- **Dropped continuation frame.** The next frame's counter mismatches, the whole
  session is discarded, and `dropped_sessions` increments. There is no recovery
  short of the next first frame.
- **Out-of-order frames.** Same outcome as a drop — the reassembler expects
  strictly increasing frame counters and tears down on the first surprise.
- **Two bursts, same source and PGN.** Allowed, as long as the sender used
  different sequence counters; they reassemble in parallel. If a new first frame
  reuses a sequence value already in flight, it replaces the stale session rather
  than duplicating it.
- **Sequence counter wrap.** The transmit sequence is three bits, so it wraps
  after eight transfers. A wrapped first frame for a `(source, PGN, seq)` that
  still has a stale reassembly pending replaces that stale state — it does not
  grow an unbounded pile of sessions.
- **Session cap reached.** New first frames are refused once `max_rx_sessions`
  in-flight sessions exist; the frame is dropped and a resource rejection is
  recorded. This bounds worst-case memory under a flood of first frames.
- **Null or broadcast source.** A frame claiming `NULL_ADDRESS` or
  `BROADCAST_ADDRESS` as its source is invalid and dropped on both send and
  receive paths.

## Advanced

- **Mixing with NMEA PGNs.** Fast Packet is a transport, not a message format.
  The reassembled `Message` still has to be interpreted as the specific NMEA 2000
  parameter group its PGN names. See [NMEA 2000](nmea-2000.md) for how `machbus`
  routes a completed Fast Packet payload into the NMEA layer, and
  [Serial GNSS](serial-gnss.md) for the position-data path that most commonly
  rides Fast Packet.
- **Performance.** A transfer is a short burst of back-to-back frames with no
  inter-frame negotiation, so it is fast and cheap on the wire — the cost is the
  absence of recovery. On a busy bus, sizing `max_rx_sessions` to the number of
  distinct broadcasters you expect keeps memory predictable without dropping
  legitimate traffic.
- **Surface vs low-level.** `FastPacketProtocol` is the low-level building block.
  In a full stack the NMEA layer drives it for you; reach for the protocol object
  directly in tests and tightly controlled loops where you own each frame.

## Validate locally

```sh
make run EXAMPLE=transport_demo
make test
```

The `transport_demo` example sends a multi-frame Fast Packet payload and
reassembles it end to end in software, alongside the TP and ETP paths so you can
see the three transports side by side. The unit and property tests in the Fast
Packet module exercise the maximum 223-byte payload, the nine-byte two-frame
minimum, out-of-order and orphan frames, sequence-counter wrap, parallel
sessions, timeouts, and the session cap.

## What this proves / does not prove

Proves: the first-frame/continuation framing, the sequence and frame counters,
the 223-byte ceiling, in-order reassembly, parallel sessions keyed on
`(source, PGN, sequence)`, and the drop-on-mismatch behavior all behave
deterministically in software, and the `machbus` API encodes and decodes them
correctly.

Does not prove: real-hardware timing, interoperability with a specific NMEA 2000
device, or any conformance or certification claim. Those still require official
standards, real hardware, and interoperability evidence.

## See also

- [Transport Protocol](../standards/iso11783-datalink-transport.md) — the ISOBUS/J1939 multi-frame
  transport with the handshake and flow control Fast Packet omits.
- [NMEA 2000](nmea-2000.md) — how reassembled Fast Packet payloads become NMEA
  parameter groups.
- [Serial GNSS](serial-gnss.md) — the position-data path that most often rides
  Fast Packet.
