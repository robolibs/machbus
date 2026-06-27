//! Safety policy manager: monitors data-source freshness and
//! escalates `Normal → Degraded → Emergency → Shutdown`.
//!
//! Mirrors the C++ `machbus::net::SafetyPolicy`. The C++ `echo::category`
//! log calls are replaced with `tracing::*`.

use alloc::{collections::BTreeMap, format, string::String, vec::Vec};

use super::event::Event;
use super::state_machine::StateMachine;

/// Coarse safety level of the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SafeState {
    /// All systems operating normally.
    #[default]
    Normal = 0,
    /// Partial function loss; safe defaults applied.
    Degraded = 1,
    /// Critical failure; outputs disabled.
    Emergency = 2,
    /// System shutting down.
    Shutdown = 3,
}

/// Action to take when a freshness requirement is missed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum DegradedAction {
    /// Hold last known good value.
    #[default]
    HoldLast = 0,
    /// Gradually reduce to zero / safe value.
    RampDown = 1,
    /// Immediately set to a safe value.
    Immediate = 2,
    /// Disable output entirely.
    Disable = 3,
}

/// Maximum age and escalation timing for one named data source.
#[derive(Debug, Clone)]
pub struct FreshnessRequirement {
    pub source_name: String,
    /// Max allowed age before transitioning to Degraded.
    pub max_age_ms: u32,
    /// Time in Degraded before escalating to Emergency.
    pub escalation_ms: u32,
    pub action: DegradedAction,
}

impl FreshnessRequirement {
    #[must_use]
    pub fn new(source_name: impl Into<String>) -> Self {
        Self {
            source_name: source_name.into(),
            max_age_ms: 500,
            escalation_ms: 2_000,
            action: DegradedAction::HoldLast,
        }
    }

    #[must_use]
    pub fn max_age_ms(mut self, ms: u32) -> Self {
        self.max_age_ms = ms;
        self
    }

    #[must_use]
    pub fn escalation_ms(mut self, ms: u32) -> Self {
        self.escalation_ms = ms;
        self
    }

    #[must_use]
    pub fn action(mut self, a: DegradedAction) -> Self {
        self.action = a;
        self
    }
}

/// Top-level safety configuration applied to the policy.
#[derive(Debug, Clone, Copy)]
pub struct SafetyConfig {
    pub heartbeat_timeout_ms: u32,
    pub command_freshness_ms: u32,
    pub escalation_delay_ms: u32,
    pub default_action: DegradedAction,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            heartbeat_timeout_ms: 500,
            command_freshness_ms: 200,
            escalation_delay_ms: 2_000,
            default_action: DegradedAction::HoldLast,
        }
    }
}

impl SafetyConfig {
    #[must_use]
    pub fn heartbeat_timeout(mut self, ms: u32) -> Self {
        self.heartbeat_timeout_ms = ms;
        self
    }

    #[must_use]
    pub fn command_freshness(mut self, ms: u32) -> Self {
        self.command_freshness_ms = ms;
        self
    }

    #[must_use]
    pub fn escalation_delay(mut self, ms: u32) -> Self {
        self.escalation_delay_ms = ms;
        self
    }

    #[must_use]
    pub fn default_degraded_action(mut self, a: DegradedAction) -> Self {
        self.default_action = a;
        self
    }
}

/// Monitors data source freshness and escalates safety state.
pub struct SafetyPolicy {
    config: SafetyConfig,
    state: StateMachine<SafeState>,
    requirements: Vec<FreshnessRequirement>,
    last_seen_ms: BTreeMap<String, u32>,
    current_time_ms: u32,
    degraded_since_ms: u32,

    /// `(old, new)` on every safety-state change.
    pub on_state_change: Event<(SafeState, SafeState)>,
    /// Emitted when a specific source first goes stale.
    pub on_source_timeout: Event<String>,
    /// Emitted with a reason string when entering `Emergency`.
    pub on_emergency: Event<String>,
}

impl SafetyPolicy {
    #[must_use]
    pub fn new(config: SafetyConfig) -> Self {
        Self {
            config,
            state: StateMachine::new(SafeState::Normal),
            requirements: Vec::new(),
            last_seen_ms: BTreeMap::new(),
            current_time_ms: 0,
            degraded_since_ms: 0,
            on_state_change: Event::new(),
            on_source_timeout: Event::new(),
            on_emergency: Event::new(),
        }
    }

