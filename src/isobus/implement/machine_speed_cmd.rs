//! Machine selected speed status + command (ISO 11783-7/9).
//!
//! Mirrors the C++ `machbus::isobus::implement::machine_speed_cmd.hpp`.
//! Two messages share a similar layout:
//!
//! - `PGN_MACHINE_SPEED` (0xF022, 100 ms) — status from TECU.
//! - `PGN_MACHINE_SELECTED_SPEED_CMD` (0xFD43) — command from implement.
//!
//! The C++ `MachineSpeedInterface` is intentionally not ported.

/// Origin of the machine-selected speed (ISO 11783-9 §4.4.2.8).
///
/// This is a **3-bit** field. Read as 2 bits, raw 4 (`Simulated`) aliases to
/// `WheelBased` and raw 7 (`NotAvailable`) aliases to `Blended` — an autonomy
/// controller would then close its loop on a test-bench or absent speed
/// believing it came from the wheels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SpeedSource {
    #[default]
    WheelBased = 0,
    GroundBased = 1,
    NavigationBased = 2,
    Blended = 3,
    /// Bench/spoofed speed. Never close a control loop on this.
    Simulated = 4,
    NotAvailable = 7,
}

impl SpeedSource {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            0 => Self::WheelBased,
            1 => Self::GroundBased,
            2 => Self::NavigationBased,
            3 => Self::Blended,
            4 => Self::Simulated,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::WheelBased),
            1 => Some(Self::GroundBased),
            2 => Some(Self::NavigationBased),
            3 => Some(Self::Blended),
            4 => Some(Self::Simulated),
            7 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    /// `true` when the value does not describe real motion of this machine, so
    /// a controller must not treat it as feedback.
    #[must_use]
    pub const fn is_trustworthy_for_control(self) -> bool {
        !matches!(self, Self::Simulated | Self::NotAvailable)
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum MachineDirection {
    Reverse = 0,
    Forward = 1,
    Error = 2,
    #[default]
    NotAvailable = 3,
}

impl MachineDirection {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Reverse,
            1 => Self::Forward,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Reverse),
            1 => Some(Self::Forward),
            2 => Some(Self::Error),
            3 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SpeedExitCode {
    NotLimited = 0,
    OperatorLimited = 1,
    SystemLimited = 2,
    #[default]
    NotAvailable = 3,
}

impl SpeedExitCode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::NotLimited,
            1 => Self::OperatorLimited,
            2 => Self::SystemLimited,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NotLimited),
            1 => Some(Self::OperatorLimited),
            2 => Some(Self::SystemLimited),
            3 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

fn encode_speed_mps_non_na(mps: f64) -> u16 {
    if !mps.is_finite() {
        return 0;
    }
    let raw = mps / 0.001;
    if raw <= 0.0 {
        0
    } else if raw >= f64::from(u16::MAX) {
        u16::MAX - 1
    } else {
        raw as u16
    }
}

/// A compact speed/direction/source triple retained for C++ parity.
///
/// **This is not PGN 0xF022.** It used to claim to be, and it is a second,
/// contradictory layout for that PGN: it puts direction, source and limit
/// status in byte 5 where ISO 11783-7 puts them in byte 8, and it reads the
/// speed source as 2 bits rather than 3 — so raw 4 (Simulated) aliases to
/// WheelBased, the same hazard [`MachineSelectedSpeedFull`] was already fixed
/// for. The committed real-bus capture contradicts this layout.
///
/// Use [`MachineSelectedSpeedFull`] for anything that touches a bus.
///
/// [`MachineSelectedSpeedFull`]: crate::isobus::implement::MachineSelectedSpeedFull
#[deprecated(
    since = "0.1.0",
    note = "not the ISO 11783-7 PGN 0xF022 layout; use MachineSelectedSpeedFull"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct MachineSelectedSpeedMsg {
    pub speed_raw: u16,
    pub direction: MachineDirection,
    pub source: SpeedSource,
    pub limit_status: SpeedExitCode,
}

#[allow(deprecated)]
impl Default for MachineSelectedSpeedMsg {
    fn default() -> Self {
        Self {
            speed_raw: 0xFFFF,
            direction: MachineDirection::NotAvailable,
            source: SpeedSource::WheelBased,
            limit_status: SpeedExitCode::NotAvailable,
        }
    }
}

