# Hardening plan narrative

This page is the human-readable version of the hardening plan. The old audit
notes were useful while the port was being brought under control, but they were
too raw for the book. This chapter keeps the same intent and organizes it as a
maintainer story.

## North star

machbus should be boring to maintain:

- the protocol bytes are checked by fixtures and property tests;
- stack behavior is exercised through the virtual bus before it reaches a
  binding;
- C and Python expose stable facades, not Rust internals;
- every release command is a make target;
- hardware evidence is added only with trace provenance and reports;
- public docs say exactly what has been proven and no more.

## Phase 1: make the repo self-checking

This phase is mostly complete. The repository now has named gates for the
things that previously lived in informal notes:

- formatting and whitespace;
- default and all-feature Rust builds;
- Clippy and rustdoc warnings;
- generated C header drift;
- C demos and C ABI compile surfaces;
- Python develop and wheel-install smokes;
- trace replay;
- fuzz smoke;
- SocketCAN example typechecking.

Earlier passes also wired repo/doc-governance "tests" for the public
claim-boundary, package contents, and hardware-evidence manifest. Those policed
documentation prose and repo layout rather than code behavior and have since
been removed (see `validation-history.md`); the claim boundary, package
contents, and hardware-evidence manifest are now maintained by hand as
conventions, no longer test-enforced.

The rule for future work is: if a release checklist item can be checked by a
machine, give it a make target.

## Phase 2: protect the wire

The port came from C++ concepts, but Rust needs its own safety rails. Keep
adding tests around:

- exact byte fixtures for every encoder/decoder;
- short, overlong, reserved-bit, sentinel, and padding rejection;
- arbitrary-input decoder smokes for panic resistance;
- transport boundary conditions: TP, ETP, Fast Packet, BAM, RTS/CTS, aborts,
  sequence numbers, and timeouts;
- PGN/identifier normalization and rejection before invalid frames reach stack
  logic.

When a bug is fixed, preserve the input as a fixture or property seed unless it
is too large. Large captures should be reduced first.

## Phase 3: keep facades honest

The Rust API can be richer than the bindings. That is fine. What is not fine is
a binding function that compiles but is not exercised.

For every new C/Python operation:

1. add Rust behavior and Rust tests;
2. add the C or Python facade;
3. add one happy-path binding test;
4. add at least one guardrail test for disabled subsystems, bad lengths, null
   pointers, out-of-range values, or precondition failures;
5. run the binding make target and `make verify`.

Avoid exposing borrowed Rust storage, raw layout, or internal state-machine
types through C or Python.

## Phase 4: make hardware evidence boring

The docs now have a capture playbook and an executable evidence manifest. The
next step is not to write more prose; it is to run the captures and check in
small, named evidence packets.

Each packet should contain:

- the requirement ID;
- the exact command, usually `candump -td -L`;
- the reduced trace;
- manifest provenance;
- a report with commit, interface, bitrate, peer device/tool, behavior proven,
  and caveats;
- a replay or test when practical.

Do not batch unrelated flows into one giant trace. Small evidence is easier to
review and easier to replay.

## Phase 5: improve interoperability confidence

After the local and hardware evidence paths are stable, improve independent
peer coverage in this order:

1. address claim and PGN Request flows;
2. diagnostics request/response and clear flows;
3. TP/BAM and large payload transfer;
4. VT object-pool upload;
5. TC DDOP upload and process data;
6. File Server workflows;
7. Section Control lifecycle;
8. TIM authority/interlock behavior.

Each area should add tests first, then traces, then docs.

## Phase 6: keep the docs as the evidence index

The book should be understandable without reading every test. For each new
feature or hardening fix:

- put concepts and examples in the tutorial chapters;
- put exact evidence status in `protocol-coverage.md`;
- put capture instructions in `hardware-evidence.md`;
- put release implications in `release.md` or `validation-history.md`;
- keep the CSV matrix machine-readable.

If the docs and tests disagree, treat that as a bug.
