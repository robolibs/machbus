use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use super::commands::{ActivationCode, VT_STRING_VALUE_MAX_LEN, cmd};
use super::objects::{ObjectID, ObjectPool};
use super::wire::{decode_vt_string_value, vt_string_payload_is_canonical};
use super::working_set::WorkingSet;
use crate::net::constants::NULL_ADDRESS;
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::{
    PGN_ECU_TO_VT, PGN_LANGUAGE_COMMAND, PGN_VT_TO_ECU, PGN_WORKING_SET_MASTER,
};
use crate::net::state_machine::StateMachine;
use crate::net::types::{Address, Pgn};

// ─── Public types ─────────────────────────────────────────────────────

/// VT client connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VTState {
    #[default]
    Disconnected,
    WaitForVTStatus,
    SendWorkingSetMaster,
    SendGetMemory,
    WaitForMemory,
    UploadPool,
    WaitForPoolStore,
    WaitForEndOfPool,
    /// Language mismatch — reload pool with correct language.
    ReloadPool,
    Connected,
}

/// VT version preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum VTVersion {
    Version3 = 3,
    #[default]
    Version4 = 4,
    Version5 = 5,
    Version6 = 6,
}

impl VTVersion {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Two-letter ISO 639-1 language code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageCode {
    pub code: [u8; 2],
}

impl Default for LanguageCode {
    fn default() -> Self {
        Self { code: *b"en" }
    }
}

impl LanguageCode {
    /// Parse a canonical two-letter language code.
    pub fn try_parse(s: &str) -> Result<Self> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 || !is_canonical_language_code([bytes[0], bytes[1]]) {
            return Err(Error::invalid_data(
                "language code must be exactly two ASCII letters",
            ));
        }
        Ok(Self {
            code: [bytes[0], bytes[1]],
        })
    }

    /// Parse a canonical two-letter language code, falling back to the
    /// default when user input is invalid.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        Self::try_parse(s).unwrap_or_default()
    }
}

impl core::fmt::Display for LanguageCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.code))
    }
}

/// VT client configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VTClientConfig {
    pub timeout_ms: u32,
    pub preferred_version: VTVersion,
}

impl Default for VTClientConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 6000,
            preferred_version: VTVersion::Version4,
        }
    }
}

impl VTClientConfig {
    #[must_use]
    pub const fn with_timeout(mut self, ms: u32) -> Self {
        self.timeout_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_version(mut self, v: VTVersion) -> Self {
        self.preferred_version = v;
        self
    }
}

/// Frame the client wants the caller to put on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientOutbound {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    /// `None` for broadcast; `Some(addr)` to address the VT specifically.
    pub dest: Option<Address>,
}

impl ClientOutbound {
    #[must_use]
    pub fn broadcast(pgn: Pgn, data: Vec<u8>) -> Self {
        Self {
            pgn,
            data,
            dest: None,
        }
    }

    #[must_use]
    pub fn to(pgn: Pgn, data: Vec<u8>, dest: Address) -> Self {
        Self {
            pgn,
            data,
            dest: Some(dest),
        }
    }
}

/// Macro definition local to the client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VTMacro {
    pub macro_id: ObjectID,
    /// Each entry is one VT command byte sequence.
    pub commands: Vec<Vec<u8>>,
}

// ─── VTClient ─────────────────────────────────────────────────────────

/// VT client. See module-level doc for the pump-style API contract.
pub struct VTClient {
    config: VTClientConfig,
    state: StateMachine<VTState>,
    pool: ObjectPool,
    working_set: WorkingSet,
    timer_ms: u32,
    pending_end_of_pool_delay_ms: u32,
    vt_address: Address,
    vt_version: u16,
    extended_version_label: String,
    vt_supports_extended_versions: bool,
    unsupported_functions: Vec<u8>,
    is_active_ws: bool,
    /// Address of *this* client's control function — used to detect
    /// whether the active-WS-master address in VT_STATUS is us.
    /// `None` until the user calls [`Self::set_self_address`]; while
    /// `None` the client never claims active-WS status.
    self_address: Option<Address>,

    current_language: LanguageCode,
    vt_language: LanguageCode,
    auto_reload_on_language_change: bool,

    macros: Vec<VTMacro>,

    pub on_soft_key: Event<(ObjectID, ActivationCode)>,
    pub on_button: Event<(ObjectID, ActivationCode)>,
    /// Full Soft Key Activation: `(key_object_id, parent_mask_id, key_number,
    /// activation)`. Carries the parent data/alarm-mask and key number the
    /// terse [`on_soft_key`](Self::on_soft_key) event omits.
    pub on_soft_key_detailed: Event<(ObjectID, ObjectID, u8, ActivationCode)>,
    /// Full Button Activation: `(button_object_id, parent_mask_id, button_number,
    /// activation)`.
    pub on_button_detailed: Event<(ObjectID, ObjectID, u8, ActivationCode)>,
    pub on_numeric_value_change: Event<(ObjectID, u32)>,
    pub on_string_value_change: Event<(ObjectID, String)>,
    /// VT Pointing Event: `(x_px, y_px, touch_state)`.
    pub on_pointing_event: Event<(u16, u16, ActivationCode)>,
    /// VT Select Input Object: `(object_id, selected, open_for_input)`.
    pub on_select_input_object: Event<(ObjectID, bool, bool)>,
    /// Select Input Object response: `(object_id, response_code, error_bits)`.
    pub on_select_input_object_response: Event<(ObjectID, u8, u8)>,
    /// Graphics Context response: `(graphics_context_id, subcommand, error_bits)`.
    pub on_graphics_context_response: Event<(ObjectID, u8, u8)>,
    /// VT ESC: `(aborted_input_object_id, error_code)`.
    pub on_vt_esc: Event<(ObjectID, u8)>,
    /// VT ESC with optional VT v6 transfer sequence number:
    /// `(aborted_input_object_id, error_code, tan)`.
    pub on_vt_esc_detailed: Event<(ObjectID, u8, Option<u8>)>,
    pub on_state_change: Event<VTState>,
    pub on_macro_executed: Event<ObjectID>,
    pub on_pool_error: Event<u8>,
    pub on_versions_received: Event<Vec<String>>,
    /// `(success, error_code)`.
    pub on_store_version_response: Event<(bool, u8)>,
    /// `(success, error_code)`.
    pub on_load_version_response: Event<(bool, u8)>,
    pub on_extended_versions_received: Event<Vec<String>>,
    pub on_extended_store_response: Event<(bool, u8)>,
    pub on_extended_load_response: Event<(bool, u8)>,
    pub on_unsupported_function: Event<u8>,
    pub on_active_ws_status: Event<bool>,
    /// `(old_lang, new_lang)`.
    pub on_language_change: Event<(LanguageCode, LanguageCode)>,
}

