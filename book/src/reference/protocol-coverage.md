# Protocol coverage

This page is the map of what machbus exercises today. Read it as an evidence
guide, not as a promise about every device or every corner of ISO 11783,
J1939, or NMEA 2000.

The short version:

- a lot of low-level wire formats have fixture and property coverage;
- the in-process virtual bus covers many multi-node workflows;
- the C and Python bindings are tested against the same virtual-bus behavior;
- real vcan and physical-bus captures are planned, but the required capture
  rows are still open until reports and reduced traces are checked in.

The machine-readable detail lives in
[`assets/protocol_matrix.csv`](assets/protocol_matrix.csv). This prose page is
the human version: it explains how to read that matrix and where to go next.

## Reading the evidence levels

| Level | Meaning in this repository |
|---|---|
| Fixture | A test checks exact bytes, decoded fields, rejected malformed bytes, or a named regression case. |
| Virtual bus | Two or more machbus stacks exchange frames through the in-memory bus. |
| Binding smoke | C or Python calls exercise the same Rust behavior through the public facade. |
| Trace replay | A checked-in `candump`-style file is parsed and summarized by the replay tooling. |
| Hardware evidence | A reduced real vcan or physical-bus trace plus a report is checked in. |

If a row has only fixture or virtual-bus coverage, treat it as local software
evidence. For the hardware-evidence contract, future completed rows must link to a named `reduced-hardware` trace and a capture report before they are used as hardware evidence.

## Areas with strong local coverage

These areas are currently the best exercised parts of the port. The evidence is
still local, but it is broad enough that regressions should be caught quickly.

| Area | What is covered | Main files |
|---|---|---|
| CAN identifiers and PGNs | 29-bit identifier layout, PDU1/PDU2 PGN rules, raw-ID helpers, invalid frame rejection, and driver-frame conversion. | `src/net/identifier.rs`, `src/net/pgn.rs`, `src/net/frame.rs`, `tests/protocol_fixtures.rs`, `tests/standard/iso11783_03_datalink.rs` |
| NAME and address claim | NAME packing, address arbitration, cannot-claim behavior, request-for-address-claim responses, duplicate/self echo cases, and commanded address handling. | `src/net/name.rs`, `src/net/address_claimer.rs`, `src/net/network_manager.rs`, `tests/agisostack_compat.rs`, `tests/standard/iso11783_05_network_management.rs` |
| Transport Protocol and ETP | BAM/RTS/CTS/DT flows, abort paths, receive limits, timing/cadence fixtures, large receive profiles, malformed CM/DT handling, and generated receive streams. | `src/net/tp.rs`, `src/net/etp.rs`, `tests/protocol_fixtures.rs`, `tests/protocol_fixtures.rs` |
| Fast Packet | NMEA-style fast-packet transmit/receive, sequence handling, malformed stream handling, and generated receive streams. | `src/net/fast_packet.rs`, `tests/protocol_fixtures.rs` |
| Diagnostics | DM1/DM2 request behavior, DTC packing, DM3/DM11 clear flows, DM13/DM22/Product/Software/FreezeFrame-style codecs, memory-access helpers, and malformed payload rejection. | `src/j1939/diagnostic.rs`, `src/j1939/dm_memory.rs`, `tests/standard/iso11783_12_diagnostics.rs`, `tests/protocol_fixtures.rs` |
| Utility PGNs | Heartbeat, Maintain Power, Language Command, Shortcut Button, Request2/Transfer, Acknowledgment, Group Function, and Control Function Functionalities. | `src/j1939/`, `src/isobus/`, `tests/protocol_fixtures.rs`, `tests/standard/session_harness.rs` |

## ISOBUS application families

The library also has higher-level session plugins for common ISOBUS roles. In
most cases, the Rust side is richer than the C/Python facade, and the docs
explain the facade separately.

