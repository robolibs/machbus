//! Helpers that extract implement geometry, sections, and rates from a
//! [`DDOP`].
//!
//! Mirrors the C++ `machbus::isobus::tc::DDOPHelpers`. Free functions
//! plus the class-style `DDOPHelpers` static façade for source-level
//! parity.

use alloc::{string::String, vec::Vec};

use super::ddi_database::{ddi, ddi_is_rate, ddi_is_total};
use super::ddop::DDOP;
use super::objects::{
    DDI, DeviceElement, DeviceElementType, DeviceProcessData, DeviceProperty, ElementNumber,
    ObjectID,
};

/// One section's geometry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionInfo {
    pub element_id: ObjectID,
    pub number: ElementNumber,
    pub designator: String,
    /// Present when the section has a fixed DeviceProperty X offset.
    pub offset_x: Option<i32>,
    /// X offset from connector (positive = forward), mm.
    pub offset_x_mm: i32,
    /// Present when the section has a fixed DeviceProperty Y offset.
    pub offset_y: Option<i32>,
    /// Y offset from center (positive = left), mm.
    pub offset_y_mm: i32,
    /// Present when the section has a fixed DeviceProperty Z offset.
    pub offset_z: Option<i32>,
    pub offset_z_mm: i32,
    /// Present when the section has a fixed DeviceProperty width.
    pub width: Option<i32>,
    pub width_mm: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubBoomInfo {
    pub element_id: ObjectID,
    pub number: ElementNumber,
    pub designator: String,
    pub offset_x: Option<i32>,
    pub offset_y: Option<i32>,
    pub offset_z: Option<i32>,
    pub sections: Vec<SectionInfo>,
    pub rates: Vec<RateInfo>,
}

/// Whole-implement geometry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImplementGeometry {
    pub connector_x_mm: i32,
    pub boom_offset_x_mm: i32,
    pub boom_offset_y_mm: i32,
    pub total_width_mm: i32,
    pub sections: Vec<SectionInfo>,
    pub sub_booms: Vec<SubBoomInfo>,
}

/// Lightweight info on a rate / total DDI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateInfo {
    pub process_data_id: ObjectID,
    pub ddi: DDI,
    pub designator: String,
    pub trigger_methods: u8,
    pub presentation_id: ObjectID,
    /// Fixed value for DeviceProperty-backed rates/totals. `None` means the
    /// entry came from DeviceProcessData and is runtime process data.
    pub value: Option<i32>,
    /// Whether the value is writable process data. DeviceProperty entries are
    /// definition-time constants and therefore not editable.
    pub editable: bool,
}

impl Default for RateInfo {
    fn default() -> Self {
        Self {
            process_data_id: ObjectID(0),
            ddi: DDI(0),
            designator: String::new(),
            trigger_methods: 0,
            presentation_id: ObjectID::NULL,
            value: None,
            editable: false,
        }
    }
}

// ─── Free helpers ─────────────────────────────────────────────────────

