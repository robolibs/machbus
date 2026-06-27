# vcan SocketCAN address-claim smoke

Requirement: vcan_address_claim
Trace id: vcan_address_claim
Trace path: tests/fixtures/traces/vcan_address_claim.candump

Machbus commit: ed5f76f
Interface: vcan0
Bitrate: n/a for vcan (SocketCAN virtual interface; ISO bitrate not applied)
Capture command: candump -td -L vcan0
Peer/tool: machbus examples/socketcan_address_claim (emitter) and examples/socketcan_capture (listen-only recorder), same repo commit

Behavior proven:

- Running examples/socketcan_address_claim on vcan0 drives the real stack
  through address claim to Claimed at preferred address 0x80.
- A separate listen-only SocketCAN socket observed two extended frames on
  the bus: a PGN Request for Address Claimed (id 18EAFFFE, data 00 EE 00 ..)
  and the machbus Address Claimed response (PGN 0xEE00, id 18EEFF80) whose
  source address byte is 0x80, carrying the 8-byte NAME.
- The reduced trace is replayed and asserted by
  tests/protocol_fixtures.rs::fixture_vcan_address_claim_capture_is_verifiable
  (Address Claimed present, source 0x80, full NAME payload).

Caveats and non-claims:

- This is a Linux vcan loopback smoke, not a physical-bus or peer-ECU
  certification capture. It proves the machbus address-claim emission and
  the capture harness end to end on a virtual interface only.
- can-utils was not installable in this environment, so the reduction was
  performed with the equivalent examples/socketcan_capture recorder, which
  writes byte-identical candump text; the candump command above is the
  canonical reproduction method on a host that has can-utils.
- No independent peer/analyzer cross-check was performed; interoperability
  against third-party ECUs is covered by the still-open physical_* and
  peer_* requirements.
