//! ISO 11783-6 Virtual Terminal — render / layout / GTUI runtime.
//!
//! This is the product-level VT surface called out as the top P0 gap in
//! `GAP.md`: a typed
//!
//! ```text
//! IOP bytes → ObjectPool → validated → render Scene → live runtime
//! ```
//!
//! pipeline. It does **not** ship a GPU or a calibrated font rasteriser;
//! instead hosted backends consume a retained [`Scene`] and produce either a
//! command stream or a concrete framebuffer snapshot.
//!
//! # Module layout
//!
//! - [`audio`] — VT audio termination notification/response helpers.
//! - [`backend`] — the small hosted backend trait.
//! - [`bus_message`] — VT-to-ECU event payload builders/parsers.
//! - [`graphics_context_response`] — F.57 Graphics Context response helpers.
//! - [`scene`] — typed scene graph ([`Scene`], [`SceneNode`], [`Rect`]).
//! - [`style`] — palette / colour / font style model.
//! - [`text`] — monospace text measurement, layout, alignment, wrapping, and clipping.
//! - [`layout`] — the [`LayoutEngine`] that builds a scene from a pool.
//! - [`input`] — focus / edit / soft-key runtime that preserves active
//!   selected/open input objects by ID across scene rebuilds.
//! - [`mask_change_messages`] — active/soft-key mask error notifications and responses.
//! - [`operator_response`] — ECU responses to soft-key/button/pointing operator events.
//! - [`user_layout`] — VT On User-Layout Hide/Show response helpers.
//! - [`value_response`] — ECU responses to numeric/string value changes.
//! - [`gtui`] — the [`GtuiRenderer`] backend.
//! - [`framebuffer`] — deterministic software framebuffer backend for command snapshots.
//! - [`IopDocument`] — the high-level pipeline entry point.
//! - [`coverage`] — the per-object-type render coverage ledger.
//!
//! # Example
//!
//! ```
//! use machbus::isobus::vt::render::gtui::GtuiRenderer;
//! use machbus::isobus::vt::render::{IopDocument, LayoutConfig};
//! use machbus::isobus::vt::{
//!     create_data_mask, create_working_set, DataMaskBody, ObjectPool, WorkingSetBody,
//! };
//!
//! // `bytes` is an uploaded IOP object pool. Here we build a minimal one:
//! // one Working Set whose first child is a Data Mask.
//! let pool = ObjectPool::default()
//!     .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
//!     .with_object(create_data_mask(2, &DataMaskBody::default()));
//! let bytes = pool.serialize().unwrap();
//!
//! let doc = IopDocument::load(&bytes, LayoutConfig::default()).expect("pool parses");
//! let scene = doc.scene();
//! let cmds = GtuiRenderer::default().render(scene);
//! println!("{} draw commands", cmds.len());
//! ```
//!
//! [`RenderCommand`]: gtui::RenderCommand
//! [`Scene`]: scene::Scene
//! [`SceneNode`]: scene::SceneNode
//! [`Rect`]: scene::Rect

pub mod animation;
pub mod audio;
pub mod backend;
pub mod bus_message;
pub mod framebuffer;
pub mod graphics_context_response;
pub mod gtui;
pub mod input;
pub mod layout;
pub mod macros;
pub mod mask_change_messages;
pub mod operator_response;
pub mod runtime;
pub mod scene;
pub mod style;
pub mod text;
pub mod user_layout;
pub mod value_response;

pub use animation::{AnimationFrame, animation_frame, is_looping};
pub use audio::{ControlAudioSignalTermination, ControlAudioSignalTerminationResponse};
pub use backend::VtRenderBackend;
pub use bus_message::{VtBusMessage, VtBusMessageKind, validate_vt_to_ecu_envelope};
pub use framebuffer::{Framebuffer, FramebufferRenderer};
pub use graphics_context_response::{GraphicsContextErrorFlags, GraphicsContextResponse};
pub use gtui::{GtuiRenderer, RenderCommand};
pub use input::{EditState, InputRuntime, OperatorEvent, VtEvent};
pub use layout::{LayoutConfig, LayoutEngine, PlacementMap, RuntimeOverrides};
pub use macros::{
    MacroApplyReport, MacroEffect, MacroTriggerIndex, apply_macro_effects, decode_macro_effects,
};
pub use mask_change_messages::{
    ChangeActiveMaskError, ChangeActiveMaskResponse, ChangeSoftKeyMaskError,
    ChangeSoftKeyMaskResponse, MaskErrorFlags,
};
pub use operator_response::{
    ControlActivationResponse, ControlActivationResponseKind, PointingEventResponse,
    SelectInputObjectResponse, VtEscResponse,
};
pub use runtime::{
    ActivationHoldTiming, AnimationTick, RenderUpdate, UserLayoutPlacement, VtRenderRuntime,
    VtRuntimeCommand,
};
pub use scene::{
    ChildPlacement, NodeKind, Rect, Scene, SceneLanguage, SceneNode, SoftKeyKind, SoftKeyNode,
    UnsupportedRecord,
};
pub use style::{Colour, FillType, FontDecoration, FontMetrics, Palette, ResolvedStyle};
pub use user_layout::{
    UserLayoutHideShowRecord, UserLayoutHideShowResponse, validate_ecu_to_vt_response_envelope,
};
pub use value_response::{NumericValueChangeResponse, StringValueChangeResponse};

use crate::isobus::vt::{ObjectID, ObjectPool};
use crate::net::Result;

/// One VT object pool loaded as a renderable document: the deserialised
/// + validated pool, plus a [`Scene`] built for the active mask.
///
/// This is the high-level entry point for a host that wants the full
/// "load bytes → render" pipeline in one call. Callers that need
/// finer control (custom active mask, custom placements, value updates)
/// should use [`LayoutEngine`] directly.
#[derive(Debug, Clone)]
pub struct IopDocument {
    pool: ObjectPool,
    scene: Scene,
}

