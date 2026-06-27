use alloc::{string::String, vec::Vec};

// ═══════════════════════════════════════════════════════════════════════
// ENUMERATION TYPES
// ═══════════════════════════════════════════════════════════════════════

macro_rules! repr_u8_enum {
    ($name:ident { $($variant:ident = $value:literal),* $(,)? }, default = $default:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum $name {
            $($variant = $value,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self::$default
            }
        }

        impl $name {
            #[inline]
            #[must_use]
            pub const fn as_u8(self) -> u8 {
                self as u8
            }
        }
    };
}

repr_u8_enum!(GNSSFixType {
    NoFix = 0,
    GNSSFix = 1,
    DGNSSFix = 2,
    PreciseGNSS = 3,
    RTKFixed = 4,
    RTKFloat = 5,
    DeadReckon = 6,
    ManualInput = 7,
    SimulateMode = 8,
    Error = 14,
    Unavailable = 15,
}, default = NoFix);

impl GNSSFixType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::GNSSFix,
            2 => Self::DGNSSFix,
            3 => Self::PreciseGNSS,
            4 => Self::RTKFixed,
            5 => Self::RTKFloat,
            6 => Self::DeadReckon,
            7 => Self::ManualInput,
            8 => Self::SimulateMode,
            14 => Self::Error,
            15 => Self::Unavailable,
            _ => Self::NoFix,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoFix),
            1 => Some(Self::GNSSFix),
            2 => Some(Self::DGNSSFix),
            3 => Some(Self::PreciseGNSS),
            4 => Some(Self::RTKFixed),
            5 => Some(Self::RTKFloat),
            6 => Some(Self::DeadReckon),
            7 => Some(Self::ManualInput),
            8 => Some(Self::SimulateMode),
            14 => Some(Self::Error),
            15 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(GNSSSystem {
    GPS = 0,
    GLONASS = 1,
    GPSAndGLO = 2,
    GPS_SBAS = 3,
    GPS_SBAS_GLO = 4,
    Chayka = 5,
    Integrated = 6,
    Surveyed = 7,
    Galileo = 8,
}, default = GPS);

impl GNSSSystem {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::GPS),
            1 => Some(Self::GLONASS),
            2 => Some(Self::GPSAndGLO),
            3 => Some(Self::GPS_SBAS),
            4 => Some(Self::GPS_SBAS_GLO),
            5 => Some(Self::Chayka),
            6 => Some(Self::Integrated),
            7 => Some(Self::Surveyed),
            8 => Some(Self::Galileo),
            _ => None,
        }
    }
}

repr_u8_enum!(ReferenceStationType {
    None = 0,
    RTCM = 1,
    Error = 14,
    Unavailable = 15,
}, default = None);

impl ReferenceStationType {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::RTCM),
            14 => Some(Self::Error),
            15 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(GNSSDOPMode {
    Mode1D = 0,
    Mode2D = 1,
    Mode3D = 2,
    Auto = 3,
    Reserved1 = 4,
    Reserved2 = 5,
    Error = 6,
    Unavailable = 7,
}, default = Auto);

impl GNSSDOPMode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            0 => Self::Mode1D,
            1 => Self::Mode2D,
            2 => Self::Mode3D,
            3 => Self::Auto,
            4 => Self::Reserved1,
            5 => Self::Reserved2,
            6 => Self::Error,
            _ => Self::Unavailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Mode1D),
            1 => Some(Self::Mode2D),
            2 => Some(Self::Mode3D),
            3 => Some(Self::Auto),
            4 | 5 => None,
            6 => Some(Self::Error),
            7 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(HeadingReference {
    True = 0,
    Magnetic = 1,
    Error = 2,
    Unavailable = 3,
}, default = Unavailable);

impl HeadingReference {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::True),
            1 => Some(Self::Magnetic),
            2 => Some(Self::Error),
            3 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(WindReference {
    TrueNorth = 0,
    Magnetic = 1,
    Apparent = 2,
    TrueBoat = 3,
    TrueWater = 4,
    Error = 6,
    Unavailable = 7,
}, default = Unavailable);

impl WindReference {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::TrueNorth,
            1 => Self::Magnetic,
            2 => Self::Apparent,
            3 => Self::TrueBoat,
            4 => Self::TrueWater,
            6 => Self::Error,
            _ => Self::Unavailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::TrueNorth),
            1 => Some(Self::Magnetic),
            2 => Some(Self::Apparent),
            3 => Some(Self::TrueBoat),
            4 => Some(Self::TrueWater),
            5 => None,
            6 => Some(Self::Error),
            7 | 0xFF => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(TemperatureSource {
    Sea = 0,
    Outside = 1,
    Inside = 2,
    EngineRoom = 3,
    MainCabin = 4,
    LiveWell = 5,
    BaitWell = 6,
    Refrigeration = 7,
    HeatingSystem = 8,
    DewPoint = 9,
    ApparentWindChill = 10,
    TheoreticalWindChill = 11,
    HeatIndex = 12,
    Freezer = 13,
    ExhaustGas = 14,
    ShaftSeal = 15,
}, default = Sea);

