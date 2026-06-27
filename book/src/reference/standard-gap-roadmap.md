# Standard gap roadmap

This page is the human-readable companion to
[`assets/standard_gap_matrix.csv`](assets/standard_gap_matrix.csv).
Binding exposure decisions are tracked separately in
[`assets/standard_binding_matrix.csv`](assets/standard_binding_matrix.csv).

The matrix is deliberately written in repository-owned language. It names broad
behavior families, source modules, test status, external-trace status, and the
next engineering action. It does not copy licensed standard prose, tables,
examples, diagrams, or generated text extracts.

## How to read the matrix

| Column | Meaning |
| --- | --- |
| `part` | The standard area or evidence family. |
| `area` | A short repo-owned behavior name. |
| `repo_module` | The current code, docs, or evidence surface. |
| `status` | Whether the area is planned, implemented but still needs standard-suite tests, or later complete. |
| `test_status` | Whether existing local tests exist and whether the new standard suite has caught up. |
| `external_trace_status` | Whether independent traces or hardware reports exist. |
| `next_action` | The next non-leaking implementation or evidence step. |

The binding matrix uses the same non-leaking style. It records whether a
standard feature family is currently Rust-only, intentionally internal, or
available through the C/Python facades. A binding row is a maintenance
contract, not a conformance claim: a feature can be exposed through a facade
and still require more standard-suite tests or external trace evidence.

## Current priorities

1. Keep this matrix and the older protocol matrix in sync by hand.
2. Add standard-derived code tests under `tests/standard/`.
3. Promote a row only when implementation, tests, documentation, and evidence
   all support the claim.
4. Do not use a green local test as a replacement for external interoperability
   evidence.
6. Update the binding matrix when a standard feature moves between Rust-only,
   facade-exposed, partial-facade, or intentionally internal.

## Status vocabulary

The first implementation pass uses conservative statuses:

- `planned` means the repo has a roadmap item but not enough checked evidence.
- `implemented-needs-standard-tests` means code and local tests exist, but the
  standard-derived test suite and/or external evidence still needs fuller
  coverage before a completion claim is safe.
- `standard-suite` in `test_status` means at least one repo-owned
  standard-derived test file now covers the part; it is not by itself a
  conformance claim.
- Future `complete` rows must name exact tests and, when the claim requires it,
  reduced traces or hardware reports.

The point of this page is to make the hardening work auditable without turning
the private documents into public documentation.
