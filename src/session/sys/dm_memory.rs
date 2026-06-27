//! Stack-owned DM14/DM15/DM16 and ECU Identification workflow.
//!
//! The fixed wire codecs live in [`crate::j1939::dm_memory`]. This module
//! adds stack-level routing so service-tool style requests and responses can
//! be proven over the same transport/addressing path as the rest of the stack.

use crate::j1939::{Dm14Request, Dm15Response, Dm16Transfer, EcuIdentification};
use crate::net::types::Address;

/// Event emitted for stack-owned DM memory/ECU-identification traffic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmMemoryEvent {
    Dm14Request {
        source: Address,
        request: Dm14Request,
    },
    Dm15Response {
        source: Address,
        response: Dm15Response,
    },
    Dm16Transfer {
        source: Address,
        transfer: Dm16Transfer,
    },
    EcuIdentification {
        source: Address,
        identification: EcuIdentification,
    },
}
