# VT object pools

An ISOBUS implement does not paint pixels on the tractor's display. Instead it
ships the terminal a complete description of its user interface — every mask,
button, number field, bar graph, and icon — as a tree of typed objects called an
**object pool**. The Virtual Terminal owns the screen and the rendering; the
implement owns the *description*. This tutorial explains why that split exists,
walks the object families that make up a pool, and shows how to build, validate,
and serialize one with `machbus` using the real `isobus::vt` types.

If you have not yet read [Working sets and object
pools](../standards/virtual-terminal.md) or [Virtual Terminal
concepts](../standards/virtual-terminal.md), start there: this page
assumes you know that a terminal hosts many implements at once and that each
implement uploads its pool before it can show anything.

## Why a pool exists

A tractor cab has one display but many implements may want to use it — a sprayer,
a baler, a seeder — sometimes in the same season, sometimes swapped within
minutes. None of those implement vendors can ship code that runs *on* the
terminal; terminals come from a different set of vendors entirely. So ISOBUS
inverts the usual arrangement. The implement does not run UI code on the screen.
It describes its whole UI declaratively and uploads that description once. The
terminal then renders it, handles touch and soft-key input locally, and only
tells the implement when something the implement cares about happens (a button
press, an edited value).

That description is the object pool. Because it is data, not code, the same pool
renders on a small monochrome terminal and a large colour one; the terminal
adapts. And because the implement keeps the canonical copy, it can change values
at runtime by sending small commands rather than re-uploading the screen. The
pool is the contract: get it right and the implement's UI appears; get it wrong
and the terminal rejects it before a single frame is drawn.

## Mental model

A pool is a flat list of objects, but the objects form a **tree by reference**.
Each object has a unique 16-bit ID; parents name their children by ID. The root
is always the **Working Set** — the implement's identity on the terminal — and
the masks below it are the full-screen "pages" the operator sees.

```
Working Set (id, the implement's root)
├── Data Mask           ── a normal full-screen page
│   ├── Output Number   ── a live readout (references a Number Variable)
│   ├── Output String   ── a label (references a String Variable + Font)
│   ├── Button          ── a soft target the operator can press
│   │   └── Output String / Picture  (the button's face)
│   ├── Container       ── groups objects, can be shown/hidden as a unit
│   │   ├── Line / Rectangle / Ellipse / Polygon   (drawn shapes)
│   │   └── Meter / Linear Bar Graph / Arched Bar Graph
│   └── Input Number    ── an editable field (references a Number Variable)
├── Alarm Mask          ── a page the terminal forces to the front on alarm
│   └── ... (its own child objects + a referenced Soft Key Mask)
└── Soft Key Mask       ── the row of physical/soft keys for a mask
    └── Key
        └── Output String / Picture  (the key's face)
```

The leaf objects that show data (numbers, strings, bars) do not store their value
directly. They point at a **variable** object (Number Variable or String
Variable). To change what the screen shows, the implement updates the variable;
every object that references it redraws. That indirection is the whole reason a
running pool needs so few bytes on the wire — covered in [VT
updates](vt-updates.md).

## Anatomy of the object families

`machbus` models every declared VT object type in `isobus::vt`. The discriminant
is `ObjectType`, a `#[repr(u8)]` enum whose byte values match ISO 11783-6. Each
type has a typed body struct (`DataMaskBody`, `ButtonBody`, …) with `encode` /
`decode`, plus a `VTObject` container that carries the ID, the type, the encoded
body bytes, and a list of child IDs. Group the types by what they do.

### Structural objects (the skeleton)

These define the page hierarchy and the regions on screen.

| Type | machbus body | Role |
| --- | --- | --- |
| `WorkingSet` | `WorkingSetBody` | The implement's root. Exactly one per pool. Children are masks. |
| `DataMask` | `DataMaskBody` | A normal full-screen page; names a Soft Key Mask. |
| `AlarmMask` | `AlarmMaskBody` | A page the terminal raises on an alarm; carries priority and an acoustic-signal hint. |
| `SoftKeyMask` | `SoftKeyMaskBody` | The set of soft keys shown alongside a mask. |
| `Container` | `ContainerBody` | Groups child objects so they can be moved or hidden together. |
| `WindowMask` | `WindowMaskBody` | A reusable framed region (the version-6 "auxiliary" window family). |
| `KeyGroup` | `KeyGroupBody` | Groups keys for the version-6 key arrangement. |
| `Key` | `KeyBody` | One soft key; its `key_code` is what the terminal reports on press. |

The Working Set sits at the top because the terminal needs one well-known entry
point. Its body carries no fixed fields in this representation — its meaning is
entirely its child list, which must reference at least one Data Mask or Alarm
Mask.

### Input objects (the operator talks back)

These accept operator input. The terminal manages the editing UI; the implement
just learns the final value.

