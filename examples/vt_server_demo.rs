//! Pump a VT server through a client connect handshake. Mirrors
//! `vt_server_demo.cpp`.

use machbus::isobus::vt::{
    DataMaskBody, ObjectPool, VTServer, VTServerConfig, VTServerState, WorkingSetBody, cmd,
    create_data_mask, create_working_set,
};
use machbus::net::Message;
use machbus::net::pgn_defs::PGN_ECU_TO_VT;

fn main() {
    println!("=== VT Server Demo ===");

    let mut server = VTServer::new(
        VTServerConfig::default()
            .with_screen(800, 480)
            .with_version(4),
    );
    server.start().unwrap();
    println!(
        "[boot] state={:?}, screen={}×{}",
        server.state(),
        server.screen_width(),
        server.screen_height()
    );

    // Client at 0x42 asks for memory.
    let mut get_memory = [0xFFu8; 8];
    get_memory[0] = cmd::GET_MEMORY;
    let out = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, get_memory.to_vec(), 0x42));
    println!(
        "[get_memory] reply len={}, dest=0x{:02X}, state={:?}",
        out[0].data.len(),
        out[0].dest.unwrap_or(0),
        server.state()
    );

    // Client uploads a tiny pool.
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend(pool.serialize().unwrap());
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, transfer, 0x42));
    println!(
        "[xfer] client {} pool uploaded? {}",
        server.clients().len(),
        server.clients()[0].pool_uploaded
    );

    // End of Pool. A successful End Of Object Pool response makes the
    // uploaded working set active; there is no separate PoolActivate command.
    let mut end_of_pool = [0xFFu8; 8];
    end_of_pool[0] = cmd::END_OF_POOL;
    let out = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, end_of_pool.to_vec(), 0x42));
    assert_eq!(out[0].data[1], 0x00, "should return success");
    println!(
        "[end_of_pool] reply success={}, state={:?}",
        out[0].data[1] == 0,
        server.state()
    );
    assert_eq!(server.state(), VTServerState::Connected);

    // Periodic VT status broadcast.
    let bytes = server
        .update(machbus::isobus::vt::VT_STATUS_INTERVAL_MS)
        .unwrap();
    println!(
        "\n[status] broadcast cmd=0x{:02X}, active_ws=0x{:02X}",
        bytes[0], bytes[1]
    );
}
