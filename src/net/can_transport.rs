//! Core CAN transport boundary for caller-owned IO.
//!
//! This trait is intentionally tiny and non-blocking. Embedded applications
//! implement it for their board/HAL adapter, while hosted code can implement it
//! for SocketCAN, simulated links, or test transports. The protocol core speaks
//! [`Frame`] only; concrete controller frame conversion belongs in the adapter.

use super::frame::Frame;

/// Non-blocking CAN transport boundary used by embedded drivers.
pub trait CanTransport {
    /// Implementation-specific transmit error.
    type Error;

    /// Next received `(port, frame)`, or `None` when no frame is pending.
    fn recv(&mut self) -> Option<(u8, Frame)>;

    /// Transmit `frame` on `port`.
    fn send(&mut self, port: u8, frame: &Frame) -> core::result::Result<(), Self::Error>;
}

impl<T: CanTransport + ?Sized> CanTransport for &mut T {
    type Error = T::Error;

    fn recv(&mut self) -> Option<(u8, Frame)> {
        (**self).recv()
    }

    fn send(&mut self, port: u8, frame: &Frame) -> core::result::Result<(), Self::Error> {
        (**self).send(port, frame)
    }
}
