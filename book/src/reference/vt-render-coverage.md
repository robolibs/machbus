# VT render coverage

This page is the repo-owned coverage ledger for the ISO 11783-6 Virtual
Terminal renderer. It is deliberately a claim boundary, not a certification
statement.

The current renderer is a retained command-list renderer with an optional
software framebuffer consumer. Hosted backends share the small
`VtRenderBackend` scene-consumer contract:

```text
ObjectPool -> LayoutEngine -> Scene -> GtuiRenderer -> RenderCommand[]
                                                └── FramebufferRenderer -> RGB pixels
VtRenderRuntime -> RenderCommand[] -> FramebufferRenderer -> RGB pixels
```

That is enough to load a pool, choose a mask, lay out many common objects, and
produce deterministic draw commands for a host backend. The hosted
`FramebufferRenderer` can also rasterise those commands into an RGB buffer for
snapshots or framebuffer experiments, then export packed RGB888 or RGB565 bytes
for display-driver handoff. Scenes retain the effective palette produced by the
base renderer palette, active Colour Palette object, and active Colour Map, so
mask/group background indices, Graphics Context colours, and hosted
Picture/Scaled Graphic framebuffer expansion use the same palette selection
rather than falling back to the repo default approximation. When callers have a
live `VtRenderRuntime`,
`FramebufferRenderer::render_runtime` uses the runtime command stream so
runtime-only Graphics Context replay/primitive expansion is included. Backends
consume `Scene`/runtime command output; they do not parse object pools or own
VT protocol state. This is still not a calibrated pixel/font/bitmap VT
terminal, and profile/display calibration remains future backend work.

## Status vocabulary

| Status | Meaning |
|---|---|
| `drawable` | The object becomes a scene node and draw command. |
| `interactive` | The object is drawable and also participates in input/focus handling. |
| `soft-key` | The object is resolved into the soft-key area rather than the data-mask node list. |
| `reference-resolved` | The object is consumed as value, style, palette, label, or pointer metadata. |
| `parsed-but-not-rendered` | The object model exists, but faithful visual output is still missing or placeholder-only. |
| `missing-object-model` | The object family is in the render inventory but has no `ObjectType` model yet. |
| `out-of-scope` | The renderer intentionally does not draw this family; another VT protocol layer owns it. |

## Current ledger

The machine-readable ledger lives at:

```text
book/src/reference/assets/vt_object_render_coverage.csv
```

The ledger now includes first-class rows for the formerly missing standard
families:

- `OutputList` is parsed, resolves its selected index from its inline value or
  Number Variable, materialises selected drawable items as scene nodes clipped
  to the Output List rectangle, including selected `Key` objects as
  display-only key designators, updates that rectangle through Change Size,
  treats the one-byte Value AID as Get-Attribute-only/read-only for Change
  Attribute while accepting Change Numeric Value for selected-index changes,
  follows the standard blank/no-display cases without fabricating an
  index/count fallback label, and draws compact text for simple selected
  `OutputString`/`OutputNumber` item values.
  Selected
  `ObjectPointer` entries may also target an
  `ExternalObjectPointer`; when the host-registered external pool grants
  access, the resolved external object is materialised inside the Output List
  clip, while an unavailable external item with no local default stays blank
  instead of creating an unsupported placeholder. Upload validation, Change List
  Item replay, and Object Pointer
  retargeting now reject Output List item references to style/reference
  metadata objects that cannot be presented.
- `OutputNumber` and `InputNumber` fixed-field Change Attribute replay now
  includes the raw Value AIDs as well as formatting/geometry fields, so
  variable-reference-NULL numeric fields redraw from the new inline value.
  Their formatted text also rejects non-finite scale values before retained
  mutation, rejects decimal counts outside the standard `0..=7` range, rejects
  the reserved non-standard hexadecimal format selector, and observes the
  standard fixed/exponential, leading-zero, zero-as-blank, and truncate option
  bits.
- `InputAttributes` and `ExtendedInputAttributes` are parsed as reference
  metadata and enforced for hosted `InputString` edits. Change String Value
  against an `InputAttributes` object updates its validation string and rebuilds
  the hosted input-validation state without increasing the fixed validation
  string length; shorter transfers are padded with spaces. When an Input String
  references a String Variable, classic Input Attributes are applied only to
  8-bit strings and Extended Input Attributes only to ISO WideStrings. Classic
  validation remains byte-oriented in the hosted edit path, so blacklist rules
  cannot admit non-single-byte characters into an 8-bit field, while Extended
  Input Attributes enforce both whitelist and blacklist code-plane ranges.
  Validation type is readable through Get Attribute Value, but it is not
  Change Attribute mutable for either validation-reference object.
  Extended Input Attributes reject duplicate code-plane records during encode
  and decode so each Unicode plane is represented at most once.
- `WorkingSetSpecialControls` applies initial colour-map and colour-palette
  selections before first render, updates retained colour selections through
  Select Colour Map/Palette and Change Attribute, and exposes advertised
  language/country pairs on `Scene` so a host VT can make a deterministic
  language-selection decision; pool validation rejects malformed language pairs
  before render/runtime use while accepting the standard two-space country
  not-applicable sentinel. Scene language matching now treats that sentinel as
  language-only support in either the advertised pair or the host query, while
  still requiring exact country matches when both sides provide country codes.
  `Scene::select_language` gives hosts a deterministic first-preference match
  with exact-country matches preferred over language-only fallback.
  The object parser also honours the standard
  number-of-bytes-to-follow extension shape: known VT6 language pairs are
  decoded, unknown trailing extension bytes are preserved and skipped so the
  next object in the pool is still found, and Get Attribute Value AID 1 reports
  the retained byte count including those extension bytes. Server Get Attribute
  Value AID 2 and AID 3 report the live retained colour selection, so a later
  Select Colour Map/Palette command clears or replaces earlier WSSC Change
  Attribute overlays instead of returning stale colour references.
- `ColourPalette` uses the ISO 11783-6:2018 Table B.73 body shape:
  reserved `Options`, a two-byte ARGB-entry count, and repeated little-endian
  ARGB entries in B,G,R,A byte order. Pool decoding rejects nonzero reserved
  options, count/body mismatches, and counts above the 256-entry palette range;
  Change Attribute admits only the reserved Options AID with value zero. The
  hosted palette overlay applies entries from colour index 0 upward; alpha is
  retained in the object model but ignored by the current opaque RGB renderer.
- AUX object families (`AuxFunction`, `AuxInput`, `AuxFunction2`,
  `AuxInput2`, and `AuxControlDesignator`) are deliberately `out-of-scope` for
  retained rendering. They are object-pool/protocol state used by auxiliary
  routing and assignment; they are not data-mask draw nodes or placeholder
  graphics to complete in the render backend.
- `WorkingSet` now uses the standard object/macro/language tail order for
  serialization and pool walking. Its two-byte language-code list is parsed,
  validated as ASCII letter pairs, and exposed as language-only
  `SceneLanguage` entries unless `WorkingSetSpecialControls` supplies one or
  more language/country pairs, in which case the special-controls list
  supersedes the Working Set list.
- `ObjectLabelRef` is parsed as a counted object-label list, validated for one
  list per pool, unique labelled targets, String Variable label references,
  standard Annex K font-type values, and output/drawable graphic-designator
  references, and materialised as runtime metadata for VT popup/editor labels.
- `ExternalObjectDefinition`, `ExternalReferenceName`, and
  `ExternalObjectPointer` now use the standard VT5 record shapes: enabled/NAME
  metadata for definition/reference objects, counted object IDs on
  definitions, and default-object/reference-NAME/external-object-id fields on
  pointers. Change Attribute updates the mutable metadata fields, Change List
  Item updates External Object Definition object-list entries, and the
  four-byte External Object Pointer Change Numeric Value payload updates both
  the External Reference NAME object ID and the referenced external object ID;
  Change Attribute AID 2 may set the External Reference NAME to NULL to restore
  the local default fallback. Hosted layout and live render runtimes can be
  given the local Working Set NAME plus
  referenced Working Set pools; an External Object Pointer placed in the scene
  then materialises the referenced object only when the local External Reference
  NAME is enabled, the registered referenced pool NAME matches, an enabled
  External Object Definition grants that local NAME access to the requested
  object, and the target object exists. Live runtime registration is keyed by
  the referenced NAME, so refreshed pools replace stale copies, and hosts can
  unregister a referenced pool on disconnect. Local Working Set NAME changes
  revalidate external grants, repeated same-NAME announcements, identical
  referenced-pool registrations, and missing-pool unregisters are no-ops, and
  referenced-pool register/unregister events respect active-mask locks: the
  resolver state changes immediately, but a locked active mask keeps its
  current pixels until unlock or timeout materialises the external/default
  object swap. If any check fails, or if the validated external target is an
  `ObjectPointer` chain ending at NULL, the pointer draws its local default
  object. A NULL local default draws nothing without creating an unsupported
  placeholder, matching the same standard no-object handling used for local
  `ObjectPointer` NULL values.

