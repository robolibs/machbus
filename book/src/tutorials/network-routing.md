# Network routing

Most ISOBUS machines are not one bus. A tractor carries its own segment, the
implement hanging off the back carries another, and something has to sit between
them deciding which traffic crosses and which stays local. That something is a
**Network Interconnect Unit** (NIU) — a router or bridge that joins two CAN
segments and selectively forwards frames between them. This tutorial explains
why those multi-segment networks exist, what an NIU actually does to each frame,
and how to drive one with `machbus` through `Niu`, `Router`, and `NiuConfig`
(ISO 11783-4).

If you have read [Address claim](address-claim.md), you already know how a single
node finds its place on one segment. This page is about what happens when there
is more than one segment and the two have to be stitched together without
flooding either side.

## Why this exists

A single CAN segment has hard limits. It can only be so long, carry so many
nodes, and sustain so much traffic before arbitration latency and bus load make
it unreliable. A working machine routinely exceeds what one segment can carry:

- The **tractor segment** hosts the tractor ECU, the operator's terminal, engine
  and transmission controllers, and the network-management traffic that keeps
  them coordinated.
- The **implement segment** hosts whatever is mounted or towed — a sprayer, a
  seeder, a baler — with its own controllers, sensors, and high-rate sensor
  chatter that the tractor side never needs to see.

Connectors get plugged and unplugged in the field, implements are swapped
between tractors, and each side wants to keep its own dense local traffic
private while still exchanging the handful of messages that matter across the
join: ground speed, guidance, lighting, address claims. If you wired the two
segments into one electrically, every frame on one side would burden the other,
and the combined node count and cable length would blow past the physical
limits. So instead you put a small forwarding device in the middle that listens
to both sides and copies across only what should cross.

That device is the NIU. Depending on how much it rewrites, the standard calls it
a repeater (copies everything), a bridge (copies a filtered subset), a router
(also rewrites addresses so a whole segment looks like a single node), or a
gateway (repackages message content). `machbus` ships the two that matter most:
the filtering bridge as `Niu`, and the address-translating router as `Router`.

## Mental model

Picture two buses joined by one box. Frames flow in from either side; a filter in
the middle decides each one's fate; survivors are emitted on the opposite side.

```
   TRACTOR SEGMENT                              IMPLEMENT SEGMENT
 ┌───────────────────┐                       ┌───────────────────┐
 │ tractor ECU       │                       │ sprayer controller│
 │ terminal (VT)     │        ┌───────┐      │ section valves    │
 │ engine / trans    │ ─────► │  NIU  │ ────►│ rate sensors      │
 │ ...               │ ◄───── │ filter│ ◄────│ ...               │
 └───────────────────┘  Side  │ + rate│ Side └───────────────────┘
                      Tractor  │ limit │ Implement
                               │ (+ xlat
                               │  table)│
                               └───────┘
                       for each frame:
                       allow / block / monitor / rate-limit
                       (router also rewrites src & dst address)
```

The NIU is a **pump**, not a daemon. You hand it one frame at a time together
with the side it arrived on; it returns either the frame to put on the *other*
side, or nothing if the frame was dropped. The caller — typically the loop
around your two `IsoNet` segments — is responsible for actually placing the
returned frame on the destination bus. Nothing forwards itself.

## Anatomy: the pieces of an NIU

An NIU in `machbus` is built from a few well-separated parts.

| Piece | Type | What it holds |
| --- | --- | --- |
| The two sides | `Side::Tractor`, `Side::Implement` | A two-valued label for "which segment a frame came from". `Side::other()` flips it. |
| Configuration | `NiuConfig` | Name, default-forwarding policy, filter mode, and loop-guard tuning. |
| Filter rules | `Vec<FilterRule>` | The per-PGN / per-NAME table of what to do. |
| Forward decision | `ForwardPolicy` | `Allow`, `Block`, or `Monitor` for a matched frame. |
| The forwarder | `Niu` | Applies the rules, rate-limits, and counts forwarded vs. blocked. |
| Translation table | `AddressTranslationDb` | Maps a NAME's address on one side to its address on the other. |
| The router | `Router` | A `Niu` plus an `AddressTranslationDb` — forwards *and* rewrites addresses. |

