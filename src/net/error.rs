//! Error and result types for the machbus stack.
//!
//! Mirrors the C++ `machbus::net::error`. The C++ `Error` struct carries
//! both a code and an optional human-readable message; we keep both via
//! [`Error::new`] / [`Error::with_message`] and a set of factory helpers.

use core::fmt;

use super::types::{Address, Pgn};
use alloc::{format, string::String};

/// Discrete error categories used throughout the stack.
///
/// Numeric ordering matches the C++ `ErrorCode` enum so that any stored
/// or wire-encoded values stay compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    Ok = 0,
    Timeout,
    AddressClaimFailed,
    AddressConflict,
    TransportAborted,
    TransportTimeout,
    InvalidPgn,
    InvalidAddress,
    InvalidData,
    BufferOverflow,
    NotConnected,
    InvalidState,
    PoolError,
    PoolValidation,
    SessionExists,
    NoResources,
    DriverError,
    SocketError,
    InterfaceDown,
}

impl ErrorCode {
    /// Static description suitable for logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Timeout => "timeout",
            Self::AddressClaimFailed => "address claim failed",
            Self::AddressConflict => "address conflict",
            Self::TransportAborted => "transport aborted",
            Self::TransportTimeout => "transport timeout",
            Self::InvalidPgn => "invalid PGN",
            Self::InvalidAddress => "invalid address",
            Self::InvalidData => "invalid data",
            Self::BufferOverflow => "buffer overflow",
            Self::NotConnected => "not connected",
            Self::InvalidState => "invalid state",
            Self::PoolError => "object pool error",
            Self::PoolValidation => "object pool validation",
            Self::SessionExists => "session already exists",
            Self::NoResources => "no resources",
            Self::DriverError => "driver error",
            Self::SocketError => "socket error",
            Self::InterfaceDown => "interface down",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Rich error type — code plus optional context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

impl Error {
    /// Construct an error from a code with no extra context.
    #[must_use]
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            message: String::new(),
        }
    }

    /// Construct an error with explicit human-readable context.
    #[must_use]
    pub fn with_message(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    // ─── Factory helpers (mirror C++ `Error::timeout`, etc.) ─────────
    #[must_use]
    pub fn timeout() -> Self {
        Self::new(ErrorCode::Timeout)
    }
    #[must_use]
    pub fn invalid_pgn(pgn: Pgn) -> Self {
        Self::with_message(ErrorCode::InvalidPgn, format!("0x{pgn:04X} ({pgn})"))
    }
    #[must_use]
    pub fn invalid_address(addr: Address) -> Self {
        Self::with_message(ErrorCode::InvalidAddress, format!("0x{addr:02X}"))
    }
    #[must_use]
    pub fn not_connected() -> Self {
        Self::new(ErrorCode::NotConnected)
    }
    #[must_use]
    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Self::with_message(ErrorCode::InvalidState, msg)
    }
    #[must_use]
    pub fn invalid_data(msg: impl Into<String>) -> Self {
        Self::with_message(ErrorCode::InvalidData, msg)
    }
    #[must_use]
    pub fn transport_aborted(msg: impl Into<String>) -> Self {
        Self::with_message(ErrorCode::TransportAborted, msg)
    }
    #[must_use]
    pub fn buffer_overflow() -> Self {
        Self::new(ErrorCode::BufferOverflow)
    }
    #[must_use]
    pub fn driver_error(msg: impl Into<String>) -> Self {
        Self::with_message(ErrorCode::DriverError, msg)
    }
    #[must_use]
    pub fn socket_error(msg: impl Into<String>) -> Self {
        Self::with_message(ErrorCode::SocketError, msg)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            fmt::Display::fmt(&self.code, f)
        } else {
            write!(f, "{}: {}", self.code, self.message)
        }
    }
}

impl core::error::Error for Error {}

impl From<ErrorCode> for Error {
    fn from(code: ErrorCode) -> Self {
        Self::new(code)
    }
}

/// `Result` alias used throughout the stack.
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_helpers_set_codes() {
        assert_eq!(Error::timeout().code, ErrorCode::Timeout);
        assert_eq!(Error::invalid_pgn(0xEA00).code, ErrorCode::InvalidPgn);
        assert_eq!(Error::invalid_address(0x42).code, ErrorCode::InvalidAddress);
        assert_eq!(Error::not_connected().code, ErrorCode::NotConnected);
        assert_eq!(Error::buffer_overflow().code, ErrorCode::BufferOverflow);
    }

    #[test]
    fn invalid_pgn_carries_value_in_message() {
        let e = Error::invalid_pgn(0xEA00);
        // Message holds context only; the "invalid PGN" prefix comes from
        // the Display impl rendering the code.
        assert!(e.message.contains("0xEA00"));
        assert!(e.message.contains("59904"));
        assert_eq!(format!("{e}"), "invalid PGN: 0xEA00 (59904)");
    }

    #[test]
    fn display_with_message_includes_both() {
        let e = Error::invalid_state("not yet claimed");
        assert_eq!(format!("{e}"), "invalid state: not yet claimed");
    }

    #[test]
    fn display_without_message_shows_code_only() {
        let e = Error::new(ErrorCode::Timeout);
        assert_eq!(format!("{e}"), "timeout");
    }

    #[test]
    fn errorcode_from_conversion() {
        let e: Error = ErrorCode::SessionExists.into();
        assert_eq!(e.code, ErrorCode::SessionExists);
        assert!(e.message.is_empty());
    }

    #[test]
    fn result_alias_works() {
        fn ok_value() -> Result<u32> {
            Ok(7)
        }
        fn err_value() -> Result<u32> {
            Err(Error::timeout())
        }
        assert!(matches!(ok_value(), Ok(7)));
        assert!(matches!(err_value(), Err(e) if e.code == ErrorCode::Timeout));
    }
}
