# Diagnostics

When something goes wrong on a machine — a sensor reads out of range, a valve
stops responding, a supply voltage sags — the ECU that noticed needs a way to
tell the rest of the network, and a service technician needs a way to read that
fault back out later. Diagnostics is the shared language for that. This tutorial
explains the diagnostic message family (the "DM" messages), shows how a fault is
shaped on the wire, and walks the `machbus` types that publish and read faults at
both the low (codec) level and through the session facade.

Diagnostics on ISOBUS reuses the J1939 diagnostic layer almost verbatim; the
ISO 11783-12 (Diagnostics services) part adds a few ISOBUS-specific wrinkles such
as a sixth ECU-Identification field and the control-function functionalities
advertisement. `machbus` ships the codecs for both worlds in one place.

## Why this exists

A fault that only one ECU knows about is useless to everyone else. The whole
point of network diagnostics is to make a fault *visible* and *durable*:

- **Visible now.** A failing node broadcasts its currently active faults so a
  dashboard, a logger, or a supervising controller can react in real time —
  light a lamp, slow the machine, or refuse to start a task.
- **Durable for service.** A fault that came and went still matters. Diagnostics
  keeps a *previously active* history so a technician who plugs in an hour later
  can see what happened, how often, and under what conditions.
- **Serviceable.** A service tool needs to clear codes after a repair, read
  identification strings to confirm which ECU and software it is talking to, and
  sometimes read or write raw memory. Diagnostics covers all of that with one
  family of messages.

Without a common diagnostic language, every manufacturer would invent its own,
and a single mixed-vendor implement train would be unserviceable.

## Mental model

Think of each node as keeping two lists of faults plus a lamp panel:

```
                 ┌──────────────── one ECU ─────────────────┐
   sensor says   │                                          │
   "out of range"│   raise ──► ACTIVE list ──► broadcast DM1 │──► bus (every 1s)
        ──────────►          (spn, fmi, count)               │
                 │              │                            │
                 │       clear / repaired                    │
                 │              ▼                            │
                 │          PREVIOUSLY-ACTIVE list ──► DM2    │──► on request
                 │                                          │
                 │   lamp panel: MIL / red-stop / amber / EP │
                 └──────────────────────────────────────────┘

   service tool ──► request DM1/DM2 ──────────────────────► read codes
   service tool ──► DM3 / DM11 / DM22 ────────────────────► clear codes
   service tool ──► DM14 (read/write) ◄── DM15 / DM16 ─────► raw memory
   service tool ──► request ECU Ident ◄── strings ─────────► identify the ECU
```

A node *publishes* faults; a service tool *reads and clears* them. Most read
traffic is either a periodic broadcast (DM1, roughly once a second) or a
request/response pair driven by the PGN Request mechanism. See
[PGN request](request-pgn.md) for that request/response primitive — diagnostics
leans on it heavily.

## Anatomy of a DTC

A **Diagnostic Trouble Code** (DTC) is the unit of fault information. In
`machbus` it is `j1939::Dtc`, a 4-byte field with three meaningful parts:

| Part | Type | Width | Meaning |
| --- | --- | --- | --- |
| SPN — Suspect Parameter Number | `u32` | 19 bits | *What* is faulty: a numeric handle for the parameter or component (engine speed, a specific valve, a supply rail). Values above the 19-bit ceiling are clamped on encode. |
| FMI — Failure Mode Indicator | `Fmi` | 5 bits | *How* it is faulty: above normal, below normal, voltage high/low, mechanical failure, root cause unknown, condition exists, and so on. |
| Occurrence count | `u8` | 7 bits | *How often* it has happened since the code was first set. |

So a DTC reads as "parameter X is failing in manner Y, and it has happened N
times." `machbus` represents the FMI as the `j1939::Fmi` enum (`VoltageLow`,
`MechanicalFail`, `AbnormalRateChange`, `RootCauseUnknown`, `ConditionExists`,
and the rest of the J1939-73 set). Encoding clamps the SPN to its 19-bit range
and masks the occurrence count to 7 bits, so a DTC always round-trips through a
valid wire field:

