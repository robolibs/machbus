# Minimal example

Start with `examples/session_minimal.rs`.

Run it with:

```sh
make run EXAMPLE=session_minimal
```

What it proves:

- the crate builds
- a minimal node can be composed from plugins and spawned over a transport
- basic event and diagnostic flow is usable

What it does not prove:

- real CAN hardware behavior
- VT/TC interoperability
- official conformance

## How to read the output

The exact text may change as examples evolve, but the output should show a
node claiming an address, an event crossing the virtual bus, and a clean exit. If the
example panics or times out during address claim, debug the virtual endpoint and
NAME/address setup before moving to a larger scenario.
