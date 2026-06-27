//! Build a CF Functionalities (PGN 0xFC8E) payload advertising VT
//! server, TC client, FS client. Mirrors `functionalities_demo.cpp`.

use machbus::isobus::{
    BasicTractorEcuOptions, Functionalities, Functionality, MinimumControlFunctionOptions,
    TaskControllerGeoServerOptions,
};

fn main() {
    println!("=== Functionalities Demo ===");

    let mut f = Functionalities::new()
        .with_ut_server(4)
        .with_tc_basic_server(4)
        .with_tc_geo_server(4)
        .with_basic_tecu_server(2)
        .with_file_server(1);

    // Toggle individual option bits via the per-bit setter API.
    f.set_minimum_control_function_option_state(
        MinimumControlFunctionOptions::SupportOfHeartbeatProducer,
        true,
    );
    f.set_basic_tractor_ecu_server_option_state(BasicTractorEcuOptions::FrontHitchOption, true);
    f.set_basic_tractor_ecu_server_option_state(BasicTractorEcuOptions::GuidanceOption, true);
    f.set_task_controller_geo_server_option_state(
        TaskControllerGeoServerOptions::PolygonBasedPrescriptionMapsAreSupported,
        true,
    );
    f.tc_geo_client_channels = 8; // not a flag
    f.tc_sc_server_booms = 1;
    f.tc_sc_server_sections = 16;

    // Show what we built.
    println!("[supported] {} functionalities:", f.supported().len());
    for (functionality, generation) in f.supported() {
        let mandatory = if matches!(functionality, Functionality::MinimumControlFunction) {
            " (mandatory)"
        } else {
            ""
        };
        println!("  - {functionality:?} (gen={generation}){mandatory}");
    }

    println!("\n[bitfields]");
    println!("  min_cf_options       = 0x{:02X}", f.min_cf_options);
    println!(
        "  basic_tecu_server    = 0x{:02X}",
        f.basic_tecu_server_options
    );
    println!("  tc_geo_server        = 0x{:02X}", f.tc_geo_server_options);

    let bytes = f.serialize();
    println!("\n[wire] {} bytes:", bytes.len());
    println!("  {:02X?}", bytes);

    // Read a few bits back via the per-bit getters.
    println!(
        "\n[introspect] heartbeat producer? {}",
        f.get_minimum_control_function_option_state(
            MinimumControlFunctionOptions::SupportOfHeartbeatProducer
        )
    );
    println!(
        "             front hitch option? {}",
        f.get_basic_tractor_ecu_server_option_state(BasicTractorEcuOptions::FrontHitchOption)
    );
}