```rust
{{#include ../../../examples/diagnostic_demo.rs:48:61}}
```

Two helpers matter for fault bookkeeping. `Dtc::matches` compares two DTCs by
`(spn, fmi)` only, ignoring the occurrence count — that is the right notion of
"is this the same fault?" when you decide whether to bump a counter versus add a
new code. Full `PartialEq` compares the count too.

### The lamp panel

A DTC says what is wrong; the **lamp status** says how loud to be about it.
`j1939::DiagnosticLamps` carries four lamps — malfunction (MIL), red-stop,
amber-warning, and engine-protect — each as a `LampStatus` (`Off`, `On`,
`Error`, `NotAvailable`) plus a matching `LampFlash` (`SlowFlash`, `FastFlash`,
`Off`, `NotAvailable`). The same two-byte lamp block rides at the front of every
active/previously-active DTC message, so a reader learns both the codes and how
the operator should be alerted from one frame.

## The DM message family

The diagnostic PGNs are conventionally named DM1, DM2, and so on. `machbus`
gives each a codec type. You do not have to use all of them; pick the ones your
node needs.

| Message | `machbus` type | Role |
| --- | --- | --- |
| DM1 | `DmDtcList` | Active DTCs + lamp panel. Broadcast periodically. |
| DM2 | `DmDtcList` | Previously active DTCs + lamps. Sent on request. |
| DM3 | `DmClearAllRequest` (`Dm3ClearPreviouslyActiveRequest`) | Clear previously active codes. |
| DM11 | `DmClearAllRequest` (`Dm11ClearActiveRequest`) | Clear active codes. |
| DM4 | `Dm4Message` | Driver information (lamps + DTCs, alternate layout). |
| DM6 / DM12 / DM23 | `Dm6Message` / `Dm12Message` / `Dm23Message` (aliases of `DmDtcList`) | Pending, emissions-related, and previously-MIL-off DTC lists. |
| DM7 / DM8 | `Dm7Command` / `Dm8TestResult` | Command a non-continuous monitor test and report its result. |
| DM13 | `Dm13Signals` | Suspend / resume broadcasts network-wide. |
| DM20 | `Dm20Response` | Monitor performance ratios. |
| DM21 | `Dm21Readiness` | Diagnostic readiness counters. |
| DM22 | `Dm22Message` | Clear/reset a *single* DTC, with Ack/Nack. |
| DM25 | `FreezeFrame` / `Dm25Request` | Freeze-frame / expanded snapshot of a DTC. |
| DM5 | `DiagnosticProtocolId` | Which diagnostic protocols the ECU speaks. |
| DM9 / DM10 | `Dm9VehicleIdentificationRequest` / `Dm10VehicleIdentification` | Request / return the VIN. |
| DM14 / DM15 / DM16 | `Dm14Request` / `Dm15Response` / `Dm16Transfer` | Memory access: request, response, data transfer. |
| ECU / Software / Product ID | `EcuIdentification`, `SoftwareIdentification`, `ProductIdentification` | `*`-delimited identification strings. |

### Active and previously active lists (DM1 / DM2)

`DmDtcList` is the workhorse. It holds a `DiagnosticLamps` panel and a
`Vec<Dtc>`. `encode` always produces at least 8 bytes: lamps, then the DTCs (or a
zero placeholder when the list is empty), padded with `0xFF`. `decode` filters
out the all-zero SPN/FMI placeholder so an empty list decodes back to an empty
list. The same type serves DM2 for the previously-active history.

### Clearing and resetting

There are two clearing styles. **Clear-all** (DM3 for previously-active, DM11 for
active) carries no selector — `DmClearAllRequest` is just the reserved all-`0xFF`
payload, and the PGN decides which list is cleared. **Individual clear** (DM22,
`Dm22Message`) names one `(spn, fmi)` and asks for it specifically; the responder
answers with an Ack or a Nack. `Dm22Control` enumerates the request and
Ack/Nack variants for both active and previously-active targets, and
`Dm22NackReason` explains a refusal (`AccessDenied`, `UnknownDtc`,
`DtcNoLongerActive`, `DtcNoLongerPrevious`, `GeneralNack`).

