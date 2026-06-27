//! Backend abstraction for hosted VT render targets.
//!
//! Backends consume the retained [`Scene`] and produce their own output
//! representation. They do not parse object pools or own VT protocol state.

use alloc::vec::Vec;
use core::convert::Infallible;

use crate::isobus::vt::render::framebuffer::{Framebuffer, FramebufferError, FramebufferRenderer};
use crate::isobus::vt::render::gtui::{GtuiRenderer, RenderCommand};
use crate::isobus::vt::render::scene::Scene;

/// Common interface for hosted VT render backends.
///
/// This trait intentionally stays small: a backend receives an already-built
/// scene and returns a backend-specific output. The object-pool parser, layout
/// engine, runtime state, caches, windows, GPUs, and hardware framebuffers stay
/// outside this contract.
pub trait VtRenderBackend {
    type Output;
    type Error;

    fn render(&self, scene: &Scene) -> Result<Self::Output, Self::Error>;
}

impl VtRenderBackend for GtuiRenderer {
    type Output = Vec<RenderCommand>;
    type Error = Infallible;

    fn render(&self, scene: &Scene) -> Result<Self::Output, Self::Error> {
        Ok(GtuiRenderer::render(self, scene))
    }
}

impl VtRenderBackend for FramebufferRenderer {
    type Output = Framebuffer;
    type Error = FramebufferError;

    fn render(&self, scene: &Scene) -> Result<Self::Output, Self::Error> {
        self.try_render_scene(scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::vt::ObjectID;
    use crate::isobus::vt::ObjectType;
    use crate::isobus::vt::render::scene::{NodeKind, Rect, SceneNode};
    use crate::isobus::vt::render::style::{Colour, ResolvedStyle};

    fn backend_scene() -> Scene {
        let mut scene = Scene::new(ObjectID::new(1), (32, 16));
        scene.nodes.push(SceneNode {
            id: ObjectID::new(2),
            object_type: ObjectType::DataMask,
            parent: ObjectID::new(1),
            rect: Rect::new(0, 0, 32, 16),
            clip: None,
            style: ResolvedStyle {
                background: Colour::rgb(1, 2, 3),
                ..ResolvedStyle::default()
            },
            visible: true,
            enabled: true,
            kind: NodeKind::Group {
                background: 0,
                transparent_bg: false,
                children: Vec::new(),
            },
        });
        scene
    }

    #[test]
    fn render_backends_share_scene_only_contract() {
        let scene = backend_scene();

        let commands = VtRenderBackend::render(&GtuiRenderer::default(), &scene).unwrap();
        assert!(!commands.is_empty());

        let framebuffer = VtRenderBackend::render(&FramebufferRenderer::default(), &scene).unwrap();
        assert_eq!(framebuffer.width(), 32);
        assert_eq!(framebuffer.height(), 16);
    }
}