impl TemperatureSource {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Outside,
            2 => Self::Inside,
            3 => Self::EngineRoom,
            4 => Self::MainCabin,
            5 => Self::LiveWell,
            6 => Self::BaitWell,
            7 => Self::Refrigeration,
            8 => Self::HeatingSystem,
            9 => Self::DewPoint,
            10 => Self::ApparentWindChill,
            11 => Self::TheoreticalWindChill,
            12 => Self::HeatIndex,
            13 => Self::Freezer,
            14 => Self::ExhaustGas,
            15 => Self::ShaftSeal,
            _ => Self::Sea,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Sea),
            1 => Some(Self::Outside),
            2 => Some(Self::Inside),
            3 => Some(Self::EngineRoom),
            4 => Some(Self::MainCabin),
            5 => Some(Self::LiveWell),
            6 => Some(Self::BaitWell),
            7 => Some(Self::Refrigeration),
            8 => Some(Self::HeatingSystem),
            9 => Some(Self::DewPoint),
            10 => Some(Self::ApparentWindChill),
            11 => Some(Self::TheoreticalWindChill),
            12 => Some(Self::HeatIndex),
            13 => Some(Self::Freezer),
            14 => Some(Self::ExhaustGas),
            15 => Some(Self::ShaftSeal),
            _ => None,
        }
    }
}

repr_u8_enum!(HumiditySource {
    Inside = 0,
    Outside = 1,
    Unavailable = 0xFF,
}, default = Unavailable);

impl HumiditySource {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Inside,
            1 => Self::Outside,
            _ => Self::Unavailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Inside),
            1 => Some(Self::Outside),
            0xFF => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(PressureSource {
    Atmospheric = 0,
    Water = 1,
    Steam = 2,
    CompressedAir = 3,
    Hydraulic = 4,
    Filter = 5,
    AltimeterSetting = 6,
    Oil = 7,
    Fuel = 8,
    Reserved = 253,
    Error = 254,
    Unavailable = 255,
}, default = Unavailable);

impl PressureSource {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Atmospheric,
            1 => Self::Water,
            2 => Self::Steam,
            3 => Self::CompressedAir,
            4 => Self::Hydraulic,
            5 => Self::Filter,
            6 => Self::AltimeterSetting,
            7 => Self::Oil,
            8 => Self::Fuel,
            253 => Self::Reserved,
            254 => Self::Error,
            _ => Self::Unavailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Atmospheric),
            1 => Some(Self::Water),
            2 => Some(Self::Steam),
            3 => Some(Self::CompressedAir),
            4 => Some(Self::Hydraulic),
            5 => Some(Self::Filter),
            6 => Some(Self::AltimeterSetting),
            7 => Some(Self::Oil),
            8 => Some(Self::Fuel),
            253 => None,
            254 => Some(Self::Error),
            255 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(TimeSource {
    GPS = 0,
    GLONASS = 1,
    RadioStation = 2,
    LocalCesiumClock = 3,
    LocalRubidiumClock = 4,
    LocalCrystalClock = 5,
}, default = GPS);

impl TimeSource {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::GLONASS,
            2 => Self::RadioStation,
            3 => Self::LocalCesiumClock,
            4 => Self::LocalRubidiumClock,
            5 => Self::LocalCrystalClock,
            _ => Self::GPS,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::GPS),
            1 => Some(Self::GLONASS),
            2 => Some(Self::RadioStation),
            3 => Some(Self::LocalCesiumClock),
            4 => Some(Self::LocalRubidiumClock),
            5 => Some(Self::LocalCrystalClock),
            _ => None,
        }
    }
}

repr_u8_enum!(FluidType {
    Fuel = 0,
    Water = 1,
    GrayWater = 2,
    LiveWell = 3,
    Oil = 4,
    BlackWater = 5,
    FuelGasoline = 6,
    Error = 14,
    Unavailable = 15,
}, default = Unavailable);

