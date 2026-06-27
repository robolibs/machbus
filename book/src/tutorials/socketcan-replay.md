# SocketCAN replay

Trace replay lets you turn a capture into repeatable test evidence.

Use it for:

- compact trace fixtures
- bracketed candump fixtures
- malformed-line parser tests
- standard-ID rejection
- CAN FD token rejection in classic-only paths

Always record interface, bitrate, device list, command, and expected result.

## Local replay command

```sh
make trace-replay-demo
```

## Capture metadata

For a new trace, record:

- interface name, bitrate, and sample-point policy
- kernel/driver or adapter details
- connected devices and source addresses
- exact command used to capture
- expected accepted/rejected frame counts
- whether the trace is virtual CAN, bench hardware, or field capture