impl IopDocument {
    /// Load raw IOP bytes, deserialise, validate, and build a scene for
    /// the standard initial mask (first Working Set child).
    ///
    /// Never panics: malformed bytes are reported as a [`net::Error`] of
    /// kind [`ErrorCode::PoolValidation`] /
    /// [`ErrorCode::InvalidData`].
    ///
    /// [`net::Error`]: crate::net::Error
    /// [`ErrorCode::PoolValidation`]: crate::net::ErrorCode::PoolValidation
    /// [`ErrorCode::InvalidData`]: crate::net::ErrorCode::InvalidData
    pub fn load(bytes: &[u8], config: LayoutConfig) -> Result<Self> {
        let pool = ObjectPool::deserialize(bytes)?;
        pool.validate()?;
        let scene = LayoutEngine::new(config).build(&pool, ObjectID::NULL);
        Ok(Self { pool, scene })
    }

    /// Build a document from an already-deserialised, already-validated
    /// pool. Useful when the caller wants to share one pool across many
    /// scenes or mutate it (e.g. apply value-change commands) before
    /// rendering.
    pub fn from_pool(pool: &ObjectPool, config: LayoutConfig) -> Result<Self> {
        pool.validate()?;
        let scene = LayoutEngine::new(config).build(pool, ObjectID::NULL);
        Ok(Self {
            pool: pool.clone(),
            scene,
        })
    }

    /// Rebuild the scene against the same pool, optionally targeting a
    /// different mask and/or custom placements.
    #[must_use]
    pub fn with_scene(mut self, engine: &LayoutEngine, active_mask: ObjectID) -> Self {
        let pool = self.pool.clone();
        self.scene = engine.build(&pool, active_mask);
        self
    }

    #[inline]
    #[must_use]
    pub fn pool(&self) -> &ObjectPool {
        &self.pool
    }

    #[inline]
    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Coverage ledger for this document: which object types appear in
    /// the pool and whether the renderer covers them.
    #[must_use]
    pub fn coverage(&self) -> coverage::DocumentCoverage {
        coverage::coverage_for(&self.pool, &self.scene)
    }
}

/// Per-object-type render coverage metadata.
///
/// Keeps the "every object type is rendered or explicitly rejected"
/// ledger required by `GAP.md` honest. The data is repo-owned
/// (object-type names + renderer status), not standard prose.
pub mod coverage {
    use crate::isobus::vt::render::scene::Scene as RenderScene;
    use crate::isobus::vt::{ObjectID, ObjectPool, ObjectType};

