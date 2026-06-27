//! ISO 11783-12 Control Function Functionalities (PGN 0xFC8E).
//!
//! Mirrors the C++ `machbus::isobus::functionalities.hpp`. Lets ECUs
//! advertise the protocol functionalities they support (UT server,
//! TC client, FS, TIM, etc.) and the options/generations of each.
//!
//! The C++ `ControlFunctionFunctionalities` class embeds `IsoNet&`
//! to handle PGN-Request responses. The Rust port keeps this module as the
//! data model plus [`Functionalities::serialize`] / [`Functionalities::decode`],
//! while the `session` facade wires the
//! model into a stack-level PGN Request responder.

use alloc::{format, vec, vec::Vec};

use crate::net::error::{Error, Result};

// ─── Functionality enum ────────────────────────────────────────────────

/// Functionality byte (`data\[0\]` of each per-functionality block).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Functionality {
    MinimumControlFunction = 0,
    UniversalTerminalServer = 1,
    UniversalTerminalWorkingSet = 2,
    AuxOInputs = 3,
    AuxOFunctions = 4,
    AuxNInputs = 5,
    AuxNFunctions = 6,
    TaskControllerBasicServer = 7,
    TaskControllerBasicClient = 8,
    TaskControllerGeoServer = 9,
    TaskControllerGeoClient = 10,
    TaskControllerSectionControlServer = 11,
    TaskControllerSectionControlClient = 12,
    BasicTractorEcuServer = 13,
    BasicTractorEcuImplementClient = 14,
    TractorImplementManagementServer = 15,
    TractorImplementManagementClient = 16,
    FileServer = 17,
    FileServerClient = 18,
}

impl Functionality {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        Self::try_from_u8(value)
    }

    #[must_use]
    pub const fn try_from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::MinimumControlFunction),
            1 => Some(Self::UniversalTerminalServer),
            2 => Some(Self::UniversalTerminalWorkingSet),
            3 => Some(Self::AuxOInputs),
            4 => Some(Self::AuxOFunctions),
            5 => Some(Self::AuxNInputs),
            6 => Some(Self::AuxNFunctions),
            7 => Some(Self::TaskControllerBasicServer),
            8 => Some(Self::TaskControllerBasicClient),
            9 => Some(Self::TaskControllerGeoServer),
            10 => Some(Self::TaskControllerGeoClient),
            11 => Some(Self::TaskControllerSectionControlServer),
            12 => Some(Self::TaskControllerSectionControlClient),
            13 => Some(Self::BasicTractorEcuServer),
            14 => Some(Self::BasicTractorEcuImplementClient),
            15 => Some(Self::TractorImplementManagementServer),
            16 => Some(Self::TractorImplementManagementClient),
            17 => Some(Self::FileServer),
            18 => Some(Self::FileServerClient),
            _ => None,
        }
    }

    /// Maximum number of option bytes this functionality may advertise in the
    /// PGN 0xFC8E payload. Trailing zero option bytes are omitted on the wire.
    #[must_use]
    pub const fn option_byte_len(self) -> usize {
        match self {
            Self::UniversalTerminalServer
            | Self::UniversalTerminalWorkingSet
            | Self::TaskControllerBasicServer
            | Self::TaskControllerBasicClient
            | Self::FileServer
            | Self::FileServerClient => 0,
            Self::AuxNInputs | Self::AuxNFunctions => 2,
            Self::TaskControllerSectionControlServer | Self::TaskControllerSectionControlClient => {
                2
            }
            Self::TractorImplementManagementServer | Self::TractorImplementManagementClient => 11,
            _ => 1,
        }
    }
}

