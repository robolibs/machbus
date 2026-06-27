//! Part-oriented standard-suite harness.
//!
//! The files under `tests/standard/` are intentionally grouped by ISO 11783,
//! AEF TIM, or NMEA 2000 area. This top-level integration test module makes
//! those checked-in standard files execute under Cargo instead of serving only as
//! documentation/evidence snippets.
//!
//! The facade-level integration checks run on the `session` facade via the
//! shared two-node harness in `session_harness.rs`.

#[path = "standard/session_harness.rs"]
mod session_harness;

#[path = "standard/aef_tim_automation.rs"]
mod aef_tim_automation;
#[path = "standard/iso11783_02_physical.rs"]
mod iso11783_02_physical;
#[path = "standard/iso11783_03_datalink.rs"]
mod iso11783_03_datalink;
#[path = "standard/iso11783_04_network.rs"]
mod iso11783_04_network;
#[path = "standard/iso11783_05_network_management.rs"]
mod iso11783_05_network_management;
#[path = "standard/iso11783_06_vt_commands.rs"]
mod iso11783_06_vt_commands;
#[path = "standard/iso11783_06_vt_objects.rs"]
mod iso11783_06_vt_objects;
#[path = "standard/iso11783_06_vt_render.rs"]
mod iso11783_06_vt_render;
#[path = "standard/iso11783_07_implement_messages.rs"]
mod iso11783_07_implement_messages;
#[path = "standard/iso11783_08_powertrain.rs"]
mod iso11783_08_powertrain;
#[path = "standard/iso11783_09_tecu.rs"]
mod iso11783_09_tecu;
#[path = "standard/iso11783_10_tc_ddop.rs"]
mod iso11783_10_tc_ddop;
#[path = "standard/iso11783_10_tc_process_data.rs"]
mod iso11783_10_tc_process_data;
#[path = "standard/iso11783_11_ddi_usage.rs"]
mod iso11783_11_ddi_usage;
#[path = "standard/iso11783_12_diagnostics.rs"]
mod iso11783_12_diagnostics;
#[path = "standard/iso11783_13_file_server.rs"]
mod iso11783_13_file_server;
#[path = "standard/iso11783_14_sequence_control.rs"]
mod iso11783_14_sequence_control;
#[path = "standard/nmea2000_selected_subset.rs"]
mod nmea2000_selected_subset;
