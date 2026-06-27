# VT auxiliary control (AUX-N)

Auxiliary control lets a physical input device — a joystick, a multi-function
lever, a bank of switches — drive implement functions through the virtual
terminal. The classic example is a lever on the armrest mapped to "raise/lower
the hitch": the operator moves the lever, the input device reports the new
state, the VT routes it to the implement, and the implement actuates. This is
the **AUX-N** workflow defined in ISO 11783-6 (Virtual Terminal), and this page
explains the model, the capability exchange, the assignment that binds an input
to a function, and how to drive the pieces with `machbus`.

Read [VT concepts](../standards/virtual-terminal.md) first: auxiliary
control sits on top of an active VT session, an object pool, and a working set.
Everything here assumes the implement has already connected to a VT.

## Why this exists

An implement has functions an operator wants under their fingers — lift, fold,
section on/off, flow rate — but the implement has no buttons of its own. It
borrows them. A separate **auxiliary input device** (joystick or control panel)
offers a set of physical inputs, and the operator decides, through the VT's
configuration screen, which input drives which implement function. The VT is the
broker that learns both sides, lets the operator pair them, remembers the
pairing, and forwards live input changes to the implement.

This decouples controls from implements. One joystick can drive a planter today
and a sprayer tomorrow; one implement can be operated from whatever input device
happens to be in the cab. The price of that flexibility is a careful negotiation
so that an input never gets wired to a function it cannot safely drive.

## Mental model

```
 auxiliary input device            virtual terminal             implement
 (joystick / panel)                (broker + memory)            (working set)
 ──────────────────               ────────────────             ──────────────
 declares INPUTs        ─pool─►    holds both pools     ◄─pool─  declares FUNCTIONs
 (lever, switch...)                 + operator screen            (lift, fold...)

 operator pairs input ↔ function on the VT screen
                                        │
                                        ▼
                              VT stores the assignment
                                        │
 lever moves ─input status─►   VT routes to function  ─function status─► implement acts
```

Two object pools meet at the VT. The input device's pool contains auxiliary
**input** objects; the implement's pool contains auxiliary **function** objects.
The operator (or a stored preferred assignment) pairs them. After that, every
physical change on a bound input flows through the VT to the matching function.

## Functions versus inputs

The single most important distinction in AUX-N is *who provides what*.

| Concept | Provided by | What it represents | machbus object body |
| --- | --- | --- | --- |
| Auxiliary **function** | the implement | a thing the operator can command (lift, fold, rate) | `AuxFunction2Body` (new style) / `AuxFunctionBody` (classic) |
| Auxiliary **input** | the input device | a physical control the operator can move | `AuxInput2Body` (new style) / `AuxInputBody` (classic) |
| Auxiliary **control designator** | either side | optional operator-facing label/icon for an aux object | `AuxControlDesignatorBody` |

An implement never owns inputs and an input device never owns functions. They
advertise their halves in their object pools, and the VT joins them. The
`machbus` object types live in `isobus::vt::objects`, and the matching object
type tags are `ObjectType::AuxFunction2` (31) and `ObjectType::AuxInput2` (32)
for the new style, with `AuxFunction` (29) and `AuxInput` (30) for the classic
style and `AuxControlDesig` (33) for designators.

### The two styles

ISO 11783-6 defines an older auxiliary scheme and a newer one. `machbus` carries
both:

- **New style (AUX-N / type 2).** Function and input objects are
  `AuxFunction2Body` / `AuxInput2Body`; live status rides on
  `PGN_AUX_INPUT_TYPE2` and the setpoint range is the full `0..=65535`.
- **Classic style.** Function and input objects are `AuxFunctionBody` /
  `AuxInputBody`; live status rides on `PGN_AUX_INPUT_STATUS` with a setpoint
  range of `0..=10000` (0.0–100.0%).

New designs should use the new style. The classic types remain so `machbus` can
talk to older devices still on the bus.

## Anatomy: function and input types

Both a function and an input carry a **type** that classifies its behavior. The
type is what makes matching possible. `machbus` models it as `AuxFunctionType`
in `isobus::auxiliary`:

| Type | `AuxFunctionType` | Behavior |
| --- | --- | --- |
| 0 | `Type0` | Boolean on/off (a latched or momentary switch). |
| 1 | `Type1` | Variable speed (analog, e.g. a proportional lever). |
| 2 | `Type2` | Variable position (analog, absolute position). |

A function's live value is reported with an `AuxFunctionState` —
`Off`, `On`, or `Variable` — alongside a 16-bit setpoint. `machbus` derives the
state from the type and the setpoint with `auxiliary::derive_state`: a boolean
(`Type0`) is `On` for any non-zero setpoint and `Off` otherwise, while the analog
types are always `Variable`. That keeps the reported state consistent with the
value the device actually measured.

## Capability exchange: who advertises what