#[allow(deprecated)]
impl MachineSelectedSpeedMsg {
    /// Speed in m/s. Returns `0.0` if the raw is the `0xFFFF`
    /// not-available sentinel.
    #[must_use]
    pub fn speed_mps(&self) -> f64 {
        if self.speed_raw == 0xFFFF {
            0.0
        } else {
            self.speed_raw as f64 * 0.001
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.speed_raw & 0xFF) as u8;
        data[1] = ((self.speed_raw >> 8) & 0xFF) as u8;
        // bytes 2..4 (distance) reserved.
        // byte 4: direction (0..2), source (2..4), limit (4..6),
        //         reserved bits 6..8 = 1.
        data[4] = (self.direction.as_u8() & 0x03)
            | ((self.source.as_u8() & 0x03) << 2)
            | ((self.limit_status.as_u8() & 0x03) << 4)
            | 0xC0;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        // ISO 11783-7:2022 §5.4: "All undefined bits should be received as
        // 'don't care' (either masked out or ignored). This permits them to be
        // defined and used in the future without causing any incompatibilities."
        // Requiring the exact value machbus writes made them load-bearing on
        // receive, so a transmitter one revision ahead was rejected outright.
        Some(Self {
            speed_raw: (data[0] as u16) | ((data[1] as u16) << 8),
            direction: MachineDirection::try_from_u8(data[4] & 0x03)?,
            source: SpeedSource::try_from_u8((data[4] >> 2) & 0x03)?,
            limit_status: SpeedExitCode::try_from_u8((data[4] >> 4) & 0x03)?,
        })
    }
}

/// Machine Selected Speed Command (PGN 0xFD43). Issued by an
/// implement to request a target speed/direction from the TECU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineSpeedCommandMsg {
    pub target_speed_raw: u16,
    pub direction_cmd: MachineDirection,
}

impl Default for MachineSpeedCommandMsg {
    fn default() -> Self {
        Self {
            target_speed_raw: 0xFFFF,
            direction_cmd: MachineDirection::NotAvailable,
        }
    }
}

impl MachineSpeedCommandMsg {
    #[must_use]
    pub fn target_speed_mps(&self) -> f64 {
        if self.target_speed_raw == 0xFFFF {
            0.0
        } else {
            self.target_speed_raw as f64 * 0.001
        }
    }

    #[must_use]
    pub fn with_speed_mps(mut self, mps: f64) -> Self {
        self.target_speed_raw = encode_speed_mps_non_na(mps);
        self
    }

    #[must_use]
    pub const fn with_direction(mut self, d: MachineDirection) -> Self {
        self.direction_cmd = d;
        self
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.target_speed_raw & 0xFF) as u8;
        data[1] = ((self.target_speed_raw >> 8) & 0xFF) as u8;
        // byte 2: direction (0..2), reserved bits 2..8 = 1.
        data[2] = (self.direction_cmd.as_u8() & 0x03) | 0xFC;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        // Undefined bits and bytes are "don't care" on receive (§5.4); the
        // defined two bits of byte 2 are masked out below.

        Some(Self {
            target_speed_raw: (data[0] as u16) | ((data[1] as u16) << 8),
            direction_cmd: MachineDirection::try_from_u8(data[2] & 0x03)?,
        })
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    /// The deprecated legacy layout still round-trips through itself; what it
    /// must not do is claim to be PGN 0xF022, which
    /// [`MachineSelectedSpeedFull`] owns.
    ///
    /// [`MachineSelectedSpeedFull`]: crate::isobus::implement::MachineSelectedSpeedFull
    #[test]
    fn legacy_status_round_trip() {
        let m = MachineSelectedSpeedMsg {
            speed_raw: 5000, // 5 m/s
            direction: MachineDirection::Forward,
            source: SpeedSource::GroundBased,
            limit_status: SpeedExitCode::OperatorLimited,
        };
        let bytes = m.encode();
        let decoded = MachineSelectedSpeedMsg::decode(&bytes).unwrap();
        assert_eq!(decoded, m);
        assert!((decoded.speed_mps() - 5.0).abs() < 1e-9);

        // The 2-bit source read is exactly why it may not be used on a bus: a
        // conformant transmitter's Simulated (4) is unrepresentable here.
        assert_eq!(SpeedSource::from_u8(4), SpeedSource::Simulated);
        let aliased = MachineSelectedSpeedMsg {
            source: SpeedSource::Simulated,
            ..m
        };
        assert_eq!(
            MachineSelectedSpeedMsg::decode(&aliased.encode())
                .expect("round trip")
                .source,
            SpeedSource::WheelBased,
            "the legacy 2-bit field aliases Simulated onto WheelBased"
        );
    }

