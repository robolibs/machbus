//! TC server demo responding to TechnicalCapabilities + RequestValue.
//! Mirrors `tc_server_demo.cpp`.

use machbus::isobus::tc::{
    ProcessDataCommands, TCServerConfig, TCServerState, TaskControllerServer,
};
use machbus::net::Message;
use machbus::net::pgn_defs::PGN_ECU_TO_TC;

fn main() {
    println!("=== TC Server Demo ===");

    let mut server = TaskControllerServer::new(
        TCServerConfig::default()
            .with_number(1)
            .with_version(4)
            .with_booms(1)
            .with_sections(8)
            .with_channels(4),
    );
    server.start().unwrap();

    // Hook a value request: gauge sensor returns 42 for any DDI.
    server.on_value_request(|_elem, _ddi, _addr| Ok(42));

    // Client (0x42) asks for tech capabilities → server registers + replies.
    let out = server.handle_client_message(&Message::new(
        PGN_ECU_TO_TC,
        vec![
            ProcessDataCommands::TechnicalCapabilities.as_u8(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
        0x42,
    ));
    println!(
        "[caps] reply len={}, version={}, booms={}, sections={}",
        out[0].data.len(),
        out[0].data[1],
        out[0].data[2],
        out[0].data[3],
    );
    assert_eq!(server.state(), TCServerState::Active);

    // Client requests value for element 3, DDI 0xCAFE.
    let req = TaskControllerServer::build_request_value(3, 0xCAFE)
        .expect("demo element number fits TC process-data wire field");
    let out = server.handle_client_message(&Message::new(PGN_ECU_TO_TC, req.to_vec(), 0x42));
    let value = i32::from_le_bytes(out[0].data[4..8].try_into().unwrap());
    println!("[req_value(elem=3, ddi=0xCAFE)] → {value}");

    // Periodic TC Status broadcast.
    let bytes = server
        .update(machbus::isobus::tc::TC_STATUS_INTERVAL_MS)
        .unwrap();
    println!(
        "\n[status] broadcast tc_number={}, version={}, sections={}",
        bytes[1], bytes[3], bytes[6]
    );
}
