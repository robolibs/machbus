//! ISO 11783-6 Virtual Terminal — codecs, state tracker, helpers.
//!
//! Mirrors the C++ `machbus::isobus::vt::*` namespace. The biggest
//! single subsystem in machbus (~3,600 LOC C++).
//!
//! Module layout (mirrors C++):
//!
//! - [`commands`] — VT command function codes (constants) +
//!   [`ActivationCode`].
//! - [`objects`] — `ObjectID`, [`ObjectType`], body structs (Window,
//!   Key, KeyGroup, Macro, AlarmMask), [`VTObject`], [`ObjectPool`],
//!   VT6 features (gestures, 24-bit graphics ctx, scaled bitmap,
//!   colour palette).
//! - [`working_set`] — client-side mask tracking.
//! - [`auxiliary_caps`] — VT v5 Auxiliary Channel Capability discovery.
//! - [`state_tracker`] — client-side mirror of VT state, alarm stack.
//! - [`update_helper`] — deduplicating + batching update wrapper.
//! - [`server_working_set`] — server-side WS tracking *(pending)*.
//! - [`server`] — VT server *(pending)*.
//! - [`client`] — VT client FSM *(pending)*.
//!
//! The C++ `IsoNet&`-coupled wrapper classes are not ported per the
//! universal pattern — see `book/src/reference/behavior-differences.md`.
//!
//! [`ActivationCode`]: commands::ActivationCode
//! [`ObjectType`]: objects::ObjectType
//! [`VTObject`]: objects::VTObject
//! [`ObjectPool`]: objects::ObjectPool

pub mod auxiliary_caps;
pub mod client;
pub mod commands;
pub mod objects;
#[cfg(feature = "default")]
pub mod render;
pub mod server;
pub mod server_working_set;
pub mod state_tracker;
pub mod update_helper;
mod wire;
pub mod working_set;

pub use crate::vt_storage::StoredPoolVersion;
pub use auxiliary_caps::{AuxCapabilities, AuxCapabilityDiscovery, AuxChannelCapability};
pub use client::{
    ClientOutbound, LanguageCode, VTClient, VTClientConfig, VTMacro, VTState, VTVersion,
};
pub use commands::{ActivationCode, KeyActivationCode, cmd};
pub use objects::{
    AlarmMaskBody, AnimationBody, ArchedBarGraphBody, AuxControlDesignatorBody, AuxFunction2Body,
    AuxFunctionBody, AuxInput2Body, AuxInputBody, ButtonBody, ChildRef, ColourMapBody,
    ColourPalette, ColourPaletteBody, ColourPaletteEntry, ContainerBody, DataMaskBody,
    ExtendedInputAttributesBody, ExtendedInputCodePlane, ExternalObjectDefinitionBody,
    ExternalObjectPointerBody, ExternalReferenceNameBody, FillAttributesBody, FontAttributesBody,
    GestureType, GraphicContextBody, GraphicDataBody, GraphicsContextBody, GraphicsContextV6,
    InputAttributesBody, InputBooleanBody, InputListBody, InputNumberBody, InputStringBody,
    KeyBody, KeyGroupBody, LanguageCountryPair, LineAttributesBody, LinearBarGraphBody, MacroBody,
    MacroCommand, MacroRef, MeterBody, NumberVariableBody, ObjectID, ObjectLabelRefBody,
    ObjectLabelRefEntry, ObjectPointerBody, ObjectPool, ObjectType, OutputEllipseBody,
    OutputLineBody, OutputListBody, OutputNumberBody, OutputPolygonBody, OutputRectangleBody,
    OutputStringBody, PictureGraphicBody, PolygonPoint, ScaledBitmapBody, ScaledGraphicBody,
    SoftKeyMaskBody, StringVariableBody, TouchGesture, VTObject, WideCharRange, WindowMaskBody,
    WorkingSetBody, WorkingSetSpecialControlsBody, create_alarm_mask, create_animation,
    create_arched_bar_graph, create_aux_control_designator, create_aux_function,
    create_aux_function2, create_aux_input, create_aux_input2, create_button, create_colour_map,
    create_colour_palette, create_container, create_data_mask, create_extended_input_attributes,
    create_external_object_definition, create_external_object_pointer,
    create_external_reference_name, create_fill_attributes, create_font_attributes,
    create_graphic_context, create_graphic_data, create_graphics_context, create_input_attributes,
    create_input_boolean, create_input_list, create_input_number, create_input_string, create_key,
    create_key_group, create_line_attributes, create_linear_bar_graph, create_macro, create_meter,
    create_number_variable, create_object_label_ref, create_object_pointer, create_output_ellipse,
    create_output_line, create_output_list, create_output_number, create_output_polygon,
    create_output_rectangle, create_output_string, create_picture_graphic, create_scaled_bitmap,
    create_scaled_graphic, create_soft_key_mask, create_string_variable, create_window_mask,
    create_working_set, create_working_set_special_controls,
    picture_graphic_fill_pattern_buffer_is_valid,
};
pub use server::{OutboundFrame, VT_STATUS_INTERVAL_MS, VTServer, VTServerConfig, VTServerState};
pub use server_working_set::{
    GraphicsContextCommand, MaskLockState, ObjectLabelState, ServerRenderEffect, ServerWorkingSet,
};
pub use state_tracker::{AlarmEntry, AlarmPriority, TrackedAttribute, VTClientStateTracker};
pub use update_helper::{UpdateOp, VTClientUpdateHelper};
pub use working_set::WorkingSet;
