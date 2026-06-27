# Rust-port behavior differences

The original C++ code used several push-style callbacks and duplicate type names
that do not map cleanly into a Rust crate. These are the intentional differences
in the Rust port.

## ISO 11783-6 VT object-pool codec conformance

The VT object-pool codec in `src/isobus/vt/objects.rs` was audited against
ISO 11783-6 (Virtual Terminal) and brought into conformance on the major
divergences. This section records what changed and what remains.

### Resolved divergences (now conformant)

- **Number display math**: `(value + offset) * scale` (was a division).
- **Scale field type**: IEEE-754 `f32` (was `i32`).
- **Child positions**: signed `i16` X/Y (was `u16`); each child record is
  the standard 6 bytes `[oid:u16][x:i16][y:i16]` (was 2 bytes, OID only).
- **Child counts**: `u8` per the standard (was `u16`).
- **Macro reference lists**: `[num_macros:u8][num × (event:u8, macro:u8)]`
  round-trip through the codec (were absent).
- **Per-type child record size**: 6-byte positional records for
  WorkingSet/DataMask/AlarmMask/Container/Key/Button/WindowMask; 2-byte
  OID-only records for SoftKeyMask/KeyGroup.
- **Body field layouts** corrected for: WorkingSet (gained
  background/selectable/active_mask), AlarmMask (dropped non-standard
  `options` byte), InputBoolean (background, width, foreground as a Font
  Attributes ref, variable_ref, value, enabled — no height/options),
  InputString (`Length` is `u8`; justification precedes length),
  InputNumber (gained `value`/`justification`/standard Options 2; dropped
  non-standard `input_attributes`), OutputNumber (gained `value`/`justification`),
  OutputLine/OutputRectangle/OutputEllipse (`line_attributes` first),
  Meter/LinearBarGraph/ArchedBarGraph (`u16` min/max + `value`;
  ArchedBarGraph `bar_width` not `number_of_ticks`), PictureGraphic
  (`actual_width`/`actual_height`/`transparency`/u32 raw-data length),
  StringVariable (gained `Length` field).
- **No per-object length prefix**: the codec now serializes each object as
  the ISO 11783-6 wire layout `[id:u16][type:u8][body…]` (the earlier
  `[len:u16]` field is gone). Object boundaries are recovered by a
  parse-by-type walker (`object_body_total_len`) that knows every body's
  length, so the same `ObjectPool::deserialize` path consumes
  machbus-produced pools, real `.iop` files, and pools uploaded by
  third-party VT clients. `net::iop_parser` delegates to this codec — one
  source of truth.
- **Self-delimiting bodies**: to support the prefix-free format,
  `InputAttributes` gained its standard `length:u8` field
  (`[type][len][string]`), `Macro` gained its standard `num_bytes:u16`
  prefix (`[num_bytes][commands…]`), and `StringVariable` emits the actual
  value length on the wire — all three now match ISO 11783-6.

### Remaining (intentional) gaps

- **Hosted VT rendering is still partial evidence, not certification**:
  Working Set language codes and Working Set Special Controls language/country
  pairs are modelled for hosted selection policy, but text-resource selection
  and profile-specific editor behaviour still need more evidence. Standard
  `GraphicContext` surfaces, PictureGraphic-backed `ScaledGraphic`,
  `Animation`, and the bounded PNG `GraphicData` subset now have
  backend-neutral render coverage where their referenced image/frame payloads
  are valid. Unsupported PNG format variants and unsupported
  graphics-context canvas operations remain explicit placeholders rather than
  certification-quality rendering.
- **Protocol-state object bodies**: Aux* objects remain outside the retained
  data-mask renderer, even though they are modelled by the VT object
  pool/protocol layers. Compatibility extension rows remain compatibility
  claims, not ISO standard-completeness claims.
- **Palette / font metrics**: the render layer's colour palette RGB
  values and font-size→pixel mapping are repo-owned approximations (the
  standard's exact values are licensed material, not reproduced here).

## Wire codecs return values instead of pushing frames

Most ISOBUS subsystem codecs return `Vec<Outbound>` or typed values instead of
directly pushing into a global network manager. The session facade is
responsible for routing those outbound frames onto the transport.


Why:

- unit tests can assert exact frame sequences without a live bus;
- no hidden global state is required;
- C/Python bindings can expose fallible calls with a stable error channel.

## Duplicate C++ names were disambiguated

Some C++ headers used the same public name for different layouts. Rust requires
one canonical item per module path, so the port uses explicit names:

- classic FS `FileServerProperties` lives in `src/isobus/fs/types.rs`;
- v2 FS properties use `FileServerPropertiesV2` in
  `src/isobus/fs/properties.rs`;
- v2 volume state uses `VolumeStateV2`.

## Section Control is split by role

The Section Control master and client logic live under `src/isobus/sc/master.rs`
and `src/isobus/sc/client.rs`. This keeps role-specific timing and state
separate while allowing a facade to compose them later.

## DTC memory is explicit

Diagnostic occurrence/history handling is centralized in `DmMemory`. The diagnostics plugins expose active and previous DTC lists; it does not hide occurrence-count
changes behind unrelated message helpers.

## Implement pump bridge is explicit

The `Implement` plugin has concrete inbound PGN callbacks for the supported
hitch, PTO, and auxiliary-valve command PGNs. Unsupported implement-message
families remain codec-only until wired through an explicit facade method; no
no-op event bridge is kept to imply hidden behavior.

## C ABI is an explicit facade

The C header is generated from `src/ffi.rs` only. Internal Rust constants,
helper modules, and protocol tables are not part of the C ABI unless they are
wrapped by an `Machbus*` type or `machbus_*` function in that file.
