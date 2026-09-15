//! Embedded HAL-shaped CAN adapter sketch.
//!
//! Checked with:
//!
//! ```sh
//! cargo check --no-default-features --features embedded --example embedded_hal_adapter
//! ```
//!
//! This intentionally avoids depending on a concrete HAL crate. Replace
//! `BoardCan` with your MCU driver's non-blocking receive/transmit methods and
//! keep the `CanTransport` implementation as the boundary between board IO and
//! `machbus`.

use machbus::net::{CanTransport, Error, ErrorCode, Frame, Identifier, Name, Priority};
use machbus::session::Session;
use machbus::time::Instant;

#[derive(Debug)]
struct BoardCanError;

impl From<BoardCanError> for Error {
    fn from(value: BoardCanError) -> Self {
        Error::with_message(ErrorCode::DriverError, format!("{value:?}"))
    }
}

#[derive(Clone, Copy)]
struct HalFrame {
    id: u32,
    len: u8,
    data: [u8; 8],
}

trait BoardCan {
    fn try_recv(&mut self) -> Option<HalFrame>;
    fn try_send(&mut self, frame: HalFrame) -> Result<(), BoardCanError>;
}

struct HalCanTransport<C> {
    port: u8,
    can: C,
}

impl<C> HalCanTransport<C> {
    const fn new(port: u8, can: C) -> Self {
        Self { port, can }
    }
}

impl<C: BoardCan> CanTransport for HalCanTransport<C> {
    type Error = BoardCanError;

    fn recv(&mut self) -> Option<(u8, Frame)> {
        let raw = self.can.try_recv()?;
        let frame = Frame::new(Identifier { raw: raw.id }, raw.data, raw.len);
        Some((self.port, frame))
    }

    fn send(&mut self, _port: u8, frame: &Frame) -> Result<(), Self::Error> {
        self.can.try_send(HalFrame {
            id: frame.id.raw,
            len: frame.length,
            data: frame.data,
        })
    }
}

#[derive(Default)]
struct DummyBoardCan {
    sent: usize,
}

impl BoardCan for DummyBoardCan {
    fn try_recv(&mut self) -> Option<HalFrame> {
        None
    }

    fn try_send(&mut self, _frame: HalFrame) -> Result<(), BoardCanError> {
        self.sent += 1;
        Ok(())
    }
}

fn main() -> machbus::net::Result<()> {
    let name = Name::default()
        .with_identity_number(0x23456)
        .with_function_code(0x80)
        .with_self_configurable(true);
    let can = HalCanTransport::new(0, DummyBoardCan::default());

    #[cfg(feature = "default")]
    let mut driver = {
        let (controls, driver) = Session::builder(name, 0x81).spawn(can)?;
        controls.start()?;
        driver
    };

    #[cfg(feature = "embedded")]
    let mut driver = {
        let mut session = Session::builder(name, 0x81).build()?;
        session.start()?;
        machbus::session::Driver::new(session, can)
    };

    let _ = driver.poll_at(Instant::ZERO.add_millis(100))?;

    // Application messages use ordinary machbus frames; the adapter owns all
    // conversion to/from the concrete HAL frame.
    let _example_frame = Frame::from_message(Priority::Default, 0x00EA00, 0x81, 0xFF, &[0; 3]);

    Ok(())
}