The object-type bytes were also aligned to ISO 11783-6:2018 Table A.1 for the
VT4/VT6 range: Graphics Context is type 36, Output List is 37, Extended Input
Attributes is 38, Colour Palette is 45, Graphic Data is 46, Working Set Special
Controls is 47, and Scaled Graphic is 48. The repo still carries
`ScaledBitmap` and `GraphicsContext` compatibility-extension rows; those are not
full standard-rendering claims.

The legacy plural `GraphicsContext` row is scoped as an machbus compatibility
extension rather than an ISO 11783-6 completion target. The standard graphics
context family is the singular `GraphicContext` row.

The standard `GraphicContext` row now exposes its visible canvas surface,
Change Background Colour, including macro replay, updates the opaque canvas
backing colour and the canvas command's initial indexed background, accepted
subcommands expand to backend-neutral primitives, and Copy
Canvas/Viewport can emit indexed Picture Graphic pixel payloads when the
bounded software canvas can replay the supported point/line/rectangle/ellipse/
polygon/text-cell/Draw-VT-object subset. Replay honours the placed scene
position of the `GraphicContext`, while Copy Viewport samples from the canvas
viewport instead of using placed screen coordinates and applies the current
viewport zoom to the copied pixel payload. Full persistent pixel-raster
semantics still need a real raster/graphics backend before a full VT display
claim is safe.

`PictureGraphic` now emits backend-neutral indexed-image commands for
uncompressed payloads and valid count/value RLE payloads whose decoded byte
length covers the standard source dimensions, using per-row byte strides for
packed 1-bit and 4-bit rows. Unused packed bits at the end of each row are
ignored instead of becoming pixels in the next row. Short decoded payloads stay
explicit placeholders; extra bytes beyond the expected row-padded bitmap
length are ignored/truncated as the Picture Graphic raw-data rule requires
instead of inventing additional pixels. Its target display width drives the
render rectangle while the actual bitmap dimensions remain the source image
dimensions in the command. The command stream carries the
transparent/opaque option separately from the transparency colour index, so
opaque pictures still draw pixels that happen to equal the transparency colour.
Change Attribute updates to Picture Graphic options preserve the uploaded
raw/RLE data-shape bit and only change the runtime transparent/flashing bits,
so command replay cannot reinterpret the stored bitmap payload.
`ScaledGraphic` now resolves `PictureGraphic` and valid `ObjectPointer` graphic
value chains into the same command shape after optional RLE decode and the same
row-padded decoded length check plus Picture Graphic extra-byte truncation,
applies the standard ScaleType byte for scaling mode
and horizontal/vertical justification inside the Width/Height field, preserves
the source bitmap dimensions in the command, and keeps a Picture Graphic
source's transparent option distinct from RLE compression. Upload,
server-retained mutation, and direct hosted-runtime
replay all reject Scaled Graphic value chains whose ObjectPointer indirection
reaches a non-graphic object or cycles. Standard `GraphicData` objects are now
decoded with their PNG/u32-length body shape; a `ScaledGraphic` that points at
`GraphicData` can emit backend-neutral RGBA image commands for non-interlaced
or Adam7-interlaced grayscale, indexed-colour, grayscale-alpha, RGB, or RGBA
PNG payloads, including 16-bit grayscale/grayscale-alpha within the 32-bit
RGBA maximum, using
zlib DEFLATE streams, including stored, fixed-Huffman, and dynamic-Huffman
blocks with literal and length/distance-copy data plus
PNG chunk CRC checks, scanline filters 0..4, and
palette/grayscale/true-colour transparency metadata,
and it keeps unsupported PNG format variants, including 16-bit RGB and RGBA
payloads that exceed the standard 32-bit RGBA maximum, as explicit placeholders
instead of misinterpreting PNG bytes as indexed bitmap data. The placeholder
path inspects the PNG signature, IHDR, basic chunk shape, dimensions, bit
depth, colour type, and the 32-bit-per-pixel limit, so malformed PNG payloads
and unsupported-but-well-formed PNG payloads report different reasons.
`ScaledBitmap` is still an machbus compatibility extension, not a
standard-completeness claim. The coverage ledger marks it out of scope, and
standard PNG `GraphicData` is likewise kept as unsupported there rather than
drawn through the old indexed shortcut.
Indexed-image commands carry the producing VT object ID so backends
can correlate later resource updates with the visible object that draws them.
The hosted framebuffer consumes copied `GraphicsContextPictureData` in
command-stream order, so a Copy Canvas/Viewport update affects subsequent
Picture Graphic draws without retroactively changing an earlier draw of the
same Picture Graphic object. When the command stream contains only bare
Copy Canvas/Viewport intent, including the standard Graphics Context copy
replay subcommands, it also keeps a per-Graphics Context indexed canvas for
direct replay, so later Picture Graphic updates use GC-owned pixels rather than
framebuffer outline/background pixels, including viewport zoom carried on the
copy intent or remembered from earlier viewport/zoom replay. Those updates are
applied in target Picture Graphic pixel coordinates, not stretched over the
later display rectangle, so copied sources clip to smaller Picture Graphics and
extra pixels in larger Picture Graphics keep their existing bitmap contents. If
the direct framebuffer path has no valid GC-owned backing canvas, such as an
oversized canvas refused by the bounded software-canvas guard, Copy
Canvas/Viewport produces no Picture Graphic update rather than sampling visible
framebuffer/debug pixels.
It can also consume bare `GraphicsContextCanvas` plus
`GraphicsContextReplay` command slices for the basic cursor/colour,
erase-rectangle, point, line, rectangle-outline, ellipse-outline, and polygon
polyline subset plus default-cell DrawText and viewport position/size updates.
For text replay, a standard NULL Font Attributes selector resets DrawText to
the default text colour instead of leaking the current drawing foreground into
text or copied Picture Graphic pixels. NULL/non-NULL line-attribute selection
is clipped to the current Graphic Context viewport. Non-NULL fill-attribute
selection fills rectangle, ellipse, and closed polygon primitives into the
visible framebuffer and the GC-owned copy canvas. This gives simple pixel
backends a direct replay path even when the hosted runtime has not pre-expanded
the replay into ordinary drawing commands.
The active scene palette is carried with the retained scene; hosted command and
framebuffer paths therefore apply Select Colour Map/Palette and Working Set
Special Controls palette selections to indexed Picture/Scaled Graphic pixels,
not just to style attributes. Colour Map object-pool records use the standard
two-byte colour-index count and reject non-standard entry counts outside the
defined 2, 16, and 256-entry forms.
The public client API exposes both `select_colour_map` and
`select_colour_palette`; both helpers emit the same standard function code while
leaving object-family validation to the VT server/runtime admission path.
When `GraphicContext` Draw VT Object replays those image commands into the
hosted bounded software canvas, the canvas expands packed 1-bit, packed 4-bit,
and byte-per-pixel 8-bit indexed Picture Graphic payloads, quantises decoded
standard PNG/RGBA `GraphicData` pixels to the nearest active VT palette entry,
normalises colours outside the Graphics Context canvas format to the configured
transparency/no-copy index, then uses nearest-neighbour scaling before Copy
Canvas/Viewport emits concrete pixel data. Change Generic Attribute now updates the render-affecting fixed fields for
`PictureGraphic` and `ScaledGraphic`; standard `GraphicData` has no render-time
Change Attribute path in the server/runtime. Payload-byte replacement remains a
separate object-pool/update operation. Malformed compressed payloads still fall
back to explicit placeholders. Decoded bitmap payloads are also length-checked against
their 1-bit, 4-bit, or 8-bit source dimensions before an image command is
emitted, so short payloads and unsupported bitmap formats produce explicit
placeholders instead of malformed image commands.

