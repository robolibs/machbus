#[pymethods]
impl VtPool {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Load a prebuilt `.iop` object-pool byte buffer.
    ///
    /// The buffer is validated with `crate::net::validate` and then decoded
    /// into a typed [`crate::isobus::vt::ObjectPool`] via its conformant
    /// deserializer (the same parse-by-type walker `crate::net::parse_iop_data`
    /// is built on), so the resulting pool can be serialized and uploaded.
    #[staticmethod]
    fn from_iop(data: Vec<u8>) -> PyResult<Self> {
        if !crate::net::validate(&data) {
            // Surface the precise parser error for a malformed buffer.
            crate::net::parse_iop_data(&data).map_err(err_runtime)?;
            return Err(PyValueError::new_err("invalid IOP object-pool data"));
        }
        let inner = crate::isobus::vt::ObjectPool::deserialize(&data).map_err(err_runtime)?;
        Ok(Self { inner })
    }

    /// Number of objects currently in the pool.
    fn object_count(&self) -> usize {
        self.inner.size()
    }

    /// Serialize the pool to ISO 11783-6 object-pool bytes.
    fn serialize(&self) -> PyResult<Vec<u8>> {
        self.inner.serialize().map_err(err_runtime)
    }

    // ─── Typed object builders (common set) ──────────────────────────

    /// Add a Working Set (Type 0). Returns the object id.
    #[pyo3(signature = (id, background_colour=0, selectable=1, active_mask=0xFFFF))]
    fn add_working_set(
        &mut self,
        id: u16,
        background_colour: u8,
        selectable: u8,
        active_mask: u16,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{WorkingSetBody, create_working_set};
        let body = WorkingSetBody {
            background_colour,
            selectable,
            active_mask: active_mask.into(),
            ..Default::default()
        };
        self.add_object(create_working_set(id, &body))
    }

    /// Add a Data Mask (Type 1). Returns the object id.
    #[pyo3(signature = (id, background_color=0, soft_key_mask=0xFFFF))]
    fn add_data_mask(
        &mut self,
        id: u16,
        background_color: u8,
        soft_key_mask: u16,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{DataMaskBody, create_data_mask};
        let body = DataMaskBody {
            background_color,
            soft_key_mask: soft_key_mask.into(),
        };
        self.add_object(create_data_mask(id, &body))
    }

    /// Add a Container (Type 3). Returns the object id.
    #[pyo3(signature = (id, width=0, height=0, hidden=false))]
    fn add_container(&mut self, id: u16, width: u16, height: u16, hidden: bool) -> PyResult<u16> {
        use crate::isobus::vt::{ContainerBody, create_container};
        let body = ContainerBody {
            width,
            height,
            hidden,
        };
        self.add_object(create_container(id, &body))
    }

