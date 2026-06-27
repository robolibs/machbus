# ISO 11783-5 — network management and address claiming

This is the part that makes "hitch any implement to any tractor" actually work. Part 5
turns *two strange boxes that both want the same address, with no installer and no
central authority* into a deterministic, self-healing assignment. It is the first
thing every node does after power-on, and nothing above it may speak until it
finishes.

## Why this exists

SAE J1939 mostly assigns addresses by convention — fine for one manufacturer's truck.
On a farm, a random implement from another vendor, built years apart, may want the
same address as something already on the bus. There is no human to resolve it. Part 5
resolves it automatically using the one thing that is guaranteed unique and
comparable: the [NAME](iso11783-general-device-classes.md).

## NAME arbitration: lowest NAME wins

When two CFs contend for an address, they compare 64-bit NAMEs and the numerically
*lower* one keeps the address. Because NAMEs are unique, the contest always has a
winner and the network always converges.

```
   Both want address 0x80:

   CF-A   NAME = 0x00A0_0000_0000_1234   ┐
   CF-B   NAME = 0x00C0_0000_0000_5678   ┘ compare as 64-bit numbers
                       │
                 0x00A0… < 0x00C0…
                       ▼
        CF-A keeps 0x80   ·   CF-B must move (if self-configurable) or go silent
```

This is the same "lowest number wins" rule as CAN arbitration — applied to identity
instead of priority.

## The state machine

```
   POWER ON
      │   have NAME + preferred address; may not send app traffic yet
      ▼
   CLAIMING ── broadcast "I claim 0x80, NAME = N" ──►  bus
      │
      │   wait the contention window, then:
      │
      ├─ no contender ──────────────►  CLAIMED   (may now send app traffic)
      ├─ contender, higher NAME ────►  CLAIMED   (the contender yields)
      └─ contender, lower NAME ─────►  lose 0x80:
            ├─ self-configurable ─►  claim another address (→ CLAIMING)
            └─ not ───────────────►  SILENT (cannot operate)
```

Rules machbus enforces:

- **No application traffic before CLAIMED.** The session core refuses app sends until
  the claim completes — this is why "drive the claim loop first" is the cardinal rule.
- **Request for Address Claim.** Any node can ask everyone to re-announce; a CF replies
  with its current claim.
- **Address violation → diagnostic.** Detecting another node on your claimed address is
  surfaced as a DTC (SPN derived from the offending address).
- **Loss mid-run.** A lower-NAME newcomer can take your address while you are running;
  a self-configurable CF moves, and the change is reported on the event stream.
- **Convergence.** Because the tie-break is numeric and NAMEs are unique, the network
  reaches a stable assignment with no oscillation.

## NAME management (dynamic identity)

A further wrinkle: a CF can be *commanded* to adopt a new NAME (NAME management). The
node applies the new identity and re-claims. machbus models this with the
`NameManagement` plugin, which answers the management requests, adopts a commanded
NAME, and triggers a fresh claim.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Claiming an address | `Session::start()` then drive `poll()`; watch for the `Claimed` event | [Address claim](../tutorials/address-claim.md) |
| Runtime "is it claimed?" | `controls.is_claimed()` (or watch for the `Claimed` event) | [Address claim](../tutorials/address-claim.md) |
| Dynamic NAME adoption | `session::plugins::NameManagement` | [NAME management](../tutorials/name-management.md) |
| Address-conflict debugging | — | [Address conflicts](../troubleshooting/address-conflicts.md) |

## See also

- [ISO 11783-1 — the NAME](iso11783-general-device-classes.md) — the identity this part arbitrates on.
- [The networking foundation](foundations.md) — the overview that ties parts 1–5
  together.
