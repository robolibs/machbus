# Virtual bus

The virtual bus is the easiest way to test two roles without hardware. It is the
recommended first integration environment because it exercises real stack
routing without requiring SocketCAN permissions, wiring, or a running tractor.

Common setup:

- tractor node
- implement node
- both attached to the same in-process bus
- run ticks until both claim addresses
- send one workflow message
- drain events on the peer

This is how many stack tests prove behavior without relying on SocketCAN timing
or machine wiring. After the virtual-bus workflow passes, move to SocketCAN
traces.

```sh
make run EXAMPLE=virtual_can_demo
```

## Two-node checklist

| Step | Tractor-like node | Implement-like node |
| --- | --- | --- |
| NAME | function/identity for tractor role | function/identity for implement role |
| preferred address | normally tractor range | normally implement range |
| enabled surfaces | diagnostics, GNSS, tractor/persona helpers | diagnostics, VT/TC/FS/implement helpers |
| startup | start address claim | start address claim |
| loop | call `tick()` | call `tick()` |
| proof | drain tractor events | drain implement events |

## Why this matters

The virtual bus catches address conflicts, destination-specific PGN mistakes,
event fan-out bugs, and many high-level stack regressions before you touch real
hardware. It does not prove physical timing or transceiver behavior.
