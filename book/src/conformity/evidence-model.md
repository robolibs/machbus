# Evidence model

machbus uses layered evidence. Each layer catches different failures.

| Level | Catches | Misses |
| --- | --- | --- |
| Static API shape | Missing public functions, feature drift | Runtime behavior |
| Unit tests | Local codec/state mistakes | Cross-role interaction |
| Golden byte fixtures | Exact wire byte regressions | Untested byte variants |
| Property/fuzz smoke | Panic and bounds bugs | Semantic conformance gaps |
| Session/role tests | Multi-role workflows | Real hardware timing |
| Binding tests | C/Python wrapper drift | Every host platform |
| Standards-text review | Rules never implemented right | Anything not written down |
| Trace replay | Captured log regressions | Devices not in captures |
| Hardware capture | Real bus behavior | Official certification |
| AEF/PlugFest-style validation | Interoperability evidence | Future regressions |

## Why standards-text review earns its own row

Every layer above it takes the implementation's own assumptions as the baseline.
Unit tests prove the code does what its author meant; fixtures pin the bytes it
already produces; session tests run both halves of a conversation against each
other. None of that can catch a wire rule that was reconstructed rather than
read, because both sides of the test share the reconstruction.

That is not hypothetical. The [standards-text
audit](../reference/audit/standards-text-audit.md) found nine defects, seven of
which only misbehave against a *conformant peer* — over-strict receive paths and
uniform defaults where the standard varies. Testing a stack against itself could
not have surfaced any of them.

The layer's blind spot is the mirror image: it sees only what the documents
write down. Much of ISO 11783 deliberately does not — PGN assignments, DDIs and
NAMEs all live in the electronic database at isobus.net rather than in the
standards, so no PGN constant in this crate is checkable this way.

## Repository evidence map

- `tests/protocol_fixtures.rs`: protocol fixture checks.
- `tests/standard/session_harness.rs`: multi-node session integration behavior.
- `examples/c_abi/`: C ABI demos plus surface/layout checks (`make c-demo` / `c-full-demo`).
- `examples/c_abi`: C examples and layout probe.
- `examples/python_binding`: Python wheel/demo/regression smoke.
- `tests/fuzz_targets.rs`: bounded fuzz/property smoke.
- `book/src/reference/audit/`: previous audit evidence and rationale.
- `book/src/reference/audit/standards-text-audit.md`: what was checked against
  the ISO/AEF/NMEA text, what it found, and what it cannot show.

The new book should stay readable, but it should link back to evidence when a
chapter makes a strong claim.
