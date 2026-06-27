//! Task lifecycle state machine (ISO 11783-10 Task Controller).
//!
//! The TC client advertises a task status byte
//! ([`TCClientTaskStatus`]: Idle / Active / Paused / Completed), but the
//! byte alone does not enforce which transitions are legal. This module
//! adds the lifecycle engine: a small state machine that accepts
//! `start` / `pause` / `resume` / `complete` and rejects illegal
//! transitions, so a task runtime cannot, for example, resume a task that
//! was never started or restart a completed one.
//!
//! It owns no I/O; it is the deterministic core a task runtime drives, and
//! its [`status_byte`](TaskLifecycle::status_byte) maps directly to the
//! wire status the client reports.

use alloc::{format, vec::Vec};

use crate::isobus::tc::client::TCClientTaskStatus;
use crate::isobus::tc::objects::{DDI, ElementNumber};
use crate::net::error::{Error, Result};

/// Lifecycle state of a single task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskLifecycle {
    state: TCClientTaskStatus,
}

impl TaskLifecycle {
    /// A new, not-yet-started task (Idle).
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: TCClientTaskStatus::Idle,
        }
    }

    /// Current lifecycle state.
    #[must_use]
    pub const fn state(&self) -> TCClientTaskStatus {
        self.state
    }

    /// The wire status byte reported by the TC client for this state.
    #[must_use]
    pub const fn status_byte(&self) -> u8 {
        self.state.as_u8()
    }

    /// `true` once the task has reached the terminal Completed state.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        matches!(self.state, TCClientTaskStatus::Completed)
    }

    /// Start an Idle task → Active.
    pub fn start(&mut self) -> Result<()> {
        self.transition(
            TCClientTaskStatus::Idle,
            TCClientTaskStatus::Active,
            "start",
        )
    }

    /// Pause an Active task → Paused.
    pub fn pause(&mut self) -> Result<()> {
        self.transition(
            TCClientTaskStatus::Active,
            TCClientTaskStatus::Paused,
            "pause",
        )
    }

    /// Resume a Paused task → Active.
    pub fn resume(&mut self) -> Result<()> {
        self.transition(
            TCClientTaskStatus::Paused,
            TCClientTaskStatus::Active,
            "resume",
        )
    }

    /// Complete an Active or Paused task → Completed (terminal).
    pub fn complete(&mut self) -> Result<()> {
        if matches!(
            self.state,
            TCClientTaskStatus::Active | TCClientTaskStatus::Paused
        ) {
            self.state = TCClientTaskStatus::Completed;
            Ok(())
        } else {
            Err(Self::illegal("complete", self.state))
        }
    }

    fn transition(
        &mut self,
        from: TCClientTaskStatus,
        to: TCClientTaskStatus,
        op: &str,
    ) -> Result<()> {
        if self.state == from {
            self.state = to;
            Ok(())
        } else {
            Err(Self::illegal(op, self.state))
        }
    }

    fn illegal(op: &str, state: TCClientTaskStatus) -> Error {
        Error::invalid_state(format!(
            "illegal task transition: cannot {op} from {state:?}"
        ))
    }
}

/// One logged process-data sample in a task's time log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogEntry {
    /// Milliseconds since the task runtime's reference clock.
    pub timestamp_ms: u32,
    pub element: ElementNumber,
    pub ddi: DDI,
    pub value: i32,
}

/// A task's data-logging session.
///
/// ISO 11783-10 ties data logging to task state: samples are recorded only
/// while the task is Active. [`record`](TaskLog::record) consults the task
/// [`TaskLifecycle`] and silently drops samples taken while the task is
/// Idle, Paused, or Completed, so a paused task does not accumulate log
/// data — matching the task-controller logging contract.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskLog {
    entries: Vec<LogEntry>,
}

impl TaskLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a sample if (and only if) the task is currently Active.
    /// Returns `true` if the sample was logged.
    pub fn record(
        &mut self,
        lifecycle: &TaskLifecycle,
        timestamp_ms: u32,
        element: ElementNumber,
        ddi: DDI,
        value: i32,
    ) -> bool {
        if lifecycle.state() != TCClientTaskStatus::Active {
            return false;
        }
        self.entries.push(LogEntry {
            timestamp_ms,
            element,
            ddi,
            value,
        });
        true
    }

    #[must_use]
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop all logged samples (e.g. after exporting the session).
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// A high-level task-runtime handle: a [`TaskLifecycle`], a [`TaskLog`],
/// and a monotonic millisecond clock composed into one work-manager API.
///
/// This is the single object a TC work-manager product drives: it forwards
/// lifecycle transitions, advances the clock with [`tick`](TaskSession::tick),
/// and logs process-data values with the current timestamp — automatically
/// gated on the Active state by [`TaskLog`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskSession {
    lifecycle: TaskLifecycle,
    log: TaskLog,
    clock_ms: u32,
}

impl TaskSession {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the session clock by `delta_ms` (saturating).
    pub fn tick(&mut self, delta_ms: u32) {
        self.clock_ms = self.clock_ms.saturating_add(delta_ms);
    }

    /// Current session clock in milliseconds.
    #[must_use]
    pub const fn clock_ms(&self) -> u32 {
        self.clock_ms
    }

    /// Start the task (Idle → Active).
    pub fn start(&mut self) -> Result<()> {
        self.lifecycle.start()
    }

    /// Pause the task (Active → Paused).
    pub fn pause(&mut self) -> Result<()> {
        self.lifecycle.pause()
    }

