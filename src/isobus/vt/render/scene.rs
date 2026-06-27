//! Typed render scene graph.
//!
//! A [`Scene`] is the retained, validated, renderer-ready form of one
//! VT object pool. It is produced by the [`layout`] engine from a
//! validated [`ObjectPool`] and consumed by the [`gtui`] backend (or
//! any other backend).
//!
//! Design notes:
//!
//! - The scene is **flat by parent, nested by reference**: every placed
//!   node carries its absolute device-pixel rectangle and the id of its
//!   parent mask/container. This keeps the GTUI command list simple and
//!   avoids deep recursion when rendering.
//! - Children of a VT mask/container carry an `(x, y)` position in the
//!   pool model used by machbus (see `split_body_and_children`). Because
//!   the public pool API only exposes `children: Vec<ObjectID>`, the
//!   scene records the position supplied by the layout engine (either
//!   explicitly by the caller, or via the default auto-layout).
//! - Objects whose type is outside the renderer's coverage are recorded
//!   in [`Scene::unsupported`] instead of dropping them silently, so an
//!   operator-facing terminal can show "unsupported object" affordances
//!   and the coverage ledger stays accurate.
//!
//! [`ObjectPool`]: crate::isobus::vt::objects::ObjectPool
//! [`layout`]: crate::isobus::vt::render::layout
//! [`gtui`]: crate::isobus::vt::render::gtui

use crate::isobus::vt::render::style::{Palette, ResolvedStyle};
use crate::isobus::vt::{ObjectID, ObjectType};

/// Absolute device-pixel rectangle (inclusive origin, exclusive far
/// corner). The origin is the top-left of the node within its parent
/// mask coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    #[inline]
    #[must_use]
    pub const fn new(x: i32, y: i32, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    #[inline]
    #[must_use]
    pub const fn at(self, x: i32, y: i32) -> Self {
        Self { x, y, ..self }
    }

    #[inline]
    #[must_use]
    pub const fn size(self, w: u16, h: u16) -> Self {
        Self { w, h, ..self }
    }

    /// Right edge (exclusive).
    #[inline]
    #[must_use]
    pub const fn right(self) -> i32 {
        self.x + self.w as i32
    }

    /// Bottom edge (exclusive).
    #[inline]
    #[must_use]
    pub const fn bottom(self) -> i32 {
        self.y + self.h as i32
    }

    /// Test whether a point is inside this rectangle.
    #[must_use]
    pub const fn contains(self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// Translate by an offset (used to convert a child-relative rect
    /// into absolute coordinates).
    #[must_use]
    pub const fn translate(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            w: self.w,
            h: self.h,
        }
    }
}

/// A child placement as seen by the layout engine: which object, and
/// where it sits within its parent mask/container.
///
/// `x`/`y` are **signed** because ISO 11783-6 child locations are signed
/// 16-bit values (a child may legitimately sit at a negative offset
/// relative to its parent's top-left corner).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildPlacement {
    pub id: ObjectID,
    pub x: i16,
    pub y: i16,
}

impl ChildPlacement {
    #[inline]
    #[must_use]
    pub const fn new(id: ObjectID, x: i16, y: i16) -> Self {
        Self { id, x, y }
    }
}

/// One wide-character validation range in an ISO 10646 code plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputValidationRange {
    pub plane: u8,
    pub first: u16,
    pub last: u16,
}

/// Character-set rule resolved from an InputString's InputAttributes or
/// ExtendedInputAttributes object. `allow_listed` distinguishes the two
/// validation modes: `true` = the listed characters/ranges are the *only*
/// valid ones (whitelist, validation type 0); `false` = the listed
/// characters/ranges are rejected (blacklist, validation type 1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputValidation {
    pub allow_listed: bool,
    /// Classic Input Attributes apply to single-byte string entry only.
    /// Extended Input Attributes set this to `false` and validate ISO 10646
    /// code-plane ranges instead.
    pub byte_oriented: bool,
    /// Classic byte-oriented InputAttributes validation string.
    pub chars: Vec<u8>,
    /// Wide-character ExtendedInputAttributes ranges.
    pub ranges: Vec<InputValidationRange>,
}

