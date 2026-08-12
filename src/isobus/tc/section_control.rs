//! Section-control runtime (ISO 11783-10 TC-SC).
//!
//! A boom is split into sections (see
//! [`crate::isobus::tc::ddop_helpers::SectionInfo`]). Section control
//! decides, each control step, which sections are commanded ON. A section
//! applies only when **all** of the following hold:
//!
//! - the task is active (no application off-task);
//! - the section-control master switch is on;
//! - that individual section is requested on (by the operator or by
//!   automatic coverage/boundary logic the caller supplies).
//!
//! The runtime tracks the last commanded state per section and reports
//! whether anything changed, so a caller emits a new section-command
//! setpoint only on change. It is topology-agnostic (index-based) and
//! decoupled from the task runtime (the active flag is passed in).

use alloc::{vec, vec::Vec};

/// Per-step section-control runtime over a fixed number of sections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionControl {
    requested: Vec<bool>,
    commanded: Vec<bool>,
}

impl SectionControl {
    /// A runtime for `count` sections, all initially requested off.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            requested: vec![false; count],
            commanded: vec![false; count],
        }
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.requested.len()
    }

    /// Request a single section on/off. Returns `false` if the index is
    /// out of range (no change).
    pub fn request(&mut self, section: usize, on: bool) -> bool {
        match self.requested.get_mut(section) {
            Some(slot) => {
                *slot = on;
                true
            }
            None => false,
        }
    }

    /// Request all sections on/off.
    pub fn request_all(&mut self, on: bool) {
        for slot in &mut self.requested {
            *slot = on;
        }
    }

    /// Recompute commanded section states: a section is ON only while the
    /// task is active, the master switch is on, and the section is
    /// requested. Returns `true` if any commanded state changed.
    pub fn update(&mut self, task_active: bool, master_on: bool) -> bool {
        let mut changed = false;
        let gate = task_active && master_on;
        for (cmd, req) in self.commanded.iter_mut().zip(&self.requested) {
            let next = gate && *req;
            if *cmd != next {
                *cmd = next;
                changed = true;
            }
        }
        changed
    }

    /// The commanded ON/OFF state per section after the last `update`.
    #[must_use]
    pub fn commanded(&self) -> &[bool] {
        &self.commanded
    }

    /// Count of sections currently commanded ON.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.commanded.iter().filter(|&&on| on).count()
    }
}

/// One section's state in a condensed work state DDI (ISO 11783-10).
///
/// Each condensed work state DDI covers 16 sections in a 32-bit value — see
/// the data dictionary names, "Actual Condensed Work State (1-16)",
/// "(17-32)", and so on — which is **two bits per section**, not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SectionState {
    #[default]
    Off = 0,
    On = 1,
    Error = 2,
    /// Value 11. The data dictionary names it "not installed" for the work
    /// state DDIs (161-176) and "no change" for the setpoint DDIs (290-305) —
    /// two readings of the same encoding, and both mean "this slot is not a
    /// section I am saying anything about".
    NotInstalled = 3,
}

impl SectionState {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0x03 {
            0 => Self::Off,
            1 => Self::On,
            2 => Self::Error,
            _ => Self::NotInstalled,
        }
    }

    /// Whether the section is actually applying.
    #[must_use]
    pub const fn is_on(self) -> bool {
        matches!(self, Self::On)
    }
}

impl From<bool> for SectionState {
    fn from(on: bool) -> Self {
        if on { Self::On } else { Self::Off }
    }
}

/// Sections covered by one condensed work state DDI.
pub const SECTIONS_PER_CONDENSED_DDI: usize = 16;
/// Condensed work state DDIs needed to describe the 256-section maximum.
pub const CONDENSED_DDI_COUNT: usize = 16;

