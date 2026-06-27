//! CAN adapter seam between the protocol core and hosted CAN backends.
//!
//! Hosted builds use `wirebit` directly. Embedded builds keep a tiny
//! no-IO stand-in so the protocol core can compile as `no_std + alloc`
//! without exposing a fake root-level `wirebit` crate alias.

#[cfg(feature = "wirebit")]
pub use wirebit::{CanEndpoint, CanFrame, Error, Frame, Link, Result, ShmLink};

#[cfg(feature = "wirebit")]
pub mod can {
    pub use wirebit::{BusState, CanConfig};
}

#[cfg(feature = "wirebit")]
pub mod topology {
    pub use wirebit::Built;
}

#[cfg(not(feature = "wirebit"))]
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(not(feature = "wirebit"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Empty,
    Other(alloc::string::String),
}

#[cfg(not(feature = "wirebit"))]
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => f.write_str("empty"),
            Self::Other(msg) => f.write_str(msg),
        }
    }
}

#[cfg(not(feature = "wirebit"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame;

#[cfg(not(feature = "wirebit"))]
pub trait Link {
    fn send(&mut self, frame: &Frame) -> Result<()>;
    fn recv(&mut self) -> Result<Frame>;
    fn can_send(&self) -> bool;
    fn can_recv(&self) -> bool;
    fn name(&self) -> &str;
}

#[cfg(not(feature = "wirebit"))]
pub mod can {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum BusState {
        #[default]
        ErrorActive,
        ErrorPassive,
        BusOff,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct CanConfig;
}

#[cfg(not(feature = "wirebit"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CanFrame {
    raw_id: u32,
    pub data: [u8; 8],
    pub can_dlc: u8,
    extended: bool,
    rtr: bool,
    err: bool,
}

#[cfg(not(feature = "wirebit"))]
impl CanFrame {
    #[must_use]
    pub fn make_ext(id: u32, payload: &[u8]) -> Self {
        let mut data = [0u8; 8];
        let len = payload.len().min(8);
        data[..len].copy_from_slice(&payload[..len]);
        Self {
            raw_id: id,
            data,
            can_dlc: len as u8,
            extended: true,
            rtr: false,
            err: false,
        }
    }

    #[must_use]
    pub const fn is_extended(&self) -> bool {
        self.extended
    }

    #[must_use]
    pub const fn is_rtr(&self) -> bool {
        self.rtr
    }

    #[must_use]
    pub const fn is_err(&self) -> bool {
        self.err
    }

    #[must_use]
    pub const fn id(&self) -> u32 {
        self.raw_id
    }
}

#[cfg(not(feature = "wirebit"))]
pub struct CanEndpoint<L: Link> {
    _link: L,
}

#[cfg(not(feature = "wirebit"))]
impl<L: Link> CanEndpoint<L> {
    #[must_use]
    pub const fn new(link: L) -> Self {
        Self { _link: link }
    }

    pub fn recv_can(&mut self) -> Result<CanFrame> {
        Err(Error::Empty)
    }

    pub fn send_can(&mut self, _frame: &CanFrame) -> Result<()> {
        Ok(())
    }

    #[must_use]
    pub const fn bus_state(&self) -> can::BusState {
        can::BusState::ErrorActive
    }
}

#[cfg(not(feature = "wirebit"))]
pub struct ShmLink;

#[cfg(not(feature = "wirebit"))]
impl Link for ShmLink {
    fn send(&mut self, _frame: &Frame) -> Result<()> {
        Ok(())
    }

    fn recv(&mut self) -> Result<Frame> {
        Err(Error::Empty)
    }

    fn can_send(&self) -> bool {
        false
    }

    fn can_recv(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "shim"
    }
}

#[cfg(not(feature = "wirebit"))]
pub mod topology {
    pub struct Built;
}
