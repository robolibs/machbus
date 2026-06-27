# Serial GNSS

Many GNSS receivers do not speak NMEA 2000 on a CAN bus. Instead they emit a
stream of short text lines over a plain serial port (UART/RS-232/USB). Each line
is a self-contained, human-readable sentence such as a position fix or a
course-and-speed report. This tutorial shows how `machbus` turns that raw byte
stream into typed GNSS fixes with `nmea::SerialGNSS`, how the parser validates
each line, and how the parsed result connects to the rest of the stack.

If you are looking for satellite navigation carried as binary PGNs on the CAN
bus instead of text on a wire, that is a different path — see
[NMEA 2000](nmea-2000.md). This page is specifically about the *serial,
sentence-based* receivers.

## Why this exists

A receiver chip and the application that consumes its position usually live on
different boards, connected by a UART. The receiver cannot send a Rust struct
down a wire, so it sends text: an ASCII line per measurement, terminated by a
carriage return and/or line feed. The format is a long-standing, public de-facto
convention; almost every consumer and survey-grade receiver can produce it.

The job of a serial GNSS layer is therefore narrow and well defined:

- accept bytes as they arrive, in whatever chunk sizes the OS hands you,
- find complete lines and reject corrupt ones,
- recognise the sentence kinds you care about, and
- decode their fields into engineering units and a fix-quality classification.

`machbus` does exactly this and nothing more: it is a pump-style parser with no
I/O of its own. You own the serial port; you feed the bytes; you receive typed
fixes and events.

## Mental model

```
serial port bytes ──► SerialGNSS::feed_bytes(&[u8])
                          │
                          ▼
                  split on CR / LF into lines
                          │
                          ▼
              validate one line (a "sentence")
              $  prefix · ASCII · length · checksum
                          │
              ┌───────────┴───────────┐
            valid                    invalid
              │                        │
        look at the 3-char        drop it; the last
        sentence type             good fix is untouched
              │
              ▼
        decode fields → update cached GNSSPosition
              │
              ▼
        emit on_position / on_cog / on_sog
```

The parser holds one "latest known" `GNSSPosition` and updates it field by field
as sentences arrive. A bad or unknown line never disturbs that cache, so a
glitch on the wire cannot erase a good fix.

## Anatomy of a sentence

A serial GNSS sentence is one line of printable ASCII with a fixed shape:

```
$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47
└┬┘└┬┘ └────────────── comma-separated fields ──────────────┘ └┬┘
 │  │                                                          checksum
 │  sentence type (3 chars): what this line carries
 talker ID (2 chars): which system produced it (e.g. GP, GL, GN)
```

The pieces, in the order the parser reads them:

| Part | Shape | Meaning |
| --- | --- | --- |
| Start | `$` | Marks the beginning of a sentence. Lines that do not start with it are dropped. |
| Talker ID | two characters after `$` | Which navigation system emitted the line (GPS, GLONASS, a combined solution, and so on). `machbus` skips it — it keys only on the sentence type. |
| Sentence type | three characters | The structural identity: position fix, course/speed, satellite list, etc. |
| Fields | comma-separated text | The payload. Empty fields are allowed and common; their commas still appear. |
| Checksum | `*` then two hex digits | XOR of every byte between `$` and `*`, written as uppercase hex. Optional on the wire, but verified when present. |

The whole line is ASCII and conventionally short. `machbus` caps a buffered
sentence at `NMEA0183_MAX_SENTENCE_BYTES` (96 bytes) so a receiver that never
emits a terminator cannot grow the buffer without bound.

## Which sentence kinds machbus parses

`SerialGNSS` recognises the sentence type from the three characters after the
talker ID and decodes exactly these:

| Type | Carries | What `machbus` extracts |
| --- | --- | --- |
| `GGA` | Primary position fix | Latitude, longitude, altitude, fix quality, satellites used, HDOP, geoidal separation, UTC time. |
| `RMC` | Recommended minimum | Latitude, longitude, speed over ground, course over ground, UTC time; only when the line reports a valid fix. |
| `VTG` | Track and ground speed | Course over ground and speed over ground. |
| `GSA` | Active satellites / DOPs | Fix dimensionality (no-fix vs fixed) plus PDOP, HDOP, VDOP. |
| `GLL` | Geographic lat/lon | Latitude, longitude, UTC time; only when the line is marked valid. |
| `GSV` | Satellites in view | The satellites-in-view count (kept separate from the satellites *used* in the fix). |
| `ZDA` | Date and time | A full UTC date/time plus the receiver's local-zone offset. |

Any other sentence type is ignored without touching the cache. Do not assume
support for sentence kinds not in this table — the parser silently skips them.