/// Pack up to 16 section states into one condensed work state value (DDIs
/// 161-176, 290-305 and the tramline equivalents).
///
/// This used to pack one bit per section into a `u16`. Sections
/// `[On, On, Off, …]` came out as `0b11`, which a conformant implement reads
/// as section 1 = not-installed — and "not installed" and "error" had no
/// representation at all, so a failed section was indistinguishable from a
/// closed one.
/// Unused slots are filled with 11, not 00: DDI 161 requires "if less than 16
/// child device element actual work states are available, then the unused bits
/// shall be set to value 11 (not installed)", and DDI 290 repeats it for
/// setpoints with 11 = "no change". Zero-filling made a 12-section sprayer
/// report four phantom sections as "disabled/off", so a conformant TC logged
/// coverage and as-applied area for them — and on the setpoint DDIs 00 is a
/// positive command to shut that section down rather than an abstention.
#[must_use]
pub fn pack_condensed_work_state(sections: &[SectionState]) -> u32 {
    let mut packed = u32::MAX;
    for (i, &state) in sections.iter().take(SECTIONS_PER_CONDENSED_DDI).enumerate() {
        packed &= !(0x03 << (i * 2));
        packed |= u32::from(state.as_u8()) << (i * 2);
    }
    packed
}

/// Unpack one condensed work state value into `count` section states (max 16).
#[must_use]
pub fn unpack_condensed_work_state(packed: u32, count: usize) -> Vec<SectionState> {
    let len = count.min(SECTIONS_PER_CONDENSED_DDI);
    (0..len)
        .map(|i| SectionState::from_bits(((packed >> (i * 2)) & 0x03) as u8))
        .collect()
}

/// Pack up to 256 section states into the 16 condensed work state values that
/// describe them.
#[must_use]
pub fn pack_condensed_work_state_256(sections: &[SectionState]) -> [u32; CONDENSED_DDI_COUNT] {
    // Same DDI 161 rule, and worse here: zero-filling reported up to 240
    // sections the implement does not have as "off".
    let mut packed = [u32::MAX; CONDENSED_DDI_COUNT];
    for (i, &state) in sections
        .iter()
        .take(SECTIONS_PER_CONDENSED_DDI * CONDENSED_DDI_COUNT)
        .enumerate()
    {
        let slot = i / SECTIONS_PER_CONDENSED_DDI;
        let within = i % SECTIONS_PER_CONDENSED_DDI;
        packed[slot] &= !(0x03 << (within * 2));
        packed[slot] |= u32::from(state.as_u8()) << (within * 2);
    }
    packed
}