Before any pairing, each side has to know what the other offers, and the
application has to know what the VT itself supports. `machbus` gives you a small
pump-style helper, `AuxCapabilityDiscovery` in `isobus::vt::auxiliary_caps`, that
asks a VT which auxiliary objects it can handle.

The flow is request/response over the VT command channel:

1. You call `request_capabilities()`. It returns the 8-byte *Get Supported
   Objects* request payload to send on `PGN_ECU_TO_VT`, naming the new-style
   auxiliary object types in the request, and marks a request as pending.
2. The VT replies on `PGN_VT_TO_ECU`. You hand the inbound `Message` to
   `handle_response()`.
3. On a well-formed reply, the helper returns the populated `AuxCapabilities`
   and clears the pending flag. Each entry is an `AuxChannelCapability` carrying
   `channel_id`, `aux_type` (0 boolean / 1 analog / 2 bidirectional),
   `resolution` (step count for analog channels), and `function_type`.

A second `request_capabilities()` while one is already in flight returns an
error, so you cannot accidentally overlap two discoveries.

## Assignment workflow

Discovery tells you *what is possible*; assignment is *what is chosen*. The
binding of one input to one function is an **assignment**, and a remembered
default pairing is a **preferred assignment**.

```
implement pool loaded  ──►  functions visible to VT
input device pool loaded ─►  inputs visible to VT
                              │
              operator opens the VT aux config screen
                              │
              picks: input  ◄───►  function   (must be type-compatible)
                              │
              VT records the assignment ──► confirms to both sides
                              │
   input moves ──►  VT forwards value ──►  function acts on implement
```

The lifecycle of a single binding:

1. **Advertise.** Both pools are uploaded; the VT now holds the function and
   input objects with their types.
2. **Propose.** A preferred assignment may be offered at connect time so a known
   joystick comes up already wired the way the operator left it. If none
   applies, the operator pairs manually.
3. **Validate.** The VT checks the input type against the function type. If they
   are not compatible, the assignment is refused.
4. **Confirm.** A valid assignment is stored and acknowledged to both the
   implement and the input device, so each knows the binding is live.
5. **Operate.** Live input changes are forwarded as function status. For the new
   style this is `PGN_AUX_INPUT_TYPE2`; classic uses `PGN_AUX_INPUT_STATUS`.
6. **Release.** The operator can clear an assignment, or it falls away when a
   pool is unloaded or the session ends.

### How assignments are stored, recalled, and confirmed

An assignment is keyed by *identity*, not by position. A function is identified
by the implement's NAME plus the function's object, and an input by the input
device's NAME plus the input's object. That is why a preferred assignment
survives a power cycle: when the same NAMEs reappear, the VT recognises the pair
and restores the binding. Recall is the VT re-applying a stored preferred
assignment; confirmation is the VT telling both sides the binding now holds.

## Matching rules

A function can only be driven by a compatible input. The type field is the
gate:

- A boolean function (`Type0`) needs a boolean input. A latched/momentary switch
  can raise or lower a hitch; an analog lever cannot pretend to be a clean on/off.
- An analog function (`Type1` variable speed or `Type2` variable position) needs
  an analog input that can deliver a value across its range.
- A bidirectional input (`aux_type == 2` in the capability descriptor) can serve
  controls that need both directions from one physical axis.

`machbus` enforces the *value* side of these rules in its object bodies: encoding
or decoding an `AuxFunction2Body`, `AuxInputBody`, `AuxInput2Body`, or the classic
`AuxFunctionBody` rejects any type outside `0..=2`, and `AuxInput2Body` rejects an
`input_status` outside `0..=3`. A reserved or out-of-range type never makes it
onto the wire, so an obviously incompatible object is caught before it can be
assigned. The higher-level "does this input suit this function" decision is the
VT operator's to make on the configuration screen.

## Doing it with machbus

The auxiliary types are deliberately small, composable pieces. You assemble the
objects in your pool, run capability discovery against the live VT, and then
encode/decode live status frames.

Declare an implement function (new style) as an object body:

```rust
// Illustrative shape, not a compiled example.
use machbus::isobus::vt::objects::AuxFunction2Body;

let lift = AuxFunction2Body {
    function_type: 0,        // boolean on/off
    function_attributes: 0,
    name: name_object_id,    // a String/label object in the pool
    icon: icon_object_id,    // a Picture/icon object in the pool
};
let bytes = lift.encode()?;  // rejects function_type > 2
```

Discover what the VT supports before you rely on a binding:

```rust
// Illustrative shape, not a compiled example.
use machbus::isobus::vt::auxiliary_caps::AuxCapabilityDiscovery;

let mut discovery = AuxCapabilityDiscovery::new();
let request = discovery.request_capabilities()?; // 8-byte payload for PGN_ECU_TO_VT
// ... send `request`, then feed the VT's reply back in ...
if let Some(caps) = discovery.handle_response(&incoming_msg) {
    for ch in &caps.channels {
        // ch.channel_id, ch.aux_type, ch.resolution, ch.function_type
    }
}
```