// ─── Per-functionality option bitflag enums ───────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MinimumControlFunctionOptions {
    NoOptions = 0x00,
    Type1EcuInternalWeakTermination = 0x01,
    Type2EcuInternalEndPointTermination = 0x02,
    SupportOfHeartbeatProducer = 0x04,
    SupportOfHeartbeatConsumer = 0x08,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuxOOptions {
    NoOptions = 0x00,
    SupportsType0Function = 0x01,
    SupportsType1Function = 0x02,
    SupportsType2Function = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum AuxNOptions {
    NoOptions = 0x0000,
    SupportsType0Function = 0x0001,
    SupportsType1Function = 0x0002,
    SupportsType2Function = 0x0004,
    SupportsType3Function = 0x0008,
    SupportsType4Function = 0x0010,
    SupportsType5Function = 0x0020,
    SupportsType6Function = 0x0040,
    SupportsType7Function = 0x0080,
    SupportsType8Function = 0x0100,
    SupportsType9Function = 0x0200,
    SupportsType10Function = 0x0400,
    SupportsType11Function = 0x0800,
    SupportsType12Function = 0x1000,
    SupportsType13Function = 0x2000,
    SupportsType14Function = 0x4000,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskControllerGeoServerOptions {
    NoOptions = 0x00,
    PolygonBasedPrescriptionMapsAreSupported = 0x01,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BasicTractorEcuOptions {
    Class1NoOptions = 0x01,
    Class2NoOptions = 0x02,
    ClassRequiredLighting = 0x04,
    NavigationOption = 0x08,
    FrontHitchOption = 0x10,
    GuidanceOption = 0x20,
}

/// TIM option *bit indices* (not bit values) — packed into a 23-bit
/// bitmask spread across 3 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TractorImplementManagementOptions {
    FrontPtoDisengagementIsSupported = 0,
    FrontPtoEngagementCcwIsSupported = 1,
    FrontPtoEngagementCwIsSupported = 2,
    FrontPtoSpeedCcwIsSupported = 3,
    FrontPtoSpeedCwIsSupported = 4,
    RearPtoDisengagementIsSupported = 5,
    RearPtoEngagementCcwIsSupported = 6,
    RearPtoEngagementCwIsSupported = 7,
    RearPtoSpeedCcwIsSupported = 8,
    RearPtoSpeedCwIsSupported = 9,
    FrontHitchMotionIsSupported = 10,
    FrontHitchPositionIsSupported = 11,
    RearHitchMotionIsSupported = 12,
    RearHitchPositionIsSupported = 13,
    VehicleSpeedInForwardDirectionIsSupported = 14,
    VehicleSpeedInReverseDirectionIsSupported = 15,
    VehicleSpeedStartMotionIsSupported = 16,
    VehicleSpeedStopMotionIsSupported = 17,
    VehicleSpeedForwardSetByServerIsSupported = 18,
    VehicleSpeedReverseSetByServerIsSupported = 19,
    VehicleSpeedChangeDirectionIsSupported = 20,
    GuidanceCurvatureIsSupported = 21,
}

// ─── Per-functionality entry ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionalityData {
    pub functionality: Functionality,
    pub generation: u8,
    pub option_bytes: Vec<u8>,
}

impl FunctionalityData {
    #[must_use]
    pub fn expected_option_byte_len(&self) -> usize {
        self.functionality.option_byte_len()
    }
}

// ─── Functionalities builder ──────────────────────────────────────────

/// Container for the supported functionalities and their options.
/// Construct, set options, then call [`Self::serialize`] to produce
/// the PGN_CF_FUNCTIONALITIES payload.
#[derive(Debug, Clone)]
pub struct Functionalities {
    supported: Vec<(Functionality, u8)>,

    pub min_cf_options: u8,
    pub aux_o_inputs_options: u8,
    pub aux_o_functions_options: u8,
    pub aux_n_inputs_options: u16,
    pub aux_n_functions_options: u16,
    pub tc_geo_server_options: u8,
    /// TC GEO Client: number of control channels.
    pub tc_geo_client_channels: u8,
    pub tc_sc_server_booms: u8,
    pub tc_sc_server_sections: u8,
    pub tc_sc_client_booms: u8,
    pub tc_sc_client_sections: u8,
    pub basic_tecu_server_options: u8,
    pub basic_tecu_client_options: u8,

    /// 23 TIM option bits packed into 3 bytes.
    pub tim_server_options: [u8; 3],
    pub tim_client_options: [u8; 3],
    /// 32 aux valves × 2 bits (state, flow) = 64 bits = 8 bytes.
    pub tim_server_aux_valves: [u8; 8],
    pub tim_client_aux_valves: [u8; 8],
}

impl Default for Functionalities {
    fn default() -> Self {
        // Always include MinimumControlFunction by default — every
        // ISO 11783 device supports it.
        let mut f = Self {
            supported: Vec::new(),
            min_cf_options: 0,
            aux_o_inputs_options: 0,
            aux_o_functions_options: 0,
            aux_n_inputs_options: 0,
            aux_n_functions_options: 0,
            tc_geo_server_options: 0,
            tc_geo_client_channels: 0,
            tc_sc_server_booms: 0,
            tc_sc_server_sections: 0,
            tc_sc_client_booms: 0,
            tc_sc_client_sections: 0,
            basic_tecu_server_options: 0,
            basic_tecu_client_options: 0,
            tim_server_options: [0; 3],
            tim_client_options: [0; 3],
            tim_server_aux_valves: [0; 8],
            tim_client_aux_valves: [0; 8],
        };
        f.set_functionality_supported(Functionality::MinimumControlFunction, 1, true);
        f
    }
}

impl Functionalities {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_functionality_supported(
        &mut self,
        functionality: Functionality,
        generation: u8,
        is_supported: bool,
    ) {
        if is_supported {
            let generation = generation.max(1);
            for entry in &mut self.supported {
                if entry.0 == functionality {
                    entry.1 = generation;
                    return;
                }
            }
            self.supported.push((functionality, generation));
            tracing::debug!(
                target: "machbus.isobus.functionalities",
                functionality = ?functionality,
                generation,
                "functionality enabled",
            );
        } else {
            if functionality == Functionality::MinimumControlFunction {
                return;
            }
            self.supported.retain(|(f, _)| *f != functionality);
            tracing::debug!(
                target: "machbus.isobus.functionalities",
                functionality = ?functionality,
                "functionality disabled",
            );
        }
    }

    // ─── Per-bit option state helpers (mirror C++ set/get pairs) ──

    /// Toggle a single Minimum CF option bit. Mirrors C++
    /// `set_minimum_control_function_option_state`.
    pub fn set_minimum_control_function_option_state(
        &mut self,
        option: MinimumControlFunctionOptions,
        on: bool,
    ) {
        let bit = option as u8;
        if bit == 0 {
            // NoOptions sentinel — clear all bits when set, no-op when off.
            if on {
                self.min_cf_options = 0;
            }
            return;
        }
        if on {
            self.min_cf_options |= bit;
        } else {
            self.min_cf_options &= !bit;
        }
    }

    /// Read a single Minimum CF option bit.
    #[must_use]
    pub fn get_minimum_control_function_option_state(
        &self,
        option: MinimumControlFunctionOptions,
    ) -> bool {
        let bit = option as u8;
        bit != 0 && (self.min_cf_options & bit) != 0
    }

    pub fn set_aux_o_inputs_option_state(&mut self, option: AuxOOptions, on: bool) {
        toggle_u8_bits(&mut self.aux_o_inputs_options, option as u8, on);
    }

    #[must_use]
    pub fn get_aux_o_inputs_option_state(&self, option: AuxOOptions) -> bool {
        let bit = option as u8;
        bit != 0 && (self.aux_o_inputs_options & bit) != 0
    }

    pub fn set_aux_o_functions_option_state(&mut self, option: AuxOOptions, on: bool) {
        toggle_u8_bits(&mut self.aux_o_functions_options, option as u8, on);
    }

    #[must_use]
    pub fn get_aux_o_functions_option_state(&self, option: AuxOOptions) -> bool {
        let bit = option as u8;
        bit != 0 && (self.aux_o_functions_options & bit) != 0
    }

    pub fn set_aux_n_inputs_option_state(&mut self, option: AuxNOptions, on: bool) {
        toggle_u16_bits(&mut self.aux_n_inputs_options, option as u16, on);
    }

    #[must_use]
    pub fn get_aux_n_inputs_option_state(&self, option: AuxNOptions) -> bool {
        let bit = option as u16;
        bit != 0 && (self.aux_n_inputs_options & bit) != 0
    }

    pub fn set_aux_n_functions_option_state(&mut self, option: AuxNOptions, on: bool) {
        toggle_u16_bits(&mut self.aux_n_functions_options, option as u16, on);
    }

    #[must_use]
    pub fn get_aux_n_functions_option_state(&self, option: AuxNOptions) -> bool {
        let bit = option as u16;
        bit != 0 && (self.aux_n_functions_options & bit) != 0
    }

    pub fn set_task_controller_geo_server_option_state(
        &mut self,
        option: TaskControllerGeoServerOptions,
        on: bool,
    ) {
        toggle_u8_bits(&mut self.tc_geo_server_options, option as u8, on);
    }

    #[must_use]
    pub fn get_task_controller_geo_server_option_state(
        &self,
        option: TaskControllerGeoServerOptions,
    ) -> bool {
        let bit = option as u8;
        bit != 0 && (self.tc_geo_server_options & bit) != 0
    }

    pub fn set_basic_tractor_ecu_server_option_state(
        &mut self,
        option: BasicTractorEcuOptions,
        on: bool,
    ) {
        toggle_u8_bits(&mut self.basic_tecu_server_options, option as u8, on);
    }

    #[must_use]
    pub fn get_basic_tractor_ecu_server_option_state(
        &self,
        option: BasicTractorEcuOptions,
    ) -> bool {
        let bit = option as u8;
        bit != 0 && (self.basic_tecu_server_options & bit) != 0
    }

    pub fn set_basic_tractor_ecu_implement_client_option_state(
        &mut self,
        option: BasicTractorEcuOptions,
        on: bool,
    ) {
        toggle_u8_bits(&mut self.basic_tecu_client_options, option as u8, on);
    }

    #[must_use]
    pub fn get_basic_tractor_ecu_implement_client_option_state(
        &self,
        option: BasicTractorEcuOptions,
    ) -> bool {
        let bit = option as u8;
        bit != 0 && (self.basic_tecu_client_options & bit) != 0
    }

    #[must_use]
    pub fn is_functionality_supported(&self, functionality: Functionality) -> bool {
        self.supported.iter().any(|(f, _)| *f == functionality)
    }

    #[must_use]
    pub fn functionality_generation(&self, functionality: Functionality) -> u8 {
        self.supported
            .iter()
            .find(|(f, _)| *f == functionality)
            .map_or(0, |(_, g)| *g)
    }

    // ─── TIM option bits packed into 3 bytes ──────────────────────

    pub fn set_tim_server_option(&mut self, option: TractorImplementManagementOptions, on: bool) {
        let bit = option as u8;
        let byte_idx = (bit / 8) as usize;
        let bit_idx = bit % 8;
        if byte_idx >= 3 {
            return;
        }
        if on {
            self.tim_server_options[byte_idx] |= 1 << bit_idx;
        } else {
            self.tim_server_options[byte_idx] &= !(1 << bit_idx);
        }
    }

    #[must_use]
    pub fn tim_server_option(&self, option: TractorImplementManagementOptions) -> bool {
        let bit = option as u8;
        let byte_idx = (bit / 8) as usize;
        let bit_idx = bit % 8;
        byte_idx < 3 && (self.tim_server_options[byte_idx] & (1 << bit_idx)) != 0
    }

    pub fn set_tim_client_option(&mut self, option: TractorImplementManagementOptions, on: bool) {
        let bit = option as u8;
        let byte_idx = (bit / 8) as usize;
        let bit_idx = bit % 8;
        if byte_idx >= 3 {
            return;
        }
        if on {
            self.tim_client_options[byte_idx] |= 1 << bit_idx;
        } else {
            self.tim_client_options[byte_idx] &= !(1 << bit_idx);
        }
    }

    #[must_use]
    pub fn tim_client_option(&self, option: TractorImplementManagementOptions) -> bool {
        let bit = option as u8;
        let byte_idx = (bit / 8) as usize;
        let bit_idx = bit % 8;
        byte_idx < 3 && (self.tim_client_options[byte_idx] & (1 << bit_idx)) != 0
    }

    /// 32 aux valves × 2 bits each (state, flow). `valve_index` ∈ `0..32`.
    pub fn set_tim_server_aux_valve(
        &mut self,
        valve_index: u8,
        state_supported: bool,
        flow_supported: bool,
    ) {
        if valve_index >= 32 {
            return;
        }
        let byte_idx = (valve_index / 4) as usize;
        let bit_offset = (valve_index % 4) * 2;
        // Clear the 2 bits.
        self.tim_server_aux_valves[byte_idx] &= !(0x03 << bit_offset);
        if state_supported {
            self.tim_server_aux_valves[byte_idx] |= 0x01 << bit_offset;
        }
        if flow_supported {
            self.tim_server_aux_valves[byte_idx] |= 0x02 << bit_offset;
        }
    }

    #[must_use]
    pub fn tim_server_aux_valve_state_supported(&self, valve_index: u8) -> bool {
        if valve_index >= 32 {
            return false;
        }
        let byte_idx = (valve_index / 4) as usize;
        let bit_offset = (valve_index % 4) * 2;
        (self.tim_server_aux_valves[byte_idx] & (0x01 << bit_offset)) != 0
    }

    #[must_use]
    pub fn tim_server_aux_valve_flow_supported(&self, valve_index: u8) -> bool {
        if valve_index >= 32 {
            return false;
        }
        let byte_idx = (valve_index / 4) as usize;
        let bit_offset = (valve_index % 4) * 2;
        (self.tim_server_aux_valves[byte_idx] & (0x02 << bit_offset)) != 0
    }

    pub fn set_tim_client_aux_valve(
        &mut self,
        valve_index: u8,
        state_supported: bool,
        flow_supported: bool,
    ) {
        if valve_index >= 32 {
            return;
        }
        let byte_idx = (valve_index / 4) as usize;
        let bit_offset = (valve_index % 4) * 2;
        self.tim_client_aux_valves[byte_idx] &= !(0x03 << bit_offset);
        if state_supported {
            self.tim_client_aux_valves[byte_idx] |= 0x01 << bit_offset;
        }
        if flow_supported {
            self.tim_client_aux_valves[byte_idx] |= 0x02 << bit_offset;
        }
    }

    // ─── Fluent helpers ───────────────────────────────────────────

    #[must_use]
    pub fn with_min_cf(mut self, generation: u8) -> Self {
        self.set_functionality_supported(Functionality::MinimumControlFunction, generation, true);
        self
    }

    #[must_use]
    pub fn with_ut_server(mut self, generation: u8) -> Self {
        self.set_functionality_supported(Functionality::UniversalTerminalServer, generation, true);
        self
    }

    #[must_use]
    pub fn with_ut_working_set(mut self, generation: u8) -> Self {
        self.set_functionality_supported(
            Functionality::UniversalTerminalWorkingSet,
            generation,
            true,
        );
        self
    }

    #[must_use]
    pub fn with_tc_basic_server(mut self, generation: u8) -> Self {
        self.set_functionality_supported(
            Functionality::TaskControllerBasicServer,
            generation,
            true,
        );
        self
    }

    #[must_use]
    pub fn with_tc_basic_client(mut self, generation: u8) -> Self {
        self.set_functionality_supported(
            Functionality::TaskControllerBasicClient,
            generation,
            true,
        );
        self
    }

    #[must_use]
    pub fn with_tc_geo_server(mut self, generation: u8) -> Self {
        self.set_functionality_supported(Functionality::TaskControllerGeoServer, generation, true);
        self
    }

    #[must_use]
    pub fn with_tc_geo_client(mut self, generation: u8) -> Self {
        self.set_functionality_supported(Functionality::TaskControllerGeoClient, generation, true);
        self
    }

    #[must_use]
    pub fn with_basic_tecu_server(mut self, generation: u8) -> Self {
        self.set_functionality_supported(Functionality::BasicTractorEcuServer, generation, true);
        self
    }

    #[must_use]
    pub fn with_basic_tecu_implement_client(mut self, generation: u8) -> Self {
        self.set_functionality_supported(
            Functionality::BasicTractorEcuImplementClient,
            generation,
            true,
        );
        self
    }

    #[must_use]
    pub fn with_tim_server(mut self, generation: u8) -> Self {
        self.set_functionality_supported(
            Functionality::TractorImplementManagementServer,
            generation,
            true,
        );
        self
    }

    #[must_use]
    pub fn with_tim_client(mut self, generation: u8) -> Self {
        self.set_functionality_supported(
            Functionality::TractorImplementManagementClient,
            generation,
            true,
        );
        self
    }

    #[must_use]
    pub fn with_file_server(mut self, generation: u8) -> Self {
        self.set_functionality_supported(Functionality::FileServer, generation, true);
        self
    }

    #[must_use]
    pub fn with_file_server_client(mut self, generation: u8) -> Self {
        self.set_functionality_supported(Functionality::FileServerClient, generation, true);
        self
    }

    /// All currently supported `(Functionality, generation)` entries.
    #[must_use]
    pub fn supported(&self) -> &[(Functionality, u8)] {
        &self.supported
    }

    // ─── Serialization (PGN 0xFC8E payload) ───────────────────────

    /// Build the PGN_CF_FUNCTIONALITIES payload:
    /// `[0xFF:1][count:1][functionality:1][gen:1][opt_len:1][opts...]×N`.
    ///
    /// AgIsoStack++ and ISO 11783-12 put a fixed `0xFF` byte before the block
    /// count. Option-byte count is canonicalized by omitting trailing zero
    /// option bytes. If the resulting response fits in one CAN frame, the
    /// remaining bytes are `0xFF` padded to the normal 8-byte frame length.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(0xFF);
        data.push(self.supported.len() as u8);
        for (functionality, generation) in &self.supported {
            let mut option_bytes = self.option_bytes_for(*functionality);
            while option_bytes.last() == Some(&0) {
                option_bytes.pop();
            }
            data.push(functionality.as_u8());
            data.push(*generation);
            data.push(option_bytes.len() as u8);
            data.extend(option_bytes);
        }
        if data.len() < 8 {
            data.resize(8, 0xFF);
        }
        data
    }

    /// Decode a PGN_CF_FUNCTIONALITIES payload into per-functionality blocks.
    ///
    /// This parser is intentionally strict: it rejects unknown functionality
    /// codes, generation zero, duplicate entries, over-wide/truncated option
    /// blocks, non-canonical trailing zero option bytes, and trailing bytes
    /// after the advertised block count. A parser that accepted prefix data
    /// would make malformed capability advertisements look valid.
    pub fn decode(data: &[u8]) -> Result<Vec<FunctionalityData>> {
        let Some((&control_byte, rest)) = data.split_first() else {
            return Err(Error::invalid_data(
                "control-function functionalities payload missing leading FF byte",
            ));
        };
        if control_byte != 0xFF {
            return Err(Error::invalid_data(
                "control-function functionalities payload has invalid leading byte",
            ));
        }
        let Some((&count, _)) = rest.split_first() else {
            return Err(Error::invalid_data(
                "control-function functionalities payload missing count",
            ));
        };
        if count == 0 {
            return Err(Error::invalid_data(
                "control-function functionalities payload has zero functionality count",
            ));
        }

        let mut offset = 2usize;
        let mut seen = 0u32;
        let mut decoded = Vec::with_capacity(count as usize);

        for block_index in 0..count {
            if data.len() < offset + 3 {
                return Err(Error::invalid_data(format!(
                    "functionality block {block_index} missing functionality/generation/option-count bytes",
                )));
            }

            let functionality_byte = data[offset];
            let generation = data[offset + 1];
            let option_len = usize::from(data[offset + 2]);
            offset += 3;
            if generation == 0 {
                return Err(Error::invalid_data(format!(
                    "functionality 0x{functionality_byte:02X} has invalid generation zero",
                )));
            }

            let functionality = Functionality::from_u8(functionality_byte).ok_or_else(|| {
                Error::invalid_data(format!(
                    "unknown functionality byte 0x{functionality_byte:02X}",
                ))
            })?;

            let seen_bit = 1u32 << u32::from(functionality.as_u8());
            if (seen & seen_bit) != 0 {
                return Err(Error::invalid_data(format!(
                    "duplicate functionality byte 0x{functionality_byte:02X}",
                )));
            }
            seen |= seen_bit;

            let expected_option_len = functionality.option_byte_len();
            if option_len > expected_option_len {
                return Err(Error::invalid_data(format!(
                    "functionality 0x{functionality_byte:02X} has option length {option_len}, max {expected_option_len}",
                )));
            }
            if data.len() < offset + option_len {
                return Err(Error::invalid_data(format!(
                    "functionality 0x{functionality_byte:02X} option block truncated",
                )));
            }
            if option_len > 0 && data[offset + option_len - 1] == 0 {
                return Err(Error::invalid_data(format!(
                    "functionality 0x{functionality_byte:02X} option block has non-canonical trailing zero",
                )));
            }

            decoded.push(FunctionalityData {
                functionality,
                generation,
                option_bytes: data[offset..offset + option_len].to_vec(),
            });
            offset += option_len;
        }

        if offset != data.len()
            && !(data.len() == 8 && data[offset..].iter().all(|byte| *byte == 0xFF))
        {
            return Err(Error::invalid_data(
                "control-function functionalities payload has trailing bytes",
            ));
        }

        Ok(decoded)
    }

    fn option_bytes_for(&self, f: Functionality) -> Vec<u8> {
        match f {
            Functionality::MinimumControlFunction => vec![self.min_cf_options],
            Functionality::AuxOInputs => vec![self.aux_o_inputs_options],
            Functionality::AuxOFunctions => vec![self.aux_o_functions_options],
            Functionality::AuxNInputs => vec![
                (self.aux_n_inputs_options & 0xFF) as u8,
                ((self.aux_n_inputs_options >> 8) & 0xFF) as u8,
            ],
            Functionality::AuxNFunctions => vec![
                (self.aux_n_functions_options & 0xFF) as u8,
                ((self.aux_n_functions_options >> 8) & 0xFF) as u8,
            ],
            Functionality::TaskControllerGeoServer => vec![self.tc_geo_server_options],
            Functionality::TaskControllerGeoClient => vec![self.tc_geo_client_channels],
            Functionality::TaskControllerSectionControlServer => {
                vec![self.tc_sc_server_booms, self.tc_sc_server_sections]
            }
            Functionality::TaskControllerSectionControlClient => {
                vec![self.tc_sc_client_booms, self.tc_sc_client_sections]
            }
            Functionality::BasicTractorEcuServer => vec![self.basic_tecu_server_options],
            Functionality::BasicTractorEcuImplementClient => vec![self.basic_tecu_client_options],
            Functionality::TractorImplementManagementServer => {
                let mut v = Vec::with_capacity(3 + 8);
                v.extend_from_slice(&self.tim_server_options);
                v.extend_from_slice(&self.tim_server_aux_valves);
                v
            }
            Functionality::TractorImplementManagementClient => {
                let mut v = Vec::with_capacity(3 + 8);
                v.extend_from_slice(&self.tim_client_options);
                v.extend_from_slice(&self.tim_client_aux_valves);
                v
            }
            // UT, TC Basic, FS, FSC: no option bytes.
            _ => Vec::new(),
        }
    }
}

