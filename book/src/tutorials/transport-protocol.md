# Transport Protocol (TP and ETP)

A classic CAN frame carries at most eight payload bytes. Plenty of ISOBUS and
J1939 messages — a VT object pool, a TC device description, a diagnostic list, a
GNSS record — are far larger than that. The Transport Protocol (TP) and its
larger sibling the Extended Transport Protocol (ETP) are how a control function
breaks one logical message into a stream of 8-byte frames, ships them across the
bus, and reassembles them on the far side. This tutorial explains both forms of
TP (broadcast and connection-managed), shows where ETP takes over, and grounds
all of it in the `machbus` `net::TransportProtocol` and
`net::ExtendedTransportProtocol` engines.

## Why this exists

The data link can only move eight bytes at a time, but applications think in
whole messages. Something has to chop a large payload into frames, number them,
pace them so a slow receiver is not flooded, detect a dropped or duplicated
frame, and signal the end. Doing this ad hoc per message type would be a mess,
so ISO 11783-3 (Data link layer) and J1939 define one reusable multi-frame
transport that every larger PGN rides on top of.

Two needs pull the design in different directions:

- **Broadcast.** Sometimes a producer wants to push data to *everyone* — no
  single receiver to pace against, no acknowledgement to wait for. Fast, simple,
  fire-and-forget.
- **Point-to-point.** Sometimes there is exactly one receiver that must be able
  to pace the flow, reject the transfer if it is out of buffer space, and
  confirm that every byte arrived. Slower, but reliable and flow-controlled.

TP answers both: a broadcast form (BAM) and a connection-managed form (CMDT,
built on RTS/CTS). ETP answers a third need — payloads too big for TP's 16-bit
byte counter.

## Mental model

Every multi-frame transfer is split into two channels that share the same PGN
target but use different connection-management byte codes:

- a **connection-management** channel that sets the transfer up, paces it, and
  closes it out (RTS, CTS, EndOfMsgAck, BAM, Abort), and
- a **data-transfer** channel that carries the actual bytes, one sequence number
  plus seven payload bytes per frame.

### Connection-managed (CMDT) exchange

```
sender (0x10)                              receiver (0x20)
   │                                              │
   │  RTS  (here come N bytes, P packets) ───────►│   open the session,
   │                                              │   allocate a buffer
   │◄─────── CTS (send me K packets from seq S)   │   receiver paces
   │                                              │
   │  DT seq 1 ──────────────────────────────────►│
   │  DT seq 2 ──────────────────────────────────►│   ingest 7 bytes/frame
   │   ...  (K frames) ──────────────────────────►│
   │                                              │
   │◄─────── CTS (next window) ───────────────────│   repeat until all
   │  DT ...  ───────────────────────────────────►│   packets delivered
   │                                              │
   │◄─────── EndOfMsgAck (got all N bytes) ───────│   confirm + close
   ▼                                              ▼
 Complete                                      Complete
```

The receiver is in charge of pace: it never gets more than the **per-CTS packet
count** it just asked for, and it can pause the sender by issuing a CTS for zero
packets. Any problem on either side ends the session with an **Abort**.

### Broadcast (BAM) exchange

```
sender (0x10)                       everyone (0xFF)
   │                                       │
   │  BAM (here come N bytes, P packets) ──►│   each listener opens a
   │                                       │   receive session
   │  DT seq 1  ───────────────────────────►│
   │   (≥50 ms gap)                         │
   │  DT seq 2  ───────────────────────────►│   no CTS, no ack —
   │   ...                                  │   purely time-paced
   │  DT seq P  ───────────────────────────►│
   ▼                                       ▼
 done (no ack)                       Complete per listener
```

BAM has no handshake. The sender announces, then drips out data frames spaced by
a minimum interval (`TP_BAM_INTER_PACKET_MS`, 50 ms in `machbus`) so listeners
can keep up. There is no acknowledgement and no retransmission: a listener that
misses a frame simply fails to reassemble and drops the session.

## Size boundaries: single frame → TP → ETP

