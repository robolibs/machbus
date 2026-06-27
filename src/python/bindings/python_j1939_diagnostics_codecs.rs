#[pymethods]
impl PyEec1 {
    #[new]
    #[pyo3(signature = (engine_speed_rpm=0.0, driver_demand_percent=0.0, actual_engine_percent=0.0, engine_torque_percent=0.0, starter_mode=0, source_address=0))]
    fn new(
        engine_speed_rpm: f64,
        driver_demand_percent: f64,
        actual_engine_percent: f64,
        engine_torque_percent: f64,
        starter_mode: u8,
        source_address: u8,
    ) -> Self {
        Self {
            engine_torque_percent,
            driver_demand_percent,
            actual_engine_percent,
            engine_speed_rpm,
            starter_mode,
            source_address,
        }
    }

    /// Decode an 8-byte EEC1 payload. Returns `None` on an invalid payload.
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Eec1::decode(&data).map(|e| Self {
            engine_torque_percent: e.engine_torque_percent,
            driver_demand_percent: e.driver_demand_percent,
            actual_engine_percent: e.actual_engine_percent,
            engine_speed_rpm: e.engine_speed_rpm,
            starter_mode: e.starter_mode,
            source_address: e.source_address,
        })
    }

    /// Encode to the 8-byte wire payload.
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Eec1 {
            engine_torque_percent: self.engine_torque_percent,
            driver_demand_percent: self.driver_demand_percent,
            actual_engine_percent: self.actual_engine_percent,
            engine_speed_rpm: self.engine_speed_rpm,
            starter_mode: self.starter_mode,
            source_address: self.source_address,
        }
        .encode()
        .to_vec()
    }

    fn __repr__(&self) -> String {
        format!(
            "Eec1(engine_speed_rpm={:.1}, driver_demand_percent={:.1})",
            self.engine_speed_rpm, self.driver_demand_percent
        )
    }
}

// ── j1939::engine codecs (besides Eec1, already defined above) ────────

/// EEC2 — accelerator pedal / engine load (PGN 0x0F003).
#[pyclass(name = "Eec2", get_all, set_all)]
#[derive(Clone)]
pub struct PyEec2 {
    pub accel_pedal_position: u8,
    pub engine_load_percent: f64,
    pub accel_pedal_low_idle: u8,
    pub accel_pedal_kickdown: u8,
    pub road_speed_limit: u8,
}

