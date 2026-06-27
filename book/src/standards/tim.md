# TIM — Tractor Implement Management (AEF)

TIM lets an *implement command the tractor* — adjust speed, work the hitch, spin the
PTO — so the implement can optimize the whole operation rather than just react to it.
That is powerful and dangerous, so TIM is built around **authority with safety
interlocks**. It is an AEF-driven capability layered on the ISO messages.

## Why this exists

Often the implement knows best. A baler knows when to slow the tractor for a dense
windrow; a precision planter knows when to lift. Letting the implement make those
adjustments directly is more accurate and less tiring than the operator relaying them.
But handing a stranger's box control of a moving tractor demands a hard safety model —
hence authority that must be explicitly granted and can be revoked the instant an
interlock trips.

## The authority handshake

```
   IMPLEMENT ── request authority over {rear hitch, rear PTO} ──►  TRACTOR
             ◄── grant (only if interlocks are clear) ──────────
   IMPLEMENT ── command: rear PTO engage CW ──►  (allowed only while granted)
             ◄── status: PTO engaged ──────────
                 …
             ── any interlock trips → authority revoked → further commands refused
```

The guard is the whole point: a guarded command is refused **before any frame goes on
the wire** unless authority is currently granted *and* the local interlocks are clear.
A revoked grant immediately blocks subsequent commands.

## How machbus expresses it

The `Tim` plugin is a local authority/interlock guard plus the guarded command helpers
and the decoded PTO/hitch/aux status stream:

```
   ctrl.with_mut::<Tim, _>(|t| {
       t.request_authority(options)?;     // ask
       t.grant_authority()?;              // (tractor side) grant if interlocks clear
       t.command_pto_engage(Pto::Rear, true)   // refused unless granted + clear
   });
   // a blocked command surfaces as Event::Tim(TimEvent::CommandBlocked { .. })
   // an interlock change surfaces as Event::Tim(TimEvent::AuthorityStateChanged(..))
```

`set_interlocks` updates the local safety state; if it revokes authority, the change is
reported and further guarded commands fail until authority is re-granted.

## Certification

TIM is defined and **certified by AEF**. machbus ships the *mechanism* — the authority
guard, the guarded commands, the status decoding — but **no certification**. Shipping a
TIM-capable product to market is a separate AEF process; see
[Conformity first](../conformity/index.md).

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Authority + guarded commands | `session::plugins::Tim` | [TIM and automation](../tutorials/tim.md) |
| The hitch/PTO messages it guards | `session::plugins::Implement` | [ISO 11783-7 — implement messages](iso11783-implement-messages.md) |
| The certification boundary | — | [Conformity first](../conformity/index.md) |

## Failure modes worth knowing

- **Commanding without authority** — refused by design; check the grant first.
- **Ignoring revocation** — an interlock can revoke mid-operation; watch for the
  authority-state event and stop commanding.
- **Treating the mechanism as certification** — it is not.

## See also

- [ISO 11783-9 — the tractor ECU](iso11783-tractor-ecu.md) — the facilities TIM borrows.
- [ISO 11783-14 — sequence control](iso11783-sequence-control.md) — automated motion
  that often pairs with TIM.