The choice of transport is decided purely by payload length, and `machbus`
pins the boundaries with compile-time constants in `net::constants`:

| Payload length | Mechanism | Constant |
| --- | --- | --- |
| `0..=8` bytes | One ordinary CAN frame, no transport | `CAN_DATA_LENGTH` = 8 |
| `9..=1785` bytes | TP (BAM or CMDT) | `TP_MAX_DATA_LENGTH` = 1785 |
| `1786..=117_440_505` bytes | ETP (connection-mode only) | `ETP_MAX_DATA_LENGTH` = 117_440_505 |

These bounds are enforced, not advisory. `TransportProtocol::send` rejects a
payload of 8 bytes or fewer with an "use single frame" error and rejects
anything above `TP_MAX_DATA_LENGTH` with a buffer-overflow error.
`ExtendedTransportProtocol::send` mirrors that from the other side: it rejects
anything at or below 1785 bytes ("use TP") and anything above the ETP ceiling.
TP's 1785-byte limit is a direct consequence of its wire format: the byte count
is a 16-bit field and the packet count is a single byte, so at seven bytes per
packet the largest transfer is 255 packets — exactly 1785 bytes.

Every data frame, in both protocols, carries one sequence byte plus seven
payload bytes (`TP_BYTES_PER_FRAME` = 7), so the packet count for any payload is
always `ceil(total_bytes / 7)` — exposed as `TransportSession::total_packets`.

## Anatomy: the two message channels

`machbus` routes a frame to a transport engine by its target PGN. TP uses one
PGN for connection management and one for data transfer; ETP uses its own pair.
You will rarely touch these bytes directly, but understanding them makes the
state machine legible.

**TP connection-management frames** all start with a control byte
(`net::tp::tp_cm`):

| Control | Name | Who sends it | Carries |
| --- | --- | --- | --- |
| `RTS` (0x10) | Request To Send | sender | total bytes, total packets, advertised packets-per-CTS, target PGN |
| `CTS` (0x11) | Clear To Send | receiver | how many packets to send now, which sequence to start at |
| `EOMA` (0x13) | EndOfMsgAck | receiver | echoed total bytes/packets, confirming completion |
| `BAM` (0x20) | Broadcast Announce | sender | total bytes, total packets, target PGN |
| `ABORT` (0xFF) | Connection Abort | either side | the abort reason byte |

**TP data-transfer frames** are dead simple: byte 0 is the 1-based sequence
number, bytes 1–7 are payload. The receiver computes the buffer offset from the
sequence number (`(seq - 1) * 7`) and copies the seven bytes into place.

The connection-management vs data-transfer split is the heart of the design:
control traffic and bulk traffic are separated so flow control can interleave
with the byte stream without ambiguity.

## Lifecycle and state machine

`machbus` models each transfer as a `net::TransportSession` whose `state` field
is a `net::SessionState`. The same enum serves both directions; which states a
session passes through depends on whether it is a `Transmit` or `Receive`
session (`net::TransportDirection`) and whether it is broadcast or
connection-managed.

| State | Side | Meaning |
| --- | --- | --- |
| `None` | — | Freshly constructed, not yet started. |
| `WaitingForCTS` | sender (CMDT) | RTS sent; waiting for the receiver to clear a window. |
| `SendingData` | sender (CMDT) | A CTS granted a window; drain that many DT frames. |
| `WaitingForEndOfMsg` | sender (CMDT) | Last packet sent; waiting for EndOfMsgAck. |
| `ReceivingData` | receiver (BAM) | Mid-stream broadcast reassembly. |
| `WaitingForData` | receiver (CMDT) | CTS sent; waiting for the granted DT window. |
| `Complete` | either | All bytes transferred (and acknowledged, for CMDT). |
| `Aborted` | either | Torn down before completion. |

