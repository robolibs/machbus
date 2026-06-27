# Install Rust

machbus is a Rust crate with optional C and Python surfaces.

Recommended local setup:

```sh
rustc --version
cargo --version
make build
```

If this repository is opened through its Nix shell, use the repository-provided
environment and still prefer Makefile targets for validation.

Feature flags are documented in [Feature flags](../reference/feature-flags.md).