`Animation` now uses the standard fixed body and positional child-list layout:
`Value`, `Enabled`, first/default/last child indices, sequence mode, and the
disabled-behaviour option bits are decoded from the object body while frame
object IDs and X/Y locations come from the standard child records. The selected
frame is rendered through the same backend-neutral node path as a normal child
object and clipped to the Animation area, so a frame that is still
placeholder-backed remains an explicit placeholder. Change Numeric Value updates
the selected child index, Change List Item updates positional Animation frame
slots through server/runtime/macro replay, and Change Attribute covers the
standard Animation AIDs. Upload validation plus server/runtime Change Attribute
and Change Numeric Value admission now reject selected/default/first/last child
indices that do not resolve inside the positional child list, while still accepting
Value `255` as the standard no-selected-item value. A valid selected child
outside the First/Last animation sequence remains visible until the next
refresh tick; at refresh time, the animation value is range-checked before the
standard single-shot or loop advancement is applied. `VtRenderRuntime` owns the
per-object animation clocks and exposes a refresh-interval hint plus
clock-advance APIs so hosted backends can schedule deterministic redraws.
Animation clocks advance only for visible, enabled Animation objects, so an
inactive or hidden animation resumes from the frame where it was suspended.
Multiple visible instances of the same Animation object share the same
referenced frame. Hosts can also call
`tick_animation` from a terminal/UI/display timer to advance elapsed time and
receive the next scheduler
hint with the render update. Clock advances that stay inside the same visible
active frame update elapsed time without dirtying/rebuilding the scene; crossing
to a different visible frame rebuilds deterministically. The `vt_gtui_server`
example now uses `VtRenderRuntime` and `tick_animation` directly, giving the
host-loop scheduler path a concrete command-list backend demonstration while
leaving real UI/window/GPU product work deferred.

Input String generic attribute replay now covers the standard options AID and
the object codec accepts the VT4+ wrap-on-hyphen option bit. Those
transparency/wrapping option bits are not treated as the field enable flag;
runtime Enable/Disable overlays control whether the field is operator-enabled.
The Enable/Disable Object command path is now target-gated the same way in the
server, direct hosted runtime replay, and macro helper reports: only input
fields, Buttons, and Animation objects can create enabled-state overlays.
Other drawable/style/reference objects are ignored before retained replay state
changes, avoiding stale inert enable metadata.
Input Boolean generic attribute replay covers geometry and the typed
foreground/variable-reference fields. Its fixed value AID 5 and enabled-state
AID 6 are Get-Attribute-only/read-only for Change Attribute; value changes use
Change Numeric Value and enabled-state overlays use Enable/Disable Object.
Input List and Output List selected-value AID 4 are likewise
Get-Attribute-only/read-only for Change Attribute, so selected-index changes
stay on the standard Change Numeric Value path. Output List width/height now
also participate in the Change Size path instead of relying only on generic
attributes. Server-side Change
List Item replay is now bounded by the uploaded Input List, Output List, or
External Object Definition item count before retaining a mutation, and Output
List item retargets additionally preserve the renderer-presentable item type
set instead of accepting inert metadata objects. Change Polygon Point is
likewise bounded by the uploaded Polygon point list before the server retains
the point update for runtime replay.
Selected Object Pointer items resolve to their pointed object text, and the
standard no-display cases for NULL placeholders, NULL Object Pointers, and
hidden Container items produce an empty selected item instead of a misleading
index label. For Input List operator navigation, true NULL item slots remain
invisible and unselectable, while the standard empty Object Pointer-NULL and
hidden-Container item positions remain selectable even though they draw blank.
Placed Object Pointer objects also rebuild through the standard
Change Numeric Value path, so the numeric pointer value can retarget the
materialised child just like the generic target-id attribute.
Scaled Graphic also consumes Change Numeric Value as its standard graphic
Value-object reference update; setting that value to NULL removes the graphic
without reporting an unsupported/missing Graphic Data placeholder, while setting
it to an ObjectPointer is accepted only if that pointer chain still resolves to
NULL, GraphicData, or PictureGraphic.
Hosted runtime and pool-owned macro-helper Change Numeric Value replay now use
the same one-/two-/four-byte numeric target width and scalar/pointer checks as
the server path, so direct runtime tests and macro helper tests cannot truncate
high bytes or bypass value-source validation for Input List, Output List,
Object Pointer, Scaled Graphic, External Object Pointer, Meter, bar-graph, or
Animation state.
Output Meter, Linear Bar Graph, and Arched Bar Graph min/max semantics now
follow the standard render clamp: object pools and Change Attribute replay may
retain min values that are not less than max values, and the framebuffer draws
the value and target-value indicators as the minimum rather than rejecting the
pool or mutating to an arbitrary range.
Macro Change String Value replay also uses strict UTF-8 admission before
retained text mutation in both the hosted runtime and the pool-owned macro
helper, matching the server-side wire path instead of applying lossy
replacement decoding. The pool-owned helper also uses the same fixed-length
target set as the server/runtime command path: String Variable, inline Output
String, and Input Attributes validation string; variable-backed Output String
objects are not silently retargeted through the display object.
Fill Attributes pattern references are typed at upload time, during fixed
Change Fill Attributes, during macro replay, and during server/runtime Change
Attribute replay: non-NULL pattern IDs must resolve to PictureGraphic objects
before they can be retained as fill-pattern state. When Fill Type 3 is active,
monochrome pattern rows must end on whole bytes and 16-colour pattern rows must
contain an even number of pixels, so row widths that would leave unused packed bits
are rejected before upload or retained runtime mutation. For normal output shapes,
Fill Type 1 now follows the standard line-colour fill rule instead of using the
Fill Colour field or skipping the fill; Fill Type 2 keeps using the Fill Colour
field; and Fill Type 3 now tiles the referenced PictureGraphic raw/RLE pixel
data through backend-neutral commands, the hosted framebuffer, and the Graphics
Context canvas path. Pattern tiling is anchored to the active mask origin rather
than each shape's local origin, and PictureGraphic transparency/flashing options
are ignored for fill-pattern use.
Font Attributes font-type values are likewise kept on the standard-coded path:
object-pool validation and Change Attribute replay reject reserved font-type
codes before a retained style can drift away from ISO-defined values.
Font size validation now follows the proportional-style split: fixed
non-proportional sizes stay on the 0..=14 enumeration, while proportional font
style admits standard height values from 8 upward and rejects state transitions
that would leave size/style inconsistent.

Macro objects are reference objects, not direct drawables. The hosted render
runtime decodes and applies the modelled render-affecting command subset in
order: visibility, enabled state, numeric/string values, child
location/position, size, Output Line end-point geometry/direction, background
colour, option- and target-checked input selection, mask-type-checked soft-key mask/list-item changes,
Alarm Mask priority,
Object Label metadata, mask-lock runtime state, generic-attribute replay,
Font/Line/Fill Attributes object-value mutations, audio terminal side effects,
Delete Object Pool lifecycle clearing, Macro-object-gated bounded nested Execute Macro calls,
polygon point/scale changes, colour-map/palette selection, and
server/runtime current-working-set-checked Change Active Mask. The standalone
macro helper rejects wrong-family colour-selection targets and active-mask
targets before reporting hosted runtime replay work. Macro
Change Attribute replay now shares the same supported-AID table used by server
admission and direct hosted runtime replay, so unsupported or missing generic
attribute targets are skipped instead of leaking inert replay records. The
Data Mask and Alarm Mask Soft Key Mask attribute accepts `NULL` as the standard
"no associated Soft Key Mask" clear in all three paths, and the server updates
its active Soft Key Mask selection state as well as the retained Get Attribute
Value result.
Text and numeric field Font Attributes references are treated as required
object IDs: Change Attribute replay now rejects `NULL` for AID 4 on
Output String, Output Number, Input String, and Input Number, and for
Input Boolean foreground AID 3, instead of silently retaining a non-standard
nullable style selector.
Output Polygon Line Attributes are also treated as a required object ID:
Change Attribute replay rejects `NULL` for Polygon AID 3 in server admission,
direct runtime replay, and macro reporting, matching the standard
0..=65534 range while leaving nullable fill references to mean no fill.
Fill Attributes generic replay now applies the same pattern-buffer gate before
retaining type-3 pattern updates or macro reports: monochrome Picture Graphic
patterns must have byte-aligned raw rows, and 16-colour patterns must have
whole-byte row pairs, so invalid packed rows do not leak into render replay.
The standalone macro helper also rejects invalid one-/two-byte payload widths,
typed references, scalar/range values, and Window Mask generic-attribute
values, including type changes whose retained required-object list does not
match the target typed-window form and wrong-family Window Mask
Name/Title/Icon designators, before reporting runtime replay work. Macro
Change Soft Key Mask replay now uses the standard Mask Type parameter
(`1` Data Mask, `2` Alarm Mask) rather than the older object-id-at-byte-1
shortcut, so wrong mask-type/object combinations are skipped before retained
pool mutation. Get Attribute Value for Data Mask / Alarm Mask AID 2 reads that
retained soft-key-mask selection, including NULL clears, instead of reporting
only the uploaded object-pool body. The client command surface has helpers for
both the common Data Mask form and the standard Alarm Mask form, so hosts do not
need to hand-build a Mask Type `2` frame. Object
macro-reference lists can also be executed for a caller-supplied
raw VT event byte, preserving the object-pool macro order while leaving
profile-specific event-name mapping to the host. Recursive macro loops are
guarded instead of recursing forever. Unknown or too-short macro commands
remain explicit unsupported effects instead of being guessed.
The standalone macro helper applies the same pool-owned value, geometry,
background, attribute-object, and polygon mutations that it can represent
without runtime state, reports generic Change Attribute effects for the render
runtime's validated replay path, applies Delete Object Pool by clearing the
pool, and reports overlay/input/audio/object-label/mask/nested-macro changes
for the scene/runtime layer. Those reports are target-gated for the modelled
standard overlay effects: Hide/Show remains Container-only and Lock/Unlock Mask
is limited to Data Mask / Window Mask objects before the hosted runtime
consumes the effect. Colour-map and palette selection is reported as runtime
state rather than mutating the object pool.

