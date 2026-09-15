//! Walk a VT client through its full connect FSM:
//! Disconnected → WaitForVTStatus → SendWorkingSetMaster → SendGetMemory
//!  → WaitForMemory → UploadPool → WaitForEndOfPool → Connected.
//! Mirrors `vt_client_demo.cpp`.

use machbus::isobus::vt::{
    DataMaskBody, ObjectPool, VTClient, VTClientConfig, VTState, WorkingSetBody, cmd,
    create_data_mask, create_working_set,
};
use machbus::net::Message;
use machbus::net::pgn_defs::PGN_VT_TO_ECU;

fn main() {
    println!("=== VT Client Demo ===");

    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([10u16]))
        .with_object(create_data_mask(10, &DataMaskBody::default()));

    let mut client = VTClient::new(VTClientConfig::default());
    client.set_object_pool(pool);
    client.connect().unwrap();
    println!("[1] connect()    → {:?}", client.state());

    // Pretend a VT server (addr 0x80) just broadcast its status.
    let mut status = vec![cmd::VT_STATUS, 0xFF];
    status.resize(8, 0xFF);
    status[6] = 4; // version 4
    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, status, 0x80));
    println!("[2] VT_STATUS    → {:?}", client.state());

    // update() emits the WS Master frame.
    let frames = client.update(1);
    println!(
        "[3] update() → {} frame ({:?})",
        frames.len(),
        client.state()
    );

    // update() emits GET_MEMORY.
    let frames = client.update(1);
    println!("[4] update() → GET_MEMORY ({:?})", client.state());
    assert_eq!(frames[0].data[0], cmd::GET_MEMORY);

    // VT replies "memory OK".
    let mut memory_response = [0xFFu8; 8];
    memory_response[0] = cmd::GET_MEMORY_RESPONSE;
    memory_response[1] = 0x00;
    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, memory_response.to_vec(), 0x80));
    println!("[5] mem OK     → {:?}", client.state());

    // update() ships pool transfer + EOP.
    let frames = client.update(1);
    println!(
        "[6] update() → {} frames (pool transfer + EOP)",
        frames.len()
    );

    // VT activates the pool.
    let mut end_of_pool_response = [0xFFu8; 8];
    end_of_pool_response[0] = cmd::END_OF_POOL;
    end_of_pool_response[1] = 0x00;
    end_of_pool_response[6] = 0x00;
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        end_of_pool_response.to_vec(),
        0x80,
    ));
    assert_eq!(client.state(), VTState::Connected);
    println!("[7] EOP ack    → {:?}  ✓", client.state());

    // Now send a UI command.
    let out = client.change_numeric_value(0xCAFE, 42).unwrap();
    println!(
        "\n[ui] change_numeric_value(0xCAFE, 42) → pgn=0x{:04X}, dest=0x{:02X}, len={}",
        out.pgn,
        out.dest.unwrap_or(0),
        out.data.len()
    );
}
