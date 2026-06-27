# Logging and traces

Traces turn bus behavior into reviewable evidence.

Useful trace sources:

- `candump` from SocketCAN
- compact fixture files
- bracketed candump-style fixture files
- malformed-line fixtures for parser hardening

machbus includes replay tooling and tests that reject invalid trace shapes such
as standard IDs where extended IDs are required, overlong classic CAN payloads,
bad hex, and CAN FD-looking tokens in classic-only paths.

Trace evidence should include the command, interface, bitrate, connected
devices, and expected result.