// ─── Bit-twiddling helpers ────────────────────────────────────────────

#[inline]
fn toggle_u8_bits(field: &mut u8, bit: u8, on: bool) {
    if bit == 0 {
        if on {
            *field = 0;
        }
        return;
    }
    if on {
        *field |= bit;
    } else {
        *field &= !bit;
    }
}

#[inline]
fn toggle_u16_bits(field: &mut u16, bit: u16, on: bool) {
    if bit == 0 {
        if on {
            *field = 0;
        }
        return;
    }
    if on {
        *field |= bit;
    } else {
        *field &= !bit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_cf_is_supported_by_default() {
        let f = Functionalities::default();
        assert!(f.is_functionality_supported(Functionality::MinimumControlFunction));
        assert_eq!(
            f.functionality_generation(Functionality::MinimumControlFunction),
            1
        );
    }

    #[test]
    fn fluent_helpers_register_functionalities() {
        let f = Functionalities::new()
            .with_ut_server(4)
            .with_tc_basic_client(4)
            .with_file_server_client(1);
        assert!(f.is_functionality_supported(Functionality::UniversalTerminalServer));
        assert!(f.is_functionality_supported(Functionality::TaskControllerBasicClient));
        assert!(f.is_functionality_supported(Functionality::FileServerClient));
        // MinCF is still there from Default.
        assert!(f.is_functionality_supported(Functionality::MinimumControlFunction));
    }

    #[test]
    fn unset_functionality_removes_it() {
        let mut f = Functionalities::new().with_ut_server(4);
        assert!(f.is_functionality_supported(Functionality::UniversalTerminalServer));
        f.set_functionality_supported(Functionality::UniversalTerminalServer, 4, false);
        assert!(!f.is_functionality_supported(Functionality::UniversalTerminalServer));
    }

    #[test]
    fn tim_option_bits_round_trip() {
        use crate::isobus::tim::{TimOption, TimOptionSet};

        let mut f = Functionalities::new();
        f.set_tim_server_option(
            TractorImplementManagementOptions::FrontPtoEngagementCwIsSupported,
            true,
        );
        f.set_tim_server_option(
            TractorImplementManagementOptions::GuidanceCurvatureIsSupported,
            true,
        );
        assert!(
            f.tim_server_option(TractorImplementManagementOptions::FrontPtoEngagementCwIsSupported)
        );
        assert!(
            f.tim_server_option(TractorImplementManagementOptions::GuidanceCurvatureIsSupported)
        );
        assert!(
            !f.tim_server_option(TractorImplementManagementOptions::RearHitchMotionIsSupported)
        );
        assert_eq!(
            f.tim_server_options,
            TimOptionSet::from_options(&[
                TimOption::FrontPtoEngagementCwIsSupported,
                TimOption::GuidanceCurvatureIsSupported,
            ])
            .as_bytes()
        );
    }

    #[test]
    fn tim_aux_valve_2bit_packing() {
        let mut f = Functionalities::new();
        f.set_tim_server_aux_valve(0, true, false);
        f.set_tim_server_aux_valve(1, false, true);
        f.set_tim_server_aux_valve(31, true, true);
        assert!(f.tim_server_aux_valve_state_supported(0));
        assert!(!f.tim_server_aux_valve_flow_supported(0));
        assert!(!f.tim_server_aux_valve_state_supported(1));
        assert!(f.tim_server_aux_valve_flow_supported(1));
        assert!(f.tim_server_aux_valve_state_supported(31));
        assert!(f.tim_server_aux_valve_flow_supported(31));
    }

    #[test]
    fn serialize_min_cf_only() {
        let f = Functionalities::default();
        let bytes = f.serialize();
        // [fixed=0xFF][count=1][func=0][gen=1][opt_len=0][padding]
        assert_eq!(bytes, vec![0xFF, 1, 0, 1, 0, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn min_cf_option_state_set_and_get() {
        let mut f = Functionalities::new();
        assert!(!f.get_minimum_control_function_option_state(
            MinimumControlFunctionOptions::SupportOfHeartbeatProducer
        ));
        f.set_minimum_control_function_option_state(
            MinimumControlFunctionOptions::SupportOfHeartbeatProducer,
            true,
        );
        f.set_minimum_control_function_option_state(
            MinimumControlFunctionOptions::SupportOfHeartbeatConsumer,
            true,
        );
        assert!(f.get_minimum_control_function_option_state(
            MinimumControlFunctionOptions::SupportOfHeartbeatProducer
        ));
        assert!(f.get_minimum_control_function_option_state(
            MinimumControlFunctionOptions::SupportOfHeartbeatConsumer
        ));
        assert_eq!(f.min_cf_options, 0x0C);

        // Setting NoOptions=true clears all bits.
        f.set_minimum_control_function_option_state(MinimumControlFunctionOptions::NoOptions, true);
        assert_eq!(f.min_cf_options, 0);
    }

    #[test]
    fn aux_n_inputs_option_state_round_trip_u16() {
        let mut f = Functionalities::new();
        f.set_aux_n_inputs_option_state(AuxNOptions::SupportsType9Function, true);
        f.set_aux_n_inputs_option_state(AuxNOptions::SupportsType14Function, true);
        assert!(f.get_aux_n_inputs_option_state(AuxNOptions::SupportsType9Function));
        assert!(f.get_aux_n_inputs_option_state(AuxNOptions::SupportsType14Function));
        assert!(!f.get_aux_n_inputs_option_state(AuxNOptions::SupportsType0Function));
        assert_eq!(f.aux_n_inputs_options, 0x0200 | 0x4000);
        f.set_aux_n_inputs_option_state(AuxNOptions::SupportsType9Function, false);
        assert!(!f.get_aux_n_inputs_option_state(AuxNOptions::SupportsType9Function));
    }

    #[test]
    fn basic_tecu_server_and_client_are_independent() {
        let mut f = Functionalities::new();
        f.set_basic_tractor_ecu_server_option_state(BasicTractorEcuOptions::Class2NoOptions, true);
        f.set_basic_tractor_ecu_implement_client_option_state(
            BasicTractorEcuOptions::FrontHitchOption,
            true,
        );
        assert!(
            f.get_basic_tractor_ecu_server_option_state(BasicTractorEcuOptions::Class2NoOptions)
        );
        assert!(
            !f.get_basic_tractor_ecu_server_option_state(BasicTractorEcuOptions::FrontHitchOption)
        );
        assert!(f.get_basic_tractor_ecu_implement_client_option_state(
            BasicTractorEcuOptions::FrontHitchOption
        ));
    }

    #[test]
    fn no_options_variants_compile() {
        // The NoOptions sentinels exist on every bitfield enum.
        let _ = MinimumControlFunctionOptions::NoOptions;
        let _ = AuxOOptions::NoOptions;
        let _ = AuxNOptions::NoOptions;
        let _ = TaskControllerGeoServerOptions::NoOptions;
    }

    #[test]
    fn serialize_includes_aux_n_two_byte_options() {
        let mut f = Functionalities::new().with_ut_server(4);
        f.set_functionality_supported(Functionality::AuxNInputs, 1, true);
        f.aux_n_inputs_options = 0x0301; // bits 0, 8, 9
        let bytes = f.serialize();
        // Header: 0xFF, count=3 (MinCF, UT server, AuxNInputs)
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 3);
        // Find the AuxNInputs entry by walking blocks.
        // Header: 2 bytes
        // MinCF: func=0, gen=1, opt_len=0 → 3 bytes
        // UT server: func=1, gen=4, opt_len=0 → 3 bytes
        // AuxNInputs: func=5, gen=1, opt_len=2, opts=2 bytes
        assert_eq!(bytes[8], Functionality::AuxNInputs.as_u8());
        assert_eq!(bytes[9], 1); // generation
        assert_eq!(bytes[10], 2); // option byte count
        assert_eq!(bytes[11], 0x01); // low byte
        assert_eq!(bytes[12], 0x03); // high byte
    }

    #[test]
    fn functionality_from_u8_rejects_reserved_values() {
        assert_eq!(
            Functionality::from_u8(0),
            Some(Functionality::MinimumControlFunction)
        );
        assert_eq!(
            Functionality::from_u8(18),
            Some(Functionality::FileServerClient)
        );
        assert_eq!(Functionality::from_u8(19), None);
        assert_eq!(Functionality::from_u8(0xFF), None);
    }

    #[test]
    fn decode_round_trips_variable_option_lengths() {
        let mut f = Functionalities::new()
            .with_tc_geo_client(2)
            .with_tim_server(1)
            .with_file_server(1);
        f.tc_geo_client_channels = 4;
        f.set_tim_server_option(
            TractorImplementManagementOptions::FrontPtoDisengagementIsSupported,
            true,
        );
        f.set_tim_server_option(
            TractorImplementManagementOptions::RearHitchPositionIsSupported,
            true,
        );
        f.set_tim_server_aux_valve(0, true, true);
        f.set_tim_server_aux_valve(31, false, true);

        let bytes = f.serialize();
        let decoded = Functionalities::decode(&bytes).unwrap();
        assert_eq!(decoded.len(), 4);
        assert_eq!(
            decoded[0],
            FunctionalityData {
                functionality: Functionality::MinimumControlFunction,
                generation: 1,
                option_bytes: vec![],
            }
        );
        assert_eq!(
            decoded[1],
            FunctionalityData {
                functionality: Functionality::TaskControllerGeoClient,
                generation: 2,
                option_bytes: vec![4],
            }
        );
        assert_eq!(
            decoded[2].functionality,
            Functionality::TractorImplementManagementServer
        );
        assert_eq!(decoded[2].expected_option_byte_len(), 11);
        assert_eq!(
            decoded[2].option_bytes,
            f.option_bytes_for(decoded[2].functionality)
        );
        assert_eq!(
            decoded[3],
            FunctionalityData {
                functionality: Functionality::FileServer,
                generation: 1,
                option_bytes: vec![],
            }
        );
    }

    #[test]
    fn decode_rejects_malformed_payloads() {
        assert!(Functionalities::decode(&[]).is_err());
        assert!(Functionalities::decode(&[1, 0, 1, 0]).is_err());
        assert!(Functionalities::decode(&[0xFF]).is_err());
        assert!(Functionalities::decode(&[0xFF, 1, 0x63, 1, 1, 0]).is_err());
        assert!(Functionalities::decode(&[0xFF, 1, 0, 1]).is_err());
        assert!(Functionalities::decode(&[0xFF, 1, 0, 1, 0, 0]).is_err());
        assert!(Functionalities::decode(&[0xFF, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF]).is_err());
        assert!(Functionalities::decode(&[0xFF, 2, 0, 1, 0, 0, 2, 1, 0]).is_err());
        assert!(Functionalities::decode(&[0xFF, 1, 0, 1, 1, 0, 0xFF, 0xFF]).is_err());
        assert!(
            Functionalities::decode(&[
                0xFF, 1, 15, 1, 11, // TIM server needs 11 option bytes
                0x01, 0x20, 0x20,
            ])
            .is_err()
        );
    }
}