    /// Resume the task (Paused → Active).
    pub fn resume(&mut self) -> Result<()> {
        self.lifecycle.resume()
    }

    /// Complete the task (Active/Paused → Completed).
    pub fn complete(&mut self) -> Result<()> {
        self.lifecycle.complete()
    }

    #[must_use]
    pub const fn state(&self) -> TCClientTaskStatus {
        self.lifecycle.state()
    }

    #[must_use]
    pub const fn status_byte(&self) -> u8 {
        self.lifecycle.status_byte()
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.lifecycle.is_finished()
    }

    /// `true` while the task is Active (the state in which application and
    /// data logging happen).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.lifecycle.state() == TCClientTaskStatus::Active
    }

    /// Log a process-data value at the current clock; only recorded while
    /// the task is Active. Returns `true` if it was logged.
    pub fn log_value(&mut self, element: ElementNumber, ddi: DDI, value: i32) -> bool {
        self.log
            .record(&self.lifecycle, self.clock_ms, element, ddi, value)
    }

    #[must_use]
    pub fn log(&self) -> &TaskLog {
        &self.log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_start_pause_resume_complete() {
        let mut t = TaskLifecycle::new();
        assert_eq!(t.state(), TCClientTaskStatus::Idle);
        assert_eq!(t.status_byte(), 0x00);
        t.start().unwrap();
        assert_eq!(t.state(), TCClientTaskStatus::Active);
        t.pause().unwrap();
        assert_eq!(t.state(), TCClientTaskStatus::Paused);
        t.resume().unwrap();
        assert_eq!(t.state(), TCClientTaskStatus::Active);
        t.complete().unwrap();
        assert_eq!(t.state(), TCClientTaskStatus::Completed);
        assert!(t.is_finished());
        assert_eq!(t.status_byte(), 0x03);
    }

    #[test]
    fn complete_from_paused_is_allowed() {
        let mut t = TaskLifecycle::new();
        t.start().unwrap();
        t.pause().unwrap();
        t.complete().unwrap();
        assert!(t.is_finished());
    }

    #[test]
    fn illegal_transitions_are_rejected_and_state_is_preserved() {
        let mut t = TaskLifecycle::new();
        // Cannot pause/resume/complete before starting.
        assert!(t.pause().is_err());
        assert!(t.resume().is_err());
        assert!(t.complete().is_err());
        assert_eq!(t.state(), TCClientTaskStatus::Idle);

        t.start().unwrap();
        // Cannot start an already-active task, or resume one that is active.
        assert!(t.start().is_err());
        assert!(t.resume().is_err());
        assert_eq!(t.state(), TCClientTaskStatus::Active);

        t.complete().unwrap();
        // Completed is terminal: every further transition is rejected.
        assert!(t.start().is_err());
        assert!(t.pause().is_err());
        assert!(t.resume().is_err());
        assert!(t.complete().is_err());
        assert_eq!(t.state(), TCClientTaskStatus::Completed);
    }

    #[test]
    fn task_log_records_only_while_active() {
        let mut life = TaskLifecycle::new();
        let mut log = TaskLog::new();
        let el = ElementNumber(3);
        let ddi = DDI(0x1234);

        // Idle: dropped.
        assert!(!log.record(&life, 0, el, ddi, 10));
        assert!(log.is_empty());

        life.start().unwrap();
        assert!(log.record(&life, 100, el, ddi, 11));
        assert!(log.record(&life, 200, el, ddi, 12));
        assert_eq!(log.len(), 2);

        // Paused: dropped (no accumulation while paused).
        life.pause().unwrap();
        assert!(!log.record(&life, 300, el, ddi, 13));
        assert_eq!(log.len(), 2);

        // Resumed: logging continues.
        life.resume().unwrap();
        assert!(log.record(&life, 400, el, ddi, 14));
        assert_eq!(log.len(), 3);

        // Completed: dropped.
        life.complete().unwrap();
        assert!(!log.record(&life, 500, el, ddi, 15));
        assert_eq!(log.len(), 3);

        assert_eq!(
            log.entries()[0],
            LogEntry {
                timestamp_ms: 100,
                element: el,
                ddi,
                value: 11,
            }
        );
        assert_eq!(log.entries()[2].value, 14);

        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn task_session_composes_lifecycle_log_and_clock() {
        let mut s = TaskSession::new();
        let el = ElementNumber(5);
        let ddi = DDI(0xABCD);

        // Before start: Idle, logging dropped.
        assert_eq!(s.state(), TCClientTaskStatus::Idle);
        assert!(!s.log_value(el, ddi, 1));

        s.start().unwrap();
        s.tick(500);
        assert!(s.log_value(el, ddi, 42));
        s.tick(500);
        assert!(s.log_value(el, ddi, 43));
        assert_eq!(s.clock_ms(), 1000);
        assert_eq!(s.log().len(), 2);
        // Timestamps reflect the session clock at log time.
        assert_eq!(s.log().entries()[0].timestamp_ms, 500);
        assert_eq!(s.log().entries()[1].timestamp_ms, 1000);

        s.pause().unwrap();
        s.tick(1000);
        assert!(!s.log_value(el, ddi, 99)); // paused → dropped
        assert_eq!(s.log().len(), 2);

        s.complete().unwrap();
        assert!(s.is_finished());
        assert_eq!(s.status_byte(), 0x03);
    }
}