    /// Add a freshness requirement; returns `&mut Self` for chaining.
    pub fn require_freshness(&mut self, req: FreshnessRequirement) -> &mut Self {
        let name = req.source_name.clone();
        self.last_seen_ms.insert(name.clone(), 0);
        self.requirements.push(req);
        tracing::debug!(target: "machbus.safety", source = %name, "freshness requirement added");
        self
    }

    /// Mark a source as having been observed at the current tick.
    pub fn report_alive(&mut self, source: impl Into<String>) {
        let name = source.into();
        self.last_seen_ms.insert(name, self.current_time_ms);
    }

    /// Periodic update. Drives state escalation based on source ages.
    pub fn update(&mut self, elapsed_ms: u32) {
        self.current_time_ms = self.current_time_ms.saturating_add(elapsed_ms);

        // Terminal states do nothing further.
        if self.state.is(SafeState::Shutdown) || self.state.is(SafeState::Emergency) {
            return;
        }

        let mut any_stale = false;
        // Iterate by index to avoid borrowing self for the duration.
        let n = self.requirements.len();
        for i in 0..n {
            let (req_name, req_max_age, req_escalation_ms) = {
                let r = &self.requirements[i];
                (r.source_name.clone(), r.max_age_ms, r.escalation_ms)
            };

            let last_seen = self.last_seen_ms.get(&req_name).copied();
            let stale = match last_seen {
                None => true,
                Some(t) => self.current_time_ms.saturating_sub(t) > req_max_age,
            };
            if !stale {
                continue;
            }
            any_stale = true;

            if self.state.is(SafeState::Normal) {
                self.degraded_since_ms = self.current_time_ms;
                self.state.transition(SafeState::Degraded);
                self.on_state_change
                    .emit(&(SafeState::Normal, SafeState::Degraded));
                self.on_source_timeout.emit(&req_name);
                let age = last_seen.map_or(self.current_time_ms, |t| {
                    self.current_time_ms.saturating_sub(t)
                });
                tracing::warn!(
                    target: "machbus.safety",
                    source = %req_name, age_ms = age,
                    "source stale — entering Degraded"
                );
            } else if self.state.is(SafeState::Degraded) {
                let time_in_degraded = self.current_time_ms.saturating_sub(self.degraded_since_ms);
                if time_in_degraded > req_escalation_ms {
                    self.state.transition(SafeState::Emergency);
                    self.on_state_change
                        .emit(&(SafeState::Degraded, SafeState::Emergency));
                    let reason = format!(
                        "source '{req_name}' exceeded escalation timeout ({time_in_degraded} ms)"
                    );
                    self.on_emergency.emit(&reason);
                    tracing::error!(target: "machbus.safety", reason = %reason, "EMERGENCY");
                    return; // terminal
                }
            }
        }

        if !any_stale && self.state.is(SafeState::Degraded) {
            self.state.transition(SafeState::Normal);
            self.on_state_change
                .emit(&(SafeState::Degraded, SafeState::Normal));
            tracing::info!(target: "machbus.safety", "all sources fresh — returning to Normal");
        }
    }

    pub fn trigger_emergency(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        let prev = self.state.state();
        self.state.transition(SafeState::Emergency);
        if prev != SafeState::Emergency {
            self.on_state_change.emit(&(prev, SafeState::Emergency));
            self.on_emergency.emit(&reason);
            tracing::error!(target: "machbus.safety", reason = %reason, "manual emergency triggered");
        }
    }

    /// Reset to `Normal`. Resets every source's last-seen timestamp to
    /// the current time so a stale source doesn't immediately re-trip.
    pub fn reset_to_normal(&mut self) {
        let prev = self.state.state();
        self.state.transition(SafeState::Normal);
        if prev != SafeState::Normal {
            self.on_state_change.emit(&(prev, SafeState::Normal));
            tracing::info!(target: "machbus.safety", from = ?prev, "reset to Normal");
        }
        let now = self.current_time_ms;
        for ts in self.last_seen_ms.values_mut() {
            *ts = now;
        }
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> SafeState {
        self.state.state()
    }

    #[inline]
    #[must_use]
    pub fn is_safe(&self) -> bool {
        self.state.is(SafeState::Normal)
    }

    #[inline]
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.state.is(SafeState::Degraded)
    }

    /// The most severe action across stale sources. In `Normal` this
    /// returns the configured default (informational only).
    #[must_use]
    pub fn current_action(&self) -> DegradedAction {
        let mut worst = self.config.default_action;
        if self.state.is(SafeState::Normal) {
            return worst;
        }
        for req in &self.requirements {
            let stale = match self.last_seen_ms.get(&req.source_name) {
                None => true,
                Some(t) => self.current_time_ms.saturating_sub(*t) > req.max_age_ms,
            };
            if stale && (req.action as u8) > (worst as u8) {
                worst = req.action;
            }
        }
        worst
    }

