//! Stack-level CAN fault-confinement polling (ISO 11783-2/-3 fail-safe).
//!
//! On each `Stack::tick` this reads every connected port's CAN error-confinement
//! state and feeds a per-port `FaultConfinementMonitor`. When a port's required
//! action changes (normal → degrade → fail-safe and back), a
//! `BusEvent::ConfinementChanged` is queued so the application can drive its
//! safety policy.