impl FluidType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x0F {
            0 => Self::Fuel,
            1 => Self::Water,
            2 => Self::GrayWater,
            3 => Self::LiveWell,
            4 => Self::Oil,
            5 => Self::BlackWater,
            6 => Self::FuelGasoline,
            14 => Self::Error,
            _ => Self::Unavailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Fuel),
            1 => Some(Self::Water),
            2 => Some(Self::GrayWater),
            3 => Some(Self::LiveWell),
            4 => Some(Self::Oil),
            5 => Some(Self::BlackWater),
            6 => Some(Self::FuelGasoline),
            7..=13 => None,
            14 => Some(Self::Error),
            15 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(DCType {
    Battery = 0,
    Alternator = 1,
    Converter = 2,
    SolarCell = 3,
    WindGenerator = 4,
}, default = Battery);

impl DCType {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Battery),
            1 => Some(Self::Alternator),
            2 => Some(Self::Converter),
            3 => Some(Self::SolarCell),
            4 => Some(Self::WindGenerator),
            _ => None,
        }
    }
}

repr_u8_enum!(BatteryType {
    Flooded = 0,
    Gel = 1,
    AGM = 2,
}, default = Flooded);

impl BatteryType {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Flooded),
            1 => Some(Self::Gel),
            2 => Some(Self::AGM),
            _ => None,
        }
    }
}

repr_u8_enum!(BatteryChemistry {
    LeadAcid = 0,
    LiIon = 1,
    NiCad = 2,
    ZnO = 3,
    NiMh = 4,
}, default = LeadAcid);

impl BatteryChemistry {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::LeadAcid),
            1 => Some(Self::LiIon),
            2 => Some(Self::NiCad),
            3 => Some(Self::ZnO),
            4 => Some(Self::NiMh),
            _ => None,
        }
    }
}

repr_u8_enum!(BatteryNominalVoltage {
    V6 = 0,
    V12 = 1,
    V24 = 2,
    V32 = 3,
    V62 = 4,
    V42 = 5,
    V48 = 6,
}, default = V12);

impl BatteryNominalVoltage {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::V6),
            1 => Some(Self::V12),
            2 => Some(Self::V24),
            3 => Some(Self::V32),
            4 => Some(Self::V62),
            5 => Some(Self::V42),
            6 => Some(Self::V48),
            _ => None,
        }
    }
}

repr_u8_enum!(BatteryEqSupport {
    No = 0,
    Yes = 1,
    Error = 2,
    Unavailable = 3,
}, default = Unavailable);

impl BatteryEqSupport {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::No),
            1 => Some(Self::Yes),
            2 => Some(Self::Error),
            3 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(TransmissionGear {
    Forward = 0,
    Neutral = 1,
    Reverse = 2,
    Unknown = 3,
}, default = Unknown);

impl TransmissionGear {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Forward),
            1 => Some(Self::Neutral),
            2 => Some(Self::Reverse),
            3 => Some(Self::Unknown),
            _ => None,
        }
    }
}

repr_u8_enum!(RudderDirection {
    NoOrder = 0,
    Starboard = 1,
    Port = 2,
    Unavailable = 7,
}, default = NoOrder);

impl RudderDirection {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            1 => Self::Starboard,
            2 => Self::Port,
            7 => Self::Unavailable,
            _ => Self::NoOrder,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoOrder),
            1 => Some(Self::Starboard),
            2 => Some(Self::Port),
            7 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(SteeringMode {
    MainSteering = 0,
    NonFollowUp = 1,
    FollowUp = 2,
    HeadingControlStandalone = 3,
    HeadingControl = 4,
    TrackControl = 5,
    Unavailable = 7,
}, default = Unavailable);

impl SteeringMode {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::MainSteering),
            1 => Some(Self::NonFollowUp),
            2 => Some(Self::FollowUp),
            3 => Some(Self::HeadingControlStandalone),
            4 => Some(Self::HeadingControl),
            5 => Some(Self::TrackControl),
            7 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(TurnMode {
    RudderLimitControlled = 0,
    TurnRateControlled = 1,
    RadiusControlled = 2,
    Unavailable = 7,
}, default = Unavailable);

impl TurnMode {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::RudderLimitControlled),
            1 => Some(Self::TurnRateControlled),
            2 => Some(Self::RadiusControlled),
            7 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(SpeedWaterRefType {
    PaddleWheel = 0,
    PitotTube = 1,
    DopplerLog = 2,
    UltraSound = 3,
    Electromagnetic = 4,
    Error = 254,
    Unavailable = 255,
}, default = Unavailable);

