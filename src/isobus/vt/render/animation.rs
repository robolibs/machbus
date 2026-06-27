//! Animation runtime for the VT renderer.
//!
//! An Animation object (ISO 11783-6 type 44) holds an ordered positional
//! child list, a Value index, a refresh interval, and option bits for sequence
//! and disabled presentation.
//! This module resolves which frame is active at a given elapsed time so
//! the renderer can draw the right child — the frame-timing half of the
//! "Animation object is enabled for animation" behaviour.
//!
//! It is pure and deterministic: it takes an [`AnimationBody`] and an
//! elapsed-millisecond count and returns the active frame. Driving the
//! clock and deciding when an animation is enabled are the caller's
//! responsibility.

use crate::isobus::vt::{AnimationBody, ChildRef, ObjectID};

/// The active frame of an animation at a point in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame {
    /// Index into the animation's frame list.
    pub index: usize,
    /// The child object id to display for this frame.
    pub object: ObjectID,
    /// X location relative to the Animation object's top-left corner.
    pub x: i16,
    /// Y location relative to the Animation object's top-left corner.
    pub y: i16,
}

/// `true` if the animation loops (ISO 11783-6 options bit 0).
#[must_use]
pub fn is_looping(body: &AnimationBody) -> bool {
    body.options & 0x01 != 0
}

/// Disabled Behaviour from ISO 11783-6 Animation options bits 1-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledBehaviour {
    Pause,
    ResetToFirst,
    DefaultObject,
    Blank,
}

impl DisabledBehaviour {
    #[must_use]
    pub const fn from_options(options: u8) -> Self {
        match (options >> 1) & 0x03 {
            0 => Self::Pause,
            1 => Self::ResetToFirst,
            2 => Self::DefaultObject,
            _ => Self::Blank,
        }
    }
}

/// Resolve the active frame of an animation at `elapsed_ms`.
///
/// Returns `None` for an empty child list, NULL/no-item placeholders, invalid
/// selected indices, invalid enabled sequence bounds, or disabled Blank mode.
/// A zero refresh interval freezes on the current Value. Looping animations
/// wrap within the First/Last child range; non-looping animations clamp on the
/// Last child index.
#[must_use]
pub fn animation_frame(
    body: &AnimationBody,
    children: &[ChildRef],
    effective_enabled: bool,
    elapsed_ms: u32,
) -> Option<AnimationFrame> {
    let count = children.len();
    if count == 0 {
        return None;
    }
    let selected = if effective_enabled {
        enabled_index(body, count, elapsed_ms)?
    } else {
        disabled_index(body)?
    };
    let child = *children.get(selected)?;
    if child.id == ObjectID::NULL {
        return None;
    }
    Some(AnimationFrame {
        index: selected,
        object: child.id,
        x: child.x,
        y: child.y,
    })
}

fn enabled_index(body: &AnimationBody, count: usize, elapsed_ms: u32) -> Option<usize> {
    let first = usize::from(body.first_child_index);
    let last = usize::from(body.last_child_index);
    if body.value == u8::MAX || first > last || last >= count {
        return None;
    }
    let mut value = usize::from(body.value);
    if value >= count {
        return None;
    }
    let ticks = if body.refresh_interval_ms == 0 {
        0
    } else {
        (elapsed_ms / u32::from(body.refresh_interval_ms)) as usize
    };
    if ticks == 0 {
        return Some(value);
    }

    value = animation_index_after_tick(value, first, last, is_looping(body));
    let remaining_ticks = ticks.saturating_sub(1);
    if remaining_ticks == 0 {
        return Some(value);
    }

    let window = last - first + 1;
    let index = if is_looping(body) {
        let offset = value.saturating_sub(first).saturating_add(remaining_ticks);
        first + (offset % window)
    } else {
        value.saturating_add(remaining_ticks).min(last)
    };
    Some(index)
}

fn animation_index_after_tick(value: usize, first: usize, last: usize, looping: bool) -> usize {
    let mut value = value.clamp(first, last);
    if value < last {
        value += 1;
    } else if looping {
        value = first;
    }
    value
}

