# Examples overview

Examples are the safest way to learn the current API shape because they compile
with the repository. Prefer them over freehand snippets.

Run examples through the Makefile:

```sh
make run EXAMPLE=session_minimal
```

`session_minimal` demonstrates the
[session facade](../guide/session-facade.md) — building a node from plugins,
claiming an address, and routing events across a virtual bus.

Special binding examples have named targets:

```sh
make c-demo
make c-full-demo
make python-demo
```

Each example chapter explains:

- command to run
- expected output shape
- what it proves
- what it does not prove

## Current example families

| Family | Example stems |
| --- | --- |
| session facade | `session_minimal` |
| basics | `address_claim`, `heartbeat_demo`, `virtual_can_demo`, `transport_demo` |
| diagnostics/powertrain | `diagnostic_demo`, `engine_powertrain_demo`, `tractor_ecu_demo` |
| VT/TC/FS | `vt_client_demo`, `vt_server_demo`, `tc_client_demo`, `tc_server_demo`, `file_server_demo` |
| GNSS/NMEA | `gnss_monitor`, `gnss_batch`, `serial_gnss`, `speed_monitor` |
| bindings | `examples/c_abi`, `examples/python_binding` |
