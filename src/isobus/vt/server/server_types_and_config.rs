use alloc::{string::String, vec, vec::Vec};

use super::auxiliary_caps::AuxChannelCapability;
use super::commands::{KeyActivationCode, VT_STRING_VALUE_MAX_LEN, cmd};
use super::objects::{
    ObjectID, ObjectPool, ObjectType, VTObject, change_attribute_targets_one_byte_field,
    change_attribute_targets_two_byte_field, change_soft_key_mask_type_matches,
    external_object_pointer_default_is_valid_for_context, is_enable_disable_object_type,
    is_object_label_graphic_representation_type, is_select_input_object_type,
    is_select_input_open_target_type, is_standard_font_size_for_style, is_standard_font_type,
    key_group_icon_reference_is_valid, key_group_name_reference_is_valid,
    object_pointer_numeric_value_is_valid_for_context, output_list_item_reference_is_valid,
    picture_graphic_fill_pattern_buffer_is_valid, scaled_graphic_scale_type_is_valid,
    scaled_graphic_value_source_is_valid, text_justification_is_valid,
    vt_change_attribute_id_is_supported,
    window_mask_icon_reference_is_valid, window_mask_required_object_types,
    window_mask_text_reference_is_valid,
};
use super::server_working_set::{
    AudioSignalState, AuxInputRuntimeState, AuxRuntimeStyle, GraphicsContextCommand,
    MAX_STORED_VERSIONS, MaskLockState, ObjectLabelState, ServerObjectState, ServerRenderEffect,
    ServerWorkingSet, graphics_context_payload_is_canonical,
    graphics_context_payload_without_padding, graphics_context_subcommand_is_supported,
};
use super::wire::{decode_vt_string_value, vt_string_payload_is_canonical};
use crate::isobus::{AuxFunctionState, AuxFunctionType, AuxNFunction, AuxOFunction};
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_AUX_INPUT_STATUS, PGN_AUX_INPUT_TYPE2, PGN_ECU_TO_VT};
use crate::net::state_machine::StateMachine;
use crate::net::types::Address;

/// VT server state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VTServerState {
    #[default]
    Disconnected,
    WaitForClientStatus,
    SendWorkingSetMaster,
    WaitForPoolUpload,
    Connected,
}

/// VT status broadcast cadence.
pub const VT_STATUS_INTERVAL_MS: u32 = 1000;

/// §4.6.9: "If the VT does not receive this message for a period of 3 s ... it
/// is determined to be an unexpected shutdown of the Working Set Master."
pub const WORKING_SET_MAINTENANCE_TIMEOUT_MS: u32 = 3000;
pub const VT_SERVER_MIN_VERSION: u16 = 3;
pub const VT_SERVER_MAX_VERSION: u16 = 6;

const SELECT_INPUT_ERROR_DISABLED: u8 = 0x01;
const SELECT_INPUT_ERROR_INVALID_OBJECT_ID: u8 = 0x02;
const SELECT_INPUT_ERROR_NOT_ON_ACTIVE_OR_HIDDEN: u8 = 0x04;
const SELECT_INPUT_ERROR_COULD_NOT_COMPLETE: u8 = 0x08;
const SELECT_INPUT_ERROR_INVALID_OPTION: u8 = 0x20;
const GRAPHICS_CONTEXT_ERROR_INVALID_OBJECT_ID: u8 = 0x01;
const GRAPHICS_CONTEXT_ERROR_INVALID_SUBCOMMAND_ID: u8 = 0x02;
const GRAPHICS_CONTEXT_ERROR_INVALID_PARAMETER: u8 = 0x04;
const GRAPHICS_CONTEXT_ERROR_INVALID_RESULTS: u8 = 0x08;

