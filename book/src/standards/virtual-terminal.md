# The Virtual Terminal

The Virtual Terminal — ISO 11783-6 — is the most elaborate service on the bus, and
the one that makes ISOBUS visible to a farmer. It is a screen-sharing protocol: an
implement with no display of its own ships a complete description of its user
interface to the terminal in the cab, and from then on the two cooperate to show
controls and react to the operator. This chapter tells that story end to end.

## Why this exists

A modern cab cannot grow a new dial and a new screen layout every time a different
implement is attached. So the cab terminal is deliberately *dumb about implements*:
it is a general-purpose renderer that knows how to draw rectangles, numbers,
strings, bars, and buttons, and how to report touches and key presses. The
*implement* is the one that knows what its UI should look like. The VT protocol is
the contract that lets the implement describe its UI once, hand it over, and then
drive it — so any implement can present rich controls on any certified terminal.

```
   IMPLEMENT (VT client)                      CAB (VT server)
   "knows what the UI means"                   "knows how to draw"

     ── object pool (the whole UI) ──────────────►   uploaded once
     ── change_numeric_value(rate, 42) ──────────►   field redraws
     ◄─ soft key 3 pressed ───────────────────────   operator input
```

## Mental model: the object pool

The implement's UI is a **pool of objects**, each with a numeric ID, that reference
one another to form a tree. A *working set* object is the root; it points at data
masks (full-screen layouts); masks contain containers, output fields, input fields,
buttons, bar graphs, and so on; fonts, colours, and pictures are shared leaf
objects. machbus models all ~48 object types in `isobus::vt`.

```
   WorkingSet ──► DataMask "main" ──► OutputNumber "rate"
       │              │           └─► SoftKeyMask ──► Key 1, Key 2, Key 3
       │              └─► OutputString "status"
       └──► DataMask "settings" ──► InputNumber, InputList …

   shared: Font, Colour, Picture, Macro, … (referenced by ID)
```

The pool is *data*, not code. The implement builds it (often exported from a design
tool as an `.iop` file), and machbus's codec serializes it to the exact wire layout
the terminal expects and parses it back — including the per-type length walk that
makes the byte stream unambiguous.

## The lifecycle: find, upload, activate, run

A VT client moves through a connect state machine. The shape:

```
  DISCONNECTED
     │  who is a VT?  (look for VT status)
     ▼
  DISCOVERED  ── learns the VT's address + version
     │  send working-set announcement
     ▼
  UPLOADING   ── transfer the whole object pool (big → Transport Protocol)
     │  "End of Object Pool"; VT validates it
     ▼
  ACTIVATING  ── ask the VT to make this pool the active one
     │  activation OK
     ▼
  CONNECTED   ── steady state: push value changes, receive input events
```

Two realities make this interesting:

- **The pool is large.** The upload rides the Transport Protocol from the
  [foundation chapter](foundations.md) — often hundreds or thousands of bytes in one
  connection-mode transfer. A failed transfer means no UI.
- **Versions differ.** Terminals advertise a VT version and capabilities (screen
  size, colour depth, soft-key count, fonts). A good client adapts; machbus exposes
  the version handshake and capabilities so you can.

## Anatomy of the runtime conversation

Once `CONNECTED`, the protocol is a steady two-way stream:

**Client → VT (commands).** The implement keeps the displayed UI in sync with its
internal state. The common commands, all on the machbus VT client:

| Command | Effect on screen |
| --- | --- |
| change numeric value | update a number/bar/gauge |
| change string value | update text |
| hide / show, enable / disable | toggle visibility / interactivity |
| change active mask | switch the whole screen |
| change soft-key mask | switch the row of soft keys |
| change attribute / size / colour / position | restyle or move an object |
| select input object, lock/unlock mask | drive focus and modality |

**VT → client (events).** The terminal reports what the operator did and what it
decided:

