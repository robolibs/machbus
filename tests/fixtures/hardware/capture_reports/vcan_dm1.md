# vcan SocketCAN DM1 smoke

Requirement: vcan_dm1
Trace id: vcan_dm1
Trace path: tests/fixtures/traces/vcan_dm1.candump

Machbus commit: ed5f76f
Interface: vcan0
Bitrate: n/a for vcan (SocketCAN virtual interface; ISO bitrate not applied)
Capture command: candump -td -L vcan0
Peer/tool: machbus examples/socketcan_address_claim (emitter) and examples/socketcan_capture (listen-only recorder), same repo commit

Behavior proven:

- examples/socketcan_address_claim claims address 0x80, raises SPN 100 /
  FMI 1, and broadcasts DM1.
- The listen-only socket observed the machbus DM1 frame (PGN 0xFECA,
  id 18FECA80) from source 0x80 with lamp bytes and a single DTC whose SPN
  low byte is 0x64 (100) and FMI is 1, alongside the Address Claimed
  context frame.
- The reduced trace is asserted by
  tests/protocol_fixtures.rs::fixture_vcan_dm1_capture_is_verifiable
  (DM1 present from 0x80, SPN 100 low byte, FMI 1).

Caveats and non-claims:

- This is a Linux vcan loopback smoke, not a physical-bus or peer-ECU
  certification capture. It proves machbus DM1 emission plus the capture
  harness end to end on a virtual interface only.
- can-utils was not installable here, so the trace was recorded with the
  equivalent examples/socketcan_capture recorder (byte-identical candump
  text) and reduced by filtering the captured file to the Address Claimed
  and DM1 lines; the candump command above is the canonical reproduction.
- Only a single-DTC, single-frame DM1 is proven; multi-DTC TP reassembly
  and independent-peer decoding remain out of scope for this row.