fn disabled_index(body: &AnimationBody) -> Option<usize> {
    match DisabledBehaviour::from_options(body.options) {
        DisabledBehaviour::Pause => {
            if body.value == u8::MAX {
                None
            } else {
                Some(usize::from(body.value))
            }
        }
        DisabledBehaviour::ResetToFirst => Some(usize::from(body.first_child_index)),
        DisabledBehaviour::DefaultObject => Some(usize::from(body.default_child_index)),
        DisabledBehaviour::Blank => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(loop_on: bool) -> (AnimationBody, Vec<ChildRef>) {
        let body = AnimationBody {
            width: 32,
            height: 32,
            refresh_interval_ms: 100,
            value: 0,
            enabled: 1,
            first_child_index: 0,
            default_child_index: 1,
            last_child_index: 2,
            options: u8::from(loop_on),
        };
        let children = vec![
            ChildRef::at_origin(ObjectID::new(10)),
            ChildRef::new(ObjectID::new(11), 2, 3),
            ChildRef::at_origin(ObjectID::new(12)),
        ];
        (body, children)
    }

    #[test]
    fn advances_one_frame_per_interval() {
        let (a, children) = anim(false);
        assert_eq!(animation_frame(&a, &children, true, 0).unwrap().index, 0);
        assert_eq!(animation_frame(&a, &children, true, 99).unwrap().index, 0);
        assert_eq!(animation_frame(&a, &children, true, 100).unwrap().index, 1);
        assert_eq!(animation_frame(&a, &children, true, 250).unwrap().index, 2);
        assert_eq!(
            animation_frame(&a, &children, true, 250).unwrap().object,
            ObjectID::new(12)
        );
        assert_eq!(animation_frame(&a, &children, true, 100).unwrap().x, 2);
    }

    #[test]
    fn non_looping_clamps_on_last_frame() {
        let (a, children) = anim(false);
        // Well past the end stays on the final frame.
        assert_eq!(
            animation_frame(&a, &children, true, 10_000).unwrap().index,
            2
        );
    }

    #[test]
    fn looping_wraps_around() {
        let (a, children) = anim(true);
        assert!(is_looping(&a));
        assert_eq!(animation_frame(&a, &children, true, 300).unwrap().index, 0);
        assert_eq!(animation_frame(&a, &children, true, 400).unwrap().index, 1);
        assert_eq!(animation_frame(&a, &children, true, 500).unwrap().index, 2);
        assert_eq!(animation_frame(&a, &children, true, 600).unwrap().index, 0);
    }

    #[test]
    fn zero_interval_freezes_on_first_frame() {
        let (mut a, children) = anim(true);
        a.refresh_interval_ms = 0;
        assert_eq!(
            animation_frame(&a, &children, true, 5_000).unwrap().index,
            0
        );
    }

    #[test]
    fn out_of_sequence_value_is_drawn_until_first_refresh_tick() {
        let (mut a, children) = anim(false);
        a.value = 0;
        a.first_child_index = 1;
        a.last_child_index = 2;

        assert_eq!(animation_frame(&a, &children, true, 0).unwrap().index, 0);
        assert_eq!(animation_frame(&a, &children, true, 99).unwrap().index, 0);
        assert_eq!(animation_frame(&a, &children, true, 100).unwrap().index, 2);
        assert_eq!(animation_frame(&a, &children, true, 200).unwrap().index, 2);
    }

    #[test]
    fn loop_mode_wraps_after_range_checking_value_above_last() {
        let (mut a, children) = anim(true);
        a.value = 2;
        a.first_child_index = 0;
        a.last_child_index = 1;

        assert_eq!(animation_frame(&a, &children, true, 0).unwrap().index, 2);
        assert_eq!(animation_frame(&a, &children, true, 100).unwrap().index, 0);
        assert_eq!(animation_frame(&a, &children, true, 200).unwrap().index, 1);
    }

    #[test]
    fn empty_frame_list_yields_none() {
        let (a, _) = anim(false);
        assert!(animation_frame(&a, &[], true, 100).is_none());
    }

    #[test]
    fn disabled_behaviour_selects_pause_first_default_or_blank() {
        let (mut a, children) = anim(false);
        a.enabled = 0;
        a.value = 2;
        assert_eq!(
            animation_frame(&a, &children, false, 0).unwrap().object,
            ObjectID::new(12)
        );
        a.options = 0b010;
        assert_eq!(animation_frame(&a, &children, false, 0).unwrap().index, 0);
        a.options = 0b100;
        assert_eq!(animation_frame(&a, &children, false, 0).unwrap().index, 1);
        a.options = 0b110;
        assert!(animation_frame(&a, &children, false, 0).is_none());
    }
}