Lock/Unlock Mask follows the ISO command-byte polarity (`0` unlocks and `1`
locks), admits only data/user-layout mask targets, and does not apply to Soft
Key Masks or Alarm Masks. In the hosted render runtime, locking the active data
mask freezes the retained visible scene while accepted ECU commands continue to
mutate the backing pool; explicit unlock or host-driven timeout expiry rebuilds
the scene and materialises the queued visual changes. A zero timeout remains
locked until the ECU sends an explicit unlock, while non-zero timeouts are
advanced deterministically through `VtRenderRuntime::advance_mask_lock_time`.
The server records the same lock state in the no_std-safe working-set state so
hosted render replay can observe it without making `VTServer` depend on
renderer types.

Text drawing now carries a renderer-ready `TextLayout` in each `DrawText`
command. The layout decodes ISO WideString UTF-16LE values, substitutes
non-printing control characters deterministically, normalises CR/LF line
endings, applies horizontal and vertical offsets, hard-wraps where enabled,
clips to the object box, and records hidden rows/columns for host backends
that need faithful text placement. The hosted framebuffer consumes those
resolved font metrics instead of a fixed-size debug cell and reflects the basic
bold, italic, inverted, underline, and strikethrough decoration bits in
deterministic text cell coverage. Output String, Output Number, Input String,
and Input Number now admit the VT4+ horizontal/vertical justification bit pairs
instead of only the VT3 horizontal values; reserved horizontal or vertical value
`3` is still rejected. Those text fields also apply their background colour
only when the transparent-background option is clear. Input String now also
feeds the standard auto-wrap option into `TextLayout` so command and
framebuffer backends see wrapped input-field rows instead of a clipped
single-line preview. Numeric text fields now admit only the standard `0..=7`
decimal counts and fixed/exponential format selector values; the old
hosted-renderer hexadecimal shortcut is rejected during pool upload and Change
Attribute replay.

Graphics Context objects still do not fully rasterise every primitive into a
GTUI canvas, but accepted canonical Graphics Context subcommands are now
exposed as backend-neutral `GraphicsContextReplay` command-stream entries from
`VtRenderRuntime`; malformed payloads for known subcommands are rejected before
they mutate server/runtime state, and object-reference payloads for known
line/fill/font selector, Draw VT Object, Copy Canvas, and Copy Viewport
subcommands are type-checked before replay state is retained; Draw VT Object
requires a drawable non-recursive standard/rendered target, explicitly rejects
the machbus-only `ScaledBitmap` compatibility object, and copy subcommands
require a non-NULL Picture Graphic target. Unknown subcommand IDs are rejected
before they can become inert replay records. The VT server now also emits the
F.57 Graphics Context response for commands whose object/subcommand bytes are
present, using the standard invalid object, invalid subcommand, invalid
parameter, and invalid-result bits, and accepts single-frame `FF16` padding
without retaining that padding in replay state. F.56 Draw Text payloads use the
standard counted-string range, so a zero-byte text body is accepted and retained
as a no-text replay command rather than rejected as malformed. The standard `GraphicContext`
line/fill/font attribute selectors, cursor/colour, erase-rectangle, point,
line, rectangle, closed-ellipse, polygon, and DrawText subcommands are also
expanded into backend-neutral primitives using the object's viewport.
Line-attribute colour
selectors, non-zero stroke widths, and full two-byte line-art paintbrush
patterns are preserved in those backend commands and in the bounded indexed canvas that
produces Copy Canvas/Viewport Picture Graphic pixel payloads. The type-36
`GraphicContext` object body now follows the full standard fixed record:
viewport/canvas size and position, initial zoom/cursor, foreground and
background colours, font/line/fill attribute references, canvas format,
options, and transparency colour. Pool validation rejects missing or wrong-type
font/line/fill attribute references before upload/runtime use. Options bit 1 is
accepted, so an initial Graphics Context can draw from its referenced
line/font/fill attribute colours instead of only from object foreground/
background colours. Pan, zoom,
pan-and-zoom, and viewport-size subcommands now update and emit
backend-neutral viewport commands, so later primitives use the adjusted
viewport origin/size; non-finite, zero, and out-of-range zoom values are
rejected before command replay or retained-state mutation. Draw VT Object
materialises the referenced drawable standard/rendered
object at the current graphics cursor, advances the cursor to that object's
bottom right corner, and replays the referenced object's supported backend
primitives into the bounded software canvas, treating colours outside the
Graphics Context canvas format as transparent. Copy Canvas and Copy Viewport now
emit explicit
`GraphicsContextCopyToPicture` commands carrying the target Picture Graphic,
source selection, effective viewport, and raw zoom bits; when the supported
primitive/text-cell/Draw-VT-object subset was replayed into the bounded
software canvas, the runtime also emits `GraphicsContextPictureData` with
concrete indexed pixels for the target Picture Graphic. Copy Viewport uses the
current viewport zoom for that copied pixel payload, so zoomed viewport copies
do not collapse back to unzoomed canvas coordinates. That software canvas now
covers point, line, rectangle, ellipse, polygon, text-cell coverage, and Draw
VT Object primitives that lower to those same backend-neutral commands.
Generic attribute changes to the standard `GraphicContext` writable viewport,
initial zoom/cursor, colours, attribute references, format, options, and
transparency colour now also rebuild the visible canvas surface. Canvas
Width/Height AIDs 5 and 6 are read-only and remain Get-Attribute-only, so
Change Attribute cannot resize the persistent canvas allocation.
The canvas command carries the transparency colour, and hosted runtime/
framebuffer copy paths initialize transparent Graphics Context backing pixels
with that colour. Copy Canvas/Viewport payloads also carry that colour as the
standard do-not-copy index, so copied pixels leave the target Picture Graphic's
existing pixels in place instead of overwriting them with palette index 0.
Copied palette indexes outside the target Picture Graphic format are also
normalised to that do-not-copy colour before the runtime payload is emitted.
The hosted `FramebufferRenderer` can consume the resulting command stream for
deterministic RGB snapshots and display-handoff byte exports, including via the
runtime-aware framebuffer path. It preloads copied Picture Graphic pixels from
`GraphicsContextPictureData` only for later target-object `IndexedImage`
commands, can also copy bare Copy Canvas/Viewport intent, including standard
Graphics Context copy replay subcommands, from the current per-Graphics Context
indexed canvas into later Picture Graphic updates while applying copy-time or
remembered viewport zoom, and expands indexed
Picture/Scaled Graphic pixels
through the active scene palette while still preserving the copy intent command
for backends with their own canvas/resource cache. Opaque Graphics Context
canvas surfaces fill their visible backing area in the software framebuffer;
transparent canvases leave existing framebuffer pixels untouched apart from the
debug/coverage outline. Basic Graphics Context replay records can also be
interpreted directly by the framebuffer for cursor/colour, erase-rectangle,
point, line, rectangle-outline, ellipse-outline, polygon polyline, and
default-cell counted DrawText drawing, including zero-length no-text replay,
with viewport position/size updates; NULL
line-attribute selection disables subsequent line primitives until a non-NULL
line-attribute selector restores them, and non-NULL fill-attribute selection
fills rectangle, ellipse, and closed polygon primitives into both the visible
framebuffer and the GC-owned copy canvas. Copy Canvas/Viewport intent can
update later visible Picture Graphic draws from GC-owned indexed canvas pixels
instead of sampling the visible framebuffer, while preserving Picture Graphic
target dimensions for clipping, unchanged extra target pixels, and transparent
handling of copied colours outside the target bitmap format. Direct copy
intent without a valid owned canvas backing is ignored rather than converted
from visible framebuffer pixels. Framebuffer
text-cell coverage
uses resolved font metrics and basic decoration bits including bold, italic,
inverted, underline, and strikethrough; it is still not a calibrated glyph
rasterizer.
Both
it and `GtuiRenderer` implement the shared `VtRenderBackend` trait. Full
calibrated display/font/graphics rasterisation remains a backend gap.

