# Release checklist

Release work is a paperwork-and-evidence exercise. The code may already be
merged, but a tag should wait until the versions, generated files, package
payload, docs, and validation logs tell the same story.

## Before changing the version

1. Read the current `CHANGELOG.md`.
2. Check whether the release is Rust-only, Python-only, C-header relevant, or
   all three.
3. Inspect `Cargo.toml`, `pyproject.toml`, `PROJECT`, and generated C header
   metadata for version drift.
4. Check `book/src/reference/audit/conformance.md` before writing any release
   notes that mention protocol evidence.

## Required commands

Run these from the repository root:

```sh
make fmt
make bind-c
make verify
make standard-suite-check
git diff --check
make bind-c-check
```

`make verify` already runs most of the named checks, but the release checklist
keeps the high-risk steps visible. `make bind-c` updates the generated header;
`make bind-c-check` confirms it is not drifting afterward. The repeated standard
suite is deliberate: a release reviewer should see it even when they do not
expand the full `make verify` target.

## Standard and evidence audit before a tag

Before tagging, inspect `book/src/reference/assets/standard_gap_matrix.csv` and
record the current status counts in the release note or validation log. In
particular, count:

- rows still marked `implemented-needs-standard-tests`;
- rows with `external_trace_status=missing`;
- rows marked `complete`.

Rows with `external_trace_status=missing` are not release blockers by
themselves, but they are claim blockers. The release note must not turn those
rows into broad hardware, external interoperability, or certification claims.
The stable evidence map lives at
`tests/fixtures/evidence/gap_external_evidence_map.txt`; hardware capture
requirements live at `tests/fixtures/hardware/capture_requirements.txt`.
Promoting a broad row requires the mapped hardware or public-data evidence
requirements to be complete, with reports and reduced traces where required.

## Package contents inspection

Publishing requires package contents inspection, not just tests. Inspect the
list before tagging:

```sh
cargo package --allow-dirty --list
```

Confirm that release metadata, the `book/` mdBook sources, generated headers,
examples, tests, fixtures, and Python metadata are present while local build
artifacts (`target/`, `.git/`, virtualenvs, proptest regressions) are absent.
The `[package].include` list in `Cargo.toml` is the source of truth for what
ships.

The current crate still has `publish = false`. If that changes, check the
registry story for path dependencies before upload. The workspace currently
uses path dependencies, and local packaging does not prove those path
dependencies are available from the target registry.

## Changelog rules

Write concrete entries:

- Added
- Changed
- Fixed
- Hardened
- Validation

Avoid placeholder migration language. If the old C++ project motivated a
change, describe the actual behavior that was ported and the tests that now
cover it.

## Tagging rule

Tag only after the above evidence is recorded. At minimum, record the commit,
the version numbers, the command list, and any caveats about hardware or
protocol evidence.
