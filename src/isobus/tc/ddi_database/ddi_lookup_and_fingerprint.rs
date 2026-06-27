pub const DDI_DATABASE_SIZE: usize = 760;
pub const DDI_DATABASE_VERSION: u32 = 2025121001;
pub const DDI_DATABASE_FINGERPRINT_FNV1A64: u64 = 0x1C4D_EA1E_6B4F_9641;

const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

#[must_use]
pub fn ddi_database_fingerprint() -> u64 {
    let mut hash = FNV1A64_OFFSET;
    for entry in DDI_DATABASE {
        hash = fnv1a64_u16(hash, entry.ddi);
        hash = fnv1a64_str(hash, entry.name);
        hash = fnv1a64_str(hash, entry.unit);
        hash = fnv1a64_u64(hash, entry.resolution.to_bits());
        hash = fnv1a64_i32(hash, entry.min_value);
        hash = fnv1a64_i32(hash, entry.max_value);
    }
    hash
}

fn fnv1a64_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ u64::from(byte)).wrapping_mul(FNV1A64_PRIME)
}

fn fnv1a64_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash = fnv1a64_byte(hash, byte);
    }
    fnv1a64_byte(hash, 0)
}

fn fnv1a64_str(hash: u64, value: &str) -> u64 {
    fnv1a64_bytes(hash, value.as_bytes())
}

fn fnv1a64_u16(hash: u64, value: u16) -> u64 {
    fnv1a64_bytes(hash, &value.to_le_bytes())
}

fn fnv1a64_u64(hash: u64, value: u64) -> u64 {
    fnv1a64_bytes(hash, &value.to_le_bytes())
}

fn fnv1a64_i32(hash: u64, value: i32) -> u64 {
    fnv1a64_bytes(hash, &value.to_le_bytes())
}

// ─── Lookup helpers ────────────────────────────────────────────────────

#[must_use]
pub fn ddi_lookup(ddi: u16) -> Option<&'static DDIDefinition> {
    DDI_DATABASE.iter().find(|e| e.ddi == ddi)
}

#[must_use]
pub fn ddi_name(ddi: u16) -> &'static str {
    ddi_lookup(ddi).map_or("Unknown", |e| e.name)
}

#[must_use]
pub fn ddi_unit(ddi: u16) -> &'static str {
    ddi_lookup(ddi).map_or("", |e| e.unit)
}

#[must_use]
pub fn ddi_unit_description(ddi: u16) -> &'static str {
    ddi_lookup(ddi).map_or("Unknown", |e| ddi_unit_description_for(e))
}

#[must_use]
pub fn ddi_resolution(ddi: u16) -> f64 {
    ddi_lookup(ddi).map_or(1.0, |e| e.resolution)
}

#[must_use]
pub fn ddi_display_range(ddi: u16) -> (f64, f64) {
    ddi_lookup(ddi).map_or((0.0, 0.0), |e| {
        (f64::from(e.min_value), f64::from(e.max_value))
    })
}

#[must_use]
pub fn ddi_data_dictionary_entry(ddi: u16) -> DataDictionaryEntry {
    match ddi_lookup(ddi) {
        Some(e) => DataDictionaryEntry {
            ddi: e.ddi,
            name: e.name,
            unit_symbol: e.unit,
            unit_description: ddi_unit_description_for(e),
            resolution: e.resolution,
            display_range: (f64::from(e.min_value), f64::from(e.max_value)),
        },
        None => DataDictionaryEntry::unknown(),
    }
}

