//! Sequence Control Data (SCD) version labels (ISO 11783-14).
//!
//! A sequence-control client stores its sequence definitions (the SCD) under a
//! version label. On connection the master compares the client's stored SCD
//! label with the one it holds to decide whether the stored definitions can be
//! reused or must be re-uploaded — the same reuse/re-upload decision the TC
//! makes from its structure label. This module is the repo-owned label model
//! and that decision; the SCD-transfer message layouts are a separate layer.

/// A 7-byte SCD version/configuration label.
pub type ScdLabel = [u8; 7];

/// The all-`0xFF` sentinel meaning "no stored SCD".
pub const SCD_LABEL_NONE: ScdLabel = [0xFF; 7];

/// What to do with a peer's stored SCD given the expected label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScdAction {
    /// The peer has no stored SCD (all-`0xFF`); upload it.
    Upload,
    /// The stored label matches; reuse the stored SCD.
    Reuse,
    /// The stored label differs; replace (re-upload) it.
    Reupload,
}

/// Decide what to do with a peer that reports `stored` when `expected` is held.
#[must_use]
pub fn scd_action(stored: ScdLabel, expected: ScdLabel) -> ScdAction {
    if stored == SCD_LABEL_NONE {
        ScdAction::Upload
    } else if stored == expected {
        ScdAction::Reuse
    } else {
        ScdAction::Reupload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scd_label_decisions() {
        let a: ScdLabel = [1, 2, 3, 4, 5, 6, 7];
        let b: ScdLabel = [9, 9, 9, 9, 9, 9, 9];
        // No stored SCD ⇒ upload.
        assert_eq!(scd_action(SCD_LABEL_NONE, a), ScdAction::Upload);
        // Matching label ⇒ reuse.
        assert_eq!(scd_action(a, a), ScdAction::Reuse);
        // Different label ⇒ re-upload.
        assert_eq!(scd_action(a, b), ScdAction::Reupload);
    }
}
