# Tutorial: decode NMEA 2000

NMEA 2000 shares the physical CAN bus and the PGN addressing model with J1939,
but its navigation messages have their own decoding style in machbus. Instead
of one struct per message with a static `decode` function, you use a
**pump-style decoder**: [`machbus::nmea::NMEAInterface`]. You subscribe to the
`on_*` events for the PGNs you care about, then feed it `Message`s. As each
message arrives, the interface decodes it and fires the matching event with the
decoded value. No protocol stack, no session, no IO.

This tutorial walks through `examples/nmea2000_decode.rs`. It sets up an
interface listening for the GNSS navigation profile, encodes a position fix and
a course/speed update with the matching `build_*` helpers, and feeds the bytes
back in so the decode path actually fires and the event handlers print.

## The imports

```rust
use machbus::geo::Wgs;
use machbus::nmea::{GNSSPosition, NMEAConfig, NMEAInterface};
use machbus::net::Message;
use machbus::net::pgn_defs::{PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_POSITION_RAPID};
```

- `NMEAInterface` is the pump decoder itself — it holds the event channels and
  any reassembly state.
- `NMEAConfig` selects which PGN families the interface should decode. You
  enable the parts you need rather than paying for everything.
- `GNSSPosition` is the decoded position type: a WGS coordinate plus fix
  metadata (satellites, fix type, and more).
- `Wgs` (from `machbus::geo`) is the geographic-coordinate type: latitude,
  longitude, altitude.
- `Message` (from `machbus::net`) is the assembled message you feed in — the
  same type the J1939 codecs use.
- `PGN_GNSS_POSITION_RAPID` (129025) and `PGN_GNSS_COG_SOG_RAPID` (129026) are
  named PGN constants from `machbus::net::pgn_defs`, so you do not sprinkle
  magic numbers through your code.

## The pump / event decode model

The mental model is different from the J1939 one-shot codecs, so it is worth
stating plainly:

1. You construct an `NMEAInterface` with a config that says which PGNs to
   decode.
2. You **subscribe** handlers to the `on_*` events. Each event corresponds to a
   decoded quantity — `on_position`, `on_cog`, `on_sog`, and so on.
3. You **feed** the interface raw `Message`s with `handle_message`. For each
   message, the interface decodes it (reassembling Fast Packet PGNs across
   multiple frames first, if needed) and dispatches the result to every
   subscribed handler for that event.

This inverts control compared to calling `decode` yourself: you describe what
you want once, then push messages through. It maps naturally onto a real bus
loop where messages arrive continuously and you react to them.

## Setting up the interface

```rust
{{#include ../../../examples/nmea2000_decode.rs:setup}}
```

Walk through it:

- `NMEAConfig::default().with_gnss_navigation(true)` builds a config that
  enables the GNSS navigation profile — rapid position, COG/SOG, heading,
  attitude, and the related navigation PGNs. The builder style means you can
  chain `.with_*` calls to enable exactly the families you need.
- `NMEAInterface::new(config)` constructs the decoder. It is declared `mut`
  because subscribing and feeding messages mutate its internal state.
- `nmea.on_position.subscribe(|pos: &GNSSPosition| { … })` registers a handler
  for decoded positions. The closure takes `&GNSSPosition` — handlers are
  `FnMut(&T)`, so they receive the decoded value by reference and can hold
  mutable captured state across calls. Inside, the example reads
  `pos.wgs.latitude`, `pos.wgs.longitude`, `pos.satellites_used`, and
  `pos.fix_type`.
- `nmea.on_cog.subscribe(|cog_rad: &f64| { … })` and
  `nmea.on_sog.subscribe(|sog_mps: &f64| { … })` register handlers for course
  over ground and speed over ground. Note the units carried over the wire: COG
  is in **radians**, SOG is in **metres per second**. The handlers convert for
  display — `cog_rad.to_degrees()` and `sog_mps * 3.6` (m/s → km/h). machbus
  hands you SI units; presentation is your choice.

Nothing has decoded yet at this point. You have only declared what to do when a
position, course, or speed arrives.

## Feeding messages

```rust
{{#include ../../../examples/nmea2000_decode.rs:feed}}
```

This is where the decode path fires:

- The `GNSSPosition` is built with `Wgs::new(52.379_189, 4.899_431, 0.0)` —
  latitude, longitude, altitude for a point in Amsterdam — and
  `satellites_used: 12`. The `..Default::default()` fills the remaining fields
  (fix type and the rest) with their defaults.
- `NMEAInterface::build_position(&fix)` encodes that fix into a `[u8; 8]`
  payload for PGN 129025, the rapid-position message. This is the inverse of
  what the decoder does — it exists so the example can produce valid bytes to
  feed back in.
- `nmea.handle_message(&Message::new(PGN_GNSS_POSITION_RAPID, pos_bytes.to_vec(), 0x80))`
  feeds the encoded bytes back as a `Message` from source `0x80`. The interface
  recognises PGN 129025, decodes it to a `GNSSPosition`, and fires
  `on_position` — which runs your subscribed handler and prints the line.
- The second pair does the same for course and speed:
  `NMEAInterface::build_cog_sog(60.0_f64.to_radians(), 5.0)` encodes a heading
  of 60° (passed in as radians) and a speed of 5 m/s into PGN 129026's payload,
  and `handle_message` feeds it in. The interface fires both `on_cog` and
  `on_sog`, running their handlers.

## The rapid-PGN satellite caveat

Look closely at the expected output below: it reports `sats 0`, even though the
fix was built with `satellites_used: 12`. **This is correct, not a bug.** PGN
129025 is the *rapid* position message — it is deliberately minimal, carrying
only latitude and longitude so it can be sent at high rate. It does not carry a
satellite count. When the interface decodes a rapid-position frame, there is no
satellite field to read, so `satellites_used` decodes to its default of `0`.

The takeaway: a decoded struct reflects only what the wire format actually
carries. If you need the satellite count, fix quality, and the rest, you
subscribe to the richer (Fast Packet) GNSS PGN that carries them — but you pay
for it in bus bandwidth and reassembly. The rapid PGN is for position, fast;
nothing more.

## Fast Packet reassembly

Many NMEA 2000 PGNs carry more than eight bytes and are transmitted as a *Fast
Packet*: a sequence of CAN frames the receiver must stitch back together before
decoding. `handle_message` handles this transparently. You feed it each frame
as it arrives, and it holds partial state internally; only when a Fast Packet
is complete does it decode and dispatch the event. The two PGNs in this example
are single-frame rapid messages, so no reassembly happens here, but the same
`handle_message` call is what drives reassembly for the larger PGNs.

## Run it

```console
$ cargo run --example nmea2000_decode
```

Output:

```text
position: 52.379189, 4.899431  (sats 0, fix GNSSFix)
course over ground: 60.0°
speed over ground: 18.0 km/h
```

Read it back against the inputs:

- The latitude and longitude match the Amsterdam coordinate exactly.
- `sats 0` is the rapid-PGN caveat from above — the rapid message does not
  carry a satellite count, so it decodes to the default.
- `fix GNSSFix` is the default fix type printed with `{:?}`.
- Course is `60.0°` — the 60° heading round-tripped through radians.
- Speed is `18.0 km/h` — 5 m/s converted (5 × 3.6).

## What to change for real bus data

The example encodes its own bytes with `build_position` and `build_cog_sog` so
the demo is reproducible. On a live bus you drop those entirely. Your job
becomes: read frames off the wire, wrap each one in a `Message` (PGN, payload,
source), and call `nmea.handle_message(&msg)` for every frame. The interface
does the rest — decoding single-frame PGNs immediately and reassembling Fast
Packet PGNs across frames before dispatching. Your `on_*` handlers fire exactly
as they do here. You write the subscription logic once and let the message pump
drive it.

## See also

- [NMEA 2000 on the bus](nmea2000.md) — the message families and how NMEA 2000
  relates to J1939.
- [Tutorial: inspect CAN identifiers](tut-inspect-can.md) — how to read the PGN
  off the identifier, including the high PGN range NMEA 2000 lives in.
- [Positioning: NMEA and GNSS](../standards/positioning.md) — the positioning
  standards behind these PGNs.
- [NMEA 2000](../tutorials/nmea-2000.md) — NMEA 2000 in a full application.
- [Serial GNSS](../tutorials/serial-gnss.md) — the serial-input counterpart for
  GNSS data.