A note on two satellite counts: `GGA`/`RMC`-style fixes populate
`satellites_used` (how many birds contributed to the solution), while `GSV`
populates a separate `latest_satellites_in_view()` count. They mean different
things and the parser keeps them apart on purpose.

## The parse → typed fix workflow

The result type is `nmea::GNSSPosition`, a plain value with the fields the
sentences above can fill in. The most relevant ones:

| Field | Unit | Source sentences |
| --- | --- | --- |
| `wgs` (`concord::Wgs`: latitude, longitude, altitude) | degrees / metres | GGA, RMC, GLL |
| `fix_type` (`GNSSFixType`) | enum | GGA, RMC, GSA, GLL |
| `satellites_used` | count | GGA |
| `speed_mps` | metres per second | RMC, VTG |
| `cog_rad` | radians | RMC, VTG |
| `hdop` / `pdop` / `vdop` | dimensionless | GGA, GSA |
| `geoidal_separation_m` | metres | GGA |
| `timestamp_us` | microseconds since midnight UTC | GGA, RMC, GLL, ZDA |

Unit conversion happens inside the parser so you never see raw NMEA encodings:

- **Coordinates** arrive as `ddmm.mmmm` (degrees and decimal minutes) plus a
  hemisphere letter. The parser splits degrees from minutes, divides the minutes
  by 60, and applies the sign from the N/S or E/W letter to produce signed
  decimal degrees in `wgs`.
- **Speed** from `RMC` arrives in knots and from `VTG` in km/h; both are
  converted to metres per second in `speed_mps`.
- **Course** arrives in degrees and is converted to radians in `cog_rad`.
- **Time** arrives as `hhmmss.sss` and becomes microseconds-since-midnight in
  `timestamp_us`; `ZDA` additionally fills a full calendar date.

### Fix quality

`GGA` carries a numeric quality flag that the parser maps onto `GNSSFixType`:
quality `0` means no fix (the position is not updated); `1` is a standard GNSS
fix; `2` is a differential fix; `4` and `5` are RTK fixed and RTK float
respectively. `GSA` and the valid-flag in `RMC`/`GLL` can also lift a stale
no-fix up to a basic fix. You can ask a `GNSSPosition` whether it has any fix
with `has_fix()` and whether it is centimetre-grade with `is_rtk()`.

## Checksum validation and partial-line handling

Two robustness properties matter most in practice.

**Checksum.** When a line contains a `*`, the parser XORs every byte between the
`$` and the `*`, compares it to the two hex digits that follow, and drops the
whole line on a mismatch. A malformed checksum field — non-hex digits, a missing
digit, or trailing junk after the two digits — is treated as a *bad* sentence,
not as "no checksum present", so corrupted lines cannot sneak through. A line
with no `*` at all is still accepted, which supports receivers and loggers that
omit the field; the field-level validation below is what protects you there.

**Partial lines.** `feed_bytes` is a pump: it does not require a whole sentence
per call. Bytes accumulate in an internal line buffer and a sentence is only
parsed once a CR or LF terminates it. A sentence split across two, three, or
more `feed_bytes` calls reassembles correctly. If a line ever exceeds the byte
cap, or a non-ASCII byte appears mid-line, the parser drops the current line and
keeps discarding until the next terminator — so one corrupt line cannot poison
the line that follows it.

## Doing it with machbus

The whole surface is `nmea::SerialGNSS`. Construct it, subscribe to the events
you care about, and feed it bytes.

```rust
{{#include ../../../examples/serial_gnss.rs:20:46}}
```

Three events fire as sentences land:

- `on_position` — a new or updated position fix is available.
- `on_cog` — a fresh course-over-ground value (radians).
- `on_sog` — a fresh speed-over-ground value (metres per second).

You do not have to use events. At any time you can pull the cached state
directly: `latest_position()` returns the current fix or `None` if no fix has
been seen yet, `latest_satellites_in_view()` returns the most recent `GSV`
count, and `latest_utc_datetime()` returns the most recent `ZDA` date/time.

The example also demonstrates the chunked-feed property by splitting one `GGA`
across three `feed_bytes` calls and confirming the fix still parses:

```rust
{{#include ../../../examples/serial_gnss.rs:63:75}}
```

The serial port itself is yours to open. `SerialGNSSConfig` carries the usual
UART settings (`baud`, `data_bits`, `stop_bits`, `parity`) as plain pass-through
values for whatever backend you use; the parser consumes only bytes and never
touches hardware.

## Events and responsibilities

