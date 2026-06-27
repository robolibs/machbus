//! Periodic [`Scheduler`] and 32-bit [`ProcessingFlags`].
//!
//! Mirrors the C++ `machbus::net::Scheduler` and `ProcessingFlags`.
//! Both are polled from `update`-style loops.

use alloc::{boxed::Box, string::String, vec::Vec};

/// One periodic task tracked by [`Scheduler`].
pub struct PeriodicTask {
    pub name: String,
    pub interval_ms: u32,
    pub elapsed_ms: u32,
    pub enabled: bool,
    /// 0 = unlimited retries.
    pub max_retries: u8,
    pub retry_count: u8,
    /// Returns `true` if the task completed successfully; `false` to
    /// retry on the next interval.
    pub callback: Option<Box<dyn FnMut() -> bool>>,
}

impl PeriodicTask {
    #[inline]
    #[must_use]
    pub fn due(&self) -> bool {
        self.enabled && self.interval_ms != 0 && self.elapsed_ms >= self.interval_ms
    }
}

impl core::fmt::Debug for PeriodicTask {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PeriodicTask")
            .field("name", &self.name)
            .field("interval_ms", &self.interval_ms)
            .field("elapsed_ms", &self.elapsed_ms)
            .field("enabled", &self.enabled)
            .field("max_retries", &self.max_retries)
            .field("retry_count", &self.retry_count)
            .field("has_callback", &self.callback.is_some())
            .finish()
    }
}

/// Lightweight periodic-task scheduler. Tasks are identified by their
/// insertion index.
#[derive(Default)]
pub struct Scheduler {
    tasks: Vec<PeriodicTask>,
}

impl Scheduler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a periodic task. Returns its index, which is the handle for
    /// [`Self::enable`] / [`Self::disable`] / [`Self::trigger`].
    pub fn add<F>(
        &mut self,
        name: impl Into<String>,
        interval_ms: u32,
        max_retries: u8,
        callback: F,
    ) -> usize
    where
        F: FnMut() -> bool + 'static,
    {
        self.tasks.push(PeriodicTask {
            name: name.into(),
            interval_ms,
            elapsed_ms: 0,
            enabled: true,
            max_retries,
            retry_count: 0,
            callback: Some(Box::new(callback)),
        });
        self.tasks.len() - 1
    }

    /// Enable a task; resets elapsed and retry counters.
    pub fn enable(&mut self, index: usize) {
        if let Some(t) = self.tasks.get_mut(index) {
            t.enabled = true;
            t.elapsed_ms = 0;
            t.retry_count = 0;
        }
    }

    pub fn disable(&mut self, index: usize) {
        if let Some(t) = self.tasks.get_mut(index) {
            t.enabled = false;
        }
    }

    /// Force a task to run on the next [`Self::update`] call.
    pub fn trigger(&mut self, index: usize) {
        if let Some(t) = self.tasks.get_mut(index) {
            t.elapsed_ms = t.interval_ms;
        }
    }

    /// Advance the scheduler by `delta_ms` and run any due tasks.
    pub fn update(&mut self, delta_ms: u32) {
        for t in &mut self.tasks {
            if !t.enabled {
                continue;
            }
            t.elapsed_ms = t.elapsed_ms.saturating_add(delta_ms);
            if t.due() {
                t.elapsed_ms -= t.interval_ms;
                if let Some(cb) = t.callback.as_mut() {
                    let completed = cb();
                    if completed {
                        t.retry_count = 0;
                    } else {
                        t.retry_count = t.retry_count.saturating_add(1);
                        if t.max_retries > 0 && t.retry_count >= t.max_retries {
                            t.enabled = false;
                        }
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.tasks.len()
    }

    #[must_use]
    pub fn is_enabled(&self, index: usize) -> bool {
        self.tasks.get(index).is_some_and(|t| t.enabled)
    }

    pub fn clear(&mut self) {
        self.tasks.clear();
    }
}

impl core::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Scheduler")
            .field("count", &self.tasks.len())
            .finish()
    }
}

/// Bit-flag based one-shot processor. Handlers are registered per
/// flag index `0..=31`; setting a flag schedules the handler to fire
/// on the next [`Self::process`] call, which then clears the flag.
#[derive(Default)]
pub struct ProcessingFlags {
    flags: u32,
    handlers: Vec<Option<Box<dyn FnMut()>>>,
}

impl ProcessingFlags {
    pub const MAX_FLAGS: u8 = 32;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) the handler for a flag index.
    pub fn register_flag<F>(&mut self, index: u8, handler: F)
    where
        F: FnMut() + 'static,
    {
        let i = index as usize;
        while self.handlers.len() <= i {
            self.handlers.push(None);
        }
        self.handlers[i] = Some(Box::new(handler));
    }

    pub fn set(&mut self, index: u8) {
        if index < Self::MAX_FLAGS {
            self.flags |= 1 << index;
        }
    }

    pub fn clear(&mut self, index: u8) {
        if index < Self::MAX_FLAGS {
            self.flags &= !(1 << index);
        }
    }

    #[must_use]
    pub fn is_set(&self, index: u8) -> bool {
        index < Self::MAX_FLAGS && (self.flags & (1 << index)) != 0
    }

    /// Run handlers for all set flags, clearing each flag before its
    /// handler fires.
    pub fn process(&mut self) {
        let limit = Self::MAX_FLAGS.min(self.handlers.len() as u8);
        for i in 0..limit {
            let mask = 1u32 << i;
            if (self.flags & mask) != 0 {
                self.flags &= !mask;
                if let Some(h) = self.handlers[i as usize].as_mut() {
                    h();
                }
            }
        }
    }

    #[inline]
    #[must_use]
    pub const fn pending(&self) -> u32 {
        self.flags
    }

    #[inline]
    #[must_use]
    pub const fn any_pending(&self) -> bool {
        self.flags != 0
    }
}

