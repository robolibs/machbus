# Validation history

This page is the human log of the hardening gate. It is not a changelog and it
is not a marketing page; it records what the local repository has actually run.

The top-level `Makefile` is the source of truth. When in doubt, inspect the
target body and run the make target rather than copying individual commands.

## Current full gate

Run:

```sh
make verify
```

The current gate runs:

1. `make check`
2. `make test`
3. `make check-all`
4. `make test-all`
5. `make clippy`
6. `make rustdoc`
7. `make bind-c-check`
8. `make c-demo`
9. `make c-full-demo`
10. `make python-demo`
11. `make trace-replay-demo`
12. `make fuzz-smoke`
13. `make wirebit-examples-check`
14. `make standard-suite-check`
15. `make whitespace-check`

Those names are intentionally explicit. They make CI and local logs readable:
generated C header drift, C compile surfaces, Python wheel smoke, trace replay,
fuzz smoke, SocketCAN example typechecking, the standard suite, and whitespace
checks are all visible without reading the entire Rust test log.

> **Gate scope change.** Earlier passes wired several repo/doc-governance
> "tests" — claim-boundary, gap- and protocol-matrix, package-contents, and
> hardware-evidence manifest checks. Those `.rs` files policed documentation
> prose, the README, `Cargo.toml`, and workflow YAML rather than code behavior,
> so they were removed. The gate now focuses on code and the standard suite.
> The gap/protocol matrices, hardware-evidence fixtures, and conformance docs
> remain as maintained-by-hand references, not test-enforced contracts.

## What the gate currently proves

The gate combines these kinds of evidence:

- default and all-feature Rust builds/tests;
- Clippy warnings denied;
- rustdoc warnings denied;
- generated C header drift checked by `make bind-c-check`;
- C examples and C ABI compile surfaces built with warnings denied;
- Python extension build/install smoke plus wheel-install smoke;
- replay of compact, bracketed, malformed, and standard-ID rejection candump
  fixtures;
- `tests/fuzz_targets.rs` run through `make fuzz-smoke` as an arbitrary-input
  decoder smoke;
- `cargo check --features wirebit --examples` run through
  `make wirebit-examples-check`, which keeps SocketCAN examples buildable and
  does not require a live vcan;
- `tests/standard.rs` run through `make standard-suite-check`, which runs the
  standard-derived ISO 11783, AEF TIM, and NMEA 2000 tests;
- `git diff --check` run through `make whitespace-check`.

The test suite now exercises code behavior only. The earlier repo/doc-governance
"tests" — which policed README/`Cargo.toml`/workflow prose, the gap and protocol
matrices, package contents, hardware-evidence manifests, and a private-standard
leak scan — were removed so `.rs` tests cover code, not documentation or repo
layout. The non-disclosure boundary is now a maintained convention, not an
automated scan.

## Focused gates retained

| Gate | Why it exists |
|---|---|
| `make whitespace-check` | Runs `git diff --check` so whitespace damage is caught by make, not only by manual review. |
| `make fuzz-smoke` | Runs `tests/fuzz_targets.rs` so arbitrary-input decoder coverage is visible in logs. |
| `make wirebit-examples-check` | Runs `cargo check --features wirebit --examples`; this keeps SocketCAN helpers compiling and does not require a live vcan. |
| `make standard-suite-check` | Runs `tests/standard.rs` to execute the standard-derived coverage suite. |

## VT object-pool wire-format conformance

- Dropped the non-standard per-object `[len:u16]` prefix; `ObjectPool`
  now serializes/deserializes the ISO 11783-6 wire layout
  (`[id][type][body…]`) with a parse-by-type walker
  (`object_body_total_len`) covering all 48 object types. The same path
  now decodes real `.iop` files and third-party VT-client uploads.
- Made `InputAttributes`, `Macro`, and `StringVariable` self-delimiting
  per the standard (length/`num_bytes` fields), so the prefix-free format
  is unambiguous.
- `net::iop_parser` now delegates to the conformant codec (one source of
  truth); legacy naive per-type lengths removed.
- Evidence: full `cargo test` suite green (incl. an all-48-types
  serialize→deserialize round-trip and regenerated VT object-pool /
  command / working-set-storage fixtures).

## VT renderer runtime

- InputString edits enforce InputAttributes character-set validation.
- VT6 Colour Palette objects override the render palette.
- Macro runtime: event→macro dispatch, command-stream decode, and apply
  (Change Numeric/String Value to the pool; Hide/Show, Enable/Disable via
  runtime overlay; Change Active Mask reported for rebuild).
- Corrected `MacroCommand::get_command_length` to ISO 11783-6 (Virtual Terminal) data
  lengths (fixed commands are 8-byte frames; the legacy table had
  impossible values such as `0xA7 = 9`).
- Evidence: full `cargo test` green; clippy/rustdoc/whitespace clean.

## Earlier hardening milestones

The broad 2026 hardening pass added or tightened coverage around:

- root release-metadata package gates;
- repo-local generated-artifact `.gitignore` policy;
- source-persistence-free proptest/fuzz-smoke configuration;
- fallible ingress paths for Section Control and TC-GEO handlers;
- DDI-aware TC-GEO prescription-rate engineering conversion;
- Process Data Value payload helpers;
- Task Controller client/server direct-dispatch validation;
- zero-capacity/default-capacity event-queue behavior;
- C ABI layout assertions;
- NMEA 2000 management heartbeat/configuration fixtures;
- fallible typed access to plugged subsystems (`with`/`with_mut` returning `Option`);
- 4096-byte ETP receive-profile fixture coverage;
- TP BAM cadence fixture coverage;
- ETP CTS hold/resume and receiver Abort cancellation fixtures;
- malformed TP/ETP CM/DT corpora;
- generated/arbitrary Fast Packet receive streams;
- arbitrary Identifier/PGN/NAME/DataSpan/Message decoder coverage.

Keep future entries short and evidence-oriented. Prefer "target X passed after
change Y" over long copied terminal logs.
