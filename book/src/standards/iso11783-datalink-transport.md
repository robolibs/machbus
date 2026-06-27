# ISO 11783-3 — data link and transport

A CAN frame holds at most 8 bytes. An object pool, a DDOP, or a file is far bigger.
Part 3 (carrying SAE J1939's transport) is how ISOBUS pretends messages can be any
size: it chops a large payload into 7-byte packets, ships them with flow control,
and reassembles them on the far side. Master this and the rest of the stack stops
being mysterious — almost every "describe yourself" service rides this layer.

## Why this exists

The services that make ISOBUS valuable — sending a whole UI to a terminal, a whole
device description to a task controller, a whole file to a server — are all
*bulk transfers*. Without a transport, none of them could exist on an 8-byte bus.

## The three (plus one) ways to move data

```
   ≤ 8 bytes ─────────────► single frame              (one shot)
   ≤ 1785 bytes ──────────► Transport Protocol (TP)   (7-byte packets + flow control)
   up to megabytes ───────► Extended TP (ETP)         (big counters; pools, files)
   modest multi-frame ────► Fast Packet               (NMEA 2000's lighter scheme)
```

machbus picks automatically: hand it a payload and it chooses single-frame, Fast
Packet, TP, or ETP; inbound, you receive the fully reassembled `Message`.

## Broadcast transport (BAM): fire and forget

For data addressed to everyone, the sender announces the size, then streams packets
at a fixed pace with **no feedback**. Listeners reassemble what they catch.

```
   sender ── BAM "1024 bytes / 147 packets, PGN X" ──►  (everyone)
   sender ── DT 1 ─► DT 2 ─► … ─► DT 147 ─►              (paced, no acks)
                                       │
                              receivers reassemble independently
```

## Connection-mode transport (RTS/CTS): the receiver sets the pace

Point-to-point transfers let the *destination* throttle, so a small ECU is never
flooded. This is the handshake to know cold:

```
   SENDER (drives)                                   RECEIVER (paces)

     ── RTS:  "1024 bytes, 147 packets, PGN X" ──────►   request to send
     ◄─ CTS:  "send 16 packets, starting at #1" ─────    clear-to-send window
     ── DT #1 … #16 ─────────────────────────────────►   one window of data
     ◄─ CTS:  "send 16 packets, starting at #17" ────
     ── DT #17 … #32 ────────────────────────────────►
        … repeat until all 147 packets sent …
     ── DT … #147 ───────────────────────────────────►
     ◄─ EndOfMsgAck:  "got all 1024 bytes" ─────────     done
     ◄─ (or) Abort:   "stop — reason R" ────────────     failure, any point
```

Things that bite in practice and that machbus's TP/ETP engines handle for you:

- **Windows and holds.** A receiver may send a CTS with a *zero* count to say "hold —
  I'm busy"; the sender waits and resumes on the next non-zero CTS.
- **Timeouts.** If a CTS or a data packet does not arrive within the protocol window,
  the session aborts; robust callers retry the whole transfer.
- **One session per peer-pair-and-PGN.** Starting a second transfer to the same peer
  for the same PGN while one is active is an error; machbus queues it behind the
  active one.
- **Abort.** Either side can abort with a reason at any point; the half-sent payload
  is discarded.

## Extended TP and Fast Packet

**ETP** is the same shape as RTS/CTS with larger counters for megabyte payloads (big
object pools, files). **Fast Packet** is NMEA 2000's lighter multi-frame format for
records like a GNSS position — a header frame plus a few continuation frames, no
windowed flow control. machbus reassembles all of them beneath the message layer.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Sending any-size payloads | `Session::send_raw` / codec send — transport is automatic | [Transport Protocol](../tutorials/transport-protocol.md) |
| Fast Packet (GNSS) | `Plugin::fast_packet_pgns` registers reassembly | [Fast Packet](../tutorials/fast-packet.md) |
| Watching a transfer | `examples/transport_demo.rs` | [Reading candump traces](../isobus-basics/reading-candump-traces.md) |

## See also

- [ISO 11783-4 — network layer](iso11783-network-layer.md) — moving these messages across
  joined segments.
- [The Virtual Terminal](virtual-terminal.md) and
  [The Task Controller](task-controller.md) — the biggest users of transport.
