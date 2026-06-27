//! GTUI renderer backend.
//!
//! "GTUI" here means a **retained-mode, GPU-less terminal scene
//! renderer**: the backend walks a [`Scene`] and emits an ordered list
//! of [`RenderCommand`]s. A host terminal (real display, screenshot
//! dumper, headless test fixture) consumes that list and draws it with
//! whatever native primitives it has.
//!
//! The backend is deliberately simple and deterministic so it can run
//! in `no_std`-friendly contexts and in unit tests. It does not own a
//! font rasteriser; text is emitted as a `DrawText` command carrying
//! the resolved cell origin, the resolved style, and the lossy-decoded
//! string.
//!
//! [`Scene`]: crate::isobus::vt::render::scene::Scene

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("gtui/gtui_renderer_and_png_decode.rs");
include!("gtui/png_filter_deflate_and_tests.rs");
