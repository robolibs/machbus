# The networking foundation

Before any implement can borrow a screen or report a sprayed litre, four things must
already work: the wire must carry bits, those bits must form *named* messages, the
node must own an *address*, and it must be able to move data larger than one frame.
That spine is ISO 11783 parts 1–5 standing on SAE J1939 and CAN. This page is the map;
each layer has its own chapter.

## Why this is the spine

A farmer buys a tractor from one company and a planter from another, ten years apart,
and expects them to cooperate the moment the hitch pin drops — no installer, no
network admin. Plug-and-play between strangers on a noisy two-wire bus is the demand
that shapes every choice below. Get this layer right and everything above it is just
conversation; get it wrong and nothing works at all.

## The layer cake

```
Each layer trusts the one beneath it (read bottom-up):

   ISO 11783-5   network management  address claiming (lowest NAME wins)
   ISO 11783-4   network layer       joining segments, routing, loop guard
   ISO 11783-3   data link           TP / ETP / Fast Packet (big payloads)
   SAE J1939     naming              29-bit identifier · PGN · PDU1/PDU2
   ISO 11783-1   general             the NAME · device classes · layering
   ISO 11783-2   physical            250 kbit/s CAN · arbitration
```

## Read it layer by layer

- [ISO 11783-1 — general and device classes](iso11783-general-device-classes.md) — the architecture
  and the NAME (with its bit-field).
- [ISO 11783-2 — the physical layer](iso11783-physical-layer.md) — the CAN bus and
  non-destructive arbitration.
- [SAE J1939 — the heritage](j1939.md) — the identifier, PGNs, and the PDU1/PDU2 trap.
- [ISO 11783-3 — data link and transport](iso11783-datalink-transport.md) — moving more than 8
  bytes (TP/ETP/Fast Packet) with the RTS/CTS handshake.
- [ISO 11783-4 — the network layer](iso11783-network-layer.md) — joining CAN segments safely.
- [ISO 11783-5 — network management and address claiming](iso11783-network-management.md)
  — the plug-and-play handshake that runs first.

## The two ideas to carry away

1. **Lowest number wins, everywhere.** CAN arbitration (lowest identifier wins the
   bus), message priority (lowest value wins), and address claiming (lowest NAME wins
   the address) are the same rule applied at three levels. Once it clicks, the whole
   foundation feels inevitable.
2. **Layers above pretend messages are any size and addresses are stable.** Transport
   quietly chops and reassembles; address claiming quietly resolves conflicts. The
   application services get to ignore both.

## How machbus expresses the foundation

You almost never touch it directly — that is the point. Build a node with a NAME and a
preferred address; the claim runs first. Send a payload of any size; the network layer
picks single-frame / Fast Packet / TP / ETP. Receive fully reassembled `Message`s,
routed to the subsystem that cares. React to `AddressClaim` and bus/confinement events
on the unified stream. See [The session facade](../guide/session-facade.md).

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| Claiming an address | `Session::start()` + drive `poll()` | [Address claim](../tutorials/address-claim.md) |
| Sending any-size data | `Session::send_raw` / codec send (transport automatic) | [Transport Protocol](../tutorials/transport-protocol.md) |
| Routing across segments | `net::niu` | [Network routing](../tutorials/network-routing.md) |
| Seeing it on the wire | `candump` + replay | [Reading candump traces](../isobus-basics/reading-candump-traces.md) |

## Failure modes worth knowing (across the foundation)

- **PDU1 vs PDU2 confusion** — a frame that looks right but routes wrong.
- **Talking before claimed** — application sends are rejected until the claim completes.
- **Transport timeouts/aborts** — a stalled CTS or missed packet kills a transfer; retry.
- **Address loss mid-run** — a lower-NAME newcomer can take your address.
- **Bus-off / error confinement** — a flood of CAN errors takes a node off the bus.

## See also

- [The standards, end to end](index.md) and [Standards capability map](standards-capability-map.md).
- [The Virtual Terminal](virtual-terminal.md) — the first big service built on this
  spine.
