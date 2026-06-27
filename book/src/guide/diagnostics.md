# 6. Talking diagnostics

> **Anchor example:** `examples/diagnostic_demo.rs` — run it any time with
> `cargo run --example diagnostic_demo`.

In [chapter 5](transport.md) we moved a payload too big for one frame. Now we
make the node say something a human cares about: **what is wrong with it.**

Picture a service technician plugging a laptop into a machine that quit in the
field. The first two questions are always "what failed?" and "which box do I
replace?" Diagnostics answers both. A faulting ECU broadcasts the codes that are
*active right now*, keeps a history of ones that *were* active so a tool that
arrives late still sees them, and exposes an identity so the tool knows exactly
which unit and software it is talking to.

By the end of this chapter you will have built a fault code by hand, packed it
into the active-fault message, read it back out, and you will know how the
previously-active list, clearing, and identity fit around it.

## What we are building

We work at the **codec layer** here — pure build-the-struct, `encode`, `decode`.
No bus, no claim loop yet; just the bytes a diagnostic message is made of, so the
anatomy is unmistakable. The example:

1. builds a DTC (a single fault) from its three parts,
2. packs two of them into a DM1 (the "active faults" message) with a lamp lit,
3. encodes that to bytes and decodes it straight back, and
4. round-trips a lone DTC field byte-for-byte.

Then we look at how the previously-active list, clearing, and identity ride the
exact same types once you wrap a session around them.

## Step 1 — the imports

```rust
{{#include ../../../examples/diagnostic_demo.rs:4:4}}
```

Five names, one module. `Dtc` is a single fault. `Fmi` says *how* it failed.
`DmDtcList` is the message that carries a list of faults plus a lamp panel
(`DiagnosticLamps`), and `LampStatus` is the state of one lamp. Everything lives
under `machbus::j1939` because ISOBUS diagnostics is the J1939 diagnostic layer
with a few extra fields — see
[Diagnostics basics](../standards/iso11783-diagnostics.md).

## Step 2 — anatomy of a DTC

A **Diagnostic Trouble Code** is the unit of fault information, and it has
exactly three meaningful parts:

| Part | Field | Means |
| --- | --- | --- |
| SPN — Suspect Parameter Number | `spn: u32` | *What* is faulty (a numeric handle for the parameter or component). 19 bits on the wire. |
| FMI — Failure Mode Indicator | `fmi: Fmi` | *How* it is faulty (`VoltageLow`, `MechanicalFail`, `AbnormalRateChange`, …). 5 bits. |
| Occurrence count | `occurrence_count: u8` | *How often* it has happened. 7 bits. |

Read together: "parameter X is failing in manner Y, and it has happened N
times." Build one and watch it survive a round-trip through its 4-byte wire form:

```rust
{{#include ../../../examples/diagnostic_demo.rs:48:61}}
```

The SPN here (`0x1_2345`) is wide on purpose: encoding clamps it to 19 bits and
masks the count to 7 bits, so a `Dtc` always produces a *valid* wire field rather
than corrupting the bytes around it. The `assert_eq!` proves the value you put in
is the value you get back.

## Step 3 — pack faults into a DM1

DM1 is the **active diagnostics** message: the faults a node has *right now*. In
`machbus` it is `DmDtcList` — a lamp panel plus a `Vec<Dtc>`. We build one with
the amber warning lamp on and two faults in the list:

```rust
{{#include ../../../examples/diagnostic_demo.rs:9:32}}
```

The lamp panel and the DTC list are **independent fields**. A DTC says what is
wrong; the lamp says how loudly to alert the operator. We lit `amber_warning` and
left the malfunction, red-stop, and engine-protect lamps at their default. The
same two-byte lamp block rides at the front of every active/previously-active
message, so a reader learns the codes *and* the alert level from one frame.

## Step 4 — encode, send, decode

On a real bus you would put `dm1.encode()` on the wire and the receiver would
call `DmDtcList::decode(..)`. Here we do both ends in one breath so you can see
the bytes survive the trip:

```rust
{{#include ../../../examples/diagnostic_demo.rs:34:46}}
```

`encode` always produces at least 8 bytes (lamps, then the DTCs), and `decode`
returns the faults back as a `Vec<Dtc>`.

## Step 5 — run it

```sh
cargo run --example diagnostic_demo
```

Expected output:

```text
=== Diagnostic Demo ===
[DM1] 2 active DTCs, amber=On
[encode/decode] 10 bytes round-trip → 2 DTCs
  SPN=0x00208 FMI=VoltageLow OC=0
  SPN=0x000BE FMI=AbnormalRateChange OC=7

[DTC] 4-byte field: SPN=0x12345 → wire [45, 23, 27, 0C] → SPN=0x12345
✓ DTC round-trips byte-exact
```

## What just happened

```
  Dtc{spn:520,  fmi:VoltageLow,         oc:0}  ─┐
  Dtc{spn:190,  fmi:AbnormalRateChange, oc:7}  ─┤
  lamps{ amber: On }                            ├─► DmDtcList.encode() ─► 10 bytes
                                                ─┘                          │
                                                                           ▼
        DmDtcList.decode(bytes)  ◄────────────────────────  back to 2 DTCs + lamps
```