Soft-key masks support an initial physical-key paging model. `LayoutConfig`
can reserve physical soft-key positions for navigation and render only the
selected application-key page; `VtRenderRuntime` tracks and clamps the current
soft-key page. Reserved navigation cells are exposed separately from
application keys, and host navigation events now move between pages with a
`SoftKeyPageChanged` event. Visible soft-key cells also carry a zero-based
physical cell index, so keypad/backlit-button hosts can send
`PhysicalSoftKey` events without synthesising pointer coordinates; application
cells emit `SoftKeyActivated`, while enabled navigation cells page locally.
Hosts that need hardware-style activation timing can instead send
`PhysicalSoftKeyDown` and `PhysicalSoftKeyUp`: application cells emit
`Pressed`, host-driven `Held`, and matching `Released` transitions, while
navigation cells wait until physical release before changing the page and keep
that pending cell from being overwritten by another pointer, tap, one-shot
physical, direct semantic soft-key, or direct navigation activation. Stray
pointer releases do not clear a hardware-origin application or navigation
soft-key press; only the matching physical release completes or aborts it.
Two-cell navigation profiles keep both previous/next cells visible for stable
hardware-cell layout, but disable the previous cell on the first page and the
next cell on the final page so boundary physical-key presses cannot synthesize
a local page turn. Navigation-cell reservation is clamped so malformed or
over-reserved host profiles cannot consume every physical soft-key position and
overlap the remaining application key cell. Pointer hits on visible soft-key
cells are routed through the render runtime too: application cells emit
`SoftKeyActivated`, and navigation cells change pages on tap/matching release
instead of on press; stray or drag-off releases are ignored. The renderer does
not reserve
navigation keys when the whole Soft Key Mask fits on the reported physical
keys, and validation rejects Soft Key Masks above 64 virtual keys. Soft Key
Mask children may now be direct Keys, Object Pointers, or External Object
Pointers; local pointers resolve to Key objects, and External Object Pointers
can resolve through the host-registered referenced Working Set pool when the
external NAME, local Working Set NAME, enabled External Object Definition, and
target Key all validate. Invalid or unavailable external soft-key targets fall
back to the local default Key/NULL-slot rule. NULL pointers reserve the physical
slot without drawing a visible key, and trailing NULL slots are trimmed before
paging so they do not create empty pages; the runtime page-count and
next/previous helpers use the same trimmed view as the renderer. Soft Key Mask
background changes update the backing colour for cells, while a Key's own
background colour overrides the mask background for that cell. Soft-key cell
geometry now follows the configured VT profile for both vertical side columns
and horizontal/landscape soft-key rows; normal Soft Key Mask cells, navigation
cells, placed Key Group cells, pointer hit-testing, and zero-based physical-key
events all use the same configured physical-cell rectangles. The render runtime
now has an initial VT/operator-selected user-layout mapping layer:
available Window Masks and Key Groups can be placed into data-mask grid cells
or soft-key cells, unavailable interactive selections and out-of-grid
placements are rejected, and Key Groups cannot claim active Soft Key Mask
application cells or the active profile's VT-reserved soft-key navigation cells
when the current Soft Key Mask requires
paging. That exclusion now follows the navigation cells actually rendered after
Soft Key Mask child resolution and trailing NULL trimming, not the raw child
count, so non-paged profiles keep the remaining soft-key cells available for
Key Groups. Accepted placement overrides survive normal scene rebuilds while
they remain valid; if a later active Soft Key Mask claims those cells as
application or navigation soft keys, the runtime removes the stale Key Group
mapping before exposing the rebuilt scene. If a changed Window Mask or Key
Group grows or otherwise makes its remembered placement invalid, rebuild
revalidation removes that changed mapping before stable unchanged mappings are
reconsidered, preserving still-valid operator placements in their original
cells; the changed-object priority is retained across Lock/Unlock Mask
deferrals and applied when the locked scene finally refreshes. If several
deferred changed placements conflict with each other, the runtime uses Object
ID order as the tie-break rather than hash-map or ECU command order. Hosts can
export and restore deterministic logical-cell placement snapshots for
their own non-volatile storage; exported snapshots are sorted by object ID
rather than hash-map iteration order, while restore still admits currently
unavailable objects so recalled cells can be blanked. Available Key Group
objects can now be placed by the layout engine as one-to-four normal
soft-key-sized cells; their children may be direct Keys, Object Pointers that
resolve to Keys, or External Object Pointers that resolve through a
host-registered referenced Working Set to a granted external Key. The resulting
user-layout cells activate the resolved local or external Key IDs. Runtime
operator-placement validation and overlap rectangles use the same resolver, so
Key Groups whose only available child is a valid external Key can still be
placed into user-layout soft-key cells. If an External Object Pointer Key Group
child cannot currently resolve because the referenced Working Set pool is not
registered and its local default is NULL, that child now remains a blank
non-activating physical cell instead of collapsing later Key slots upward.
Runtime Object Pointer retargeting keeps those slot rules after upload: Key
Group pointer children must continue to resolve to non-NULL Keys, while Soft
Key Mask pointer children may resolve to NULL reserved cells or Keys only.
External Object Pointer children in Soft Key Masks and Key Groups keep the same
local fallback-slot rule during Change Attribute replay: default-object AID 1
accepts NULL or Key objects and rejects arbitrary uploaded objects in both
server and direct hosted-runtime paths.
Key Group mapping-screen designators are typed too: the Name field must
resolve to an Output String or an Object Pointer to an Output String, while the
Icon field is NULL or an Object Label graphic-representation output object.
Upload validation and Generic Attribute replay use the same checks.
Window Mask mapping-screen designators use the same hosted admission checks:
non-NULL Name and Window Title references must resolve to Output String
directly or through an Object Pointer, while non-NULL Window Icon references
must resolve to Object Label graphic-representation output objects. Upload,
server-retained replay, direct runtime replay, and macro replay all reject
wrong-family designators before they become retained render state.
Soft-key and Key Group labels now prefer display text from child output
objects, including Object Pointer indirection, before falling back to the Key
code. Pointer taps on those cells activate the contained Key IDs with the same
backend-neutral `SoftKeyActivated` event used by normal
application soft keys. Zero-based `PhysicalSoftKey` host events also activate
placed Key Group cells when no normal Soft Key Mask cell owns that physical
position, so hardware/backlit-button hosts do not need to synthesize pointer
coordinates for operator-placed Key Groups. Resolved external soft-key and Key
Group slots retain the referenced Key Number, so Soft Key Activation payloads
do not lose the external Working Set's Key Code. Select Input Object focus
replay can now focus visible Key IDs inside available Key Group cells as
focus-only targets, matching the VT4+ Key focus path without opening an edit
transaction. Explicit Commit on a selected visible soft-key or Key Group key
now activates that focus-only target, or changes pages for a selected
navigation cell, while open input edits still keep their normal commit path.
Pointer press/move/release paths now emit the same
activation-code `Pressed`/`Released`/`Aborted` events as normal application
soft-key cells, including immediate `Aborted` emission when the pointer slides
off the pressed Key Group key. The physical soft-key down/up path works for
placed Key Group cells too, so real key hardware can get `Pressed`, held
repeat, and `Released` transitions without manufacturing pointer coordinates;
stray pointer move/release events do not abort that hardware-origin Key Group
press.
Unavailable Key Groups are placed but do not emit key cells or activation
events; the renderer blanks/fills the remembered cells even when the Key Group
has the transparent option set. Transparent available Key Groups still leave
the underlying user-layout/window area unpainted and emit their key cells. Window Mask
bodies now use the VT4+ user-layout record shape;
available free-form windows size themselves from the fixed 2 × 6 user-layout
cell grid. Width, height, window-type, and options updates are admitted only
inside their standard scalar ranges before server/runtime state is mutated;
window-type updates also have to match the retained required-object list for
the target typed-window form. Standard typed windows 1..18 materialise their
required objects into VT-controlled slots both when nested and when selected as
the active mask. Active and nested Window Mask children are now lowered with a
Window Mask clip rectangle, so oversized free-form or typed-window child output
cannot paint outside the assigned window region. Window clips are intersected
with object-local clips such as Output List selected-item viewports and
Animation frame boxes.
Available transparent windows preserve the underlying user-layout pixels instead
of painting their background colour. Unavailable windows blank their cell
region without rendering children. The runtime can also generate VT On
User-Layout Hide/Show
notifications for currently placed Window Mask and Key Group nodes, packing two
visibility records per payload, sorting those Window Mask / Key Group records
by Object ID before packing, and preserving optional VT v6 TAN bits in the
full-message path. A separate active-mask helper emits the same H.20 message
shape for the active Data Mask plus active Soft Key Mask, so hosts can notify
an inactive-but-still-visible Working Set or hide those masks before making it
active again. The matching H.21 response is modelled as a separate
`UserLayoutHideShowResponse` helper that parses/builds checked ECU-to-VT
payloads, including NULL-second-record, status-bit, VT5 reserved-byte, and VT6
TAN-nibble validation. Runtime placement helpers reject overlapping physical
placement rectangles before mutating the operator layout snapshot, including
custom VT profiles where the soft-key area overlaps the data-mask/user-layout
area rather than sitting outside the main canvas. `VTServer` now also answers
the standard Get Window Mask Data technical-data request with configurable
VT-owned user-layout Data Mask and user-layout Soft Key cell background colours,
so Working Sets can colour-match free-form Window Mask and Key Group content
without guessing from one of their own object colours. The same technical-data
path now distinguishes the standard Get Supported Objects query from the AUX
type-2 capability subquery: the standard response returns a numerically sorted
supported-object byte list, omits Auxiliary Function/Input type-1 objects, and
does not advertise local reserved compatibility object codes as standard VT
objects. Get Supported WideChars now reports the ISO WideChar minimum
character set ranges for code plane 0, clipped to the requested inquiry range,
and returns standard error bits for invalid code planes or inverted ranges
instead of falling through to Unsupported Function. The client-side
`get_supported_widechars` builder emits a canonical code-plane-0 full-range
query, and `get_supported_widechars_range` exposes the standard code-plane plus
first/last range fields. Parameterless technical-data requests on the server
side are canonical as well: Get Hardware, Get Number of Soft Keys, Get Text
Font Data, and Get Window Mask Data only answer fixed `[code][FF×7]` requests,
so malformed reserved bytes cannot be accepted as prefix-compatible capability
queries. Pool/session-control requests reject hidden reserved-byte garbage too:
Get Memory must preserve the `0xFF` tail after its requested memory-size field,
while Get Versions and End Of Object Pool must arrive as `[code][FF×7]`.
Malformed forms cannot open the upload window, return a prefix-compatible
versions response, or activate a pending pool.