| Type | machbus body | Edits |
| --- | --- | --- |
| `InputBoolean` | `InputBooleanBody` | A toggle, backed by a Number Variable. |
| `InputNumber` | `InputNumberBody` | A bounded number with scale/offset/decimals and a min/max range. |
| `InputString` | `InputStringBody` | Free text with an optional validation character set. |
| `InputList` | `InputListBody` | A pick-one list whose items are other object IDs. |

An Input Number, for example, carries `min_value`/`max_value` (encode rejects a
min above the max), an integer `scale` and `offset`, a decimal count, and a
display `format`. It references a Font Attributes object, an optional Input
Attributes object, and the Number Variable it reads and writes.

An Input List has the same value-source split as the list-style output objects:
if `variable_reference` points at a Number Variable, that variable supplies the
selected index; if it is `NULL`, the `value` byte in `InputListBody` is the
inline selected index, with `255` meaning no chosen item.

### Output objects (the implement shows data)

These are read-only on the terminal. Numbers and strings render a value; the
shape and graph objects draw geometry.

| Type | machbus body | Draws |
| --- | --- | --- |
| `OutputNumber` | `OutputNumberBody` | A formatted number from a Number Variable. |
| `OutputString` | `OutputStringBody` | Text from a String Variable (or an inline value). |
| `Line` | `OutputLineBody` | A straight line with a referenced Line Attributes. |
| `Rectangle` | `OutputRectangleBody` | A box with line + fill attributes and per-side line suppression. |
| `Ellipse` | `OutputEllipseBody` | A circle/arc; `ellipse_type` selects closed/segment/section. |
| `Polygon` | `OutputPolygonBody` | A closed or open shape; needs at least three points. |
| `Meter` | `MeterBody` | A round gauge with a needle, driven by a Number Variable. |
| `LinearBarGraph` | `LinearBarGraphBody` | A straight fill bar with an optional target line. |
| `ArchedBarGraph` | `ArchedBarGraphBody` | A curved fill bar. |

The graph and gauge bodies all carry a `min_value`/`max_value` window and a
`variable_reference` ID; `encode` enforces the min-below-max rule and that
angle fields stay in the half-degree range the wire allows.

### Graphics and attribute objects (look and feel)

These do not appear on their own; other objects reference them.

| Type | machbus body | Provides |
| --- | --- | --- |
| `PictureGraphic` | `PictureGraphicBody` | A bitmap (1/4/8-bit indexed, optionally RLE). |
| `ObjectPointer` | `ObjectPointerBody` | An indirection — renders whatever object it points at. |
| `FontAttributes` | `FontAttributesBody` | Colour, size, type, and style for text. |
| `LineAttributes` | `LineAttributesBody` | Colour, width, and dash pattern for strokes. |
| `FillAttributes` | `FillAttributesBody` | Fill colour or pattern for closed shapes. |
| `InputAttributes` | `InputAttributesBody` | A valid/invalid character set for input strings. |
| `NumberVariable` | `NumberVariableBody` | A shared 32-bit value that output/input numbers read. |
| `StringVariable` | `StringVariableBody` | A shared text value that output/input strings read. |
| `ColourMap` / `ColourPalette` | `ColourMapBody` / `ColourPaletteBody` | Colour remapping (the palette form is version 6). |

The two variable types are the hinge of the whole runtime model: many objects can
point at one variable, so one update redraws all of them.

Colour Maps are tied to VT graphics depth: their record carries a two-byte
entry count and the standard sizes are 2, 16, or 256 entries.

### Behaviour and auxiliary objects

| Type | machbus body | Role |
| --- | --- | --- |
| `Macro` | `MacroBody` | A recorded list of VT commands the terminal runs on an event. |
| `AuxFunction` / `AuxInput` | `AuxFunctionBody` / `AuxInputBody` | The classic auxiliary-control objects. |
| `AuxFunction2` / `AuxInput2` | `AuxFunction2Body` / `AuxInput2Body` | The version-2 auxiliary family. |
| `AuxControlDesig` | `AuxControlDesignatorBody` | A designator that names an aux function or input. |
| `Animation` | `AnimationBody` | A timed sequence of picture/object frames. |
| `GraphicContext` / `GraphicsContext` | `GraphicContextBody` / `GraphicsContextBody` | A drawable canvas with a viewport. |

Auxiliary objects connect an implement function to a physical control (a joystick
button, a switch) that the operator assigns on the terminal. See [VT auxiliary
capabilities](vt-auxiliary-capabilities.md) for how that pairing works.