/// Unpack the 16 condensed work state values into `count` section states.
#[must_use]
pub fn unpack_condensed_work_state_256(
    packed: &[u32; CONDENSED_DDI_COUNT],
    count: usize,
) -> Vec<SectionState> {
    let len = count.min(SECTIONS_PER_CONDENSED_DDI * CONDENSED_DDI_COUNT);
    (0..len)
        .map(|i| {
            let value = packed[i / SECTIONS_PER_CONDENSED_DDI];
            let within = i % SECTIONS_PER_CONDENSED_DDI;
            SectionState::from_bits(((value >> (within * 2)) & 0x03) as u8)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_apply_only_when_active_master_on_and_requested() {
        let mut sc = SectionControl::new(4);
        assert_eq!(sc.count(), 4);
        sc.request(0, true);
        sc.request(2, true);

        // Task active + master on → requested sections command on.
        assert!(sc.update(true, true));
        assert_eq!(sc.commanded(), &[true, false, true, false]);
        assert_eq!(sc.active_count(), 2);

        // No change on a second identical update.
        assert!(!sc.update(true, true));

        // Master off → all off (change).
        assert!(sc.update(true, false));
        assert_eq!(sc.active_count(), 0);

        // Master back on → sections resume.
        assert!(sc.update(true, true));
        assert_eq!(sc.commanded(), &[true, false, true, false]);

        // Task inactive → all off regardless of master/requested.
        assert!(sc.update(false, true));
        assert_eq!(sc.active_count(), 0);
    }

    #[test]
    fn request_all_and_out_of_range() {
        let mut sc = SectionControl::new(3);
        sc.request_all(true);
        assert!(sc.update(true, true));
        assert_eq!(sc.active_count(), 3);

        // Out-of-range request is rejected without panicking.
        assert!(!sc.request(99, true));
        assert!(sc.request(1, false));
        assert!(sc.update(true, true));
        assert_eq!(sc.commanded(), &[true, false, true]);
    }

    /// L4 — each condensed work state DDI covers 16 sections in a 32-bit
    /// value ("Actual Condensed Work State (1-16)", "(17-32)", …), so it is
    /// two bits per section. Packing one bit per section made
    /// `[On, On, Off, …]` come out as `0b11`, which a conformant implement
    /// reads as section 1 = not-installed, and left "error" and "not
    /// installed" with no representation at all.
    #[test]
    fn condensed_work_state_packs_two_bits_per_section() {
        use SectionState::{Error, NotInstalled, Off, On};

        // The exact case from the finding: two sections on, the rest off.
        // DDI 161: "If less than 16 child device element actual work states are
        // available, then the unused bits shall be set to value 11 (not
        // installed)." Slots 4-16 are unused here, so they are all ones —
        // zero-filling them reported thirteen phantom sections as
        // "disabled/off", which a conformant TC logs coverage for.
        assert_eq!(pack_condensed_work_state(&[On, On, Off]), 0xFFFF_FFC5);
        assert_ne!(
            pack_condensed_work_state(&[On, On, Off]),
            0b11,
            "one-bit-per-section packing reads back as section 1 not-installed"
        );

        let states = [On, Off, Error, NotInstalled];
        let packed = pack_condensed_work_state(&states);
        assert_eq!(packed, 0xFFFF_FFE1);
        assert_eq!(unpack_condensed_work_state(packed, 4), states);

        // Every state survives a round trip, in every slot.
        for slot in 0..SECTIONS_PER_CONDENSED_DDI {
            for state in [Off, On, Error, NotInstalled] {
                let mut sections = [Off; SECTIONS_PER_CONDENSED_DDI];
                sections[slot] = state;
                let packed = pack_condensed_work_state(&sections);
                assert_eq!(
                    unpack_condensed_work_state(packed, SECTIONS_PER_CONDENSED_DDI)[slot],
                    state
                );
            }
        }

        // A full 16-section DDI uses all 32 bits.
        let all_not_installed = [NotInstalled; SECTIONS_PER_CONDENSED_DDI];
        assert_eq!(pack_condensed_work_state(&all_not_installed), u32::MAX);
    }

    #[test]
    fn condensed_work_state_256_spreads_across_sixteen_ddis() {
        use SectionState::{Error, NotInstalled, Off, On};

        let mut sections = [Off; 256];
        sections[0] = On;
        sections[15] = NotInstalled;
        sections[16] = Error;
        sections[255] = On;

        // A 12-section sprayer: everything past slot 12 is unused, so DDI 161
        // requires all-ones. Zero-filling reported 244 sections the implement
        // does not have as "disabled/off", and on the setpoint DDIs that is a
        // positive command to shut each of them down.
        let twelve = pack_condensed_work_state_256(&[On; 12]);
        assert_eq!(
            twelve[0], 0xFF55_5555,
            "slots 13-16 of the first DDI are not installed"
        );
        for (i, value) in twelve.iter().enumerate().skip(1) {
            assert_eq!(
                *value,
                u32::MAX,
                "DDI {i} covers sections this implement does not have"
            );
        }

        let packed = pack_condensed_work_state_256(&sections);
        assert_eq!(packed[0] & 0x03, u32::from(On.as_u8()));
        assert_eq!(packed[0] >> 30, u32::from(NotInstalled.as_u8()));
        assert_eq!(packed[1] & 0x03, u32::from(Error.as_u8()));
        assert_eq!(packed[15] >> 30, u32::from(On.as_u8()));

        let round_tripped = unpack_condensed_work_state_256(&packed, 256);
        assert_eq!(round_tripped.as_slice(), sections.as_slice());
    }
}