### Freeze-frame and expanded information (DM25)

A freeze-frame captures the state of the machine at the moment a fault latched.
`j1939::FreezeFrame` pairs the `Dtc` with a timestamp and a list of `SpnSnapshot`
values (each an SPN plus its captured value). A service tool asks for one with
`Dm25Request`, naming the `(spn, fmi)` and a `frame_number` (`0` = most recent).

### Memory access (DM14 / DM15 / DM16)

Memory access lets a tool read, write, or erase ECU memory by address — the
foundation of calibration and reflashing. The flow is request/response:

- `Dm14Request` (tool → ECU) names a `Dm14Command` (`Read`, `Write`,
  `StatusRequest`, `Erase`, `BootLoad`, `EdcpGeneration`), a `Dm14PointerType`,
  a 24-bit `address`, a `length`, and a security `key`. The encoder rejects an
  address that will not fit the 24-bit wire field.
- `Dm15Response` (ECU → tool) returns a `Dm15Status` (`Proceed`, `Busy`,
  `Completed`, `Error`, `EdcpFault`), echoes the address and length, and carries
  a `seed` byte for the security handshake.
- `Dm16Transfer` carries the actual bytes. A single frame fits 7 data bytes;
  larger transfers ride the transport protocol underneath.

### Identification strings

Identification messages are `*`-delimited printable-ASCII fields:

- `EcuIdentification` — part number, serial number, location, type,
  manufacturer, and (in the ISO 11783 six-field form) a hardware ID. Encode with
  `encode_j1939` for the five-field form, `encode_iso11783` for six, or `encode`
  to follow whichever the hardware-ID field implies.
- `SoftwareIdentification` — one or more version strings.
- `ProductIdentification` — make, model, serial number.
- `Dm10VehicleIdentification` — the VIN, returned in response to a
  `Dm9VehicleIdentificationRequest`.

All of these reject embedded `*` and non-printable bytes on encode, so a
malformed string never silently corrupts the field boundaries.

## How a node publishes faults; how a tool reads them

The two roles use the same messages from opposite ends.

**A node publishes** by keeping an active list and broadcasting DM1. The broadcast
is periodic (about once per second) and also sent on demand when a tool requests
DM1 with a PGN Request. When a fault clears, the node moves the DTC into its
previously-active list, where it stays available for DM2.

**A service tool reads** by either listening for the periodic DM1 broadcast or by
sending a PGN Request for DM1 or DM2 and decoding the response. It clears codes
by sending DM3 / DM11 (clear-all) or DM22 (one code), and identifies the ECU by
requesting ECU Identification.

## The functionality advertisement

ISO 11783-12 also defines a **control function functionalities** message
(PGN `0xFC8E`): a node advertises which protocol roles it implements so peers can
discover capabilities without trial and error. In `machbus` this is
`isobus::functionalities::Functionalities`. You declare a set of `Functionality`
values — `UniversalTerminalServer`, `TaskControllerBasicClient`, `FileServer`,
`TractorImplementManagementServer`, and so on — each with a generation and an
options bitfield, then `serialize` it into the PGN payload. Fluent helpers make
the common cases short:

```rust
// Illustrative shape — advertise a UT-server + TC-basic-client node.
let functionalities = Functionalities::new()
    .with_ut_server(4)
    .with_tc_basic_client(4);
```

`MinimumControlFunction` is always present by default, since every ISO 11783
device supports it. The model decodes strictly: it rejects unknown functionality
codes, duplicates, wrong option-block lengths, and trailing junk, so a malformed
advertisement is never accepted as valid.

## Doing it with machbus

Pick the layer that suits you. For applications, use the session facade; drop to
the codec layer when you own every byte.

### The session facade (recommended)

Plug the [`Diagnostics`](../guide/session-facade.md) plugin into a `Session`. It
owns the active/previous lists, the periodic DM1 broadcast, and request handling;
you raise faults through fine control and read inbound diagnostics off the event
stream. `examples/session_minimal.rs` shows exactly this — build with the plugin,
raise a DTC, and watch a peer receive the DM1:

```rust
{{#include ../../../examples/session_minimal.rs:build}}
```

```rust
{{#include ../../../examples/session_minimal.rs:finecontrol}}
```

The DM1 a peer sends back arrives as `Event::Diag(DiagEvent::Dm1Received { .. })`
on `driver.poll()` (or filtered via `controls.drain::<DiagEvent>()`). For the
service-tool messages, plug `DmMemory` (DM14/15/16 + ECU/Software/Product
identification) and `ControlFunctionalities` (the `0xFC8E` advertisement)
alongside it.

### The codec layer (`j1939::diagnostic`)

Every type above is a pure encoder/decoder: build the struct, call `encode`, put
the bytes on the wire; or take bytes off the wire and `decode`. There is no state
machine, no timer, no list management — you compose the codecs into your own
logic. The example builds a DM1 with two DTCs, encodes it, and decodes it back:

```rust
{{#include ../../../examples/diagnostic_demo.rs:10:46}}
```

This layer is right for tests, for embedded loops where you own every byte, and
for any node that wants full control over which messages it speaks.

### More fine control through the plugins

The `Diagnostics`, `DmMemory`, and `ControlFunctionalities` plugins offer the
same bookkeeping the codec layer omits — active/previous lists, the periodic DM1
broadcast, request handling — without writing any of it yourself. Plug each piece
on the builder:

- `Diagnostics::every(ms)` turns on the diagnostics subsystem with a broadcast
  interval. Through fine control you reach a handle with `raise(spn, fmi)`,
  `clear(spn, fmi)`, `set_lamps(..)`, `broadcast_dm1()`, `active()`, `previous()`,
  and senders for DM7/DM8/DM13/DM22. `raise` is idempotent by `(spn, fmi)`; the
  next poll (or scheduled broadcast) puts the fault on the wire. The plugin
  answers inbound PGN Requests for DM1 and DM2, honors DM3/DM11 clear-all
  requests, and processes DM22 individual clears with an Ack/Nack — all without
  extra code from you.
- `DmMemory` handles DM14/DM15/DM16 plus ECU/Software/Product identification,
  with `send_dm14`, `send_dm15`, `send_dm16`, `request_ecu_identification`, and
  `send_ecu_identification`. When you supply an `EcuIdentification`, the session
  answers PGN Requests for it automatically.
- `ControlFunctionalities` installs the PGN `0xFC8E` responder; you adjust the
  advertised set later through fine control, and subsequent requests reflect the
  change.

A minimal diagnostics-enabled flow looks like this:

```rust
// Illustrative shape, not a compiled call.
let (ctrl, mut driver) = Session::builder(my_name, 0x80)
    .plug(Diagnostics::every(1000))
    .spawn(EndpointTransport::new(0, endpoint))?;
ctrl.start()?;

while !ctrl.is_claimed() {
    driver.poll()?;
}
ctrl.with_mut::<Diagnostics, _>(|d| d.raise(520, Fmi::VoltageLow)); // active, goes out on DM1
// ... later, after the repair ...
ctrl.with_mut::<Diagnostics, _>(|d| d.clear(520, Fmi::VoltageLow)); // moves it to previously-active
```

## Events and responsibilities

When the diagnostics plugins are enabled, inbound diagnostic traffic surfaces as
events you drain and act on.

| Event (`DiagEvent` via `ctrl.drain::<DiagEvent>()` or `driver.poll()`) | Meaning | Typical action |
| --- | --- | --- |
| `Dm1Received { source, active, lamps }` | A peer broadcast its active faults. | Log / display; react to the peer's lamps. |
| `Raised(dtc)` | You added a fault locally. | Update your own UI / logging. |
| `Cleared(dtc)` | A fault moved to previously-active (locally or by a tool). | Confirm the repair / reset state. |
| `Dm7Command` / `Dm8Result` | A peer ran a non-continuous monitor test. | Run the test / record the outcome. |
| `Dm13Signals` | A peer asked to suspend or resume broadcasts. | The plugin already gates DM1; observe if needed. |
| `Dm22Message` | An individual clear request/response. | Usually handled for you; observe for auditing. |