impl InputValidation {
    /// Whether `c` may be entered into the field under this rule.
    /// Characters can match either the classic one-byte validation list
    /// or an ExtendedInputAttributes code-plane range.
    #[must_use]
    pub fn accepts(&self, c: char) -> bool {
        let codepoint = c as u32;
        if self.byte_oriented && codepoint > u32::from(u8::MAX) {
            return false;
        }
        let listed_byte = u8::try_from(codepoint)
            .ok()
            .is_some_and(|b| self.chars.contains(&b));
        let plane = (codepoint >> 16) as u8;
        let scalar_low = codepoint as u16;
        let listed_range = self
            .ranges
            .iter()
            .any(|r| r.plane == plane && r.first <= scalar_low && scalar_low <= r.last);
        let listed = listed_byte || listed_range;
        if self.allow_listed { listed } else { !listed }
    }
}

/// Decoded Picture Graphic data used as a Fill Attributes type-3 pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillPattern {
    pub object_id: ObjectID,
    pub width: u16,
    pub height: u16,
    pub format: u8,
    pub compressed: bool,
    pub data: Vec<u8>,
}

/// Concrete content carried by a placed scene node. This mirrors the
/// object families the renderer actually knows how to draw; anything
/// else becomes [`NodeKind::Unsupported`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// A Data/Alarm/Window mask or a Container: a grouping node that
    /// owns a background colour and a list of placed children.
    Group {
        background: u8,
        transparent_bg: bool,
        children: Vec<ChildPlacement>,
    },
    OutputString {
        text: String,
        transparent_bg: bool,
        justification: u8,
    },
    OutputNumber {
        /// Pre-formatted display string (number already scaled/offset).
        text: String,
        transparent_bg: bool,
        justification: u8,
    },
    OutputList {
        /// Index of the currently selected list item.
        selected: usize,
        item_count: usize,
        /// Compact text resolved from the selected item object. This is used
        /// directly only when the selected item was not materialised as its
        /// own scene node.
        selected_text: Option<String>,
        /// The selected item was materialised as its own scene node. Backends
        /// should not emit the compact fallback label for this OutputList.
        selected_item_materialized: bool,
    },
    OutputLine {
        direction: u8,
    },
    OutputRectangle {
        /// Bits 0..=3 suppress top/right/bottom/left lines.
        line_suppression: u8,
        fill_pattern: Option<FillPattern>,
    },
    OutputEllipse {
        /// `true` = filled ellipse (fill style resolved separately).
        filled: bool,
        fill_pattern: Option<FillPattern>,
        /// `true` = closed/complete ellipse, `false` = arc-like open.
        closed: bool,
        /// Standard ellipse type: 0 closed, 1 open arc, 2 closed segment,
        /// 3 closed section.
        ellipse_type: u8,
        /// Raw half-degree start angle from the object body.
        start_angle: u8,
        /// Raw half-degree end angle from the object body.
        end_angle: u8,
    },
    OutputPolygon {
        points: Vec<(i16, i16)>,
        fill_pattern: Option<FillPattern>,
    },
    Meter {
        value: u32,
        min_value: i32,
        max_value: i32,
        needle_colour: u8,
        border_colour: u8,
        arc_colour: u8,
        show_value: bool,
        number_of_ticks: u8,
        start_angle: u8,
        end_angle: u8,
    },
    LinearBarGraph {
        value: u32,
        target_value: u32,
        min_value: i32,
        max_value: i32,
        colour: u8,
        target_line_colour: u8,
        show_border: bool,
        show_target_line: bool,
        show_ticks: bool,
        number_of_ticks: u8,
        line_only: bool,
        horizontal: bool,
        direction_positive: bool,
    },
    ArchedBarGraph {
        value: u32,
        target_value: u32,
        min_value: i32,
        max_value: i32,
        colour: u8,
        target_line_colour: u8,
        show_border: bool,
        show_target_line: bool,
        line_only: bool,
        clockwise: bool,
        start_angle: u8,
        end_angle: u8,
        bar_width: u16,
    },
    PictureGraphic {
        raw_width: u16,
        raw_height: u16,
        format: u8,
        options: u8,
        transparency: u8,
        data: Vec<u8>,
    },
    ScaledGraphic {
        width: u16,
        height: u16,
        format: u8,
        options: u8,
        standard_png: bool,
        transparent: bool,
        transparency: u8,
        data: Vec<u8>,
    },
    ScaledBitmap {
        width: u16,
        height: u16,
        format: u8,
        options: u8,
        data: Vec<u8>,
    },
    GraphicContext {
        canvas_width: u16,
        canvas_height: u16,
        background: u8,
        transparency_colour: u8,
        transparent: bool,
    },
    /// machbus compatibility extension (object type 50): a geometry-less
    /// graphics-context state object, rendered best-effort as a fill+border
    /// swatch of a fixed default extent using its 24-bit RGB fill/line state.
    GraphicsContext {
        fill_rgb: u32,
        line_rgb: u32,
        line_width: u16,
        line_style: u8,
    },
    InputBoolean {
        enabled: bool,
        value: bool,
    },
    InputString {
        enabled: bool,
        text: String,
        transparent_bg: bool,
        auto_wrap: bool,
        justification: u8,
        /// Maximum encoded string length accepted by hosted edit
        /// transactions. `0` means no runtime length limit.
        max_length: u8,
        /// Character-set rule from the field's InputAttributes object, if
        /// any. `None` means no constraint (accept all characters).
        validation: Option<InputValidation>,
    },
    InputNumber {
        enabled: bool,
        real_time_editing: bool,
        text: String,
        transparent_bg: bool,
        justification: u8,
        min_value: i32,
        max_value: i32,
    },
    InputList {
        enabled: bool,
        real_time_editing: bool,
        /// Index of the currently selected list item.
        selected: usize,
        item_count: usize,
        /// List indexes the operator can choose directly. NULL item slots are
        /// counted for index stability but are not selectable.
        selectable_indices: Vec<usize>,
        /// Compact text resolved from the currently selected item object.
        selected_text: Option<String>,
        /// The selected item was materialised as its own display-only scene
        /// node. Backends should not emit the compact fallback label for this
        /// InputList.
        selected_item_materialized: bool,
    },
    Button {
        label: String,
        enabled: bool,
        transparent_bg: bool,
        draw_border: bool,
        /// Standard Key Number / Key Code carried by the Button object.
        key_number: u8,
    },
    /// A Key object rendered as a display-only designator outside the active
    /// Soft Key Mask, for example as the selected item of an Output List.
    ///
    /// Soft-key activation still comes from [`Scene::soft_keys`] and Key Group
    /// nodes, not from this node.
    KeyDesignator {
        label: String,
        /// Standard Key Number / Key Code carried by the Key object.
        key_number: u8,
    },
    /// A Key Group placed in a user-layout/window area. The group can occupy
    /// one to four normal soft-key cells.
    KeyGroup {
        available: bool,
        transparent: bool,
        /// One entry per physical Key Group child slot. [`ObjectID::NULL`]
        /// marks an unresolved/blank slot, for example an External Object
        /// Pointer whose referenced Working Set is not currently registered.
        key_ids: Vec<ObjectID>,
        /// Key numbers aligned with `key_ids`; blank slots carry zero.
        key_numbers: Vec<u8>,
        /// Labels aligned with `key_ids`; blank slots carry an empty label.
        labels: Vec<String>,
    },
    /// An object type the renderer does not cover. It is still placed
    /// (so hit-testing/focus work) but draws as a hatched placeholder.
    Unsupported {
        type_byte: u8,
        reason: &'static str,
    },
}

