# Conformance and claim boundary

This page is the repository's wording guardrail. It exists so the rest of the
book can stay readable without accidentally making a bigger promise than the
checked-in evidence supports.

## What we can say

Safe wording is narrow and evidence-based:

- machbus contains ISO 11783, J1939, and NMEA 2000 related codecs and stack
  components;
- listed flows are fixture-tested, virtual-bus-tested, binding-smoked, or
  trace-replayed exactly where the docs say they are;
- local validation is performed with `make verify`;
- SocketCAN and vcan tooling exists for operator-run captures.

## Phrases that must not appear as public claims

The test suite keeps these phrases out of README/package/public reference text
unless they appear here as explicit non-claims:

- AEF certified
- ISOBUS certified
- conformant implementation
- fully conformant
- fully compliant
- production-ready
- production ready
- hardware-tested
- hardware tested
- interoperability tested
- PlugFest validated
- field-proven
- speaks the same wire as a real

Those phrases are dangerous because each one sounds like external evidence or
certification. Local unit tests, virtual-bus tests, vcan smokes, or trace replay
do not create that external evidence by themselves.

## Evidence classes

| Evidence class | Current status | Where to look |
|---|---|---|
| Local build/test gate | Present | `Makefile`, `book/src/reference/validation-history.md` |
| Public claim boundary | Present | this page and `book/src/conformity/` |
| Protocol fixtures | Present for selected flows | `tests/protocol_fixtures.rs`, `tests/fixtures/`, `book/src/reference/assets/protocol_matrix.csv` |
| AgIsoStack/reference-style bytes | Present for selected rows only | `tests/agisostack_compat.rs`, `tests/fixtures/oracle/agisostack_manifest.txt` |
| C ABI behavior | Present for exposed facade calls | `src/ffi.rs`, `include/machbus.h`, `examples/c_abi/` |
| Python behavior | Present for exposed facade calls | `src/python/mod.rs`, `examples/python_binding/regression.py` |
| SocketCAN/vcan tooling | Present | `examples/socketcan_capture.rs`, `examples/candump_replay.rs` |
| Physical-bus reports | Evidence contract exists; no completed reports/traces yet | `tests/fixtures/hardware/capture_requirements.txt`, `tests/fixtures/hardware/capture_reports/` |
| AEF-style external validation | Not present in this checkout | no report checked in |

## Current hardware statement

There is a checked-in physical-bus report directory, but it currently contains
no completed reports/traces yet. The requirement rows are all `missing`, which
is deliberate. It prevents a future reader from mistaking planned capture work
for completed evidence.

A row can move out of `missing` only when it names a reduced trace with
`reduced-hardware` provenance and a schema-complete capture report.

## Explicit non-claims

- No AEF certification is claimed.
- No real machine safety claim is made.
- Nothing in this checkout is currently AEF-tested.
- No official ISO 11783 text is embedded in this repository.
- No broad external peer compatibility statement is made beyond the exact
  local fixtures, virtual-bus tests, checked-in traces, and reports named in
  the docs.
- No binary compatibility is promised for Rust internals; the documented Rust
  API and generated C/Python facades are the intended public surfaces.
