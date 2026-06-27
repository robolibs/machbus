//! Generic [`StateMachine<S>`] with an `on_transition` event.
//!
//! Mirrors the C++ `machbus::net::StateMachine<StateEnum>`. The state
//! type is bound to `Copy + PartialEq` so transitions can fire the
//! event with `(from, to)` by value.

use super::event::Event;

/// Tiny FSM helper: holds the current state, fires
/// [`Self::on_transition`] only when the state actually changes.
pub struct StateMachine<S: Copy + PartialEq> {
    state: S,
    /// Fires `(from, to)` when [`Self::transition`] changes the state.
    pub on_transition: Event<(S, S)>,
}

impl<S: Copy + PartialEq> StateMachine<S> {
    #[must_use]
    pub fn new(initial: S) -> Self {
        Self {
            state: initial,
            on_transition: Event::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> S {
        self.state
    }

    /// Move to `new_state`. No-op (and no event) if already there.
    pub fn transition(&mut self, new_state: S) {
        if new_state != self.state {
            let old = self.state;
            self.state = new_state;
            self.on_transition.emit(&(old, new_state));
        }
    }

    /// Force the state without firing [`Self::on_transition`]. Useful
    /// for resets and tests.
    pub fn force_state(&mut self, new_state: S) {
        self.state = new_state;
    }

    #[inline]
    #[must_use]
    pub fn is(&self, s: S) -> bool {
        self.state == s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum S {
        Idle,
        Working,
        Done,
    }

    #[test]
    fn initial_state_and_is() {
        let m = StateMachine::new(S::Idle);
        assert_eq!(m.state(), S::Idle);
        assert!(m.is(S::Idle));
        assert!(!m.is(S::Working));
    }

    #[test]
    fn transition_changes_state_and_fires_event() {
        let mut m = StateMachine::new(S::Idle);
        let log = Rc::new(RefCell::new(Vec::<(S, S)>::new()));
        let l = log.clone();
        m.on_transition.subscribe(move |&t| l.borrow_mut().push(t));

        m.transition(S::Working);
        m.transition(S::Done);
        assert_eq!(m.state(), S::Done);
        assert_eq!(
            *log.borrow(),
            vec![(S::Idle, S::Working), (S::Working, S::Done)]
        );
    }

    #[test]
    fn no_event_on_self_transition() {
        let mut m = StateMachine::new(S::Idle);
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        m.on_transition.subscribe(move |_| *c.borrow_mut() += 1);

        m.transition(S::Idle);
        m.transition(S::Idle);
        assert_eq!(*count.borrow(), 0);
        assert_eq!(m.state(), S::Idle);
    }

    #[test]
    fn force_state_skips_event() {
        let mut m = StateMachine::new(S::Idle);
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        m.on_transition.subscribe(move |_| *c.borrow_mut() += 1);

        m.force_state(S::Done);
        assert_eq!(m.state(), S::Done);
        assert_eq!(*count.borrow(), 0);
    }
}