#[must_use]
fn ddi_unit_description_for(entry: &DDIDefinition) -> &'static str {
    match entry.unit {
        "" | "n.a. -" => "n.a.",
        "#" => "Count",
        "%" => "Percent",
        "/m2" => "Count per area unit",
        "/s" => "Count per time",
        "A" => "Current",
        "deg" => "Angle",
        "g" => "Mass large",
        "h" => "Time",
        "Hz" => "Frequency",
        "kg" => "Mass",
        "kg/h" => "Mass flow",
        "kWh" => "Energy",
        "kWh/m2" => "Energy per area unit",
        "L" => "Volume",
        "m" => "Length",
        "m2" => "Area",
        "mg/1000" => "Mass per count",
        "mg/kg" => "Mass per mass unit",
        "mg/l" | "mg/l (mass per unit volume)" => "Mass per volume unit",
        "mg/m2" => "Mass per area unit",
        "mg/s" => "Mass flow",
        "mK" => "Temperature",
        "ml" => "Volume",
        "ml/1000" => "Volume per count",
        "ml/m" => "Volume per distance unit",
        "ml/m2" => "Volume per area unit",
        "ml/s" => "Volume flow",
        "mm" => "Length",
        "mm/s" => "Speed",
        "mm2/s" => "Kinematic viscosity",
        "mm3/kg" => "Volume per mass unit",
        "mm3/m2" => "Volume per area unit",
        "mm3/m3" => "Volume per volume unit",
        "mm3/s" => "Volume flow",
        "mS/m" => "Electrical conductivity",
        "ms" => "Time",
        "N" => "Force",
        "N*m" => "Torque",
        "Ohm" => "Electrical resistance",
        "Pa" => "Pressure",
        "ppm" => "Parts per million",
        "r/min" => "Rotational speed",
        "s" => "Time",
        "V" => "Voltage",
        "W" => "Power",
        _ => "Unknown",
    }
}

#[must_use]
pub fn ddi_to_engineering(ddi: u16, raw: i32) -> f64 {
    raw as f64 * ddi_resolution(ddi)
}

#[must_use]
pub fn ddi_from_engineering(ddi: u16, eng: f64) -> i32 {
    let res = ddi_resolution(ddi);
    let raw = if res != 0.0 { eng / res } else { eng };
    if !raw.is_finite() {
        0
    } else if raw <= f64::from(i32::MIN) {
        i32::MIN
    } else if raw >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        raw as i32
    }
}

// ─── DDI Category helpers ──────────────────────────────────────────────

#[must_use]
pub fn ddi_is_rate(ddi: impl Into<u16>) -> bool {
    let ddi = ddi.into();
    (1..=55).contains(&ddi)
        || (432..=451).contains(&ddi)
        || (574..=577).contains(&ddi)
        || (588..=637).contains(&ddi)
}

#[must_use]
pub fn ddi_is_total(ddi: impl Into<u16>) -> bool {
    let ddi = ddi.into();
    (116..=123).contains(&ddi) || (130..=131).contains(&ddi) || (351..=353).contains(&ddi)
}

#[must_use]
pub fn ddi_is_geometry(ddi: impl Into<u16>) -> bool {
    let ddi = ddi.into();
    (134..=140).contains(&ddi)
}

#[must_use]
pub fn ddi_is_section_control(ddi: impl Into<u16>) -> bool {
    let ddi = ddi.into();
    (153..=161).contains(&ddi)
}

#[must_use]
pub const fn ddi_is_speed_distance(ddi: u16) -> bool {
    (ddi >= 56 && ddi <= 60) || (ddi >= 33 && ddi <= 35)
}

#[must_use]
pub const fn ddi_is_proprietary(ddi: u16) -> bool {
    ddi >= 57344 && ddi <= 65534
}

/// Classification of a DDI reference for validation (ISO 11783-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DdiClass {
    /// Present in the public DDI database.
    Known,
    /// In the proprietary/custom DDI range (a vendor-defined DDI).
    Proprietary,
    /// Neither in the database nor the proprietary range — a likely error.
    Unknown,
}

/// Classify a single DDI reference.
#[must_use]
pub fn classify_ddi(ddi: u16) -> DdiClass {
    if ddi_lookup(ddi).is_some() {
        DdiClass::Known
    } else if ddi_is_proprietary(ddi) {
        DdiClass::Proprietary
    } else {
        DdiClass::Unknown
    }
}

/// `true` if `ddi` is acceptable to reference: either a known database DDI
/// or an explicitly proprietary/custom one.
#[must_use]
pub fn ddi_is_acceptable(ddi: u16) -> bool {
    !matches!(classify_ddi(ddi), DdiClass::Unknown)
}

