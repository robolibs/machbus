//! `stack.tc_server()` — Task Controller *server* handle.
//!
//! Wraps [`crate::isobus::tc::TaskControllerServer`]. Inbound
//! `PGN_ECU_TO_TC` is routed through `TaskControllerServer::handle_client_message`;
//! periodic TC_STATUS broadcasts are auto-shipped on [`Stack::tick`].

use crate::isobus::tc::{DDI, ElementNumber, TCServerState};

/// TC server-side events on the unified queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcServerEvent {
    StateChanged(TCServerState),
    ClientVersionReceived {
        address: u8,
        version: u8,
    },
    PeerControlAssignment {
        source: u8,
        destination: u8,
        source_element: ElementNumber,
        source_ddi: DDI,
        destination_element: ElementNumber,
        destination_ddi: DDI,
    },
}
