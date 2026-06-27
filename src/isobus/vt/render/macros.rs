//! Macro runtime for the VT renderer: dispatch, decode, and apply.
//!
//! ISO 11783-6 objects carry a macro reference list — `[event_id:u8,
//! macro_id:u8]` pairs (see [`crate::isobus::vt::MacroRef`]) — that binds
//! an object event (key pressed, value changed, on-show, …) to a Macro
//! object that runs when that event fires.
//!
//! Three pieces:
//! - [`MacroTriggerIndex`] — dispatch: `(object, event_id) → [Macro ids]`,
//!   built purely from [`crate::isobus::vt::MacroRef`].
//! - [`decode_macro_effects`] — interpret a Macro's command stream into
//!   typed [`MacroEffect`]s for the modelled command subset; unknown or
//!   too-short commands become [`MacroEffect::Unsupported`] (no guessing).
//! - [`apply_macro_effects`] — write pool-owned value/geometry/style effects
//!   into the pool and surface visibility/enabled/colour/mask runtime-state
//!   changes in a [`MacroApplyReport`]. Macro `Change String Value` uses the
//!   same strict UTF-8 admission as the server/runtime command path before it
//!   can mutate retained text bytes.
//!
//! Command parameter layouts reuse the established machbus VT command path
//! shapes for the modelled subset. Commands with a leading selector byte, such
//! as Change Soft Key Mask's standard Mask Type field, keep that selector
//! before the referenced object IDs.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("macros/macro_effects_and_pool_mutations.rs");
include!("macros/macro_value_updates_and_triggers.rs");
include!("macros/macro_runtime_tests.rs");
