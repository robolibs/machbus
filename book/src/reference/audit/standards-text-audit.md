# Audit: review against the standards text

A pass over the implementation with the ISO 11783 series, AEF 023 RIG 2 and the
NMEA 2000 appendices open beside it, checking field orders, ranges, timings,
default priorities and reserved-bit rules against what the documents actually
say.

This is a distinct kind of evidence from the rest of the suite. Unit tests and
fixtures prove the code does what its author intended; trace replay proves it
matches a capture. Neither catches a wire rule that was *reconstructed* rather
than read — and reconstructions are exactly where this pass found problems.

> The standards are **not** redistributed in this repository and never will be.
> This page records what was checked and what was found; reproducing it needs
> your own licensed copies.

## What each document could evidence

The series is not uniform. Three of its parts turn out to be short pointer
documents that define no messages at all, and that shapes what can be checked.

| Document | Substantive for this audit |
| --- | --- |
| ISO 11783-1:2017 | §6.13 safe mode (defers to part 9), §7 electronic database |
| ISO 11783-2:2019 | bit rate and sample point, bus power minima, §9.6 fail-safe |
| ISO 11783-3:2018 | transport timeouts, size limits, Table 8 abort reasons |
| ISO 11783-4:2011 | Table 2 NIU function codes |
| ISO 11783-5:2019 | NAME self-configurable bit and claim behaviour |
| ISO 11783-6:2018 | VT object attribute tables, Annex D queries, Annex J auxiliary |
| ISO 11783-7:2022 | §5.2.4 Table 1 SLOT bands, §5.4 reserved bits, Clause 11 TIM |
| ISO 11783-8:2006 | **3 pages** — §4.2/§4.3 precedence over J1939-71 only |
| ISO 11783-9:2012 | tractor classes, facilities handshake, §4.7 safe mode |
| ISO 11783-10:2015 | process data commands, TC/client status, DDOP, TimeLog |
| ISO 11783-11:2011 | **3 pages** — §4.2 DDI entry shape only |
| ISO 11783-12:2019 | DM1/DM2, diagnostic protocol, B.9 functionalities |
| ISO 11783-13:2022 | error codes, flags, volume and file operations |
| ISO 11783-14:2013 | F.3 SCClientStatus and its timeouts |
| AEF 023 RIG 2 | TIM function messages, SLOTs and facility blocks |
| NMEA 2000 App. A/B | per-PGN priorities, GNSS data dictionary items |

## The limit worth knowing about

**No PGN number in this crate is verifiable from the standards.** That is the
series' own design, stated three times over:

- ISO 11783-1 §7 — "The electronic database with the ISO 11783-1 parameter
  group, address and identity assignments is accessible at: www.isobus.net",
  listing PGNs, industry groups, preferred addresses, NAMEs and manufacturer
  codes as living there.
- ISO 11783-7 §4.2 — the same for the part 7 PGN and SPN assignments.
- ISO 11783-11 §4.1 — the same for the DDIs, maintained by VDMA as the
  ISO-appointed maintenance agency.

So this audit evidences **message definitions** — field order, widths, ranges,
reserved-bit rules, timings, priorities — and not the numbers those messages
travel under. A citation beside a PGN constant names the clause defining the
message, never one stating its value. The same applies to every DDI range in
`ddi_database`: only the five-attribute *shape* mandated by ISO 11783-11 §4.2 is
checkable.

## What was found

Nine defects, each fixed with a test that fails when the fix is reverted.

