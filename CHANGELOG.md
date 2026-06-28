# Changelog

## [0.1.5] - 2026-06-28

### <!-- 0 -->⛰️  Features

- Terminal virtual terminal :P
- Terminal virtual terminal :P
- Created a simple TOOL
- OPENSOURCEed

## [0.1.4] - 2026-06-23

### <!-- 0 -->⛰️  Features

- Merge NO_STD
- Refactor `Stack` to `Session` in documentation and examples

## [0.1.2] - 2026-06-18

### <!-- 3 -->📚 Documentation

- Update ISOBUS basics for new chapters and details

## [0.1.1] - 2026-06-18

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Add workflow to build and deploy book

## [0.1.0] - 2026-06-18

### <!-- 0 -->⛰️  Features

- Make_public

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Update Codeberg Pages deployment workflow
- Migrate book deployment to Forgejo Actions

## [Unreleased]

### Hardened

- Added a broad local hardening gate through `make verify`, covering default
  and all-feature Rust checks/tests, Clippy, rustdoc, generated C-header drift,
  C demos, Python binding smokes, and candump replay.
- Hardened candump replay to accept both compact `candump -L` and bracketed
  classic candump traces while rejecting DLC mismatches and CAN-FD-looking
  compact tokens, compact overlong payloads, bad hex, and flag-looking IDs
  before fixture promotion.
- Added protocol fixture coverage across J1939 transport, diagnostics,
  engine/powertrain, NMEA 2000 fast packet/GNSS/management, ISOBUS File Server,
  Task Controller, Section Control, TIM, implement controls, NIU/router, and
  stack persona surfaces.
- Hardened C and Python facades with validation for invalid strings, null data,
  out-of-range implement aux-valve indexes, zero-size VT server screens,
  invalid File Server roots/volume labels/preload paths, and invalid Task
  Controller server topology.
- Hardened Rust protocol builders/state accounting for TC process-data
  12-bit element numbers, VT server version advertisement, and File Server
  client/server file-position overflow edges.
- Hardened Task Controller client/server direct routing so wrong-PGN messages,
  NULL/broadcast sources, and handshake responses from non-bound TC sources are
  ignored before callbacks, acknowledgements, client registration, or state
  transitions.
- Hardened NMEA 2000 interface dispatch so NULL/broadcast source addresses are
  ignored before cache mutation or event emission.
- Hardened Task Controller GEO direct GNSS ingress so wrong-PGN payloads and
  NULL/broadcast source addresses are ignored before prescription-position
  cache mutation.
- Hardened PGN-bearing encoders and transport setup so PGN Request,
  Acknowledgment, Group Function, TP, ETP, and Fast Packet reject invalid
  high-bit PGNs before wire-field truncation; Group Function also rejects
  overlong parameter lists.
- Hardened TP, ETP, and Fast Packet endpoint validation so NULL/broadcast
  sources, invalid destinations, and destination-specific TP control frames
  sent to broadcast cannot open or mutate transport reassembly sessions.
- Added allocation-free ETP receive-admission profiling for protocol-maximum
  transfer policy audits, and added a GNSS Satellites in View Fast Packet
  public-PGN fixture stream.
- Added fixture-backed Task Controller DDOP helper expectations for
  two-section sprayer geometry, rate/total DDI extraction, and parent lookup.
- Expanded the remaining local golden-vector gaps with a second TC DDOP
  metered-drill object-pool fixture, J1939 diagnostic request/response
  payload fixtures, and File Server directory/current-directory plus
  relative-open/read workflow fixtures.
- Added a compiled property-style fuzz-smoke integration test for externally
  fed decoders, covering CAN frame conversion, Identifier/NAME/DataSpan,
  TP/ETP/Fast Packet, VT object pools, TC DDOP, File Server codecs, J1939
  diagnostics, NMEA-0183 serial input, and IOP parsing.
- Added an executable oracle-claim manifest for selected AgIsoStack and
  review-vector evidence, with an integration test that rejects stale test
  names, duplicate IDs, malformed rows, and missing fixture artifacts.
- Added an executable trace manifest for all checked-in `candump` fixtures,
  with provenance/coverage/claim rows and a test that rejects unlisted traces
  or stale paths.
- Added an executable protocol-matrix consistency test that checks the matrix
  schema, required public protocol families, row uniqueness, status vocabulary,
  local-complete/external-oracle counts, and oracle/trace manifest links.
- Added a public claim-boundary test for README/package metadata and softened
  the README intro so the project does not imply certification, hardware
  interoperability, or broad real-machine wire parity beyond checked evidence.
- Added an executable hardware-evidence contract for H10 vcan, physical-bus,
  and independent-peer capture flows, requiring future completed evidence rows
  to have both reduced hardware traces and capture reports.