A `FilterRule` is the unit of policy. It carries the PGN it matches (`0` meaning
"any PGN", used by NAME-based rules), the `ForwardPolicy` to apply, a
`bidirectional` flag (when false the rule only applies to tractor-originated
frames), optional `source_name` / `destination_name` constraints, and an
optional `max_frequency_ms` that throttles how often a matching frame may cross.
Rules can also be marked `persistent`; local runtime reloads through
`Niu::clear_filters` keep those rules as the baseline while removing temporary
rules.

`NiuConfig` sets the defaults around those rules. The most important field is
`filter_mode`:

| `NiuFilterMode` | Default for unmatched frames |
| --- | --- |
| `PassAll` | Forward unless a rule blocks it — open by default, block the noisy exceptions. |
| `BlockAll` | Drop unless a rule allows it — closed by default, allow only the messages that must cross. |

`PassAll` suits a bridge that should be mostly transparent; `BlockAll` suits a
tight join where you enumerate exactly what is permitted to cross. `NiuConfig`
also carries `forward_global_by_default` and `forward_specific_by_default`,
which let broadcast and destination-specific traffic default differently under
`PassAll`.

## How a forwarding decision is made

When a frame arrives, `Niu::process_frame(frame, origin, now_ms)` walks a fixed
sequence before it ever consults a rule:

1. **Inactive NIU.** If the NIU has not been `start`ed, everything is dropped.
2. **Bad source address.** A frame whose source is the broadcast address, or the
   null address on anything other than an address-claim, is dropped as
   malformed. It never reaches the filter.