| Family | Current shape | Evidence style |
|---|---|---|
| Virtual Terminal client/server/render runtime | Object-pool upload helpers, status/events, auxiliary capability discovery, visibility helpers, server/client roles, standard-aware retained attribute/value replay including Output Line Change End Point replay, Get Attribute Value responses from current body/retained state with invalid-object/invalid-attribute error bits, Working Set Special Controls AID 1 byte-count reporting, Number Variable value reads, and String Variable fixed-length reads, standard shape-object AID ordering, open Output Ellipse arc/segment/section command emission and framebuffer rasterisation, output-shape line-attribute width-zero no-stroke handling through command emission and framebuffer replay, Input Number/List enabled-bit overlays that preserve real-time-editing bits, and Container-only Hide/Show with Hidden AID 3 synchronization across Hide/Show, Change Attribute, hosted runtime replay, Macro replay, and parent-to-child scene visibility, numeric-target validation for Change Numeric Value, server-side Change Attribute AID admission that rejects unsupported/read-only AIDs, wrong-type reference-valued AIDs, Button Options static latchable-bit mutation, and out-of-range scalar option/format/justification/boolean/shape-angle/state-relative min-max values before retained-state mutation, Graphics Context payload/reference validation before replay retention, F.57 response/error emission with client-side event parsing, plus line-attribute stroke-width and line-art command expansion, Alarm Mask Change Priority metadata replay, bounds-checked Change List Item and Change Polygon Point replay, Soft Key Mask Key/ObjectPointer/ExternalObjectPointer child resolution with NULL-slot reservation, runtime user-layout placement plus host-persistable recall snapshots for Window Mask and Key Group nodes, VT On User-Layout Hide/Show emission for placed nodes, VT Pointing Event emission for non-interactive Data Mask / free-form Window Mask touch areas, checked VT-to-ECU bus-message payload/full-message parsing with VT6 TAN constructors for activation and numeric-value notifications, H.3/H.5/H.7/H.9/H.11/H.13/H.14/H.15/H.16/H.17/H.19/F.57 ECU response/error helpers for Soft Key Activation, Button Activation, Pointing Event, Select Input Object, VT ESC, Numeric Value Change, Change Active Mask, Change Soft Key Mask, String Value Change, and Graphics Context, Lock/Unlock Mask deferred refresh with host-driven timeout expiry, and a hosted render runtime that turns pools into backend-neutral command streams plus a shared backend trait and runtime-aware software framebuffer snapshot/export backend. | Fixture tests, virtual-bus tests, C/Python smoke coverage for client/server, and Rust-only standard-suite/book-ledger coverage for rendering. |
| Task Controller client/server | DDOP/object model helpers, value commands, TC-GEO rate conversion, prescription-rate helpers, structure/localization labels, and direct-dispatch validation. | Fixture tests, virtual-bus tests, C/Python smoke coverage. |
| File Server | Connect, properties/status, directory, open/read/write/close-style workflows. | Virtual-bus tests and C/Python smoke coverage. |
| Section Control | Master/client lifecycle, section routing, ready/playback/ack/completion events. | Virtual-bus tests and C/Python smoke coverage. |
| TIM | Authority and command/status logic with interlock-oriented tests. | Local session tests; independent-peer captures are still required. |
| Tractor/implement/GNSS | TECU, implement status, powertrain, guidance, NMEA 2000, and serial GNSS helpers. | Fixture tests, virtual-bus tests, and binding smoke where exposed. |

For a tutorial-style entry point, start in the `Tutorials` section of the book
instead of this reference page.

## Binding coverage

The Rust API is the implementation surface. The C and Python APIs expose a
stable subset:

- C is checked by `make bind-c-check`, `make c-demo`, and `make c-full-demo`
  (the `examples/c_abi/` demos against the generated `include/machbus.h`).
- Python is checked by `make python-demo`, which runs the basic example,
  regression runner, wheel build, wheel install, and the same regression smoke
  from a disposable environment.

The binding contracts are described in
[`audit/bindings.md`](audit/bindings.md). When a Rust feature is not yet exposed
through C or Python, do not infer a binding promise from the Rust module alone.

## Trace and hardware path

Trace tooling exists now:

- `examples/candump_replay.rs` parses compact and bracketed `candump` text;
- `tests/fixtures/traces/manifest.txt` records provenance for checked-in
  traces;
- `make trace-replay-demo` replays the current trace fixtures;
- the hardware-evidence fixtures under `tests/fixtures/hardware/` and
  `tests/fixtures/traces/` track required future capture rows (maintained by hand).

The hardware-facing documentation is in
[`hardware-evidence.md`](hardware-evidence.md). The current open capture IDs
are:

| ID | Evidence class |
|---|---|
| `vcan_address_claim` | vcan development capture |
| `vcan_dm1` | vcan development capture |
| `physical_address_claim` | isolated physical-bus capture |
| `peer_request_address_claim` | independent-peer capture |
| `tp_bam_transfer` | independent TP observer capture |
| `etp_connection_transfer` | independent ETP observer capture |
| `network_interconnect_router` | multi-segment router capture |
| `diagnostic_request_response` | service-tool style peer capture |
| `vt_object_pool_upload` | independent VT upload capture |
| `vt_object_pool_upload_failure` | independent VT failure-path capture |
| `implement_message_broadcast` | independent implement decoder capture |
| `powertrain_engine_trace` | independent powertrain decoder capture |
| `tractor_ecu_facility_trace` | TECU peer capture |
| `tc_ddop_upload` | independent TC capture |
| `file_server_read_write` | independent File Server peer capture |
| `section_control_lifecycle` | independent Section Control peer capture |
| `tim_authority_interlock` | independent TIM peer capture |
| `nmea2000_gnss_environment_trace` | independent NMEA 2000 observer capture |

The public-data evidence ID is `ddi_public_database_refresh`. It is separate
from the capture list because DDI freshness is proven by source provenance,
refresh metadata, generated-file diffs, and a public-data report rather than by
a CAN trace.

## How to use the CSV matrix

Use `assets/protocol_matrix.csv` when you need exact row-level status. The CSV
is intentionally boring and machine-readable; keep it updated by hand as
coverage changes.

Use this page when you need the story:

1. find the protocol family above;
2. check whether evidence is fixture-only, virtual-bus, binding, trace, or
   hardware evidence;
3. open the named source/test files;
4. add new evidence to the CSV, tests, and docs together.

When in doubt, prefer narrower wording. A single passing byte fixture proves
that byte fixture. It does not prove a whole protocol family.