impl SpeedWaterRefType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::PaddleWheel,
            1 => Self::PitotTube,
            2 => Self::DopplerLog,
            3 => Self::UltraSound,
            4 => Self::Electromagnetic,
            254 => Self::Error,
            _ => Self::Unavailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::PaddleWheel),
            1 => Some(Self::PitotTube),
            2 => Some(Self::DopplerLog),
            3 => Some(Self::UltraSound),
            4 => Some(Self::Electromagnetic),
            254 => Some(Self::Error),
            255 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(MagneticVariationSource {
    Manual = 0,
    Chart = 1,
    Table = 2,
    Calculated = 3,
    WMM2000 = 4,
    WMM2005 = 5,
    WMM2010 = 6,
    WMM2015 = 7,
    WMM2020 = 8,
    WMM2025 = 9,
}, default = Manual);

impl MagneticVariationSource {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Manual),
            1 => Some(Self::Chart),
            2 => Some(Self::Table),
            3 => Some(Self::Calculated),
            4 => Some(Self::WMM2000),
            5 => Some(Self::WMM2005),
            6 => Some(Self::WMM2010),
            7 => Some(Self::WMM2015),
            8 => Some(Self::WMM2020),
            9 => Some(Self::WMM2025),
            _ => None,
        }
    }
}

repr_u8_enum!(NavigationDirection {
    Forward = 0,
    Reverse = 1,
    Error = 6,
    Unknown = 7,
}, default = Unknown);

impl NavigationDirection {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Forward),
            1 => Some(Self::Reverse),
            6 => Some(Self::Error),
            7 => Some(Self::Unknown),
            _ => None,
        }
    }
}

repr_u8_enum!(DistanceCalculationType {
    GreatCircle = 0,
    RhumbLine = 1,
}, default = GreatCircle);

impl DistanceCalculationType {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::GreatCircle),
            1 => Some(Self::RhumbLine),
            _ => None,
        }
    }
}

repr_u8_enum!(XTEMode {
    Autonomous = 0,
    Differential = 1,
    Estimated = 2,
    Simulator = 3,
    Manual = 4,
}, default = Autonomous);

impl XTEMode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x0F {
            1 => Self::Differential,
            2 => Self::Estimated,
            3 => Self::Simulator,
            4 => Self::Manual,
            _ => Self::Autonomous,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Autonomous),
            1 => Some(Self::Differential),
            2 => Some(Self::Estimated),
            3 => Some(Self::Simulator),
            4 => Some(Self::Manual),
            _ => None,
        }
    }
}

repr_u8_enum!(OnOff {
    Off = 0,
    On = 1,
    Error = 2,
    Unavailable = 3,
}, default = Unavailable);

impl OnOff {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Off),
            1 => Some(Self::On),
            2 => Some(Self::Error),
            3 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(ChargeState {
    NotCharging = 0,
    Bulk = 1,
    Absorption = 2,
    Overcharge = 3,
    Equalise = 4,
    Float = 5,
    NoFloat = 6,
    ConstantVI = 7,
    Disabled = 8,
    Fault = 9,
    Error = 14,
    Unavailable = 15,
}, default = Unavailable);

impl ChargeState {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NotCharging),
            1 => Some(Self::Bulk),
            2 => Some(Self::Absorption),
            3 => Some(Self::Overcharge),
            4 => Some(Self::Equalise),
            5 => Some(Self::Float),
            6 => Some(Self::NoFloat),
            7 => Some(Self::ConstantVI),
            8 => Some(Self::Disabled),
            9 => Some(Self::Fault),
            14 => Some(Self::Error),
            15 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(ChargerMode {
    Standalone = 0,
    Primary = 1,
    Secondary = 2,
    Echo = 3,
    Unavailable = 15,
}, default = Unavailable);

impl ChargerMode {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Standalone),
            1 => Some(Self::Primary),
            2 => Some(Self::Secondary),
            3 => Some(Self::Echo),
            15 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(ConverterMode {
    Off = 0,
    LowPower = 1,
    Fault = 2,
    Bulk = 3,
    Absorption = 4,
    Float = 5,
    Storage = 6,
    Equalise = 7,
    Passthru = 8,
    Inverting = 9,
    Assisting = 10,
    PSUMode = 11,
    NotAvailable = 0xFF,
}, default = NotAvailable);

impl ConverterMode {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Off),
            1 => Some(Self::LowPower),
            2 => Some(Self::Fault),
            3 => Some(Self::Bulk),
            4 => Some(Self::Absorption),
            5 => Some(Self::Float),
            6 => Some(Self::Storage),
            7 => Some(Self::Equalise),
            8 => Some(Self::Passthru),
            9 => Some(Self::Inverting),
            10 => Some(Self::Assisting),
            11 => Some(Self::PSUMode),
            0xFF => Some(Self::NotAvailable),
            _ => None,
        }
    }
}

repr_u8_enum!(MOBStatus {
    EmitterActivated = 0,
    ManualButtonActivation = 1,
    TestMode = 2,
    NotActive = 3,
}, default = NotActive);