Input handling now separates selected input state from open edit transactions.
Typing emits edit-preview events, while `Commit` emits the final value-change
event and `Cancel` aborts the open edit with a VT ESC semantic event instead of
a final value. Rebinding the hosted input runtime to a rebuilt scene preserves
the selected/open transaction by VT object ID rather than by the previous
focus-order index, so mask changes, user-layout placement, or external object
materialisation cannot move an in-progress edit to a different reordered node.
Disabled input nodes are also rejected before tap handling can focus them, open
an edit transaction, or emit a list/boolean value event; runtime Enable/Disable
overlays use the same path after scene rebind. If a previously focused/open
input later disappears or becomes hidden/disabled, typing, backspace, and
commit clear the stale selected/open/edit state before any value event can be
emitted.
Hosted `InputString`
edits enforce both resolved input-validation attributes and the object's fixed
maximum string length before mutating the edit buffer. The resolved validation
keeps classic Input Attributes byte-oriented and uses Extended Input Attributes
for WideString code-plane ranges, including blacklist ranges. Validation
failures are rejected before opening the input transaction, and hardware
characters delivered to a focused non-editable object stay ignored without
creating open input state. Hosted `InputNumber`
now uses the standard secondary options byte for enabled and real-time-editing
state and replays the raw Value attribute when the field stores its value
inline: host key input accepts only decimal digits before mutating the edit
buffer, normal number edits preview until commit, commits reject raw values
outside the declared min/max range without closing the transaction, and
real-time number edits emit complete numeric value-change events immediately
while rejecting non-decimal or out-of-range edits without mutating the edit buffer. Input
String enabled state now follows runtime Enable/Disable overlays rather than
the object's transparency/wrapping options byte, while the transparent bit still
controls whether the field paints its background colour and the auto-wrap bit
controls text layout wrapping. Tapping an
enabled `InputList` respects its real-time-editing option too: normal lists emit
a backend-neutral selection preview until commit/cancel, while real-time lists
emit a complete next-selection event immediately. Input List upload validation
now requires its optional variable reference to resolve to a Number Variable
and each non-NULL item slot to resolve to an uploaded object before
render/runtime use, while preserving NULL slots as standard no-item
placeholders.
Input List rendering now resolves the selected item's display text through the
same output/variable/pointer/external-pointer/container paths as list-like
display objects and leaves non-displayable selections blank instead of
fabricating an index/count diagnostic label. Host-registered referenced pools
can supply External Object Pointer item text; unresolved external items with no
local default stay blank. Selected non-interactive drawable items can also
materialise as clipped display-value scene nodes, with the parent Input List
remaining the interactive hit target and the compact text fallback suppressed.
Accepted Change List Item effects rebuild that visible label or materialised
item when the selected item slot is retargeted. Accepted
Change Numeric Value effects for an Input List now retain the selected index
and rebuild the visible selected item too. Value `255` and out-of-range
Number Variable values now preserve the standard blank selected-item display
instead of clamping to a valid item; the next operator selection restarts at the
first selectable list entry. Selected NULL item slots are blank as well, and
operator selection skips true NULL and unavailable external slots while keeping
the standard empty Object Pointer-NULL and hidden-Container item positions
selectable; all of those positions still preserve their indexes for Working
Set values. Input List bodies also carry the standard inline selected
value and one-byte item count, so lists with
a NULL variable reference use that inline value for rendering, Change Numeric
Value replay, and Get Attribute Value. Enabled Button objects use
activation-code-aware pointer sequencing: press latches the candidate button and
emits `Pressed`, matching release emits `Released`, pointer slide-off or
drag-off emits `Aborted`, and stray releases without a preceding press are
ignored. Button
labels now resolve display text from child output objects, including Object
Pointer indirection, before emitting backend-neutral `DrawText`. Button
activation payloads use the scene-resolved Key Number, so Buttons materialised
from a registered external Working Set do not lose their external Key Code.
Button option bit 4 (`0x10`) is treated as the standard Disabled bit for
initial scene state, while bit 6 (`0x40`) remains reserved object-pool data.
Button rendering resolves the Button background and border colour fields
through the active palette, skips the Button face fill when the transparent
background option is set, and suppresses the hosted border stroke when either
the suppress-border or no-border option is set.
Hardware Enter and explicit Commit on a focused/selected enabled Button emit
the same `ButtonActivated` semantic event as a one-shot button activation,
without opening an edit transaction.
Application
soft-key pointer paths follow the same `Pressed`/`Released`/`Aborted` model.
Direct host-provided `SoftKeyActivate(id)` events are checked against the
active scene first, so hidden, disabled, or stale Key IDs cannot produce a
semantic activation event.
`VtRenderRuntime` now has a first bus-message bridge for completed semantic
events: boolean/list/number final changes lower to Numeric Value Change
payloads, final strings lower to String Value Change payloads, focus and
edit-open/close state changes lower to Select Input Object payloads, and
accepted soft-key or button activations lower to deterministic press+release
activation payload pairs using scene/object-pool parent IDs and key-code fields.
Pointer activation-code events lower directly to the matching activation-code
payload. Direct semantic soft-key/button activation lowering re-checks the
active scene's visible/enabled state before building payloads, so caller-built
`VtEvent` values cannot bypass disabled Buttons, stale Soft Key Mask cells, or
unavailable Key Group slots. Held-repeat timing is runtime-owned through
`ActivationHoldTiming` and
`advance_activation_hold_time`: after a press, host event loops advance the
timer and receive deterministic repeated `Held` events plus optional bus
payloads or full PGN/addressed messages. Invalid full-message endpoints are
rejected before repeat timing is consumed.
The stateful `handle_operator_event_with_bus_messages` path emits selected/open
input transitions before edit previews or value changes, lowers cancel aborts
to VT ESC payloads, and emits close transitions after commit/cancel. The cancel
sequence is VT ESC followed by a selected-but-not-open Select Input Object
notification. Edit previews, local soft-key page navigation, and ignored events
otherwise remain local-only. The ECU-side client also parses VT ESC as an
aborted-input event and can build matching VT ESC response payloads, including
explicit error-code variants for hosts that need to acknowledge a concrete
abort/error reason. The no_std-safe `VTServer` also responds to a canonical
ECU ESC Input command with a VT ESC response frame for the selected input
object, and rejects malformed reserved bytes without mutating retained input
escape state.
`VtBusMessage` can also wrap these payloads in full `PGN_VT_TO_ECU` messages
with explicit VT source and ECU destination addresses, and
`VtRenderRuntime` exposes full-message lowering helpers for both direct
semantic events and stateful operator events. Those helpers preserve the same
event order as the payload-only path. The checked full-message path rejects
null/broadcast VT sources and null/broadcast ECU destinations before stateful
operator-event mutation or emission, so a host cannot accidentally turn a valid
payload into an unusable VT message envelope. Payload-only and full-message
lowering also reject semantic value/input/activation/ESC events that target the
NULL object ID before any bus payload is emitted. The stateful operator-event
and held-activation bus helpers snapshot the runtime before applying local state
changes and restore it if semantic-event lowering fails, keeping focus, edit,
and held-repeat state atomic with bus emission.
The bus-message helper is split into the semantic
`src/isobus/vt/render/bus_message.rs` module and now supports the receive-side
admission path too: command bytes map back to typed message families,
payload-only and addressed `PGN_VT_TO_ECU` messages parse through one checked
path, Soft Key/Button/Numeric Value Change have explicit VT6 TAN constructors,
and malformed reserved bytes, bad TAN low nibbles, string length/UTF-8
mismatches, inconsistent Select Input Object state, NULL concrete-object IDs,
and invalid Pointing Event parent-mask/touch-state shapes are rejected before a
host treats the payload as render-runtime bus evidence.
ECU-to-VT Select Input Object and ESC Input effects are replayed into the
hosted `InputRuntime`: option `FF` selects input fields or VT4+ Button/Key
targets for focus only, NULL+`FF` removes focus, option `0` opens only input
field objects for data input, and invalid option/target combinations do not
mutate retained state. The ECU-side client decodes VT Select Input Object
messages using the standard byte-4 selection plus byte-5 open bit layout, and
the VT server emits/VT client parses the paired Select Input Object response
with selected/opened response codes plus disabled, invalid-object,
not-on-active-mask-or-hidden (`0x04`), busy, and invalid-option error bits.
Targets outside the active Data/Alarm Mask or inside a hidden Container are
rejected without mutating retained focus/open state. ESC aborts an open edit
transaction without producing a final value. Pointer and
hardware key events are present as backend-neutral host
inputs, including Enter/Commit activation for focused Buttons and selected
focus-only soft-key or Key Group keys. The stateful bus bridge lowers those
hardware/profile activations to the same checked Button/Soft Key press+release
payload pairs as pointer/tap activations. Pointer activity in non-interactive
Data Mask or free-form Window Mask areas is reported as VT Pointing Event
semantic events and can be lowered to payload-only or addressed VT-to-ECU
messages; button, input, soft-key, and Key
Group hits keep their activation/select-input semantics instead of being
misreported as pointing. VT ESC carries optional VT v6 transfer sequence
numbers through the semantic event, VT-to-ECU payload builder, client parser,
and ECU-to-VT response builder; the response builder also preserves an explicit
error-code byte when requested. `VTServer` emits the corresponding VT ESC
response for accepted ESC Input commands. Ordered full-message lowering keeps
VT ESC TAN payloads in caller order and rejects malformed TAN values instead of
returning a partial addressed-message prefix. User-layout hide/show
notifications use the same
payload-only and addressed-message lowering path, including NULL object-id and
four-bit TAN validation. Control Audio Signal Termination has checked
payload/full-message helpers too: H.22 VT-to-ECU notifications support the VT5
reserved-byte and VT6 TAN forms, and the H.23 response helper is VT6-only with
strict cause-byte, reserved-byte, TAN-nibble, and destination-specific envelope
validation. Operator-event ECU responses are checked too:
`ControlActivationResponse` builds/parses H.3/H.5 Soft Key/Button Activation
responses with VT5 reserved-byte or VT6 TAN payloads, and
`PointingEventResponse` builds/parses H.7 Pointing Event responses across VT3
implied-press, VT4/VT5 touch-state, and VT6 TAN-plus-parent-mask forms before
wrapping them in addressed `PGN_ECU_TO_VT` messages. H.9 Select Input Object
and H.11 VT ESC responses are modelled as strict helpers too, including the
VT4/prior versus VT5+ open-input byte split, ESC reserved bytes, selected/open
consistency, VT6 TAN handling, and full-message envelope validation. H.13
Numeric Value Change and H.19 String Value Change ECU responses are also
checked helpers: numeric responses preserve the exact four value bytes and the
VT5/VT6 reserved/TAN split, while string responses enforce the standard
reserved-byte-only shape before full-message wrapping/parsing. H.14/H.15
Change Active Mask and H.16/H.17 Change Soft Key Mask error
notification/response helpers cover active-mask and soft-key-mask drawing or
reference failure reports, reserved error-bit/tail-byte checks, and
destination-specific VT-to-ECU / ECU-to-VT envelopes.