    /// Add an Output Number (Type 12). Returns the object id.
    #[pyo3(signature = (
        id, width=0, height=0, background_color=0, font_attributes=0xFFFF,
        options=0, variable_reference=0xFFFF, value=0, offset=0, scale=1.0,
        number_of_decimals=0, format=0, justification=0
    ))]
    #[allow(clippy::too_many_arguments)]
    fn add_output_number(
        &mut self,
        id: u16,
        width: u16,
        height: u16,
        background_color: u8,
        font_attributes: u16,
        options: u8,
        variable_reference: u16,
        value: u32,
        offset: i32,
        scale: f32,
        number_of_decimals: u8,
        format: u8,
        justification: u8,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{OutputNumberBody, create_output_number};
        let body = OutputNumberBody {
            width,
            height,
            background_color,
            font_attributes: font_attributes.into(),
            options,
            variable_reference: variable_reference.into(),
            value,
            offset,
            scale,
            number_of_decimals,
            format,
            justification,
        };
        self.add_object(create_output_number(id, &body).map_err(err_runtime)?)
    }

    /// Add an Output String (Type 11). Returns the object id.
    #[pyo3(signature = (
        id, width=0, height=0, background_color=0, font_attributes=0xFFFF,
        options=0, variable_reference=0xFFFF, justification=0, value=String::new()
    ))]
    #[allow(clippy::too_many_arguments)]
    fn add_output_string(
        &mut self,
        id: u16,
        width: u16,
        height: u16,
        background_color: u8,
        font_attributes: u16,
        options: u8,
        variable_reference: u16,
        justification: u8,
        value: String,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{OutputStringBody, create_output_string};
        let body = OutputStringBody {
            width,
            height,
            background_color,
            font_attributes: font_attributes.into(),
            options,
            variable_reference: variable_reference.into(),
            justification,
            value: value.into_bytes(),
        };
        self.add_object(create_output_string(id, &body).map_err(err_runtime)?)
    }

    /// Add an Output Rectangle (Type 14). Returns the object id.
    #[pyo3(signature = (
        id, width=0, height=0, line_attributes=0xFFFF, line_suppression=0,
        fill_attributes=0xFFFF
    ))]
    fn add_output_rectangle(
        &mut self,
        id: u16,
        width: u16,
        height: u16,
        line_attributes: u16,
        line_suppression: u8,
        fill_attributes: u16,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{OutputRectangleBody, create_output_rectangle};
        let body = OutputRectangleBody {
            width,
            height,
            line_attributes: line_attributes.into(),
            line_suppression,
            fill_attributes: fill_attributes.into(),
        };
        self.add_object(create_output_rectangle(id, &body).map_err(err_runtime)?)
    }

    /// Add a Soft Key Mask (Type 4). Returns the object id.
    #[pyo3(signature = (id, background_color=0))]
    fn add_soft_key_mask(&mut self, id: u16, background_color: u8) -> PyResult<u16> {
        use crate::isobus::vt::{SoftKeyMaskBody, create_soft_key_mask};
        let body = SoftKeyMaskBody { background_color };
        self.add_object(create_soft_key_mask(id, &body))
    }

    /// Add a Key (Type 5). Returns the object id.
    #[pyo3(signature = (id, background_color=0, key_code=0))]
    fn add_key(&mut self, id: u16, background_color: u8, key_code: u8) -> PyResult<u16> {
        use crate::isobus::vt::{KeyBody, create_key};
        let body = KeyBody {
            background_color,
            key_code,
        };
        self.add_object(create_key(id, &body))
    }

    /// Add a Button (Type 6). Returns the object id.
    #[pyo3(signature = (
        id, width=0, height=0, background_color=0, border_color=0, key_code=0,
        options=0
    ))]
    #[allow(clippy::too_many_arguments)]
    fn add_button(
        &mut self,
        id: u16,
        width: u16,
        height: u16,
        background_color: u8,
        border_color: u8,
        key_code: u8,
        options: u8,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{ButtonBody, create_button};
        let body = ButtonBody {
            width,
            height,
            background_color,
            border_color,
            key_code,
            options,
        };
        self.add_object(create_button(id, &body))
    }

    /// Add an Input Number (Type 9). `options` is Options 1; `options2`
    /// carries enabled and real-time-editing bits. Returns the object id.
    #[pyo3(signature = (
        id, width=0, height=0, background_color=0, font_attributes=0xFFFF,
        options=0, variable_reference=0xFFFF, value=0, min_value=0, max_value=0,
        offset=0, scale=1.0, number_of_decimals=0, format=0, justification=0,
        options2=0
    ))]
    #[allow(clippy::too_many_arguments)]
    fn add_input_number(
        &mut self,
        id: u16,
        width: u16,
        height: u16,
        background_color: u8,
        font_attributes: u16,
        options: u8,
        variable_reference: u16,
        value: u32,
        min_value: i32,
        max_value: i32,
        offset: i32,
        scale: f32,
        number_of_decimals: u8,
        format: u8,
        justification: u8,
        options2: u8,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{InputNumberBody, create_input_number};
        let body = InputNumberBody {
            width,
            height,
            background_color,
            font_attributes: font_attributes.into(),
            options,
            variable_reference: variable_reference.into(),
            value,
            min_value,
            max_value,
            offset,
            scale,
            number_of_decimals,
            format,
            justification,
            options2,
        };
        self.add_object(create_input_number(id, &body).map_err(err_runtime)?)
    }

    /// Add an Input List (Type 10). Returns the object id.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (id, width=0, height=0, variable_reference=0xFFFF, value=255, options=0, items=Vec::new()))]
    fn add_input_list(
        &mut self,
        id: u16,
        width: u16,
        height: u16,
        variable_reference: u16,
        value: u8,
        options: u8,
        items: Vec<u16>,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{InputListBody, create_input_list};
        let body = InputListBody {
            width,
            height,
            variable_reference: variable_reference.into(),
            value,
            options,
            items: items.into_iter().map(Into::into).collect(),
        };
        self.add_object(create_input_list(id, &body).map_err(err_runtime)?)
    }

    /// Add a Font Attributes object (Type 23). Returns the object id.
    #[pyo3(signature = (id, font_color=0, font_size=0, font_type=0, font_style=0))]
    fn add_font_attributes(
        &mut self,
        id: u16,
        font_color: u8,
        font_size: u8,
        font_type: u8,
        font_style: u8,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{FontAttributesBody, create_font_attributes};
        let body = FontAttributesBody {
            font_color,
            font_size,
            font_type,
            font_style,
        };
        self.add_object(create_font_attributes(id, &body))
    }

    /// Add a Fill Attributes object (Type 25). Returns the object id.
    #[pyo3(signature = (id, fill_type=0, fill_color=0, fill_pattern=0xFFFF))]
    fn add_fill_attributes(
        &mut self,
        id: u16,
        fill_type: u8,
        fill_color: u8,
        fill_pattern: u16,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{FillAttributesBody, create_fill_attributes};
        let body = FillAttributesBody {
            fill_type,
            fill_color,
            fill_pattern: fill_pattern.into(),
        };
        self.add_object(create_fill_attributes(id, &body).map_err(err_runtime)?)
    }

    /// Add a Picture Graphic (Type 20). Returns the object id.
    #[pyo3(signature = (
        id, width=0, actual_width=0, actual_height=0, format=0, options=0,
        transparency=0, data=Vec::new()
    ))]
    #[allow(clippy::too_many_arguments)]
    fn add_picture_graphic(
        &mut self,
        id: u16,
        width: u16,
        actual_width: u16,
        actual_height: u16,
        format: u8,
        options: u8,
        transparency: u8,
        data: Vec<u8>,
    ) -> PyResult<u16> {
        use crate::isobus::vt::{PictureGraphicBody, create_picture_graphic};
        let body = PictureGraphicBody {
            width,
            actual_width,
            actual_height,
            format,
            options,
            transparency,
            data,
        };
        self.add_object(create_picture_graphic(id, &body).map_err(err_runtime)?)
    }
}