/// Validate a set of referenced DDIs (from a DDOP, example, or tutorial):
/// returns the DDIs that are neither known nor proprietary, i.e. the ones
/// that should be fixed. An empty result means every reference is valid.
#[must_use]
pub fn unknown_ddis(ddis: &[u16]) -> Vec<u16> {
    ddis.iter()
        .copied()
        .filter(|&d| !ddi_is_acceptable(d))
        .collect()
}

/// Legacy `DDIDatabase` static-method facade. Mirrors the C++
/// `class DDIDatabase` for source-level parity.
pub struct DDIDatabase;

/// AgIsoStack++ naming alias for the DDI lookup facade.
pub type DataDictionary = DDIDatabase;

impl DDIDatabase {
    #[must_use]
    pub const fn version() -> u32 {
        DDI_DATABASE_VERSION
    }

    #[must_use]
    pub fn fingerprint() -> u64 {
        ddi_database_fingerprint()
    }

    #[must_use]
    pub fn lookup(ddi: u16) -> Option<DDIEntry> {
        ddi_lookup(ddi).copied()
    }

    #[must_use]
    pub fn get_entry(ddi: u16) -> DataDictionaryEntry {
        ddi_data_dictionary_entry(ddi)
    }

    #[must_use]
    pub fn unit_for(ddi: u16) -> &'static str {
        ddi_unit(ddi)
    }

    #[must_use]
    pub fn unit_description_for(ddi: u16) -> &'static str {
        ddi_unit_description(ddi)
    }

    #[must_use]
    pub fn name_for(ddi: u16) -> &'static str {
        ddi_name(ddi)
    }

    #[must_use]
    pub fn to_engineering(ddi: u16, raw: i32) -> f64 {
        ddi_to_engineering(ddi, raw)
    }

    #[must_use]
    pub fn from_engineering(ddi: u16, eng: f64) -> i32 {
        ddi_from_engineering(ddi, eng)
    }

    #[must_use]
    pub fn is_geometry_ddi(ddi: u16) -> bool {
        ddi_is_geometry(ddi)
    }

    #[must_use]
    pub fn is_rate_ddi(ddi: u16) -> bool {
        ddi_is_rate(ddi)
    }

    #[must_use]
    pub fn is_total_ddi(ddi: u16) -> bool {
        ddi_is_total(ddi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const DDI_SOURCE_MANIFEST: &str =
        include_str!("../../../../tests/fixtures/isobus/tc_ddi_source_manifest.txt");

    #[test]
    fn ddi_reference_classification_distinguishes_known_proprietary_unknown() {
        // A real database DDI classifies as Known.
        assert_eq!(
            classify_ddi(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE),
            DdiClass::Known
        );
        // A DDI in the proprietary range is custom-but-acceptable.
        assert_eq!(classify_ddi(60000), DdiClass::Proprietary);
        assert!(ddi_is_acceptable(60000));
        // The first undefined DDI below the proprietary range is Unknown.
        let unknown = (1u16..57344)
            .find(|&d| ddi_lookup(d).is_none())
            .expect("some DDI below the proprietary range is undefined");
        assert_eq!(classify_ddi(unknown), DdiClass::Unknown);
        assert!(!ddi_is_acceptable(unknown));

        // unknown_ddis flags only the unacceptable references.
        let refs = [
            ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE,
            60000,
            unknown,
        ];
        assert_eq!(unknown_ddis(&refs), vec![unknown]);
        // A fully-valid reference set yields no findings.
        assert!(unknown_ddis(&[ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE, 60000]).is_empty());
    }

    fn ddi_source_manifest() -> BTreeMap<&'static str, &'static str> {
        let mut values = BTreeMap::new();
        for (line_no, raw_line) in DDI_SOURCE_MANIFEST.lines().enumerate() {
            let line_no = line_no + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('|').collect();
            assert_eq!(
                fields.len(),
                2,
                "DDI source manifest line {line_no} must have key|value"
            );
            let key = fields[0].trim();
            let value = fields[1].trim();
            assert!(
                !key.is_empty() && !value.is_empty(),
                "DDI source manifest line {line_no} must not contain empty fields"
            );
            assert!(
                values.insert(key, value).is_none(),
                "DDI source manifest key `{key}` is duplicated"
            );
        }
        values
    }

    #[test]
    fn database_has_760_entries() {
        assert_eq!(DDI_DATABASE.len(), DDI_DATABASE_SIZE);
        assert_eq!(DDI_DATABASE_SIZE, 760);
        assert_eq!(DDI_DATABASE_VERSION, 2025121001);
    }

    #[test]
    fn database_fingerprint_matches_snapshot() {
        assert_eq!(ddi_database_fingerprint(), DDI_DATABASE_FINGERPRINT_FNV1A64);
    }

    #[test]
    fn source_provenance_manifest_tracks_generated_version_and_refresh_state() {
        let manifest = ddi_source_manifest();
        for required in [
            "source_url",
            "txt_export_url",
            "latest_known_version",
            "current_generated_version",
            "status",
            "last_checked",
            "refresh_gate",
        ] {
            assert!(
                manifest.contains_key(required),
                "DDI source manifest must contain `{required}`"
            );
        }

        assert!(
            manifest["source_url"].starts_with("https://www.isobus.net/isobus/"),
            "DDI source manifest should point at the public ISOBUS database page"
        );
        assert!(
            manifest["txt_export_url"].starts_with("https://www.isobus.net/isobus/exports/"),
            "DDI source manifest should point at a public database export endpoint"
        );

        let current_generated: u32 = manifest["current_generated_version"]
            .parse()
            .expect("current_generated_version is numeric");
        let latest_known: u32 = manifest["latest_known_version"]
            .parse()
            .expect("latest_known_version is numeric");
        assert_eq!(
            current_generated, DDI_DATABASE_VERSION,
            "DDI source manifest current_generated_version must match DDI_DATABASE_VERSION"
        );
        assert!(
            latest_known >= current_generated,
            "latest known public DDI database version must not be older than the generated table"
        );

        let expected_status = if latest_known > current_generated {
            "needs_refresh"
        } else {
            "current"
        };
        assert_eq!(
            manifest["status"], expected_status,
            "DDI source manifest status must reflect whether the generated table is stale"
        );
        assert!(
            manifest["refresh_gate"].contains("make verify")
                && manifest["refresh_gate"].contains("DDI_DATABASE_VERSION")
                && manifest["refresh_gate"].contains("DDI_DATABASE_FINGERPRINT_FNV1A64"),
            "DDI source manifest refresh_gate must name the required generated constants and validation gate"
        );
    }

    #[test]
    fn lookup_known_ddi() {
        let e = ddi_lookup(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE).unwrap();
        assert_eq!(e.unit, "mm3/m2");
        assert!((e.resolution - 0.01).abs() < 1e-9);
    }

    #[test]
    fn lookup_unknown_ddi() {
        assert!(ddi_lookup(0xABCD).is_none());
        assert_eq!(ddi_name(0xABCD), "Unknown");
        assert_eq!(ddi_unit(0xABCD), "");
        assert_eq!(ddi_unit_description(0xABCD), "Unknown");
        assert_eq!(ddi_resolution(0xABCD), 1.0);
        assert_eq!(
            DDIDatabase::get_entry(0xABCD),
            DataDictionaryEntry::unknown()
        );
    }

    #[test]
    fn agisostack_facade_returns_entry_shape_and_unknown_sentinel() {
        let net_weight = DataDictionary::get_entry(229);
        assert_eq!(net_weight.ddi, 229);
        assert_eq!(net_weight.name, "Actual Net Weight");
        assert_eq!(net_weight.unit_symbol, "g");
        assert_eq!(net_weight.unit_description, "Mass large");
        assert_eq!(net_weight.resolution, 1.0);
        assert_eq!(
            net_weight.display_range,
            (f64::from(i32::MIN), f64::from(i32::MAX))
        );

        let unknown = DataDictionary::get_entry(1957);
        assert_eq!(unknown, DataDictionaryEntry::unknown());
    }

    #[test]
    fn generated_unit_symbols_have_descriptions() {
        for entry in DDI_DATABASE {
            let description = ddi_unit_description_for(entry);
            assert_ne!(
                description, "Unknown",
                "DDI {} ({}) has uncovered unit symbol {:?}",
                entry.ddi, entry.name, entry.unit
            );
        }

        assert_eq!(ddi_unit_description(ddi::INTERNAL_DATA_BASE_DDI), "n.a.");
        assert_eq!(ddi_unit_description(ddi::ACTUAL_WORKING_WIDTH), "Length");
        assert_eq!(
            ddi_unit_description(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE),
            "Volume per area unit"
        );
        assert_eq!(
            ddi_unit_description(ddi::SETPOINT_MASS_PER_MASS_APPLICATION_RATE),
            "Mass per mass unit"
        );
        assert_eq!(
            ddi_unit_description(ddi::ACTUAL_COEFFICIENT_OF_VARIATION_OF_SEED_SPACING_PERCENTAGE),
            "Parts per million"
        );
    }

    #[test]
    fn engineering_conversion_round_trip() {
        let ddi = ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE;
        // resolution = 0.01, so raw=100 → 1.0 engineering
        assert!((ddi_to_engineering(ddi, 100) - 1.0).abs() < 1e-9);
        assert_eq!(ddi_from_engineering(ddi, 1.0), 100);
    }

    #[test]
    fn engineering_conversion_clamps_unencodable_values() {
        let ddi = ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE;
        assert_eq!(ddi_from_engineering(ddi, f64::NAN), 0);
        assert_eq!(ddi_from_engineering(ddi, f64::INFINITY), 0);
        assert_eq!(ddi_from_engineering(ddi, f64::NEG_INFINITY), 0);
        assert_eq!(ddi_from_engineering(ddi, 1.0e20), i32::MAX);
        assert_eq!(ddi_from_engineering(ddi, -1.0e20), i32::MIN);
    }

    #[test]
    fn category_helpers() {
        assert!(ddi_is_rate(1u16));
        assert!(ddi_is_rate(55u16));
        assert!(!ddi_is_rate(56u16));
        assert!(ddi_is_rate(ddi::ACTUAL_APPLICATION_RATE_OF_PHOSPHOR));
        assert!(ddi_is_rate(
            ddi::SETPOINT_ELECTRICAL_ENERGY_PER_AREA_APPLICATION_RATE
        ));
        assert!(ddi_is_rate(
            ddi::MAXIMUM_VOLUME_PER_DISTANCE_APPLICATION_RATE
        ));
        assert!(ddi_is_proprietary(57344));
        assert!(ddi_is_proprietary(65534));
        assert!(!ddi_is_proprietary(65535));
        assert!(ddi_is_geometry(134u16));
        assert!(ddi_is_geometry(140u16));
    }

    #[test]
    fn legacy_class_facade() {
        assert_eq!(DDIDatabase::version(), DDI_DATABASE_VERSION);
        assert_eq!(DDIDatabase::fingerprint(), DDI_DATABASE_FINGERPRINT_FNV1A64);
        assert!(DDIDatabase::is_rate_ddi(1));
        let e = DDIDatabase::lookup(0).unwrap();
        assert_eq!(e.ddi, 0);
    }

    #[test]
    fn aliases_resolve_to_canonical() {
        assert_eq!(ddi::WORKING_WIDTH, ddi::ACTUAL_WORKING_WIDTH);
        assert_eq!(
            ddi::SETPOINT_VOLUME_PER_AREA,
            ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE
        );
    }

    #[test]
    fn database_is_sorted_by_ddi() {
        for w in DDI_DATABASE.windows(2) {
            assert!(w[0].ddi < w[1].ddi, "{} >= {}", w[0].ddi, w[1].ddi);
        }
    }
}