impl VTClient {
    #[must_use]
    pub fn new(config: VTClientConfig) -> Self {
        Self {
            config,
            state: StateMachine::new(VTState::Disconnected),
            pool: ObjectPool::default(),
            working_set: WorkingSet::default(),
            timer_ms: 0,
            pending_end_of_pool_delay_ms: 0,
            vt_address: NULL_ADDRESS,
            vt_version: 0,
            extended_version_label: String::new(),
            vt_supports_extended_versions: false,
            unsupported_functions: Vec::new(),
            is_active_ws: false,
            self_address: None,
            current_language: LanguageCode::default(),
            vt_language: LanguageCode::default(),
            auto_reload_on_language_change: true,
            macros: Vec::new(),
            on_soft_key: Event::new(),
            on_button: Event::new(),
            on_soft_key_detailed: Event::new(),
            on_button_detailed: Event::new(),
            on_pointing_event: Event::new(),
            on_select_input_object: Event::new(),
            on_select_input_object_response: Event::new(),
            on_graphics_context_response: Event::new(),
            on_vt_esc: Event::new(),
            on_vt_esc_detailed: Event::new(),
            on_numeric_value_change: Event::new(),
            on_string_value_change: Event::new(),
            on_state_change: Event::new(),
            on_macro_executed: Event::new(),
            on_pool_error: Event::new(),
            on_versions_received: Event::new(),
            on_store_version_response: Event::new(),
            on_load_version_response: Event::new(),
            on_extended_versions_received: Event::new(),
            on_extended_store_response: Event::new(),
            on_extended_load_response: Event::new(),
            on_unsupported_function: Event::new(),
            on_active_ws_status: Event::new(),
            on_language_change: Event::new(),
        }
    }

    pub fn set_object_pool(&mut self, pool: ObjectPool) {
        self.pool = pool;
    }

    pub fn set_working_set(&mut self, ws: WorkingSet) {
        self.working_set = ws;
    }

    /// Tell the client what address its control function holds. The
    /// C++ obtains this via `cf_->cf().address()` — we accept it
    /// directly since we don't carry a CF reference.
    pub fn set_self_address(&mut self, addr: Address) {
        self.self_address = Some(addr);
    }

    // ─── Connect / disconnect ─────────────────────────────────────────

