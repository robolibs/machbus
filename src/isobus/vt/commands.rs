//! ISO 11783-6 VT command function codes (shared client/server).
//!
//! Mirrors the C++ `machbus::isobus::vt::vt_cmd` namespace and the
//! [`ActivationCode`] / [`KeyActivationCode`] enums from
//! `commands.hpp`.
//!
//! ## Naming
//!
//! Constants keep the `SCREAMING_SNAKE_CASE` of the C++ source, but
//! group the `vt_cmd` namespace into the public `cmd` submodule so
//! call sites read `cmd::HIDE_SHOW` instead of `vt_cmd::HIDE_SHOW`.

/// VT string-value change commands encode the declared string length in a
/// two-byte little-endian field.
pub const VT_STRING_VALUE_MAX_LEN: usize = u16::MAX as usize;

/// VT command function codes (ISO 11783-6 Annex B / Annex F).
pub mod cmd {
    // ─── VT-to-ECU notifications (Table B.1) ──────────────────────────

    pub const SOFT_KEY_ACTIVATION: u8 = 0x00;
    pub const BUTTON_ACTIVATION: u8 = 0x01;
    pub const POINTING_EVENT: u8 = 0x02;
    pub const SELECT_INPUT_OBJECT: u8 = 0x03;
    pub const VT_ESC: u8 = 0x04;
    pub const NUMERIC_VALUE_CHANGE: u8 = 0x05;
    pub const VT_CHANGE_ACTIVE_MASK: u8 = 0x06;
    pub const VT_CHANGE_SOFT_KEY_MASK: u8 = 0x07;
    pub const STRING_VALUE_CHANGE: u8 = 0x08;
    pub const USER_LAYOUT_HIDE_SHOW: u8 = 0x09;
    pub const CONTROL_AUDIO_SIGNAL_TERMINATION: u8 = 0x0A;
    /// Object Pool Transfer (per ISO 11783-6 F.39).
    pub const OBJECT_POOL_TRANSFER: u8 = 0x11;
    /// End of Object Pool Transfer.
    pub const END_OF_POOL: u8 = 0x12;
    pub const VT_STATUS: u8 = 0xFE;
    pub const WORKING_SET_MAINTENANCE: u8 = 0xFF;

    // ─── ECU-to-VT commands (Table B.2) ───────────────────────────────

    pub const SELECT_ACTIVE_WORKING_SET: u8 = 0x90;
    pub const ESC_INPUT: u8 = 0x92;
    pub const HIDE_SHOW: u8 = 0xA0;
    pub const ENABLE_DISABLE: u8 = 0xA1;
    pub const SELECT_INPUT_OBJECT_COMMAND: u8 = 0xA2;
    pub const CONTROL_AUDIO_SIGNAL: u8 = 0xA3;
    pub const SET_AUDIO_VOLUME: u8 = 0xA4;
    pub const CHANGE_CHILD_LOCATION: u8 = 0xA5;
    pub const CHANGE_SIZE: u8 = 0xA6;
    pub const CHANGE_BACKGROUND_COLOUR: u8 = 0xA7;
    pub const CHANGE_NUMERIC_VALUE: u8 = 0xA8;
    pub const CHANGE_END_POINT: u8 = 0xA9;
    pub const CHANGE_FONT_ATTRIBUTES: u8 = 0xAA;
    pub const CHANGE_LINE_ATTRIBUTES: u8 = 0xAB;
    pub const CHANGE_FILL_ATTRIBUTES: u8 = 0xAC;
    pub const CHANGE_ACTIVE_MASK: u8 = 0xAD;
    pub const CHANGE_SOFT_KEY_MASK: u8 = 0xAE;
    pub const CHANGE_ATTRIBUTE: u8 = 0xAF;
    pub const CHANGE_PRIORITY: u8 = 0xB0;
    pub const CHANGE_LIST_ITEM: u8 = 0xB1;
    pub const DELETE_OBJECT_POOL: u8 = 0xB2;
    pub const CHANGE_STRING_VALUE: u8 = 0xB3;
    pub const CHANGE_CHILD_POSITION: u8 = 0xB4;
    pub const CHANGE_OBJECT_LABEL: u8 = 0xB5;
    pub const CHANGE_POLYGON_POINT: u8 = 0xB6;
    pub const CHANGE_POLYGON_SCALE: u8 = 0xB7;
    pub const GRAPHICS_CONTEXT: u8 = 0xB8;
    pub const GET_ATTRIBUTE_VALUE: u8 = 0xB9;
    pub const SELECT_COLOUR_MAP: u8 = 0xBA;
    pub const IDENTIFY_VT: u8 = 0xBB;
    pub const EXECUTE_EXTENDED_MACRO: u8 = 0xBC;
    pub const LOCK_UNLOCK_MASK: u8 = 0xBD;
    pub const EXECUTE_MACRO: u8 = 0xBE;

    // ─── Object pool transfer (Annex F) ───────────────────────────────

