# Hardware, vcan, and trace evidence

This chapter is the operator guide for moving from local software tests toward
real bus evidence. It explains what to run, where to record the result, and how
the repository prevents accidental overclaiming.

The important rule is simple: a capture is not evidence until the reduced trace,
manifest row, test or replay path, and Markdown report are all checked in.

## Where the evidence contract lives

The executable contract is split across four places:

- `tests/fixtures/hardware/capture_requirements.txt`
- `tests/fixtures/hardware/capture_playbook.txt`
- `tests/fixtures/hardware/can_adapter_matrix.txt`
- `tests/fixtures/evidence/gap_external_evidence_map.txt`
- `tests/fixtures/evidence/public_data_requirements.txt`
- `tests/fixtures/hardware/capture_reports/`
- `tests/fixtures/traces/manifest.txt`

The hardware-evidence fixtures (`tests/fixtures/hardware/`,
`tests/fixtures/traces/`) are the manual contract: every required flow should
have a concrete playbook row, completed rows should point at a `reduced-hardware`
trace, and reports should include the required fields. Maintain these by hand as
real captures are added.

The gap map is stable: it ties each broad matrix row listed in
`tests/fixtures/isobus/iop_parser.hex` to concrete evidence
requirements. Keep a gap-map row even after evidence is collected. Promotion
from `external_trace_status=missing` requires every referenced hardware or
public-data requirement to be complete, with reports and traces where the row
requires them. Most rows map to hardware or vcan capture requirements. The DDI
row is different: it requires public-data refresh evidence, because the safe
proof is source provenance and refresh metadata rather than a CAN trace.

## Adapter matrix

The adapter matrix is a small evidence checklist for the interface class used
to collect a trace. It is not a list of certified devices. It records whether a
row is a virtual `vcan` setup, a native SocketCAN `can` adapter, or an `slcan`
bridge, and what must be written down before the trace can be used.

The key boundary is:

- `vcan` is useful development and replay evidence, but it is
  **not physical timing proof**.
- physical adapter rows must be configured at `250000` bit/s, run on an
  isolated bench with termination checked, and produce a reduced trace plus a
  capture report before they count as repository evidence.

The standard-suite test for ISO 11783-2 checks this matrix so adapter evidence
cannot drift away from the capture requirements.

## Start with Linux vcan

For development, use a disposable virtual CAN interface:

```sh
sudo modprobe vcan
sudo ip link add dev vcan0 type vcan
sudo ip link set vcan0 up
```

Record with the same timestamped format used by the playbook:

```sh
candump -td -L vcan0
```

In another terminal, run the monitor:

```sh
MACHBUS_SOCKETCAN_IFACE=vcan0 \
MACHBUS_SOCKETCAN_MONITOR_SECONDS=10 \
  cargo run --features wirebit --example socketcan_capture
```

In a third terminal, emit the address-claim and DM1 smoke frames:

```sh
MACHBUS_SOCKETCAN_IFACE=vcan0 \
  cargo run --features wirebit --example candump_replay -- <trace>
```

Expected local observation:

- `candump` shows an extended Address Claimed frame;
- the monitor prints an Address Claimed summary;
- the example later emits a DM1 frame for SPN `100` / FMI `1`.

This is useful development feedback. It becomes repository evidence only after
you reduce the trace, list it in `tests/fixtures/traces/manifest.txt`, add a
report under `tests/fixtures/hardware/capture_reports/`, and update the matching
row in `tests/fixtures/hardware/capture_requirements.txt`.

## Required capture flows

The current required IDs are listed below. Keep these IDs stable because the
docs, playbook, tests, and future reports join on them.

| ID | What the capture should prove |
|---|---|
| `vcan_address_claim` | machbus emits Address Claimed on a Linux vcan SocketCAN interface. |
| `vcan_dm1` | machbus emits the DM1 smoke frame after the example raises SPN `100` / FMI `1`. |
| `physical_address_claim` | machbus emits Address Claimed on an isolated physical 250 kbit/s CAN bus. |
| `peer_request_address_claim` | an independent peer sends Request Address Claimed and machbus responds. |
| `tp_bam_transfer` | an external observer sees at least one multi-packet TP/BAM transfer. |
| `etp_connection_transfer` | RTS/CTS/DPO/DT/EOMA traffic is captured for an ETP-sized transfer. |
| `network_interconnect_router` | translated NIU/router traffic is captured across two observed bus segments. |
| `diagnostic_request_response` | an external peer participates in a diagnostic request/response or clear workflow. |
| `vt_object_pool_upload` | object-pool upload traffic is captured against an independent VT or reference tool. |
| `vt_object_pool_upload_failure` | a failed VT replacement upload is captured without accepting a stale active pool as the new upload. |
| `implement_message_broadcast` | selected implement/tractor message PGNs are decoded by an independent observer. |
| `powertrain_engine_trace` | selected EEC/TSC/transmission PGNs are decoded by an independent observer. |
| `tractor_ecu_facility_trace` | tractor facility and maintain-power traffic is captured with source-scoped peer observations. |
| `tc_ddop_upload` | DDOP upload or activation traffic is captured against an independent TC or reference tool. |
| `file_server_read_write` | connect/open/read/write/close style File Server traffic is captured with an independent peer. |
| `section_control_lifecycle` | Ready/PlayBack/pause/resume/abort style Section Control traffic is captured with an independent peer. |
| `tim_authority_interlock` | TIM authority grant/revoke and blocked-command behavior is captured with an independent peer. |
| `nmea2000_gnss_environment_trace` | selected NMEA 2000 GNSS/navigation/environment PGNs are decoded by an independent observer. |

## Promoting a capture

Use this checklist every time a run looks worth keeping:

1. capture with `candump -td -L`;
2. trim the file to the smallest reproducible sequence;
3. store the reduced trace under `tests/fixtures/traces/`;
4. add a `tests/fixtures/traces/manifest.txt` row with provenance
   `reduced-hardware`;
5. add or update a replay/test target that states what the trace proves;
6. write a report under `tests/fixtures/hardware/capture_reports/`;
7. update `tests/fixtures/hardware/capture_requirements.txt` from `missing` to
   `complete` and point it at the trace ID and report path;
8. update `book/src/reference/protocol-coverage.md` if the capture changes the
   human evidence story;
9. run `make verify`.

Reports must include the requirement ID, trace ID/path, machbus commit,
interface, bitrate, exact capture command, peer tool/device, exact behavior
proven, and caveats.

## Replaying checked-in traces

For local trace parsing, use:

```sh
make trace-replay-demo
```

For one capture:

```sh
cargo run --example candump_replay -- tests/fixtures/traces/time_date_agisostack.candump
```

The replay helper accepts compact `candump -L` lines and bracketed classic
formats. It intentionally rejects malformed lines before building driver frames
and rejects standard 11-bit IDs instead of silently treating them as J1939 or
ISOBUS traffic.

## Physical-bus notes

For an isolated physical CAN setup, configure the adapter for the bus speed the
test requires, normally 250 kbit/s for ISOBUS/J1939 work in this repository.
Use an analyzer or independent tool when possible, then record both the
machbus-side command and the external observation in the capture report.

Do not connect an experimental stack to a machine network where it can affect
motion, hydraulics, steering, PTO, implement sections, or other safety-relevant
behavior. Use an isolated bench, simulator, or reference peer first.
