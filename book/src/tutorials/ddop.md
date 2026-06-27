# DDOP — building a Device Description Object Pool

Before a Task Controller can log a single number from your implement or send it
a single rate command, the implement has to describe itself. That self
description is the **Device Description Object Pool** (DDOP): a typed, structured
inventory of what the machine *is* — its booms, sections, tanks and sensors —
and what measurable quantities each part produces or accepts. This tutorial
builds a DDOP from the ground up with `machbus`, explains every object kind, and
walks the serialize / validate / upload path the TC client uses.

If the VT object pool is how an implement draws itself on a terminal, the DDOP
is how it explains itself to a controller. No task data flows until the TC has a
DDOP it understands and has accepted.

## Why this exists

A Task Controller is generic. It does not ship with a built-in model of your
sprayer, spreader or seeder. It knows how to log values over time and position,
how to total them, and how to push setpoints — but only against a description
the implement provides. The implement is the authority on its own structure, so
it hands the TC a machine-readable map: "I am a sprayer with twelve boom
sections; section 3 sits 4.5 m to the left of my hitch; it reports an actual
application rate in litres per hectare and accepts a setpoint rate."

With that map, the TC can address process data by *element and meaning* instead
of by raw application bytes. It can decide which sections are inside a treatment
zone, scale a raw count into engineering units for display, and write a complete
task log that a farm management system can read back later. The DDOP is the
contract that makes all of that possible, and ISO 11783-10 (Task Controller)
defines its object kinds and how they relate.

## Mental model

A DDOP is a flat list of objects joined by identifier references into a tree.
The tree always has one **Device** at the root, fans out into **Device
Elements** for the physical or logical parts of the machine, and hangs
**Process Data** and **Property** objects off those elements. **Value
Presentation** objects sit to the side and are pointed at by process data or
properties when a raw integer needs scaling into a human unit.

```
Device  "Sprayer 600"                         (DVC — the machine)
  │
  ├─ DeviceElement  Device/root               (DET — whole-machine node)
  │     └─ DeviceElement  Function "Boom"      (DET — a logical assembly)
  │            ├─ DeviceElement Section 1       (DET — addressable part)
  │            │     ├─ DeviceProperty   Y offset = -4500 mm   (DPT, fixed)
  │            │     ├─ DeviceProperty   working width = 1000  (DPT, fixed)
  │            │     └─ DeviceProcessData actual rate, DDI →   (DPD, live)
  │            │                              └── Value Presentation (DVP)
  │            ├─ DeviceElement Section 2 …
  │            └─ DeviceElement Section N …
  │
  └─ (Value Presentations referenced by DDI-bearing objects)
```

Two things make the tree real: every object owns a unique **object ID**, and the
links are nothing more than IDs stored in other objects. An element names its
parent by ID and lists its children by ID; a process-data object names its value
presentation by ID. Get the IDs right and the tree is well formed; get one wrong
and the pool fails validation before it ever reaches the bus.

## Anatomy: the five object kinds

`machbus` exposes exactly five DDOP object types from
`isobus::tc`, each a plain struct with consuming `with_*` setters. The first
byte of every serialized object is its kind, modelled by `TCObjectType`.

| Object | machbus type | Wire tag | What it carries |
| --- | --- | --- | --- |
| Device | `DeviceObject` | DVC | The machine itself: designator, software version, serial, structure + localization labels. |
| Device Element | `DeviceElement` | DET | A node in the tree: a type, an element number, a parent, and a child list. |
| Process Data | `DeviceProcessData` | DPD | A live measurable: a DDI, trigger methods, an optional presentation. |
| Property | `DeviceProperty` | DPT | A fixed value baked into the description: a DDI and a constant `i32`. |
| Value Presentation | `DeviceValuePresentation` | DVP | Scale, offset, decimals and a unit string for turning raw values into engineering units. |

### Device (`DeviceObject`)

There is exactly one device at the root. Its designator is required — `machbus`
rejects an empty one — and it carries two seven-byte labels that drive caching
(covered below). You build it with `with_designator`, `with_software_version`,
`with_serial_number`, `with_structure_label` and `with_localization_label`.

### Device Element (`DeviceElement`)

