//! File-server smoke test: open / write / seek / read round-trip.
//! Mirrors `file_server_demo.cpp`.

use machbus::isobus::fs::{FSError, FSFunction, FileServer, FileServerConfig, OpenFlags};
use machbus::net::Message;
use machbus::net::pgn_defs::PGN_FILE_CLIENT_TO_SERVER;

fn main() {
    println!("=== File Server Demo ===");

    let mut server = FileServer::new(FileServerConfig::default());
    server.add_directory("\\").unwrap();
    println!("[boot] open files cap = {}, max per client = {}", 32, 8);

    // Client (0x42) opens a new file with Create+Write.
    let mut req = vec![FSFunction::OpenFile.as_u8(), 0x01];
    let path = b"data.bin";
    req.push(path.len() as u8);
    req.push(OpenFlags::Write | OpenFlags::Create);
    req.extend_from_slice(path);
    let out = server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, req, 0x42));
    let handle = out[0].data[3];
    assert_eq!(out[0].data[2], FSError::Success.as_u8());
    println!("[open ] handle = {handle}");

    // Write 5 bytes.
    let mut wreq = vec![FSFunction::WriteFile.as_u8(), 0x02, handle];
    wreq.extend_from_slice(&5u16.to_le_bytes());
    wreq.extend_from_slice(b"hello");
    let out = server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, wreq, 0x42));
    println!(
        "[write] wrote {} bytes (success={})",
        u16::from_le_bytes([out[0].data[3], out[0].data[4]]),
        out[0].data[2] == 0
    );

    // Seek back to 0.
    let mut sreq = vec![FSFunction::SeekFile.as_u8(), 0x03, handle];
    sreq.extend_from_slice(&0u32.to_le_bytes());
    server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, sreq, 0x42));

    // Read 5 bytes back.
    let mut rreq = vec![0xFF; 8];
    rreq[0] = FSFunction::ReadFile.as_u8();
    rreq[1] = 0x04;
    rreq[2] = handle;
    rreq[3..5].copy_from_slice(&5u16.to_le_bytes());
    let out = server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, rreq, 0x42));
    let read_count = u16::from_le_bytes([out[0].data[3], out[0].data[4]]) as usize;
    let read_data = &out[0].data[5..5 + read_count];
    println!(
        "[read ] {} bytes: {:?}",
        read_count,
        std::str::from_utf8(read_data).unwrap_or("?")
    );
    assert_eq!(read_data, b"hello");

    // Status broadcast on the next cadence tick.
    let frames = server.update(2001);
    println!(
        "\n[status] broadcasts: {}, busy={}",
        frames.len(),
        server.is_busy()
    );
}