The CMDT sender walks `WaitingForCTS → SendingData → WaitingForCTS → … →
WaitingForEndOfMsg → Complete`, looping back to `WaitingForCTS` after each window
until the last packet, then waiting for the ack. The CMDT receiver walks
`WaitingForData → WaitingForData → … → Complete`, issuing a fresh CTS each time
the granted window is exhausted and emitting the EndOfMsgAck once the final byte
lands. A BAM receiver starts in `ReceivingData` and goes straight to `Complete`
when the last sequence arrives — no CTS, no ack.

### Per-CTS packet count and back-pressure

The per-CTS packet count is the receiver's flow-control knob. The RTS advertises
how many packets the sender is willing to send per round (byte 4), but the
*receiver's* CTS is authoritative: the sender clamps each window to the
receiver's requested count, capped by `TP_MAX_PACKETS_PER_CTS` (16). A receiver
can therefore slow a fast sender simply by granting small windows, and a CTS
that requests **zero** packets is a hold — the receiver is paused but the session
stays alive. While paused, `machbus` re-emits a keep-alive CTS every
`TP_T_HOLD_MS` (500 ms) so the sender's timer does not expire.

### Timeout windows

Timers live on the session and advance through `TransportProtocol::update`.
Different waiting states use different windows, taken from `net::constants`:

| Waiting state | Timeout constant | Value |
| --- | --- | --- |
| `WaitingForCTS` / `WaitingForEndOfMsg` (sender) | `TP_TIMEOUT_T3_MS` | 1250 ms |
| `WaitingForData` / `ReceivingData` (receiver) | `TP_TIMEOUT_T1_MS` | 750 ms |

The auxiliary `TpTimerSession` tracker (a coarse `TpSessionState` view used by
the network layer) also applies `TP_TIMEOUT_T4_MS` (1050 ms) while actively
sending. When any window elapses, the session moves to `Aborted`, fires the
abort event with reason `Timeout`, and — for a connection-managed session — puts
an Abort frame on the wire. A BAM session that stalls just expires silently;
there is nobody to notify.

### Every abort reason

Aborts are wire-compatible bytes, modelled as `net::TransportAbortReason`. Every
variant maps to a real failure path in the engines:

| Reason | Numeric | Raised when |
| --- | --- | --- |
| `None` | 0 | Placeholder / unknown byte decoded back to no-reason. |
| `AlreadyInSession` | 1 | An RTS arrives for a transfer that already has a live receive session. |
| `ResourcesUnavailable` | 2 | No free session slot, or the advertised size exceeds the receive-allocation cap. |
| `Timeout` | 3 | A waiting window elapsed. |
| `ConnectionModeError` | 4 | A CTS arrived while the sender was already mid-window (a protocol-ordering error). |
| `MaxRetransmitsExceeded` | 5 | The receiver asked the sender to back up too many times. |
| `UnexpectedPgn` | 6 | Reserved; a frame targeted a PGN that does not fit the session. |
| `BadSequence` | 7 | A data frame arrived with the wrong sequence (zero, ahead, or — in ETP — before its DPO). |
| `DuplicateSequence` | 8 | A data frame repeated a sequence already received. |
| `UnexpectedDataSize` | 9 | The advertised byte count is malformed or out of range for the protocol. |

The numeric column is the byte sent on the wire, so an abort received from a
real ECU decodes to the same variant via `TransportAbortReason::from_u8`.

## Doing it with machbus

The `transport_demo` example drives a full CMDT round trip in software. Both
sides are plain `TransportProtocol` engines; you pump frames between them by
hand, which makes every step visible:

```rust
{{#include ../../../examples/transport_demo.rs:17:56}}
```

Read it as the state machine in motion:

1. `tx.send(...)` validates the payload, opens a `Transmit` session in
   `WaitingForCTS`, and returns the RTS frame.
2. `rx.process_frame(rts)` opens a `Receive` session, allocates the reassembly
   buffer, and returns a CTS granting the first window.
3. `tx.process_frame(cts)` moves the sender to `SendingData`;
   `tx.get_pending_data_frames()` drains that window as DT frames.
