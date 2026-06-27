//! ISO 11783-6 VT object pool: types, bodies, [`VTObject`] container,
//! [`ObjectPool`] with serialize / deserialize / validate.
//!
//! Mirrors the C++ `machbus::isobus::vt::objects.hpp`. VT version-6
//! features (touch gestures, 24-bit graphics context, scaled bitmap,
//! external object refs, colour palette) are included at the bottom.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("objects/object_ids_and_base_types.rs");
include!("objects/standard_object_bodies.rs");
include!("objects/graphics_context_and_pool_core.rs");
include!("objects/pool_validation_and_builders.rs");
include!("objects/extended_object_bodies.rs");
include!("objects/object_pool_tests.rs");
