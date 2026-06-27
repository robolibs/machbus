//! Live VT render runtime.
//!
//! [`LayoutEngine`] is intentionally pure: given a pool and active mask, it
//! produces a [`Scene`]. [`VtRenderRuntime`] owns the long-lived state around
//! that pure step: active mask selection, runtime visibility/enabled overlays,
//! input focus/edit state, dirty tracking, deterministic command rendering, and
//! the first payload-ready VT-to-ECU bridge for completed operator events.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("runtime/runtime_types_and_state.rs");
include!("runtime/runtime_command_application.rs");
include!("runtime/runtime_events_and_timers.rs");
include!("runtime/graphics_context_command_expansion.rs");
include!("runtime/graphics_context_canvas.rs");
include!("runtime/retained_state_pool_mutations.rs");
include!("runtime/generic_attribute_updates.rs");
include!("runtime/list_polygon_helpers.rs");