Two faults plus a two-byte lamp block encoded to 10 bytes, and decoding gave
back the same two faults. SPN `520` prints as `0x00208`, SPN `190` as `0x000BE`
— that is just hex, not a different value. The standalone `Dtc` proved its 4-byte
field round-trips byte-exact (`45 23 27 0C`), which is the guarantee everything
above is built on.

## Active vs previously-active

DM1 is only half of the read story. When a fault clears, it does not vanish — it
moves to the **previously-active** list, which is published as DM2. DM2 uses the
*same* `DmDtcList` type. The split is what lets a late-arriving service tool
reconstruct what happened:

```
fault raised  ──►  ACTIVE list   ──► DM1  (broadcast, ~once a second)
                       │
                  fault clears / repaired
                       ▼
                  PREVIOUSLY-ACTIVE list ──► DM2  (sent on request)
```

A healthy node still broadcasts DM1 — with an **empty** list. "Nothing wrong" is
a real, useful state, not the absence of a message.

## Clearing and identity (the session surface)

The codec layer you just used has no list management on purpose. Once you wrap a
session around it — plug in the `Diagnostics` plugin — the active/previous
bookkeeping, the periodic DM1 broadcast, and request handling come for free. The
shapes below are *illustrative* — for the verified behavior and the full plugin
API, follow the [Diagnostics tutorial](../tutorials/diagnostics.md).

```rust
// Illustrative shape, not a compiled call — see the tutorial.
ctrl.with_mut::<Diagnostics, _>(|diag| {
    diag.raise(Dtc { spn: 520, fmi: Fmi::VoltageLow, occurrence_count: 1 });
}); // adds an active fault → DM1
// ... after the repair ...
ctrl.with_mut::<Diagnostics, _>(|diag| {
    diag.clear();
}); // clears the active list
```

**Clearing** on the `Diagnostics` plugin is *clear-all*: `clear()` takes no
arguments and empties the active DTC list in one call. (The DM protocol itself
also defines a service-tool *single* clear that names one `(spn, fmi)` and gets an
Ack or Nack back; that targeted form is not exposed on this plugin.)

**Identity** answers the second technician question — *which box?* A node can be
asked for its identification strings (part number, serial, manufacturer,
software version, and so on). The tool reads those to confirm exactly which ECU
and firmware it is about to clear codes on or reflash. The codecs for those
strings live alongside the DM messages; the tutorial covers them in full.

## A note on size

Two faults fit in one CAN frame. A long fault list, or a multi-field identity
string, will not — those payloads ride the transport protocol you met in
[chapter 5](transport.md). You do not change how you build the message; the lower
layer fragments and reassembles it for you.

## Things that trip people up

- **No-fault is a valid state.** An empty `DmDtcList` still encodes to a proper
  DM1. Do not treat "zero DTCs" as an error or a reason to stay silent.
- **Clearing is a *move*, not a delete.** A cleared active code lands in the
  previously-active list. A technician an hour later still sees it on DM2. That
  is the whole point — do not expect it to disappear.
- **Occurrence counting is yours to manage** at this layer. The count does not
  auto-increment. When the *same* `(spn, fmi)` recurs, bump the count instead of
  pushing a duplicate (use `Dtc::matches`, which compares by `(spn, fmi)` and
  ignores the count). The `Diagnostics` plugin's `raise` is idempotent by
  `(spn, fmi)`, so the counting policy stays explicitly yours.
- **Lamps and codes are independent.** A lamp on with no code, or codes with all
  lamps off, are both legal. Decide your lamp policy deliberately; a reader will
  not infer one from the other.
- **Decoders are strict.** A wrong length or a bad placeholder returns nothing
  rather than a half-parsed value. Check the result; do not unwrap blindly on
  bytes from the wire.

## Validate locally

```sh
cargo run --example diagnostic_demo
make test
```

The example builds a DM1 with two DTCs, round-trips it through encode/decode, and
asserts a single DTC field round-trips byte-exact. The test suite exercises every
DM codec — round-trips, SPN clamping, lamp packing, and the strict-decode
rejections.

## What this proves / does not prove

Proves: you can build a DTC from its three parts, pack faults into the active
message, and round-trip them through `encode`/`decode` in software — and you know
where the previously-active list, clearing, and identity fit.

Does not prove: real-hardware timing, interoperability with a specific
third-party ECU or service tool, or any conformance/certification claim. The same
caveats from the earlier chapters apply.

## Next

→ [7. Your first Virtual Terminal client](virtual-terminal.md) — put a screen on
the operator's terminal.

## See also

- [Diagnostics tutorial](../tutorials/diagnostics.md) — the full DM family,
  the session diagnostics surface, clearing, memory access, and identity strings.
- [Diagnostics basics](../standards/iso11783-diagnostics.md) — the conceptual
  primer for DTCs and the DM messages.