/// ISO 11783-6 Table K.8 WideChar minimum character set ranges for code plane
/// 0. Get Supported WideChars responses must include these ranges when the
/// inquiry intersects code plane 0.
const WIDECHAR_MINIMUM_CODE_PLANE_0: &[(u16, u16)] = &[
    (0x0020, 0x007E),
    (0x00A0, 0x017E),
    (0x02C6, 0x02C7),
    (0x02C9, 0x02C9),
    (0x02D8, 0x02DD),
    (0x037E, 0x037E),
    (0x0384, 0x038A),
    (0x038C, 0x038C),
    (0x038E, 0x03A1),
    (0x03A3, 0x03CE),
    (0x0401, 0x040C),
    (0x040E, 0x044F),
    (0x0451, 0x045C),
    (0x045E, 0x045F),
    (0x20AC, 0x20AC),
];

/// ISO 11783-6 object types this VT server accepts in object pools and reports
/// through the standard Get Supported Objects response.
///
/// The list is numerically sorted as required by the standard response. It
/// deliberately omits Auxiliary Function/Input type 1 objects (29/30), because
/// VTs shall not advertise those in this response. The machbus reserved
/// compatibility object codes 49/50 are also accepted only as local extension
/// records and are not advertised as standard supported objects.
const SUPPORTED_STANDARD_OBJECT_TYPES: &[u8] = &[
    ObjectType::WorkingSet as u8,
    ObjectType::DataMask as u8,
    ObjectType::AlarmMask as u8,
    ObjectType::Container as u8,
    ObjectType::SoftKeyMask as u8,
    ObjectType::Key as u8,
    ObjectType::Button as u8,
    ObjectType::InputBoolean as u8,
    ObjectType::InputString as u8,
    ObjectType::InputNumber as u8,
    ObjectType::InputList as u8,
    ObjectType::OutputString as u8,
    ObjectType::OutputNumber as u8,
    ObjectType::Line as u8,
    ObjectType::Rectangle as u8,
    ObjectType::Ellipse as u8,
    ObjectType::Polygon as u8,
    ObjectType::Meter as u8,
    ObjectType::LinearBarGraph as u8,
    ObjectType::ArchedBarGraph as u8,
    ObjectType::PictureGraphic as u8,
    ObjectType::NumberVariable as u8,
    ObjectType::StringVariable as u8,
    ObjectType::FontAttributes as u8,
    ObjectType::LineAttributes as u8,
    ObjectType::FillAttributes as u8,
    ObjectType::InputAttributes as u8,
    ObjectType::ObjectPointer as u8,
    ObjectType::Macro as u8,
    ObjectType::AuxFunction2 as u8,
    ObjectType::AuxInput2 as u8,
    ObjectType::AuxControlDesig as u8,
    ObjectType::WindowMask as u8,
    ObjectType::KeyGroup as u8,
    ObjectType::GraphicContext as u8,
    ObjectType::OutputList as u8,
    ObjectType::ExtendedInputAttributes as u8,
    ObjectType::ColourMap as u8,
    ObjectType::ObjectLabelRef as u8,
    ObjectType::ExternalObjectDefinition as u8,
    ObjectType::ExternalReferenceName as u8,
    ObjectType::ExternalObjectPointer as u8,
    ObjectType::Animation as u8,
    ObjectType::ColourPalette as u8,
    ObjectType::GraphicData as u8,
    ObjectType::WorkingSetSpecialControls as u8,
    ObjectType::ScaledGraphic as u8,
];

/// Server configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VTServerConfig {
    pub screen_width: u16,
    pub screen_height: u16,
    pub vt_version: u16,
    /// Graphic capability reported by Get Hardware (0=monochrome, 1=16-colour,
    /// 2=256-colour).
    pub graphic_type: u8,
    /// Hardware features bitfield reported by Get Hardware.
    pub hardware_features: u8,
    /// Soft-key key-cell pixel dimensions reported by Get Number Of Soft Keys.
    pub soft_key_x_pixels: u8,
    pub soft_key_y_pixels: u8,
    /// Virtual / physical soft-key counts reported by Get Number Of Soft Keys.
    pub virtual_soft_keys: u8,
    pub physical_soft_keys: u8,
    /// Background colours reported by Get Window Mask Data (0xC4).
    ///
    /// These describe VT-owned user-layout areas, not any particular Working
    /// Set's Data Mask / Soft Key Mask object. A Working Set can use this to
    /// pre-scale or colour-match free-form Window Mask and Key Group content
    /// placed by the operator into the VT's user-layout regions.
    pub user_layout_data_mask_background_colour: u8,
    pub user_layout_soft_key_background_colour: u8,
    /// Small/large font-size and font-style bitfields reported by Get Text Font
    /// Data (`0xFF` = all sizes/styles supported).
    pub small_font_sizes: u8,
    pub large_font_sizes: u8,
    pub font_styles: u8,
    /// Object-pool memory this VT will accept, in bytes, answered in Annex D.3
    /// byte 3 of the Get Memory Response (0 = enough, 1 = not enough, do not
    /// transmit). `0` means "no limit", which is what the server assumed
    /// implicitly before it decoded the requested size at all.
    pub max_pool_bytes: u32,
}

