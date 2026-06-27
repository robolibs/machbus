# What is tested

The project currently verifies a broad local surface through `make verify`.

Major tested areas include:

- Address claim and NAME management.
- J1939 diagnostic messages and diagnostic request behavior.
- Transport Protocol and Extended Transport Protocol boundaries.
- NMEA 2000 Fast Packet and NMEA 0183 serial GNSS parsing.
- Virtual Terminal client/server handshake, object pool transfer, typed object
  bodies, updates, and server-side semantic state cache.
- Task Controller client/server workflows, DDOP parsing/helpers, process data,
  measurement triggers, and TC-GEO helpers.
- File Server client/server operations.
- Sequence Control master/client workflows.
- Tractor, implement, guidance, powertrain, and facility helpers.
- Rust session presets and plugins.
- C ABI and Python bindings.

For exact rows and evidence links, see [Protocol matrix](../reference/protocol-matrix.md).
