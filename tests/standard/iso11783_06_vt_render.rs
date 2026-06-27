//! ISO 11783-6 Virtual Terminal — render / layout / GTUI runtime tests.
//!
//! These are the ISO 11783-6 render-coverage tests for the P0 VT renderer
//! product gap called out in `GAP.md` ("Minimum VT renderer backlog").
//! They exercise the full
//!
//! ```text
//! IOP bytes → ObjectPool → validated → render Scene → GTUI command list
//! ```
//!
//! pipeline and the operator-facing input/focus/edit runtime, and prove
//! that every object type is either rendered, consumed as reference
//! data, modelled as a soft key, explicitly marked parsed-but-not-rendered,
//! or deliberately marked outside the renderer.

#![cfg(test)]

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("iso11783_06_vt_render/render_helpers.rs");
include!("iso11783_06_vt_render/framebuffer_geometry_tests.rs");
include!("iso11783_06_vt_render/generic_attributes_tests.rs");
include!("iso11783_06_vt_render/fill_attribute_pattern_validation_tests.rs");
include!("iso11783_06_vt_render/graphics_context_commands_tests.rs");
include!("iso11783_06_vt_render/graphics_context_text_payload_tests.rs");
include!("iso11783_06_vt_render/graphics_context_canvas_format_tests.rs");
include!("iso11783_06_vt_render/macro_graphics_context_tests.rs");
include!("iso11783_06_vt_render/special_controls_inputs_values_tests.rs");
include!("iso11783_06_vt_render/audio_termination_tests.rs");
include!("iso11783_06_vt_render/bus_message_payload_tests.rs");
include!("iso11783_06_vt_render/graphics_context_response_tests.rs");
include!("iso11783_06_vt_render/mask_change_message_tests.rs");
include!("iso11783_06_vt_render/operator_response_tests.rs");
include!("iso11783_06_vt_render/value_response_tests.rs");
include!("iso11783_06_vt_render/user_layout_window_keygroup_tests.rs");
include!("iso11783_06_vt_render/user_layout_window_availability_tests.rs");
include!("iso11783_06_vt_render/window_mask_clipping_tests.rs");
include!("iso11783_06_vt_render/user_layout_revalidation_tests.rs");
include!("iso11783_06_vt_render/user_layout_response_tests.rs");
include!("iso11783_06_vt_render/runtime_events_user_layout_tests.rs");
include!("iso11783_06_vt_render/input_edit_admission_tests.rs");
include!("iso11783_06_vt_render/focused_hardware_activation_tests.rs");
include!("iso11783_06_vt_render/input_number_decimal_key_validation_tests.rs");
include!("iso11783_06_vt_render/disabled_input_tap_tests.rs");
include!("iso11783_06_vt_render/activation_exclusivity_tests.rs");
include!("iso11783_06_vt_render/animation_change_list_item_tests.rs");
include!("iso11783_06_vt_render/retained_state_input_list_tests.rs");
include!("iso11783_06_vt_render/output_list_external_pointer_tests.rs");
include!("iso11783_06_vt_render/output_list_no_display_rules_tests.rs");
include!("iso11783_06_vt_render/output_list_key_designator_tests.rs");
include!("iso11783_06_vt_render/input_list_no_display_rules_tests.rs");
include!("iso11783_06_vt_render/input_list_selected_item_materialization_tests.rs");
include!("iso11783_06_vt_render/input_list_external_pointer_resolution_tests.rs");
include!("iso11783_06_vt_render/external_object_pointer_default_tests.rs");
include!("iso11783_06_vt_render/bitmap_png_gtui_tests.rs");
include!("iso11783_06_vt_render/graphic_data_png_limit_tests.rs");