Elements are the skeleton. Each has a `DeviceElementType` that tells the TC what
role it plays. The variants are `Device`, `Function`, `Bin`, `Section`, `Unit`,
`Connector` and `NavigationReference`. The whole-machine root is usually a
`Device` element; a boom or sub-boom is a `Function`; an individually
controllable boom segment is a `Section`; a hitch reference point is a
`Connector`. An element names its `parent_id` and lists the IDs of its
`child_objects` — those children may be other elements (deeper structure) or the
Process Data and Property objects attached to it.

### Process Data (`DeviceProcessData`)

This is the live wire. A process-data object says "this element can report or
accept *this* quantity," named by its **DDI**. It also declares the
`trigger_methods` that say when the value should be logged, and may point at a
Value Presentation. Triggers are a bitmask you OR together via
`with_trigger` using `TriggerMethod`: `TimeInterval`, `DistanceInterval`,
`ThresholdLimits`, `OnChange` and `Total`.

### Property (`DeviceProperty`)

A property is a *fixed* value that is part of the description rather than
something measured at runtime. Section geometry — X/Y/Z offsets and working
width — is expressed as properties, because those numbers do not change during a
task. The struct carries a DDI and a constant `i32` `value`. The helpers read
these to reconstruct implement geometry.

### Value Presentation (`DeviceValuePresentation`)

Raw DDI values are integers in defined base units. A presentation describes how
to display them: `scale`, `offset`, `decimal_digits` and a `unit_designator`
string. A process-data or property object references one by ID through
`with_presentation`; if it has none, the reference is the null ID `0xFFFF` and no
scaling is implied. The scale must be finite — `NaN` or infinity is rejected at
serialize and validate time.

## ElementNumber, DDI, and how a value becomes meaningful

Two fields turn an otherwise anonymous struct into a quantity the TC can act on.

**The DDI** (Data Dictionary Identifier) names *what the value means and in what
unit*. It is a 16-bit code from the ISO 11783-11 data dictionary, modelled as
`DDI`. One DDI may identify a volume-per-area setpoint rate; a different DDI may
identify an actual rate, a total volume, or a working width. The implement does
not invent meaning — it picks the DDI that matches the real quantity, and both
sides then agree on units and resolution. `machbus` ships named constants under
the `ddi` module (for example
`ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE`, `ddi::ACTUAL_WORKING_WIDTH`,
`ddi::DEVICE_ELEMENT_OFFSET_Y`) so you reference quantities by name instead of
by magic number. Use the constant whose meaning matches your value; this page
deliberately does not reprint the dictionary.

**The ElementNumber** (`ElementNumber`) distinguishes *which physical instance*
a value belongs to. When twelve sections all report the same DDI, the element
number is what the TC uses to tell section 1's rate from section 7's. Keep
element numbers stable and matched to the real-world part — they appear in the
task log and in section-control addressing.

Together: the DDI says "application rate, litres per hectare," the element number
says "section 7," and the value presentation says "raw count 7000 displays as
70.00 L/ha." That triple is the whole point of the DDOP.

## The builder workflow

A practical DDOP comes together in a predictable order:

1. Create the root `DeviceObject` with a designator and version.
2. Add a root `DeviceElement` (type `Device`) and, beneath it, `Function`
   elements for booms and `Section` elements for each controllable segment.
3. For each section, add `DeviceProperty` objects for fixed geometry (offsets,
   width) using the offset/width DDIs, and `DeviceProcessData` objects for the
   live rates and counts it reports or accepts.
4. Add `DeviceValuePresentation` objects for any value that needs scaling, and
   point the relevant process-data/property objects at them.
5. Wire the tree: set each element's `parent_id` and `child_objects` so the IDs
   form one connected hierarchy.
6. Validate, serialize, upload, then activate through the TC client.

`machbus` gives you two ways to add objects. The fluent `with_*` methods on
`DDOP` (`with_device`, `with_element`, `with_process_data`, `with_property`,
`with_value_presentation`) build a pool by chaining and are ideal for static
descriptions. The fallible `add_*` methods (`add_device`, `add_element`, …)
return the assigned `ObjectID` so you can capture an ID and reference it from a
later object — useful when you are generating sections in a loop. If you leave an
object's `id` at `0`, the pool allocates the next free identifier for you via
`next_id`; supply an explicit `with_id` when you want stable IDs across runs.