Change Object Label handling is now gated by the uploaded Object Label
Reference List and by the same output/drawable graphic-designator type check
used during pool validation. A hosted `VtRenderRuntime` starts with labels from
the object pool, and accepted label changes override that metadata without
forcing a data-mask redraw. Server, direct runtime, and macro replay all reject
undeclared label targets, bad String Variable references, invalid graphic
designators, and reserved Annex K font-type bytes even when the label string
reference is NULL.

Working Set Special Controls colour startup is now part of both hosted render
runtime construction and server upload acceptance. The renderer uses the
specified initial Colour Palette object, overlays its standard B,G,R,A ARGB
entries from index 0 upward, and then routes colours through the specified
initial colour map unless a later Select Colour Map/Palette command overrides
or resets those selections. When the Working Set Special Controls Colour
Palette attribute is NULL or absent, the hosted renderer now keeps the VT
default palette instead of falling back to the first Colour Palette object in
the pool. The retained Working Set Special Controls colour references are
updated as the standard requires. Change Attribute updates to the object's
colour-map and colour-palette attributes also rebuild the hosted scene and
update server-retained render state. Server Get Attribute Value for AID 2 and
AID 3 now reads that live retained selection, so a later Select Colour
Map/Palette command overrides any earlier Change Attribute overlay instead of
returning stale colour references. Its advertised language/country pairs are
exposed on the scene with the two-space country sentinel preserved; host
preference matching is deterministic and ASCII-case-insensitive, while the
returned pair keeps the Working Set's uploaded casing.

