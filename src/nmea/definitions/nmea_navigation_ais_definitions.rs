#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavigationData {
    pub sid: u8,
    pub distance_to_wp_m: f64,
    pub bearing_reference: HeadingReference,
    pub perpendicular_crossed: bool,
    pub arrival_circle_entered: bool,
    pub calc_type: DistanceCalculationType,
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

impl Default for NavigationData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            distance_to_wp_m: 0.0,
            bearing_reference: HeadingReference::Unavailable,
            perpendicular_crossed: false,
            arrival_circle_entered: false,
            calc_type: DistanceCalculationType::GreatCircle,
            eta_time: 0.0,
            eta_date: 0,
            bearing_origin_to_dest_rad: 0.0,
            bearing_pos_to_dest_rad: 0.0,
            origin_wp_number: 0,
            dest_wp_number: 0,
            dest_latitude: 0.0,
            dest_longitude: 0.0,
            wp_closing_velocity_mps: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MOBData {
    pub sid: u8,
    pub emitter_id: u32,
    pub status: MOBStatus,
    pub activation_time: f64,
    pub position_source: MOBPositionSource,
    pub position_date: u16,
    pub position_time: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub cog_reference: HeadingReference,
    pub cog_rad: f64,
    pub sog_mps: f64,
    pub mmsi: u32,
    pub battery: MOBBatteryStatus,
}

impl Default for MOBData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            emitter_id: 0,
            status: MOBStatus::NotActive,
            activation_time: 0.0,
            position_source: MOBPositionSource::EstimatedByVessel,
            position_date: 0,
            position_time: 0.0,
            latitude: 0.0,
            longitude: 0.0,
            cog_reference: HeadingReference::Unavailable,
            cog_rad: 0.0,
            sog_mps: 0.0,
            mmsi: 0,
            battery: MOBBatteryStatus::Good,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AISClassAPosition {
    pub message_id: u8,
    pub repeat: AISRepeat,
    pub user_id: u32,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: bool,
    pub raim: bool,
    pub seconds: u8,
    pub cog_rad: f64,
    pub sog_mps: f64,
    pub transceiver: AISTransceiverInfo,
    pub heading_rad: f64,
    pub rot_rad_per_s: f64,
    pub nav_status: AISNavStatus,
}