A handful of version-6 types round out the enum — `ExternalObjectDefinition`,
`ExternalReferenceName`, `ExternalObjectPointer`, `ScaledGraphic`,
`ScaledBitmap`, `GraphicData`, and `ObjectLabelRef`. They let one pool reference
objects defined by another working set and scale graphics; `machbus` carries
body structs for all of them so a real-world pool round-trips without loss.
`GraphicData` is the standard PNG-payload object; the hosted renderer can draw a
small deterministic subset as RGBA image commands and reports unsupported PNG
compression/format variants as explicit placeholders. `ScaledGraphic` uses the
standard Width, Height, ScaleType, Options, and Value fields; ScaleType selects
the scaling mode plus horizontal/vertical justification, while Value names a
`GraphicData`, `PictureGraphic`, `ObjectPointer`, or NULL graphic source. When
Value names an `ObjectPointer`, the pointer chain must still resolve to NULL,
`GraphicData`, or `PictureGraphic`; non-graphic targets and pointer cycles are
rejected by pool validation and by runtime retarget commands.

## ObjectID: uniqueness and references

Every object is identified by `vt::ObjectID`, a `#[repr(transparent)]` newtype
over `u16`. It is deliberately distinct from the Task Controller's object ID type
so the two cannot be mixed up. The reserved value `ObjectID::NULL` (`0xFFFF`)
means "no object" — a field set to NULL is simply absent, which is how an alarm
mask says it has no soft-key mask, or an output number says it has no variable.

Two rules govern IDs:

1. **Uniqueness.** No two objects in a pool may share an ID. `ObjectPool::add`
   enforces this and refuses a duplicate, so you cannot accidentally build an
   ambiguous pool.
2. **References are by ID, not by pointer.** A Data Mask names its Soft Key Mask
   by ID; a Meter names its Number Variable by ID; a Button lists its face
   objects in its child list. Every such reference must resolve to an object of
   the *right type* that actually exists in the pool. A reference left at
   `ObjectID::NULL` is treated as intentionally empty and is allowed.

## Validate before you upload

A terminal will reject a malformed pool, but it is far cheaper to catch the
problem locally first. `machbus` validates at two layers.

**Raw IOP parsing.** A pool exported from a design tool arrives as an `.iop`
byte buffer. `net::parse_iop_data` walks it object header by object header
(`[id:2][type:1][width:2][height:2]` then the type body) and returns a list of
`RawIopObject`s, or fails. It is strict: an empty or short buffer, a trailing
partial header, or a body that runs past the end of the input all return an
`InvalidData` error rather than silently decoding a prefix. `net::validate` is
the boolean form — it returns `true` only if the whole buffer walks to a clean
end.

**Structured pool validation.** Once you have a `vt::ObjectPool` (built directly
or via `ObjectPool::deserialize`), `ObjectPool::validate` checks the object
*graph*:

- exactly one Working Set exists (zero or several is an error);
- the Working Set references at least one Data Mask or Alarm Mask;
- every object's body decodes for its declared type (`validate_body`);
- typed references resolve to the right type — a Data Mask's soft-key mask must
  actually be a Soft Key Mask, an Output Number's variable reference must be a
  Number Variable, and so on;
- no child reference points at a non-existent object.

`deserialize` itself rejects an unknown object-type byte and a malformed
child-list tail with a `PoolValidation` error, so importing a corrupt pool fails
at parse time rather than at render time. The failure classes you will actually
hit are: unknown type, duplicate ID, a missing or wrong-typed child or variable
reference, a body too short for its type, and a pool whose serialized object
exceeds the body-length field.

## Building a pool with machbus

Build objects with `VTObject` plus a typed body helper, or with one of the
`create_*` convenience functions, then add them to an `ObjectPool`. The shape,
using the real API:

```rust
use machbus::isobus::vt::{
    ObjectPool, DataMaskBody, NumberVariableBody, OutputNumberBody,
    create_data_mask, create_number_variable,
};

let mut pool = ObjectPool::default();

// Root + one page.
pool.add(working_set)?;                       // references the data mask by child id
pool.add(create_data_mask(1000, &DataMaskBody::default()))?;

// A shared value and a readout that points at it.
pool.add(create_number_variable(2000, &NumberVariableBody { value: 0 }))?;
let readout = machbus::isobus::vt::VTObject::default()
    .with_id(3000u16)
    .with_type(machbus::isobus::vt::ObjectType::OutputNumber)
    .with_output_number_body(&OutputNumberBody {
        variable_reference: 2000u16.into(),
        ..Default::default()
    })?;
pool.add(readout)?;

pool.validate()?;                             // catch graph errors before upload
let bytes = pool.serialize()?;                // ready for the upload sequence
```

The body helpers come in two flavours. Helpers for types that cannot encode an
invalid body (`with_data_mask_body`, `with_number_variable_body`) return `Self`
directly. Helpers whose body has validity rules (`with_output_number_body`,
`with_input_number_body`, `with_meter_body`, …) return `Result<Self>`, because
`encode` rejects out-of-range options, bad angle values, or a min above the max
*at construction time* — you never get a half-built object onto the wire.

