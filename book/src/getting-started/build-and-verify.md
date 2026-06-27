# Build and verify

Use Makefile targets first.

```sh
make build
make test
make verify
```

`make verify` is the canonical local gate. It runs the normal build/test lane,
all-feature checks, clippy, rustdoc, C binding checks, C examples, Python smoke,
trace replay, fuzz smoke, claim-boundary checks, package checks, hardware
evidence contract checks, and whitespace checks.

Embedded/no-std checks are intentionally separate while that profile is still
evolving. If you changed feature gates, protocol-core imports, the session loop,
CAN transport seams, storage/file splits, or embedded examples, also run:

```sh
make no-std-check
make no-std-target-check
make no-std-surface-check
make embedded-examples-check
```

`make no-std-target-check` uses the configured `NO_STD_TARGET` embedded target
and will tell you to install it if it is missing.

If you only changed documentation, still run at least:

```sh
make book
make whitespace-check
```

before claiming the book is healthy. Run `make verify` when documentation changes
also depend on source, examples, bindings, or generated artifacts.
