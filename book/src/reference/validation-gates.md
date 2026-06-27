# Validation gates

Use Makefile targets. The Makefile is the contract for this repository: when a
chapter says "build", "test", "run an example", or "check a binding", prefer the
target below instead of typing the underlying Cargo, C compiler, or Python
commands by hand.

| Target | Purpose |
| --- | --- |
| `make build` | Build the crate. |
| `make test` | Run default tests. |
| `make verify` | Full local validation gate. |
| `make no-std-check` | Check the transitional embedded `no_std + alloc` surface. |
| `make no-std-target-check` | Check the embedded surface on `NO_STD_TARGET` (default `thumbv7em-none-eabihf`). |
| `make no-std-surface-check` | Check embedded public imports and loop shape through a dedicated no-std surface test. |
| `make embedded-examples-check` | Compile embedded-shaped examples without requiring host IO. |
| `make bind-c-check` | Check generated C header. |
| `make c-demo` | Build/run C demo. |
| `make c-full-demo` | Build/run full C demo. |
| `make python-demo` | Build/install/run Python smoke. |
| `make trace-replay-demo` | Run trace replay examples. |
| `make fuzz-smoke` | Run arbitrary-input decoder smoke tests. |
| `make wirebit-examples-check` | Compile SocketCAN examples without requiring live CAN. |
| `make standard-suite-check` | Run the standard-derived ISO 11783, AEF TIM, and NMEA 2000 test suite. |
| `make book` | Build this mdBook. |
| `make whitespace-check` | Run `git diff --check`. |

If in doubt, run `make verify`.

## Typical development loop

For ordinary source or documentation work:

```sh
make build
make test
make book
make whitespace-check
```

For protocol, binding, or release-sensitive work:

```sh
make verify
```

`make verify` is intentionally broader than a normal unit-test run. It checks
default and all-feature builds, clippy, rustdoc, generated C header drift, C
demos, Python smoke/regression behavior, trace replay, fuzz smoke, SocketCAN
example compilation, the standard-derived coverage suite, and whitespace.

For embedded/no-std work, also run:

```sh
make no-std-check
make no-std-target-check
make no-std-surface-check
make embedded-examples-check
```

Those targets are separate from `make verify` while the embedded profile is
transitional, so run them explicitly when changing feature gates, protocol core
imports, CAN transport boundaries, or embedded examples.

## Running examples through the Makefile

The generic example target is:

```sh
make run EXAMPLE=session_minimal
make run EXAMPLE=vt_server_demo
make run EXAMPLE=tc_server_demo
```

The target wraps the repository's chosen Cargo command. If an example chapter
names an example file, use the file stem after `EXAMPLE=`.

## What a green local gate proves

A green `make verify` proves that the code and documentation passed the
repository's checked evidence. It catches stale generated headers, broken
examples, binding drift, trace parser regressions, malformed-payload test
failures, and public wording that crosses the stated claim boundary.

It does not prove vendor interoperability, physical CAN timing on your machine,
or official product approval. For those, add trace captures and reports through
the hardware evidence process described in
[Hardware evidence](hardware-evidence.md).