| Area | Defect | Clause |
| --- | --- | --- |
| AutoDrive | never sent Required Tractor Facilities, so a conforming TECU may never broadcast the Machine Info it refuses to engage without | 11783-9 §4.4.2 |
| AutoDrive | facility *request* set reserved bits to 1, asking for every undefined facility | 11783-7 §5.4, 11783-9 §4.4.2 |
| File Server | rejected volume requests whose reserved bits were set | 11783-13 B.29/B.30, §4.9 |
| Diagnostics | one unknown functionality code discarded the whole message | 11783-12 B.9 |
| Task Controller | every Process Data message sent at priority 6 | 11783-10 B.2 |
| Task Controller | TimeLog encoder omitted five declared position columns | 11783-10 Table 3 |
| Powertrain | Python exposed only the strict speed decoder, which rejects real frames | 11783-8 §4.2 |
| NMEA 2000 | every PGN transmitted at priority 6 | NMEA 2000 App. B.1 |
| NMEA 2000 | a reserved Sequence ID discarded the whole parameter group | NMEA 2000 DD056 |

### The pattern

**Seven of the nine only misbehave against a *conformant peer*.** They are
receive paths that were stricter than the standard allows, or defaults that were
uniform where the standard varies. Testing a stack against itself cannot surface
either: both sides share the same wrong assumption.

The reserved-bit cases share one root. ISO 11783-7 §5.4 says both halves of the
rule, and only the first half had been applied:

> "All undefined and reserved bits shall be transmitted with a value of '1' …
> All undefined bits should be received as 'don't care' (either masked out or
> ignored). This permits them to be defined and used in the future without
> causing any incompatibilities."

That clause also carries an exception which is easy to miss and which the
facility messages fall under — feature-availability messages where "the default
value is zero ('0') for forward compatibility. The value of zero indicates 'not
supported'".

The two priority defects are the same mistake in different subsystems: a
per-message default collapsed into one constant. Worth checking wherever a
plugin passes a literal priority.

## Corrections to citations

Four places named a clause that does not say what was claimed:

- Auxiliary functions were attributed to **ISO 11783-11**, which contains no
  mention of them; they are ISO 11783-6 **Annex J**. (The module body said
  "Annex G", which is "Status Messages" in the 2018 edition.)
- The 100 ms guidance cadence cited ISO 11783-7 §5.2.7.2, which defines the
  *form* of an on-change rate but states no numbers — §5.2.7.1 puts every rate
  in the electronic database. Now cited to AEF 023 §D.7.1, which does state
  "2000 ms periodic, 100 ms on change".
- `Dm5Message` was labelled DM5. J1939-73's DM5 is Diagnostic Readiness 1 on PGN
  65230; this is ISO 11783-12 B.5 "Diagnostic protocol" on PGN 64818.
- Wheel/ground/machine speed were attributed to J1939-71 alone, but ISO 11783-8
  §4.2 gives ISO 11783-7 precedence wherever both define a parameter.

## Judgement calls left as they are

Recorded rather than changed, because the normative text supports current
behaviour and changing it would reject data that works today:

- **DDOP designators** carry two limits: Table A.1's normative range is 128
  *bytes* (enforced), while the description column says 32 *characters*. Annex A
  derives one from the other at 4 bytes/character. A 128-character ASCII
  designator therefore serializes.
- **Element number 4095** is inside B.3.2's stated 0–4095 range but is also how
  B.8.1/B.8.2 encode "element number not available".
- **Auxiliary PGN constants** are evidenced by Annex J for their message
  definitions only; the values are not printed in part 6.

## What this evidence is not

Reading the text is not conformance testing. It cannot show how a specific
tractor behaves, cannot substitute for AEF validation, and does not move any row
in the [conformance boundary](conformance.md). Several areas were confirmed
*correct* against the text and are still untested against real hardware —
`ISO 11783-7 §5.2.4` curvature banding and the AEF 023 TIM SLOTs among them.

Its value is narrower and real: it is the only layer here that can catch a rule
the implementation never got right in the first place.

## See also

- [Evidence model](../../conformity/evidence-model.md) — where this layer sits.
- [Conformance and claim boundary](conformance.md) — what may be claimed.
- [`machbus drive` safety model](../../tutorials/drive-tool.md) — the ISO
  11783-9 §4.7 clauses the operator layer answers to.
- [AutoDrive](../../tutorials/autodrive.md#the-tractor-has-to-advertise-the-facility-first)
  — the facilities handshake this pass added.
