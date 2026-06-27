# Protocol fixtures

Fixtures are grouped by protocol family and are intentionally small. They are
used by `tests/protocol_fixtures.rs` to keep golden bytes outside test code.

- `agisostack/` — values mirrored from AgIsoStack++ compatibility tests.
- `j1939/` — J1939 and ISO 11783 lower-layer payload fixtures, including
  TP/ETP control, data-transfer, malformed-frame, and abort payloads.
- `isobus/` — ISO 11783 application-layer payload fixtures, including TC
  process-data, object-pool, DDOP, peer-control, TIM, SC, FS, VT render
  command traces, VT external-evidence requirements/reports, and implement
  control/status corpora.
- `nmea/` — NMEA 2000/NMEA 0183 payload or frame fixtures, including
  malformed Fast Packet receive streams.
- `traces/` — text traces such as candump captures.

`.bin` files are raw payload/frame bytes. `.hex` files are ASCII hex payloads
for fixtures that need to stay reviewable in patches.
