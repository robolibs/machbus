//! Internal protocol/event layer for the `session` facade.
//!
//! This is **not** a public facade — [`crate::session`] is the only public
//! entry point. This module holds the unified [`Event`] enum, the per-subsystem
//! `*Event` types, and the reusable decode helpers the session plugins build on.
//! (The former `Stack` facade that also lived here has been removed; `session`
//! is now the single facade.)

pub mod auxiliary;
pub mod diag;
pub mod dm_memory;
pub mod events;
pub mod fault_confinement;
pub mod fs;
pub mod fs_server;
pub mod gnss;
pub mod guidance;
pub mod heartbeat;
pub mod imp;
pub mod language_command;
pub mod maintain_power;
pub mod powertrain;
pub mod sc;
pub mod shortcut_button;
pub mod tc;
pub mod tc_server;
pub mod tim;
pub mod vt;
pub mod vt_server;

pub use auxiliary::AuxiliaryEvent;
pub use diag::DiagEvent;
pub use dm_memory::DmMemoryEvent;
pub use events::{BusEvent, ClaimEvent, Event, EventQueue, OverflowPolicy};
pub use fs::FsEvent;
pub use fs_server::FsServerEvent;
pub use gnss::GnssEvent;
pub use guidance::GuidanceEvent;
pub use heartbeat::HeartbeatEvent;
pub use imp::{Hitch, ImplementEvent, Pto};
pub use language_command::LanguageCommandEvent;
pub use maintain_power::MaintainPowerEvent;
pub use powertrain::{PowertrainEvent, PowertrainSnapshot};
pub use sc::ScEvent;
pub use shortcut_button::ShortcutButtonEvent;
pub use tc::TcEvent;
pub use tc_server::TcServerEvent;
pub use tim::TimEvent;
pub use vt::VtEvent;
pub use vt_server::VtServerEvent;