#[pymethods]
impl PyEec2 {
    #[new]
    #[pyo3(signature = (accel_pedal_position=0xFF, engine_load_percent=0.0, accel_pedal_low_idle=0x03, accel_pedal_kickdown=0x03, road_speed_limit=0xFF))]
    fn new(
        accel_pedal_position: u8,
        engine_load_percent: f64,
        accel_pedal_low_idle: u8,
        accel_pedal_kickdown: u8,
        road_speed_limit: u8,
    ) -> Self {
        Self {
            accel_pedal_position,
            engine_load_percent,
            accel_pedal_low_idle,
            accel_pedal_kickdown,
            road_speed_limit,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Eec2::decode(&data).map(|e| Self {
            accel_pedal_position: e.accel_pedal_position,
            engine_load_percent: e.engine_load_percent,
            accel_pedal_low_idle: e.accel_pedal_low_idle,
            accel_pedal_kickdown: e.accel_pedal_kickdown,
            road_speed_limit: e.road_speed_limit,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Eec2 {
            accel_pedal_position: self.accel_pedal_position,
            engine_load_percent: self.engine_load_percent,
            accel_pedal_low_idle: self.accel_pedal_low_idle,
            accel_pedal_kickdown: self.accel_pedal_kickdown,
            road_speed_limit: self.road_speed_limit,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "Eec2(engine_load_percent={:.1}, accel_pedal_position={})",
            self.engine_load_percent, self.accel_pedal_position
        )
    }
}

/// EEC3 (PGN 0x0FEC0).
#[pyclass(name = "Eec3", get_all, set_all)]
#[derive(Clone)]
pub struct PyEec3 {
    pub nominal_friction_percent: f64,
    pub desired_operating_speed_rpm: f64,
    pub operating_speed_asymmetry: u8,
}

#[pymethods]
impl PyEec3 {
    #[new]
    #[pyo3(signature = (nominal_friction_percent=0.0, desired_operating_speed_rpm=0.0, operating_speed_asymmetry=0xFF))]
    fn new(
        nominal_friction_percent: f64,
        desired_operating_speed_rpm: f64,
        operating_speed_asymmetry: u8,
    ) -> Self {
        Self {
            nominal_friction_percent,
            desired_operating_speed_rpm,
            operating_speed_asymmetry,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Eec3::decode(&data).map(|e| Self {
            nominal_friction_percent: e.nominal_friction_percent,
            desired_operating_speed_rpm: e.desired_operating_speed_rpm,
            operating_speed_asymmetry: e.operating_speed_asymmetry,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Eec3 {
            nominal_friction_percent: self.nominal_friction_percent,
            desired_operating_speed_rpm: self.desired_operating_speed_rpm,
            operating_speed_asymmetry: self.operating_speed_asymmetry,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "Eec3(desired_operating_speed_rpm={:.1})",
            self.desired_operating_speed_rpm
        )
    }
}

/// EngineTemp1 (ET1, PGN 0x0FEEE).
#[pyclass(name = "EngineTemp1", get_all, set_all)]
#[derive(Clone)]
pub struct PyEngineTemp1 {
    pub coolant_temp_c: f64,
    pub fuel_temp_c: f64,
    pub oil_temp_c: f64,
    pub turbo_oil_temp_c: f64,
    pub intercooler_temp_c: f64,
}

#[pymethods]
impl PyEngineTemp1 {
    #[new]
    #[pyo3(signature = (coolant_temp_c=-40.0, fuel_temp_c=-40.0, oil_temp_c=-40.0, turbo_oil_temp_c=-40.0, intercooler_temp_c=-40.0))]
    fn new(
        coolant_temp_c: f64,
        fuel_temp_c: f64,
        oil_temp_c: f64,
        turbo_oil_temp_c: f64,
        intercooler_temp_c: f64,
    ) -> Self {
        Self {
            coolant_temp_c,
            fuel_temp_c,
            oil_temp_c,
            turbo_oil_temp_c,
            intercooler_temp_c,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::EngineTemp1::decode(&data).map(|e| Self {
            coolant_temp_c: e.coolant_temp_c,
            fuel_temp_c: e.fuel_temp_c,
            oil_temp_c: e.oil_temp_c,
            turbo_oil_temp_c: e.turbo_oil_temp_c,
            intercooler_temp_c: e.intercooler_temp_c,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::EngineTemp1 {
            coolant_temp_c: self.coolant_temp_c,
            fuel_temp_c: self.fuel_temp_c,
            oil_temp_c: self.oil_temp_c,
            turbo_oil_temp_c: self.turbo_oil_temp_c,
            intercooler_temp_c: self.intercooler_temp_c,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("EngineTemp1(coolant_temp_c={:.1})", self.coolant_temp_c)
    }
}

/// EngineTemp2 (ET2, PGN 0x0FEED).
#[pyclass(name = "EngineTemp2", get_all, set_all)]
#[derive(Clone)]
pub struct PyEngineTemp2 {
    pub engine_oil_temp_c: f64,
    pub turbo_oil_temp_c: f64,
    pub engine_intercooler_temp_c: f64,
    pub turbo_1_temp_c: f64,
}

#[pymethods]
impl PyEngineTemp2 {
    #[new]
    #[pyo3(signature = (engine_oil_temp_c=-40.0, turbo_oil_temp_c=-40.0, engine_intercooler_temp_c=-40.0, turbo_1_temp_c=-40.0))]
    fn new(
        engine_oil_temp_c: f64,
        turbo_oil_temp_c: f64,
        engine_intercooler_temp_c: f64,
        turbo_1_temp_c: f64,
    ) -> Self {
        Self {
            engine_oil_temp_c,
            turbo_oil_temp_c,
            engine_intercooler_temp_c,
            turbo_1_temp_c,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::EngineTemp2::decode(&data).map(|e| Self {
            engine_oil_temp_c: e.engine_oil_temp_c,
            turbo_oil_temp_c: e.turbo_oil_temp_c,
            engine_intercooler_temp_c: e.engine_intercooler_temp_c,
            turbo_1_temp_c: e.turbo_1_temp_c,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::EngineTemp2 {
            engine_oil_temp_c: self.engine_oil_temp_c,
            turbo_oil_temp_c: self.turbo_oil_temp_c,
            engine_intercooler_temp_c: self.engine_intercooler_temp_c,
            turbo_1_temp_c: self.turbo_1_temp_c,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "EngineTemp2(engine_oil_temp_c={:.1})",
            self.engine_oil_temp_c
        )
    }
}

/// EngineFluidLP (PGN 0x0FEEF).
#[pyclass(name = "EngineFluidLp", get_all, set_all)]
#[derive(Clone)]
pub struct PyEngineFluidLp {
    pub oil_pressure_kpa: f64,
    pub coolant_pressure_kpa: f64,
    pub oil_level_percent: u8,
    pub coolant_level_percent: u8,
    pub fuel_delivery_pressure_kpa: f64,
    pub crankcase_pressure_kpa: f64,
}

#[pymethods]
impl PyEngineFluidLp {
    #[new]
    #[pyo3(signature = (oil_pressure_kpa=0.0, coolant_pressure_kpa=0.0, oil_level_percent=0xFF, coolant_level_percent=0xFF, fuel_delivery_pressure_kpa=0.0, crankcase_pressure_kpa=0.0))]
    fn new(
        oil_pressure_kpa: f64,
        coolant_pressure_kpa: f64,
        oil_level_percent: u8,
        coolant_level_percent: u8,
        fuel_delivery_pressure_kpa: f64,
        crankcase_pressure_kpa: f64,
    ) -> Self {
        Self {
            oil_pressure_kpa,
            coolant_pressure_kpa,
            oil_level_percent,
            coolant_level_percent,
            fuel_delivery_pressure_kpa,
            crankcase_pressure_kpa,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::EngineFluidLp::decode(&data).map(|e| Self {
            oil_pressure_kpa: e.oil_pressure_kpa,
            coolant_pressure_kpa: e.coolant_pressure_kpa,
            oil_level_percent: e.oil_level_percent,
            coolant_level_percent: e.coolant_level_percent,
            fuel_delivery_pressure_kpa: e.fuel_delivery_pressure_kpa,
            crankcase_pressure_kpa: e.crankcase_pressure_kpa,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::EngineFluidLp {
            oil_pressure_kpa: self.oil_pressure_kpa,
            coolant_pressure_kpa: self.coolant_pressure_kpa,
            oil_level_percent: self.oil_level_percent,
            coolant_level_percent: self.coolant_level_percent,
            fuel_delivery_pressure_kpa: self.fuel_delivery_pressure_kpa,
            crankcase_pressure_kpa: self.crankcase_pressure_kpa,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "EngineFluidLp(oil_pressure_kpa={:.1})",
            self.oil_pressure_kpa
        )
    }
}

/// EngineHours (PGN 0x0FEE5).
#[pyclass(name = "EngineHours", get_all, set_all)]
#[derive(Clone)]
pub struct PyEngineHours {
    pub total_hours: f64,
    pub total_revolutions: f64,
}

#[pymethods]
impl PyEngineHours {
    #[new]
    #[pyo3(signature = (total_hours=0.0, total_revolutions=0.0))]
    fn new(total_hours: f64, total_revolutions: f64) -> Self {
        Self {
            total_hours,
            total_revolutions,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::EngineHours::decode(&data).map(|e| Self {
            total_hours: e.total_hours,
            total_revolutions: e.total_revolutions,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::EngineHours {
            total_hours: self.total_hours,
            total_revolutions: self.total_revolutions,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("EngineHours(total_hours={:.2})", self.total_hours)
    }
}

/// FuelEconomy (PGN 0x0FEF2).
#[pyclass(name = "FuelEconomy", get_all, set_all)]
#[derive(Clone)]
pub struct PyFuelEconomy {
    pub fuel_rate_lph: f64,
    pub instantaneous_lph: f64,
    pub throttle_position: f64,
}

#[pymethods]
impl PyFuelEconomy {
    #[new]
    #[pyo3(signature = (fuel_rate_lph=0.0, instantaneous_lph=0.0, throttle_position=0.0))]
    fn new(fuel_rate_lph: f64, instantaneous_lph: f64, throttle_position: f64) -> Self {
        Self {
            fuel_rate_lph,
            instantaneous_lph,
            throttle_position,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::FuelEconomy::decode(&data).map(|e| Self {
            fuel_rate_lph: e.fuel_rate_lph,
            instantaneous_lph: e.instantaneous_lph,
            throttle_position: e.throttle_position,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::FuelEconomy {
            fuel_rate_lph: self.fuel_rate_lph,
            instantaneous_lph: self.instantaneous_lph,
            throttle_position: self.throttle_position,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("FuelEconomy(fuel_rate_lph={:.2})", self.fuel_rate_lph)
    }
}

/// TSC1 — torque/speed control 1 (PGN 0x0F006). `override_mode` is the raw
/// 2-bit override-control-mode value (0=none, 1=speed, 2=torque, 3=limit).
#[pyclass(name = "Tsc1", get_all, set_all)]
#[derive(Clone)]
pub struct PyTsc1 {
    pub override_mode: u8,
    pub requested_speed_rpm: f64,
    pub requested_torque_percent: f64,
}

#[pymethods]
impl PyTsc1 {
    #[new]
    #[pyo3(signature = (override_mode=0, requested_speed_rpm=0.0, requested_torque_percent=0.0))]
    fn new(override_mode: u8, requested_speed_rpm: f64, requested_torque_percent: f64) -> Self {
        Self {
            override_mode,
            requested_speed_rpm,
            requested_torque_percent,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Tsc1::decode(&data).map(|e| Self {
            override_mode: e.override_mode.as_u8(),
            requested_speed_rpm: e.requested_speed_rpm,
            requested_torque_percent: e.requested_torque_percent,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Tsc1 {
            override_mode: crate::j1939::OverrideControlMode::from_u8(self.override_mode),
            requested_speed_rpm: self.requested_speed_rpm,
            requested_torque_percent: self.requested_torque_percent,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "Tsc1(override_mode={}, requested_speed_rpm={:.1})",
            self.override_mode, self.requested_speed_rpm
        )
    }
}

/// VEP1 — vehicle electrical power (PGN 0x0F009).
#[pyclass(name = "Vep1", get_all, set_all)]
#[derive(Clone)]
pub struct PyVep1 {
    pub battery_voltage_v: f64,
    pub alternator_current_a: f64,
    pub charging_system_voltage_v: f64,
    pub key_switch_voltage_v: f64,
}

#[pymethods]
impl PyVep1 {
    #[new]
    #[pyo3(signature = (battery_voltage_v=0.0, alternator_current_a=0.0, charging_system_voltage_v=0.0, key_switch_voltage_v=0.0))]
    fn new(
        battery_voltage_v: f64,
        alternator_current_a: f64,
        charging_system_voltage_v: f64,
        key_switch_voltage_v: f64,
    ) -> Self {
        Self {
            battery_voltage_v,
            alternator_current_a,
            charging_system_voltage_v,
            key_switch_voltage_v,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Vep1::decode(&data).map(|e| Self {
            battery_voltage_v: e.battery_voltage_v,
            alternator_current_a: e.alternator_current_a,
            charging_system_voltage_v: e.charging_system_voltage_v,
            key_switch_voltage_v: e.key_switch_voltage_v,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Vep1 {
            battery_voltage_v: self.battery_voltage_v,
            alternator_current_a: self.alternator_current_a,
            charging_system_voltage_v: self.charging_system_voltage_v,
            key_switch_voltage_v: self.key_switch_voltage_v,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("Vep1(battery_voltage_v={:.2})", self.battery_voltage_v)
    }
}

/// AmbientConditions (PGN 0x0FEF5).
#[pyclass(name = "AmbientConditions", get_all, set_all)]
#[derive(Clone)]
pub struct PyAmbientConditions {
    pub barometric_pressure_kpa: f64,
    pub ambient_air_temp_c: f64,
    pub intake_air_temp_c: f64,
    pub road_surface_temp_c: f64,
}

#[pymethods]
impl PyAmbientConditions {
    #[new]
    #[pyo3(signature = (barometric_pressure_kpa=0.0, ambient_air_temp_c=-40.0, intake_air_temp_c=-40.0, road_surface_temp_c=-40.0))]
    fn new(
        barometric_pressure_kpa: f64,
        ambient_air_temp_c: f64,
        intake_air_temp_c: f64,
        road_surface_temp_c: f64,
    ) -> Self {
        Self {
            barometric_pressure_kpa,
            ambient_air_temp_c,
            intake_air_temp_c,
            road_surface_temp_c,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::AmbientConditions::decode(&data).map(|e| Self {
            barometric_pressure_kpa: e.barometric_pressure_kpa,
            ambient_air_temp_c: e.ambient_air_temp_c,
            intake_air_temp_c: e.intake_air_temp_c,
            road_surface_temp_c: e.road_surface_temp_c,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::AmbientConditions {
            barometric_pressure_kpa: self.barometric_pressure_kpa,
            ambient_air_temp_c: self.ambient_air_temp_c,
            intake_air_temp_c: self.intake_air_temp_c,
            road_surface_temp_c: self.road_surface_temp_c,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "AmbientConditions(barometric_pressure_kpa={:.1})",
            self.barometric_pressure_kpa
        )
    }
}

/// DashDisplay (PGN 0x0FEFC).
#[pyclass(name = "DashDisplay", get_all, set_all)]
#[derive(Clone)]
pub struct PyDashDisplay {
    pub fuel_level_percent: u8,
    pub washer_fluid_level: u8,
    pub fuel_filter_diff_kpa: f64,
    pub oil_filter_diff_kpa: f64,
    pub cargo_ambient_temp_c: f64,
}

#[pymethods]
impl PyDashDisplay {
    #[new]
    #[pyo3(signature = (fuel_level_percent=0xFF, washer_fluid_level=0xFF, fuel_filter_diff_kpa=0.0, oil_filter_diff_kpa=0.0, cargo_ambient_temp_c=-40.0))]
    fn new(
        fuel_level_percent: u8,
        washer_fluid_level: u8,
        fuel_filter_diff_kpa: f64,
        oil_filter_diff_kpa: f64,
        cargo_ambient_temp_c: f64,
    ) -> Self {
        Self {
            fuel_level_percent,
            washer_fluid_level,
            fuel_filter_diff_kpa,
            oil_filter_diff_kpa,
            cargo_ambient_temp_c,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::DashDisplay::decode(&data).map(|e| Self {
            fuel_level_percent: e.fuel_level_percent,
            washer_fluid_level: e.washer_fluid_level,
            fuel_filter_diff_kpa: e.fuel_filter_diff_kpa,
            oil_filter_diff_kpa: e.oil_filter_diff_kpa,
            cargo_ambient_temp_c: e.cargo_ambient_temp_c,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::DashDisplay {
            fuel_level_percent: self.fuel_level_percent,
            washer_fluid_level: self.washer_fluid_level,
            fuel_filter_diff_kpa: self.fuel_filter_diff_kpa,
            oil_filter_diff_kpa: self.oil_filter_diff_kpa,
            cargo_ambient_temp_c: self.cargo_ambient_temp_c,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "DashDisplay(fuel_level_percent={})",
            self.fuel_level_percent
        )
    }
}

/// VehiclePosition (PGN 0x0FEF7).
#[pyclass(name = "VehiclePosition", get_all, set_all)]
#[derive(Clone)]
pub struct PyVehiclePosition {
    pub latitude_deg: f64,
    pub longitude_deg: f64,
}

#[pymethods]
impl PyVehiclePosition {
    #[new]
    #[pyo3(signature = (latitude_deg=0.0, longitude_deg=0.0))]
    fn new(latitude_deg: f64, longitude_deg: f64) -> Self {
        Self {
            latitude_deg,
            longitude_deg,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::VehiclePosition::decode(&data).map(|e| Self {
            latitude_deg: e.latitude_deg,
            longitude_deg: e.longitude_deg,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::VehiclePosition {
            latitude_deg: self.latitude_deg,
            longitude_deg: self.longitude_deg,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "VehiclePosition(latitude_deg={:.6}, longitude_deg={:.6})",
            self.latitude_deg, self.longitude_deg
        )
    }
}

/// FuelConsumption (PGN 0x0FEE9).
#[pyclass(name = "FuelConsumption", get_all, set_all)]
#[derive(Clone)]
pub struct PyFuelConsumption {
    pub trip_fuel_l: f64,
    pub total_fuel_l: f64,
}

#[pymethods]
impl PyFuelConsumption {
    #[new]
    #[pyo3(signature = (trip_fuel_l=0.0, total_fuel_l=0.0))]
    fn new(trip_fuel_l: f64, total_fuel_l: f64) -> Self {
        Self {
            trip_fuel_l,
            total_fuel_l,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::FuelConsumption::decode(&data).map(|e| Self {
            trip_fuel_l: e.trip_fuel_l,
            total_fuel_l: e.total_fuel_l,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::FuelConsumption {
            trip_fuel_l: self.trip_fuel_l,
            total_fuel_l: self.total_fuel_l,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("FuelConsumption(total_fuel_l={:.1})", self.total_fuel_l)
    }
}

/// Aftertreatment 1 (AT1).
#[pyclass(name = "Aftertreatment1", get_all, set_all)]
#[derive(Clone)]
pub struct PyAftertreatment1 {
    pub def_tank_level: f64,
    pub intake_nox_ppm: f64,
    pub outlet_nox_ppm: f64,
    pub intake_nox_reading_status: u8,
    pub outlet_nox_reading_status: u8,
}

#[pymethods]
impl PyAftertreatment1 {
    #[new]
    #[pyo3(signature = (def_tank_level=0.0, intake_nox_ppm=0.0, outlet_nox_ppm=0.0, intake_nox_reading_status=0, outlet_nox_reading_status=0))]
    fn new(
        def_tank_level: f64,
        intake_nox_ppm: f64,
        outlet_nox_ppm: f64,
        intake_nox_reading_status: u8,
        outlet_nox_reading_status: u8,
    ) -> Self {
        Self {
            def_tank_level,
            intake_nox_ppm,
            outlet_nox_ppm,
            intake_nox_reading_status,
            outlet_nox_reading_status,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Aftertreatment1::decode(&data).map(|e| Self {
            def_tank_level: e.def_tank_level,
            intake_nox_ppm: e.intake_nox_ppm,
            outlet_nox_ppm: e.outlet_nox_ppm,
            intake_nox_reading_status: e.intake_nox_reading_status,
            outlet_nox_reading_status: e.outlet_nox_reading_status,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Aftertreatment1 {
            def_tank_level: self.def_tank_level,
            intake_nox_ppm: self.intake_nox_ppm,
            outlet_nox_ppm: self.outlet_nox_ppm,
            intake_nox_reading_status: self.intake_nox_reading_status,
            outlet_nox_reading_status: self.outlet_nox_reading_status,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("Aftertreatment1(def_tank_level={:.1})", self.def_tank_level)
    }
}

/// Aftertreatment 2 (AT2, PGN 65110).
#[pyclass(name = "Aftertreatment2", get_all, set_all)]
#[derive(Clone)]
pub struct PyAftertreatment2 {
    pub dpf_differential_pressure_kpa: f64,
    pub def_concentration: f64,
    pub dpf_soot_load_percent: f64,
    pub dpf_active_regeneration_status: u8,
    pub dpf_passive_regeneration_status: u8,
}

#[pymethods]
impl PyAftertreatment2 {
    #[new]
    #[pyo3(signature = (dpf_differential_pressure_kpa=0.0, def_concentration=0.0, dpf_soot_load_percent=0.0, dpf_active_regeneration_status=0, dpf_passive_regeneration_status=0))]
    fn new(
        dpf_differential_pressure_kpa: f64,
        def_concentration: f64,
        dpf_soot_load_percent: f64,
        dpf_active_regeneration_status: u8,
        dpf_passive_regeneration_status: u8,
    ) -> Self {
        Self {
            dpf_differential_pressure_kpa,
            def_concentration,
            dpf_soot_load_percent,
            dpf_active_regeneration_status,
            dpf_passive_regeneration_status,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Aftertreatment2::decode(&data).map(|e| Self {
            dpf_differential_pressure_kpa: e.dpf_differential_pressure_kpa,
            def_concentration: e.def_concentration,
            dpf_soot_load_percent: e.dpf_soot_load_percent,
            dpf_active_regeneration_status: e.dpf_active_regeneration_status,
            dpf_passive_regeneration_status: e.dpf_passive_regeneration_status,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Aftertreatment2 {
            dpf_differential_pressure_kpa: self.dpf_differential_pressure_kpa,
            def_concentration: self.def_concentration,
            dpf_soot_load_percent: self.dpf_soot_load_percent,
            dpf_active_regeneration_status: self.dpf_active_regeneration_status,
            dpf_passive_regeneration_status: self.dpf_passive_regeneration_status,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "Aftertreatment2(dpf_soot_load_percent={:.1})",
            self.dpf_soot_load_percent
        )
    }
}

/// ComponentIdentification (PGN 0x0FEEB).
#[pyclass(name = "ComponentIdentification", get_all, set_all)]
#[derive(Clone)]
pub struct PyComponentIdentification {
    pub make: String,
    pub model: String,
    pub serial_number: String,
    pub unit_number: String,
}

#[pymethods]
impl PyComponentIdentification {
    #[new]
    #[pyo3(signature = (make=String::new(), model=String::new(), serial_number=String::new(), unit_number=String::new()))]
    fn new(make: String, model: String, serial_number: String, unit_number: String) -> Self {
        Self {
            make,
            model,
            serial_number,
            unit_number,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::ComponentIdentification::decode(&data).map(|e| Self {
            make: e.make,
            model: e.model,
            serial_number: e.serial_number,
            unit_number: e.unit_number,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::ComponentIdentification {
            make: self.make.clone(),
            model: self.model.clone(),
            serial_number: self.serial_number.clone(),
            unit_number: self.unit_number.clone(),
        }
        .encode()
    }
    fn __repr__(&self) -> String {
        format!(
            "ComponentIdentification(make={:?}, model={:?})",
            self.make, self.model
        )
    }
}

/// VehicleIdentification (PGN 0x0FEEC).
#[pyclass(name = "VehicleIdentification", get_all, set_all)]
#[derive(Clone)]
pub struct PyVehicleIdentification {
    pub vin: String,
}

#[pymethods]
impl PyVehicleIdentification {
    #[new]
    #[pyo3(signature = (vin=String::new()))]
    fn new(vin: String) -> Self {
        Self { vin }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::VehicleIdentification::decode(&data).map(|e| Self { vin: e.vin })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::VehicleIdentification {
            vin: self.vin.clone(),
        }
        .encode()
    }
    fn __repr__(&self) -> String {
        format!("VehicleIdentification(vin={:?})", self.vin)
    }
}

// ── j1939::diagnostic codecs ──────────────────────────────────────────

/// FMI — failure-mode identifier (J1939-73). Integer-valued enum mirror.
#[pyclass(name = "Fmi", eq, eq_int)]
#[derive(Clone, Copy, PartialEq)]
pub enum PyFmi {
    AboveNormal = 0,
    BelowNormal = 1,
    Erratic = 2,
    VoltageHigh = 3,
    VoltageLow = 4,
    CurrentLow = 5,
    CurrentHigh = 6,
    MechanicalFail = 7,
    AbnormalFrequency = 8,
    AbnormalUpdate = 9,
    AbnormalRateChange = 10,
    RootCauseUnknown = 11,
    BadDevice = 12,
    OutOfCalibration = 13,
    SpecialInstructions = 14,
    AboveNormalLeast = 15,
    AboveNormalModerate = 16,
    BelowNormalLeast = 17,
    BelowNormalModerate = 18,
    ReceivedNetworkData = 19,
    DataDriftedHigh = 20,
    DataDriftedLow = 21,
    ConditionExists = 31,
}

#[pymethods]
impl PyFmi {
    /// Build a `Fmi` from its raw J1939 value (`0..=31`); unknown values map to
    /// `RootCauseUnknown`.
    #[staticmethod]
    fn from_raw(value: u8) -> Self {
        match Fmi::from_u8(value).as_u8() {
            0 => Self::AboveNormal,
            1 => Self::BelowNormal,
            2 => Self::Erratic,
            3 => Self::VoltageHigh,
            4 => Self::VoltageLow,
            5 => Self::CurrentLow,
            6 => Self::CurrentHigh,
            7 => Self::MechanicalFail,
            8 => Self::AbnormalFrequency,
            9 => Self::AbnormalUpdate,
            10 => Self::AbnormalRateChange,
            12 => Self::BadDevice,
            13 => Self::OutOfCalibration,
            14 => Self::SpecialInstructions,
            15 => Self::AboveNormalLeast,
            16 => Self::AboveNormalModerate,
            17 => Self::BelowNormalLeast,
            18 => Self::BelowNormalModerate,
            19 => Self::ReceivedNetworkData,
            20 => Self::DataDriftedHigh,
            21 => Self::DataDriftedLow,
            31 => Self::ConditionExists,
            _ => Self::RootCauseUnknown,
        }
    }
    /// The raw J1939 5-bit FMI value for this variant.
    fn as_raw(&self) -> u8 {
        *self as u8
    }
}

/// 4-byte DTC: 19-bit SPN, 5-bit FMI, 7-bit occurrence count. `fmi` is the
/// raw 5-bit value; use the `Fmi` enum to interpret it.
#[pyclass(name = "Dtc", get_all, set_all)]
#[derive(Clone)]
pub struct PyDtc {
    pub spn: u32,
    pub fmi: u8,
    pub occurrence_count: u8,
}

impl PyDtc {
    fn to_rust(&self) -> Dtc {
        Dtc {
            spn: self.spn,
            fmi: Fmi::from_u8(self.fmi),
            occurrence_count: self.occurrence_count,
        }
    }
    fn from_rust(d: Dtc) -> Self {
        Self {
            spn: d.spn,
            fmi: d.fmi.as_u8(),
            occurrence_count: d.occurrence_count,
        }
    }
}

#[pymethods]
impl PyDtc {
    #[new]
    #[pyo3(signature = (spn=0, fmi=11, occurrence_count=0))]
    fn new(spn: u32, fmi: u8, occurrence_count: u8) -> Self {
        Self {
            spn,
            fmi,
            occurrence_count,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        Dtc::decode(&data).map(Self::from_rust)
    }
    fn encode(&self) -> Vec<u8> {
        self.to_rust().encode().to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "Dtc(spn={}, fmi={}, occurrence_count={})",
            self.spn, self.fmi, self.occurrence_count
        )
    }
}

/// Tracks a DTC's occurrence count after it was cleared from the active list.
#[pyclass(name = "PreviouslyActiveDtc", get_all, set_all)]
#[derive(Clone)]
pub struct PyPreviouslyActiveDtc {
    pub dtc: PyDtc,
    pub occurrence_count: u8,
}

#[pymethods]
impl PyPreviouslyActiveDtc {
    #[new]
    #[pyo3(signature = (dtc, occurrence_count=0))]
    fn new(dtc: PyDtc, occurrence_count: u8) -> Self {
        Self {
            dtc,
            occurrence_count,
        }
    }
    fn __repr__(&self) -> String {
        format!(
            "PreviouslyActiveDtc(occurrence_count={})",
            self.occurrence_count
        )
    }
}

/// 2-byte lamp status block (DM1/2/3/6/12/23). Each field is a raw 2-bit value.
#[pyclass(name = "DiagnosticLamps", get_all, set_all)]
#[derive(Clone)]
pub struct PyDiagnosticLamps {
    pub malfunction: u8,
    pub malfunction_flash: u8,
    pub red_stop: u8,
    pub red_stop_flash: u8,
    pub amber_warning: u8,
    pub amber_warning_flash: u8,
    pub engine_protect: u8,
    pub engine_protect_flash: u8,
}

impl PyDiagnosticLamps {
    fn to_rust(&self) -> crate::j1939::DiagnosticLamps {
        crate::j1939::DiagnosticLamps {
            malfunction: crate::j1939::LampStatus::from_u8(self.malfunction),
            malfunction_flash: crate::j1939::LampFlash::from_u8(self.malfunction_flash),
            red_stop: crate::j1939::LampStatus::from_u8(self.red_stop),
            red_stop_flash: crate::j1939::LampFlash::from_u8(self.red_stop_flash),
            amber_warning: crate::j1939::LampStatus::from_u8(self.amber_warning),
            amber_warning_flash: crate::j1939::LampFlash::from_u8(self.amber_warning_flash),
            engine_protect: crate::j1939::LampStatus::from_u8(self.engine_protect),
            engine_protect_flash: crate::j1939::LampFlash::from_u8(self.engine_protect_flash),
        }
    }
    fn from_rust(l: crate::j1939::DiagnosticLamps) -> Self {
        Self {
            malfunction: l.malfunction.as_u8(),
            malfunction_flash: l.malfunction_flash.as_u8(),
            red_stop: l.red_stop.as_u8(),
            red_stop_flash: l.red_stop_flash.as_u8(),
            amber_warning: l.amber_warning.as_u8(),
            amber_warning_flash: l.amber_warning_flash.as_u8(),
            engine_protect: l.engine_protect.as_u8(),
            engine_protect_flash: l.engine_protect_flash.as_u8(),
        }
    }
}

#[pymethods]
impl PyDiagnosticLamps {
    #[new]
    #[pyo3(signature = (malfunction=0, malfunction_flash=2, red_stop=0, red_stop_flash=2, amber_warning=0, amber_warning_flash=2, engine_protect=0, engine_protect_flash=2))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        malfunction: u8,
        malfunction_flash: u8,
        red_stop: u8,
        red_stop_flash: u8,
        amber_warning: u8,
        amber_warning_flash: u8,
        engine_protect: u8,
        engine_protect_flash: u8,
    ) -> Self {
        Self {
            malfunction,
            malfunction_flash,
            red_stop,
            red_stop_flash,
            amber_warning,
            amber_warning_flash,
            engine_protect,
            engine_protect_flash,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::DiagnosticLamps::decode(&data).map(Self::from_rust)
    }
    fn encode(&self) -> Vec<u8> {
        self.to_rust().encode().to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "DiagnosticLamps(malfunction={}, amber_warning={})",
            self.malfunction, self.amber_warning
        )
    }
}

/// DM1/2 — lamp status + DTC list.
#[pyclass(name = "DmDtcList", get_all, set_all)]
#[derive(Clone)]
pub struct PyDmDtcList {
    pub lamps: PyDiagnosticLamps,
    pub dtcs: Vec<PyDtc>,
}

#[pymethods]
impl PyDmDtcList {
    #[new]
    #[pyo3(signature = (lamps, dtcs=Vec::new()))]
    fn new(lamps: PyDiagnosticLamps, dtcs: Vec<PyDtc>) -> Self {
        Self { lamps, dtcs }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::DmDtcList::decode(&data).map(|m| Self {
            lamps: PyDiagnosticLamps::from_rust(m.lamps),
            dtcs: m.dtcs.into_iter().map(PyDtc::from_rust).collect(),
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::DmDtcList {
            lamps: self.lamps.to_rust(),
            dtcs: self.dtcs.iter().map(PyDtc::to_rust).collect(),
        }
        .encode()
    }
    fn __repr__(&self) -> String {
        format!("DmDtcList(dtcs={})", self.dtcs.len())
    }
}

/// DM3/DM11 clear-all request (no fields).
#[pyclass(name = "DmClearAllRequest")]
#[derive(Clone)]
pub struct PyDmClearAllRequest;

#[pymethods]
impl PyDmClearAllRequest {
    #[new]
    fn new() -> Self {
        Self
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::DmClearAllRequest::decode(&data).map(|_| Self)
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::DmClearAllRequest.encode().to_vec()
    }
    fn __repr__(&self) -> String {
        "DmClearAllRequest()".to_string()
    }
}

/// DM4 — driver's information message.
#[pyclass(name = "Dm4Message", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm4Message {
    pub mil_status: u8,
    pub red_stop_lamp: u8,
    pub amber_warning: u8,
    pub protect_lamp: u8,
    pub dtcs: Vec<PyDtc>,
}

#[pymethods]
impl PyDm4Message {
    #[new]
    #[pyo3(signature = (mil_status=0, red_stop_lamp=0, amber_warning=0, protect_lamp=0, dtcs=Vec::new()))]
    fn new(
        mil_status: u8,
        red_stop_lamp: u8,
        amber_warning: u8,
        protect_lamp: u8,
        dtcs: Vec<PyDtc>,
    ) -> Self {
        Self {
            mil_status,
            red_stop_lamp,
            amber_warning,
            protect_lamp,
            dtcs,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm4Message::decode(&data).map(|m| Self {
            mil_status: m.mil_status.as_u8(),
            red_stop_lamp: m.red_stop_lamp.as_u8(),
            amber_warning: m.amber_warning.as_u8(),
            protect_lamp: m.protect_lamp.as_u8(),
            dtcs: m.dtcs.into_iter().map(PyDtc::from_rust).collect(),
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Dm4Message {
            mil_status: crate::j1939::LampStatus::from_u8(self.mil_status),
            red_stop_lamp: crate::j1939::LampStatus::from_u8(self.red_stop_lamp),
            amber_warning: crate::j1939::LampStatus::from_u8(self.amber_warning),
            protect_lamp: crate::j1939::LampStatus::from_u8(self.protect_lamp),
            dtcs: self.dtcs.iter().map(PyDtc::to_rust).collect(),
        }
        .encode()
    }
    fn __repr__(&self) -> String {
        format!("Dm4Message(dtcs={})", self.dtcs.len())
    }
}

/// DM7 — non-continuous monitor test command.
#[pyclass(name = "Dm7Command", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm7Command {
    pub spn: u32,
    pub test_id: u8,
}

#[pymethods]
impl PyDm7Command {
    #[new]
    #[pyo3(signature = (spn=0, test_id=0))]
    fn new(spn: u32, test_id: u8) -> Self {
        Self { spn, test_id }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm7Command::decode(&data).map(|m| Self {
            spn: m.spn,
            test_id: m.test_id,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Dm7Command {
            spn: self.spn,
            test_id: self.test_id,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("Dm7Command(spn={}, test_id={})", self.spn, self.test_id)
    }
}

/// DM8 — non-continuous monitor test result.
#[pyclass(name = "Dm8TestResult", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm8TestResult {
    pub spn: u32,
    pub test_id: u8,
    pub test_result: u8,
    pub test_value: u16,
    pub test_limit_min: u16,
    pub test_limit_max: u16,
}

#[pymethods]
impl PyDm8TestResult {
    #[new]
    #[pyo3(signature = (spn=0, test_id=0xFF, test_result=0xFF, test_value=0xFFFF, test_limit_min=0xFFFF, test_limit_max=0xFFFF))]
    fn new(
        spn: u32,
        test_id: u8,
        test_result: u8,
        test_value: u16,
        test_limit_min: u16,
        test_limit_max: u16,
    ) -> Self {
        Self {
            spn,
            test_id,
            test_result,
            test_value,
            test_limit_min,
            test_limit_max,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm8TestResult::decode(&data).map(|m| Self {
            spn: m.spn,
            test_id: m.test_id,
            test_result: m.test_result,
            test_value: m.test_value,
            test_limit_min: m.test_limit_min,
            test_limit_max: m.test_limit_max,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Dm8TestResult {
            spn: self.spn,
            test_id: self.test_id,
            test_result: self.test_result,
            test_value: self.test_value,
            test_limit_min: self.test_limit_min,
            test_limit_max: self.test_limit_max,
        }
        .encode()
    }
    fn __repr__(&self) -> String {
        format!(
            "Dm8TestResult(spn={}, test_result={})",
            self.spn, self.test_result
        )
    }
}

/// DM13 — suspend/resume broadcast. Network/signal fields are raw enum values.
#[pyclass(name = "Dm13Signals", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm13Signals {
    pub primary_vehicle_network: u8,
    pub sae_j1922_network: u8,
    pub sae_j1587_network: u8,
    pub current_data_link: u8,
    pub suspend_signal: u8,
    pub suspend_duration_s: u16,
}

#[pymethods]
impl PyDm13Signals {
    #[new]
    #[pyo3(signature = (primary_vehicle_network=3, sae_j1922_network=3, sae_j1587_network=3, current_data_link=3, suspend_signal=15, suspend_duration_s=0xFFFF))]
    fn new(
        primary_vehicle_network: u8,
        sae_j1922_network: u8,
        sae_j1587_network: u8,
        current_data_link: u8,
        suspend_signal: u8,
        suspend_duration_s: u16,
    ) -> Self {
        Self {
            primary_vehicle_network,
            sae_j1922_network,
            sae_j1587_network,
            current_data_link,
            suspend_signal,
            suspend_duration_s,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm13Signals::decode(&data).map(|m| Self {
            primary_vehicle_network: m.primary_vehicle_network.as_u8(),
            sae_j1922_network: m.sae_j1922_network.as_u8(),
            sae_j1587_network: m.sae_j1587_network.as_u8(),
            current_data_link: m.current_data_link.as_u8(),
            suspend_signal: m.suspend_signal.as_u8(),
            suspend_duration_s: m.suspend_duration_s,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        Some(
            crate::j1939::Dm13Signals {
                primary_vehicle_network: crate::j1939::Dm13Command::from_u8(
                    self.primary_vehicle_network,
                ),
                sae_j1922_network: crate::j1939::Dm13Command::from_u8(self.sae_j1922_network),
                sae_j1587_network: crate::j1939::Dm13Command::from_u8(self.sae_j1587_network),
                current_data_link: crate::j1939::Dm13Command::from_u8(self.current_data_link),
                suspend_signal: crate::j1939::Dm13SuspendSignal::from_u8(self.suspend_signal)?,
                suspend_duration_s: self.suspend_duration_s,
            }
            .encode()
            .to_vec(),
        )
    }
    fn __repr__(&self) -> String {
        format!("Dm13Signals(suspend_signal={})", self.suspend_signal)
    }
}

/// DM21 — diagnostic readiness 2.
#[pyclass(name = "Dm21Readiness", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm21Readiness {
    pub distance_with_mil_on_km: u16,
    pub distance_since_codes_cleared_km: u16,
    pub minutes_with_mil_on: u16,
    pub time_since_codes_cleared_min: u16,
    pub comprehensive_component: u8,
    pub fuel_system: u8,
    pub misfire: u8,
}

#[pymethods]
impl PyDm21Readiness {
    #[new]
    #[pyo3(signature = (distance_with_mil_on_km=0xFFFF, distance_since_codes_cleared_km=0xFFFF, minutes_with_mil_on=0xFFFF, time_since_codes_cleared_min=0xFFFF, comprehensive_component=0xFF, fuel_system=0xFF, misfire=0xFF))]
    fn new(
        distance_with_mil_on_km: u16,
        distance_since_codes_cleared_km: u16,
        minutes_with_mil_on: u16,
        time_since_codes_cleared_min: u16,
        comprehensive_component: u8,
        fuel_system: u8,
        misfire: u8,
    ) -> Self {
        Self {
            distance_with_mil_on_km,
            distance_since_codes_cleared_km,
            minutes_with_mil_on,
            time_since_codes_cleared_min,
            comprehensive_component,
            fuel_system,
            misfire,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm21Readiness::decode(&data).map(|m| Self {
            distance_with_mil_on_km: m.distance_with_mil_on_km,
            distance_since_codes_cleared_km: m.distance_since_codes_cleared_km,
            minutes_with_mil_on: m.minutes_with_mil_on,
            time_since_codes_cleared_min: m.time_since_codes_cleared_min,
            comprehensive_component: m.comprehensive_component,
            fuel_system: m.fuel_system,
            misfire: m.misfire,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Dm21Readiness {
            distance_with_mil_on_km: self.distance_with_mil_on_km,
            distance_since_codes_cleared_km: self.distance_since_codes_cleared_km,
            minutes_with_mil_on: self.minutes_with_mil_on,
            time_since_codes_cleared_min: self.time_since_codes_cleared_min,
            comprehensive_component: self.comprehensive_component,
            fuel_system: self.fuel_system,
            misfire: self.misfire,
        }
        .encode()
    }
    fn __repr__(&self) -> String {
        format!(
            "Dm21Readiness(distance_with_mil_on_km={})",
            self.distance_with_mil_on_km
        )
    }
}

/// DM22 — individual DTC clear/reset. `control` and `nack_reason` are raw bytes.
#[pyclass(name = "Dm22Message", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm22Message {
    pub control: u8,
    pub nack_reason: Option<u8>,
    pub spn: u32,
    pub fmi: u8,
}

#[pymethods]
impl PyDm22Message {
    #[new]
    #[pyo3(signature = (control, spn=0, fmi=11, nack_reason=None))]
    fn new(control: u8, spn: u32, fmi: u8, nack_reason: Option<u8>) -> Self {
        Self {
            control,
            nack_reason,
            spn,
            fmi,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm22Message::decode(&data).map(|m| Self {
            control: m.control.as_u8(),
            nack_reason: m.nack_reason.map(|r| r.as_u8()),
            spn: m.spn,
            fmi: m.fmi.as_u8(),
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        Some(
            crate::j1939::Dm22Message {
                control: crate::j1939::Dm22Control::from_u8(self.control)?,
                nack_reason: match self.nack_reason {
                    Some(r) => Some(crate::j1939::Dm22NackReason::from_u8(r)?),
                    None => None,
                },
                spn: self.spn,
                fmi: Fmi::from_u8(self.fmi),
            }
            .encode()
            .to_vec(),
        )
    }
    fn __repr__(&self) -> String {
        format!("Dm22Message(control={}, spn={})", self.control, self.spn)
    }
}

/// DM9 — request the Vehicle Identification Number response PGN (no fields).
#[pyclass(name = "Dm9VehicleIdentificationRequest")]
#[derive(Clone)]
pub struct PyDm9VehicleIdentificationRequest;

#[pymethods]
impl PyDm9VehicleIdentificationRequest {
    #[new]
    fn new() -> Self {
        Self
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm9VehicleIdentificationRequest::decode(&data).map(|_| Self)
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::Dm9VehicleIdentificationRequest
            .encode()
            .ok()
            .map(|a| a.to_vec())
    }
    fn __repr__(&self) -> String {
        "Dm9VehicleIdentificationRequest()".to_string()
    }
}

/// DM10 — Vehicle Identification Number response.
#[pyclass(name = "Dm10VehicleIdentification", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm10VehicleIdentification {
    pub vin: String,
}

#[pymethods]
impl PyDm10VehicleIdentification {
    #[new]
    #[pyo3(signature = (vin=String::new()))]
    fn new(vin: String) -> Self {
        Self { vin }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm10VehicleIdentification::decode(&data).map(|m| Self { vin: m.vin })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::Dm10VehicleIdentification {
            vin: self.vin.clone(),
        }
        .encode()
        .ok()
    }
    fn __repr__(&self) -> String {
        format!("Dm10VehicleIdentification(vin={:?})", self.vin)
    }
}

/// ProductIdentification — `*`-delimited make/model/serial.
#[pyclass(name = "ProductIdentification", get_all, set_all)]
#[derive(Clone)]
pub struct PyProductIdentification {
    pub make: String,
    pub model: String,
    pub serial_number: String,
}

#[pymethods]
impl PyProductIdentification {
    #[new]
    #[pyo3(signature = (make=String::new(), model=String::new(), serial_number=String::new()))]
    fn new(make: String, model: String, serial_number: String) -> Self {
        Self {
            make,
            model,
            serial_number,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::ProductIdentification::decode(&data).map(|m| Self {
            make: m.make,
            model: m.model,
            serial_number: m.serial_number,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::ProductIdentification {
            make: self.make.clone(),
            model: self.model.clone(),
            serial_number: self.serial_number.clone(),
        }
        .encode()
        .ok()
    }
    fn __repr__(&self) -> String {
        format!("ProductIdentification(make={:?})", self.make)
    }
}

/// SoftwareIdentification — `*`-delimited version strings.
#[pyclass(name = "SoftwareIdentification", get_all, set_all)]
#[derive(Clone)]
pub struct PySoftwareIdentification {
    pub versions: Vec<String>,
}

#[pymethods]
impl PySoftwareIdentification {
    #[new]
    #[pyo3(signature = (versions=Vec::new()))]
    fn new(versions: Vec<String>) -> Self {
        Self { versions }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::SoftwareIdentification::decode(&data).map(|m| Self {
            versions: m.versions,
        })
    }
    fn encode(&self) -> Option<Vec<u8>> {
        crate::j1939::SoftwareIdentification {
            versions: self.versions.clone(),
        }
        .encode()
        .ok()
    }
    fn __repr__(&self) -> String {
        format!("SoftwareIdentification(versions={})", self.versions.len())
    }
}

/// DM20 monitor performance ratio entry (7-byte record).
#[pyclass(name = "MonitorPerformanceRatio", get_all, set_all)]
#[derive(Clone)]
pub struct PyMonitorPerformanceRatio {
    pub spn: u32,
    pub numerator: u16,
    pub denominator: u16,
}

#[pymethods]
impl PyMonitorPerformanceRatio {
    #[new]
    #[pyo3(signature = (spn=0, numerator=0, denominator=0))]
    fn new(spn: u32, numerator: u16, denominator: u16) -> Self {
        Self {
            spn,
            numerator,
            denominator,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::MonitorPerformanceRatio::decode(&data).map(|m| Self {
            spn: m.spn,
            numerator: m.numerator,
            denominator: m.denominator,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::MonitorPerformanceRatio {
            spn: self.spn,
            numerator: self.numerator,
            denominator: self.denominator,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!(
            "MonitorPerformanceRatio(spn={}, {}/{})",
            self.spn, self.numerator, self.denominator
        )
    }
}

/// DM20 — performance ratios response.
#[pyclass(name = "Dm20Response", get_all, set_all)]
#[derive(Clone)]
pub struct PyDm20Response {
    pub ignition_cycles: u8,
    pub obd_monitoring_conditions_met: u8,
    pub ratios: Vec<PyMonitorPerformanceRatio>,
}

#[pymethods]
impl PyDm20Response {
    #[new]
    #[pyo3(signature = (ignition_cycles=0, obd_monitoring_conditions_met=0, ratios=Vec::new()))]
    fn new(
        ignition_cycles: u8,
        obd_monitoring_conditions_met: u8,
        ratios: Vec<PyMonitorPerformanceRatio>,
    ) -> Self {
        Self {
            ignition_cycles,
            obd_monitoring_conditions_met,
            ratios,
        }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::Dm20Response::decode(&data).map(|m| Self {
            ignition_cycles: m.ignition_cycles,
            obd_monitoring_conditions_met: m.obd_monitoring_conditions_met,
            ratios: m
                .ratios
                .into_iter()
                .map(|r| PyMonitorPerformanceRatio {
                    spn: r.spn,
                    numerator: r.numerator,
                    denominator: r.denominator,
                })
                .collect(),
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::Dm20Response {
            ignition_cycles: self.ignition_cycles,
            obd_monitoring_conditions_met: self.obd_monitoring_conditions_met,
            ratios: self
                .ratios
                .iter()
                .map(|r| crate::j1939::MonitorPerformanceRatio {
                    spn: r.spn,
                    numerator: r.numerator,
                    denominator: r.denominator,
                })
                .collect(),
        }
        .encode()
    }
    fn __repr__(&self) -> String {
        format!("Dm20Response(ratios={})", self.ratios.len())
    }
}

/// DM25 SPN snapshot (7-byte record).
#[pyclass(name = "SpnSnapshot", get_all, set_all)]
#[derive(Clone)]
pub struct PySpnSnapshot {
    pub spn: u32,
    pub value: u32,
}

#[pymethods]
impl PySpnSnapshot {
    #[new]
    #[pyo3(signature = (spn=0, value=0))]
    fn new(spn: u32, value: u32) -> Self {
        Self { spn, value }
    }
    #[staticmethod]
    fn decode(data: Vec<u8>) -> Option<Self> {
        crate::j1939::SpnSnapshot::decode(&data).map(|m| Self {
            spn: m.spn,
            value: m.value,
        })
    }
    fn encode(&self) -> Vec<u8> {
        crate::j1939::SpnSnapshot {
            spn: self.spn,
            value: self.value,
        }
        .encode()
        .to_vec()
    }
    fn __repr__(&self) -> String {
        format!("SpnSnapshot(spn={}, value={})", self.spn, self.value)
    }
}