impl Default for AISClassAPosition {
    fn default() -> Self {
        Self {
            message_id: 0,
            repeat: AISRepeat::Initial,
            user_id: 0,
            latitude: 0.0,
            longitude: 0.0,
            accuracy: false,
            raim: false,
            seconds: 0,
            cog_rad: 0.0,
            sog_mps: 0.0,
            transceiver: AISTransceiverInfo::ChannelA_VDL_Rx,
            heading_rad: 0.0,
            rot_rad_per_s: 0.0,
            nav_status: AISNavStatus::UnderWayMotoring,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AISClassBPosition {
    pub message_id: u8,
    pub repeat: AISRepeat,
    pub user_id: u32,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: bool,
    pub raim: bool,
    pub seconds: u8,
    pub cog_rad: f64,
    pub sog_mps: f64,
    pub transceiver: AISTransceiverInfo,
    pub heading_rad: f64,
    pub unit: AISUnit,
    pub mode: AISMode,
}

impl Default for AISClassBPosition {
    fn default() -> Self {
        Self {
            message_id: 0,
            repeat: AISRepeat::Initial,
            user_id: 0,
            latitude: 0.0,
            longitude: 0.0,
            accuracy: false,
            raim: false,
            seconds: 0,
            cog_rad: 0.0,
            sog_mps: 0.0,
            transceiver: AISTransceiverInfo::ChannelA_VDL_Rx,
            heading_rad: 0.0,
            unit: AISUnit::ClassB_CS,
            mode: AISMode::Autonomous,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AISStaticData {
    pub message_id: u8,
    pub repeat: AISRepeat,
    pub user_id: u32,
    pub imo_number: u32,
    pub callsign: String,
    pub name: String,
    pub vessel_type: u8,
    pub length_m: f64,
    pub beam_m: f64,
    pub pos_ref_starboard_m: f64,
    pub pos_ref_bow_m: f64,
    pub eta_date: u16,
    pub eta_time: f64,
    pub draught_m: f64,
    pub destination: String,
    pub dte: AISDTE,
}

impl Default for AISStaticData {
    fn default() -> Self {
        Self {
            message_id: 0,
            repeat: AISRepeat::Initial,
            user_id: 0,
            imo_number: 0,
            callsign: String::new(),
            name: String::new(),
            vessel_type: 0,
            length_m: 0.0,
            beam_m: 0.0,
            pos_ref_starboard_m: 0.0,
            pos_ref_bow_m: 0.0,
            eta_date: 0,
            eta_time: 0.0,
            draught_m: 0.0,
            destination: String::new(),
            dte: AISDTE::NotReady,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TrimTabData {
    pub port_pct: i8,
    pub starboard_pct: i8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectionData {
    pub data_mode: DataMode,
    pub cog_reference: HeadingReference,
    pub sid: u8,
    pub cog_rad: f64,
    pub sog_mps: f64,
    pub heading_rad: f64,
    pub speed_through_water_mps: f64,
    pub set_rad: f64,
    pub drift_mps: f64,
}

impl Default for DirectionData {
    fn default() -> Self {
        Self {
            data_mode: DataMode::Autonomous,
            cog_reference: HeadingReference::Unavailable,
            sid: 0xFF,
            cog_rad: 0.0,
            sog_mps: 0.0,
            heading_rad: 0.0,
            speed_through_water_mps: 0.0,
            set_rad: 0.0,
            drift_mps: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeteorologicalData {
    pub sid: u8,
    pub date: u16,
    pub time: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub wind_speed_mps: f64,
    pub wind_dir_rad: f64,
    pub wind_reference: WindReference,
    pub wind_gusts_mps: f64,
    pub atmospheric_pressure_pa: f64,
    pub ambient_temperature_k: f64,
    pub station_id: String,
    pub station_name: String,
}

impl Default for MeteorologicalData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            date: 0,
            time: 0.0,
            latitude: 0.0,
            longitude: 0.0,
            wind_speed_mps: 0.0,
            wind_dir_rad: 0.0,
            wind_reference: WindReference::Unavailable,
            wind_gusts_mps: 0.0,
            atmospheric_pressure_pa: 0.0,
            ambient_temperature_k: 0.0,
            station_id: String::new(),
            station_name: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConverterStatusData {
    pub sid: u8,
    pub connection_number: u8,
    pub operating_state: ConverterMode,
    pub charge_mode: u8,
}

impl Default for ConverterStatusData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            connection_number: 0,
            operating_state: ConverterMode::NotAvailable,
            charge_mode: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_type_round_trip() {
        for v in 0..=15u8 {
            let f = GNSSFixType::from_u8(v);
            // Only documented values round-trip exactly.
            if matches!(v, 0..=8 | 14 | 15) {
                assert_eq!(f.as_u8(), v);
            } else {
                assert_eq!(f, GNSSFixType::NoFix);
            }
        }
    }

    #[test]
    fn dop_mode_masks_to_three_bits() {
        assert_eq!(GNSSDOPMode::from_u8(0xF8 | 3), GNSSDOPMode::Auto);
    }

    #[test]
    fn temperature_source_round_trip() {
        for s in [
            TemperatureSource::Sea,
            TemperatureSource::Outside,
            TemperatureSource::ShaftSeal,
        ] {
            assert_eq!(TemperatureSource::from_u8(s.as_u8()), s);
        }
    }

    #[test]
    fn fluid_type_masks_low_nibble() {
        assert_eq!(FluidType::from_u8(0xF0 | 1), FluidType::Water);
    }

    #[test]
    fn defaults_match_cpp() {
        assert_eq!(WindData::default().sid, 0xFF);
        assert_eq!(BatteryStatusData::default().state_of_charge_pct, 0xFF);
        assert_eq!(GNSSFixType::default(), GNSSFixType::NoFix);
        assert_eq!(WindReference::default(), WindReference::Unavailable);
    }

    #[test]
    fn resolution_constants_match() {
        assert_eq!(LAT_LON_RESOLUTION, 1.0e-7);
        assert_eq!(KELVIN_OFFSET, 273.15);
        assert!((ROT_RESOLUTION - 3.125e-8).abs() < 1e-15);
    }
}
