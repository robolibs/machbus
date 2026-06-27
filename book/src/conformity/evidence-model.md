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
| Trace replay | Captured log regressions | Devices not in captures |
| Hardware capture | Real bus behavior | Official certification |
| AEF/PlugFest-style validation | Interoperability evidence | Future regressions |

## Repository evidence map

- `tests/protocol_fixtures.rs`: protocol fixture checks.
- `tests/standard/session_harness.rs`: multi-node session integration behavior.
- `examples/c_abi/`: C ABI demos plus surface/layout checks (`make c-demo` / `c-full-demo`).
- `examples/c_abi`: C examples and layout probe.
- `examples/python_binding`: Python wheel/demo/regression smoke.
- `tests/fuzz_targets.rs`: bounded fuzz/property smoke.
- `book/src/reference/audit/`: previous audit evidence and rationale.

The new book should stay readable, but it should link back to evidence when a
chapter makes a strong claim.