/// One node in the retained scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneNode {
    pub id: ObjectID,
    pub object_type: ObjectType,
    /// Parent mask/container id, or `NULL` for the active mask itself.
    pub parent: ObjectID,
    pub rect: Rect,
    /// Optional clip rectangle that constrains this node's drawing. This is
    /// used for standard objects that display another object through a bounded
    /// viewport, such as OutputList selected-item presentation.
    pub clip: Option<Rect>,
    pub style: ResolvedStyle,
    pub visible: bool,
    pub enabled: bool,
    pub kind: NodeKind,
}

impl SceneNode {
    #[inline]
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        matches!(
            self.kind,
            NodeKind::InputBoolean { .. }
                | NodeKind::InputString { .. }
                | NodeKind::InputNumber { .. }
                | NodeKind::InputList { .. }
                | NodeKind::Button { .. }
        )
    }

    #[inline]
    #[must_use]
    pub const fn is_pointer_interactive(&self) -> bool {
        self.is_interactive()
            || matches!(
                self.kind,
                NodeKind::KeyGroup {
                    available: true,
                    ..
                }
            )
    }
}

/// Kind of soft-key cell resolved for the active mask's soft-key area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftKeyKind {
    /// Application-provided Key object from the active Soft Key Mask.
    Application,
    /// Host-reserved navigation key that pages to the previous application-key
    /// set.
    NavigationPrevious,
    /// Host-reserved navigation key that pages to the next application-key set.
    NavigationNext,
}