3. **Loop echo.** If this exact frame was just forwarded toward this side inside
   the loop-guard window, it is dropped (see [Loop prevention](#loop-prevention)).
4. **Learn NAMEs.** Address-claim frames are recorded so the NIU knows which NAME
   owns which address on each side — this is what makes NAME-based rules work.
5. **Resolve policy.** Now the rule table is consulted.

Rule matching runs top to bottom. A rule matches when its PGN matches (or is
`0`), its direction matches the origin side, and any `source_name` /
`destination_name` constraints are satisfied by the NAMEs the NIU has learned.
The first matching rule wins, and its `ForwardPolicy` decides the outcome:

- **`Allow`** — forward the frame to the other side; bump the forwarded counter.
- **`Block`** — drop it; bump the blocked counter.
- **`Monitor`** — forward it *and* additionally raise `on_monitored`, so you can
  observe a class of traffic without blocking it.

If a matching rule carries a `max_frequency_ms`, the NIU also rate-limits: the
first crossing always passes, and subsequent crossings are dropped until that
many milliseconds have elapsed. Rate limiting is how you let a high-rate
diagnostic or status PGN cross "occasionally" without relaying every frame. If
*no* rule matches, the `filter_mode` default applies.

## Address translation across segments

A plain `Niu` forwards a frame's bytes unchanged. That is fine when both
segments share one address space, but a `Router` does more: it makes an entire
segment appear, from the other side, as a single well-known node.

The reason is addressing. A node's source address is only meaningful on the
segment where it claimed it. Address `0x80` on the implement side and address
`0x80` on the tractor side can be two completely different control functions. If
the router copied addresses across verbatim, replies would go to the wrong node.
So the `Router` keeps an `AddressTranslationDb`: for each NAME it cares about, it
stores that NAME's `tractor_address` and its `implement_address`. On forward, it
rewrites the source and destination so each segment sees the address that is
valid *for that segment*.

```
implement side                 router                  tractor side
 src=0x80 (sprayer) ──►  translate 0x80→0x21   ──►  src=0x21 (sprayer-as-seen)
 dst=0x21 (tractor) ──►  translate 0x21→0x80   ──►  dst=0x80 (tractor-local addr)
```

`Router::add_translation(name, tractor_addr, implement_addr)` populates the
table; `AddressTranslationDb` enforces that a given side-local address belongs to
only one active NAME, returning an `AddressConflict` error if you try to map two
NAMEs onto the same address on the same side. Translation behaves differently for
the two traffic shapes:

- **Broadcast frames** — only the source is translated; the destination stays
  the broadcast address.
- **Destination-specific frames** — both ends are translated, and if the
  destination has *no* translation, the frame is blocked. There is no point
  forwarding a directed frame to a node the other side cannot address.

Address-claim frames get special care: the router checks that the NAME inside the
claim matches the NAME the translation table expects for that address, and blocks
the claim if it does not — preventing a node from impersonating another's mapped
identity across the bridge.

## Loop prevention

The standard requires that the physical network have exactly one path between any
two control functions. If two NIUs accidentally create a second path, a single
broadcast can circle forever, each NIU dutifully forwarding what the other just
emitted. `machbus` adds a defensive **loop guard** so a misconfiguration
degrades instead of melting the bus.

Every forwarded frame is remembered — its identifier, length, and payload, tagged
with the side it was sent toward and a timestamp. When a frame arrives that
exactly matches something recently forwarded toward the side it just came from,
the NIU treats it as an echo and drops it. Two `NiuConfig` knobs tune this:
`loop_guard_window_ms` (how long a forwarded frame stays remembered) and
`loop_guard_max_recent_forwards` (how many to remember). Setting either to `0`
disables the guard. The guard is a safety net, not a substitute for a correct
single-path topology — design the network so loops cannot form, and keep the
guard as backstop.

## The NIU control protocol

An NIU is not only a silent forwarder; it is a node that other tools can
configure over the bus. It participates in a dedicated control message,
`NiuNetworkMsg`, carried on its own PGN. Each message names a `NiuFunction` and a
port, and `Niu::handle_niu_message` reacts to it:

| `NiuFunction` | What the NIU does |
| --- | --- |
| `AddFilterEntry` | Install a new allow rule for the given PGN. |
| `DeleteFilterEntry` | Remove the rule matching that PGN. |
| `DeleteAllEntries` | Clear the whole filter table, including persistent rules. |
| `SetFilterMode` | Switch between `PassAll` and `BlockAll`. |
| `RequestPortStats` | Reply with forwarded / blocked counts. |
| `RequestFilterDb` / `FilterDbResponse` | Read back the installed rules. |
| `RequestPortConfig` / `PortConfigResponse` | Report port configuration. |
| `OpenConnection` / `CloseConnection` | Manage a forwarded destination-specific connection. |

Every handled message also raises `on_niu_message`, so your application can log
or audit reconfiguration as it happens. This is how a diagnostic tool on the bus
can tighten or loosen the filter, or read traffic statistics, without
recompiling the NIU.

## Doing it with machbus

The `examples/niu_demo.rs` example builds a bridge in `BlockAll` mode, allows a
couple of PGNs, blocks one, and then feeds frames through it.

You configure the NIU declaratively, then start it:

```rust
{{#include ../../../examples/niu_demo.rs:11:19}}
```

`NiuConfig::default().name(...).mode(...)` sets the policy; `allow_pgn`,
`allow_pgn_rate_limited`, and `block_pgn` install rules; `start` flips the state
to `Active` so frames begin flowing. The `true` argument on each rule makes it
bidirectional.

Then each frame is pumped through `process_frame`, which returns `Some(frame)` to
forward or `None` to drop:

```rust
{{#include ../../../examples/niu_demo.rs:27:60}}
```

The heartbeat is allowed and forwards; the request PGN is explicitly blocked; the
DM1 is rate-limited to one crossing per 100 ms, so the frame at `t=50` passes,
the one at `t=100` is dropped as too soon, and the one at `t=200` passes again
once the window has elapsed. `forwarded()` and `blocked()` report the running
totals.

To add address translation, wrap the same configuration in a `Router` instead and
register the NAMEs that cross. The shape (illustrative, not from the example) is:

```rust
// illustrative shape — see the niu.rs API for exact signatures
let mut router = Router::new(NiuConfig::default().mode(NiuFilterMode::BlockAll));
router.niu_mut().allow_pgn(PGN_HEARTBEAT, true);
router.add_translation(sprayer_name, 0x21, 0x80)?; // tractor 0x21, implement 0x80
let forwarded = router.process_frame(frame, Side::Implement, now_ms);
```

`Router::niu_mut()` gives you the underlying `Niu` for filter and event
management, while `Router::process_frame` runs the filter first and then rewrites
addresses on whatever survives.

## Events and responsibilities

The NIU raises events so your application can observe forwarding without sitting
in the decision path:

| Event | Fires when | Typical use |
| --- | --- | --- |
| `on_forwarded` | A frame crossed to the other side. | Metrics, tracing. |
| `on_blocked` | A frame was dropped (rule, rate limit, loop, bad source). | Diagnose over-tight filters. |
| `on_monitored` | A `Monitor`-policy frame crossed. | Tap a traffic class for inspection. |
| `on_niu_message` | A control message reconfigured the NIU. | Audit remote reconfiguration. |

Your responsibilities around the pump:

- **Drive both directions.** Call `process_frame` for frames from each side; the
  NIU does not poll a bus itself.
- **Deliver what it returns.** A returned frame must actually be placed on the
  destination segment's `IsoNet` — the NIU only decides, it does not transmit.
- **Pass honest time.** `now_ms` drives rate limiting and the loop guard; feed a
  monotonic clock so windows mean what they say.
- **Claim before translating.** A router needs to learn NAMEs from address claims
  before its NAME-based rules and translations work; let claims flow.

## Edge cases and failures

- **Filter misconfiguration.** A `BlockAll` NIU with no allow rules silently
  drops everything; a `PassAll` NIU with no block rules is a transparent
  repeater. If "nothing crosses", check `blocked()` and the mode before
  suspecting the bus.
- **Translation collisions.** Mapping two NAMEs onto the same side-local address
  fails with an `AddressConflict`. The database refuses it rather than making
  routing non-deterministic — fix the address plan.
- **Directed frame with no destination translation.** A `Router` blocks a
  destination-specific frame whose destination has no mapping, because the other
  side has no valid address to deliver it to. Broadcasts are unaffected.
- **Forwarding loops.** Two paths between the same nodes will loop. The loop
  guard suppresses the immediate echo, but the real fix is a single-path
  topology. If loop-guard blocked counts climb, you have a redundant path.
- **Segment overload.** An NIU that forwards too liberally can push one segment
  past its bus-load budget. Use `BlockAll` plus narrow allow rules, and
  rate-limit high-frequency PGNs, to keep each side within its capacity.
- **Stale clock.** If `now_ms` never advances, rate-limited rules behave as if no
  time passes and may block every crossing after the first.

## Advanced

- **Multi-router topologies.** Larger machines chain several NIUs — for example a
  tractor segment, an implement segment, and a sub-implement behind that. Keep
  the single-path rule across the whole tree; every additional join is another
  place a loop or a translation gap can hide. Give each NIU a distinct `name` so
  control messages and traces are unambiguous.
- **Performance and bus load.** Forwarding is not free: every crossed frame is
  traffic the destination segment must absorb. Think in terms of each segment's
  budget and forward only what the other side needs. Rate limiting and a
  `BlockAll` default are your main levers; broadcast-heavy PGNs are the usual
  culprits.
- **Bridge vs. router.** Use `Niu` when both segments share an address space and
  you only need to filter. Use `Router` when a segment must appear as a single
  node to the other side, or when the two sides allocate addresses
  independently and a directed reply must reach the right node.
- **Snapshots for audit.** `Niu::policy_snapshot` and `Router::policy_snapshot`
  return deterministic, runtime-state-free dumps of the configured policy and
  translations — useful for regression fixtures and operator review without
  leaking mutable counters or learned state.

## Validate locally

```sh
make run EXAMPLE=niu_demo
make test
```

The example runs entirely in software: it builds a `BlockAll` bridge, forwards a
heartbeat, blocks a request PGN, and demonstrates the 100 ms rate limit on a DM1,
printing the forwarded and blocked totals at the end. No bus or hardware is
required.

## What this proves / does not prove

Proves: the filter rules, forward policies, rate limiting, loop guard, and
address translation behave deterministically in software, and the `Niu` /
`Router` API drives them as described.

Does not prove: real-hardware forwarding latency, priority-ordered queueing under
load, interoperability with a specific third-party NIU, or any
conformance/certification claim. Those still require official standards, real
hardware, and interoperability evidence. `machbus` is not certified.

## See also

- [Address claim](address-claim.md) — how each node on a segment gets the address
  the NIU learns and translates.
- [Transport protocol](../standards/iso11783-datalink-transport.md) — what happens to multi-frame
  messages that cross a segment.
- [PGNs, priority, source, destination](../standards/j1939.md)
  — the addressing fields every filter rule matches on.
- [Receiving and routing](../standards/iso11783-network-layer.md) — how a single node dispatches
  the traffic an NIU delivers to it.