    #[inline]
    #[must_use]
    pub fn config(&self) -> &SafetyConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn defaults_to_normal() {
        let p = SafetyPolicy::new(SafetyConfig::default());
        assert_eq!(p.state(), SafeState::Normal);
        assert!(p.is_safe());
    }

    #[test]
    fn missing_source_transitions_to_degraded() {
        let mut p = SafetyPolicy::new(SafetyConfig::default());
        p.require_freshness(FreshnessRequirement::new("hb").max_age_ms(100));
        // Source has never been reported alive — first tick past max_age trips it.
        p.update(150);
        assert_eq!(p.state(), SafeState::Degraded);
    }

    #[test]
    fn fresh_source_returns_to_normal() {
        let mut p = SafetyPolicy::new(SafetyConfig::default());
        p.require_freshness(FreshnessRequirement::new("hb").max_age_ms(100));
        p.update(150); // → Degraded
        assert_eq!(p.state(), SafeState::Degraded);

        p.report_alive("hb");
        p.update(10); // age = 10 < 100 → recover
        assert_eq!(p.state(), SafeState::Normal);
    }

    #[test]
    fn escalates_to_emergency_after_escalation_window() {
        let mut p = SafetyPolicy::new(SafetyConfig::default());
        p.require_freshness(
            FreshnessRequirement::new("hb")
                .max_age_ms(100)
                .escalation_ms(500),
        );

        // Trip into Degraded.
        p.update(150);
        assert_eq!(p.state(), SafeState::Degraded);

        // Stay stale long enough to escalate (> 500 ms in Degraded).
        for _ in 0..6 {
            p.update(100);
        }
        assert_eq!(p.state(), SafeState::Emergency);
    }

    #[test]
    fn emergency_is_terminal_until_reset() {
        let mut p = SafetyPolicy::new(SafetyConfig::default());
        p.require_freshness(
            FreshnessRequirement::new("hb")
                .max_age_ms(100)
                .escalation_ms(200),
        );
        p.update(150);
        for _ in 0..3 {
            p.update(100);
        }
        assert_eq!(p.state(), SafeState::Emergency);

        // Reporting alive does not auto-recover from Emergency.
        p.report_alive("hb");
        p.update(10);
        assert_eq!(p.state(), SafeState::Emergency);

        // Manual reset works.
        p.reset_to_normal();
        assert_eq!(p.state(), SafeState::Normal);
    }

    #[test]
    fn manual_trigger_emergency_fires_event() {
        let mut p = SafetyPolicy::new(SafetyConfig::default());
        let reasons = Rc::new(RefCell::new(Vec::<String>::new()));
        let r = reasons.clone();
        p.on_emergency
            .subscribe(move |s| r.borrow_mut().push(s.clone()));

        p.trigger_emergency("operator pressed e-stop");
        assert_eq!(p.state(), SafeState::Emergency);
        assert_eq!(reasons.borrow().len(), 1);
        assert!(reasons.borrow()[0].contains("e-stop"));
    }

    #[test]
    fn current_action_picks_worst_stale() {
        let mut p = SafetyPolicy::new(SafetyConfig::default());
        p.require_freshness(
            FreshnessRequirement::new("a")
                .max_age_ms(50)
                .action(DegradedAction::HoldLast),
        );
        p.require_freshness(
            FreshnessRequirement::new("b")
                .max_age_ms(50)
                .action(DegradedAction::Disable),
        );

        p.update(100); // both stale, → Degraded
        assert_eq!(p.state(), SafeState::Degraded);
        // Disable (=3) is worse than HoldLast (=0).
        assert_eq!(p.current_action(), DegradedAction::Disable);
    }

    #[test]
    fn state_change_event_observable() {
        let mut p = SafetyPolicy::new(SafetyConfig::default());
        p.require_freshness(FreshnessRequirement::new("x").max_age_ms(100));

        let log = Rc::new(RefCell::new(Vec::<(SafeState, SafeState)>::new()));
        let l = log.clone();
        p.on_state_change
            .subscribe(move |t| l.borrow_mut().push(*t));

        p.update(150);
        assert_eq!(
            log.borrow().last().copied(),
            Some((SafeState::Normal, SafeState::Degraded))
        );
    }
}