Command-trace replay has initial fixture coverage. The repo-owned
`tests/fixtures/isobus/vt_render_trace.hex` trace is replayed through
`VTServer`, its accepted `ServerRenderEffect` stream, and
`VtRenderRuntime::from_server_working_set` to prove that post-activation
ECU-to-VT command bytes can update rendered scene state. The same constructor
now folds the server-retained selected input object back into the hosted input
runtime, so server snapshots preserve focus as well as draw state. It also
materialises retained Change Priority state into Alarm Mask metadata; priority
changes affect alarm ordering metadata, not the retained data-mask draw list.
Runtime Hide/Show or Enable/Disable commands that match the current effective
object state, plus Enable/Disable commands aimed at non-enableable object
families, including the same effects executed from Macro commands, repeated
overlays, background/size/style updates, child position, soft-key mask/end-point
changes, list item replacement, polygon point changes, same-extents polygon
scaling, and mapped Change Generic Attribute replay, Change Numeric Value, and
Change String Value with the same encoded object body now stay `Unchanged`, so
replayed duplicate or wrong-target state does not force a scene rebuild or dirty
flag. Server-side Change Child Location/Position admission also now requires
the parent object to actually own the target child before retaining geometry
state, matching the hosted runtime helper instead of preserving inert
non-parent overlays.
Server-side Change Attribute admission now checks that the target object owns a
mutable AID, that reference-valued AIDs point to admissible objects, and that
scalar option-bit, format, justification, boolean-state, shape
type/direction/suppression, half-degree angle, and state-relative min/max values
stay inside the supported ranges before storing retained attribute state or
appending render replay effects; unsupported AIDs, wrong-type object references,
out-of-range scalar values, and read-only value AIDs such as Number Variable
Value stay on their dedicated command paths instead of becoming inert retained
state. Hosted runtime Change Generic Attribute replay now applies the same
reference/scalar checks for the mapped render-affecting AIDs before mutating its
retained pool, including macro replay and server-state import paths; the
Graphics Context fixed-field gate includes 0..=32767 writable viewport
dimensions, read-only canvas-size rejection, canonical two-byte signed
viewport/cursor positions, finite in-range zoom, typed style references, and
standard NULL style selectors.
Server snapshot import applies retained generic attributes as
a converged final state, so state-relative ranges such as Input Number min/max
survive folding into a fresh hosted runtime even when their canonical map order
differs from the command order that the server originally accepted.
This is a local reduced fixture, not a substitute for the still-required
independent `.iop` and reference-tool traces. The standard-suite also loads the
reviewable `tests/fixtures/isobus/vt_object_pool.hex` pool through
`IopDocument`, lowers it to backend commands, and renders it through the hosted
framebuffer so the byte-walker and render pipeline stay covered by fixture
bytes instead of only hand-built pools. The `iop_inspect` example can also load
candidate raw `.iop` / `.bin` pools directly from disk, or a named entry from a
reviewable `.hex` fixture file, so independent pools can be inspected before
they are promoted into Phase 10 evidence fixtures. The report includes both the
lowered GTUI command preview and a hosted framebuffer snapshot summary with
RGB888/RGB565 export sizes, placeholder-pixel count, and a deterministic RGB888
FNV-1a hash plus deterministic RGB565 big-/little-endian FNV-1a hashes. The
inspector also records a deterministic pool-buffer FNV-1a hash and can run
against an explicit target layout profile using `--canvas`,
`--soft-key-area`, `--physical-soft-keys`, `--navigation-soft-keys`, and
`--soft-key-page`, which is needed when promoting soft-key-paging or
target-display evidence. `iop_inspect --expect-rgb888-fnv64 ...`,
`--expect-rgb565-be-fnv64 ...`, and `--expect-rgb565-le-fnv64 ...` exit
nonzero when the rendered snapshot hashes differ. `--expect-unsupported-records`
and `--expect-placeholder-pixels` also let a promoted fixture pin explicit
caveat counts instead of silently accepting renderer drift; `--write-rgb888`,
`--write-rgb565-be`, and `--write-rgb565-le` write raw packed framebuffer
snapshots for archiving, byte-for-byte comparison, or display-driver smoke
tests. `--write-report-json` writes a stable machine-readable report with
source, pool/object counts, layout profile, canvas, coverage totals, GTUI
command count, framebuffer hash/size data, requested raw artifact paths in its
`artifacts` object, and check failures for provenance notes or CI artifacts.
The framebuffer hash data includes RGB888 plus both RGB565 byte orders so a
promoted fixture can compare host snapshots and common display-driver handoff
bytes without recomputing them out of band.
`iop_inspect --strict`
exits nonzero when unsupported scene records, framebuffer render errors, or
placeholder pixels are present, making candidate pools usable as a repeatable
pre-promotion evidence gate. The external-evidence manifest at
`tests/fixtures/isobus/vt_external_evidence_requirements.txt` names the
required independent pool categories. `vt_external_pool_basic` is now backed by
`tests/fixtures/isobus/VT3TestPool.iop`, strict `iop_inspect` JSON, and
checked-in RGB888/RGB565 raw framebuffer artifacts whose hashes are pinned in
`tests/fixtures/isobus/vt_external_reports/vt_external_pool_basic_vt3.md`;
`vt_external_pool_graphics` uses the same independent pool with
`--active-mask 0x07D0` so its alarm mask renders a visible indexed
`PictureGraphic`, with hashes pinned in
`tests/fixtures/isobus/vt_external_reports/vt_external_pool_graphics_vt3_alarm.md`.
`vt_external_command_trace` is now backed by a reduced ISO11783-CAN-Stack VT3
alarm soft-key callback trace,
`tests/fixtures/isobus/vt_external_trace_vt3_alarm.hex`, replayed through
`vt_trace_inspect` after uploading `VT3TestPool.iop`; it records one accepted
`ChangeActiveMask` effect, pinned initial/final RGB888/RGB565 hashes, and raw
initial/final framebuffer artifacts in
`tests/fixtures/isobus/vt_external_reports/vt_external_trace_vt3_alarm.md`.
Reduced open-license seeder fixtures now close the remaining rows:
`vt_external_pool_soft_keys_seeder_reduced.hex` covers more keys than physical
soft-key cells, `vt_external_pool_inputs_seeder_reduced.hex` covers
`InputBoolean` plus `InputList`, and
`vt_external_pool_user_layout_seeder_reduced.hex` covers free-form
`WindowMask` plus `KeyGroup` placement. The standard suite validates this
manifest shape and checks every `complete` row for checked-in fixture/report
artifacts plus source, licence, layout, RGB888/RGB565 hashes, strict-result,
and caveat fields, so synthetic in-repo fixtures cannot be mistaken for
Phase 10 completion evidence. For command traces,
`vt_trace_inspect` replays named ECU-to-VT command payload rows through
`VTServer`, builds `VtRenderRuntime` from the accepted server state, renders
initial/final framebuffer snapshots, and writes
`machbus-vt-trace-inspect-report-v1` JSON with pool/trace FNV-1a hashes,
the explicit target layout profile, accepted render effects, and initial/final
RGB888 plus RGB565 big-/little-endian hashes. It can also write tightly packed
initial/final RGB888 plus RGB565 big-/little-endian frame dumps, and its JSON
`artifacts` object records the requested report and raw frame paths, so
promoted command traces can archive byte-for-byte display evidence and
display-driver handoff bytes next to their JSON report. Its
`--expect-accepted-effects`,
`--expect-initial-placeholder-pixels`, and `--expect-final-placeholder-pixels`
flags pin replay/caveat counts in the same promotion command. It also accepts
named pools from reviewable `.hex` files via
`--pool-fixture`, so promoted command traces can name the same checked-in pool
fixture as their starting point. `make vt-evidence-smoke` runs the static pool
inspector and command-trace inspector with strict/hash gates and raw trace frame
dumps against repo-owned reduced fixtures; those smokes are repeatability
checks, not external certification evidence. The hosted runtime also maps
Change Attribute for the common visible shell objects: Data/Alarm/Window masks,
Containers, Soft Key Masks, Keys, and Key Groups now update their retained body
fields and rebuild the scene or soft-key area where those fields are visible;
Key objects now reject non-standard AID 3 so Key Group options are not confused
with Key fields.
Placed Object Pointers now dereference their target into the retained scene, and
Change Numeric Value on the pointer target field retargets the materialised
object. Their target value remains Get Attribute Value readable; Change
Attribute is rejected for Object Pointer retargeting. Server-side Change
Numeric Value admission also rejects invalid render-affecting scalar/pointer
values before retaining replay state, including non-boolean Input Boolean
values, missing Object Pointer targets, invalid Scaled Graphic value-source
chains, Animation selected-frame values outside the positional child list
except `255`, and invalid External Object Pointer External Reference NAME
targets. Server-side Input Boolean fixed value and enabled-state Change
Attribute admission is also boolean-gated before retained replay state changes,
and Input Boolean object-pool encode/decode rejects non-boolean fixed values.
Input Boolean foreground/variable references are typed to Font Attributes /
Number Variable.
Alarm Mask fixed Priority and Acoustic Signal object-pool fields reject
reserved values at encode/decode time, and AID 4 Acoustic Signal generic
attribute replay is range-gated in the same server/runtime/macro paths.
External Object Pointer reference-NAME Change Attribute replay accepts NULL as
the standard fallback case, so a live pointer can stop resolving an external
pool and redraw from its local default object. External Object Pointer
default-object Change Attribute replay also preserves Soft Key Mask
fallback-slot semantics by allowing only NULL or Key targets when the pointer is
used as a soft-key child. Direct hosted-runtime replay uses the same checks
before mutating its retained pool.

## Validation

The ledger is checked by the VT render tests:

```sh
make standard-suite-check
```

The relevant assertions prove that:

- every implemented `ObjectType` has a render status;
- newly modelled standard families stay visible in the static ledger;
- document-specific coverage reports only the object types present in a loaded
  pool;
- unsupported/placeholder objects are recorded instead of silently dropped.

Run the book and whitespace gates after editing this page or the CSV:

```sh
make book
make whitespace-check
```
