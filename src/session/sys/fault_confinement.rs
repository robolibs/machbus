//! Session-level CAN fault-confinement polling (ISO 11783-2/-3 fail-safe).
//!
//! [`Driver`] polls each port's [`CanTransport::bus_state`] once per pump and
//! feeds a per-port [`FaultConfinementMonitor`]. When a port's required action
//! changes (normal → degrade → fail-safe and back) a
//! [`BusEvent::ConfinementChanged`] is queued so the application — and the
//! autonomy path — can react to bus-off.
//!
//! [`Driver`]: crate::session::driver::Driver
//! [`CanTransport::bus_state`]: crate::net::can_transport::CanTransport::bus_state
//! [`FaultConfinementMonitor`]: crate::net::fault_confinement::FaultConfinementMonitor
//! [`BusEvent::ConfinementChanged`]: crate::session::sys::BusEvent::ConfinementChanged