4. Feeding the DT frames into `rx.process_frame` reassembles the bytes and, once
   the last packet lands, returns the EndOfMsgAck.
5. `tx.process_frame(eoma)` confirms delivery and fires `on_complete`.

For broadcast, you call `send` with the destination set to `BROADCAST_ADDRESS`;
the engine emits a BAM instead of an RTS, then `TransportProtocol::update`
releases one DT frame per `TP_BAM_INTER_PACKET_MS` interval until the payload is
drained — no CTS pump required.

You tune a receiver's caps when constructing the engine:
`TransportProtocol::with_max_receive_bytes` clamps the largest payload it will
accept (rejecting bigger RTS/BAM before allocating), `with_max_sessions` caps
concurrent transfers, and `with_advertised_packets_per_cts` sets the RTS byte-4
advertisement.

Raw TP/ETP engines reject a second active transfer that would reuse the same
data-frame source/destination path. That keeps DT frames unambiguous because the
PGN is named by the control traffic, not repeated in every data frame. The
higher-level `IsoNet` surface preserves this rule but queues same-path
application sends, so a session can answer several large requests to the same
peer without overlapping the underlying DT stream.

## Events and responsibilities

Both engines expose events your application subscribes to:

| Event | Fires when | Your job |
| --- | --- | --- |
| `on_complete` | A session reaches `Complete` | Take the reassembled `TransportSession::data` and hand it to the application layer. |
| `on_abort` | A session tears down (either side) | Log the `TransportAbortEvent` (PGN, peer, reason) and decide whether to retry. |
| `on_session_timeout` | A `TpTimerSession` window elapses | React to the higher-level timeout (TP only). |

You are responsible for the *pump*: call `process_frame` for every inbound TP/ETP
frame, call `update(elapsed_ms)` regularly so timers advance and BAM data flows,
and drain `get_pending_data_frames` after each CTS. The session facade wires
this pump for you; the raw engines are for tests and tightly controlled embedded
loops where you own the timing.

For observability, `TransportProtocol::stats` returns a `TransportStats` snapshot
counting dropped frames, dropped sessions, aborts sent and received, timeouts,
and resource rejections — the defensive paths that drop bad input instead of
panicking.

## Edge cases and failures

- **Timeout.** A receiver that goes quiet, or a sender that never gets its CTS,
  trips the window above and aborts. On a virtual bus you must pump *both* sides
  or every transfer times out.
- **Unexpected data frame.** A DT frame that does not match any open receive
  session is dropped and counted; it never creates a session on its own.
- **Out-of-order or zero sequence.** A DT frame whose sequence is ahead of the
  expected one (or zero) aborts the session with `BadSequence`.
- **Duplicate sequence.** A repeated sequence aborts with `DuplicateSequence`
  (connection-mode; a BAM receiver has no back-channel to abort with).
- **CTS while sending.** A CTS that arrives mid-window is a protocol-ordering
  error and aborts with `ConnectionModeError` — except an exact duplicate of the
  current window, which `machbus` treats as an idempotent retry so pump ordering
  cannot kill a healthy transfer.
- **Receiver back-pressure.** A CTS for zero packets pauses the sender; the
  engine keeps the session warm with periodic keep-alive CTS frames.
- **Out of resources.** An RTS/BAM whose advertised size exceeds the receive cap,
  or that arrives with no free session slot, is rejected with
  `ResourcesUnavailable` *before* any buffer is allocated — large transfers can
  be audited without reserving memory.
- **Malformed control frame.** Short frames, non-canonical reserved bytes, a
  destination-specific CM frame sent to broadcast, or an out-of-range byte count
  are all dropped (and counted) rather than acted on.

## ETP: extending TP for very large payloads

ETP exists because TP's counters run out. TP's byte count is 16 bits and its
packet count is a single byte, capping a transfer at 1785 bytes. ETP widens the
byte count to 32 bits and reaches `ETP_MAX_DATA_LENGTH` (about 117 MB). It is
**connection-mode only** — there is no broadcast ETP — so `send` rejects a
broadcast destination outright.