    pub fn connect(&mut self) -> Result<()> {
        if self.pool.is_empty() {
            return Err(Error::invalid_state("object pool is empty"));
        }
        let _ = serialize_pool_for_vt_transfer(&self.pool)?;
        self.clear_vt_session_binding();
        self.transition(VTState::WaitForVTStatus);
        self.timer_ms = 0;
        self.pending_end_of_pool_delay_ms = 0;
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<()> {
        self.clear_vt_session_binding();
        self.transition(VTState::Disconnected);
        self.pending_end_of_pool_delay_ms = 0;
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> VTState {
        self.state.state()
    }

    #[inline]
    #[must_use]
    pub const fn is_active_ws(&self) -> bool {
        self.is_active_ws
    }

    #[inline]
    #[must_use]
    pub const fn vt_address(&self) -> Address {
        self.vt_address
    }

    #[inline]
    #[must_use]
    pub const fn vt_version_value(&self) -> u16 {
        self.vt_version
    }

    pub fn set_vt_version_preference(&mut self, version: VTVersion) {
        self.vt_version = version.as_u8() as u16;
    }

    // ─── Language ─────────────────────────────────────────────────────

    pub fn try_set_language(&mut self, lang: LanguageCode) -> Result<()> {
        if !is_canonical_language_code(lang.code) {
            return Err(Error::invalid_data(
                "language code must be exactly two ASCII letters",
            ));
        }
        self.current_language = lang;
        Ok(())
    }

    pub fn set_language(&mut self, lang: LanguageCode) {
        let _ = self.try_set_language(lang);
    }

    pub fn try_set_language_str(&mut self, s: &str) -> Result<()> {
        self.try_set_language(LanguageCode::try_parse(s)?)
    }

    pub fn set_language_str(&mut self, s: &str) {
        let _ = self.try_set_language_str(s);
    }

    #[inline]
    #[must_use]
    pub const fn language(&self) -> LanguageCode {
        self.current_language
    }

    #[inline]
    #[must_use]
    pub const fn vt_language(&self) -> LanguageCode {
        self.vt_language
    }

    pub fn set_auto_reload_on_language_change(&mut self, enable: bool) {
        self.auto_reload_on_language_change = enable;
    }

    #[inline]
    #[must_use]
    pub const fn auto_reload_on_language_change(&self) -> bool {
        self.auto_reload_on_language_change
    }

    // ─── Pool dynamic swap ────────────────────────────────────────────

    /// Swap the pool while connected and trigger a re-upload. Returns
    /// the optional store-current-pool outbound (when `store_old &&
    /// !old_label.is_empty()`) — the actual upload frames will then
    /// arrive from [`Self::update`].
    pub fn swap_pool(
        &mut self,
        new_pool: ObjectPool,
        store_old: bool,
        old_label: &str,
    ) -> Result<Option<ClientOutbound>> {
        if self.state() != VTState::Connected {
            return Err(Error::not_connected());
        }
        if new_pool.is_empty() {
            return Err(Error::invalid_state("new pool is empty"));
        }
        let _ = serialize_pool_for_vt_transfer(&new_pool)?;
        let store_outbound = if store_old && !old_label.is_empty() {
            self.store_version(old_label).ok()
        } else {
            None
        };
        self.pool = new_pool;
        self.transition(VTState::UploadPool);
        self.timer_ms = 0;
        Ok(store_outbound)
    }

    pub fn quick_swap_to_version(&mut self, version_label: &str) -> Result<ClientOutbound> {
        if self.state() != VTState::Connected {
            return Err(Error::not_connected());
        }
        self.load_version(version_label)
    }

    // ─── VT commands (must be Connected) ──────────────────────────────

    pub fn hide_show(&self, id: impl Into<ObjectID>, visible: bool) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let id = id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::HIDE_SHOW;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = u8::from(visible);
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn enable_disable(&self, id: impl Into<ObjectID>, enabled: bool) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let id = id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::ENABLE_DISABLE;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = u8::from(enabled);
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn select_input_object(
        &self,
        id: impl Into<ObjectID>,
        option: u8,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let id = id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_INPUT_OBJECT_COMMAND;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = option;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn esc_input(&self) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::ESC_INPUT;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn vt_esc_response(&self, id: impl Into<ObjectID>) -> Result<ClientOutbound> {
        self.vt_esc_response_with_error(id, 0xFF)
    }

    pub fn vt_esc_response_with_error(
        &self,
        id: impl Into<ObjectID>,
        error_code: u8,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let id = id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_ESC;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = error_code;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn vt_esc_response_with_transfer_sequence_number(
        &self,
        id: impl Into<ObjectID>,
        transfer_sequence_number: u8,
    ) -> Result<ClientOutbound> {
        self.vt_esc_response_with_error_and_transfer_sequence_number(
            id,
            0xFF,
            transfer_sequence_number,
        )
    }

    pub fn vt_esc_response_with_error_and_transfer_sequence_number(
        &self,
        id: impl Into<ObjectID>,
        error_code: u8,
        transfer_sequence_number: u8,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        if transfer_sequence_number > 0x0F {
            return Err(Error::invalid_data(
                "VT ESC transfer sequence number exceeds 4-bit field",
            ));
        }
        let id = id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_ESC;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = error_code;
        data[7] = (transfer_sequence_number << 4) | 0x0F;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn change_numeric_value(
        &self,
        id: impl Into<ObjectID>,
        value: u32,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let id = id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_NUMERIC_VALUE;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = 0xFF;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn change_string_value(
        &self,
        id: impl Into<ObjectID>,
        value: &str,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        if value.len() > VT_STRING_VALUE_MAX_LEN {
            return Err(Error::invalid_data(
                "VT string-value payload exceeds u16 length field",
            ));
        }
        let id = id.into();
        let mut data = Vec::with_capacity(5 + value.len());
        data.push(cmd::CHANGE_STRING_VALUE);
        data.extend_from_slice(&id.to_le_bytes());
        data.extend_from_slice(&(value.len() as u16).to_le_bytes());
        data.extend_from_slice(value.as_bytes());
        while data.len() < 8 {
            data.push(0xFF);
        }
        Ok(ClientOutbound::to(PGN_ECU_TO_VT, data, self.vt_address))
    }

    pub fn change_active_mask(
        &self,
        working_set_id: ObjectID,
        mask_id: ObjectID,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_ACTIVE_MASK;
        data[1..3].copy_from_slice(&working_set_id.to_le_bytes());
        data[3..5].copy_from_slice(&mask_id.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn graphics_context_draw_text(
        &self,
        graphics_context_id: ObjectID,
        transparent: bool,
        value: &str,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        if value.is_empty() {
            return Err(Error::invalid_data(
                "VT graphics-context DrawText requires a non-empty string",
            ));
        }
        if value.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "VT graphics-context DrawText length exceeds u8 length field",
            ));
        }

        let mut data = Vec::with_capacity(6 + value.len());
        data.push(cmd::GRAPHICS_CONTEXT);
        data.extend_from_slice(&graphics_context_id.to_le_bytes());
        data.push(0x0D); // GraphicsContextSubCommandID::DrawText.
        data.push(u8::from(transparent));
        data.push(value.len() as u8);
        data.extend_from_slice(value.as_bytes());
        while data.len() < 8 {
            data.push(0xFF);
        }
        Ok(ClientOutbound::to(PGN_ECU_TO_VT, data, self.vt_address))
    }

    pub fn change_soft_key_mask(
        &self,
        data_mask_id: ObjectID,
        sk_mask_id: ObjectID,
    ) -> Result<ClientOutbound> {
        self.change_soft_key_mask_for_type(1, data_mask_id, sk_mask_id)
    }

    pub fn change_alarm_soft_key_mask(
        &self,
        alarm_mask_id: ObjectID,
        sk_mask_id: ObjectID,
    ) -> Result<ClientOutbound> {
        self.change_soft_key_mask_for_type(2, alarm_mask_id, sk_mask_id)
    }

    fn change_soft_key_mask_for_type(
        &self,
        mask_type: u8,
        mask_id: ObjectID,
        sk_mask_id: ObjectID,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_SOFT_KEY_MASK;
        data[1] = mask_type;
        data[2..4].copy_from_slice(&mask_id.to_le_bytes());
        data[4..6].copy_from_slice(&sk_mask_id.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn change_attribute(
        &self,
        id: ObjectID,
        attribute_id: u8,
        value: u32,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_ATTRIBUTE;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = attribute_id;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn change_size(&self, id: ObjectID, width: u16, height: u16) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_SIZE;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3..5].copy_from_slice(&width.to_le_bytes());
        data[5..7].copy_from_slice(&height.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Change Font Attributes (0xAA): `[id][colour][size][type][style]`.
    pub fn change_font_attributes(
        &self,
        id: ObjectID,
        colour: u8,
        size: u8,
        font_type: u8,
        style: u8,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_FONT_ATTRIBUTES;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = colour;
        data[4] = size;
        data[5] = font_type;
        data[6] = style;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Change Line Attributes (0xAB): `[id][colour][width][line-art u16]`.
    pub fn change_line_attributes(
        &self,
        id: ObjectID,
        colour: u8,
        width: u8,
        line_art: u16,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_LINE_ATTRIBUTES;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = colour;
        data[4] = width;
        data[5..7].copy_from_slice(&line_art.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Change Fill Attributes (0xAC): `[id][fill-type][colour][pattern-obj u16]`.
    pub fn change_fill_attributes(
        &self,
        id: ObjectID,
        fill_type: u8,
        colour: u8,
        fill_pattern: ObjectID,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_FILL_ATTRIBUTES;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = fill_type;
        data[4] = colour;
        data[5..7].copy_from_slice(&fill_pattern.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Change End Point (0xA9): `[id][width u16][height u16][line-direction]`.
    pub fn change_end_point(
        &self,
        id: ObjectID,
        width: u16,
        height: u16,
        line_direction: u8,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_END_POINT;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3..5].copy_from_slice(&width.to_le_bytes());
        data[5..7].copy_from_slice(&height.to_le_bytes());
        data[7] = line_direction;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Change Priority (0xB0): `[alarm-mask id][priority]`.
    pub fn change_priority(&self, id: ObjectID, priority: u8) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_PRIORITY;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = priority;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Change Polygon Point (0xB6): `[id][point-index][new-X u16][new-Y u16]`.
    pub fn change_polygon_point(
        &self,
        id: ObjectID,
        point_index: u8,
        new_x: u16,
        new_y: u16,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_POLYGON_POINT;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = point_index;
        data[4..6].copy_from_slice(&new_x.to_le_bytes());
        data[6..8].copy_from_slice(&new_y.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Change Polygon Scale (0xB7): `[id][width u16][height u16]`.
    pub fn change_polygon_scale(
        &self,
        id: ObjectID,
        width: u16,
        height: u16,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_POLYGON_SCALE;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3..5].copy_from_slice(&width.to_le_bytes());
        data[5..7].copy_from_slice(&height.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Change Object Label (0xB5): `[id][label-string id u16][font-type]
    /// [graphic-designator id u16]`. Object IDs of `0xFFFF` mean "no object".
    pub fn change_object_label(
        &self,
        id: ObjectID,
        label_string: ObjectID,
        font_type: u8,
        graphic_designator: ObjectID,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_OBJECT_LABEL;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3..5].copy_from_slice(&label_string.to_le_bytes());
        data[5] = font_type;
        data[6..8].copy_from_slice(&graphic_designator.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Select Colour Map (0xBA): `[colour-map object id]`.
    pub fn select_colour_map(&self, id: ObjectID) -> Result<ClientOutbound> {
        self.select_colour_map_or_palette(id)
    }

    /// Select Colour Palette (0xBA): `[colour-palette object id]`.
    pub fn select_colour_palette(&self, id: ObjectID) -> Result<ClientOutbound> {
        self.select_colour_map_or_palette(id)
    }

    fn select_colour_map_or_palette(&self, id: ObjectID) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_COLOUR_MAP;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Get Attribute Value (0xB9): `[object id u16][attribute id]`. The VT
    /// replies with the current value of that object attribute.
    pub fn get_attribute_value(&self, id: ObjectID, attribute_id: u8) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_ATTRIBUTE_VALUE;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = attribute_id;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Build a parameterless VT technical-data request (`[code][FF×7]`).
    fn build_get_request(&self, function: u8) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = function;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Identify VT (0xBB): asks the VT to show its identification screen.
    pub fn identify_vt(&self) -> Result<ClientOutbound> {
        self.build_get_request(cmd::IDENTIFY_VT)
    }

    /// Get Number Of Soft Keys (0xC2).
    pub fn get_number_of_soft_keys(&self) -> Result<ClientOutbound> {
        self.build_get_request(cmd::GET_NUMBER_SOFTKEYS)
    }

    /// Get Text Font Data (0xC3).
    pub fn get_text_font_data(&self) -> Result<ClientOutbound> {
        self.build_get_request(cmd::GET_TEXT_FONT_DATA)
    }

    /// Get Hardware (0xC7).
    pub fn get_hardware(&self) -> Result<ClientOutbound> {
        self.build_get_request(cmd::GET_HARDWARE)
    }

    /// Get Supported Widechars (0xC1) for the full ISO code plane 0 range.
    pub fn get_supported_widechars(&self) -> Result<ClientOutbound> {
        self.get_supported_widechars_range(0, 0x0000, u16::MAX)
    }

    /// Get Supported Widechars (0xC1) for a specific code-plane/range query:
    /// `[code][plane][first u16][last u16][FF][FF]`.
    pub fn get_supported_widechars_range(
        &self,
        code_plane: u8,
        first: u16,
        last: u16,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_SUPPORTED_WIDECHARS;
        data[1] = code_plane;
        data[2..4].copy_from_slice(&first.to_le_bytes());
        data[4..6].copy_from_slice(&last.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    /// Get Window Mask Data (0xC4).
    pub fn get_window_mask_data(&self) -> Result<ClientOutbound> {
        self.build_get_request(cmd::GET_WINDOW_MASK_DATA)
    }

    /// Get Supported Objects (0xC5).
    pub fn get_supported_objects(&self) -> Result<ClientOutbound> {
        self.build_get_request(cmd::GET_SUPPORTED_OBJECTS)
    }

    pub fn change_child_location(
        &self,
        parent_id: ObjectID,
        child_id: ObjectID,
        relative_x: u8,
        relative_y: u8,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_CHILD_LOCATION;
        data[1..3].copy_from_slice(&parent_id.to_le_bytes());
        data[3..5].copy_from_slice(&child_id.to_le_bytes());
        data[5] = relative_x;
        data[6] = relative_y;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn change_child_position(
        &self,
        parent_id: ObjectID,
        child_id: ObjectID,
        x: u16,
        y: u16,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = Vec::with_capacity(9);
        data.push(cmd::CHANGE_CHILD_POSITION);
        data.extend_from_slice(&parent_id.to_le_bytes());
        data.extend_from_slice(&child_id.to_le_bytes());
        data.extend_from_slice(&x.to_le_bytes());
        data.extend_from_slice(&y.to_le_bytes());
        Ok(ClientOutbound::to(PGN_ECU_TO_VT, data, self.vt_address))
    }

    pub fn change_background_colour(&self, id: ObjectID, colour: u8) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_BACKGROUND_COLOUR;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = colour;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn change_list_item(
        &self,
        list_id: ObjectID,
        index: u8,
        new_item_id: ObjectID,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_LIST_ITEM;
        data[1..3].copy_from_slice(&list_id.to_le_bytes());
        data[3] = index;
        data[4..6].copy_from_slice(&new_item_id.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn lock_unlock_mask(
        &self,
        mask_id: ObjectID,
        lock: bool,
        timeout_ms: u16,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::LOCK_UNLOCK_MASK;
        data[1] = if lock { 0x01 } else { 0x00 };
        data[2..4].copy_from_slice(&mask_id.to_le_bytes());
        data[4..6].copy_from_slice(&timeout_ms.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn control_audio_signal(
        &self,
        activations: u8,
        frequency_hz: u16,
        duration_ms: u16,
        off_time_ms: u16,
    ) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CONTROL_AUDIO_SIGNAL;
        data[1] = activations;
        data[2..4].copy_from_slice(&frequency_hz.to_le_bytes());
        data[4..6].copy_from_slice(&duration_ms.to_le_bytes());
        data[6..8].copy_from_slice(&off_time_ms.to_le_bytes());
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn set_audio_volume(&self, volume_percent: u8) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        if volume_percent > 100 {
            return Err(Error::invalid_state("audio volume must be 0..=100"));
        }
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SET_AUDIO_VOLUME;
        data[1] = volume_percent;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    // ─── Macros ───────────────────────────────────────────────────────

    pub fn execute_macro(&mut self, macro_id: ObjectID) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::EXECUTE_MACRO;
        data[1..3].copy_from_slice(&macro_id.to_le_bytes());
        self.on_macro_executed.emit(&macro_id);
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn register_macro(&mut self, m: VTMacro) {
        if let Some(existing) = self.macros.iter_mut().find(|x| x.macro_id == m.macro_id) {
            *existing = m;
        } else {
            self.macros.push(m);
        }
    }

    #[must_use]
    pub fn get_macro(&self, id: ObjectID) -> Option<&VTMacro> {
        self.macros.iter().find(|m| m.macro_id == id)
    }

    #[must_use]
    pub fn macros(&self) -> &[VTMacro] {
        &self.macros
    }

    #[must_use]
    pub fn unsupported_functions(&self) -> &[u8] {
        &self.unsupported_functions
    }

    // ─── Pool versioning (classic + extended) ─────────────────────────

    pub fn store_version(&self, label: &str) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        validate_classic_version_label(label)?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::STORE_VERSION;
        write_label_classic(&mut data[1..8], label);
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn load_version(&mut self, label: &str) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        validate_classic_version_label(label)?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::LOAD_VERSION;
        write_label_classic(&mut data[1..8], label);
        self.transition(VTState::WaitForEndOfPool);
        self.timer_ms = 0;
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn delete_version(&self, label: &str) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        validate_classic_version_label(label)?;
        let mut data = [0xFFu8; 8];
        data[0] = cmd::DELETE_VERSION;
        write_label_classic(&mut data[1..8], label);
        Ok(ClientOutbound::to(
            PGN_ECU_TO_VT,
            data.to_vec(),
            self.vt_address,
        ))
    }

    pub fn get_versions(&self) -> ClientOutbound {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_VERSIONS;
        ClientOutbound::to(PGN_ECU_TO_VT, data.to_vec(), self.vt_address)
    }

    /// Legacy alias for [`Self::delete_version`].
    pub fn delete_pool(&self, label: &str) -> Result<ClientOutbound> {
        self.delete_version(label)
    }

    pub fn request_extended_version_label(&self) -> ClientOutbound {
        let mut data = vec![
            cmd::EXTENDED_GET_VERSIONS,
            cmd::EXTENDED_VERSION_SUBFUNCTION,
        ];
        while data.len() < 8 {
            data.push(0xFF);
        }
        ClientOutbound::to(PGN_ECU_TO_VT, data, self.vt_address)
    }

    pub fn send_extended_store_version(&mut self, label: &str) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        validate_extended_version_label(label)?;
        let mut data = Vec::with_capacity(2 + cmd::EXTENDED_VERSION_LABEL_SIZE);
        data.push(cmd::EXTENDED_STORE_VERSION);
        data.push(cmd::EXTENDED_VERSION_SUBFUNCTION);
        push_label_extended(&mut data, label);
        while data.len() < 8 {
            data.push(0xFF);
        }
        self.extended_version_label = label.to_string();
        Ok(ClientOutbound::to(PGN_ECU_TO_VT, data, self.vt_address))
    }

    pub fn send_extended_load_version(&mut self, label: &str) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        validate_extended_version_label(label)?;
        let mut data = Vec::with_capacity(2 + cmd::EXTENDED_VERSION_LABEL_SIZE);
        data.push(cmd::EXTENDED_LOAD_VERSION);
        data.push(cmd::EXTENDED_VERSION_SUBFUNCTION);
        push_label_extended(&mut data, label);
        while data.len() < 8 {
            data.push(0xFF);
        }
        self.transition(VTState::WaitForEndOfPool);
        self.timer_ms = 0;
        Ok(ClientOutbound::to(PGN_ECU_TO_VT, data, self.vt_address))
    }

    pub fn send_extended_delete_version(&self, label: &str) -> Result<ClientOutbound> {
        self.must_be_connected()?;
        validate_extended_version_label(label)?;
        let mut data = Vec::with_capacity(2 + cmd::EXTENDED_VERSION_LABEL_SIZE);
        data.push(cmd::EXTENDED_DELETE_VERSION);
        data.push(cmd::EXTENDED_VERSION_SUBFUNCTION);
        push_label_extended(&mut data, label);
        while data.len() < 8 {
            data.push(0xFF);
        }
        Ok(ClientOutbound::to(PGN_ECU_TO_VT, data, self.vt_address))
    }

    #[inline]
    #[must_use]
    pub const fn vt_supports_extended_versions(&self) -> bool {
        self.vt_supports_extended_versions
    }

    #[inline]
    #[must_use]
    pub fn extended_version_label(&self) -> &str {
        &self.extended_version_label
    }

    // ─── Update FSM ──────────────────────────────────────────────────

    /// Advance the connect FSM. Returns the outbound frames (in
    /// emission order) the caller should ship.
    pub fn update(&mut self, elapsed_ms: u32) -> Vec<ClientOutbound> {
        self.timer_ms = self.timer_ms.saturating_add(elapsed_ms);
        let mut out = Vec::new();
        match self.state() {
            VTState::WaitForVTStatus => {
                if self.timer_ms >= self.config.timeout_ms {
                    self.transition(VTState::Disconnected);
                }
            }
            VTState::SendWorkingSetMaster => {
                let mut data = [0xFFu8; 8];
                data[0] = 1; // Number of members.
                out.push(ClientOutbound::broadcast(
                    PGN_WORKING_SET_MASTER,
                    data.to_vec(),
                ));
                self.transition(VTState::SendGetMemory);
                self.timer_ms = 0;
            }
            VTState::SendGetMemory => {
                let Ok((_, pool_size)) = serialize_pool_for_vt_transfer(&self.pool) else {
                    self.transition(VTState::Disconnected);
                    self.timer_ms = 0;
                    return out;
                };
                let mut data = [0xFFu8; 8];
                data[0] = cmd::GET_MEMORY;
                data[1..5].copy_from_slice(&pool_size.to_le_bytes());
                out.push(ClientOutbound::to(
                    PGN_ECU_TO_VT,
                    data.to_vec(),
                    self.vt_address,
                ));
                self.transition(VTState::WaitForMemory);
                self.timer_ms = 0;
            }
            VTState::UploadPool => {
                let Ok((serialized, _)) = serialize_pool_for_vt_transfer(&self.pool) else {
                    self.transition(VTState::Disconnected);
                    self.timer_ms = 0;
                    self.pending_end_of_pool_delay_ms = 0;
                    return out;
                };
                let mut transfer = Vec::with_capacity(1 + serialized.len());
                transfer.push(cmd::OBJECT_POOL_TRANSFER);
                transfer.extend(serialized);
                self.pending_end_of_pool_delay_ms =
                    object_pool_transfer_settle_ms(transfer.len(), self.config.timeout_ms);
                out.push(ClientOutbound::to(PGN_ECU_TO_VT, transfer, self.vt_address));
                self.transition(VTState::WaitForPoolStore);
                self.timer_ms = 0;
            }
            VTState::WaitForPoolStore => {
                if self.timer_ms >= self.pending_end_of_pool_delay_ms {
                    let mut eop = [0xFFu8; 8];
                    eop[0] = cmd::END_OF_POOL;
                    out.push(ClientOutbound::to(
                        PGN_ECU_TO_VT,
                        eop.to_vec(),
                        self.vt_address,
                    ));
                    self.transition(VTState::WaitForEndOfPool);
                    self.timer_ms = 0;
                    self.pending_end_of_pool_delay_ms = 0;
                } else if self.timer_ms >= self.config.timeout_ms {
                    self.transition(VTState::Disconnected);
                    self.pending_end_of_pool_delay_ms = 0;
                }
            }
            VTState::WaitForMemory | VTState::WaitForEndOfPool => {
                if self.timer_ms >= self.config.timeout_ms {
                    self.transition(VTState::Disconnected);
                }
            }
            VTState::ReloadPool => {
                self.transition(VTState::SendGetMemory);
                self.timer_ms = 0;
            }
            VTState::Disconnected | VTState::Connected => {}
        }
        out
    }

    // ─── Inbound dispatch ─────────────────────────────────────────────

    /// Feed an inbound `PGN_VT_TO_ECU` message. Side-effects only;
    /// outbound state-driven frames come from the next [`Self::update`].
    pub fn handle_vt_message(&mut self, msg: &Message) {
        if !msg.has_usable_envelope_for_pgn(PGN_VT_TO_ECU) || msg.data.is_empty() {
            return;
        }
        if !self.vt_source_matches_session(msg.source) {
            return;
        }
        match msg.data[0] {
            cmd::VT_STATUS => self.handle_vt_status(msg),
            cmd::GET_MEMORY_RESPONSE => self.handle_get_memory_response(msg),
            cmd::END_OF_POOL => self.handle_end_of_pool_response(msg),
            cmd::SOFT_KEY_ACTIVATION => self.handle_soft_key(msg),
            cmd::BUTTON_ACTIVATION => self.handle_button(msg),
            cmd::POINTING_EVENT => self.handle_pointing_event(msg),
            cmd::SELECT_INPUT_OBJECT => self.handle_select_input_object(msg),
            cmd::SELECT_INPUT_OBJECT_COMMAND => self.handle_select_input_object_response(msg),
            cmd::GRAPHICS_CONTEXT => self.handle_graphics_context_response(msg),
            cmd::NUMERIC_VALUE_CHANGE => self.handle_numeric_change(msg),
            cmd::STRING_VALUE_CHANGE => self.handle_string_change(msg),
            cmd::STORE_VERSION => self.handle_store_version_response(msg),
            cmd::LOAD_VERSION => self.handle_load_version_response(msg),
            cmd::GET_VERSIONS_RESPONSE => self.handle_get_versions_response(msg),
            cmd::VT_ESC => self.handle_vt_esc(msg),
            cmd::EXTENDED_GET_VERSIONS
            | cmd::EXTENDED_STORE_VERSION
            | cmd::EXTENDED_LOAD_VERSION
            | cmd::EXTENDED_DELETE_VERSION => self.handle_extended_version_response(msg),
            cmd::UNSUPPORTED_VT_FUNCTION => self.handle_unsupported_function(msg),
            _ => {}
        }
    }

    /// Feed an inbound `PGN_LANGUAGE_COMMAND` (0xFE0F).
    pub fn handle_language_command(&mut self, msg: &Message) {
        if !msg.has_usable_envelope_for_pgn(PGN_LANGUAGE_COMMAND)
            || msg.data.len() != 8
            || !self.vt_source_matches_session(msg.source)
        {
            return;
        }
        if !is_canonical_language_code([msg.data[0], msg.data[1]]) {
            return;
        }
        let new_lang = LanguageCode {
            code: [msg.data[0], msg.data[1]],
        };
        self.update_vt_language(new_lang);
    }

    // ─── Handlers ─────────────────────────────────────────────────────

    fn handle_vt_status(&mut self, msg: &Message) {
        if msg.data.len() != 8 {
            return;
        }
        self.vt_address = msg.source;
        let reported = msg.data[6];
        if reported > 0 {
            self.vt_version = reported as u16;
        }
        if let Some(self_addr) = self.self_address {
            let active_addr = msg.data[1];
            let was_active = self.is_active_ws;
            self.is_active_ws = active_addr == self_addr;
            if was_active != self.is_active_ws {
                self.on_active_ws_status.emit(&self.is_active_ws);
            }
        }
        if self.state() == VTState::WaitForVTStatus {
            self.transition(VTState::SendWorkingSetMaster);
            self.timer_ms = 0;
        }
    }

    fn handle_get_memory_response(&mut self, msg: &Message) {
        if self.state() != VTState::WaitForMemory {
            return;
        }
        if msg.data.len() != 8 {
            return;
        }
        if msg.data[1] == 0 {
            self.transition(VTState::UploadPool);
        } else {
            self.transition(VTState::Disconnected);
        }
    }

    fn handle_end_of_pool_response(&mut self, msg: &Message) {
        if self.state() != VTState::WaitForEndOfPool {
            return;
        }
        if msg.data.len() != 8 {
            return;
        }
        let error_code = msg.data[1];
        let pool_error_bitmask = msg.data[6];
        if error_code == 0 && pool_error_bitmask == 0 {
            self.transition(VTState::Connected);
        } else {
            let reported_error = if pool_error_bitmask == 0 {
                error_code
            } else {
                pool_error_bitmask
            };
            self.on_pool_error.emit(&reported_error);
            self.transition(VTState::Disconnected);
        }
    }

    fn handle_soft_key(&mut self, msg: &Message) {
        if msg.data.len() != 8 || msg.data[7] != 0xFF {
            return;
        }
        let Some(code) = ActivationCode::try_from_u8(msg.data[1]) else {
            return;
        };
        let key_id = ObjectID(u16_le(&msg.data[2..]));
        self.on_soft_key.emit(&(key_id, code));
        // Full layout: [code][key obj u16][parent mask u16][key number].
        let parent_id = ObjectID(u16_le(&msg.data[4..]));
        self.on_soft_key_detailed
            .emit(&(key_id, parent_id, msg.data[6], code));
    }

    fn handle_button(&mut self, msg: &Message) {
        if msg.data.len() != 8 || msg.data[7] != 0xFF {
            return;
        }
        let Some(code) = ActivationCode::try_from_u8(msg.data[1]) else {
            return;
        };
        let btn_id = ObjectID(u16_le(&msg.data[2..]));
        self.on_button.emit(&(btn_id, code));
        // Full layout: [code][button obj u16][parent mask u16][button number].
        let parent_id = ObjectID(u16_le(&msg.data[4..]));
        self.on_button_detailed
            .emit(&(btn_id, parent_id, msg.data[6], code));
    }

    /// VT Pointing Event (function 0x02): `[0x02][X u16 LE][Y u16 LE][touch
    /// state][object-id u16 LE]`. The touch-state byte uses VT v4+
    /// [`ActivationCode`] values; pre-v4 VTs send `0xFF` there, decoded as
    /// `Released`.
    fn handle_pointing_event(&mut self, msg: &Message) {
        if msg.data.len() != 8 {
            return;
        }
        let x = u16_le(&msg.data[1..]);
        let y = u16_le(&msg.data[3..]);
        let touch = ActivationCode::from_u8(msg.data[5]);
        self.on_pointing_event.emit(&(x, y, touch));
    }

    /// VT Select Input Object (function 0x03): `[0x03][object-id u16 LE]
    /// [selection][open-bitmask][reserved/TAN…]`. Byte 4 reports selection;
    /// byte 5 bit 0 reports open-for-input on VT4+ terminals.
    fn handle_select_input_object(&mut self, msg: &Message) {
        if msg.data.len() != 8 {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let selected = msg.data[3] == 1;
        let open_for_input = msg.data[4] & 0x01 != 0;
        self.on_select_input_object
            .emit(&(id, selected, open_for_input));
    }

    /// Select Input Object response (function 0xA2): `[0xA2][object-id u16
    /// LE][response][error bits][reserved 0xFF…]`.
    fn handle_select_input_object_response(&mut self, msg: &Message) {
        if msg.data.len() != 8 || msg.data[5..8].iter().any(|&byte| byte != 0xFF) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        self.on_select_input_object_response
            .emit(&(id, msg.data[3], msg.data[4]));
    }

    /// Graphics Context response (function 0xB8): `[0xB8][graphics-context-id
    /// u16 LE][subcommand][error bits][reserved 0xFF…]`.
    fn handle_graphics_context_response(&mut self, msg: &Message) {
        if msg.data.len() != 8
            || msg.data[4] & !0x1F != 0
            || msg.data[5..8].iter().any(|&byte| byte != 0xFF)
        {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        self.on_graphics_context_response
            .emit(&(id, msg.data[3], msg.data[4]));
    }

    fn handle_numeric_change(&mut self, msg: &Message) {
        if msg.data.len() != 8 || msg.data[3] != 0xFF {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let value = u32_le(&msg.data[4..]);
        self.on_numeric_value_change.emit(&(id, value));
    }

    fn handle_string_change(&mut self, msg: &Message) {
        if msg.data.len() < 5 {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let len = u16_le(&msg.data[3..]) as usize;
        let end = 5 + len;
        if !vt_string_payload_is_canonical(&msg.data, end) {
            return;
        }
        let Some(s) = decode_vt_string_value(&msg.data[5..end]) else {
            return;
        };
        self.on_string_value_change.emit(&(id, s.to_owned()));
    }

    fn handle_store_version_response(&mut self, msg: &Message) {
        if !version_operation_response_is_canonical(&msg.data) {
            return;
        }
        let success = msg.data[1] == 0;
        let error_code = msg.data[2];
        self.on_store_version_response.emit(&(success, error_code));
    }

    fn handle_load_version_response(&mut self, msg: &Message) {
        if !version_operation_response_is_canonical(&msg.data) {
            return;
        }
        let success = msg.data[1] == 0;
        let error_code = msg.data[2];
        self.on_load_version_response.emit(&(success, error_code));
        if success {
            self.transition(VTState::Connected);
        } else {
            self.transition(VTState::Disconnected);
        }
    }

    fn handle_get_versions_response(&mut self, msg: &Message) {
        if !classic_version_list_response_is_canonical(&msg.data) {
            return;
        }
        let num = msg.data[1] as usize;
        let mut labels: Vec<String> = Vec::with_capacity(num);
        let mut offset = 2usize;
        for _ in 0..num {
            if offset + 7 > msg.data.len() {
                break;
            }
            let Some(label) = decode_padded_version_label(
                &msg.data[offset..offset + 7],
                cmd::CLASSIC_VERSION_LABEL_SIZE,
                "classic VT version label",
            ) else {
                return;
            };
            labels.push(label);
            offset += 7;
        }
        self.on_versions_received.emit(&labels);
    }

    fn handle_vt_esc(&mut self, msg: &Message) {
        if msg.data.len() != 8 || msg.data[4..7].iter().any(|&byte| byte != 0xFF) {
            return;
        }
        let tan = if msg.data[7] == 0xFF {
            None
        } else if msg.data[7] & 0x0F == 0x0F {
            Some(msg.data[7] >> 4)
        } else {
            return;
        };
        let id = ObjectID(u16_le(&msg.data[1..]));
        self.on_vt_esc.emit(&(id, msg.data[3]));
        self.on_vt_esc_detailed.emit(&(id, msg.data[3], tan));
    }

    fn handle_extended_version_response(&mut self, msg: &Message) {
        if msg.data.len() < 2 {
            return;
        }
        if msg.data[1] == cmd::EXTENDED_VERSION_SUBFUNCTION {
            if !extended_version_list_response_is_canonical(&msg.data) {
                return;
            }
            self.vt_supports_extended_versions = true;
            let num = msg.data[2] as usize;
            let mut labels: Vec<String> = Vec::with_capacity(num);
            let mut offset = 3usize;
            for _ in 0..num {
                if offset + cmd::EXTENDED_VERSION_LABEL_SIZE > msg.data.len() {
                    break;
                }
                let Some(label) = decode_padded_version_label(
                    &msg.data[offset..offset + cmd::EXTENDED_VERSION_LABEL_SIZE],
                    cmd::EXTENDED_VERSION_LABEL_SIZE,
                    "extended VT version label",
                ) else {
                    return;
                };
                labels.push(label);
                offset += cmd::EXTENDED_VERSION_LABEL_SIZE;
            }
            self.on_extended_versions_received.emit(&labels);
        } else {
            if !version_operation_response_is_canonical(&msg.data) {
                return;
            }
            let success = msg.data[1] == 0;
            let error_code = msg.data[2];
            if self.state() == VTState::WaitForEndOfPool {
                self.on_extended_load_response.emit(&(success, error_code));
                if success {
                    self.transition(VTState::Connected);
                } else {
                    self.transition(VTState::Disconnected);
                }
            } else {
                self.on_extended_store_response.emit(&(success, error_code));
            }
        }
    }

    fn handle_unsupported_function(&mut self, msg: &Message) {
        if msg.data.len() != 8 {
            return;
        }
        let function = msg.data[1];
        if !self.unsupported_functions.contains(&function) {
            self.unsupported_functions.push(function);
        }
        self.on_unsupported_function.emit(&function);
    }

    fn update_vt_language(&mut self, lang: LanguageCode) {
        if self.vt_language != lang {
            self.vt_language = lang;
            self.check_language_mismatch();
        }
    }

    fn check_language_mismatch(&mut self) {
        if !self.auto_reload_on_language_change {
            return;
        }
        if self.current_language != self.vt_language {
            let old_lang = self.current_language;
            self.current_language = self.vt_language;
            self.on_language_change
                .emit(&(old_lang, self.current_language));
            if self.state() == VTState::Connected {
                self.transition(VTState::ReloadPool);
                self.timer_ms = 0;
            }
        }
    }

    fn transition(&mut self, new_state: VTState) {
        if self.state() == new_state {
            return;
        }
        if new_state == VTState::Disconnected {
            self.clear_vt_session_binding();
        }
        self.state.transition(new_state);
        self.on_state_change.emit(&new_state);
    }

    fn vt_source_matches_session(&self, source: Address) -> bool {
        self.vt_address == NULL_ADDRESS || self.vt_address == source
    }

    fn clear_vt_session_binding(&mut self) {
        self.vt_address = NULL_ADDRESS;
        self.is_active_ws = false;
    }

    fn must_be_connected(&self) -> Result<()> {
        if self.state() != VTState::Connected {
            Err(Error::not_connected())
        } else {
            Ok(())
        }
    }
}

fn write_label_classic(slot: &mut [u8], label: &str) {
    let bytes = label.as_bytes();
    for (i, b) in slot.iter_mut().take(7).enumerate() {
        *b = if i < bytes.len() { bytes[i] } else { b' ' };
    }
}

fn push_label_extended(out: &mut Vec<u8>, label: &str) {
    for i in 0..cmd::EXTENDED_VERSION_LABEL_SIZE {
        out.push(if i < label.len() {
            label.as_bytes()[i]
        } else {
            b' '
        });
    }
}

fn validate_classic_version_label(label: &str) -> Result<()> {
    validate_version_label(
        label,
        cmd::CLASSIC_VERSION_LABEL_SIZE,
        "classic VT version label",
    )
}

fn validate_extended_version_label(label: &str) -> Result<()> {
    validate_version_label(
        label,
        cmd::EXTENDED_VERSION_LABEL_SIZE,
        "extended VT version label",
    )
}

fn validate_version_label(label: &str, max_len: usize, name: &str) -> Result<()> {
    let bytes = label.as_bytes();
    if bytes.is_empty() || bytes.len() > max_len {
        return Err(Error::invalid_data(format!(
            "{name} length must be in 1..={max_len}"
        )));
    }
    if label == "." || label == ".." {
        return Err(Error::invalid_data(format!(
            "{name} must not be a dot path"
        )));
    }
    if bytes
        .iter()
        .any(|byte| *byte < 0x21 || *byte > 0x7E || *byte == b'/' || *byte == b'\\')
    {
        return Err(Error::invalid_data(format!(
            "{name} must contain printable non-path ASCII bytes"
        )));
    }
    Ok(())
}

#[inline]
const fn is_canonical_language_code(code: [u8; 2]) -> bool {
    code[0].is_ascii_alphabetic() && code[1].is_ascii_alphabetic()
}

const VT_OBJECT_POOL_TRANSFER_SETTLE_PER_PACKET_MS: u32 = 50;
const VT_OBJECT_POOL_TRANSFER_SETTLE_MIN_MS: u32 = 100;