| Event / accessor | Fires / returns when | Typical action |
| --- | --- | --- |
| `on_position` | A position-bearing sentence updated the fix. | Forward the fix to guidance, logging, or a position PGN. |
| `on_sog` | `RMC` or `VTG` produced a ground speed. | Update a speed display or feed wheel/ground-speed logic. |
| `on_cog` | `RMC` or `VTG` produced a track. | Update heading-derived behaviour. |
| `latest_position()` | Polled; `None` until a first fix. | Read current position without subscribing. |
| `latest_satellites_in_view()` | Polled; `None` until first `GSV`. | Report signal availability. |
| `latest_utc_datetime()` | Polled; `None` until first `ZDA`. | Stamp records with calendar time. |

Your responsibilities: open and read the serial port, hand the bytes to
`feed_bytes`, and decide what a fix is good enough to act on. The parser will
not tell you that a fix is "too old" — that policy is yours, using
`timestamp_us`, `fix_type`, and the DOP fields.

## Edge cases and failures

- **Bad checksum.** The sentence is dropped; no event fires and the cache is
  untouched.
- **Truncated line.** An unterminated line simply stays buffered until a
  terminator arrives; it never parses as a partial sentence.
- **Oversized line.** A line past the byte cap is discarded up to the next
  terminator, and so is its tail — the next line still parses cleanly.
- **Missing or empty fields.** Empty fields are normal. A sentence with too few
  fields, or with a coordinate/time field that fails validation, is rejected and
  the previous good fix survives. Invalid coordinates never overwrite a valid
  cached position.
- **No fix.** A `GGA` with quality `0`, or an `RMC`/`GLL` flagged invalid, marks
  the fix as `NoFix` and does not emit a position. `latest_position()` then
  returns `None`.
- **Mixed talker IDs.** Because the parser keys only on the three-character
  sentence type and ignores the talker ID, a stream that mixes GPS, GLONASS, and
  combined sentences is parsed uniformly. If several systems report into the
  same cache, the fields you read reflect whichever valid sentence arrived last.
- **Non-ASCII noise.** A non-ASCII byte aborts the current line; the parser
  resynchronises at the next terminator.

## Advanced

- **Feeding guidance and TC-GEO.** A parsed `GNSSPosition` carries a
  `concord::Wgs` and convenience conversions (`to_enu`, `to_ned`, `to_ecf`) to
  local frames. That is exactly the position input a geo-referenced task
  controller flow consumes — see
  [TC-GEO prescription](tc-geo-prescription.md) for turning position into
  per-zone setpoints.
- **Combining serial and CAN sources.** Some machines carry a serial receiver
  *and* satellite PGNs on the CAN bus. Keep them as separate inputs and pick a
  policy: prefer one source, or fuse them by timestamp and fix quality. Do not
  blindly let the latest of either overwrite the other; compare `fix_type` and
  DOP first.
- **Rate.** The fix rate is set by the receiver, not the parser; `feed_bytes`
  imposes no rate of its own. Read the serial port often enough that the OS
  buffer never overflows, and let `feed_bytes` absorb bursty reads — it is built
  to take bytes in arbitrary chunk sizes.
- **Polling vs events.** For a tight control loop, polling `latest_position()`
  once per cycle is often simpler than event callbacks; for a reactive pipeline,
  the `on_*` events fan out cleanly. Both read the same cache.

## Validate locally

```sh
make run EXAMPLE=serial_gnss
make test
```

The example parses synthetic `GGA`, `RMC`, and `GSA` sentences, prints the
decoded fix and the course/speed events, and demonstrates split-buffer parsing
across UART-style chunk boundaries. The crate tests cover checksum rejection,
malformed-field handling, oversized and non-ASCII lines, the satellites-in-view
vs satellites-used distinction, and `ZDA` calendar validation.

## What this proves / does not prove

Proves: `machbus` decodes the listed serial sentence kinds into typed fixes with
correct unit conversion, rejects corrupt and malformed input without disturbing
the last good fix, and reassembles sentences across arbitrary byte chunks.

Does not prove: real-receiver behaviour, end-to-end timing on hardware, accuracy
of any given fix, or any conformance/certification claim. `machbus` ships no
NMEA certification. A real deployment still needs the actual receiver, the wiring
and serial configuration it expects, and field validation.

## See also

- [NMEA and GNSS basics](../standards/positioning.md) — the
  conceptual primer on satellite navigation in this stack.
- [NMEA 2000](nmea-2000.md) — the same navigation data carried as binary PGNs on
  CAN instead of text on a serial wire.
- [TC-GEO prescription](tc-geo-prescription.md) — where a parsed position feeds
  geo-referenced task control.
