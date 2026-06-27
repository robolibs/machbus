//! Deterministic software framebuffer backend for VT render commands.
//!
//! This backend is deliberately small: it consumes the backend-neutral
//! [`RenderCommand`] stream and paints into an in-memory RGB framebuffer. It is
//! not a GPU/window integration and it does not parse object pools. Hosted test
//! tools, screenshots, and future SPI/framebuffer experiments can use it as a
//! concrete non-GTUI drawing target while the VT core remains backend-neutral.

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("framebuffer/framebuffer_core.rs");
include!("framebuffer/framebuffer_primitives_and_gauges.rs");
include!("framebuffer/framebuffer_tests.rs");