impl MOBStatus {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::EmitterActivated),
            1 => Some(Self::ManualButtonActivation),
            2 => Some(Self::TestMode),
            3 => Some(Self::NotActive),
            _ => None,
        }
    }
}

repr_u8_enum!(MOBPositionSource {
    EstimatedByVessel = 0,
    ReportedByEmitter = 1,
}, default = EstimatedByVessel);

impl MOBPositionSource {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::EstimatedByVessel),
            1 => Some(Self::ReportedByEmitter),
            _ => None,
        }
    }
}

repr_u8_enum!(MOBBatteryStatus {
    Good = 0,
    Low = 1,
}, default = Good);

impl MOBBatteryStatus {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Good),
            1 => Some(Self::Low),
            _ => None,
        }
    }
}

repr_u8_enum!(AISRepeat {
    Initial = 0,
    First = 1,
    Second = 2,
    Final = 3,
}, default = Initial);

impl AISRepeat {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Initial),
            1 => Some(Self::First),
            2 => Some(Self::Second),
            3 => Some(Self::Final),
            _ => None,
        }
    }
}

repr_u8_enum!(AISNavStatus {
    UnderWayMotoring = 0,
    AtAnchor = 1,
    NotUnderCommand = 2,
    RestrictedManoeuverability = 3,
    ConstrainedByDraught = 4,
    Moored = 5,
    Aground = 6,
    Fishing = 7,
    UnderWaySailing = 8,
    HazmatHighSpeed = 9,
    HazmatWingInGround = 10,
    AIS_SART = 14,
}, default = UnderWayMotoring);

impl AISNavStatus {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::UnderWayMotoring),
            1 => Some(Self::AtAnchor),
            2 => Some(Self::NotUnderCommand),
            3 => Some(Self::RestrictedManoeuverability),
            4 => Some(Self::ConstrainedByDraught),
            5 => Some(Self::Moored),
            6 => Some(Self::Aground),
            7 => Some(Self::Fishing),
            8 => Some(Self::UnderWaySailing),
            9 => Some(Self::HazmatHighSpeed),
            10 => Some(Self::HazmatWingInGround),
            14 => Some(Self::AIS_SART),
            _ => None,
        }
    }
}

repr_u8_enum!(AISUnit {
    ClassB_SOTDMA = 0,
    ClassB_CS = 1,
}, default = ClassB_CS);

impl AISUnit {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::ClassB_SOTDMA),
            1 => Some(Self::ClassB_CS),
            _ => None,
        }
    }
}

repr_u8_enum!(AISMode {
    Autonomous = 0,
    Assigned = 1,
}, default = Autonomous);

impl AISMode {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Autonomous),
            1 => Some(Self::Assigned),
            _ => None,
        }
    }
}

repr_u8_enum!(AISTransceiverInfo {
    ChannelA_VDL_Rx = 0,
    ChannelB_VDL_Rx = 1,
    ChannelA_VDL_Tx = 2,
    ChannelB_VDL_Tx = 3,
    OwnInfoNotBroadcast = 4,
}, default = ChannelA_VDL_Rx);

impl AISTransceiverInfo {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::ChannelA_VDL_Rx),
            1 => Some(Self::ChannelB_VDL_Rx),
            2 => Some(Self::ChannelA_VDL_Tx),
            3 => Some(Self::ChannelB_VDL_Tx),
            4 => Some(Self::OwnInfoNotBroadcast),
            _ => None,
        }
    }
}

repr_u8_enum!(AISDTE {
    Ready = 0,
    NotReady = 1,
}, default = NotReady);

impl AISDTE {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Ready),
            1 => Some(Self::NotReady),
            _ => None,
        }
    }
}

repr_u8_enum!(DelaySource {
    GPS = 0,
    GLONASS = 1,
    GPS_GLONASS = 2,
    Unavailable = 15,
}, default = Unavailable);

impl DelaySource {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::GPS),
            1 => Some(Self::GLONASS),
            2 => Some(Self::GPS_GLONASS),
            15 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

repr_u8_enum!(DataMode {
    Autonomous = 0,
    Differential = 1,
    Estimated = 2,
    Simulator = 3,
    Manual = 4,
}, default = Autonomous);

impl DataMode {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Autonomous),
            1 => Some(Self::Differential),
            2 => Some(Self::Estimated),
            3 => Some(Self::Simulator),
            4 => Some(Self::Manual),
            _ => None,
        }
    }
}

repr_u8_enum!(RangeResidualMode {
    RangeResiduals_Used = 0,
    RangeResiduals_Calculated = 1,
    Error = 2,
    Unavailable = 3,
}, default = Unavailable);