    #[test]
    fn status_default_speed_is_not_available() {
        let m = MachineSelectedSpeedMsg::default();
        assert_eq!(m.speed_mps(), 0.0);
    }

    #[test]
    fn command_round_trip() {
        let m = MachineSpeedCommandMsg::default()
            .with_speed_mps(2.5)
            .with_direction(MachineDirection::Forward);
        let decoded = MachineSpeedCommandMsg::decode(&m.encode()).unwrap();
        assert_eq!(decoded.direction_cmd, MachineDirection::Forward);
        assert!((decoded.target_speed_mps() - 2.5).abs() < 0.001);
    }

    #[test]
    fn command_speed_builder_clamps_without_emitting_not_available() {
        assert_eq!(
            MachineSpeedCommandMsg::default()
                .with_speed_mps(f64::NAN)
                .target_speed_raw,
            0
        );
        assert_eq!(
            MachineSpeedCommandMsg::default()
                .with_speed_mps(-1.0)
                .target_speed_raw,
            0
        );
        assert_eq!(
            MachineSpeedCommandMsg::default()
                .with_speed_mps(1_000_000.0)
                .target_speed_raw,
            0xFFFE
        );
        assert_eq!(
            MachineSpeedCommandMsg::default()
                .with_speed_mps(65.535)
                .target_speed_raw,
            0xFFFE
        );
    }

    #[test]
    fn enums_round_trip() {
        for s in [
            SpeedSource::WheelBased,
            SpeedSource::GroundBased,
            SpeedSource::NavigationBased,
            SpeedSource::Blended,
        ] {
            assert_eq!(SpeedSource::from_u8(s.as_u8()), s);
        }
        for d in [
            MachineDirection::Forward,
            MachineDirection::Reverse,
            MachineDirection::Error,
            MachineDirection::NotAvailable,
        ] {
            assert_eq!(MachineDirection::from_u8(d.as_u8()), d);
        }
        for e in [
            SpeedExitCode::NotLimited,
            SpeedExitCode::OperatorLimited,
            SpeedExitCode::SystemLimited,
            SpeedExitCode::NotAvailable,
        ] {
            assert_eq!(SpeedExitCode::from_u8(e.as_u8()), e);
        }
    }

    #[test]
    fn short_payload_returns_none() {
        assert!(MachineSelectedSpeedMsg::decode(&[0u8; 7]).is_none());
        assert!(MachineSpeedCommandMsg::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn overlong_payload_returns_none() {
        assert!(MachineSelectedSpeedMsg::decode(&[0u8; 9]).is_none());
        assert!(MachineSpeedCommandMsg::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn decoders_reject_bad_reserved_padding_and_bits() {
        // B5 — ISO 11783-7 §5.4: undefined bits and bytes are "don't care" on
        // receive. Every case below used to be asserted as a decode failure.
        let status = MachineSelectedSpeedMsg::default();

        let mut reserved_byte_zeroed = status.encode();
        reserved_byte_zeroed[2] = 0x00;
        reserved_byte_zeroed[3] = 0x00;
        assert_eq!(
            MachineSelectedSpeedMsg::decode(&reserved_byte_zeroed),
            Some(status)
        );

        let mut reserved_bits_clear = status.encode();
        reserved_bits_clear[4] &= 0x3F;
        assert_eq!(
            MachineSelectedSpeedMsg::decode(&reserved_bits_clear),
            Some(status)
        );

        let mut future_tail = status.encode();
        future_tail[5] = 0x00;
        future_tail[7] = 0x5A;
        assert_eq!(MachineSelectedSpeedMsg::decode(&future_tail), Some(status));

        let command = MachineSpeedCommandMsg::default();
        let mut command_reserved_clear = command.encode();
        command_reserved_clear[2] &= 0x03;
        assert_eq!(
            MachineSpeedCommandMsg::decode(&command_reserved_clear),
            Some(command)
        );

        let mut command_future_tail = command.encode();
        command_future_tail[3] = 0x00;
        command_future_tail[7] = 0x5A;
        assert_eq!(
            MachineSpeedCommandMsg::decode(&command_future_tail),
            Some(command)
        );
    }
}