### Reading geometry and rates back

Once a DDOP exists, the `ddop_helpers` module turns its tree back into
application-friendly views. `extract_geometry` walks connector, boom and section
elements and reads the offset/width properties into an `ImplementGeometry` (with
`SectionInfo` and `SubBoomInfo` entries and a computed `total_width_mm`).
`extract_rates` and `extract_totals` filter process-data and property objects by
their DDI class into `RateInfo` records, marking which are editable process data
and which are fixed property constants. `section_count` and
`find_parent_element` round out the surface. These are the functions a TC-side
consumer uses to understand your implement; building your DDOP so they return
sane values is a good self-check.

## Building a DDOP with machbus

The TC client demo builds a minimal pool — one device and one root element — and
runs it through the full upload handshake. Here is the pool it builds:

```rust
{{#include ../../../examples/tc_client_demo.rs:22:34}}
```

A real sprayer extends that skeleton. The shape below is illustrative (not a
compiled snippet) and shows how sections, geometry properties, a live rate and a
value presentation hang together by ID:

```rust
// illustrative shape — see the helper-module tests for compiled equivalents
let ddop = DDOP::default()
    .with_device(DeviceObject::default().with_id(1).with_designator("Sprayer 600"))
    // a value presentation: raw count → L/ha at 0.01 resolution
    .with_value_presentation(
        DeviceValuePresentation::default()
            .with_id(30).with_scale(0.01).with_decimals(2).with_unit("L/ha"),
    )
    // section 1 geometry, expressed as fixed properties
    .with_property(
        DeviceProperty::default()
            .with_id(60).with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Y).with_value(-4500),
    )
    .with_property(
        DeviceProperty::default()
            .with_id(61).with_ddi(ddi::ACTUAL_WORKING_WIDTH).with_value(1000),
    )
    // section 1 live rate, scaled by the presentation above
    .with_process_data(
        DeviceProcessData::default()
            .with_id(62)
            .with_ddi(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE)
            .with_trigger(TriggerMethod::OnChange)
            .with_presentation(30)
            .with_designator("Rate"),
    )
    .with_element(
        DeviceElement::default()
            .with_id(3).with_type(DeviceElementType::Section)
            .with_number(1).with_parent(2)
            .with_designator("S1")
            .with_children(vec![60, 61, 62]),
    );
```

The element's `child_objects` list (`vec![60, 61, 62]`) is the wiring: it ties
the two geometry properties and the live rate to section 1. The rate points at
presentation 30 by ID, so a raw value of `7000` displays as `70.00 L/ha`.

## Validation before upload

Never upload a pool you have not validated. `DDOP::validate` checks the structure
the TC will rely on and fails fast with a descriptive error:

- **At least one device and one element.** An empty pool or a device with no
  elements is rejected outright.
- **Unique object IDs.** Two objects sharing an ID is a hard error; the TC
  cannot resolve an ambiguous reference.
- **Parent references resolve to a compatible kind.** An element's `parent_id`
  must point at an existing Device or Device Element, never at a process-data or
  presentation object.
- **Child references resolve to a compatible kind.** Listed children must be
  Device Elements, Process Data, or Properties — and must exist.
- **Presentation references resolve.** If a process-data or property names a
  presentation, that Value Presentation must exist; the null ID `0xFFFF` means
  "none" and is allowed.

`serialize` runs the serialization-level checks (`validate_serializable`) before
emitting bytes: designators and unit strings must be ASCII and fit the one-byte
length field, and every presentation scale must be finite. The current Rust
surface stores text as UTF-8 and refuses to emit non-ASCII or overlong strings
rather than produce bytes that cannot decode back, so keep designators short and
plain.

## Serializing, uploading, and caching by label

`DDOP::serialize` produces the byte pool the TC client transfers. On the wire the
client splits it across a transport session, the TC stores it, replies that the
pool was received, and the client then asks the TC to activate it. The demo
drives exactly that sequence: build pool → `set_ddop` → connect → version
exchange → DDOP transfer → object-pool response → activate. See
[Task Controller client](task-controller-client.md) for the full handshake.

