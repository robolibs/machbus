# Build problems

Try:

```sh
make build
make test
make verify
```

Common causes:

- Rust toolchain missing or too old.
- C compiler missing for C ABI examples.
- Python environment has a stale wheel.
- rustdoc/clippy warning promoted to error.

Prefer fixing the Makefile gate over bypassing it.
