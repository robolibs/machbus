//! AEF 023 TIM function facilities and support response (Annex B.3.2).
//!
//! Each facility is a **2-bit tri-state**, not a flag. The crate modelled them
//! as 22 invented 1-bit options in a flat 3-byte LSB-first bitset, which cannot
//! express `11` — "the facility was not defined when the server was built" —
//! the value generation 1 requires for vehicle change of direction. It also
//! lacked the two auxiliary-valve facilities a valve-capable server must
//! support at minimum, so valve commands bypassed the guard entirely.

use alloc::vec::Vec;

use super::functions::TimFunctionId;

/// Message code for the support request/response pair (A.2.3).
pub const MSG_CODE_SUPPORT: u8 = 0xF4;

/// A support response must arrive within this window (B.3.2).
pub const SUPPORT_TIMEOUT_MS: u32 = 1500;

/// One facility, as the 2-bit value of Table 19.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Facility {
    /// `00` — the server does not support this facility.
    NotSupported = 0b00,
    /// `01` — the server supports it.
    Supported = 0b01,
    /// `10` — reserved.
    Reserved = 0b10,
    /// `11` — the facility did not exist when the server was built. Distinct
    /// from "not supported": a newer client must treat it as unknown rather
    /// than as a refusal.
    #[default]
    NotDefinedAtBuild = 0b11,
}

impl Facility {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Self {
        match raw & 0b11 {
            0b00 => Self::NotSupported,
            0b01 => Self::Supported,
            0b10 => Self::Reserved,
            _ => Self::NotDefinedAtBuild,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// `true` only for an explicit `01`. Neither "not defined at build" nor
    /// "reserved" is permission to command.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
}

/// Auxiliary-valve facilities, B.3.2.1.1. One byte: valve state in bits 8-7,
/// valve flow in bits 6-5, bits 4-1 reserved and set to 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AuxValveFacilities {
    pub valve_state: Facility,
    pub valve_flow: Facility,
}

impl AuxValveFacilities {
    #[must_use]
    pub const fn encode(self) -> u8 {
        (self.valve_state.as_u8() << 6) | (self.valve_flow.as_u8() << 4) | 0x0F
    }

    #[must_use]
    pub const fn decode(raw: u8) -> Self {
        Self {
            valve_state: Facility::from_u8(raw >> 6),
            valve_flow: Facility::from_u8(raw >> 4),
        }
    }

    /// B.3.2.1.1: "A TIM server supporting Auxiliary valve automation shall at
    /// minimum support the facilities 'Valve state' and 'Valve flow'."
    #[must_use]
    pub const fn meets_minimum(self) -> bool {
        self.valve_state.is_supported() && self.valve_flow.is_supported()
    }
}

/// The facility block for one function, kept as raw bytes because the length
/// and layout are function-specific (B.3.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFacilities {
    pub function: TimFunctionId,
    pub facilities: Vec<u8>,
}

impl FunctionFacilities {
    #[must_use]
    pub fn new(function: TimFunctionId, facilities: &[u8]) -> Self {
        Self {
            function,
            facilities: facilities.to_vec(),
        }
    }

    /// Read facility `index` (counting 2-bit fields from the MSB of byte 0).
    #[must_use]
    pub fn facility(&self, index: usize) -> Option<Facility> {
        let byte = self.facilities.get(index / 4)?;
        let shift = 6 - 2 * (index % 4);
        Some(Facility::from_u8(byte >> shift))
    }

    /// Interpret this block as auxiliary-valve facilities.
    #[must_use]
    pub fn as_aux_valve(&self) -> Option<AuxValveFacilities> {
        self.facilities
            .first()
            .copied()
            .map(AuxValveFacilities::decode)
    }
}

/// `TIM_FunctionsSupportResponse` (B.3.2). Travels over transport protocol and
/// must list functions in ascending order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FunctionsSupportResponse {
    pub functions: Vec<FunctionFacilities>,
}