impl core::fmt::Debug for ProcessingFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProcessingFlags")
            .field("flags", &format_args!("0x{:08X}", self.flags))
            .field("handlers", &self.handlers.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // ─── Scheduler ───────────────────────────────────────────────────

    #[test]
    fn scheduler_runs_task_at_interval() {
        let mut s = Scheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        s.add("tick", 100, 0, move || {
            *c.borrow_mut() += 1;
            true
        });

        s.update(50);
        assert_eq!(*count.borrow(), 0);
        s.update(60);
        assert_eq!(*count.borrow(), 1);
        // After firing, elapsed resets — needs another full interval.
        s.update(50);
        assert_eq!(*count.borrow(), 1);
        s.update(60);
        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn scheduler_preserves_overshoot_after_long_tick() {
        let mut s = Scheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        s.add("tick", 100, 0, move || {
            *c.borrow_mut() += 1;
            true
        });

        // A long poll must not erase the 250 ms overshoot. Like Timer, the
        // scheduler emits once per update; callers can drain catch-up work with
        // subsequent zero-delta polls if they need exact interval accounting.
        s.update(350);
        assert_eq!(*count.borrow(), 1);
        s.update(0);
        assert_eq!(*count.borrow(), 2);
        s.update(0);
        assert_eq!(*count.borrow(), 3);
        s.update(0);
        assert_eq!(*count.borrow(), 3);
        s.update(50);
        assert_eq!(*count.borrow(), 4);
    }

    #[test]
    fn scheduler_zero_interval_task_never_fires() {
        let mut s = Scheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        let i = s.add("zero", 0, 0, move || {
            *c.borrow_mut() += 1;
            true
        });

        for delta in [0, 1, 1000, u32::MAX] {
            s.update(delta);
        }
        s.trigger(i);
        s.update(0);

        assert_eq!(*count.borrow(), 0);
        assert!(s.is_enabled(i));
    }

    #[test]
    fn scheduler_disable_skips_task() {
        let mut s = Scheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        let i = s.add("tick", 100, 0, move || {
            *c.borrow_mut() += 1;
            true
        });

        s.disable(i);
        s.update(500);
        assert_eq!(*count.borrow(), 0);

        s.enable(i);
        s.update(150);
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn scheduler_trigger_fires_immediately() {
        let mut s = Scheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        let i = s.add("tick", 1000, 0, move || {
            *c.borrow_mut() += 1;
            true
        });
        s.trigger(i);
        s.update(0);
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn scheduler_max_retries_disables_after_failures() {
        let mut s = Scheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        let i = s.add("retry", 10, 3, move || {
            *c.borrow_mut() += 1;
            false // never completes
        });

        // Run enough updates to fire ≥ 3 times.
        for _ in 0..10 {
            s.update(10);
        }
        assert_eq!(*count.borrow(), 3);
        assert!(!s.is_enabled(i));
    }

    #[test]
    fn scheduler_completed_task_resets_retries() {
        let mut s = Scheduler::new();
        let attempts = Rc::new(RefCell::new(0u32));
        let attempts_c = attempts.clone();
        s.add("flaky", 10, 5, move || {
            let mut n = attempts_c.borrow_mut();
            *n += 1;
            (*n).is_multiple_of(2) // fails on odd attempts
        });

        // 5 fires: 1=fail, 2=ok→reset, 3=fail, 4=ok→reset, 5=fail
        // Never hits 5 consecutive fails ⇒ stays enabled.
        for _ in 0..5 {
            s.update(10);
        }
        assert_eq!(*attempts.borrow(), 5);
        assert!(s.is_enabled(0));
    }

    // ─── ProcessingFlags ────────────────────────────────────────────

    #[test]
    fn flags_set_and_process() {
        let mut pf = ProcessingFlags::new();
        let log = Rc::new(RefCell::new(Vec::<u8>::new()));
        for i in [0u8, 3, 7, 31] {
            let l = log.clone();
            pf.register_flag(i, move || l.borrow_mut().push(i));
        }
        pf.set(0);
        pf.set(3);
        pf.set(31);
        assert!(pf.any_pending());

        pf.process();
        assert_eq!(*log.borrow(), vec![0, 3, 31]);
        assert!(!pf.any_pending());
    }

    #[test]
    fn flags_oob_index_is_ignored() {
        let mut pf = ProcessingFlags::new();
        pf.set(255); // ignored
        assert!(!pf.any_pending());
        assert!(!pf.is_set(255));
    }

    #[test]
    fn flags_clear_removes_pending() {
        let mut pf = ProcessingFlags::new();
        pf.register_flag(2, || {});
        pf.set(2);
        assert!(pf.is_set(2));
        pf.clear(2);
        assert!(!pf.is_set(2));
    }

    #[test]
    fn flags_unregistered_set_does_nothing_on_process() {
        // Setting a flag with no handler must not panic.
        let mut pf = ProcessingFlags::new();
        pf.set(5);
        pf.process();
        // The flag was never cleared because there's no handler — by design
        // process() only walks flags up to handlers.len().
        assert!(pf.is_set(5));
    }
}