// ─── TC DDOP builder ──────────────────────────────────────────────────

/// Opaque builder for an ISO 11783-10 Device Descriptor Object Pool
/// ([`crate::isobus::tc::DDOP`]).
///
/// Add objects with the typed `add_*` helpers (each returns the assigned
/// ObjectID) and pass the finished DDOP to `Session(ddop=...)`.
#[pyclass(name = "Ddop")]
#[derive(Clone, Default)]
pub struct Ddop {
    inner: crate::isobus::tc::DDOP,
}

#[pymethods]
impl Ddop {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Add a Device object. Returns the assigned ObjectID.
    #[pyo3(signature = (designator, software_version=String::new(), serial_number=String::new(), id=0))]
    fn add_device(
        &mut self,
        designator: String,
        software_version: String,
        serial_number: String,
        id: u16,
    ) -> PyResult<u16> {
        use crate::isobus::tc::DeviceObject;
        let obj = DeviceObject::default()
            .with_id(id)
            .with_designator(designator)
            .with_software_version(software_version)
            .with_serial_number(serial_number);
        self.inner
            .add_device(obj)
            .map(u16::from)
            .map_err(err_runtime)
    }

    /// Add a Device Element. `element_type` is the raw type byte
    /// (1=Device, 2=Function, 3=Bin, 4=Section, 5=Unit, 6=Connector,
    /// 7=NavigationReference). Returns the assigned ObjectID.
    #[pyo3(signature = (designator, element_type=1, number=0, parent_id=0, children=Vec::new(), id=0))]
    fn add_element(
        &mut self,
        designator: String,
        element_type: u8,
        number: u16,
        parent_id: u16,
        children: Vec<u16>,
        id: u16,
    ) -> PyResult<u16> {
        use crate::isobus::tc::{DeviceElement, DeviceElementType};
        let ty = match element_type {
            1 => DeviceElementType::Device,
            2 => DeviceElementType::Function,
            3 => DeviceElementType::Bin,
            4 => DeviceElementType::Section,
            5 => DeviceElementType::Unit,
            6 => DeviceElementType::Connector,
            7 => DeviceElementType::NavigationReference,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown device element type {other} (expected 1..=7)"
                )));
            }
        };
        let elem = DeviceElement::default()
            .with_id(id)
            .with_type(ty)
            .with_number(number)
            .with_parent(parent_id)
            .with_designator(designator)
            .with_children(children);
        self.inner
            .add_element(elem)
            .map(u16::from)
            .map_err(err_runtime)
    }

    /// Add a Device Process Data object. `trigger_methods` is the raw
    /// trigger bitmask. `presentation_object_id` of 0xFFFF means none.
    /// Returns the assigned ObjectID.
    #[pyo3(signature = (ddi, designator=String::new(), trigger_methods=0, presentation_object_id=0xFFFF, id=0))]
    fn add_process_data(
        &mut self,
        ddi: u16,
        designator: String,
        trigger_methods: u8,
        presentation_object_id: u16,
        id: u16,
    ) -> PyResult<u16> {
        use crate::isobus::tc::DeviceProcessData;
        let pd = DeviceProcessData::default()
            .with_id(id)
            .with_ddi(ddi)
            .with_triggers(trigger_methods)
            .with_presentation(presentation_object_id)
            .with_designator(designator);
        self.inner
            .add_process_data(pd)
            .map(u16::from)
            .map_err(err_runtime)
    }

    /// Add a Device Property object. `presentation_object_id` of 0xFFFF
    /// means none. Returns the assigned ObjectID.
    #[pyo3(signature = (ddi, value=0, designator=String::new(), presentation_object_id=0xFFFF, id=0))]
    fn add_property(
        &mut self,
        ddi: u16,
        value: i32,
        designator: String,
        presentation_object_id: u16,
        id: u16,
    ) -> PyResult<u16> {
        use crate::isobus::tc::DeviceProperty;
        let prop = DeviceProperty::default()
            .with_id(id)
            .with_ddi(ddi)
            .with_value(value)
            .with_presentation(presentation_object_id)
            .with_designator(designator);
        self.inner
            .add_property(prop)
            .map(u16::from)
            .map_err(err_runtime)
    }

    /// Add a Device Value Presentation object. Returns the assigned ObjectID.
    #[pyo3(signature = (offset=0, scale=1.0, decimal_digits=0, unit_designator=String::new(), id=0))]
    fn add_value_presentation(
        &mut self,
        offset: i32,
        scale: f32,
        decimal_digits: u8,
        unit_designator: String,
        id: u16,
    ) -> PyResult<u16> {
        use crate::isobus::tc::DeviceValuePresentation;
        let vp = DeviceValuePresentation::default()
            .with_id(id)
            .with_offset(offset)
            .with_scale(scale)
            .with_decimals(decimal_digits)
            .with_unit(unit_designator);
        self.inner
            .add_value_presentation(vp)
            .map(u16::from)
            .map_err(err_runtime)
    }

    /// Serialize the DDOP to ISO 11783-10 object-pool bytes.
    fn serialize(&self) -> PyResult<Vec<u8>> {
        self.inner.serialize().map_err(err_runtime)
    }
}

fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySession>()?;
    module.add_class::<VtPool>()?;
    module.add_class::<Ddop>()?;
    module.add_class::<PyIdentifier>()?;
    module.add_class::<PyEec1>()?;
    // j1939::engine
    module.add_class::<PyEec2>()?;
    module.add_class::<PyEec3>()?;
    module.add_class::<PyEngineTemp1>()?;
    module.add_class::<PyEngineTemp2>()?;
    module.add_class::<PyEngineFluidLp>()?;
    module.add_class::<PyEngineHours>()?;
    module.add_class::<PyFuelEconomy>()?;
    module.add_class::<PyTsc1>()?;
    module.add_class::<PyVep1>()?;
    module.add_class::<PyAmbientConditions>()?;
    module.add_class::<PyDashDisplay>()?;
    module.add_class::<PyVehiclePosition>()?;
    module.add_class::<PyFuelConsumption>()?;
    module.add_class::<PyAftertreatment1>()?;
    module.add_class::<PyAftertreatment2>()?;
    module.add_class::<PyComponentIdentification>()?;
    module.add_class::<PyVehicleIdentification>()?;
    // j1939::diagnostic
    module.add_class::<PyFmi>()?;
    module.add_class::<PyDtc>()?;
    module.add_class::<PyPreviouslyActiveDtc>()?;
    module.add_class::<PyDiagnosticLamps>()?;
    module.add_class::<PyDmDtcList>()?;
    module.add_class::<PyDmClearAllRequest>()?;
    module.add_class::<PyDm4Message>()?;
    module.add_class::<PyDm7Command>()?;
    module.add_class::<PyDm8TestResult>()?;
    module.add_class::<PyDm13Signals>()?;
    module.add_class::<PyDm21Readiness>()?;
    module.add_class::<PyDm22Message>()?;
    module.add_class::<PyDm9VehicleIdentificationRequest>()?;
    module.add_class::<PyDm10VehicleIdentification>()?;
    module.add_class::<PyProductIdentification>()?;
    module.add_class::<PySoftwareIdentification>()?;
    module.add_class::<PyMonitorPerformanceRatio>()?;
    module.add_class::<PyDm20Response>()?;
    module.add_class::<PySpnSnapshot>()?;
    module.add_class::<PyFreezeFrame>()?;
    module.add_class::<PyDm25Request>()?;
    // j1939::dm_memory
    module.add_class::<PyDm14Request>()?;
    module.add_class::<PyDm15Response>()?;
    module.add_class::<PyDm16Transfer>()?;
    module.add_class::<PyEcuIdentification>()?;
    // j1939 misc
    module.add_class::<PyAcknowledgment>()?;
    module.add_class::<PyLanguageData>()?;
    module.add_class::<PyMaintainPowerData>()?;
    module.add_class::<PySpeedAndDistance>()?;
    module.add_class::<PyEtc1>()?;
    module.add_class::<PyTransmissionOilTemp>()?;
    module.add_class::<PyCruiseControl>()?;
    module.add_class::<PyShortcutButtonMessage>()?;
    module.add_class::<PyTimeDate>()?;
    module.add_class::<PyRequest2Msg>()?;
    module.add_class::<PyTransferMsg>()?;
    // nmea
    module.add_class::<PyGnssPosition>()?;
    module.add_class::<PyNMEAInterface>()?;
    module.add_class::<PyWindData>()?;
    module.add_class::<PyTemperatureData>()?;
    module.add_class::<PyPressureData>()?;
    module.add_class::<PyEngineData>()?;
    module.add_class::<PyWaterDepthData>()?;
    module.add_class::<PySpeedWaterData>()?;
    module.add_class::<PySystemTimeData>()?;
    module.add_class::<PyRateOfTurnData>()?;
    module.add_class::<PyAttitudeData>()?;
    module.add_class::<PyMagneticVariationData>()?;
    module.add_class::<PyRudderData>()?;
    module.add_class::<PyFluidLevelData>()?;
    module.add_class::<PyBatteryStatusData>()?;
    module.add_class::<PyGNSSDOPData>()?;
    module.add_class::<PyXTEData>()?;
    module.add_class::<PyNavigationData>()?;
    module.add_class::<PyNmeaDecoder>()?;
    module.add_function(wrap_pyfunction!(py_name, module)?)?;
    module.add_function(wrap_pyfunction!(py_validate_can_bus_config, module)?)?;
    module.add_function(wrap_pyfunction!(py_enforce_iso_can_config, module)?)?;
    Ok(())
}

#[pymodule]
fn machbus(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register(module)
}
