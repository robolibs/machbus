# Reading candump traces

`candump` is the fastest way to see what actually crossed a SocketCAN
interface. The text it prints is short, but every line packs a 29-bit
identifier whose fields you have to take apart by hand before the bytes make
sense. This page teaches you to read a line cold: which token is which, how to
pull priority, PGN, source, and destination out of the hex identifier, and how
to recognize the handful of traffic shapes you will see over and over. Every
example line here is one we made up to illustrate the math — none is copied
from a real capture.

## Why this exists

When something on the bus misbehaves, the trace is the ground truth. A node
that "isn't responding" might be answering to the wrong destination; an upload
that "hangs" might be stuck waiting for a clear-to-send. You cannot see any of
that from the application side — you have to read the wire. Knowing how to
decode a raw line turns a wall of hex into a conversation you can follow.

## Anatomy of a candump line

A compact `candump -L` style line looks like this (illustrative):

```text
(0.842301) can0 18EF2280#0102030405060708
```

Read it left to right:

| Token | Example | Meaning |
|---|---|---|
| timestamp | `(0.842301)` | When the frame was observed; present with `-t` options. |
| interface | `can0` | The SocketCAN device (`can0`, `vcan0`, …). |
| identifier | `18EF2280` | The CAN identifier in hex. Eight hex digits means an **extended** 29-bit ID. |
| `#` | `#` | Separator between identifier and payload. |
| payload | `0102030405060708` | The data bytes, two hex digits each. |

The common *bracketed* format carries the same information differently:

```text
(0.842301) can0  18EF2280  [8]  01 02 03 04 05 06 07 08
```

Here `[8]` is the **DLC** (data length code, the byte count), and the payload
bytes are spaced out. Both shapes describe one frame; machbus's
`candump_replay` example parses either one.

A quick reflex: **eight hex digits in the identifier means extended (29-bit),
which is what ISOBUS and J1939 use.** A two- or three-digit identifier is an
11-bit standard frame and is not ISOBUS/J1939 application traffic — the
`candump_replay` example deliberately rejects those rather than guess at them.

## Decoding the identifier by hand

The 29-bit identifier is not the PGN. It is a packed word, and the PGN is only
part of it. The layout, most-significant bit first:

```text
 bits 28..26 | 25  | 24 | 23..16 | 15..8 | 7..0
   priority  | EDP | DP |   PF   |  PS   |  SA
```

- **priority** (3 bits): lower number wins arbitration; it does not change
  meaning, only urgency.
- **EDP / DP** (2 bits): the data-page selectors. Together with PF and PS they
  form the 18-bit PGN.
- **PF** (PDU Format, 8 bits): the byte that decides PDU1 vs PDU2.
- **PS** (PDU Specific, 8 bits): either a **destination address** (PDU1) or a
  **group extension** that is part of the PGN (PDU2).
- **SA** (Source Address, 8 bits): who sent the frame.

The PF byte is the fork in the road:

- **PF < 240 → PDU1 (destination-specific).** PS is the destination address and
  is *not* part of the PGN. The PGN's low byte is zero.
- **PF ≥ 240 → PDU2 (broadcast).** PS *is* part of the PGN (the group
  extension), and the frame has no single destination — it is for everyone.

### Worked example: `18EF2280`

Take the identifier `18EF2280` and write it as 32 bits, then drop the top three
(only 29 are used):

```text
hex   1    8    E    F    2    2    8    0
bits 0001 1000 1110 1111 0010 0010 1000 0000
```

Slice it along the field boundaries above:

| Field | Bits | Value |
|---|---|---|
| priority | `110` | 6 |
| EDP | `0` | 0 |
| DP | `0` | 0 |
| PF | `1110 1111` | `0xEF` = 239 |
| PS | `0010 0010` | `0x22` |
| SA | `1000 0000` | `0x80` |

PF is `0xEF` = 239, which is **below 240**, so this is **PDU1**. That means:

- **PGN** = EDP·DP·PF·`00` = `0x00EF00` (the PS byte is *not* in the PGN; the
  low byte is forced to zero).
- **Destination** = PS = `0x22`.
- **Source** = SA = `0x80`.
- **Priority** = 6.

So `18EF2280` is "node `0x80` sends a proprietary-A message to node `0x22` at
priority 6." If PF had been `0xF0` or higher, PS would have folded into the PGN
and the frame would be a broadcast with no specific destination.

machbus does exactly this decode in `net::Identifier`: `priority()`,
`pgn()`, `source()`, and `destination()` return the same fields, and
`is_pdu2()` / `is_broadcast()` answer the PF≥240 question for you.

## A few PGNs you can compute

You do not need a dictionary to read most traffic — compute the PGN from PF/PS
and a short table covers the common cases. These values come from the machbus
PGN helpers and are illustrative, not exhaustive:

| PGN | PF / PS | Kind | What it is |
|---|---|---|---|
| `0xEE00` | EE / dst | PDU1 | Address claimed (announcing NAME ↔ address). |
| `0xEA00` | EA / dst | PDU1 | Request for a PGN. |
| `0xE800` | E8 / dst | PDU1 | Acknowledgement. |
| `0xEC00` | EC / dst | PDU1 | Transport connection management (RTS/CTS/BAM/abort). |
| `0xEB00` | EB / dst | PDU1 | Transport data transfer (the numbered packets). |
| `0xFECA` | FE / CA | PDU2 | A diagnostic message, broadcast. |

