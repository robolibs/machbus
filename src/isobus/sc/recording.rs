//! Sequence recording / authoring runtime (ISO 11783-14).
//!
//! The SC master/client status machinery models the `Recording` and
//! `RecordingCompletion` sequence states but the supported-status guard
//! deliberately rejects them on the wire (machbus does not yet claim a
//! recording-capable SC *peer*). This module adds the missing **authoring**
//! side: a local recorder that captures a sequence of steps and walks the
//! Ready → Recording → RecordingCompletion → Ready lifecycle, so a tool
//! can build a sequence for later playback.
//!
//! It owns no I/O and is independent of the wire status guards; it is the
//! deterministic core a recording UI/tool drives.

use alloc::{format, string::String, vec::Vec};

use crate::isobus::sc::types::{SC_MAX_SEQUENCE_STEP_ID, SCSequenceState, SequenceStep};
use crate::net::error::{Error, Result};

/// Local sequence recorder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SequenceRecorder {
    state: SCSequenceState,
    steps: Vec<SequenceStep>,
}

impl SequenceRecorder {
    /// A new recorder in the Ready state with no captured steps.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: SCSequenceState::Ready,
            steps: Vec::new(),
        }
    }

    /// Current recording lifecycle state.
    #[must_use]
    pub const fn state(&self) -> SCSequenceState {
        self.state
    }

    /// Steps captured so far (or the finalized sequence after `complete`).
    #[must_use]
    pub fn steps(&self) -> &[SequenceStep] {
        &self.steps
    }

    /// Begin recording (Ready → Recording), discarding any prior capture.
    pub fn start(&mut self) -> Result<()> {
        if self.state != SCSequenceState::Ready {
            return Err(Self::illegal("start recording", self.state));
        }
        self.steps.clear();
        self.state = SCSequenceState::Recording;
        Ok(())
    }

    /// Capture one step while Recording. Assigns the next sequential step
    /// id and returns it. Errors if not Recording or if the sequence would
    /// exceed the `0..=SC_MAX_SEQUENCE_STEP_ID` step-id range.
    pub fn record(&mut self, description: impl Into<String>, duration_ms: u32) -> Result<u16> {
        if self.state != SCSequenceState::Recording {
            return Err(Self::illegal("record step", self.state));
        }
        let step_id = self.steps.len() as u16;
        if step_id > SC_MAX_SEQUENCE_STEP_ID {
            return Err(Error::invalid_data(
                "sequence exceeds the maximum recordable step count",
            ));
        }
        self.steps.push(SequenceStep {
            step_id,
            description: description.into(),
            duration_ms,
            completed: false,
        });
        Ok(step_id)
    }

    /// Finish recording (Recording → RecordingCompletion). The captured
    /// steps are finalized and returned; the recorder stays in
    /// RecordingCompletion until [`reset`](Self::reset).
    pub fn complete(&mut self) -> Result<Vec<SequenceStep>> {
        if self.state != SCSequenceState::Recording {
            return Err(Self::illegal("complete recording", self.state));
        }
        self.state = SCSequenceState::RecordingCompletion;
        Ok(self.steps.clone())
    }

    /// Return to Ready from RecordingCompletion (ready to record again).
    pub fn reset(&mut self) -> Result<()> {
        if self.state != SCSequenceState::RecordingCompletion {
            return Err(Self::illegal("reset", self.state));
        }
        self.state = SCSequenceState::Ready;
        Ok(())
    }

    /// Abort an in-progress or just-completed recording, discarding the
    /// captured steps and returning to Ready.
    pub fn abort(&mut self) -> Result<()> {
        match self.state {
            SCSequenceState::Recording | SCSequenceState::RecordingCompletion => {
                self.steps.clear();
                self.state = SCSequenceState::Ready;
                Ok(())
            }
            other => Err(Self::illegal("abort", other)),
        }
    }

    /// Total playback duration of the captured sequence (sum of step
    /// durations, saturating).
    #[must_use]
    pub fn total_duration_ms(&self) -> u32 {
        self.steps
            .iter()
            .fold(0u32, |acc, s| acc.saturating_add(s.duration_ms))
    }

    /// The step that is active `elapsed_ms` into playback, or `None` once
    /// playback has run past the end of the sequence. Each step occupies its
    /// `duration_ms` window in recorded order.
    #[must_use]
    pub fn step_at_offset(&self, elapsed_ms: u32) -> Option<&SequenceStep> {
        let mut cursor = 0u32;
        for step in &self.steps {
            cursor = cursor.saturating_add(step.duration_ms);
            if elapsed_ms < cursor {
                return Some(step);
            }
        }
        None
    }

    fn illegal(op: &str, state: SCSequenceState) -> Error {
        Error::invalid_state(format!(
            "illegal recorder transition: cannot {op} from {state:?}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_steps_and_walks_the_lifecycle() {
        let mut r = SequenceRecorder::new();
        assert_eq!(r.state(), SCSequenceState::Ready);

        r.start().unwrap();
        assert_eq!(r.state(), SCSequenceState::Recording);
        assert_eq!(r.record("raise hitch", 500).unwrap(), 0);
        assert_eq!(r.record("engage pto", 250).unwrap(), 1);
        assert_eq!(r.steps().len(), 2);

        let seq = r.complete().unwrap();
        assert_eq!(r.state(), SCSequenceState::RecordingCompletion);
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[0].step_id, 0);
        assert_eq!(seq[1].description, "engage pto");

        r.reset().unwrap();
        assert_eq!(r.state(), SCSequenceState::Ready);
    }

    #[test]
    fn playback_offsets_map_to_steps_and_total_duration() {
        let mut r = SequenceRecorder::new();
        r.start().unwrap();
        r.record("raise hitch", 500).unwrap(); // [0, 500)
        r.record("engage pto", 250).unwrap(); // [500, 750)
        r.complete().unwrap();

        assert_eq!(r.total_duration_ms(), 750);
        assert_eq!(r.step_at_offset(0).unwrap().step_id, 0);
        assert_eq!(r.step_at_offset(499).unwrap().step_id, 0);
        assert_eq!(r.step_at_offset(500).unwrap().step_id, 1);
        assert_eq!(r.step_at_offset(749).unwrap().step_id, 1);
        // Past the end of the sequence: no active step.
        assert!(r.step_at_offset(750).is_none());

        // An empty recorder has zero duration and no active step.
        let empty = SequenceRecorder::new();
        assert_eq!(empty.total_duration_ms(), 0);
        assert!(empty.step_at_offset(0).is_none());
    }

    #[test]
    fn illegal_transitions_are_rejected() {
        let mut r = SequenceRecorder::new();
        // Cannot record or complete before starting.
        assert!(r.record("x", 1).is_err());
        assert!(r.complete().is_err());
        assert!(r.reset().is_err());
        assert!(r.abort().is_err());

        r.start().unwrap();
        // Cannot start again while recording.
        assert!(r.start().is_err());
    }

    #[test]
    fn abort_discards_captured_steps() {
        let mut r = SequenceRecorder::new();
        r.start().unwrap();
        r.record("a", 1).unwrap();
        r.abort().unwrap();
        assert_eq!(r.state(), SCSequenceState::Ready);
        assert!(r.steps().is_empty());
    }

    #[test]
    fn step_ids_are_capped_at_the_protocol_maximum() {
        let mut r = SequenceRecorder::new();
        r.start().unwrap();
        // Fill up to and including the maximum step id.
        for _ in 0..=SC_MAX_SEQUENCE_STEP_ID {
            r.record("s", 1).unwrap();
        }
        // The next step would exceed SC_MAX_SEQUENCE_STEP_ID.
        assert!(r.record("overflow", 1).is_err());
    }
}
