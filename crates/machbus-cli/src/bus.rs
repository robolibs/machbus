//! Bridge between a live SocketCAN socket and a machbus [`Session`].
//!
//! The Session is sans-IO: it produces no IO of its own. This type pumps it
//! one tick at a time — feeding received CAN frames in and draining the
//! session's outbound queue back onto the bus. Used by both `machbus term
//! --iface` (VT server) and `machbus vt-client` so the two sides share one
//! transport path.

use machbus::net::{Frame, Identifier};
use machbus::session::Session;
use machbus::time::Instant;

use crate::can::RawFrame;
use crate::socket::RawSocket;

/// A duplex SocketCAN socket bound to a single interface, used to drive a
/// [`Session`].
pub struct Bus {
    sock: RawSocket,
}

impl Bus {
    /// Open a socket bound to `iface` (must already exist, e.g. `vcan0`).
    pub fn open(iface: &str) -> std::io::Result<Self> {
        Ok(Self {
            sock: crate::socket::open(iface)?,
        })
    }

    /// Drive one pump cycle: ingest every available received frame, then flush
    /// every outbound frame the session wants to transmit.
    pub fn pump(&self, session: &mut Session, now: Instant) {
        // Inbound: raw CAN → machbus protocol frame → session.
        while let Some((raw, _iface)) = self.sock.try_recv().unwrap_or(None) {
            let len = raw.can_dlc.min(8);
            let frame = Frame::new(Identifier::from_raw(raw.id()), raw.data, len);
            session.feed(0, &frame, now);
        }
        // Outbound: session → raw CAN → bus.
        while let Some((_port, frame)) = session.poll_transmit() {
            let raw_id = frame.id.raw;
            let n = (frame.length as usize).min(8);
            let raw = RawFrame::make_ext(raw_id, &frame.data[..n]);
            let _ = self.sock.send(&raw);
        }
    }
}
