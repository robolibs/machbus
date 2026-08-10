//! Core CAN transport boundary for caller-owned IO.
//!
//! This trait is intentionally tiny and non-blocking. Embedded applications
//! implement it for their board/HAL adapter, while hosted code can implement it
//! for SocketCAN, simulated links, or test transports. The protocol core speaks
//! [`Frame`] only; concrete controller frame conversion belongs in the adapter.

use super::can_adapter::can::BusState;
use super::frame::Frame;

/// Non-blocking CAN transport boundary used by embedded drivers.
pub trait CanTransport {
    /// Implementation-specific transmit error.
    type Error;

    /// Next received `(port, frame)`, or `None` when no frame is pending.
    fn recv(&mut self) -> Option<(u8, Frame)>;

    /// Transmit `frame` on `port`.
    fn send(&mut self, port: u8, frame: &Frame) -> core::result::Result<(), Self::Error>;

    /// Current CAN error-confinement state of `port`, if the driver can report
    /// it. Defaults to `None` so existing transports keep compiling.
    ///
    /// Without this the stack cannot observe bus-off at all: a controller that
    /// has stopped transmitting looks identical to a quiet bus, and nothing can
    /// drive the ISO 11783-2 §9.6 fail-safe reaction. Implement it wherever the
    /// controller exposes its error counters.
    fn bus_state(&self, port: u8) -> Option<BusState> {
        let _ = port;
        None
    }
}

impl<T: CanTransport + ?Sized> CanTransport for &mut T {
    type Error = T::Error;

    fn recv(&mut self) -> Option<(u8, Frame)> {
        (**self).recv()
    }

    fn send(&mut self, port: u8, frame: &Frame) -> core::result::Result<(), Self::Error> {
        (**self).send(port, frame)
    }

    fn bus_state(&self, port: u8) -> Option<BusState> {
        (**self).bus_state(port)
    }
}