Memory-access traffic surfaces on the same event stream as `Dm14Request`,
`Dm15Response`, `Dm16Transfer`, and `EcuIdentification` events.

The one rule that mirrors every other ISOBUS workflow: **do not broadcast before
you are claimed.** The DM1 broadcaster checks the claim state and stays silent
until the node owns an address — see [Address claim](address-claim.md).

## Edge cases and failures

- **No active fault.** An empty `DmDtcList` still encodes to a valid 8-byte DM1
  with a zero placeholder; a healthy node broadcasts "nothing wrong," which is
  itself useful information.
- **Lamp logic.** Lamps and DTCs are independent fields. A node can set a lamp
  with no matching code, or carry codes with all lamps off. Decide your lamp
  policy deliberately; do not assume a reader infers one from the other.
- **Memory access denied.** A `Dm14Request` does not guarantee a `Proceed`. A
  responder may answer `Busy`, `Error`, or refuse via the seed/key handshake.
  Treat the `Dm15Status` as authoritative and never assume a write landed.
- **Large fault lists.** A long DTC list or a multi-field identification string
  will not fit one CAN frame. Those payloads ride the transport protocol; see
  [Transport protocol](../standards/iso11783-datalink-transport.md). DM16 transfers over 7 bytes do
  the same.
- **Suspended broadcasts.** A received DM13 suspend command stops the periodic
  DM1 until it resumes or its timer expires. While suspended, the plugin's
  `dm1_suspended()` (via `ctrl.with_mut::<Diagnostics, _>`) is true and no
  periodic DM1 goes out.
- **Malformed input.** Every decoder is strict: wrong length, reserved bits set,
  or a bad placeholder returns `None` (or an `Err`) rather than a half-parsed
  value. Check the result; do not unwrap blindly on untrusted bytes.

## Advanced

- **Service-tool scenarios.** To act as a tool rather than a faulting node,
  request DM1/DM2 with a PGN Request and decode the responses, send DM22 to
  clear a single code and inspect the returned Ack/Nack, and request ECU
  Identification to confirm which unit you are talking to before writing memory.
- **Occurrence counting.** The 7-bit occurrence count is yours to manage at the
  codec layer. Use `Dtc::matches` to recognize a recurring `(spn, fmi)` and bump
  the count instead of pushing a duplicate. The plugin's `raise` is idempotent by
  `(spn, fmi)` and does not auto-increment, so counting policy stays explicit.
- **Persistence.** The `Diagnostics` plugin keeps active and previously-active
  lists in memory for the life of the process. If you need codes to survive a
  power cycle, you own that: snapshot the lists and restore them on the next boot.
  The codec layer gives you the encode/decode primitives to store them however
  you like.
- **Codec vs the session facade.** Reach for the codec layer when you need full
  control or are writing tests; reach for the `Diagnostics` plugin when you want
  the active/previous bookkeeping, the periodic broadcast, and request handling
  done for you.

## Validate locally

```sh
make run EXAMPLE=diagnostic_demo
make test
```

The example builds a DM1 with two DTCs, round-trips it through encode/decode, and
asserts a single DTC field round-trips byte-exact. The test suite exercises every
DM codec — round-trips, SPN clamping, lamp packing, and the strict-decode
rejections described above.

## What this proves / does not prove

Proves: the DTC field layout, the DM message codecs, the identification strings,
the memory-access request/response shape, and the functionality advertisement all
encode and decode correctly in software, and the diagnostics plugins manage the
active/previous lists and periodic broadcast as described.

Does not prove: real-hardware timing, interoperability with a specific
third-party ECU or service tool, or any conformance/certification claim. A real
deployment still needs official standards, real hardware, and interoperability
evidence.

## See also

- [Diagnostics basics](../standards/iso11783-diagnostics.md) — the conceptual
  primer for DTCs and the DM family.
- [PGN request](request-pgn.md) — the request/response primitive most diagnostic
  reads depend on.
- [Transport protocol](../standards/iso11783-datalink-transport.md) — how multi-frame DTC lists and
  identification strings move.