/// A soft key (or navigation key) as resolved for the active mask's soft-key
/// area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftKeyNode {
    pub id: ObjectID,
    pub kind: SoftKeyKind,
    /// Zero-based physical soft-key cell occupied by this visible key.
    ///
    /// Hosts with real keypad hardware can use this instead of synthesising a
    /// pointer coordinate in the soft-key area.
    pub cell_index: u8,
    pub rect: Rect,
    pub style: ResolvedStyle,
    pub visible: bool,
    pub enabled: bool,
    /// Standard Key Number / Key Code carried by the resolved Key object.
    pub key_number: u8,
    pub label: String,
}

/// An object the renderer could not place or does not cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedRecord {
    pub id: ObjectID,
    pub object_type: ObjectType,
    pub reason: &'static str,
}

/// One language/country pair advertised by Working Set Special Controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneLanguage {
    pub language: [u8; 2],
    pub country: [u8; 2],
}

/// The retained render scene for one active mask of one object pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    pub active_mask: ObjectID,
    pub mask_rect: Rect,
    pub background: u8,
    /// Palette resolved for this scene build after the renderer's configured
    /// base palette, any active Colour Palette object, and any active Colour
    /// Map object have been applied.
    ///
    /// Manually constructed scenes leave this as `None`, in which case
    /// renderers use their own configured palette.
    pub effective_palette: Option<Palette>,
    pub nodes: Vec<SceneNode>,
    pub soft_keys: Vec<SoftKeyNode>,
    pub unsupported: Vec<UnsupportedRecord>,
    pub supported_languages: Vec<SceneLanguage>,
}

impl Scene {
    const COUNTRY_NOT_APPLICABLE: [u8; 2] = *b"  ";

