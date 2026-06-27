# Original hardening plan, rewritten

This chapter preserves the intent of the original long audit plan without
copying it as a wall of raw notes. It is useful when you want to understand why
the hardening work was shaped the way it was.

## Why the plan was needed

The project was translated from a C++ ISOBUS-oriented codebase. That made the
initial Rust tree useful, but not automatically trustworthy. Translation can
preserve names while losing invariants, tests, error handling, byte-exact
behavior, or the distinction between "implemented" and "proven".

The original plan therefore asked for hardening in every direction:

- protocol byte correctness;
- stack state-machine behavior;
- binding safety;
- examples that actually run;
- release gates;
- evidence wording;
- hardware and independent-peer capture path.

## Original risk themes

| Risk | Why it mattered | Current response |
|---|---|---|
| Wire-format drift | ISOBUS/J1939/NMEA behavior depends on exact bytes, sentinels, bit fields, padding, and PGNs. | Fixture tests, property tests, malformed-input corpora, protocol matrix. |
| State-machine gaps | Address claim, TP, VT, TC, diagnostics, and TIM all have ordering and timeout behavior. | Virtual-bus tests and focused stack tests. |
| Binding unsafety | C and Python can accidentally expose invalid lifetimes or untested calls. | Opaque handles, ABI versioning, binding regression tests, C demos. |
| Example rot | Examples often compile only in the happy local environment. | Make targets for C, Python, SocketCAN examples, and trace replay. |
| Evidence overreach | Local tests are not the same as external certification or machine safety evidence. | Originally claim-boundary tests; those governance tests were removed (see `validation-history.md`), so the claim boundary is now maintained by hand as a convention together with this documentation set. |
| Release drift | Version numbers, generated headers, package contents, docs, and fixtures can diverge. | Release checklist plus generated C header drift check; the earlier package-contents gate was removed and package contents are now maintained by hand as a convention. |

## What changed from raw audit notes

The original document mixed observations, to-do items, command transcripts, and
spec reminders. That was useful for active triage, but poor documentation.

The book now separates those concerns:

- concepts and tutorials explain how to use the library;
- `protocol-coverage.md` summarizes current protocol evidence;
- `hardware-evidence.md` explains how to add trace-backed evidence;
- `validation-history.md` records the make gates and latest counts;
- `release.md` explains how to tag responsibly;
- `audit/bindings.md` captures binding ownership and facade rules;
- `audit/conformance.md` captures the public claim boundary.

## What remains open

The important unfinished work is still the same:

1. run real vcan captures and check them in as reduced traces with reports;
2. run isolated physical-bus captures for the required flows;
3. add independent-peer evidence for VT, TC, File Server, Section Control, TIM,
   diagnostics, and transport workflows;
4. keep expanding fixture coverage for malformed and boundary inputs;
5. keep C and Python bindings aligned with tested Rust behavior;
6. keep docs, tests, package metadata, and release notes synchronized.

Use this page as history. Use the other reference pages as the current working
instructions.
