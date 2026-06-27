//! Layout engine: builds a [`Scene`] from a validated [`ObjectPool`].
//!
//! Responsibilities:
//!
//! - Resolve the active mask (Data / Alarm / Window) and its background.
//! - Walk mask and container children into placed [`SceneNode`]s.
//! - Supply concrete `(x, y)` placements: VT object-pool children in the
//!   machbus pool model are exposed as bare object ids (see
//!   `objects::split_body_and_children`), so the layout engine either
//!   honours caller-supplied placements ([`LayoutConfig`] +
//!   [`PlacementMap`]) or falls back to a deterministic vertical
//!   auto-stack.
//! - Resolve style references (font / line / fill) and current variable
//!   values into node content.
//! - Resolve the mask's soft-key mask into the soft-key area.
//! - Propagate visibility / enabled state from VT option bits.
//! - Record unsupported object families instead of dropping them.
//!
//! The engine never panics on a structurally odd pool: malformed bodies
//! are demoted to [`NodeKind::Unsupported`] / [`UnsupportedRecord`] so
//! the operator-facing terminal keeps rendering the rest of the mask.
//!
//! [`ObjectPool`]: crate::isobus::vt::objects::ObjectPool

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("layout/layout_config_and_helpers.rs");
include!("layout/layout_list_item_resolution.rs");
include!("layout/layout_engine_scene_build.rs");
include!("layout/input_validation_number_format_tests.rs");
