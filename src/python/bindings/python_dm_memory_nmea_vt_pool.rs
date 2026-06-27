/// DM25 freeze frame.
#[pyclass(name = "FreezeFrame", get_all, set_all)]
#[derive(Clone)]
pub struct PyFreezeFrame {
    pub dtc: PyDtc,
    pub timestamp_ms: u32,
    pub snapshots: Vec<PySpnSnapshot>,
}

impl PyFreezeFrame {
    fn to_rust(&self) -> crate::j1939::FreezeFrame {
        crate::j1939::FreezeFrame {
            dtc: self.dtc.to_rust(),
            timestamp_ms: self.timestamp_ms,
            snapshots: self
                .snapshots
                .iter()
                .map(|s| crate::j1939::SpnSnapshot {
                    spn: s.spn,
                    value: s.value,
                })
                .collect(),
        }
    }
    fn from_rust(f: crate::j1939::FreezeFrame) -> Self {
        Self {
            dtc: PyDtc::from_rust(f.dtc),
            timestamp_ms: f.timestamp_ms,
            snapshots: f
                .snapshots
                .into_iter()
                .map(|s| PySpnSnapshot {
                    spn: s.spn,
                    value: s.value,
                })
                .collect(),
        }
    }
}

#[pymethods]
impl PyFreezeFrame {
    #[new]
    #[pyo3(signature = (dtc, timestamp_ms=0, snapshots=Vec::new()))]
    fn new(dtc: PyDtc, timestamp_ms: u32, snapshots: Vec<PySpnSnapshot>) -> Self {
        Self {
            dtc,
            timestamp_ms,
            snapshots,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::FreezeFrame::decode(&data).map(Self::from_rust)
    }
    fn encode(&self) -> Option<Vec<u8>> {
        self.to_rust().encode().ok()
    }
    /// Encode as one DM25 Expanded Freeze Frame entry (PGN 64951).
    fn encode_dm25(&self) -> Vec<u8> {
        self.to_rust().encode_dm25()
    }
    fn __repr__(&self) -> String {
        format!("FreezeFrame(snapshots={})", self.snapshots.len())
    }
}

/// DM25 request.
#[pyclass(name = "Dm25Request", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm25Request {
    pub spn: u32,
    pub fmi: u8,
    pub frame_number: u8,
}