    /// One row of the document-specific coverage ledger.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CoverageRow {
        pub object_type: ObjectType,
        pub render_status: RenderStatus,
        pub count_in_pool: usize,
        pub count_rendered: usize,
        pub count_unsupported: usize,
    }

    /// One row of the standard-family coverage ledger. Rows with
    /// `object_type == None` are standard object families that are not
    /// represented by the current `ObjectType` model yet.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CoverageLedgerRow {
        pub name: &'static str,
        pub object_type: Option<ObjectType>,
        pub render_status: RenderStatus,
        pub draws_directly: bool,
        pub resolves_value: bool,
        pub interactive: bool,
        pub notes: &'static str,
    }

    /// How the renderer treats an object type.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RenderStatus {
        /// Drawn directly (output shapes, strings, inputs, meters, …).
        Drawable,
        /// Consumed as style/value data by other drawable nodes (font /
        /// fill / line attributes, variables).
        ReferenceResolved,
        /// Drawn directly and participates in operator input/focus.
        Interactive,
        /// Drawn as a soft-key cell, not a data-mask node.
        SoftKey,
        /// Parsed and tracked, but not rendered as a faithful visual yet.
        ParsedButNotRendered,
        /// Present in the standard inventory but absent from the object model.
        MissingObjectModel,
        /// Deliberately not rendered by this layer.
        OutOfScope,
    }

    impl RenderStatus {
        #[must_use]
        pub const fn label(self) -> &'static str {
            match self {
                Self::Drawable => "drawable",
                Self::ReferenceResolved => "reference-resolved",
                Self::Interactive => "interactive",
                Self::SoftKey => "soft-key",
                Self::ParsedButNotRendered => "parsed-but-not-rendered",
                Self::MissingObjectModel => "missing-object-model",
                Self::OutOfScope => "out-of-scope",
            }
        }

        #[must_use]
        pub const fn is_scene_rendered(self) -> bool {
            matches!(self, Self::Drawable | Self::Interactive | Self::SoftKey)
        }
    }

    /// The coverage ledger for one document.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DocumentCoverage {
        pub rows: Vec<CoverageRow>,
        pub total_objects: usize,
        pub total_drawable: usize,
        pub total_unsupported: usize,
    }

    impl DocumentCoverage {
        /// Pretty-print the ledger to a CSV string (header + rows).
        #[must_use]
        pub fn to_csv(&self) -> String {
            let mut out = String::from(
                "object_type,render_status,count_in_pool,count_rendered,count_unsupported\n",
            );
            for row in &self.rows {
                out.push_str(&format!(
                    "{},{},{},{},{}\n",
                    type_name(row.object_type),
                    row.render_status.label(),
                    row.count_in_pool,
                    row.count_rendered,
                    row.count_unsupported,
                ));
            }
            out
        }
    }

    /// Build the ledger for a pool + the scene the renderer produced
    /// from it. Types absent from the pool are omitted.
    #[must_use]
    pub fn coverage_for(pool: &ObjectPool, scene: &RenderScene) -> DocumentCoverage {
        let mut rows = Vec::new();
        let mut total_objects = 0usize;
        let mut total_drawable = 0usize;
        let mut total_unsupported = 0usize;

        for ty in all_object_types() {
            let in_pool = pool.objects().iter().filter(|o| o.r#type == ty).count();
            if in_pool == 0 {
                continue;
            }
            total_objects += in_pool;

            let status = render_status_for(ty);

            // Count how many instances of this type ended up as visible
            // scene nodes (drawable) or soft-key cells.
            let rendered = match status {
                RenderStatus::Drawable | RenderStatus::Interactive => {
                    scene.nodes.iter().filter(|n| n.object_type == ty).count()
                }
                RenderStatus::SoftKey => scene
                    .soft_keys
                    .iter()
                    .filter(|s| s.id != ObjectID::NULL)
                    .count()
                    .min(in_pool),
                _ => 0,
            };

            let unsupported = scene
                .unsupported
                .iter()
                .filter(|r| r.object_type == ty)
                .count();

            if status.is_scene_rendered() {
                total_drawable += rendered;
            }
            total_unsupported += unsupported;

            rows.push(CoverageRow {
                object_type: ty,
                render_status: status,
                count_in_pool: in_pool,
                count_rendered: rendered,
                count_unsupported: unsupported,
            });
        }

        DocumentCoverage {
            rows,
            total_objects,
            total_drawable,
            total_unsupported,
        }
    }

    /// Static classification of every standard render-relevant object
    /// family known to this crate. This includes object families the
    /// current object model does not expose yet, so docs and tests can
    /// show missing coverage honestly instead of silently omitting it.
    #[must_use]
    pub const fn coverage_ledger() -> &'static [CoverageLedgerRow] {
        &[
            CoverageLedgerRow {
                name: "WorkingSet",
                object_type: Some(ObjectType::WorkingSet),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "root container; standard child/macro/language tail selects the active mask and exposes Working Set language codes unless Working Set Special Controls supplies language/country pairs",
            },
            CoverageLedgerRow {
                name: "DataMask",
                object_type: Some(ObjectType::DataMask),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "main display mask with background and child list; fixed fields respond to generic attributes, including the standard NULL Soft Key Mask clear on AID 2",
            },
            CoverageLedgerRow {
                name: "AlarmMask",
                object_type: Some(ObjectType::AlarmMask),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "priority mask rendered through the mask path; priority/acoustic fixed fields are range-gated, Soft Key Mask AID 2 admits the standard NULL clear, and Change Priority updates alarm-ordering metadata",
            },
            CoverageLedgerRow {
                name: "WindowMask",
                object_type: Some(ObjectType::WindowMask),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "free-form windows use 2x6 cells; width/height/type/options generic attributes are range-gated; changed placements that grow or overlap are removed without moving stable unchanged user-layout mappings including after Lock/Unlock Mask deferral, and multiple changed-placement conflicts tie-break by Object ID; user-layout H.20 hide/show records are object-ID sorted before packing; non-NULL name/title references are typed to OutputString or ObjectPointer-to-OutputString and non-NULL icon references are typed to Object Label graphic-representation output objects; transparent available windows preserve underlying user-layout pixels; unavailable windows blank their cells with the user-layout background even when transparent; active or nested typed/free-form windows place children or required-object slots with child drawing clipped to the Window Mask region, intersecting object-local OutputList/Animation clips",
            },
            CoverageLedgerRow {
                name: "Container",
                object_type: Some(ObjectType::Container),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "translated child group; width/height/hidden are Get-Attribute-only/read-only for Change Attribute, with Change Size and Hide/Show carrying the runtime mutations",
            },
            CoverageLedgerRow {
                name: "SoftKeyMask",
                object_type: Some(ObjectType::SoftKeyMask),
                render_status: RenderStatus::SoftKey,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "drives the soft-key area with optional physical-key paging, clamped navigation reservation, host physical-cell one-shot and down/up events plus direct SoftKeyActivate events checked against the visible active scene, explicit Commit activation for selected focus-only visible cells, semantic activation lowering guarded by visible active cells, Key/ObjectPointer/ExternalObjectPointer child resolution including host-registered external Key targets, inactive boundary navigation cells that wait until physical release or focus-only Commit before paging and cannot be overwritten by another pointer, tap, one-shot physical, direct semantic soft-key, or direct navigation activation, with hardware-origin down state surviving stray pointer move/release events until physical release, NULL-slot reservation, and trailing-NULL page-count trimming; ObjectPointer retargets and ExternalObjectPointer default retargets preserve Key/NULL slot constraints; background changes update the mask backing colour",
            },
            CoverageLedgerRow {
                name: "Key",
                object_type: Some(ObjectType::Key),
                render_status: RenderStatus::SoftKey,
                draws_directly: false,
                resolves_value: true,
                interactive: true,
                notes: "one soft-key cell with a zero-based physical cell index and resolved Key Number; label resolves child output objects; vertical columns and horizontal/landscape soft-key rows both use the configured physical cell geometry; OutputList selected-Key items materialise as clipped display-only designators rather than unsupported records; default pointer activation enforces one pressed/held soft key at a time until release, pointer slide-off aborts the pressed key, focus-only Commit activates a selected visible Key, physical soft-key down/up emits Pressed/Held/Released or Aborted for hardware profiles, and display changes such as active-mask or soft-key-mask changes queue a release for an erased pressed key before ignoring the physical release; only the standard Key AIDs 1..=2 are admitted for generic attributes",
            },
            CoverageLedgerRow {
                name: "KeyGroup",
                object_type: Some(ObjectType::KeyGroup),
                render_status: RenderStatus::SoftKey,
                draws_directly: true,
                resolves_value: true,
                interactive: true,
                notes: "available groups render/place as one-to-four non-overlapping user-layout key cells after direct Key/ObjectPointer/ExternalObjectPointer child resolution, including host-registered external Key targets and their Key Numbers; configured vertical soft-key columns and horizontal/landscape soft-key rows both drive placement, GTUI cell emission, pointer hit-testing, and physical-key activation; placement rejects active Soft Key Mask application cells and the actually rendered navigation cells after Soft Key Mask child resolution and trailing NULL trimming, not raw child-count heuristics, stale remembered placements are removed if a later Soft Key Mask claims those cells, changed placements that grow/overlap are removed without moving stable unchanged mappings including after Lock/Unlock Mask deferral, multiple changed-placement conflicts tie-break by Object ID, and exported host snapshots plus H.20 hide/show records are object-ID sorted; name/icon references are typed to standard mapping-screen designators; unresolved external pointer children reserve blank non-activating cells instead of collapsing the slot map; child-label resolution, transparent backing for available groups, mandatory blank/fill for unavailable groups, tap/physical-soft-key/press/move/release activation including immediate pointer-slide-off aborts and hardware physical-key down/up Pressed/Held/Released sequencing that ignores stray pointer move/release events until the physical release, default one-pressed-key-at-a-time pointer exclusivity, visible/enabled semantic activation lowering, Select Input Object focus-only replay and explicit Commit activation for visible contained Keys, generic attributes, and runtime ObjectPointer/ExternalObjectPointer retarget validation preserve Key slot constraints",
            },
            CoverageLedgerRow {
                name: "Button",
                object_type: Some(ObjectType::Button),
                render_status: RenderStatus::Interactive,
                draws_directly: true,
                resolves_value: true,
                interactive: true,
                notes: "inline button resolves child output text into its TextLayout label, resolves background/border colours through the active palette, honours transparent-background and suppress-border/no-border options, carries the resolved Key Number for local or external materialised buttons, uses standard option bit 4 for disabled initial state, supports pointer activation events with default one-pressed-button-at-a-time exclusivity, hardware Enter and explicit Commit activation for focused/selected enabled Buttons, aborts when the pointer slides off the pressed button, queues a release when a display change erases a pressed button or moves a touch-pressed button away from its touch point, and requires visible/enabled scene state for semantic activation lowering",
            },
            CoverageLedgerRow {
                name: "InputBoolean",
                object_type: Some(ObjectType::InputBoolean),
                render_status: RenderStatus::Interactive,
                draws_directly: true,
                resolves_value: true,
                interactive: true,
                notes: "toggle on operator activation; fixed value and enabled fields are Get-Attribute-only/read-only for Change Attribute; geometry and typed FontAttributes/NumberVariable references respond to generic attributes, with required foreground FontAttributes AID 3 rejecting NULL; server/runtime Change Numeric Value rejects non-boolean values and Enable/Disable controls runtime enabled state",
            },
            CoverageLedgerRow {
                name: "InputString",
                object_type: Some(ObjectType::InputString),
                render_status: RenderStatus::Interactive,
                draws_directly: true,
                resolves_value: true,
                interactive: true,
                notes: "editable string uses TextLayout with full horizontal/vertical justification; required FontAttributes AID 4 rejects NULL during Change Attribute replay; standard options AID supported as transparent-background/auto-wrap bits while retaining wrap-on-hyphen as standard option metadata, with auto-wrap feeding the command layout and background colour applied only when non-transparent; runtime-disabled fields ignore taps before focus/open mutation; stale hidden/disabled focused input state clears before type/backspace/commit can emit value events; classic byte-oriented and extended wide-character validation enforced before edit-buffer mutation and before opening a transaction; hardware characters on non-editable focused objects stay ignored without opening input state; preview vs commit events separated",
            },
            CoverageLedgerRow {
                name: "InputNumber",
                object_type: Some(ObjectType::InputNumber),
                render_status: RenderStatus::Interactive,
                draws_directly: true,
                resolves_value: true,
                interactive: true,
                notes: "editable number uses TextLayout with full horizontal/vertical justification; required FontAttributes AID 4 rejects NULL during Change Attribute replay; finite scale plus standard 0..=7 decimals, fixed/exponential format, and transparent-background/leading-zero/zero-as-blank/truncate options affect field fill and formatted text; raw value and Options 2 are Get-Attribute-only/read-only for Change Attribute while reserved decimal/format values are rejected; Options 2 bit 1 controls real-time editing and Enable/Disable overlays bit 0; disabled fields ignore taps before focus/open mutation; host key input accepts only decimal digits before edit-buffer mutation; preview vs real-time value events separated",
            },
            CoverageLedgerRow {
                name: "InputList",
                object_type: Some(ObjectType::InputList),
                render_status: RenderStatus::Interactive,
                draws_directly: true,
                resolves_value: true,
                interactive: true,
                notes: "selected index resolved from value state through accepted Change Numeric Value retention; standard Value AID 4 is Get-Attribute-only/read-only for Change Attribute; upload validation types the NumberVariable reference and non-NULL item slots while preserving NULL placeholders; selected item text resolves through OutputString/OutputNumber/variable/ObjectPointer/ExternalObjectPointer/Container paths using the host-registered referenced-pool resolver where available and is drawn through TextLayout; selected non-interactive drawable items can materialise as clipped display-value scene nodes while the parent InputList remains the hit target; disabled fields ignore taps before focus/open/list-selection mutation; value 255/out-of-range/NULL slots/unavailable external slots/non-displayable slots stay blank without a diagnostic index/count fallback; operator selection skips true NULL item slots but keeps standard empty ObjectPointer-NULL and hidden-Container items selectable; Change List Item retargets item slots",
            },
            CoverageLedgerRow {
                name: "OutputString",
                object_type: Some(ObjectType::OutputString),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "resolves inline value or StringVariable including ISO WideString decoding and emits TextLayout with full horizontal/vertical justification; required FontAttributes AID 4 rejects NULL during Change Attribute replay; transparent-background option controls whether the field background colour is painted; Change String Value preserves fixed length with space padding and runtime/pool macro replay rejects invalid UTF-8 before retained text mutation; variable-backed OutputString objects are not implicit Change String Value retargets",
            },
            CoverageLedgerRow {
                name: "OutputNumber",
                object_type: Some(ObjectType::OutputNumber),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "applies offset/finite scale/standard 0..=7 decimals plus the standard fixed/exponential format selector and transparent-background/leading-zero/zero-as-blank/truncate options; required FontAttributes AID 4 rejects NULL during Change Attribute replay; raw value is Get-Attribute-only/read-only for Change Attribute and changes through Change Numeric Value; reserved decimal/format values are rejected; emits TextLayout with full horizontal/vertical justification",
            },
            CoverageLedgerRow {
                name: "OutputList",
                object_type: Some(ObjectType::OutputList),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "selected index resolved from inline value or NumberVariable; selected drawable item materialises as a clipped scene node including host-resolved ExternalObjectPointer targets and display-only Key designators; width/height respond to Change Size; standard one-byte Value AID 4 is Get-Attribute-only/read-only for Change Attribute while Change Numeric Value updates the selected item; upload validation, Change List Item replay, and ObjectPointer retargeting reject item references that cannot be presented, and unavailable external items with no local default stay blank; standard no-display item cases stay blank without a diagnostic index/count fallback",
            },
            CoverageLedgerRow {
                name: "OutputLine",
                object_type: Some(ObjectType::Line),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: false,
                interactive: false,
                notes: "line primitive via LineAttributes; line art paintbrush spot patterns are carried to command and framebuffer replay; explicit line width zero suppresses the stroke; Change End Point updates width, height, and direction",
            },
            CoverageLedgerRow {
                name: "OutputRectangle",
                object_type: Some(ObjectType::Rectangle),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: false,
                interactive: false,
                notes: "fill plus per-edge line suppression; Fill Attributes type 1 fills with the resolved line colour, type 2 fills with the fill colour, and type 3 tiles PictureGraphic pattern data anchored to the mask origin while ignoring PictureGraphic transparency/flashing options; line art paintbrush spot patterns are carried to command and framebuffer replay",
            },
            CoverageLedgerRow {
                name: "OutputEllipse",
                object_type: Some(ObjectType::Ellipse),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: false,
                interactive: false,
                notes: "closed ellipse plus open arc, segment, and section primitives with half-degree start/end angles; Fill Attributes type 1 fills with the resolved line colour, type 2 fills with the fill colour, and type 3 tiles PictureGraphic pattern data for closed/section fills anchored to the mask origin while ignoring PictureGraphic transparency/flashing options; explicit line width zero suppresses the stroke; framebuffer preserves non-zero stroke width, including even two-pixel strokes, and line art paintbrush spot patterns",
            },
            CoverageLedgerRow {
                name: "OutputPolygon",
                object_type: Some(ObjectType::Polygon),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: false,
                interactive: false,
                notes: "polygon point list; required LineAttributes AID 3 rejects NULL during Change Attribute replay; Fill Attributes type 1 fills with the resolved line colour, type 2 fills with the fill colour, and type 3 tiles PictureGraphic pattern data anchored to the mask origin while ignoring PictureGraphic transparency/flashing options; framebuffer preserves non-zero stroke width, including even two-pixel strokes, and line art paintbrush spot patterns",
            },
            CoverageLedgerRow {
                name: "Meter",
                object_type: Some(ObjectType::Meter),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "needle value resolved from NumberVariable; framebuffer uses configured start/end angles for the arc, border/tick colours, numeric value option, and value-positioned needle; min>=max is accepted and drawn as the standard minimum-value clamp",
            },
            CoverageLedgerRow {
                name: "LinearBarGraph",
                object_type: Some(ObjectType::LinearBarGraph),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "linear value/target bar; standard target-value fields, border/target/tick/value-line option bits, orientation, and positive/negative direction feed framebuffer rasterisation; min>=max is accepted and draws value/target as the minimum",
            },
            CoverageLedgerRow {
                name: "ArchedBarGraph",
                object_type: Some(ObjectType::ArchedBarGraph),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "arched value/target bar; standard target-value fields, border/target/value-line option bits, start/end angle, deflection direction, and bar width feed framebuffer rasterisation; min>=max is accepted and draws value/target as the minimum",
            },
            CoverageLedgerRow {
                name: "PictureGraphic",
                object_type: Some(ObjectType::PictureGraphic),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "uncompressed and valid RLE indexed payloads emit backend-neutral image commands after minimum row-padded decoded length validation for 1-bit/4-bit rows; unused packed bits at the end of each PictureGraphic row are ignored, and extra bytes beyond the expected row-padded bitmap length are ignored/truncated per the raw-data rule while short or malformed compressed payloads stay placeholder; Options AID 2 Change Attribute preserves the static raw/RLE bit while updating transparent/flashing bits and actual width/height/format AIDs 4..=6 are Get-Attribute-only; target display width drives the render rectangle while actual dimensions remain the source bitmap dimensions; hosted framebuffer expansion uses the active scene ColourMap/ColourPalette and command-ordered copied-pixel updates clipped in target Picture Graphic coordinates with copied colour indexes outside the target bitmap format treated as transparent/no-copy; hosted GraphicsContext canvas expands packed indexed pixels during Draw VT Object replay and treats indexes outside the Graphics Context canvas format as transparent/no-copy",
            },
            CoverageLedgerRow {
                name: "NumberVariable",
                object_type: Some(ObjectType::NumberVariable),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "value backing for numeric display objects",
            },
            CoverageLedgerRow {
                name: "StringVariable",
                object_type: Some(ObjectType::StringVariable),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "value backing for string display objects including ISO WideString decoding; Change String Value preserves fixed length with space padding and runtime/pool macro replay rejects invalid UTF-8 before retained text mutation",
            },
            CoverageLedgerRow {
                name: "FontAttributes",
                object_type: Some(ObjectType::FontAttributes),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "resolved into font metrics, foreground colour, standard font-type codes, proportional-size/style compatibility, and decoration bits; hosted framebuffer text coverage consumes the resolved metrics and basic bold/italic/inverted/underline/strikethrough flags",
            },
            CoverageLedgerRow {
                name: "LineAttributes",
                object_type: Some(ObjectType::LineAttributes),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "resolved into line colour, exact width, and two-byte line art paintbrush spot pattern; Change Attribute preserves the full u16 line-art value instead of truncating to a colour-sized byte",
            },
            CoverageLedgerRow {
                name: "FillAttributes",
                object_type: Some(ObjectType::FillAttributes),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "resolved into fill type, colour, and PictureGraphic-only pattern references; generic replay rejects type-3 pattern PictureGraphics whose packed monochrome/16-colour rows would leave unused bits before retaining runtime or macro effects; standard Fill Type 1 ignores the Fill Colour field and fills shapes with the current line colour; valid pattern rows render as mask-origin-anchored tiled pattern fills",
            },
            CoverageLedgerRow {
                name: "InputAttributes",
                object_type: Some(ObjectType::InputAttributes),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "basic 8-bit character validation for InputString edits, including byte-only blacklist handling that rejects non-single-byte characters; ignored for WideString variables; Change String Value updates validation string without increasing fixed length; validation type is readable but not Change Attribute mutable",
            },
            CoverageLedgerRow {
                name: "ExtendedInputAttributes",
                object_type: Some(ObjectType::ExtendedInputAttributes),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "wide-character validation model enforced for WideString InputString edits with whitelist and blacklist code-plane ranges and ignored for 8-bit variables; duplicate code-plane records are rejected; validation type is readable but not Change Attribute mutable",
            },
            CoverageLedgerRow {
                name: "ObjectPointer",
                object_type: Some(ObjectType::ObjectPointer),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "placed pointers dereference and materialise their target; NULL pointer values are standard no-object blanks rather than unsupported placeholders; target value is Get Attribute Value readable and retargets through Change Numeric Value, not Change Attribute; server/runtime Change Numeric Value rejects missing targets and preserves stricter slot contexts such as SoftKeyMask, KeyGroup, and ScaledGraphic graphic-source chains",
            },
            CoverageLedgerRow {
                name: "Macro",
                object_type: Some(ObjectType::Macro),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "decoded/applied for selected runtime effects and event references including visibility enable input-selection audio-side-effects value background size end-point soft-key-mask list-item delete-pool priority object-label mask-lock nested-macro child-location child-position standard-AID-gated generic-attribute polygon point/scale colour-map and active-mask changes",
            },
            CoverageLedgerRow {
                name: "AuxFunction",
                object_type: Some(ObjectType::AuxFunction),
                render_status: RenderStatus::OutOfScope,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "AUX routing is protocol state, not a data-mask render node",
            },
            CoverageLedgerRow {
                name: "AuxInput",
                object_type: Some(ObjectType::AuxInput),
                render_status: RenderStatus::OutOfScope,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "AUX routing is protocol state, not a data-mask render node",
            },
            CoverageLedgerRow {
                name: "AuxFunction2",
                object_type: Some(ObjectType::AuxFunction2),
                render_status: RenderStatus::OutOfScope,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "AUX-N routing is protocol state, not a data-mask render node",
            },
            CoverageLedgerRow {
                name: "AuxInput2",
                object_type: Some(ObjectType::AuxInput2),
                render_status: RenderStatus::OutOfScope,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "AUX-N routing is protocol state, not a data-mask render node",
            },
            CoverageLedgerRow {
                name: "AuxControlDesignator",
                object_type: Some(ObjectType::AuxControlDesig),
                render_status: RenderStatus::OutOfScope,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "AUX designator is not drawn by the retained renderer",
            },
            CoverageLedgerRow {
                name: "WorkingSetSpecialControls",
                object_type: Some(ObjectType::WorkingSetSpecialControls),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "initial colour-map/palette applied with NULL/missing Colour Palette selecting the VT default palette instead of an arbitrary pool palette; Select Colour Map/Palette and Change Attribute update retained colour selections, and server Get Attribute Value AID 2/3 reports the live retained selection instead of stale generic attribute overlays; language-pair controls exposed on Scene and validated as two-letter pairs plus the standard two-space country sentinel, with deterministic ASCII-case-insensitive host-preference selection",
            },
            CoverageLedgerRow {
                name: "GraphicData",
                object_type: Some(ObjectType::GraphicData),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "standard Graphic Data body is modelled as PNG payload data with u32 length; hosted renderer can decode non-interlaced or Adam7-interlaced grayscale/indexed/grayscale-alpha/RGB/RGBA PNGs, including 8- and 16-bit grayscale/grayscale-alpha/RGB/RGBA (16-bit channels down-sampled to 8 bits per channel), with zlib DEFLATE streams, including stored, fixed-Huffman, and dynamic-Huffman blocks with literal and length/distance-copy data plus PNG chunk CRC checks, scanline filters 0..4, and palette/grayscale/true-colour transparency, into RGBA commands, otherwise reporting precise malformed-vs-unsupported placeholders; Change Attribute is not admitted for this object",
            },
            CoverageLedgerRow {
                name: "ScaledGraphic",
                object_type: Some(ObjectType::ScaledGraphic),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "PictureGraphic and ObjectPointer graphic value chains emit backend-neutral indexed-image commands after optional RLE decode and minimum row-padded decoded length validation; unused packed bits at the end of each PictureGraphic row are ignored, extra bytes inherited from PictureGraphic sources are ignored/truncated, and short or malformed compressed payloads stay placeholder; supported standard GraphicData PNG references emit RGBA image commands while unsupported PNG format variants stay explicit placeholders; hosted framebuffer expansion uses the active scene ColourMap/ColourPalette or PNG RGBA pixels; the standard ScaleType byte controls scaling mode and horizontal/vertical justification inside the Width/Height field; fixed fields respond to generic attributes and Change Numeric Value including NULL no-graphic; upload/server/runtime validation rejects non-graphic or cyclic ObjectPointer value-source chains",
            },
            CoverageLedgerRow {
                name: "Animation",
                object_type: Some(ObjectType::Animation),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "standard Value/Enabled/first-default-last indices and positional child records select and place the active child with upload/server/runtime bounds checks including Change Numeric Value while preserving Value 255 as no-selection; Change List Item updates positional frame child records through server/runtime/macro replay; valid out-of-sequence selected children remain visible until the next refresh tick, then First/Last range-checking feeds single-shot or loop advancement; disabled-behaviour option bits are honoured; runtime animation clocks are per object, advance only while visible/enabled, keep multiple visible instances on the same referenced frame, and rebuild only when a visible active frame changes; frame payload may still be placeholder",
            },
            CoverageLedgerRow {
                name: "ColourMap",
                object_type: Some(ObjectType::ColourMap),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "pixel-format colour mapping metadata; body codec uses the standard two-byte entry count and admits only standard 2/16/256-entry colour-map sizes",
            },
            CoverageLedgerRow {
                name: "GraphicContext",
                object_type: Some(ObjectType::GraphicContext),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "standard graphics context exposes a canvas surface; full type-36 body covers viewport/canvas, canonical two-byte viewport/cursor positions, initial zoom/cursor, foreground/background colours, font/line/fill references, canvas format, options, and transparency colour; writable generic attributes rebuild viewport/cursor/zoom/colour/style/format/options/transparency state while read-only Canvas Width/Height AIDs 5/6 stay Get-Attribute-only; pool validation checks font/line/fill attribute references before upload/runtime use; transparent canvas commands preserve the configured transparency colour in hosted indexed backing pixels and Copy Canvas/Viewport payloads carry it as the standard do-not-copy index, with copied colour indexes outside the target Picture Graphic bitmap format also treated as transparent/no-copy; Change Background Colour, including macro replay, updates the opaque canvas backing colour and the canvas command's initial background index so Copy Canvas can preserve that backing in later Picture Graphic pixels; hosted framebuffer fills opaque canvas backing, preserves transparent backing, directly interprets basic cursor/colour erase/point/line/rectangle/ellipse/polygon/DrawText with NULL Font Attributes resetting direct text replay to the default text colour, NULL line-attribute disable plus non-NULL restore, non-NULL fill-attribute rectangle/ellipse/closed-polygon fills, and viewport replay clipped to the viewport, and keeps a per-GC indexed canvas so Copy Canvas/Viewport intent updates Picture Graphic pixels without sampling visible framebuffer outlines with copy-time or remembered viewport zoom and target-Picture-Graphic coordinate clipping, ignoring direct copy intent when no bounded GC-owned backing canvas exists; line-attribute colour selectors, stroke widths, line art paintbrush spot patterns, indexed image expansion, and standard PNG/RGBA GraphicData quantisation to the nearest active VT palette entry are preserved in backend commands and copied indexed pixels, while Draw VT Object replay treats colours outside the Graphics Context canvas format as transparent; accepted subcommands replay after known object-reference payloads are type/self-recursion checked including drawable Draw VT Object targets, explicit rejection of ScaledBitmap compatibility targets, non-NULL Picture Graphic copy targets, and F.56 counted DrawText including zero-length text, and core drawing/viewport/object-draw/copy-to-picture subcommands expand to backend-neutral commands",
            },
            CoverageLedgerRow {
                name: "ExternalObjectDefinition",
                object_type: Some(ObjectType::ExternalObjectDefinition),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "standard VT5 enabled/NAME/object-list metadata; Change Attribute updates options/NAME fields and Change List Item updates the referenced-object list",
            },
            CoverageLedgerRow {
                name: "ExternalReferenceName",
                object_type: Some(ObjectType::ExternalReferenceName),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "standard VT5 enabled/NAME metadata; Change Attribute updates options and NAME fields",
            },
            CoverageLedgerRow {
                name: "ExternalObjectPointer",
                object_type: Some(ObjectType::ExternalObjectPointer),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "standard VT5 default-object/reference-NAME/external-object-id model; hosted renderer draws the local default object when no external-pool resolver can validate the external target or when the validated target is an ObjectPointer chain ending at NULL, while NULL local defaults blank instead of producing unsupported placeholders; live external-pool register/unregister and local-name changes respect active-mask refresh locks, revalidate grants on local-name change, and treat repeated same-context host updates as no-ops; SoftKeyMask entries can materialise registered external Key targets when NAME/definition grants validate; four-byte Change Numeric Value retargets both reference NAME and external object ID while rejecting invalid local reference NAME targets in server/runtime paths; Change Attribute updates individual pointer fields, accepts NULL reference NAME fallback, and preserves SoftKeyMask Key/NULL default-slot constraints",
            },
            CoverageLedgerRow {
                name: "ColourPalette",
                object_type: Some(ObjectType::ColourPalette),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "standard VT6 reserved-options plus u16-count ARGB body in B,G,R,A byte order; nonzero reserved options and count/body mismatches rejected; overlays the render palette from index 0 upward, with alpha retained in the object model but ignored by the current opaque RGB renderer; Change Attribute admits only reserved Options AID value zero",
            },
            CoverageLedgerRow {
                name: "GraphicsContext",
                object_type: Some(ObjectType::GraphicsContext),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "machbus compatibility extension (not an ISO 11783-6 object); this geometry-less graphics-context state object is rendered best-effort as a fixed-extent fill+border swatch from its 24-bit RGB fill/line state and line width/style; a fully transparent context draws nothing; accepted subcommands replay as backend command stream entries",
            },
            CoverageLedgerRow {
                name: "ObjectLabelRef",
                object_type: Some(ObjectType::ObjectLabelRef),
                render_status: RenderStatus::ReferenceResolved,
                draws_directly: false,
                resolves_value: true,
                interactive: false,
                notes: "object label reference list materialised as VT popup/editor metadata with standard font-type and output/drawable graphic-designator type admission",
            },
            CoverageLedgerRow {
                name: "ScaledBitmap",
                object_type: Some(ObjectType::ScaledBitmap),
                render_status: RenderStatus::Drawable,
                draws_directly: true,
                resolves_value: true,
                interactive: false,
                notes: "machbus compatibility extension (not an ISO 11783-6 object); the referenced GraphicData bytes are interpreted as a raw 1/4/8-bit indexed bitmap (optionally RLE compressed) or 24-bit direct-RGB bitmap (format 3, expanded to opaque RGBA), then drawn into a scaled, offset image command; standard PNG GraphicData is not consumed through this legacy shortcut",
            },
        ]
    }

    /// Classification of every VT object type the renderer knows about.
    #[must_use]
    pub fn render_status_for(ty: ObjectType) -> RenderStatus {
        coverage_ledger()
            .iter()
            .find_map(|row| (row.object_type == Some(ty)).then_some(row.render_status))
            .unwrap_or(RenderStatus::MissingObjectModel)
    }

    /// Iterate over every defined VT object type (used by the ledger).
    pub fn all_object_types() -> impl Iterator<Item = ObjectType> {
        [
            ObjectType::WorkingSet,
            ObjectType::DataMask,
            ObjectType::AlarmMask,
            ObjectType::Container,
            ObjectType::SoftKeyMask,
            ObjectType::Key,
            ObjectType::Button,
            ObjectType::InputBoolean,
            ObjectType::InputString,
            ObjectType::InputNumber,
            ObjectType::InputList,
            ObjectType::OutputString,
            ObjectType::OutputNumber,
            ObjectType::OutputList,
            ObjectType::Line,
            ObjectType::Rectangle,
            ObjectType::Ellipse,
            ObjectType::Polygon,
            ObjectType::Meter,
            ObjectType::LinearBarGraph,
            ObjectType::ArchedBarGraph,
            ObjectType::PictureGraphic,
            ObjectType::NumberVariable,
            ObjectType::StringVariable,
            ObjectType::FontAttributes,
            ObjectType::LineAttributes,
            ObjectType::FillAttributes,
            ObjectType::InputAttributes,
            ObjectType::ExtendedInputAttributes,
            ObjectType::ObjectPointer,
            ObjectType::Macro,
            ObjectType::AuxFunction,
            ObjectType::AuxInput,
            ObjectType::AuxFunction2,
            ObjectType::AuxInput2,
            ObjectType::AuxControlDesig,
            ObjectType::WindowMask,
            ObjectType::KeyGroup,
            ObjectType::GraphicData,
            ObjectType::ScaledGraphic,
            ObjectType::Animation,
            ObjectType::ColourMap,
            ObjectType::GraphicContext,
            ObjectType::ExternalObjectDefinition,
            ObjectType::ExternalReferenceName,
            ObjectType::ExternalObjectPointer,
            ObjectType::ColourPalette,
            ObjectType::WorkingSetSpecialControls,
            ObjectType::GraphicsContext,
            ObjectType::ObjectLabelRef,
            ObjectType::ScaledBitmap,
        ]
        .into_iter()
    }

    /// Repo-owned object-type display name (not standard prose).
    #[must_use]
    pub fn type_name(ty: ObjectType) -> &'static str {
        match ty {
            ObjectType::WorkingSet => "WorkingSet",
            ObjectType::DataMask => "DataMask",
            ObjectType::AlarmMask => "AlarmMask",
            ObjectType::Container => "Container",
            ObjectType::SoftKeyMask => "SoftKeyMask",
            ObjectType::Key => "Key",
            ObjectType::Button => "Button",
            ObjectType::InputBoolean => "InputBoolean",
            ObjectType::InputString => "InputString",
            ObjectType::InputNumber => "InputNumber",
            ObjectType::InputList => "InputList",
            ObjectType::OutputString => "OutputString",
            ObjectType::OutputNumber => "OutputNumber",
            ObjectType::OutputList => "OutputList",
            ObjectType::Line => "OutputLine",
            ObjectType::Rectangle => "OutputRectangle",
            ObjectType::Ellipse => "OutputEllipse",
            ObjectType::Polygon => "OutputPolygon",
            ObjectType::Meter => "Meter",
            ObjectType::LinearBarGraph => "LinearBarGraph",
            ObjectType::ArchedBarGraph => "ArchedBarGraph",
            ObjectType::PictureGraphic => "PictureGraphic",
            ObjectType::NumberVariable => "NumberVariable",
            ObjectType::StringVariable => "StringVariable",
            ObjectType::FontAttributes => "FontAttributes",
            ObjectType::LineAttributes => "LineAttributes",
            ObjectType::FillAttributes => "FillAttributes",
            ObjectType::InputAttributes => "InputAttributes",
            ObjectType::ExtendedInputAttributes => "ExtendedInputAttributes",
            ObjectType::ObjectPointer => "ObjectPointer",
            ObjectType::Macro => "Macro",
            ObjectType::AuxFunction => "AuxFunction",
            ObjectType::AuxInput => "AuxInput",
            ObjectType::AuxFunction2 => "AuxFunction2",
            ObjectType::AuxInput2 => "AuxInput2",
            ObjectType::AuxControlDesig => "AuxControlDesignator",
            ObjectType::WindowMask => "WindowMask",
            ObjectType::KeyGroup => "KeyGroup",
            ObjectType::GraphicData => "GraphicData",
            ObjectType::ScaledGraphic => "ScaledGraphic",
            ObjectType::Animation => "Animation",
            ObjectType::ColourMap => "ColourMap",
            ObjectType::GraphicContext => "GraphicContext",
            ObjectType::ExternalObjectDefinition => "ExternalObjectDefinition",
            ObjectType::ExternalReferenceName => "ExternalReferenceName",
            ObjectType::ExternalObjectPointer => "ExternalObjectPointer",
            ObjectType::ColourPalette => "ColourPalette",
            ObjectType::WorkingSetSpecialControls => "WorkingSetSpecialControls",
            ObjectType::GraphicsContext => "GraphicsContext",
            ObjectType::ObjectLabelRef => "ObjectLabelRef",
            ObjectType::ScaledBitmap => "ScaledBitmap",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::vt::{
        DataMaskBody, ObjectType, OutputStringBody, WorkingSetBody, create_data_mask,
        create_output_string, create_working_set,
    };

    fn trivial_pool_bytes() -> Vec<u8> {
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_output_string(3, &OutputStringBody::default()).unwrap());
        pool.serialize().unwrap()
    }

    #[test]
    fn iop_document_load_round_trips_valid_pool() {
        let bytes = trivial_pool_bytes();
        let doc = IopDocument::load(&bytes, LayoutConfig::default()).expect("parses");
        assert_eq!(doc.pool().size(), 3);
        // The scene's active mask is the first working-set child.
        assert_eq!(doc.scene().active_mask, ObjectID::new(2));
    }

    #[test]
    fn iop_document_rejects_empty_bytes() {
        assert!(IopDocument::load(&[], LayoutConfig::default()).is_err());
    }

    #[test]
    fn iop_document_rejects_pool_without_working_set() {
        // A pool with only a data mask fails validation.
        let pool = ObjectPool::default().with_object(create_data_mask(2, &DataMaskBody::default()));
        let bytes = pool.serialize().unwrap();
        assert!(IopDocument::load(&bytes, LayoutConfig::default()).is_err());
    }

    #[test]
    fn coverage_ledger_counts_pool_objects() {
        let bytes = trivial_pool_bytes();
        let doc = IopDocument::load(&bytes, LayoutConfig::default()).unwrap();
        let cov = doc.coverage();
        assert!(cov.total_objects >= 3);
        let csv = cov.to_csv();
        assert!(csv.starts_with("object_type,render_status"));
        // WorkingSet/OutputString/etc all appear.
        assert!(csv.contains("WorkingSet"));
        assert!(csv.contains("OutputString"));
    }

    #[test]
    fn coverage_status_classification_is_total() {
        for ty in coverage::all_object_types() {
            let _ = coverage::render_status_for(ty);
            let _ = coverage::type_name(ty);
        }
    }

    #[test]
    fn render_status_for_known_drawables() {
        assert_eq!(
            coverage::render_status_for(ObjectType::OutputString),
            coverage::RenderStatus::Drawable
        );
        assert_eq!(
            coverage::render_status_for(ObjectType::FontAttributes),
            coverage::RenderStatus::ReferenceResolved
        );
        assert_eq!(
            coverage::render_status_for(ObjectType::Key),
            coverage::RenderStatus::SoftKey
        );
        assert_eq!(
            coverage::render_status_for(ObjectType::Animation),
            coverage::RenderStatus::Drawable
        );
    }
}
