# Hardware and AEF path

The practical path from local tests to deployment usually looks like this:

1. Run `make verify`.
2. Run examples over a virtual CAN interface.
3. Capture and replay traffic with SocketCAN/candump.
4. Test with a small hardware bench.
5. Test with real VTs, TCs, service tools, tractors, and implements.
6. Compare traces against expected behavior.
7. Use official AEF or customer-required conformance processes.

## Hardware evidence should record

- repository commit
- CAN interface
- bitrate
- device list
- test steps
- raw capture path
- expected result
- observed result
- pass/fail decision

Hardware tests should never be vague. If a trace is used as evidence, keep the
trace and a short report with enough metadata that someone else can understand
what was connected.