#[pymethods]
impl PyDm25Request {
    #[new]
    #[pyo3(signature = (spn=0, fmi=11, frame_number=0))]
    fn new(spn: u32, fmi: u8, frame_number: u8) -> Self {
        Self {
            spn,
            fmi,
            frame_number,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm25Request::decode(&data).map(|m| Self {
            spn: m.spn,
            fmi: m.fmi.as_u8(),
            frame_number: m.frame_number,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Dm25Request {
            spn: self.spn,
            fmi: Fmi::from_u8(self.fmi),
            frame_number: self.frame_number,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "Dm25Request(spn={}, frame_number={})",
            self.spn, self.frame_number
        )
    }
}

// ── j1939::dm_memory codecs ───────────────────────────────────────────

/// DM14 memory access request. `command` and `pointer_type` are raw enum values.
#[pyclass(name = "Dm14Request", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm14Request {
    pub command: u8,
    pub pointer_type: u8,
    pub address: u32,
    pub length: u16,
    pub key: u8,
}

#[pymethods]
impl PyDm14Request {
    #[new]
    #[pyo3(signature = (command=0, pointer_type=0, address=0, length=0, key=0xFF))]
    fn new(command: u8, pointer_type: u8, address: u32, length: u16, key: u8) -> Self {
        Self {
            command,
            pointer_type,
            address,
            length,
            key,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm14Request::decode(&data).map(|m| Self {
            command: m.command.as_u8(),
            pointer_type: m.pointer_type.as_u8(),
            address: m.address,
            length: m.length,
            key: m.key,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::Dm14Request {
            command: crate::j1939::Dm14Command::from_u8(self.command),
            pointer_type: crate::j1939::Dm14PointerType::from_u8(self.pointer_type),
            address: self.address,
            length: self.length,
            key: self.key,
        }
        .encode()
        .ok()
        .map(|a| a.to_vec())
    }
    fn __repr__(&self) -> String {
        format!(
            "Dm14Request(command={}, address={})",
            self.command, self.address
        )
    }
}

/// DM15 memory access response. `status` is the raw 3-bit status value.
#[pyclass(name = "Dm15Response", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm15Response {
    pub status: u8,
    pub length: u16,
    pub address: u32,
    pub edcp_extension: u8,
    pub seed: u8,
}

#[pymethods]
impl PyDm15Response {
    #[new]
    #[pyo3(signature = (status=0, length=0, address=0, edcp_extension=0xFF, seed=0xFF))]
    fn new(status: u8, length: u16, address: u32, edcp_extension: u8, seed: u8) -> Self {
        Self {
            status,
            length,
            address,
            edcp_extension,
            seed,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm15Response::decode(&data).map(|m| Self {
            status: m.status.as_u8(),
            length: m.length,
            address: m.address,
            edcp_extension: m.edcp_extension,
            seed: m.seed,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::Dm15Response {
            status: crate::j1939::Dm15Status::from_u8(self.status),
            length: self.length,
            address: self.address,
            edcp_extension: self.edcp_extension,
            seed: self.seed,
        }
        .encode()
        .ok()
        .map(|a| a.to_vec())
    }
    fn __repr__(&self) -> String {
        format!(
            "Dm15Response(status={}, length={})",
            self.status, self.length
        )
    }
}

/// DM16 binary data transfer (single frame).
#[pyclass(name = "Dm16Transfer", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm16Transfer {
    pub num_bytes: u8,
    pub data: Vec<u8>,
}

#[pymethods]
impl PyDm16Transfer {
    #[new]
    #[pyo3(signature = (num_bytes=0, data=Vec::new()))]
    fn new(num_bytes: u8, data: Vec<u8>) -> Self {
        Self { num_bytes, data }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm16Transfer::decode(&data).map(|m| Self {
            num_bytes: m.num_bytes,
            data: m.data,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::Dm16Transfer {
            num_bytes: self.num_bytes,
            data: self.data.clone(),
        }
        .encode()
        .ok()
        .map(|a| a.to_vec())
    }
    fn __repr__(&self) -> String {
        format!("Dm16Transfer(num_bytes={})", self.num_bytes)
    }
}

/// ECU Identification (`*`-delimited fields).
#[pyclass(name = "EcuIdentification", get_all, set_all)]
#[derive(Clone)]
pub struct PyEcuIdentification {
    pub ecu_part_number: String,
    pub ecu_serial_number: String,
    pub ecu_location: String,
    pub ecu_type: String,
    pub ecu_manufacturer: String,
    pub ecu_hardware_id: Option<String>,
}

#[pymethods]
impl PyEcuIdentification {
    #[new]
    #[pyo3(signature = (ecu_part_number=String::new(), ecu_serial_number=String::new(), ecu_location=String::new(), ecu_type=String::new(), ecu_manufacturer=String::new(), ecu_hardware_id=None))]
    fn new(
        ecu_part_number: String,
        ecu_serial_number: String,
        ecu_location: String,
        ecu_type: String,
        ecu_manufacturer: String,
        ecu_hardware_id: Option<String>,
    ) -> Self {
        Self {
            ecu_part_number,
            ecu_serial_number,
            ecu_location,
            ecu_type,
            ecu_manufacturer,
            ecu_hardware_id,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::EcuIdentification::decode(&data).map(|m| Self {
            ecu_part_number: m.ecu_part_number,
            ecu_serial_number: m.ecu_serial_number,
            ecu_location: m.ecu_location,
            ecu_type: m.ecu_type,
            ecu_manufacturer: m.ecu_manufacturer,
            ecu_hardware_id: m.ecu_hardware_id,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::EcuIdentification {
            ecu_part_number: self.ecu_part_number.clone(),
            ecu_serial_number: self.ecu_serial_number.clone(),
            ecu_location: self.ecu_location.clone(),
            ecu_type: self.ecu_type.clone(),
            ecu_manufacturer: self.ecu_manufacturer.clone(),
            ecu_hardware_id: self.ecu_hardware_id.clone(),
        }
        .encode()
        .ok()
    }
    fn __repr__(&self) -> String {
        format!(
            "EcuIdentification(ecu_part_number={:?})",
            self.ecu_part_number
        )
    }
}

// ── j1939 misc codecs ─────────────────────────────────────────────────

/// J1939-21 Acknowledgment. `control` is the raw 2-bit control value.
#[pyclass(name = "Acknowledgment", get_all, set_all)]
#[derive(Clone)]
pub struct PyAcknowledgment {
    pub control: u8,
    pub group_function: u8,
    pub acknowledged_pgn: u32,
    pub address: u8,
}

#[pymethods]
impl PyAcknowledgment {
    #[new]
    #[pyo3(signature = (control=0, acknowledged_pgn=0, address=0xFF, group_function=0xFF))]
    fn new(control: u8, acknowledged_pgn: u32, address: u8, group_function: u8) -> Self {
        Self {
            control,
            group_function,
            acknowledged_pgn,
            address,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Acknowledgment::decode(&data).map(|m| Self {
            control: m.control.as_u8(),
            group_function: m.group_function,
            acknowledged_pgn: m.acknowledged_pgn,
            address: m.address,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::Acknowledgment {
            control: crate::j1939::AckControl::from_u8(self.control),
            group_function: self.group_function,
            acknowledged_pgn: self.acknowledged_pgn,
            address: self.address,
        }
        .encode()
        .ok()
        .map(|a| a.to_vec())
    }
    fn __repr__(&self) -> String {
        format!(
            "Acknowledgment(control={}, acknowledged_pgn={})",
            self.control, self.acknowledged_pgn
        )
    }
}

/// ISO 11783 Language Command. Unit/format fields are raw enum values.
#[pyclass(name = "LanguageData", get_all, set_all)]
#[derive(Clone)]
pub struct PyLanguageData {
    pub language_code: Vec<u8>,
    pub decimal: u8,
    pub time_format: u8,
    pub date_format: u8,
    pub distance: u8,
    pub area: u8,
    pub volume: u8,
    pub mass: u8,
    pub temperature: u8,
    pub pressure: u8,
    pub force: u8,
    pub country_code: Vec<u8>,
    pub generic: u8,
}

#[pymethods]
impl PyLanguageData {
    #[new]
    #[pyo3(signature = (language_code=vec![b'e', b'n'], decimal=0, time_format=0, date_format=0, distance=0, area=0, volume=0, mass=0, temperature=0, pressure=0, force=0, country_code=vec![0xFF, 0xFF], generic=0))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        language_code: Vec<u8>,
        decimal: u8,
        time_format: u8,
        date_format: u8,
        distance: u8,
        area: u8,
        volume: u8,
        mass: u8,
        temperature: u8,
        pressure: u8,
        force: u8,
        country_code: Vec<u8>,
        generic: u8,
    ) -> Self {
        Self {
            language_code,
            decimal,
            time_format,
            date_format,
            distance,
            area,
            volume,
            mass,
            temperature,
            pressure,
            force,
            country_code,
            generic,
        }
    }
    /// Decode an 8-byte Language Command payload (PGN 0xFE0F).
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        use crate::net::pgn_defs::PGN_LANGUAGE_COMMAND;
        let msg = crate::net::Message::new(PGN_LANGUAGE_COMMAND, data, 0x80);
        crate::j1939::LanguageData::decode(&msg).map(|m| Self {
            language_code: m.language_code.to_vec(),
            decimal: m.decimal as u8,
            time_format: m.time_format as u8,
            date_format: m.date_format as u8,
            distance: m.distance as u8,
            area: m.area as u8,
            volume: m.volume as u8,
            mass: m.mass as u8,
            temperature: m.temperature as u8,
            pressure: m.pressure as u8,
            force: m.force as u8,
            country_code: m.country_code.to_vec(),
            generic: m.generic as u8,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        use crate::j1939::language::{
            AreaUnit, DateFormat, DecimalSymbol, DistanceUnit, ForceUnit, MassUnit, PressureUnit,
            TemperatureUnit, TimeFormat, UnitSystem, VolumeUnit,
        };
        if self.language_code.len() != 2 || self.country_code.len() != 2 {
            return None;
        }
        Some(
            crate::j1939::LanguageData {
                language_code: [self.language_code[0], self.language_code[1]],
                decimal: DecimalSymbol::try_from_u8(self.decimal)?,
                time_format: TimeFormat::try_from_u8(self.time_format)?,
                date_format: DateFormat::try_from_u8(self.date_format)?,
                distance: DistanceUnit::try_from_u8(self.distance)?,
                area: AreaUnit::try_from_u8(self.area)?,
                volume: VolumeUnit::try_from_u8(self.volume)?,
                mass: MassUnit::try_from_u8(self.mass)?,
                temperature: TemperatureUnit::try_from_u8(self.temperature)?,
                pressure: PressureUnit::try_from_u8(self.pressure)?,
                force: ForceUnit::try_from_u8(self.force)?,
                country_code: [self.country_code[0], self.country_code[1]],
                generic: UnitSystem::try_from_u8(self.generic)?,
            }
            .encode()
            .to_vec(),
        )
    }
    fn __repr__(&self) -> String {
        format!("LanguageData(language_code={:?})", self.language_code)
    }
}

/// ISO 11783 Maintain Power. State/requirement fields are raw 2-bit enum values.
#[pyclass(name = "MaintainPowerData", get_all, set_all)]
#[derive(Clone)]
pub struct PyMaintainPowerData {
    pub implement_in_work_state: u8,
    pub implement_park_state: u8,
    pub implement_ready_to_work_state: u8,
    pub implement_transport_state: u8,
    pub maintain_actuator_power: u8,
    pub maintain_ecu_power: u8,
}

#[pymethods]
impl PyMaintainPowerData {
    #[new]
    #[pyo3(signature = (implement_in_work_state=3, implement_park_state=3, implement_ready_to_work_state=3, implement_transport_state=3, maintain_actuator_power=3, maintain_ecu_power=3))]
    fn new(
        implement_in_work_state: u8,
        implement_park_state: u8,
        implement_ready_to_work_state: u8,
        implement_transport_state: u8,
        maintain_actuator_power: u8,
        maintain_ecu_power: u8,
    ) -> Self {
        Self {
            implement_in_work_state,
            implement_park_state,
            implement_ready_to_work_state,
            implement_transport_state,
            maintain_actuator_power,
            maintain_ecu_power,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::MaintainPowerData::decode(&data).map(|m| Self {
            implement_in_work_state: m.implement_in_work_state.as_u8(),
            implement_park_state: m.implement_park_state.as_u8(),
            implement_ready_to_work_state: m.implement_ready_to_work_state.as_u8(),
            implement_transport_state: m.implement_transport_state.as_u8(),
            maintain_actuator_power: m.maintain_actuator_power.as_u8(),
            maintain_ecu_power: m.maintain_ecu_power.as_u8(),
        })
    }
    fn encode(&self) -> Vec<u8> {
        use crate::j1939::maintain_power::{MaintainPowerRequirement, MaintainPowerState};
        crate::j1939::MaintainPowerData {
            implement_in_work_state: MaintainPowerState::from_u8(self.implement_in_work_state),
            implement_park_state: MaintainPowerState::from_u8(self.implement_park_state),
            implement_ready_to_work_state: MaintainPowerState::from_u8(
                self.implement_ready_to_work_state,
            ),
            implement_transport_state: MaintainPowerState::from_u8(self.implement_transport_state),
            maintain_actuator_power: MaintainPowerRequirement::from_u8(
                self.maintain_actuator_power,
            ),
            maintain_ecu_power: MaintainPowerRequirement::from_u8(self.maintain_ecu_power),
            timestamp_us: 0,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "MaintainPowerData(maintain_ecu_power={})",
            self.maintain_ecu_power
        )
    }
}

/// Speed and distance (8-byte payload). `None` fields are the J1939 sentinel.
#[pyclass(name = "SpeedAndDistance", get_all, set_all)]
#[derive(Clone)]
pub struct PySpeedAndDistance {
    pub speed_mps: Option<f64>,
    pub distance_m: Option<f64>,
}

#[pymethods]
impl PySpeedAndDistance {
    #[new]
    #[pyo3(signature = (speed_mps=None, distance_m=None))]
    fn new(speed_mps: Option<f64>, distance_m: Option<f64>) -> Self {
        Self {
            speed_mps,
            distance_m,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::SpeedAndDistance::decode(&data).map(|m| Self {
            speed_mps: m.speed_mps,
            distance_m: m.distance_m,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::SpeedAndDistance {
            speed_mps: self.speed_mps,
            distance_m: self.distance_m,
            timestamp_us: 0,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "SpeedAndDistance(speed_mps={:?}, distance_m={:?})",
            self.speed_mps, self.distance_m
        )
    }
}

/// ETC1 — electronic transmission controller 1 (PGN 0x0F005).
#[pyclass(name = "Etc1", get_all, set_all)]
#[derive(Clone)]
pub struct PyEtc1 {
    pub current_gear: i8,
    pub selected_gear: i8,
    pub output_shaft_speed_rpm: f64,
    pub shift_in_progress: u8,
    pub torque_converter_lockup: u8,
}

#[pymethods]
impl PyEtc1 {
    #[new]
    #[pyo3(signature = (current_gear=-125, selected_gear=-125, output_shaft_speed_rpm=0.0, shift_in_progress=0x03, torque_converter_lockup=0x03))]
    fn new(
        current_gear: i8,
        selected_gear: i8,
        output_shaft_speed_rpm: f64,
        shift_in_progress: u8,
        torque_converter_lockup: u8,
    ) -> Self {
        Self {
            current_gear,
            selected_gear,
            output_shaft_speed_rpm,
            shift_in_progress,
            torque_converter_lockup,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Etc1::decode(&data).map(|m| Self {
            current_gear: m.current_gear,
            selected_gear: m.selected_gear,
            output_shaft_speed_rpm: m.output_shaft_speed_rpm,
            shift_in_progress: m.shift_in_progress,
            torque_converter_lockup: m.torque_converter_lockup,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Etc1 {
            current_gear: self.current_gear,
            selected_gear: self.selected_gear,
            output_shaft_speed_rpm: self.output_shaft_speed_rpm,
            shift_in_progress: self.shift_in_progress,
            torque_converter_lockup: self.torque_converter_lockup,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("Etc1(current_gear={})", self.current_gear)
    }
}

/// Transmission oil temperature (subset of ET2).
#[pyclass(name = "TransmissionOilTemp", get_all, set_all)]
#[derive(Clone)]
pub struct PyTransmissionOilTemp {
    pub oil_temp_c: f64,
}

#[pymethods]
impl PyTransmissionOilTemp {
    #[new]
    #[pyo3(signature = (oil_temp_c=-40.0))]
    fn new(oil_temp_c: f64) -> Self {
        Self { oil_temp_c }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::TransmissionOilTemp::decode(&data).map(|m| Self {
            oil_temp_c: m.oil_temp_c,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::TransmissionOilTemp {
            oil_temp_c: self.oil_temp_c,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("TransmissionOilTemp(oil_temp_c={:.1})", self.oil_temp_c)
    }
}

/// Cruise control / vehicle speed (CCVS, PGN 0x0FEF1).
#[pyclass(name = "CruiseControl", get_all, set_all)]
#[derive(Clone)]
pub struct PyCruiseControl {
    pub wheel_speed_kmh: f64,
    pub cc_active: u8,
    pub brake_switch: u8,
    pub clutch_switch: u8,
    pub park_brake: u8,
    pub cc_set_speed_kmh: f64,
}

#[pymethods]
impl PyCruiseControl {
    #[new]
    #[pyo3(signature = (wheel_speed_kmh=0.0, cc_active=0, brake_switch=0, clutch_switch=0, park_brake=0, cc_set_speed_kmh=0.0))]
    fn new(
        wheel_speed_kmh: f64,
        cc_active: u8,
        brake_switch: u8,
        clutch_switch: u8,
        park_brake: u8,
        cc_set_speed_kmh: f64,
    ) -> Self {
        Self {
            wheel_speed_kmh,
            cc_active,
            brake_switch,
            clutch_switch,
            park_brake,
            cc_set_speed_kmh,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::CruiseControl::decode(&data).map(|m| Self {
            wheel_speed_kmh: m.wheel_speed_kmh,
            cc_active: m.cc_active,
            brake_switch: m.brake_switch,
            clutch_switch: m.clutch_switch,
            park_brake: m.park_brake,
            cc_set_speed_kmh: m.cc_set_speed_kmh,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::CruiseControl {
            wheel_speed_kmh: self.wheel_speed_kmh,
            cc_active: self.cc_active,
            brake_switch: self.brake_switch,
            clutch_switch: self.clutch_switch,
            park_brake: self.park_brake,
            cc_set_speed_kmh: self.cc_set_speed_kmh,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("CruiseControl(wheel_speed_kmh={:.1})", self.wheel_speed_kmh)
    }
}

/// Shortcut Button message (state + transition counter). `state` is the raw
/// 2-bit value.
#[pyclass(name = "ShortcutButtonMessage", get_all, set_all)]
#[derive(Clone)]
pub struct PyShortcutButtonMessage {
    pub state: u8,
    pub transition_count: u8,
}

#[pymethods]
impl PyShortcutButtonMessage {
    #[new]
    #[pyo3(signature = (state=0, transition_count=0))]
    fn new(state: u8, transition_count: u8) -> Self {
        Self {
            state,
            transition_count,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        use crate::net::pgn_defs::PGN_SHORTCUT_BUTTON;
        let msg = crate::net::Message::new(PGN_SHORTCUT_BUTTON, data, 0x80);
        crate::j1939::shortcut_button::decode_message(&msg).map(|m| Self {
            state: m.state.as_u8(),
            transition_count: m.transition_count,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::shortcut_button::encode_with_transition_count(
            crate::j1939::ShortcutButtonState::from_u8(self.state),
            self.transition_count,
        )
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "ShortcutButtonMessage(state={}, transition_count={})",
            self.state, self.transition_count
        )
    }
}

/// Time/Date (J1939-71 PGN 65254). Unset fields are `None`.
#[pyclass(name = "TimeDate", get_all, set_all)]
#[derive(Clone)]
pub struct PyTimeDate {
    pub seconds: Option<u8>,
    pub minutes: Option<u8>,
    pub hours: Option<u8>,
    pub day: Option<u8>,
    pub month: Option<u8>,
    pub year: Option<u16>,
    pub utc_offset_min: Option<i16>,
    pub utc_offset_hours: Option<i8>,
}

#[pymethods]
impl PyTimeDate {
    #[new]
    #[pyo3(signature = (seconds=None, minutes=None, hours=None, day=None, month=None, year=None, utc_offset_min=None, utc_offset_hours=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        seconds: Option<u8>,
        minutes: Option<u8>,
        hours: Option<u8>,
        day: Option<u8>,
        month: Option<u8>,
        year: Option<u16>,
        utc_offset_min: Option<i16>,
        utc_offset_hours: Option<i8>,
    ) -> Self {
        Self {
            seconds,
            minutes,
            hours,
            day,
            month,
            year,
            utc_offset_min,
            utc_offset_hours,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        use crate::net::pgn_defs::PGN_TIME_DATE;
        let msg = crate::net::Message::new(PGN_TIME_DATE, data, 0x80);
        crate::j1939::TimeDate::decode(&msg).map(|m| Self {
            seconds: m.seconds,
            minutes: m.minutes,
            hours: m.hours,
            day: m.day,
            month: m.month,
            year: m.year,
            utc_offset_min: m.utc_offset_min,
            utc_offset_hours: m.utc_offset_hours,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::TimeDate {
            seconds: self.seconds,
            minutes: self.minutes,
            hours: self.hours,
            day: self.day,
            month: self.month,
            year: self.year,
            utc_offset_min: self.utc_offset_min,
            utc_offset_hours: self.utc_offset_hours,
            timestamp_us: 0,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "TimeDate(hours={:?}, minutes={:?})",
            self.hours, self.minutes
        )
    }
}

/// Request2 message (PGN 0xC900).
#[pyclass(name = "Request2Msg", get_all, set_all)]
#[derive(Clone)]
pub struct PyRequest2Msg {
    pub requested_pgn: u32,
    pub extended_id: Vec<u8>,
    pub use_transfer: bool,
}

#[pymethods]
impl PyRequest2Msg {
    #[new]
    #[pyo3(signature = (requested_pgn=0, extended_id=Vec::new(), use_transfer=false))]
    fn new(requested_pgn: u32, extended_id: Vec<u8>, use_transfer: bool) -> Self {
        Self {
            requested_pgn,
            extended_id,
            use_transfer,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Request2Msg::decode(&data).map(|m| Self {
            requested_pgn: m.requested_pgn,
            extended_id: m.extended_id,
            use_transfer: m.use_transfer,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::Request2Msg {
            requested_pgn: self.requested_pgn,
            extended_id: self.extended_id.clone(),
            use_transfer: self.use_transfer,
        }
        .encode()
        .ok()
        .map(|a| a.to_vec())
    }
    fn __repr__(&self) -> String {
        format!("Request2Msg(requested_pgn={})", self.requested_pgn)
    }
}

/// Transfer message (PGN 0xC700): original PGN + response data.
#[pyclass(name = "TransferMsg", get_all, set_all)]
#[derive(Clone)]
pub struct PyTransferMsg {
    pub original_pgn: u32,
    pub data: Vec<u8>,
}

#[pymethods]
impl PyTransferMsg {
    #[new]
    #[pyo3(signature = (original_pgn=0, data=Vec::new()))]
    fn new(original_pgn: u32, data: Vec<u8>) -> Self {
        Self { original_pgn, data }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::TransferMsg::decode(&data).map(|m| Self {
            original_pgn: m.original_pgn,
            data: m.data,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::TransferMsg {
            original_pgn: self.original_pgn,
            data: self.data.clone(),
        }
        .encode()
        .ok()
    }
    fn __repr__(&self) -> String {
        format!("TransferMsg(original_pgn={})", self.original_pgn)
    }
}

// ── NMEA codecs ───────────────────────────────────────────────────────

/// GNSS position fix (mirror of `nmea::GNSSPosition`). Enum fields
/// (`fix_type`, `gnss_system`) are raw `u8` values.
#[pyclass(name = "GnssPosition", get_all, set_all)]
#[derive(Clone)]
pub struct PyGnssPosition {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: Option<f64>,
    pub heading_rad: Option<f64>,
    pub speed_mps: Option<f64>,
    pub cog_rad: Option<f64>,
    pub hdop: Option<f64>,
    pub pdop: Option<f64>,
    pub vdop: Option<f64>,
    pub satellites_used: u8,
    pub fix_type: u8,
    pub gnss_system: u8,
    pub geoidal_separation_m: Option<f64>,
    pub rate_of_turn_rps: Option<f64>,
    pub pitch_rad: Option<f64>,
    pub roll_rad: Option<f64>,
}

impl PyGnssPosition {
    fn to_rust(&self) -> GNSSPosition {
        use crate::nmea::definitions::{GNSSFixType, GNSSSystem};
        GNSSPosition {
            wgs: Wgs::new(
                self.latitude,
                self.longitude,
                self.altitude_m.unwrap_or(0.0),
            ),
            altitude_m: self.altitude_m,
            heading_rad: self.heading_rad,
            speed_mps: self.speed_mps,
            cog_rad: self.cog_rad,
            hdop: self.hdop,
            pdop: self.pdop,
            vdop: self.vdop,
            satellites_used: self.satellites_used,
            fix_type: GNSSFixType::from_u8(self.fix_type),
            gnss_system: GNSSSystem::try_from_u8(self.gnss_system).unwrap_or(GNSSSystem::GPS),
            geoidal_separation_m: self.geoidal_separation_m,
            rate_of_turn_rps: self.rate_of_turn_rps,
            pitch_rad: self.pitch_rad,
            roll_rad: self.roll_rad,
            timestamp_us: 0,
        }
    }
}

#[pymethods]
impl PyGnssPosition {
    #[new]
    #[pyo3(signature = (latitude=0.0, longitude=0.0, altitude_m=None, heading_rad=None, speed_mps=None, cog_rad=None, hdop=None, pdop=None, vdop=None, satellites_used=0, fix_type=0, gnss_system=0, geoidal_separation_m=None, rate_of_turn_rps=None, pitch_rad=None, roll_rad=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        latitude: f64,
        longitude: f64,
        altitude_m: Option<f64>,
        heading_rad: Option<f64>,
        speed_mps: Option<f64>,
        cog_rad: Option<f64>,
        hdop: Option<f64>,
        pdop: Option<f64>,
        vdop: Option<f64>,
        satellites_used: u8,
        fix_type: u8,
        gnss_system: u8,
        geoidal_separation_m: Option<f64>,
        rate_of_turn_rps: Option<f64>,
        pitch_rad: Option<f64>,
        roll_rad: Option<f64>,
    ) -> Self {
        Self {
            latitude,
            longitude,
            altitude_m,
            heading_rad,
            speed_mps,
            cog_rad,
            hdop,
            pdop,
            vdop,
            satellites_used,
            fix_type,
            gnss_system,
            geoidal_separation_m,
            rate_of_turn_rps,
            pitch_rad,
            roll_rad,
        }
    }
    fn __repr__(&self) -> String {
        format!(
            "GnssPosition(latitude={:.6}, longitude={:.6})",
            self.latitude, self.longitude
        )
    }
}

/// NMEA 2000 send-frame builders. All methods are static and return the 8-byte
/// payload.
#[pyclass(name = "NMEAInterface")]
pub struct PyNMEAInterface;

#[pymethods]
impl PyNMEAInterface {
    /// PGN 129025 — position rapid update. Returns 8 bytes.
    #[staticmethod]
    fn build_position(pos: &PyGnssPosition) -> Vec<u8> {
        crate::nmea::NMEAInterface::build_position(&pos.to_rust()).to_vec()
    }
    /// PGN 129026 — COG/SOG rapid. Returns 8 bytes.
    #[staticmethod]
    fn build_cog_sog(cog_rad: f64, sog_mps: f64) -> Vec<u8> {
        crate::nmea::NMEAInterface::build_cog_sog(cog_rad, sog_mps).to_vec()
    }
    /// PGN 127250 — heading/track. Returns 8 bytes.
    #[staticmethod]
    fn build_heading(heading_rad: f64, deviation_rad: f64, variation_rad: f64) -> Vec<u8> {
        crate::nmea::NMEAInterface::build_heading(heading_rad, deviation_rad, variation_rad)
            .to_vec()
    }
}

/// PGN 130306 — wind data. `reference` is a raw `WindReference` value.
#[pyclass(name = "WindData", get_all, set_all)]
#[derive(Clone)]
pub struct PyWindData {
    pub sid: u8,
    pub speed_mps: f64,
    pub direction_rad: f64,
    pub reference: u8,
}

#[pymethods]
impl PyWindData {
    #[new]
    #[pyo3(signature = (sid=0xFF, speed_mps=0.0, direction_rad=0.0, reference=0))]
    fn new(sid: u8, speed_mps: f64, direction_rad: f64, reference: u8) -> Self {
        Self {
            sid,
            speed_mps,
            direction_rad,
            reference,
        }
    }
    fn __repr__(&self) -> String {
        format!("WindData(speed_mps={:.2})", self.speed_mps)
    }
}

/// PGN 130312 — temperature. `source` is a raw `TemperatureSource` value.
#[pyclass(name = "TemperatureData", get_all, set_all)]
#[derive(Clone)]
pub struct PyTemperatureData {
    pub sid: u8,
    pub instance: u8,
    pub source: u8,
    pub actual_k: f64,
    pub set_k: f64,
}

#[pymethods]
impl PyTemperatureData {
    #[new]
    #[pyo3(signature = (sid=0xFF, instance=0, source=0, actual_k=0.0, set_k=0.0))]
    fn new(sid: u8, instance: u8, source: u8, actual_k: f64, set_k: f64) -> Self {
        Self {
            sid,
            instance,
            source,
            actual_k,
            set_k,
        }
    }
    fn __repr__(&self) -> String {
        format!("TemperatureData(actual_k={:.2})", self.actual_k)
    }
}

/// PGN 130314 — pressure. `source` is a raw `PressureSource` value.
#[pyclass(name = "PressureData", get_all, set_all)]
#[derive(Clone)]
pub struct PyPressureData {
    pub sid: u8,
    pub instance: u8,
    pub source: u8,
    pub pressure_pa: f64,
}

#[pymethods]
impl PyPressureData {
    #[new]
    #[pyo3(signature = (sid=0xFF, instance=0, source=0, pressure_pa=0.0))]
    fn new(sid: u8, instance: u8, source: u8, pressure_pa: f64) -> Self {
        Self {
            sid,
            instance,
            source,
            pressure_pa,
        }
    }
    fn __repr__(&self) -> String {
        format!("PressureData(pressure_pa={:.1})", self.pressure_pa)
    }
}

/// PGN 127488 — engine parameters rapid.
#[pyclass(name = "EngineData", get_all, set_all)]
#[derive(Clone)]
pub struct PyEngineData {
    pub instance: u8,
    pub rpm: f64,
    pub boost_pressure_pa: f64,
    pub tilt_trim: i8,
}

#[pymethods]
impl PyEngineData {
    #[new]
    #[pyo3(signature = (instance=0, rpm=0.0, boost_pressure_pa=0.0, tilt_trim=0))]
    fn new(instance: u8, rpm: f64, boost_pressure_pa: f64, tilt_trim: i8) -> Self {
        Self {
            instance,
            rpm,
            boost_pressure_pa,
            tilt_trim,
        }
    }
    fn __repr__(&self) -> String {
        format!("EngineData(rpm={:.0})", self.rpm)
    }
}

/// PGN 128267 — water depth.
#[pyclass(name = "WaterDepthData", get_all, set_all)]
#[derive(Clone)]
pub struct PyWaterDepthData {
    pub sid: u8,
    pub depth_m: f64,
    pub offset_m: f64,
    pub range_m: f64,
}

#[pymethods]
impl PyWaterDepthData {
    #[new]
    #[pyo3(signature = (sid=0xFF, depth_m=0.0, offset_m=0.0, range_m=0.0))]
    fn new(sid: u8, depth_m: f64, offset_m: f64, range_m: f64) -> Self {
        Self {
            sid,
            depth_m,
            offset_m,
            range_m,
        }
    }
    fn __repr__(&self) -> String {
        format!("WaterDepthData(depth_m={:.2})", self.depth_m)
    }
}

/// PGN 128259 — speed (water referenced). `reference` is a raw value.
#[pyclass(name = "SpeedWaterData", get_all, set_all)]
#[derive(Clone)]
pub struct PySpeedWaterData {
    pub sid: u8,
    pub water_speed_mps: f64,
    pub ground_speed_mps: f64,
    pub reference: u8,
}

#[pymethods]
impl PySpeedWaterData {
    #[new]
    #[pyo3(signature = (sid=0xFF, water_speed_mps=0.0, ground_speed_mps=0.0, reference=0))]
    fn new(sid: u8, water_speed_mps: f64, ground_speed_mps: f64, reference: u8) -> Self {
        Self {
            sid,
            water_speed_mps,
            ground_speed_mps,
            reference,
        }
    }
    fn __repr__(&self) -> String {
        format!(
            "SpeedWaterData(water_speed_mps={:.2})",
            self.water_speed_mps
        )
    }
}

/// PGN 126992 — system time. `source` is a raw `TimeSource` value.
#[pyclass(name = "SystemTimeData", get_all, set_all)]
#[derive(Clone)]
pub struct PySystemTimeData {
    pub sid: u8,
    pub source: u8,
    pub days_since_epoch: u16,
    pub seconds_since_midnight: f64,
}

#[pymethods]
impl PySystemTimeData {
    #[new]
    #[pyo3(signature = (sid=0xFF, source=0, days_since_epoch=0, seconds_since_midnight=0.0))]
    fn new(sid: u8, source: u8, days_since_epoch: u16, seconds_since_midnight: f64) -> Self {
        Self {
            sid,
            source,
            days_since_epoch,
            seconds_since_midnight,
        }
    }
    fn __repr__(&self) -> String {
        format!("SystemTimeData(days_since_epoch={})", self.days_since_epoch)
    }
}

/// PGN 127251 — rate of turn.
#[pyclass(name = "RateOfTurnData", get_all, set_all)]
#[derive(Clone)]
pub struct PyRateOfTurnData {
    pub sid: u8,
    pub rate_rad_per_s: f64,
}

#[pymethods]
impl PyRateOfTurnData {
    #[new]
    #[pyo3(signature = (sid=0xFF, rate_rad_per_s=0.0))]
    fn new(sid: u8, rate_rad_per_s: f64) -> Self {
        Self {
            sid,
            rate_rad_per_s,
        }
    }
    fn __repr__(&self) -> String {
        format!("RateOfTurnData(rate_rad_per_s={:.4})", self.rate_rad_per_s)
    }
}

/// PGN 127257 — attitude (yaw/pitch/roll).
#[pyclass(name = "AttitudeData", get_all, set_all)]
#[derive(Clone)]
pub struct PyAttitudeData {
    pub sid: u8,
    pub yaw_rad: f64,
    pub pitch_rad: f64,
    pub roll_rad: f64,
}

#[pymethods]
impl PyAttitudeData {
    #[new]
    #[pyo3(signature = (sid=0xFF, yaw_rad=0.0, pitch_rad=0.0, roll_rad=0.0))]
    fn new(sid: u8, yaw_rad: f64, pitch_rad: f64, roll_rad: f64) -> Self {
        Self {
            sid,
            yaw_rad,
            pitch_rad,
            roll_rad,
        }
    }
    fn __repr__(&self) -> String {
        format!("AttitudeData(yaw_rad={:.4})", self.yaw_rad)
    }
}

/// PGN 127258 — magnetic variation. `source` is a raw value.
#[pyclass(name = "MagneticVariationData", get_all, set_all)]
#[derive(Clone)]
pub struct PyMagneticVariationData {
    pub sid: u8,
    pub source: u8,
    pub days_since_epoch: u16,
    pub variation_rad: f64,
}

#[pymethods]
impl PyMagneticVariationData {
    #[new]
    #[pyo3(signature = (sid=0xFF, source=0, days_since_epoch=0, variation_rad=0.0))]
    fn new(sid: u8, source: u8, days_since_epoch: u16, variation_rad: f64) -> Self {
        Self {
            sid,
            source,
            days_since_epoch,
            variation_rad,
        }
    }
    fn __repr__(&self) -> String {
        format!(
            "MagneticVariationData(variation_rad={:.4})",
            self.variation_rad
        )
    }
}

/// PGN 127245 — rudder. `direction` is a raw value.
#[pyclass(name = "RudderData", get_all, set_all)]
#[derive(Clone)]
pub struct PyRudderData {
    pub position_rad: f64,
    pub instance: u8,
    pub direction: u8,
    pub angle_order_rad: f64,
}

#[pymethods]
impl PyRudderData {
    #[new]
    #[pyo3(signature = (position_rad=0.0, instance=0, direction=0, angle_order_rad=0.0))]
    fn new(position_rad: f64, instance: u8, direction: u8, angle_order_rad: f64) -> Self {
        Self {
            position_rad,
            instance,
            direction,
            angle_order_rad,
        }
    }
    fn __repr__(&self) -> String {
        format!("RudderData(position_rad={:.4})", self.position_rad)
    }
}

/// PGN 127505 — fluid level. `fluid_type` is a raw `FluidType` value.
#[pyclass(name = "FluidLevelData", get_all, set_all)]
#[derive(Clone)]
pub struct PyFluidLevelData {
    pub instance: u8,
    pub fluid_type: u8,
    pub level_pct: f64,
    pub capacity_l: f64,
}

#[pymethods]
impl PyFluidLevelData {
    #[new]
    #[pyo3(signature = (instance=0, fluid_type=0, level_pct=0.0, capacity_l=0.0))]
    fn new(instance: u8, fluid_type: u8, level_pct: f64, capacity_l: f64) -> Self {
        Self {
            instance,
            fluid_type,
            level_pct,
            capacity_l,
        }
    }
    fn __repr__(&self) -> String {
        format!("FluidLevelData(level_pct={:.1})", self.level_pct)
    }
}

/// PGN 127508 — battery status.
#[pyclass(name = "BatteryStatusData", get_all, set_all)]
#[derive(Clone)]
pub struct PyBatteryStatusData {
    pub instance: u8,
    pub voltage: f64,
    pub current_a: f64,
    pub state_of_charge_pct: u8,
    pub state_of_health_pct: u8,
    pub time_remaining_s: f64,
}

#[pymethods]
impl PyBatteryStatusData {
    #[new]
    #[pyo3(signature = (instance=0, voltage=0.0, current_a=0.0, state_of_charge_pct=0xFF, state_of_health_pct=0xFF, time_remaining_s=0.0))]
    fn new(
        instance: u8,
        voltage: f64,
        current_a: f64,
        state_of_charge_pct: u8,
        state_of_health_pct: u8,
        time_remaining_s: f64,
    ) -> Self {
        Self {
            instance,
            voltage,
            current_a,
            state_of_charge_pct,
            state_of_health_pct,
            time_remaining_s,
        }
    }
    fn __repr__(&self) -> String {
        format!("BatteryStatusData(voltage={:.2})", self.voltage)
    }
}

/// PGN 129539 — GNSS DOPs. `desired_mode`/`actual_mode` are raw values.
#[pyclass(name = "GNSSDOPData", get_all, set_all)]
#[derive(Clone)]
pub struct PyGNSSDOPData {
    pub sid: u8,
    pub desired_mode: u8,
    pub actual_mode: u8,
    pub hdop: f64,
    pub vdop: f64,
    pub tdop: f64,
}

#[pymethods]
impl PyGNSSDOPData {
    #[new]
    #[pyo3(signature = (sid=0xFF, desired_mode=0, actual_mode=0, hdop=0.0, vdop=0.0, tdop=0.0))]
    fn new(sid: u8, desired_mode: u8, actual_mode: u8, hdop: f64, vdop: f64, tdop: f64) -> Self {
        Self {
            sid,
            desired_mode,
            actual_mode,
            hdop,
            vdop,
            tdop,
        }
    }
    fn __repr__(&self) -> String {
        format!("GNSSDOPData(hdop={:.2})", self.hdop)
    }
}

/// PGN 129283 — cross-track error. `mode` is a raw `XTEMode` value.
#[pyclass(name = "XTEData", get_all, set_all)]
#[derive(Clone)]
pub struct PyXTEData {
    pub sid: u8,
    pub mode: u8,
    pub navigation_terminated: bool,
    pub xte_m: f64,
}

#[pymethods]
impl PyXTEData {
    #[new]
    #[pyo3(signature = (sid=0xFF, mode=0, navigation_terminated=false, xte_m=0.0))]
    fn new(sid: u8, mode: u8, navigation_terminated: bool, xte_m: f64) -> Self {
        Self {
            sid,
            mode,
            navigation_terminated,
            xte_m,
        }
    }
    fn __repr__(&self) -> String {
        format!("XTEData(xte_m={:.2})", self.xte_m)
    }
}

/// PGN 129284 — navigation data. `bearing_reference`/`calc_type` are raw values.
#[pyclass(name = "NavigationData", get_all, set_all)]
#[derive(Clone)]
pub struct PyNavigationData {
    pub sid: u8,
    pub distance_to_wp_m: f64,
    pub bearing_reference: u8,
    pub perpendicular_crossed: bool,
    pub arrival_circle_entered: bool,
    pub calc_type: u8,
    pub eta_time: f64,
    pub eta_date: i16,
    pub bearing_origin_to_dest_rad: f64,
    pub bearing_pos_to_dest_rad: f64,
    pub origin_wp_number: u32,
    pub dest_wp_number: u32,
    pub dest_latitude: f64,
    pub dest_longitude: f64,
    pub wp_closing_velocity_mps: f64,
}

#[pymethods]
impl PyNavigationData {
    #[new]
    #[pyo3(signature = (sid=0xFF, distance_to_wp_m=0.0, bearing_reference=0, perpendicular_crossed=false, arrival_circle_entered=false, calc_type=0, eta_time=0.0, eta_date=0, bearing_origin_to_dest_rad=0.0, bearing_pos_to_dest_rad=0.0, origin_wp_number=0, dest_wp_number=0, dest_latitude=0.0, dest_longitude=0.0, wp_closing_velocity_mps=0.0))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        sid: u8,
        distance_to_wp_m: f64,
        bearing_reference: u8,
        perpendicular_crossed: bool,
        arrival_circle_entered: bool,
        calc_type: u8,
        eta_time: f64,
        eta_date: i16,
        bearing_origin_to_dest_rad: f64,
        bearing_pos_to_dest_rad: f64,
        origin_wp_number: u32,
        dest_wp_number: u32,
        dest_latitude: f64,
        dest_longitude: f64,
        wp_closing_velocity_mps: f64,
    ) -> Self {
        Self {
            sid,
            distance_to_wp_m,
            bearing_reference,
            perpendicular_crossed,
            arrival_circle_entered,
            calc_type,
            eta_time,
            eta_date,
            bearing_origin_to_dest_rad,
            bearing_pos_to_dest_rad,
            origin_wp_number,
            dest_wp_number,
            dest_latitude,
            dest_longitude,
            wp_closing_velocity_mps,
        }
    }
    fn __repr__(&self) -> String {
        format!(
            "NavigationData(distance_to_wp_m={:.1})",
            self.distance_to_wp_m
        )
    }
}

// ── Standalone NMEA 2000 decoder ──────────────────────────────────────
// Wraps `nmea::NMEAInterface` + an internal queue. Feed raw (pgn, data)
// frames and poll/drain the decoded GNSS results. No Session required.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct NmeaQueues {
    positions: Vec<PyGnssPosition>,
    cog: Vec<f64>,
    sog: Vec<f64>,
    heading: Vec<f64>,
}

fn gnss_to_py(p: &GNSSPosition) -> PyGnssPosition {
    PyGnssPosition {
        latitude: p.wgs.latitude,
        longitude: p.wgs.longitude,
        altitude_m: p.altitude_m,
        heading_rad: p.heading_rad,
        speed_mps: p.speed_mps,
        cog_rad: p.cog_rad,
        hdop: p.hdop,
        pdop: p.pdop,
        vdop: p.vdop,
        satellites_used: p.satellites_used,
        fix_type: p.fix_type.as_u8(),
        gnss_system: p.gnss_system.as_u8(),
        geoidal_separation_m: p.geoidal_separation_m,
        rate_of_turn_rps: p.rate_of_turn_rps,
        pitch_rad: p.pitch_rad,
        roll_rad: p.roll_rad,
    }
}

/// Standalone NMEA 2000 *decoder*.
///
/// Feed received N2K frames with [`NmeaDecoder.feed`] (PGN + reassembled
/// payload bytes). Decoded GNSS results are queued internally; drain them
/// with the `poll_*` accessors or [`NmeaDecoder.drain`].
///
/// ```python
/// dec = machbus.NmeaDecoder()
/// dec.feed(129025, position_bytes)
/// pos = dec.poll_position()
/// ```
#[pyclass(name = "NmeaDecoder", unsendable)]
pub struct PyNmeaDecoder {
    iface: crate::nmea::NMEAInterface,
    queues: Rc<RefCell<NmeaQueues>>,
}

#[pymethods]
impl PyNmeaDecoder {
    #[new]
    fn new() -> Self {
        let mut iface =
            crate::nmea::NMEAInterface::new(NMEAConfig::default().with_gnss_navigation(true));
        let queues: Rc<RefCell<NmeaQueues>> = Rc::new(RefCell::new(NmeaQueues::default()));

        let q = queues.clone();
        iface
            .on_position
            .subscribe(move |p: &GNSSPosition| q.borrow_mut().positions.push(gnss_to_py(p)));
        let q = queues.clone();
        iface
            .on_cog
            .subscribe(move |c: &f64| q.borrow_mut().cog.push(*c));
        let q = queues.clone();
        iface
            .on_sog
            .subscribe(move |s: &f64| q.borrow_mut().sog.push(*s));
        let q = queues.clone();
        iface
            .on_heading
            .subscribe(move |h: &f64| q.borrow_mut().heading.push(*h));

        Self { iface, queues }
    }

    /// Feed one received N2K message: PGN + reassembled payload bytes.
    #[pyo3(signature = (pgn, data, source=0xFE))]
    fn feed(&mut self, pgn: u32, data: Vec<u8>, source: u8) {
        let msg = crate::net::Message::new(pgn as Pgn, data, source);
        self.iface.handle_message(&msg);
    }

    /// Oldest queued GNSS position, or `None`.
    fn poll_position(&mut self) -> Option<PyGnssPosition> {
        let mut q = self.queues.borrow_mut();
        if q.positions.is_empty() {
            None
        } else {
            Some(q.positions.remove(0))
        }
    }

    /// Oldest queued course-over-ground (radians), or `None`.
    fn poll_cog(&mut self) -> Option<f64> {
        let mut q = self.queues.borrow_mut();
        if q.cog.is_empty() {
            None
        } else {
            Some(q.cog.remove(0))
        }
    }

    /// Oldest queued speed-over-ground (m/s), or `None`.
    fn poll_sog(&mut self) -> Option<f64> {
        let mut q = self.queues.borrow_mut();
        if q.sog.is_empty() {
            None
        } else {
            Some(q.sog.remove(0))
        }
    }

    /// Oldest queued heading (radians), or `None`.
    fn poll_heading(&mut self) -> Option<f64> {
        let mut q = self.queues.borrow_mut();
        if q.heading.is_empty() {
            None
        } else {
            Some(q.heading.remove(0))
        }
    }

    /// Latest position fix held by the decoder (does not consume the queue).
    fn latest_position(&self) -> Option<PyGnssPosition> {
        self.iface.latest_position().as_ref().map(gnss_to_py)
    }

    /// Drain everything queued so far as a list of dicts, each with a
    /// `"kind"` of `position` / `cog` / `sog` / `heading`.
    fn drain<'py>(&mut self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyDict>>> {
        let mut q = self.queues.borrow_mut();
        let mut out = Vec::new();
        for p in q.positions.drain(..) {
            let d = PyDict::new(py);
            d.set_item("kind", "position")?;
            d.set_item("latitude", p.latitude)?;
            d.set_item("longitude", p.longitude)?;
            d.set_item("altitude_m", p.altitude_m)?;
            d.set_item("speed_mps", p.speed_mps)?;
            d.set_item("heading_rad", p.heading_rad)?;
            out.push(d);
        }
        for c in q.cog.drain(..) {
            let d = PyDict::new(py);
            d.set_item("kind", "cog")?;
            d.set_item("rad", c)?;
            out.push(d);
        }
        for s in q.sog.drain(..) {
            let d = PyDict::new(py);
            d.set_item("kind", "sog")?;
            d.set_item("mps", s)?;
            out.push(d);
        }
        for h in q.heading.drain(..) {
            let d = PyDict::new(py);
            d.set_item("kind", "heading")?;
            d.set_item("rad", h)?;
            out.push(d);
        }
        Ok(out)
    }
}

// ─── VT object pool builder ───────────────────────────────────────────

/// Opaque builder for an ISO 11783-6 VT object pool
/// ([`crate::isobus::vt::ObjectPool`]).
///
/// Construct an empty pool and add objects with the typed `add_*` helpers,
/// or load a prebuilt `.iop` byte buffer with [`VtPool::from_iop`]. Pass the
/// finished pool to `Session(vt_pool=..., working_set=...)`.
#[pyclass(name = "VtPool")]
#[derive(Clone, Default)]
pub struct VtPool {
    inner: crate::isobus::vt::ObjectPool,
}

impl VtPool {
    fn add_object(&mut self, obj: crate::isobus::vt::VTObject) -> PyResult<u16> {
        let id = obj.id.raw();
        self.inner.add(obj).map_err(err_runtime)?;
        Ok(id)
    }
}