Authoring advice that keeps pools manageable:

1. Start with the smallest useful pool: a Working Set, one Data Mask, one visible
   output object.
2. Add a Soft Key Mask only when you need keys.
3. Only then add inputs, macros, auxiliary objects, and graphics.
4. Keep a byte fixture for every pool that has ever caused a failure — a fixture
   replays in a test, where a screenshot cannot.

## Serializing a pool to bytes

`VTObject::serialize` writes the length-driven wire form: the 16-bit ID, the
type byte, a 16-bit body length, then the body bytes. For object types that have
children (Working Set, masks, containers, keys, buttons, …) the child list — a
16-bit count followed by that many IDs — is appended and *counted inside* the
body-length field, so the whole object is self-delimiting. `ObjectPool::serialize`
simply concatenates every object's serialization in order; the result is the byte
stream the upload sequence pushes to the terminal.

`ObjectPool::deserialize` is the inverse: it reads each header, validates the
type byte, splits the body from the child-list tail at the known offset for that
type, and rebuilds the `VTObject`. Because the body length is explicit, a
truncated or oversized object is caught immediately.

## Edge cases and failure modes

- **No Working Set, or more than one.** `validate` rejects both. A pool needs
  exactly one root.
- **Working Set with no mask child.** A root that points at nothing the operator
  can see is rejected — the Working Set must reference at least one Data Mask or
  Alarm Mask.
- **Dangling or mistyped reference.** Pointing an Output Number at an object that
  is not a Number Variable, or at an ID that is not in the pool, fails
  validation. A `NULL` reference is fine; a wrong one is not.
- **Body too short.** Each `decode` checks the minimum length for its type and
  returns `InvalidData`. A 3-byte buffer where a Data Mask expected 3 bytes is
  fine; one byte short is rejected.
- **Reserved bits set.** Bodies with option bitfields reject reserved bits in
  `encode` and `decode`, so a pool that sets undefined options never reaches the
  terminal.
- **Pool too large.** A single object whose body plus child tail exceeds the
  16-bit length field is rejected at `serialize`. Terminals also bound total
  pool size; an oversized pool is a deployment concern, not just an encoding one.
- **Unknown type byte on import.** `deserialize` refuses an object type it does
  not recognise rather than guessing, so a pool built for a newer feature set
  fails loudly.

## Advanced

- **Versioning.** A pool carries a `version_label`, and `net::hash_to_version`
  derives a stable short string from the raw bytes. Terminals can cache a pool by
  version and skip re-upload when the label matches — change the pool, change the
  label.
- **Object pointers and indirection.** An `ObjectPointer` renders whatever object
  its `value` names, so you can swap a whole sub-tree at runtime by repointing one
  object instead of editing many. The version-6 external-object types extend this
  across working sets.
- **Language and localisation.** String content lives in String Variable objects
  and inline output-string values, separate from layout, so a pool can present
  different text per language without changing its structure. Font Attributes and
  the colour-map/palette objects keep styling out of the layout objects too.
- **Containers for show/hide.** Grouping objects under a Container lets the
  implement hide or move a whole region with one command, which is cheaper than
  touching each child.

## Validate locally

```sh
make run EXAMPLE=iop_parser_demo
make test
```

The `iop_parser_demo` example synthesizes a tiny IOP buffer, confirms it
validates, parses it object by object, and prints the derived version string:

```rust
{{#include ../../../examples/iop_parser_demo.rs:12:36}}
```

`make test` exercises the full `vt::objects` suite — body encode/decode round
trips, the duplicate-ID guard, graph validation, and serialize/deserialize — plus
the IOP parser's strict-rejection property tests.

## What this proves / does not prove

Proves: `machbus` models every declared VT object type, enforces ID uniqueness
and typed references, encodes and decodes bodies losslessly, and rejects
malformed pools — both as raw IOP bytes and as a structured object graph —
before they would ever reach a terminal.

Does not prove: that a given terminal renders your pool the way you expect, that
your pool fits a particular terminal's size and colour limits, or any
conformance or certification claim. `machbus` is not certified; real deployment
still needs official standards, real terminal hardware, and interoperability
evidence.

## See also

- [Virtual Terminal concepts](../standards/virtual-terminal.md) —
  what a terminal is and how it hosts many implements.
- [Virtual Terminal client](virtual-terminal-client.md) — uploading a pool and
  handling terminal events.
- [VT updates](vt-updates.md) — changing values at runtime through variables and
  commands.
- [VT auxiliary capabilities](vt-auxiliary-capabilities.md) — pairing aux
  objects to physical controls.
- [VT upload problems](../troubleshooting/vt-upload.md) — when a pool is rejected.