Build and read a live function status frame with the `auxiliary` helpers:

```rust
// Illustrative shape, not a compiled example.
use machbus::isobus::auxiliary::{AuxNFunction, AuxFunctionType};

// Lever at half travel on a variable-speed function:
let frame = AuxNFunction::with_setpoint(7, AuxFunctionType::Type1, 0x8000);
let bytes = frame.encode();           // 8 bytes for PGN_AUX_INPUT_TYPE2

// Decoding an inbound status frame:
if let Some(status) = AuxNFunction::decode(&incoming_msg) {
    // status.function_number, status.r#type, status.state, status.setpoint
}
```

`with_setpoint` derives the `state` for you via `derive_state`, so a boolean
function reports `On`/`Off` and an analog one reports `Variable` without you
having to keep the two fields in sync. The classic style is the same shape with
`AuxOFunction` over `PGN_AUX_INPUT_STATUS`.

## Events and responsibilities

| Event | Who acts | Responsibility |
| --- | --- | --- |
| Capability response arrives | implement / input app | Decode with `handle_response`; cache only for this session. |
| Assignment confirmed | both sides | Treat the binding as live; begin forwarding/acting. |
| Input value change | input device | Send a status frame for the bound input. |
| Function status received | implement | Actuate to the new state/setpoint, or hold safe-state. |
| Assignment cleared / pool unloaded | both sides | Drop the binding; stop acting on the stale input. |

The application owns the safety decision. The stack moves the bytes; deciding
whether a received setpoint is safe to apply *right now* is yours.

## Edge cases and failures

- **Incompatible types.** A boolean input cannot drive an analog function and
  vice versa. `machbus` refuses reserved/out-of-range type and status values at
  encode/decode time; the VT refuses the pairing at the screen.
- **Lost assignment.** If a pool is unloaded, the session drops, or the operator
  clears the binding, the function is no longer driven. The implement must fall
  back to its safe-state, not freeze on the last value it saw.
- **Multiple input devices.** More than one joystick may be present. Each input
  is identified by its device's NAME, so assignments stay unambiguous, but the
  operator must not bind two inputs to the same function in a way that fights.
- **Latched versus momentary inputs.** A momentary input returns to off when
  released; a latched input holds its state. For a hitch this matters: a momentary
  "raise" stops when the operator lets go, while a latched switch keeps commanding
  raise until toggled. Choose the input type that matches how the function should
  behave when the operator stops touching it.
- **Stale or truncated capability responses.** `handle_response` returns `None`
  for a wrong command byte, wrong sub-function, a truncated channel list, or
  trailing bytes, and it leaves the request pending so a later valid reply still
  lands. Never treat a `None` as "no capabilities".

## Advanced

- **Preferred assignment persistence.** Because assignments are keyed by NAME and
  object identity, a VT can store a preferred set and restore it when the same
  devices reappear. Design your object identifiers to be stable across power
  cycles so the operator's choices survive.
- **Safe-state on aux loss.** Build the implement so that losing the binding
  forces functions to a defined, safe value. Tie this into the same safe-state
  logic used for VT loss; see
  [Shortcut button and safe state](../standards/implement-and-services.md).
- **Bidirectional inputs.** A capability channel with `aux_type == 2` advertises
  a control that delivers both directions on one axis. Match it only to functions
  that genuinely need both directions.
- **Classic interoperability.** Keep the classic `AuxFunctionBody` /
  `AuxInputBody` / `AuxOFunction` path available if you must talk to older
  devices; the new-style and classic frames travel on different PGNs and do not
  interfere.

## Validate locally

```sh
make test
```

The stack tests cover capability discovery against a virtual bus, including the
malformed-response filtering described above (wrong command byte, wrong
sub-function, truncated channel list, trailing bytes), and the auxiliary object
bodies and status frames round-trip in the unit tests under
`src/isobus/auxiliary.rs` and `src/isobus/vt/objects.rs`.

## What this proves / does not prove

Proves: `machbus` can build and parse the auxiliary function/input objects, build
a *Get Supported Objects* request, decode well-formed capability responses while
rejecting malformed ones, and round-trip live AUX-N and classic status frames in
software.

Does not prove: that a particular joystick and a particular implement will pair
and operate correctly on real hardware, or any conformance/certification claim.
Real deployment still needs official standards, real hardware, and
interoperability evidence.

## See also

- [Virtual terminal concepts](../standards/virtual-terminal.md) —
  the session, pool, and working-set model auxiliary control builds on.
- [Virtual terminal server](virtual-terminal-server.md) — the VT side that brokers
  and stores assignments.
- [Virtual terminal client](virtual-terminal-client.md) — the implement side that
  uploads function objects and acts on forwarded status.
- [Shortcut button and safe state](../standards/implement-and-services.md)
  — what to do when control is lost.