impl Default for VTServerConfig {
    fn default() -> Self {
        Self {
            screen_width: 480,
            screen_height: 480,
            vt_version: 5,
            graphic_type: 2,
            hardware_features: 0,
            soft_key_x_pixels: 60,
            soft_key_y_pixels: 60,
            virtual_soft_keys: 6,
            physical_soft_keys: 0,
            user_layout_data_mask_background_colour: 0,
            user_layout_soft_key_background_colour: 0,
            small_font_sizes: 0xFF,
            large_font_sizes: 0xFF,
            font_styles: 0xFF,
            max_pool_bytes: 0,
        }
    }
}

impl VTServerConfig {
    /// Validate the screen dimensions advertised by the VT server.
    ///
    /// A zero-width or zero-height VT cannot describe a usable display and
    /// should be rejected by stack/persona builders before the server starts
    /// advertising status on the bus.
    pub fn validate(&self) -> Result<()> {
        if self.screen_width == 0 {
            return Err(Error::invalid_data(
                "VTServerConfig: screen_width must be nonzero",
            ));
        }
        if self.screen_height == 0 {
            return Err(Error::invalid_data(
                "VTServerConfig: screen_height must be nonzero",
            ));
        }
        if !(VT_SERVER_MIN_VERSION..=VT_SERVER_MAX_VERSION).contains(&self.vt_version) {
            return Err(Error::invalid_data(
                "VTServerConfig: vt_version must be in 3..=6",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub const fn with_width(mut self, w: u16) -> Self {
        self.screen_width = w;
        self
    }

    #[must_use]
    pub const fn with_height(mut self, h: u16) -> Self {
        self.screen_height = h;
        self
    }

    #[must_use]
    pub const fn with_version(mut self, v: u16) -> Self {
        self.vt_version = v;
        self
    }

    #[must_use]
    pub const fn with_screen(mut self, w: u16, h: u16) -> Self {
        self.screen_width = w;
        self.screen_height = h;
        self
    }
}

/// One frame the server wants to put on the wire. `dest` is `None`
/// for broadcast (status messages).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundFrame {
    pub data: Vec<u8>,
    pub dest: Option<Address>,
}

impl OutboundFrame {
    #[must_use]
    pub fn broadcast(data: Vec<u8>) -> Self {
        Self { data, dest: None }
    }

    #[must_use]
    pub fn to(data: Vec<u8>, dest: Address) -> Self {
        Self {
            data,
            dest: Some(dest),
        }
    }
}

/// ISO 11783-6 Virtual Terminal server.
pub struct VTServer {
    state: StateMachine<VTServerState>,
    clients: Vec<ServerWorkingSet>,
    status_timer_ms: u32,
    /// Annex H.1 byte 7: what this VT is currently busy doing.
    pub(crate) busy_codes: u8,
    vt_version: u16,
    screen_width: u16,
    screen_height: u16,
    config: VTServerConfig,
    active_working_set: Address,
    aux_channels: Vec<AuxChannelCapability>,

    pub on_button_activation: Event<(ObjectID, u8)>,
    pub on_numeric_value_change: Event<(ObjectID, u32)>,
    pub on_string_value_change: Event<(ObjectID, String)>,
    pub on_input_object_selected: Event<(ObjectID, bool, bool)>,
    pub on_soft_key_activation: Event<(ObjectID, u8)>,
    pub on_state_change: Event<VTServerState>,
    pub on_client_connected: Event<Address>,
    pub on_client_disconnected: Event<Address>,
    /// `(old, new)`.
    pub on_active_ws_changed: Event<(Address, Address)>,
}

#[derive(Debug, Clone, Copy)]
struct DecodedAuxInputStatus {
    style: AuxRuntimeStyle,
    function_number: u8,
    r#type: AuxFunctionType,
    state: AuxFunctionState,
    setpoint: u16,
}

/// Why an End of Object Pool was or was not accepted (C.2.5).
///
/// The response used to hardcode "references to missing objects" for every
/// failure, so a Working Set whose pool used an unsupported attribute went
/// hunting for a non-existent bad reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndOfPoolOutcome {
    Accepted,
    /// Nothing was transferred before End of Object Pool.
    NoPool,
    /// The staged bytes did not deserialize, or the merged pool is invalid.
    Malformed,
}

/// What a command handler did, in terms Annex F can encode.
///
/// Handlers return this rather than raw error bits because the bit *positions*
/// differ per command — Change Size (F.19) reports an invalid Object ID in bit
/// 0, Hide/Show (F.3) in bit 1 — and repeating that in every handler is how
/// they drift apart. [`VtResponseShape`] owns the mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandOutcome {
    /// The command was carried out.
    Done,
    /// The referenced object does not exist, or is the wrong type.
    InvalidObject,
    /// Anything else: a malformed payload, an unknown client, a refused change.
    Other,
    /// The command's *second* object reference is bad, where the clause gives
    /// that its own bit — Change Active Mask (F.35) separates an invalid
    /// Working Set (bit 0) from an invalid mask (bit 1), so a Working Set whose
    /// runtime pool update removed the mask can tell the two apart and
    /// re-upload instead of retrying.
    InvalidSecondaryObject,
}

/// Annex F response shape for a VT command the Working Set must wait on.
///
/// F.1: "The VT shall respond to these commands even if no object pool of the
/// originating Working Set is loaded. The originator shall wait for a response
/// before sending another command. Unless stated otherwise, another command can
/// be sent if a response is not received within 1,5 s."
///
/// Every one of these commands used to return no frame at all, so a conformant
/// Working Set blocked 1,5 s per command, retried three times, then declared
/// the VT unresponsive. A burst of Change Numeric Value updates advanced at
/// roughly one command per 1,5 s instead of one per CAN frame.
#[derive(Debug, Clone, Copy)]
pub(crate) struct VtResponseShape {
    /// How many bytes after the function code the response echoes verbatim.
    /// F.1: "any attribute in a response message which also exists in the
    /// command message shall be set to the same value as in the command
    /// message".
    pub echo: usize,
    /// Bit for "invalid Object ID" in this command's error byte. Commands
    /// differ: Change Size (F.19) uses bit 0, Hide/Show (F.3) uses bit 1.
    pub invalid_object_bit: u8,
    /// Bit for "any other error" in this command's error byte.
    pub other_error_bit: u8,
    /// Bit for an invalid *second* object reference, where the clause defines
    /// one. Zero elsewhere, which folds the case back onto `other_error_bit`.
    pub invalid_secondary_object_bit: u8,
}

impl VtResponseShape {
    const fn new(echo: usize, invalid_object_bit: u8, other_error_bit: u8) -> Self {
        Self {
            echo,
            invalid_object_bit,
            other_error_bit,
            invalid_secondary_object_bit: 0,
        }
    }