    /// Build an empty scene rooted at `mask` with the given canvas size.
    #[must_use]
    pub fn new(active_mask: ObjectID, canvas: (u16, u16)) -> Self {
        Self {
            active_mask,
            mask_rect: Rect::new(0, 0, canvas.0, canvas.1),
            background: 0,
            effective_palette: None,
            nodes: Vec::new(),
            soft_keys: Vec::new(),
            unsupported: Vec::new(),
            supported_languages: Vec::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.soft_keys.is_empty()
    }

    /// Find a node by object id.
    #[must_use]
    pub fn find(&self, id: ObjectID) -> Option<&SceneNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Iterate over currently visible nodes.
    pub fn visible_nodes(&self) -> impl Iterator<Item = &SceneNode> {
        self.nodes.iter().filter(|n| n.visible)
    }

    /// Hit-test a device-pixel point, returning the topmost visible,
    /// enabled, pointer-interactive node under the cursor.
    #[must_use]
    pub fn hit_test(&self, px: i32, py: i32) -> Option<&SceneNode> {
        // The scene node list is paint-order: later nodes draw on top,
        // so the topmost hit is the last matching node. Use `rfind` so
        // we walk once from the back instead of scanning the whole list.
        self.nodes.iter().rev().find(|n| {
            n.visible
                && n.enabled
                && n.is_pointer_interactive()
                && n.rect.contains(px, py)
                && n.clip.is_none_or(|clip| clip.contains(px, py))
        })
    }

    /// Hit-test a point against visible-but-disabled interactive nodes.
    ///
    /// This is separate from [`Scene::hit_test`] because disabled input fields
    /// and buttons must block focus/value activation without being mistaken
    /// for blank mask background pointing-event targets.
    #[must_use]
    pub fn disabled_interactive_hit_test(&self, px: i32, py: i32) -> Option<&SceneNode> {
        self.nodes.iter().rev().find(|n| {
            n.visible
                && !n.enabled
                && n.is_interactive()
                && n.rect.contains(px, py)
                && n.clip.is_none_or(|clip| clip.contains(px, py))
        })
    }

    /// Hit-test a device-pixel point against visible, enabled soft-key cells.
    #[must_use]
    pub fn soft_key_hit_test(&self, px: i32, py: i32) -> Option<&SoftKeyNode> {
        self.soft_keys
            .iter()
            .rev()
            .find(|key| key.visible && key.enabled && key.rect.contains(px, py))
    }

    /// Find a visible, enabled soft-key cell by zero-based physical key
    /// position.
    #[must_use]
    pub fn soft_key_cell(&self, cell_index: u8) -> Option<&SoftKeyNode> {
        self.soft_keys
            .iter()
            .rev()
            .find(|key| key.visible && key.enabled && key.cell_index == cell_index)
    }

    /// Returns true when the scene's Working Set Special Controls advertise
    /// the requested two-byte language/country pair.
    ///
    /// A country value of two spaces is the standard "not applicable" sentinel:
    /// it matches any requested country for the same language, and a request
    /// with that sentinel matches any advertised country for the same language.
    /// Language and country letters are matched ASCII-case-insensitively because
    /// ISO admits any upper/lower-case combination for these two-letter codes.
    /// Empty language lists mean the working set did not publish a preference
    /// list, not that every locale is supported.
    #[must_use]
    pub fn supports_language(&self, language: [u8; 2], country: [u8; 2]) -> bool {
        self.supported_languages
            .iter()
            .any(|entry| Self::language_pair_matches(*entry, language, country))
    }

    /// Select the first host-preferred language/country pair advertised by the
    /// working set.
    ///
    /// Preferences are evaluated in caller order. For each preference, exact
    /// country matches win over language-only matches through the two-space
    /// country sentinel. The returned pair is the advertised working-set pair,
    /// so hosts can distinguish exact-country support from language-only
    /// fallback.
    #[must_use]
    pub fn select_language(&self, preferences: &[SceneLanguage]) -> Option<SceneLanguage> {
        for preference in preferences {
            if let Some(exact) = self.supported_languages.iter().find(|entry| {
                Self::ascii_pair_eq(entry.language, preference.language)
                    && Self::ascii_pair_eq(entry.country, preference.country)
            }) {
                return Some(*exact);
            }
            if let Some(language_only) = self.supported_languages.iter().find(|entry| {
                Self::ascii_pair_eq(entry.language, preference.language)
                    && (entry.country == Self::COUNTRY_NOT_APPLICABLE
                        || preference.country == Self::COUNTRY_NOT_APPLICABLE)
            }) {
                return Some(*language_only);
            }
        }
        None
    }

    #[inline]
    fn language_pair_matches(entry: SceneLanguage, language: [u8; 2], country: [u8; 2]) -> bool {
        Self::ascii_pair_eq(entry.language, language)
            && (Self::ascii_pair_eq(entry.country, country)
                || entry.country == Self::COUNTRY_NOT_APPLICABLE
                || country == Self::COUNTRY_NOT_APPLICABLE)
    }

    #[inline]
    fn ascii_pair_eq(a: [u8; 2], b: [u8; 2]) -> bool {
        a[0].eq_ignore_ascii_case(&b[0]) && a[1].eq_ignore_ascii_case(&b[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_and_translate() {
        let r = Rect::new(10, 20, 30, 40);
        assert!(r.contains(10, 20));
        assert!(r.contains(39, 59));
        assert!(!r.contains(40, 20));
        assert!(!r.contains(10, 60));
        let r2 = r.translate(5, 7);
        assert_eq!(r2.x, 15);
        assert_eq!(r2.y, 27);
        assert_eq!(r2.right(), 45);
    }

    #[test]
    fn scene_hit_test_returns_topmost_interactive_node() {
        let style = ResolvedStyle::default();
        let mut scene = Scene::new(ObjectID::new(1), (200, 200));
        // Non-interactive output string at (0,0,100,100).
        scene.nodes.push(SceneNode {
            id: ObjectID::new(2),
            object_type: ObjectType::OutputString,
            parent: ObjectID::new(1),
            rect: Rect::new(0, 0, 100, 100),
            clip: None,
            style,
            visible: true,
            enabled: true,
            kind: NodeKind::OutputString {
                text: "x".into(),
                transparent_bg: false,
                justification: 0,
            },
        });
        // Interactive button at (0,0,100,100) — same rect, drawn later.
        scene.nodes.push(SceneNode {
            id: ObjectID::new(3),
            object_type: ObjectType::Button,
            parent: ObjectID::new(1),
            rect: Rect::new(0, 0, 100, 100),
            clip: None,
            style,
            visible: true,
            enabled: true,
            kind: NodeKind::Button {
                label: "OK".into(),
                enabled: true,
                transparent_bg: false,
                draw_border: true,
                key_number: 1,
            },
        });
        // Disabled button — must be skipped.
        scene.nodes.push(SceneNode {
            id: ObjectID::new(4),
            object_type: ObjectType::Button,
            parent: ObjectID::new(1),
            rect: Rect::new(0, 0, 100, 100),
            clip: None,
            style,
            visible: true,
            enabled: false,
            kind: NodeKind::Button {
                label: "NO".into(),
                enabled: false,
                transparent_bg: false,
                draw_border: true,
                key_number: 2,
            },
        });

        let hit = scene
            .hit_test(50, 50)
            .expect("interactive node under cursor");
        assert_eq!(hit.id, ObjectID::new(3));
        // Outside any node.
        assert!(scene.hit_test(150, 150).is_none());
    }

    #[test]
    fn is_interactive_flag_matches_kind() {
        let mk = |kind: NodeKind| SceneNode {
            id: ObjectID::new(1),
            object_type: ObjectType::Button,
            parent: ObjectID::NULL,
            rect: Rect::default(),
            clip: None,
            style: ResolvedStyle::default(),
            visible: true,
            enabled: true,
            kind,
        };
        assert!(
            mk(NodeKind::Button {
                label: "x".into(),
                enabled: true,
                transparent_bg: false,
                draw_border: true,
                key_number: 1,
            })
            .is_interactive()
        );
        assert!(
            mk(NodeKind::InputBoolean {
                enabled: true,
                value: false,
            })
            .is_interactive()
        );
        assert!(
            !mk(NodeKind::OutputString {
                text: "x".into(),
                transparent_bg: false,
                justification: 0,
            })
            .is_interactive()
        );
        let key_group = mk(NodeKind::KeyGroup {
            available: true,
            transparent: false,
            key_ids: vec![ObjectID::new(2)],
            key_numbers: vec![2],
            labels: vec!["2".into()],
        });
        assert!(!key_group.is_interactive());
        assert!(key_group.is_pointer_interactive());
    }
}
