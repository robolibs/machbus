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