Notice the pattern: PF below `0xF0` (EE, EA, E8, EC, EB) is PDU1, so the PS slot
in the identifier is a destination, not part of the PGN. PF of `0xFE` is PDU2,
so PS (`0xCA`) is baked into the PGN and the frame goes to everyone.

## Recognizing traffic on sight

After a little practice you can name most lines without decoding every bit.

**Address claim.** PF `0xEE`, eight payload bytes — that payload is a 64-bit
NAME. Seeing several `EE00` frames right after power-up is normal: nodes are
sorting out who owns which address. See
[NAME and address claim](../standards/iso11783-network-management.md).

```text
(0.010) can0 18EEFF80#00112233445566AA   ; node 0x80 claims, broadcast
```

**A request.** PF `0xEA`, three payload bytes — those three bytes are the
requested PGN, little-endian (low byte first). A request for PGN `0xEE00`
(address claimed) shows up as payload `00 EE 00`.

```text
(0.020) can0 18EA80FF#00EE00            ; "everyone, please claim again"
```

**A transport sequence.** Watch for `0xEC` (control) and `0xEB` (data) between
the same source/destination pair. A directed transfer runs:

```text
(0.100) can0 1CEC2210#10 14 00 03 FF 00 EF 00   ; RTS: I have 0x14 bytes / 3 pkts, target PGN 0xEF00
(0.101) can0 1CEC1022#11 03 01 FF FF 00 EF 00   ; CTS: send 3 packets starting at 1
(0.102) can0 1CEB2210#01 ...                     ; DT packet 1
(0.103) can0 1CEB2210#02 ...                     ; DT packet 2
(0.104) can0 1CEB2210#03 ...                     ; DT packet 3
(0.105) can0 1CEC1022#13 14 00 03 FF 00 EF 00   ; EoMA: got all 0x14 bytes
```

The first payload byte of an `EC00` frame is the control byte: `0x10` is RTS
(request-to-send), `0x11` is CTS (clear-to-send), `0x13` is end-of-message
ack, `0x20` is BAM (broadcast announce), and `0xFF` is abort. The key insight:
**the PGN on the wire is the transport PGN, not the application PGN** — the real
target PGN rides inside the control message (the last three payload bytes of the
RTS, little-endian). See [Transport protocol](../standards/iso11783-datalink-transport.md).

**A broadcast.** PF `0xF0` or higher, or PDU1 sent to destination `0xFF`. No
reply is expected; the data is for whoever cares.

```text
(0.200) can0 18FEF142#...                ; PDU2 broadcast from node 0x42
```

## Tips for capturing and replaying

To capture on a live or virtual interface:

```sh
candump -td -L can0 > capture.candump      # -td: delta timestamps, -L: log format
candump -l can0                            # writes a timestamped candump-*.log file
```

To replay or summarize a capture with machbus, the `candump_replay` example
parses each line, rebuilds the frame, rejects anything that is not a valid
extended ISOBUS/J1939 shape, and prints a decoded one-liner per accepted frame:

```sh
cargo run --example candump_replay -- path/to/capture.candump
```

Each accepted line prints the raw identifier, the decoded PGN, source,
destination, length, and payload — the same fields you sliced out by hand
above — followed by a parsed/accepted/rejected summary. It accepts both the
compact `#` form and the bracketed `[8]` form, and it ignores CAN FD, error, and
flagged frames rather than guessing them into classic CAN. To push a real
capture onto a `vcan` interface and watch a node react, see the
[SocketCAN replay tutorial](../tutorials/socketcan-replay.md).

## Common confusions

- **The hex identifier is not the PGN.** `18EF2280` is the whole 29-bit word.
  The PGN (`0xEF00` here) is only the EDP/DP/PF part — and for PDU1 the PS byte
  is a destination, not PGN content. Strip priority and source first.
- **The PDU1 destination byte hides in the identifier.** For PF < 240, the PS
  slot is the destination. If you compute the PGN with PS still in it, you will
  invent PGNs that do not exist and miss who the frame was addressed to.
- **Byte order inside the payload.** The PGN carried inside a request or a
  transport RTS is little-endian: low byte first. `00 EE 00` means PGN
  `0x00EE00`, not `0x00EE00` read big-endian. The same applies to most
  multi-byte numeric fields.
- **Standard vs extended.** A short identifier (two or three hex digits) is an
  11-bit standard frame and is not ISOBUS/J1939 application traffic. Do not
  decode it with the PF/PS/SA layout.
- **DLC versus actual bytes.** In the bracketed form, `[8]` is the declared
  length. If the spelled-out bytes do not match it, the line is malformed and
  machbus's parser drops it instead of padding or truncating.

## See also

- [PGN, priority, source, destination](../standards/j1939.md) —
  the field-by-field primer behind this decoding.
- [CAN and J1939](../standards/j1939.md) — where the 29-bit identifier comes from.
- [Transport protocol](../standards/iso11783-datalink-transport.md) — the RTS/CTS/data dance you will
  see in traces.
- [SocketCAN replay](../tutorials/socketcan-replay.md) — capture and replay on a
  virtual bus, end to end.

This is a reading guide, not a conformance claim: machbus is not certified, and
real deployment still needs official standards, hardware, and interoperability
evidence.
