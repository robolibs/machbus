# Tutorial: inspect CAN identifiers

Every frame on a J1939, ISOBUS, or NMEA 2000 bus carries a 29-bit *extended*
identifier. That identifier is not an opaque number — it is a packed structure
holding the message priority, the Parameter Group Number (PGN), the source
address, and (sometimes) a destination address. Before you decode a single
data byte you can already learn who sent the frame, who it is for, and roughly
what it contains, purely from the identifier.

This tutorial walks through `examples/can_inspect.rs`, a tiny program that
pulls a 29-bit identifier apart using nothing but [`machbus::net`]. There is no
protocol stack here, no address claim, no session, and no IO — just bit
arithmetic wrapped in a typed API. If all you want is to sniff a bus and label
the traffic, this is the smallest possible starting point.

## The one import you need

```rust
use machbus::net::Identifier;
```

`Identifier` is the typed view over a raw 29-bit CAN identifier. You build one
from a `u32` and then ask it questions. It owns no data buffer and performs no
allocation; constructing one and reading a field is just masking and shifting,
so you can call it on every frame in a high-rate capture without worrying about
cost.

## Decomposing one identifier

Here is the heart of the program — the `describe` function that takes a raw
`u32` and prints everything the identifier encodes:

```rust
{{#include ../../../examples/can_inspect.rs:decode}}
```

Walk through it call by call.

- `Identifier::from_raw(raw)` takes the raw 29-bit value as a plain `u32` and
  wraps it. This is infallible: any `u32` is a valid identifier as far as the
  bit layout is concerned (the upper 3 bits are simply ignored — only 29 bits
  are meaningful). There is no `Option` and no error here; the fallible step
  comes later when you try to decode the *payload*.

- `id.priority()` returns the message priority. Lower numbers are higher
  priority on the bus (priority 0 wins arbitration over priority 7). The return
  type is a small `Priority` newtype rather than a bare integer, which is why
  the example writes `u8::from(id.priority())` to get a number it can print.
  Typical values you will see: 3 for fast control messages like engine speed,
  6 for slower informational and network-management traffic, 2 for some
  high-rate NMEA 2000 navigation PGNs.

- `id.pgn()` returns the Parameter Group Number — the identity of the message.
  The PGN is what tells you "this is engine speed" versus "this is wheel
  speed". The example prints it twice, once as hex (`0x{:04X}`) and once as
  decimal (`{:>6}`), because lookup tables in different specifications quote it
  in different bases.

- `id.source()` returns the source address: the 8-bit address of the ECU that
  transmitted the frame. On a live bus this is assigned dynamically through
  address claiming, but in the identifier itself it is always the low byte.

- `id.is_pdu2()` distinguishes the two addressing formats (explained in detail
  below). The example uses it to pick the label and to decide whether a
  destination address even exists.

- `id.destination()` returns the destination address — but **only meaningful
  for PDU1 frames**. For a PDU2 (broadcast) frame there is no destination, so
  the example never calls `destination()` in that branch; it prints `ALL`
  instead.

## The samples it decodes

The program runs `describe` over a list of real-world 29-bit identifiers:

```rust
{{#include ../../../examples/can_inspect.rs:samples}}
```

These are not toys. `0x0CF00400` is the classic EEC1 engine-speed broadcast.
`0x18EAFF00` is a PGN request sent to the global address. `0x18EEFF80` is an
Address Claimed announcement. `0x09F80180` is an NMEA 2000 rapid-position
frame. Each one exercises a different corner of the identifier layout, which is
exactly why they make a good demonstration set.

## The PDU1 / PDU2 trap

This is the single most important rule when reading CAN identifiers on these
buses, and it is the one people get wrong. The relevant field inside the PGN is
the **PDU Format** byte, conventionally called PF:

- **PF < 240** → this is **PDU1**, a *peer-to-peer* format. The byte that
  would otherwise be part of the PGN is actually the **destination address**.
  The frame is directed at one specific ECU (or at `0xFF`, the global address,
  meaning everyone). Use `is_pdu2()` → `false` and read `destination()`.

- **PF ≥ 240** → this is **PDU2**, a *broadcast* format. That same byte (now
  called the PDU Specific, PS) is part of the PGN itself, not an address. There
  is no destination; the frame goes to the whole bus. `is_pdu2()` → `true`,
  and `is_broadcast()` → `true`.

The example asserts exactly this distinction so the behaviour is pinned down in
code:

```rust
{{#include ../../../examples/can_inspect.rs:pdu}}
```

Read it carefully:

- `0x18EA26EE` is a PGN request. Its PF is below 240, so it is PDU1. The low
  byte `0x26` is therefore a *destination address*, and the assertions confirm
  `is_pdu2()` is `false` and `destination()` is `0x26`. The request is aimed at
  the ECU at address `0x26`.

- `0x0CF00400` is EEC1. Its PF is `0xF0` (240), which is ≥ 240, so it is PDU2.
  The assertions confirm `is_pdu2()` and `is_broadcast()` are both `true`. The
  `0x04` low byte is part of the PGN, not an address.

If you ever find yourself reading a destination address of "4" or "0" off a
broadcast message and wondering why nobody is listening, this rule is why: you
read a PGN byte as if it were an address. `is_pdu2()` is the guard that keeps
you honest.

## Run it

```console
$ cargo run --example can_inspect
```

The program prints the decoded view of each sample identifier:

```text
0x0CF00400  prio=3  PGN=0xF004 ( 61444)  src=0x00  dst=ALL   PDU2 (broadcast)
0x18EAFF00  prio=6  PGN=0xEA00 ( 59904)  src=0x00  dst=0xFF  PDU1 (peer-to-peer)
0x18EEFF80  prio=6  PGN=0xEE00 ( 60928)  src=0x80  dst=0xFF  PDU1 (peer-to-peer)
0x09F80180  prio=2  PGN=0x1F801 (129025)  src=0x80  dst=ALL   PDU2 (broadcast)
```

Match the output against the rule. The two `EA00`/`EE00` lines are PDU1 (their
PF is below 240), so they show a real destination byte — here `0xFF`, the
global address, meaning "broadcast to all" *as a PDU1 message addressed to
everyone*, which is subtly different from a PDU2 broadcast. The `F004` and
`1F801` lines are PDU2, so their destination is reported as `ALL` and the low
byte stays part of the PGN.

Note `0x09F80180` decodes to PGN `0x1F801` (129025) — a five-hex-digit PGN.
That is expected: PDU2 PGNs can exceed 16 bits because the Data Page and
Extended Data Page bits extend the range. NMEA 2000 lives heavily in this upper
PGN space.

## What to change for real bus data

This example feeds in hard-coded identifiers so the output is reproducible. To
point it at a live bus, replace the `samples` array with identifiers you read
off the wire. With SocketCAN, for instance, you would receive a frame, take its
`id` field (the 29-bit extended identifier), and hand it straight to
`Identifier::from_raw`. Nothing else changes — `Identifier` does not care where
the `u32` came from. Once you have the PGN you can branch to the right payload
decoder, which is exactly what the next two tutorials do.

## See also

- [Anatomy of a CAN frame](can-frames.md) — the structure behind the
  identifier, in prose.
- [J1939 messages and PGNs](j1939-messages.md) — what those PGNs actually mean.
- [The networking foundation](../standards/foundations.md) — how the addressing
  model fits the larger stack.
- [SAE J1939: the heritage](../standards/j1939.md) — where PDU1/PDU2 comes from.
- [Tutorial: decode J1939 PGNs](tut-j1939.md) — the natural next step: turn the
  PGN you just identified into a typed struct.
