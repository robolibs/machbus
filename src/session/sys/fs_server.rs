//! `stack.fs_server()` — File Server *server* handle.
//!
//! Wraps [`crate::isobus::fs::FileServer`]. Inbound `PGN_FILE_CLIENT_TO_SERVER`
//! is routed through `FileServer::handle_client_message`; outbound
//! frames + status broadcasts are auto-shipped on [`Stack::tick`].

use crate::isobus::fs::FileHandle;
use crate::net::types::Address;

/// FS server-side events on the unified queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsServerEvent {
    ClientConnected(Address),
    ClientDisconnected(Address),
    FileOpened { client: Address, path: String },
    FileClosed { client: Address, handle: FileHandle },
}