Uploading a pool every power-up is wasteful, so the Device object carries two
seven-byte labels for caching:

- **Structure label** identifies the *shape* of the pool — the objects and their
  relationships. Change the structure and you must change this label so the TC
  knows its cached copy is stale.
- **Localization label** identifies the *language and unit presentation*. Change
  units, decimals or designator language and bump this label.

A TC that already holds a pool with matching labels can skip the transfer and
reuse its cached copy. The discipline that makes this work: **labels are a
version stamp**. If anything in the pool changes, change the corresponding label;
if nothing changes, keep the labels byte-for-byte identical across runs so the
cache hits. Random or timestamped labels defeat caching and make field traces
hard to compare.

## Edge cases and failure modes

- **Dangling references.** A child or parent ID with no matching object fails
  validation. This is the most common authoring bug — usually a typo in an ID or
  a forgotten object.
- **Duplicate IDs.** Easy to introduce when hand-assigning IDs across many
  sections. Validation catches it, but prefer the auto-allocator or a disciplined
  numbering scheme.
- **Wrong parent kind.** Pointing an element's parent at a process-data object is
  rejected; parents are only Devices or Device Elements.
- **Non-finite presentation scale.** A `NaN`/infinite scale is refused at both
  serialize and deserialize, so a corrupted pool cannot round-trip into a usable
  one.
- **Non-ASCII or overlong text.** Designators and unit strings outside ASCII, or
  longer than the one-byte length field allows, are rejected before any bytes go
  out.
- **Exhausted identifier space.** A pool that fills the 16-bit ID space cannot
  allocate another object; `next_id` returns the null ID and `add_*` errors.
- **Empty designator on the device.** Rejected immediately — the root must be
  named.

## Advanced

- **Large DDOPs.** A real machine can carry hundreds of objects. The pool grows
  linearly and serializes in one pass; the cost is the transport upload, which is
  exactly why label-based caching matters. Validate once at build time, not on
  every connect.
- **Multi-boom geometry.** A wide implement can split into sub-booms. Model the
  main boom as a `Function` element and each sub-boom as a `Function` element
  whose parent is the main boom, with `Section` elements beneath each sub-boom.
  `extract_geometry` understands this nesting and fills the `sub_booms` list with
  per-sub-boom sections and rates.
- **Connector / navigation reference.** Use a `Connector` element with an X-offset
  property to anchor the implement to the hitch, and a `NavigationReference`
  element where guidance needs a defined reference point. The geometry helper
  reads the connector offset into `connector_x_mm`.
- **Stable IDs vs auto-allocation.** Auto-allocation is convenient for generated
  pools; explicit `with_id` is better for products you will field-debug, because
  stable IDs make TC traces and cached-pool comparisons readable.

## Validate locally

```sh
make run EXAMPLE=tc_client_demo
make run EXAMPLE=tc_geo_demo
make test
```

`tc_client_demo` builds a DDOP, sets it on the client, and runs the connect →
version → transfer → activate handshake entirely in software, ending in the
`Connected` state. `tc_geo_demo` exercises the geometry/prescription side. The
DDOP, objects and helper modules carry unit tests (round-trip serialize/
deserialize, validation of bad parent/child references, geometry and rate
extraction) that `make test` runs.

## What this proves / does not prove

Proves: a DDOP built with the `machbus` types serializes to a byte pool,
round-trips through deserialize unchanged, rejects malformed structure and text,
and uploads through the TC client handshake to an accepted, activated state in
software. The geometry and rate helpers read a well-formed pool back into
application views.

Does not prove: that a specific third-party Task Controller accepts the pool,
that the chosen DDIs and units interoperate with a particular FMIS, or any
conformance or certification claim. Real deployment still needs official
standards, real hardware, and interoperability evidence.

## See also

- [DDOP and process data](../standards/task-controller.md) — the
  conceptual primer on object kinds and DDIs.
- [Task Controller client](task-controller-client.md) — the connect and upload
  handshake that ships the pool.
- [TC-GEO prescription](tc-geo-prescription.md) — using a DDOP's geometry and
  DDIs against a prescription map.
- [TC/DDOP problems](../troubleshooting/tc-ddop.md) — when a pool is rejected or
  a cache goes stale.