ETP reuses the RTS/CTS/EndOfMsgAck shape but adds one new control message: the
**Data Packet Offset (DPO)**. The problem it solves: a single DT frame's
sequence byte only counts to 255, but an ETP transfer can have millions of
packets. So ETP does not use one global sequence space. Instead, the receiver's
CTS names an absolute *next packet* (a 24-bit number), the sender replies with a
DPO announcing the **packet offset** for the upcoming group, and then the DT
frames within that group restart their sequence numbers at 1. The receiver
reconstructs the absolute byte position as `(dpo_packet_offset + seq - 1) * 7`.
A DT frame that arrives before its DPO, or a DPO whose offset does not match the
receiver's expected position, aborts with `BadSequence`.

The ETP control codes live in `net::etp::etp_cm`: `RTS` (0x14), `CTS` (0x15),
`DPO` (0x16), `EOMA` (0x17), `ABORT` (0xFF). The exchange per window is therefore
`CTS → DPO → DT×K → CTS → …`, each window still capped at `TP_MAX_PACKETS_PER_CTS`
packets. Its sole timeout is `ETP_TIMEOUT_T1_MS` (750 ms), shared by every
waiting state.

The example pumps an ETP transfer the same way as TP, just with the DPO step
folded into `get_pending_data_frames`:

```rust
{{#include ../../../examples/transport_demo.rs:60:96}}
```

`ExtendedTransportProtocol::receive_profile_for_advertised_size` lets you
validate an advertised size against the protocol and your local receive cap
*without* allocating — useful for auditing a protocol-maximum transfer profile
before committing real memory to it.

## Advanced

- **Broadcast vs peer-to-peer.** Reach for BAM when many nodes need the same
  data and loss is tolerable (or the data repeats). Reach for CMDT when one
  receiver must pace, reject, or confirm — VT object-pool uploads and TC device
  descriptions are the canonical CMDT cases.
- **Performance.** CMDT throughput is governed by the per-CTS window and how
  promptly each side pumps; BAM throughput is governed by the 50 ms inter-packet
  floor. Bigger CTS windows mean fewer round trips but less back-pressure
  headroom.
- **Fine control vs the bare codecs.** The session facade owns the pump, the
  timers, and the event fan-out; use it in applications. The bare
  `TransportProtocol` / `ExtendedTransportProtocol` engines give you frame-level
  control for unit tests and embedded loops, at the cost of driving every
  `update` and `get_pending_data_frames` yourself.
- **Concurrent sessions.** Control frames identify the target PGN, but data
  frames do not. For that reason `machbus` rejects a second TP/ETP transfer that
  would reuse the same active data-frame source/destination path, even when the
  requested PGN is different. That avoids reassembling bytes from one transfer
  into another.

## Validate locally

```sh
make run EXAMPLE=transport_demo
make test
```

The example runs a CMDT round trip, an ETP round trip, and a Fast Packet round
trip entirely in software and prints that each payload was reassembled byte for
byte. The test suites in `src/net/tp.rs` and `src/net/etp.rs` cover malformed
control frames, out-of-order and duplicate DT packets, backward/duplicate CTS
behavior, sender and receiver timeouts, invalid endpoints, session caps,
allocation caps, and the receiver-side abort direction.

## What this proves / does not prove

Proves: the packetization, reassembly, CTS windowing, DPO offset handling,
timeout aborts, and every abort reason behave deterministically in software, and
that the size boundaries between single-frame, TP, and ETP are enforced.

Does not prove: real-hardware timing, interoperability with a specific
third-party ECU, or any conformance or certification claim. `machbus` is not
certified; real deployment still needs official standards, hardware, and
interoperability evidence.

## See also

- [PGN request](request-pgn.md) — the request/response building block whose
  large responses ride on TP.
- [Fast packet](fast-packet.md) — NMEA 2000's smaller multi-frame mechanism, an
  alternative for payloads up to 223 bytes.
- [Address claim](address-claim.md) — every transport endpoint must own a source
  address first.