    pub const GET_MEMORY: u8 = 0xC0;
    pub const GET_MEMORY_RESPONSE: u8 = 0xC0;
    pub const GET_SUPPORTED_WIDECHARS: u8 = 0xC1;
    pub const GET_NUMBER_SOFTKEYS: u8 = 0xC2;
    pub const GET_TEXT_FONT_DATA: u8 = 0xC3;
    pub const GET_WINDOW_MASK_DATA: u8 = 0xC4;
    pub const GET_SUPPORTED_OBJECTS: u8 = 0xC5;
    pub const GET_HARDWARE: u8 = 0xC7;

    // ─── Pool version commands ────────────────────────────────────────

    pub const STORE_VERSION: u8 = 0xD0;
    pub const LOAD_VERSION: u8 = 0xD1;
    pub const DELETE_VERSION: u8 = 0xD2;

    // ─── VT v5 Extended Version commands (ISO 11783-6:2018 Annex F) ──

    pub const EXTENDED_GET_VERSIONS: u8 = 0xD3;
    pub const EXTENDED_STORE_VERSION: u8 = 0xD4;
    pub const EXTENDED_LOAD_VERSION: u8 = 0xD5;
    pub const EXTENDED_DELETE_VERSION: u8 = 0xD6;
    pub const GET_VERSIONS: u8 = 0xDF;
    pub const GET_VERSIONS_RESPONSE: u8 = 0xE0;
    pub const UNSUPPORTED_VT_FUNCTION: u8 = 0xFD;

    /// VT v5 extended version label size (32 bytes vs classic 7).
    pub const EXTENDED_VERSION_LABEL_SIZE: usize = 32;
    /// Classic ISO 11783-6 version label size.
    pub const CLASSIC_VERSION_LABEL_SIZE: usize = 7;

    /// VT v5 extended-version sub-function identifier (byte 1
    /// distinguishes extended from classic).
    pub const EXTENDED_VERSION_SUBFUNCTION: u8 = 0xFE;
}

// ─── Activation code (button / soft key) ──────────────────────────────

/// Button or soft-key activation code, ISO 11783-6 Table B.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ActivationCode {
    #[default]
    Released = 0,
    Pressed = 1,
    Held = 2,
    Aborted = 3,
}

impl ActivationCode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match Self::try_from_u8(v) {
            Some(code) => code,
            None => Self::Released,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Released),
            1 => Some(Self::Pressed),
            2 => Some(Self::Held),
            3 => Some(Self::Aborted),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Alias used by the server side. C++ exposes both names for symmetry;
/// Rust keeps the alias as a re-name for source-level parity.
pub type KeyActivationCode = ActivationCode;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_code_round_trip() {
        for v in 0..=3 {
            assert_eq!(ActivationCode::from_u8(v).as_u8(), v);
            assert_eq!(
                ActivationCode::try_from_u8(v).map(ActivationCode::as_u8),
                Some(v)
            );
        }
        assert_eq!(ActivationCode::from_u8(0xFF), ActivationCode::Released);
        assert_eq!(ActivationCode::try_from_u8(0xFF), None);
    }

    #[test]
    fn cmd_codes_match_iso() {
        assert_eq!(cmd::SOFT_KEY_ACTIVATION, 0x00);
        assert_eq!(cmd::SELECT_INPUT_OBJECT, 0x03);
        assert_eq!(cmd::VT_ESC, 0x04);
        assert_eq!(cmd::NUMERIC_VALUE_CHANGE, 0x05);
        assert_eq!(cmd::STRING_VALUE_CHANGE, 0x08);
        assert_eq!(cmd::USER_LAYOUT_HIDE_SHOW, 0x09);
        assert_eq!(cmd::OBJECT_POOL_TRANSFER, 0x11);
        assert_eq!(cmd::END_OF_POOL, 0x12);
        assert_eq!(cmd::SELECT_ACTIVE_WORKING_SET, 0x90);
        assert_eq!(cmd::HIDE_SHOW, 0xA0);
        assert_eq!(cmd::SELECT_INPUT_OBJECT_COMMAND, 0xA2);
        assert_eq!(cmd::SET_AUDIO_VOLUME, 0xA4);
        assert_eq!(cmd::CHANGE_CHILD_LOCATION, 0xA5);
        assert_eq!(cmd::CHANGE_SIZE, 0xA6);
        assert_eq!(cmd::CHANGE_NUMERIC_VALUE, 0xA8);
        assert_eq!(cmd::CHANGE_LIST_ITEM, 0xB1);
        assert_eq!(cmd::DELETE_OBJECT_POOL, 0xB2);
        assert_eq!(cmd::CHANGE_STRING_VALUE, 0xB3);
        assert_eq!(cmd::CHANGE_CHILD_POSITION, 0xB4);
        assert_eq!(cmd::GET_SUPPORTED_OBJECTS, 0xC5);
        assert_eq!(cmd::STORE_VERSION, 0xD0);
        assert_eq!(cmd::LOAD_VERSION, 0xD1);
        assert_eq!(cmd::DELETE_VERSION, 0xD2);
        assert_eq!(cmd::GET_VERSIONS, 0xDF);
        assert_eq!(cmd::GET_VERSIONS_RESPONSE, 0xE0);
        assert_eq!(cmd::EXTENDED_VERSION_SUBFUNCTION, 0xFE);
    }
}