| Event | Meaning |
| --- | --- |
| soft-key / button activation | operator pressed a key |
| numeric / string value changed | operator edited an input field |
| input object selected | focus moved / edit started |
| pool error | the VT rejected something in the pool |
| language / units changed | operator changed locale; reload if needed |
| active working set changed | another implement took the screen |

machbus surfaces these as `VtEvent` variants on the unified event stream.

## Doing it with machbus

On the recommended facade, the whole lifecycle is a plugin:

```
   Session::builder(name, addr)
       .plug(VtClient::new(config, pool, working_set))
       .spawn(transport)
              │
              ▼  driver.poll() each cycle
   ┌────────────────────────────────────────────────────────┐
   │ FSM: discover → upload → activate → run                │
   │ inbound VT frames → VtEvent on the event stream        │
   │ your commands (set_value, …) → buffered, sent next tick│
   └────────────────────────────────────────────────────────┘
```

You point the client at a VT (`connect_to`), and once connected push updates with
`set_value` / `set_string` / etc. through fine control. Soft-key and value-change
events arrive on `poll()` (or `drain::<VtEvent>()`). The full walkthrough is in the
[Virtual Terminal client tutorial](../tutorials/virtual-terminal-client.md); the
server side (acting *as* the terminal) is in
[Virtual Terminal server](../tutorials/virtual-terminal-server.md).

machbus also ships a **renderer** and a VT *server*: it can play the terminal,
maintain the active pool, apply runtime commands, run macros, and report operator
input — useful for simulation, testing, and headless terminals.

## Auxiliary control (AUX)

Beyond the screen, ISO 11783-6 defines **auxiliary inputs** — physical joysticks
and switch banks that an operator assigns to implement functions. There are an older
(AUX-O) and a newer type-2 (AUX-N) scheme. machbus decodes the status messages and,
on the VT-version-5 path, the capability discovery that lets a client learn what
auxiliary channels a terminal offers.

## Failure modes worth knowing

- **Pool too big or malformed** — the VT rejects the upload or returns a pool
  error; nothing draws. Validate the pool and watch for `PoolError`.
- **Version mismatch** — commands or objects the terminal does not support fail
  silently or with a pool error; check the advertised version/capabilities first.
- **Lost activation** — another working set can become active; handle
  `ActiveWorkingSet` so you do not fight for the screen.
- **Editing races** — the operator may be editing a field while you push a new value
  to it; the protocol and a careful client reconcile who wins.
- **Reconnect** — if the VT drops, the client must re-discover and re-upload; the
  pool is not persisted on the terminal across a real power cycle.

## From concept to code

| You read about… | Build it with… | See… |
| --- | --- | --- |
| VT client (implement side) | `session::plugins::VtClient` | [Virtual Terminal client](../tutorials/virtual-terminal-client.md) |
| VT server (the terminal) | `session::plugins::VtServer` | [Virtual Terminal server](../tutorials/virtual-terminal-server.md) |
| Building an object pool | `isobus::vt` (or an `.iop` export) | [VT object pools](../tutorials/vt-object-pools.md) |
| Pushing UI updates | `VtClient::set_value` / `set_string` / … | [VT updates](../tutorials/vt-updates.md) |
| Auxiliary discovery | `VtClient` aux capabilities | [VT auxiliary capabilities](../tutorials/vt-auxiliary-capabilities.md) |

## What this proves / does not prove

machbus implements the VT client, server, renderer, and the object-pool codec, and
exercises them against real `.iop` pools and reassembled uploads. That proves the
*protocol* behavior locally; it does **not** prove pixel-accurate rendering on a
specific commercial terminal or AEF VT certification — see
[Conformity first](../conformity/index.md).

## See also

- [The Task Controller and the data dictionary](task-controller.md) — the other big
  implement-side service, usually running alongside the VT.
- [VT object pools](../tutorials/vt-object-pools.md),
  [VT updates](../tutorials/vt-updates.md),
  [VT auxiliary capabilities](../tutorials/vt-auxiliary-capabilities.md).