#[must_use]
pub fn extract_geometry(ddop: &DDOP) -> ImplementGeometry {
    let mut geo = ImplementGeometry::default();

    if let Some(elem) = ddop
        .elements()
        .iter()
        .find(|e| e.r#type == DeviceElementType::Connector)
    {
        let offsets = element_geometry(ddop, elem);
        geo.connector_x_mm = offsets.x.unwrap_or(0);
        geo.boom_offset_y_mm = offsets.y.unwrap_or(0);
    }

    let boom = ddop
        .elements()
        .iter()
        .find(|e| e.r#type == DeviceElementType::Function);

    if let Some(elem) = boom {
        let offsets = element_geometry(ddop, elem);
        let (bx, by) = (offsets.x.unwrap_or(0), offsets.y.unwrap_or(0));
        geo.boom_offset_x_mm = bx;
        geo.boom_offset_y_mm = by;
        geo.sub_booms = ddop
            .elements()
            .iter()
            .filter(|e| e.r#type == DeviceElementType::Function && e.parent_id == elem.id)
            .map(|sub| {
                let sub_offsets = element_geometry(ddop, sub);
                SubBoomInfo {
                    element_id: sub.id,
                    number: sub.number,
                    designator: sub.designator.clone(),
                    offset_x: sub_offsets.x,
                    offset_y: sub_offsets.y,
                    offset_z: sub_offsets.z,
                    sections: ddop
                        .elements()
                        .iter()
                        .filter(|section| {
                            section.r#type == DeviceElementType::Section
                                && section.parent_id == sub.id
                        })
                        .map(|section| section_info(ddop, section))
                        .collect(),
                    rates: rates_for_element_tree(ddop, sub),
                }
            })
            .collect();
    }

    for elem in ddop.elements() {
        if elem.r#type != DeviceElementType::Section {
            continue;
        }
        if let Some(boom) = boom
            && elem.parent_id != boom.id
        {
            continue;
        }
        geo.sections.push(section_info(ddop, elem));
    }

    geo.total_width_mm = geo.sections.iter().map(|s| s.width_mm).sum();
    geo
}

#[must_use]
pub fn extract_rates(ddop: &DDOP) -> Vec<RateInfo> {
    let mut rates: Vec<RateInfo> = ddop
        .process_data()
        .iter()
        .filter(|pd| ddi_is_rate(pd.ddi))
        .map(rate_info_for_process_data)
        .collect();
    rates.extend(
        ddop.properties()
            .iter()
            .filter(|prop| ddi_is_rate(prop.ddi))
            .map(rate_info_for_property),
    );
    rates
}

#[must_use]
pub fn extract_totals(ddop: &DDOP) -> Vec<RateInfo> {
    let mut totals: Vec<RateInfo> = ddop
        .process_data()
        .iter()
        .filter(|pd| ddi_is_total(pd.ddi))
        .map(|pd| RateInfo {
            process_data_id: pd.id,
            ddi: pd.ddi,
            designator: pd.designator.clone(),
            trigger_methods: pd.trigger_methods,
            presentation_id: pd.presentation_object_id,
            value: None,
            editable: true,
        })
        .collect();
    totals.extend(
        ddop.properties()
            .iter()
            .filter(|prop| ddi_is_total(prop.ddi))
            .map(|prop| RateInfo {
                process_data_id: prop.id,
                ddi: prop.ddi,
                designator: prop.designator.clone(),
                trigger_methods: 0,
                presentation_id: prop.presentation_object_id,
                value: Some(prop.value),
                editable: false,
            }),
    );
    totals
}

#[must_use]
pub fn section_count(ddop: &DDOP) -> u16 {
    u16::try_from(
        ddop.elements()
            .iter()
            .filter(|e| e.r#type == DeviceElementType::Section)
            .count(),
    )
    .unwrap_or(u16::MAX)
}

#[must_use]
pub fn section_count_checked(ddop: &DDOP) -> Option<u16> {
    u16::try_from(
        ddop.elements()
            .iter()
            .filter(|e| e.r#type == DeviceElementType::Section)
            .count(),
    )
    .ok()
}

#[must_use]
pub fn section_count_usize(ddop: &DDOP) -> usize {
    ddop.elements()
        .iter()
        .filter(|e| e.r#type == DeviceElementType::Section)
        .count()
}

#[must_use]
pub fn find_parent_element(ddop: &DDOP, child_id: impl Into<ObjectID>) -> Option<&DeviceElement> {
    let child_id = child_id.into();
    ddop.elements()
        .iter()
        .find(|e| e.child_objects.contains(&child_id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ElementGeometry {
    x: Option<i32>,
    y: Option<i32>,
    z: Option<i32>,
}

fn element_geometry(ddop: &DDOP, elem: &DeviceElement) -> ElementGeometry {
    let mut geometry = ElementGeometry::default();
    for &cid in &elem.child_objects {
        for prop in ddop.properties() {
            if prop.id == cid {
                if prop.ddi == ddi::DEVICE_ELEMENT_OFFSET_X
                    || prop.ddi == ddi::CONNECTOR_PIVOT_X_OFFSET
                {
                    geometry.x = Some(prop.value);
                } else if prop.ddi == ddi::DEVICE_ELEMENT_OFFSET_Y {
                    geometry.y = Some(prop.value);
                } else if prop.ddi == ddi::DEVICE_ELEMENT_OFFSET_Z {
                    geometry.z = Some(prop.value);
                }
            }
        }
    }
    geometry
}

fn section_info(ddop: &DDOP, elem: &DeviceElement) -> SectionInfo {
    let offsets = element_geometry(ddop, elem);
    let mut width = None;
    for &cid in &elem.child_objects {
        for prop in ddop.properties() {
            if prop.id == cid && prop.ddi == ddi::ACTUAL_WORKING_WIDTH {
                width = Some(prop.value);
            }
        }
        for prop in ddop.properties() {
            if prop.id == cid && prop.ddi == ddi::MAXIMUM_WORKING_WIDTH && width.is_none() {
                width = Some(prop.value);
            }
        }
    }
    SectionInfo {
        element_id: elem.id,
        number: elem.number,
        designator: elem.designator.clone(),
        offset_x: offsets.x,
        offset_x_mm: offsets.x.unwrap_or(0),
        offset_y: offsets.y,
        offset_y_mm: offsets.y.unwrap_or(0),
        offset_z: offsets.z,
        offset_z_mm: offsets.z.unwrap_or(0),
        width,
        width_mm: width.unwrap_or(0),
    }
}

fn rates_for_element_tree(ddop: &DDOP, elem: &DeviceElement) -> Vec<RateInfo> {
    let mut rates = Vec::new();
    collect_rates_for_children(ddop, &elem.child_objects, &mut rates);
    rates
}

fn collect_rates_for_children(ddop: &DDOP, child_ids: &[ObjectID], rates: &mut Vec<RateInfo>) {
    for &child_id in child_ids {
        for pd in ddop.process_data() {
            if pd.id == child_id && ddi_is_rate(pd.ddi) {
                rates.push(rate_info_for_process_data(pd));
            }
        }
        for prop in ddop.properties() {
            if prop.id == child_id && ddi_is_rate(prop.ddi) {
                rates.push(rate_info_for_property(prop));
            }
        }
        if let Some(child_elem) = ddop.elements().iter().find(|elem| elem.id == child_id) {
            collect_rates_for_children(ddop, &child_elem.child_objects, rates);
        }
    }
}

fn rate_info_for_process_data(pd: &DeviceProcessData) -> RateInfo {
    RateInfo {
        process_data_id: pd.id,
        ddi: pd.ddi,
        designator: pd.designator.clone(),
        trigger_methods: pd.trigger_methods,
        presentation_id: pd.presentation_object_id,
        value: None,
        editable: true,
    }
}

fn rate_info_for_property(prop: &DeviceProperty) -> RateInfo {
    RateInfo {
        process_data_id: prop.id,
        ddi: prop.ddi,
        designator: prop.designator.clone(),
        trigger_methods: 0,
        presentation_id: prop.presentation_object_id,
        value: Some(prop.value),
        editable: false,
    }
}

/// Class-style façade. Source-level parity with the C++
/// `DDOPHelpers`.
pub struct DDOPHelpers;

impl DDOPHelpers {
    #[must_use]
    pub fn extract_geometry(ddop: &DDOP) -> ImplementGeometry {
        extract_geometry(ddop)
    }

    #[must_use]
    pub fn extract_rates(ddop: &DDOP) -> Vec<RateInfo> {
        extract_rates(ddop)
    }

    #[must_use]
    pub fn extract_totals(ddop: &DDOP) -> Vec<RateInfo> {
        extract_totals(ddop)
    }

    #[must_use]
    pub fn section_count(ddop: &DDOP) -> u16 {
        section_count(ddop)
    }

    #[must_use]
    pub fn section_count_checked(ddop: &DDOP) -> Option<u16> {
        section_count_checked(ddop)
    }

    #[must_use]
    pub fn section_count_usize(ddop: &DDOP) -> usize {
        section_count_usize(ddop)
    }

    pub fn find_parent_element(ddop: &DDOP, child_id: ObjectID) -> Option<&DeviceElement> {
        find_parent_element(ddop, child_id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::objects::{DeviceObject, DeviceProcessData, DeviceProperty};
    use super::*;

    fn build_ddop() -> DDOP {
        DDOP::default()
            .with_device(
                DeviceObject::default()
                    .with_id(1)
                    .with_designator("Sprayer"),
            )
            // Connector with X offset.
            .with_property(
                DeviceProperty::default()
                    .with_id(50)
                    .with_ddi(ddi::CONNECTOR_PIVOT_X_OFFSET)
                    .with_value(1000),
            )
            .with_element(
                DeviceElement::default()
                    .with_id(2)
                    .with_type(DeviceElementType::Connector)
                    .with_children(vec![50]),
            )
            // Two sections, each with offset + width.
            .with_property(
                DeviceProperty::default()
                    .with_id(60)
                    .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Y)
                    .with_value(-2500),
            )
            .with_property(
                DeviceProperty::default()
                    .with_id(61)
                    .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                    .with_value(2000),
            )
            .with_element(
                DeviceElement::default()
                    .with_id(3)
                    .with_type(DeviceElementType::Section)
                    .with_number(1)
                    .with_designator("S1")
                    .with_children(vec![60, 61]),
            )
            .with_property(
                DeviceProperty::default()
                    .with_id(70)
                    .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Y)
                    .with_value(2500),
            )
            .with_property(
                DeviceProperty::default()
                    .with_id(71)
                    .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                    .with_value(2000),
            )
            .with_element(
                DeviceElement::default()
                    .with_id(4)
                    .with_type(DeviceElementType::Section)
                    .with_number(2)
                    .with_designator("S2")
                    .with_children(vec![70, 71]),
            )
            // One rate process data.
            .with_process_data(
                DeviceProcessData::default()
                    .with_id(80)
                    .with_ddi(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE)
                    .with_designator("Rate"),
            )
    }

    #[test]
    fn extract_geometry_finds_sections_and_total_width() {
        let ddop = build_ddop();
        let geo = extract_geometry(&ddop);
        assert_eq!(geo.connector_x_mm, 1000);
        assert_eq!(geo.sections.len(), 2);
        assert_eq!(geo.sections[0].width_mm, 2000);
        assert_eq!(geo.sections[0].offset_y_mm, -2500);
        assert_eq!(geo.sections[1].offset_y_mm, 2500);
        assert_eq!(geo.total_width_mm, 4000);
    }

    #[test]
    fn extract_rates_filters_by_ddi_range() {
        let ddop = build_ddop();
        let rates = extract_rates(&ddop);
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].ddi, ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE);
        assert_eq!(rates[0].value, None);
        assert!(rates[0].editable);
    }

    #[test]
    fn extract_rates_includes_property_backed_rate_constants() {
        let ddop = DDOP::default()
            .with_property(
                DeviceProperty::default()
                    .with_id(46)
                    .with_ddi(ddi::SETPOINT_MASS_PER_AREA_APPLICATION_RATE)
                    .with_value(7000)
                    .with_designator("Rate Setpoint"),
            )
            .with_property(
                DeviceProperty::default()
                    .with_id(47)
                    .with_ddi(ddi::DEFAULT_MASS_PER_AREA_APPLICATION_RATE)
                    .with_value(8000)
                    .with_designator("Rate Default"),
            );
        let rates = extract_rates(&ddop);
        assert_eq!(rates.len(), 2);
        assert_eq!(rates[0].process_data_id, ObjectID(46));
        assert_eq!(rates[0].value, Some(7000));
        assert!(!rates[0].editable);
        assert_eq!(rates[1].process_data_id, ObjectID(47));
        assert_eq!(rates[1].value, Some(8000));
        assert!(!rates[1].editable);
    }

    #[test]
    fn section_count_matches() {
        assert_eq!(section_count(&build_ddop()), 2);
        assert_eq!(section_count_checked(&build_ddop()), Some(2));
        assert_eq!(section_count_usize(&build_ddop()), 2);
    }

    #[test]
    fn section_count_does_not_wrap_for_invalid_oversized_pools() {
        let mut ddop = DDOP::default();
        for idx in 0..=u16::MAX {
            ddop = ddop.with_element(
                DeviceElement::default()
                    .with_id(idx)
                    .with_type(DeviceElementType::Section),
            );
        }

        assert_eq!(section_count_usize(&ddop), usize::from(u16::MAX) + 1);
        assert_eq!(section_count_checked(&ddop), None);
        assert_eq!(section_count(&ddop), u16::MAX);
    }

    #[test]
    fn find_parent_locates_owner_element() {
        let ddop = build_ddop();
        let parent = find_parent_element(&ddop, 60).unwrap();
        assert_eq!(parent.id, 3);
        assert!(find_parent_element(&ddop, 9999).is_none());
    }

    #[test]
    fn class_facade_delegates() {
        let ddop = build_ddop();
        assert_eq!(DDOPHelpers::section_count(&ddop), 2);
    }
}
