//! Built-in [`Plugin`](crate::session::Plugin) implementations.
//!
//! Each subsystem is ported here as a self-registering plugin. Phase 2 ships
//! [`Diagnostics`] as the proof-of-concept; later phases add the rest.

pub mod auxiliary;
pub mod diagnostics;
pub mod dm_memory;
pub mod fs_client;
pub mod fs_server;
pub mod functionalities;
pub mod gnss;
pub mod group_function;
pub mod guidance;
pub mod heartbeat;
pub mod implement;
pub mod language_command;
pub mod maintain_power;
pub mod name_management;
pub mod powertrain;
pub mod request2;
pub mod sc_client;
pub mod sc_master;
pub mod shortcut_button;
pub mod tc_client;
pub mod tc_server;
pub mod tim;
pub mod vt_client;
pub mod vt_server;

pub use auxiliary::Auxiliary;
pub use diagnostics::Diagnostics;
pub use dm_memory::DmMemory;
pub use fs_client::FsClient;
pub use fs_server::FsServer;
pub use functionalities::ControlFunctionalities;
pub use gnss::Gnss;
pub use group_function::GroupFunction;
pub use guidance::Guidance;
pub use heartbeat::Heartbeat;
pub use implement::Implement;
pub use language_command::LanguageCommand;
pub use maintain_power::MaintainPower;
pub use name_management::NameManagement;
pub use powertrain::Powertrain;
pub use request2::Request2;
pub use sc_client::ScClient;
pub use sc_master::ScMaster;
pub use shortcut_button::ShortcutButton;
pub use tc_client::TcClient;
pub use tc_server::TcServer;
pub use tim::Tim;
pub use vt_client::VtClient;
pub use vt_server::VtServer;