    const fn with_secondary(mut self, bit: u8) -> Self {
        self.invalid_secondary_object_bit = bit;
        self
    }

    /// The Annex F shape for `function`, or `None` if it has no such response.
    pub(crate) const fn for_command(function: u8) -> Option<Self> {
        use crate::isobus::vt::commands::cmd;
        Some(match function {
            // F.3 / F.5: Object ID + the show/enable flag, error in byte 5.
            cmd::HIDE_SHOW | cmd::ENABLE_DISABLE => Self::new(3, 0x02, 0x10),
            // F.11 / F.13: nothing echoed, error in byte 2.
            cmd::CONTROL_AUDIO_SIGNAL | cmd::SET_AUDIO_VOLUME => Self::new(0, 0x00, 0x10),
            // F.15 / F.17: parent Object ID + child Object ID, error in byte 6.
            cmd::CHANGE_CHILD_LOCATION | cmd::CHANGE_CHILD_POSITION => Self::new(4, 0x01, 0x10),
            // F.19 / F.27 / F.29 / F.31 / F.33 / F.35 / F.53 / F.61:
            // Object ID only, error in byte 4.
            cmd::CHANGE_SIZE
            | cmd::CHANGE_END_POINT
            | cmd::CHANGE_FONT_ATTRIBUTES
            | cmd::CHANGE_LINE_ATTRIBUTES
            | cmd::CHANGE_FILL_ATTRIBUTES
            | cmd::CHANGE_POLYGON_POINT
            | cmd::SELECT_COLOUR_MAP => Self::new(2, 0x01, 0x10),
            // F.35 uses bit 0 for an invalid Working Set and bit 1 for the mask.
            cmd::CHANGE_ACTIVE_MASK => Self::new(2, 0x01, 0x10).with_secondary(0x02),
            // F.21 / F.39 / F.41: Object ID + one parameter byte, error in byte 5.
            cmd::CHANGE_BACKGROUND_COLOUR | cmd::CHANGE_ATTRIBUTE | cmd::CHANGE_PRIORITY => {
                Self::new(3, 0x01, 0x10)
            }
            // F.37 / F.55: Object ID + a second two-byte field, error in byte 6.
            cmd::CHANGE_SOFT_KEY_MASK | cmd::CHANGE_POLYGON_SCALE => Self::new(4, 0x01, 0x10),
            // F.43: Object ID + list index + new Object ID, error in byte 7.
            cmd::CHANGE_LIST_ITEM => Self::new(5, 0x01, 0x10),
            // F.23: Object ID, error in byte 4.
            cmd::CHANGE_NUMERIC_VALUE => Self::new(2, 0x01, 0x10),
            // F.25: byte 2 reserved FF, Object ID in bytes 4-5, error in byte 6.
            cmd::CHANGE_STRING_VALUE => Self::new(4, 0x02, 0x10),
            // F.51: nothing echoed, error in byte 2.
            cmd::CHANGE_OBJECT_LABEL => Self::new(0, 0x01, 0x08),
            // F.45: nothing echoed, error in byte 2.
            cmd::DELETE_OBJECT_POOL => Self::new(0, 0x00, 0x08),
            // F.47 / F.49: one echoed byte, error in byte 3.
            cmd::LOCK_UNLOCK_MASK => Self::new(1, 0x01, 0x04),
            cmd::EXECUTE_MACRO => Self::new(1, 0x01, 0x04),
            // Same shape with a 16-bit Object ID: the extended macro echoes
            // bytes 2-3 and puts the error in byte 4. Missing from this table,
            // the command was carried out and never answered, so a VT-5 working
            // set blocked 1,5 s and retried — re-running the macro through the
            // render runtime on each retry.
            cmd::EXECUTE_EXTENDED_MACRO => Self::new(2, 0x01, 0x04),
            _ => return None,
        })
    }

    /// The error bits for `outcome` in this command's error byte.
    pub(crate) const fn error_bits(self, outcome: CommandOutcome) -> u8 {
        match outcome {
            CommandOutcome::Done => 0,
            CommandOutcome::InvalidObject => self.invalid_object_bit,
            CommandOutcome::Other => self.other_error_bit,
            CommandOutcome::InvalidSecondaryObject => {
                if self.invalid_secondary_object_bit == 0 {
                    self.other_error_bit
                } else {
                    self.invalid_secondary_object_bit
                }
            }
        }
    }

    /// Build the response frame for `msg` carrying `error_bits`.
    pub(crate) fn response(self, msg: &[u8], error_bits: u8) -> [u8; 8] {
        let mut out = [0xFFu8; 8];
        out[0] = msg[0];
        for (i, byte) in out.iter_mut().enumerate().take(self.echo + 1).skip(1) {
            *byte = msg.get(i).copied().unwrap_or(0xFF);
        }
        out[self.echo + 1] = error_bits;
        out
    }
}