- Expanded selected AgIsoStack++ compatibility coverage with CANMessage
  accessor semantics, LanguageCommandInterface byte packing, SpeedMessages
  payload/identifier examples, Maintain Power byte layout, ISO 11783-11 DDI
  lookup/sentinel examples, Control Function Functionalities PGN 0xFC8E
  leading-`0xFF` / option-count examples, NMEA2000 navigation raw payload
  examples, and DiagnosticProtocol DTC/lamp byte expectations, and tightened
  the oracle manifest so every compatibility test must be listed.
- Hardened Section Control master/client direct status handlers so wrong-PGN
  payloads and impossible NULL/broadcast source addresses are ignored before
  lifecycle mutation or response emission.
- Hardened NIU control/filter storage encoders so invalid filter PGNs and
  over-wide persistent rate limits fail validation, while port-stat counters
  saturate to their 16-bit wire fields instead of wrapping.
- Hardened Request2/Transfer and J1939 DM memory/ECU-identification public
  encoders so invalid high-bit PGNs, overlong Request2 extended IDs,
  over-24-bit DM14/DM15 addresses, invalid DM16 single-frame lengths, and
  malformed ECU-identification text fail before wire-field truncation or
  malformed payload emission.
- Hardened diagnostic Product/Software Identification and FreezeFrame public
  encoders so delimiter/non-printable text and over-wide count fields fail
  before malformed variable diagnostic payloads are emitted.
- Added J1939 DM6/DM12/DM23 DTC-list alias fixtures and explicit
  transport-length partial-DTC rejection coverage.
- Hardened J1939 transmission decoders so ETC1 output-shaft speed/gears,
  Transmission Oil Temperature, and Cruise Control speed fields reject raw
  `not available` sentinel values that their public APIs cannot represent.
- Hardened high-level `IsoNet` PGN boundaries so normal sends, PGN callback
  registration, stack/C/Python subscription, and Fast Packet registration
  reject invalid high-bit PGNs before identifier normalization can truncate
  them into another wire PGN or store an impossible route.
- Hardened core protocol-frame construction so hostile driver DLC values are
  rejected instead of clamped, and fallible frame constructors reject
  over-8-byte lengths/payloads plus invalid high-bit PGNs before silent
  truncation or identifier normalization.
- Hardened NMEA 2000 management public encoders/decoders so Product/Config
  Information text rejects non-printable or over-wide fields instead of
  truncating, and Heartbeat encode rejects unrepresentable intervals or
  sequence counters.
- Added explicit malformed NMEA 2000 management fixtures for non-printable
  Product/Config Information text and truncated Config Information declared
  lengths.
- Added explicit malformed Task Controller peer-control assignment fixtures for
  short, overlong, and wrong-command payloads plus invalid address rejection.
- Hardened VT string-value ingress so client, server, and state-tracker paths
  preserve UTF-8 text and reject malformed UTF-8 instead of byte-widening it
  into corrupted Rust strings.
- Hardened VT update-helper string egress so oversized string updates are
  rejected before they can enter a batch or reach the client serializer; the
  legacy `set_string_value` wrapper remains compatibility-drop behavior.
- Clarified the feature-gated async surface as local-executor/manual-tick only:
  the stack remains single-threaded and `!Send`, and `events_async()` does not
  imply a background runtime.
- Expanded J1939 engine/powertrain golden vectors with raw minimum and upper
  non-sentinel payloads for EngineHours/Revolutions, VehiclePosition, and
  FuelConsumption.
- Hardened the File Server stack client so duplicate/non-disconnected
  `connect_to` requests return `InvalidState` instead of silently succeeding
  without sending a connection handshake, and failed initial sends roll back to
  a retryable disconnected state.
- Added fallible File Server client request-builder APIs (`try_connect_*`,
  `try_open_*`, `try_read_*`, etc.) so raw Rust callers can distinguish
  invalid paths, unknown handles, oversized writes, and disconnected directory
  queries instead of receiving an undifferentiated `None`; the stack now uses
  those fallible paths internally.
- Added SocketCAN/vcan smoke documentation and candump replay tooling; this is
  trace tooling, not an ISOBUS/AEF conformance claim.

### Documentation

- Added validation, protocol coverage, binding-surface, behavior-difference,
  hardware, conformance, and release-checklist documentation.
- Replaced placeholder parity wording with concrete behavior and validation
  notes.

## [0.0.2] - 2026-06-06

### Added

- Added the initial Rust crate, C ABI, Python binding configuration, and
  project README for the machbus ISO 11783 / J1939 / NMEA 2000 stack.