impl FunctionsSupportResponse {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(MSG_CODE_SUPPORT);
        data.push(self.functions.len() as u8);
        for entry in &self.functions {
            data.push(entry.function.as_u8());
            data.push(entry.facilities.len() as u8);
            data.extend_from_slice(&entry.facilities);
        }
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 || data[0] != MSG_CODE_SUPPORT {
            return None;
        }
        let count = data[1] as usize;
        let mut functions = Vec::with_capacity(count);
        let mut cursor = 2usize;
        for _ in 0..count {
            let function = TimFunctionId::from_u8(*data.get(cursor)?)?;
            let len = *data.get(cursor + 1)? as usize;
            let start = cursor + 2;
            let end = start.checked_add(len)?;
            let facilities = data.get(start..end)?.to_vec();
            functions.push(FunctionFacilities {
                function,
                facilities,
            });
            cursor = end;
        }
        Some(Self { functions })
    }

    /// Facilities for `function`, if the server listed it.
    #[must_use]
    pub fn get(&self, function: TimFunctionId) -> Option<&FunctionFacilities> {
        self.functions.iter().find(|f| f.function == function)
    }

    /// `true` when the list is in ascending function order, as B.3.2 requires.
    #[must_use]
    pub fn is_ascending(&self) -> bool {
        self.functions
            .windows(2)
            .all(|w| w[0].function.as_u8() < w[1].function.as_u8())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_defined_at_build_is_distinct_from_not_supported() {
        // This is the value a 1-bit flag cannot express, and the one
        // generation 1 requires for vehicle change of direction.
        assert_eq!(Facility::from_u8(0b11), Facility::NotDefinedAtBuild);
        assert_eq!(Facility::from_u8(0b00), Facility::NotSupported);
        assert_ne!(Facility::NotDefinedAtBuild, Facility::NotSupported);

        // Neither is permission to command.
        assert!(!Facility::NotDefinedAtBuild.is_supported());
        assert!(!Facility::NotSupported.is_supported());
        assert!(!Facility::Reserved.is_supported());
        assert!(Facility::Supported.is_supported());
    }

    #[test]
    fn aux_valve_facilities_match_b3211() {
        let facilities = AuxValveFacilities {
            valve_state: Facility::Supported,
            valve_flow: Facility::Supported,
        };
        let raw = facilities.encode();
        assert_eq!(raw >> 6, 0b01, "valve state in bits 8-7");
        assert_eq!((raw >> 4) & 0b11, 0b01, "valve flow in bits 6-5");
        assert_eq!(raw & 0x0F, 0x0F, "bits 4-1 reserved, set to 1");
        assert_eq!(AuxValveFacilities::decode(raw), facilities);
        assert!(facilities.meets_minimum());
    }

    #[test]
    fn a_valve_server_missing_either_facility_fails_the_minimum() {
        assert!(
            !AuxValveFacilities {
                valve_state: Facility::Supported,
                valve_flow: Facility::NotSupported,
            }
            .meets_minimum()
        );

        assert!(
            !AuxValveFacilities {
                valve_state: Facility::NotDefinedAtBuild,
                valve_flow: Facility::Supported,
            }
            .meets_minimum()
        );
    }

    #[test]
    fn support_response_round_trips_variable_length_blocks() {
        let response = FunctionsSupportResponse {
            functions: vec![
                FunctionFacilities::new(TimFunctionId::AuxValve(1), &[0x5F]),
                FunctionFacilities::new(TimFunctionId::VehicleSpeed, &[0x40, 0xFF]),
                FunctionFacilities::new(TimFunctionId::ExternalGuidance, &[0x40]),
            ],
        };
        let bytes = response.encode();
        assert_eq!(bytes[0], MSG_CODE_SUPPORT);
        assert_eq!(bytes[1], 3);
        assert_eq!(FunctionsSupportResponse::decode(&bytes), Some(response));
    }

    #[test]
    fn ascending_order_is_checkable() {
        let ordered = FunctionsSupportResponse {
            functions: vec![
                FunctionFacilities::new(TimFunctionId::AuxValve(1), &[0x5F]),
                FunctionFacilities::new(TimFunctionId::VehicleSpeed, &[0x40]),
            ],
        };
        assert!(ordered.is_ascending());

        let unordered = FunctionsSupportResponse {
            functions: vec![
                FunctionFacilities::new(TimFunctionId::ExternalGuidance, &[0x40]),
                FunctionFacilities::new(TimFunctionId::AuxValve(1), &[0x5F]),
            ],
        };
        assert!(!unordered.is_ascending());
    }

    #[test]
    fn a_truncated_facility_block_is_rejected() {
        // Claims 4 facility bytes but supplies 1.
        let truncated = [MSG_CODE_SUPPORT, 1, 0x46, 4, 0x40];
        assert_eq!(FunctionsSupportResponse::decode(&truncated), None);
    }

    #[test]
    fn facilities_are_read_msb_first_within_a_byte() {
        // 0b01_00_11_01: facility 0 supported, 1 not supported,
        // 2 not-defined-at-build, 3 supported.
        let block = FunctionFacilities::new(TimFunctionId::VehicleSpeed, &[0b0100_1101]);
        assert_eq!(block.facility(0), Some(Facility::Supported));
        assert_eq!(block.facility(1), Some(Facility::NotSupported));
        assert_eq!(block.facility(2), Some(Facility::NotDefinedAtBuild));
        assert_eq!(block.facility(3), Some(Facility::Supported));
        assert_eq!(block.facility(4), None);
    }
}
