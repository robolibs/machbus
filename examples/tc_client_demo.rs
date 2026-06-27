//! TC client connect handshake (TC_STATUS → WS Master → version
//! request → DDOP transfer → activate). Mirrors `tc_client_demo.cpp`.

use machbus::isobus::tc::{
    DDOP, DeviceElement, DeviceElementType, DeviceObject, ProcessDataCommands, TCClientConfig,
    TCState, TaskControllerClient, tc_cmd,
};
use machbus::net::Message;
use machbus::net::pgn_defs::PGN_TC_TO_ECU;

fn fixed_response(command: u8, status: u8) -> Vec<u8> {
    let mut data = vec![0xFF; 8];
    data[0] = command;
    data[1] = status;
    data
}

fn main() {
    println!("=== TC Client Demo ===");

    // Build a tiny DDOP (1 device + 1 element).
    let ddop = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("Sprayer 600")
                .with_software_version("1.0.0"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_designator("Root"),
        );

    let mut client = TaskControllerClient::new(TCClientConfig::default());
    client.set_ddop(ddop);
    client.connect().unwrap();
    println!("[1] connect → {:?}", client.state());

    // TC server (0x33) broadcasts canonical process-data Status.
    client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        vec![ProcessDataCommands::Status.as_u8(), 0, 0, 0, 0, 0, 0, 0],
        0x33,
    ));
    println!(
        "[2] TC_STATUS → {:?}, tc_addr=0x{:02X}",
        client.state(),
        client.tc_address()
    );

    // update() ships WS Master.
    let _ = client.update(1);
    // update() ships VERSION_REQUEST.
    let _ = client.update(1);
    println!(
        "[3] sent WS Master + VERSION_REQUEST → {:?}",
        client.state()
    );

    // Version / technical-capabilities response.
    client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        vec![tc_cmd::VERSION_RESPONSE, 4, 1, 8, 0, 0xFF, 0xFF, 0xFF],
        0x33,
    ));
    println!(
        "[4] TC version={}, → {:?}",
        client.tc_version(),
        client.state()
    );

    // update() ships DDOP transfer.
    let frames = client.update(1);
    println!(
        "[5] DDOP frame size={} bytes → {:?}",
        frames[0].data.len(),
        client.state()
    );

    // POOL_RESPONSE then ACTIVATE_RESPONSE.
    client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        fixed_response(tc_cmd::OBJECT_POOL_RESPONSE, 0),
        0x33,
    ));
    let _ = client.update(1); // sends ACTIVATE_POOL
    client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        fixed_response(tc_cmd::ACTIVATE_RESPONSE, 0),
        0x33,
    ));
    assert_eq!(client.state(), TCState::Connected);
    println!("[6] activated → {:?}  ✓", client.state());
}