impl RangeResidualMode {
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::RangeResiduals_Used),
            1 => Some(Self::RangeResiduals_Calculated),
            2 => Some(Self::Error),
            3 => Some(Self::Unavailable),
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// RESOLUTION CONSTANTS
// ═══════════════════════════════════════════════════════════════════════

pub const LAT_LON_RESOLUTION: f64 = 1.0e-7;
pub const ALTITUDE_RESOLUTION: f64 = 0.01;
pub const SPEED_RESOLUTION: f64 = 0.01;
pub const HEADING_RESOLUTION: f64 = 0.0001;
pub const COG_RESOLUTION: f64 = 0.0001;
pub const ROT_RESOLUTION: f64 = 3.125e-8;
pub const POSITION_DELTA_RESOLUTION: f64 = 1.0e-6;
pub const POSITION_DELTA_TIME_RESOLUTION: f64 = 0.005;
pub const TEMPERATURE_RESOLUTION: f64 = 0.01;
pub const PRESSURE_RESOLUTION: f64 = 100.0;
pub const WIND_SPEED_RESOLUTION: f64 = 0.01;
pub const WIND_DIR_RESOLUTION: f64 = 0.0001;
pub const RPM_RESOLUTION: f64 = 0.25;
pub const DEPTH_RESOLUTION: f64 = 0.01;
pub const DISTANCE_RESOLUTION: f64 = 0.01;
pub const VOLTAGE_RESOLUTION: f64 = 0.01;
pub const CURRENT_RESOLUTION: f64 = 0.1;
pub const ANGLE_RESOLUTION: f64 = 0.0001;
pub const RUDDER_RESOLUTION: f64 = 0.0001;
pub const DOP_RESOLUTION: f64 = 0.01;
pub const XTE_RESOLUTION: f64 = 0.01;
pub const FLUID_LEVEL_RESOLUTION: f64 = 0.004;
pub const FLUID_CAPACITY_RESOLUTION: f64 = 0.1;
pub const HUMIDITY_RESOLUTION: f64 = 0.004;
pub const KELVIN_OFFSET: f64 = 273.15;

// ═══════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindData {
    pub sid: u8,
    pub speed_mps: f64,
    pub direction_rad: f64,
    pub reference: WindReference,
}

impl Default for WindData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            speed_mps: 0.0,
            direction_rad: 0.0,
            reference: WindReference::Unavailable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemperatureData {
    pub sid: u8,
    pub instance: u8,
    pub source: TemperatureSource,
    pub actual_k: f64,
    pub set_k: f64,
}

impl Default for TemperatureData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            instance: 0,
            source: TemperatureSource::Sea,
            actual_k: 0.0,
            set_k: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HumidityData {
    pub sid: u8,
    pub instance: u8,
    pub source: HumiditySource,
    pub actual_pct: f64,
    pub set_pct: f64,
}

impl Default for HumidityData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            instance: 0,
            source: HumiditySource::Unavailable,
            actual_pct: 0.0,
            set_pct: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressureData {
    pub sid: u8,
    pub instance: u8,
    pub source: PressureSource,
    pub pressure_pa: f64,
}

impl Default for PressureData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            instance: 0,
            source: PressureSource::Unavailable,
            pressure_pa: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OutsideEnvironmentalData {
    pub sid: u8,
    pub water_temperature_k: f64,
    pub outside_temperature_k: f64,
    pub atmospheric_pressure_pa: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineData {
    pub instance: u8,
    pub rpm: f64,
    pub boost_pressure_pa: f64,
    pub tilt_trim: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineDynamicData {
    pub instance: u8,
    pub oil_pressure_pa: f64,
    pub oil_temperature_k: f64,
    pub coolant_temperature_k: f64,
    pub alternator_voltage: f64,
    pub fuel_rate_lph: f64,
    pub engine_hours: f64,
    pub coolant_pressure_pa: f64,
    pub fuel_pressure_pa: f64,
    pub load_pct: i8,
    pub torque_pct: i8,
    pub status1: u16,
    pub status2: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransmissionData {
    pub instance: u8,
    pub gear: TransmissionGear,
    pub oil_pressure_pa: f64,
    pub oil_temperature_k: f64,
    pub discrete_status: u8,
}

impl Default for TransmissionData {
    fn default() -> Self {
        Self {
            instance: 0,
            gear: TransmissionGear::Unknown,
            oil_pressure_pa: 0.0,
            oil_temperature_k: 0.0,
            discrete_status: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineTripData {
    pub instance: u8,
    pub trip_fuel_used_l: f64,
    pub fuel_rate_avg_lph: f64,
    pub fuel_rate_economy: f64,
    pub instantaneous_economy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterDepthData {
    pub sid: u8,
    pub depth_m: f64,
    pub offset_m: f64,
    pub range_m: f64,
}

impl Default for WaterDepthData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            depth_m: 0.0,
            offset_m: 0.0,
            range_m: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedWaterData {
    pub sid: u8,
    pub water_speed_mps: f64,
    pub ground_speed_mps: f64,
    pub reference: SpeedWaterRefType,
}

impl Default for SpeedWaterData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            water_speed_mps: 0.0,
            ground_speed_mps: 0.0,
            reference: SpeedWaterRefType::Unavailable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DistanceLogData {
    pub days_since_epoch: u16,
    pub seconds_since_midnight: f64,
    pub log_m: u32,
    pub trip_log_m: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeewayData {
    pub sid: u8,
    pub leeway_rad: f64,
}

impl Default for LeewayData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            leeway_rad: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemTimeData {
    pub sid: u8,
    pub source: TimeSource,
    pub days_since_epoch: u16,
    pub seconds_since_midnight: f64,
}

impl Default for SystemTimeData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            source: TimeSource::GPS,
            days_since_epoch: 0,
            seconds_since_midnight: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadingData {
    pub sid: u8,
    pub heading_rad: f64,
    pub deviation_rad: f64,
    pub variation_rad: f64,
    pub reference: HeadingReference,
}

impl Default for HeadingData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            heading_rad: 0.0,
            deviation_rad: 0.0,
            variation_rad: 0.0,
            reference: HeadingReference::Unavailable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateOfTurnData {
    pub sid: u8,
    pub rate_rad_per_s: f64,
}

impl Default for RateOfTurnData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            rate_rad_per_s: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionDeltaHighPrecisionRapidUpdateData {
    pub sid: u8,
    pub time_delta_s: f64,
    pub latitude_delta_deg: f64,
    pub longitude_delta_deg: f64,
}

impl Default for PositionDeltaHighPrecisionRapidUpdateData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            time_delta_s: 0.0,
            latitude_delta_deg: 0.0,
            longitude_delta_deg: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeaveData {
    pub sid: u8,
    pub heave_m: f64,
    pub delay_s: f64,
    pub delay_source: DelaySource,
}

impl Default for HeaveData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            heave_m: 0.0,
            delay_s: 0.0,
            delay_source: DelaySource::Unavailable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttitudeData {
    pub sid: u8,
    pub yaw_rad: f64,
    pub pitch_rad: f64,
    pub roll_rad: f64,
}

impl Default for AttitudeData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            yaw_rad: 0.0,
            pitch_rad: 0.0,
            roll_rad: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagneticVariationData {
    pub sid: u8,
    pub source: MagneticVariationSource,
    pub days_since_epoch: u16,
    pub variation_rad: f64,
}

impl Default for MagneticVariationData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            source: MagneticVariationSource::Manual,
            days_since_epoch: 0,
            variation_rad: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RudderData {
    pub position_rad: f64,
    pub instance: u8,
    pub direction: RudderDirection,
    pub angle_order_rad: f64,
}

impl Default for RudderData {
    fn default() -> Self {
        Self {
            position_rad: 0.0,
            instance: 0,
            direction: RudderDirection::NoOrder,
            angle_order_rad: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadingTrackControlData {
    pub rudder_limit_exceeded: OnOff,
    pub off_heading_limit_exceeded: OnOff,
    pub off_track_limit_exceeded: OnOff,
    pub override_active: OnOff,
    pub steering_mode: SteeringMode,
    pub turn_mode: TurnMode,
    pub heading_reference: HeadingReference,
    pub commanded_rudder_direction: RudderDirection,
    pub commanded_rudder_angle_rad: f64,
    pub heading_to_steer_rad: f64,
    pub track_rad: f64,
    pub rudder_limit_rad: f64,
    pub off_heading_limit_rad: f64,
    pub radius_of_turn_m: f64,
    pub rate_of_turn_rad_per_s: f64,
    pub off_track_limit_m: f64,
    pub vessel_heading_rad: f64,
}

impl Default for HeadingTrackControlData {
    fn default() -> Self {
        Self {
            rudder_limit_exceeded: OnOff::Unavailable,
            off_heading_limit_exceeded: OnOff::Unavailable,
            off_track_limit_exceeded: OnOff::Unavailable,
            override_active: OnOff::Unavailable,
            steering_mode: SteeringMode::Unavailable,
            turn_mode: TurnMode::Unavailable,
            heading_reference: HeadingReference::Unavailable,
            commanded_rudder_direction: RudderDirection::Unavailable,
            commanded_rudder_angle_rad: 0.0,
            heading_to_steer_rad: 0.0,
            track_rad: 0.0,
            rudder_limit_rad: 0.0,
            off_heading_limit_rad: 0.0,
            radius_of_turn_m: 0.0,
            rate_of_turn_rad_per_s: 0.0,
            off_track_limit_m: 0.0,
            vessel_heading_rad: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluidLevelData {
    pub instance: u8,
    pub r#type: FluidType,
    pub level_pct: f64,
    pub capacity_l: f64,
}

impl Default for FluidLevelData {
    fn default() -> Self {
        Self {
            instance: 0,
            r#type: FluidType::Unavailable,
            level_pct: 0.0,
            capacity_l: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DCDetailedData {
    pub sid: u8,
    pub dc_instance: u8,
    pub r#type: DCType,
    pub voltage: f64,
    pub current_a: f64,
    pub temperature_k: f64,
}

impl Default for DCDetailedData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            dc_instance: 0,
            r#type: DCType::Battery,
            voltage: 0.0,
            current_a: 0.0,
            temperature_k: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatteryStatusData {
    pub instance: u8,
    pub voltage: f64,
    pub current_a: f64,
    pub state_of_charge_pct: u8,
    pub state_of_health_pct: u8,
    pub time_remaining_s: f64,
}

impl Default for BatteryStatusData {
    fn default() -> Self {
        Self {
            instance: 0,
            voltage: 0.0,
            current_a: 0.0,
            state_of_charge_pct: 0xFF,
            state_of_health_pct: 0xFF,
            time_remaining_s: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatteryConfigData {
    pub instance: u8,
    pub r#type: BatteryType,
    pub eq_support: BatteryEqSupport,
    pub nominal_voltage: BatteryNominalVoltage,
    pub chemistry: BatteryChemistry,
    pub capacity_ah: f64,
    pub temperature_coefficient_pct: u8,
    pub peukert_exponent: f64,
    pub charge_efficiency_pct: u8,
}

impl Default for BatteryConfigData {
    fn default() -> Self {
        Self {
            instance: 0,
            r#type: BatteryType::Flooded,
            eq_support: BatteryEqSupport::Unavailable,
            nominal_voltage: BatteryNominalVoltage::V12,
            chemistry: BatteryChemistry::LeadAcid,
            capacity_ah: 0.0,
            temperature_coefficient_pct: 0,
            peukert_exponent: 0.0,
            charge_efficiency_pct: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GNSSDOPData {
    pub sid: u8,
    pub desired_mode: GNSSDOPMode,
    pub actual_mode: GNSSDOPMode,
    pub hdop: f64,
    pub vdop: f64,
    pub tdop: f64,
}

impl Default for GNSSDOPData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            desired_mode: GNSSDOPMode::Auto,
            actual_mode: GNSSDOPMode::Unavailable,
            hdop: 0.0,
            vdop: 0.0,
            tdop: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SatelliteInfo {
    pub prn: u8,
    pub elevation_rad: f64,
    pub azimuth_rad: f64,
    pub snr_db: f64,
    pub range_residual_m: f64,
    pub system: GNSSSystem,
}

impl Default for SatelliteInfo {
    fn default() -> Self {
        Self {
            prn: 0,
            elevation_rad: 0.0,
            azimuth_rad: 0.0,
            snr_db: 0.0,
            range_residual_m: 0.0,
            system: GNSSSystem::GPS,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SatellitesInViewData {
    pub sid: u8,
    pub mode: RangeResidualMode,
    pub num_satellites: u8,
    pub satellites: Vec<SatelliteInfo>,
}

impl Default for SatellitesInViewData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            mode: RangeResidualMode::Unavailable,
            num_satellites: 0,
            satellites: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocalTimeOffsetData {
    pub days_since_epoch: u16,
    pub seconds_since_midnight: f64,
    pub local_offset_minutes: i16,
}

/// PGN 129540 GNSS Sats in View (GNSS quality / satellite visibility). Layout
/// per canboat (Apache-licensed): fast-packet with a per-satellite repeating set.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GnssSatsInViewData {
    pub sid: u8,
    /// Count field as reported by the sender.
    pub sats_in_view: u8,
    pub satellites: Vec<SatelliteInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XTEData {
    pub sid: u8,
    pub mode: XTEMode,
    pub navigation_terminated: bool,
    pub xte_m: f64,
}

impl Default for XTEData {
    fn default() -> Self {
        Self {
            sid: 0xFF,
            mode: XTEMode::Autonomous,
            navigation_terminated: false,
            xte_m: 0.0,
        }
    }
}

