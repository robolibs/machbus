//! ISO 11783-10 Device Descriptor Object Pool (DDOP).
//!
//! Mirrors the C++ `machbus::isobus::tc::DDOP` (~590 LOC). Holds the
//! four object lists, plus serialize / deserialize / validate / ISOXML
//! emitters.

use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

use super::objects::{
    DDI, DeviceElement, DeviceElementType, DeviceObject, DeviceProcessData, DeviceProperty,
    DeviceValuePresentation, ElementNumber, ObjectID, TCObjectType,
};
use crate::net::error::{Error, ErrorCode, Result};

/// Device Descriptor Object Pool.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DDOP {
    devices: Vec<DeviceObject>,
    elements: Vec<DeviceElement>,
    process_data: Vec<DeviceProcessData>,
    properties: Vec<DeviceProperty>,
    value_presentations: Vec<DeviceValuePresentation>,
    next_id: ObjectID,
}

impl DDOP {
    pub fn next_id(&mut self) -> ObjectID {
        self.allocate_next_id().unwrap_or(ObjectID::NULL)
    }

    pub fn add_device(&mut self, mut obj: DeviceObject) -> Result<ObjectID> {
        if obj.designator.is_empty() {
            return Err(Error::invalid_state("device designator is required"));
        }
        if obj.id == 0 {
            obj.id = self.allocate_next_id()?;
        }
        let id = obj.id;
        self.devices.push(obj);
        Ok(id)
    }

    pub fn add_element(&mut self, mut elem: DeviceElement) -> Result<ObjectID> {
        if elem.id == 0 {
            elem.id = self.allocate_next_id()?;
        }
        let id = elem.id;
        self.elements.push(elem);
        Ok(id)
    }

    pub fn add_process_data(&mut self, mut pd: DeviceProcessData) -> Result<ObjectID> {
        if pd.id == 0 {
            pd.id = self.allocate_next_id()?;
        }
        let id = pd.id;
        self.process_data.push(pd);
        Ok(id)
    }

    pub fn add_property(&mut self, mut prop: DeviceProperty) -> Result<ObjectID> {
        if prop.id == 0 {
            prop.id = self.allocate_next_id()?;
        }
        let id = prop.id;
        self.properties.push(prop);
        Ok(id)
    }

    pub fn add_value_presentation(&mut self, mut vp: DeviceValuePresentation) -> Result<ObjectID> {
        if vp.id == 0 {
            vp.id = self.allocate_next_id()?;
        }
        let id = vp.id;
        self.value_presentations.push(vp);
        Ok(id)
    }

    fn allocate_next_id(&mut self) -> Result<ObjectID> {
        if self.object_count() >= u16::MAX as usize {
            return Err(Error::with_message(
                ErrorCode::PoolValidation,
                "DDOP has no available object identifiers",
            ));
        }

        let mut candidate = self.next_id;
        for _ in 0..=usize::from(u16::MAX) {
            if candidate != ObjectID::NULL && !self.object_exists(candidate) {
                self.next_id = ObjectID(candidate.0.wrapping_add(1));
                if self.next_id == ObjectID::NULL {
                    self.next_id = ObjectID(0);
                }
                return Ok(candidate);
            }
            candidate += 1;
        }

        Err(Error::with_message(
            ErrorCode::PoolValidation,
            "DDOP has no available object identifiers",
        ))
    }

    /// Serialize the full pool.
    ///
    /// The DDOP wire format stores text lengths in one byte and this Rust
    /// surface currently stores text as UTF-8 [`String`] values. Until an
    /// explicit ISO-11783 text encoding adapter is added, serialization rejects
    /// non-ASCII and overlong strings instead of emitting payloads that cannot
    /// be decoded back into the same bytes.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        self.validate_serializable()?;

        let mut data = Vec::new();
        for d in &self.devices {
            data.extend(d.serialize()?);
        }
        for e in &self.elements {
            data.extend(e.serialize()?);
        }
        for pd in &self.process_data {
            data.extend(pd.serialize()?);
        }
        for p in &self.properties {
            data.extend(p.serialize()?);
        }
        for vp in &self.value_presentations {
            data.extend(vp.serialize()?);
        }
        Ok(data)
    }

    /// Validate parent / child / presentation references.
    pub fn validate(&self) -> Result<()> {
        if self.devices.is_empty() {
            return Err(Error::invalid_state("DDOP must have at least one device"));
        }
        if self.elements.is_empty() {
            return Err(Error::invalid_state(
                "DDOP must have at least one device element",
            ));
        }
        self.validate_serializable()?;
        self.validate_unique_object_ids()?;
        for elem in &self.elements {
            if elem.parent_id == elem.id {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    "element parent references itself",
                ));
            }
            if elem.parent_id != 0 {
                match self.object_kind(elem.parent_id) {
                    Some(TCObjectType::Device | TCObjectType::DeviceElement) => {}
                    Some(_) => {
                        return Err(Error::with_message(
                            ErrorCode::PoolValidation,
                            "element parent references unsupported object type",
                        ));
                    }
                    None => {
                        return Err(Error::with_message(
                            ErrorCode::PoolValidation,
                            "element references non-existent parent",
                        ));
                    }
                }
            }
            for &cid in &elem.child_objects {
                if cid == elem.id {
                    return Err(Error::with_message(
                        ErrorCode::PoolValidation,
                        "element child list references itself",
                    ));
                }
                match self.object_kind(cid) {
                    Some(
                        TCObjectType::DeviceElement
                        | TCObjectType::DeviceProcessData
                        | TCObjectType::DeviceProperty,
                    ) => {}
                    Some(_) => {
                        return Err(Error::with_message(
                            ErrorCode::PoolValidation,
                            "element child references unsupported object type",
                        ));
                    }
                    None => {
                        return Err(Error::with_message(
                            ErrorCode::PoolValidation,
                            "element references non-existent child object",
                        ));
                    }
                }
            }
        }
        for pd in &self.process_data {
            if pd.presentation_object_id != 0xFFFF
                && pd.presentation_object_id != 0
                && !self.vp_exists(pd.presentation_object_id)
            {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    "process data references non-existent presentation object",
                ));
            }
        }
        for p in &self.properties {
            if p.presentation_object_id != 0xFFFF
                && p.presentation_object_id != 0
                && !self.vp_exists(p.presentation_object_id)
            {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    "property references non-existent presentation object",
                ));
            }
        }
        Ok(())
    }

    fn validate_serializable(&self) -> Result<()> {
        for dev in &self.devices {
            if dev.designator.is_empty() {
                return Err(Error::invalid_state("device designator is required"));
            }
            validate_wire_text("device designator", &dev.designator)?;
            validate_wire_text("device software version", &dev.software_version)?;
            validate_wire_text("device serial number", &dev.serial_number)?;
        }
        for elem in &self.elements {
            validate_wire_text("device element designator", &elem.designator)?;
            if elem.child_objects.len() > u16::MAX as usize {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    "device element has too many child object references",
                ));
            }
        }
        for pd in &self.process_data {
            pd.validate_trigger_methods()?;
            validate_wire_text("process data designator", &pd.designator)?;
        }
        for prop in &self.properties {
            validate_wire_text("property designator", &prop.designator)?;
        }
        for vp in &self.value_presentations {
            if !vp.scale.is_finite() {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    "value presentation scale must be finite",
                ));
            }
            validate_wire_text("value presentation unit designator", &vp.unit_designator)?;
        }
        Ok(())
    }

    fn validate_unique_object_ids(&self) -> Result<()> {
        let mut ids = Vec::with_capacity(self.object_count());
        for id in self.devices.iter().map(|d| d.id) {
            push_unique_id(&mut ids, id)?;
        }
        for id in self.elements.iter().map(|e| e.id) {
            push_unique_id(&mut ids, id)?;
        }
        for id in self.process_data.iter().map(|pd| pd.id) {
            push_unique_id(&mut ids, id)?;
        }
        for id in self.properties.iter().map(|p| p.id) {
            push_unique_id(&mut ids, id)?;
        }
        for id in self.value_presentations.iter().map(|vp| vp.id) {
            push_unique_id(&mut ids, id)?;
        }
        Ok(())
    }

    // ─── Accessors ────────────────────────────────────────────────────

    #[must_use]
    pub fn devices(&self) -> &[DeviceObject] {
        &self.devices
    }

    #[must_use]
    pub fn elements(&self) -> &[DeviceElement] {
        &self.elements
    }

    #[must_use]
    pub fn process_data(&self) -> &[DeviceProcessData] {
        &self.process_data
    }

    #[must_use]
    pub fn properties(&self) -> &[DeviceProperty] {
        &self.properties
    }

    #[must_use]
    pub fn value_presentations(&self) -> &[DeviceValuePresentation] {
        &self.value_presentations
    }

    #[must_use]
    pub fn object_count(&self) -> usize {
        self.devices.len()
            + self.elements.len()
            + self.process_data.len()
            + self.properties.len()
            + self.value_presentations.len()
    }

    pub fn clear(&mut self) {
        self.devices.clear();
        self.elements.clear();
        self.process_data.clear();
        self.properties.clear();
        self.value_presentations.clear();
        self.next_id = ObjectID(0);
    }

    // ─── Fluent API ───────────────────────────────────────────────────

    #[must_use]
    pub fn with_device(mut self, d: DeviceObject) -> Self {
        let _ = self.add_device(d);
        self
    }

    #[must_use]
    pub fn with_element(mut self, e: DeviceElement) -> Self {
        let _ = self.add_element(e);
        self
    }

    #[must_use]
    pub fn with_process_data(mut self, pd: DeviceProcessData) -> Self {
        let _ = self.add_process_data(pd);
        self
    }

    #[must_use]
    pub fn with_property(mut self, p: DeviceProperty) -> Self {
        let _ = self.add_property(p);
        self
    }

    #[must_use]
    pub fn with_value_presentation(mut self, vp: DeviceValuePresentation) -> Self {
        let _ = self.add_value_presentation(vp);
        self
    }

    // ─── Deserialization ──────────────────────────────────────────────

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut ddop = Self::default();
        let mut offset = 0usize;
        while offset < data.len() {
            if offset + 3 > data.len() {
                return Err(truncated_error());
            }
            let obj_type_byte = data[offset];
            let obj_id = ObjectID((data[offset + 1] as u16) | ((data[offset + 2] as u16) << 8));
            offset += 3;
            match obj_type_byte {
                t if t == TCObjectType::Device.as_u8() => {
                    let dev = parse_device(data, &mut offset, obj_id)?;
                    ddop.devices.push(dev);
                }
                t if t == TCObjectType::DeviceElement.as_u8() => {
                    let e = parse_element(data, &mut offset, obj_id)?;
                    ddop.elements.push(e);
                }
                t if t == TCObjectType::DeviceProcessData.as_u8() => {
                    let pd = parse_process_data(data, &mut offset, obj_id)?;
                    ddop.process_data.push(pd);
                }
                t if t == TCObjectType::DeviceProperty.as_u8() => {
                    let p = parse_property(data, &mut offset, obj_id)?;
                    ddop.properties.push(p);
                }
                t if t == TCObjectType::DeviceValuePresentation.as_u8() => {
                    let vp = parse_value_presentation(data, &mut offset, obj_id)?;
                    ddop.value_presentations.push(vp);
                }
                _ => {
                    return Err(Error::with_message(
                        ErrorCode::PoolValidation,
                        "unknown TC object type in DDOP",
                    ));
                }
            }
            if obj_id >= ddop.next_id {
                ddop.next_id = obj_id + 1u16;
            }
        }
        Ok(ddop)
    }

    // ─── ISOXML emission ──────────────────────────────────────────────

    /// Generate an ISO 11783-10 TASKDATA.xml fragment for this DDOP.
    #[must_use]
    pub fn to_isoxml(&self) -> String {
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str(
            "<ISO11783_TaskData VersionMajor=\"4\" VersionMinor=\"0\" DataTransferOrigin=\"1\">\n",
        );

        for dev in &self.devices {
            xml.push_str(&format!(
                "  <DVC A=\"DVC-{}\" B=\"{}\" C=\"{}\" D=\"{}\">\n",
                dev.id,
                xml_escape(&dev.designator),
                xml_escape(&dev.software_version),
                xml_escape(&dev.serial_number),
            ));
            for elem in &self.elements {
                xml.push_str(&self.emit_element_xml(elem));
            }
            xml.push_str("  </DVC>\n");
        }

        for vp in &self.value_presentations {
            xml.push_str(&format!(
                "  <DVP A=\"DVP-{}\" B=\"{}\" C=\"{}\" D=\"{}\" E=\"{}\"/>\n",
                vp.id,
                vp.offset,
                vp.scale,
                u32::from(vp.decimal_digits),
                xml_escape(&vp.unit_designator),
            ));
        }

        xml.push_str("</ISO11783_TaskData>\n");
        xml
    }

    fn emit_element_xml(&self, elem: &DeviceElement) -> String {
        let type_str = match elem.r#type {
            DeviceElementType::Device => "1",
            DeviceElementType::Function => "2",
            DeviceElementType::Bin => "3",
            DeviceElementType::Section => "4",
            DeviceElementType::Unit => "5",
            DeviceElementType::Connector => "6",
            DeviceElementType::NavigationReference => "7",
        };
        let mut xml = format!(
            "    <DET A=\"DET-{}\" B=\"{}\" C=\"{}\" D=\"{}\" E=\"DET-{}\">\n",
            elem.id,
            type_str,
            xml_escape(&elem.designator),
            elem.number,
            elem.parent_id,
        );
        for &cid in &elem.child_objects {
            for pd in &self.process_data {
                if pd.id == cid {
                    xml.push_str(&format!(
                        "      <DPD A=\"DPD-{}\" B=\"{}\" C=\"{}\" D=\"{}\"",
                        pd.id,
                        pd.ddi,
                        u32::from(pd.trigger_methods),
                        xml_escape(&pd.designator),
                    ));
                    if pd.presentation_object_id != 0xFFFF {
                        xml.push_str(&format!(" E=\"DVP-{}\"", pd.presentation_object_id));
                    }
                    xml.push_str("/>\n");
                }
            }
            for prop in &self.properties {
                if prop.id == cid {
                    xml.push_str(&format!(
                        "      <DPT A=\"DPT-{}\" B=\"{}\" C=\"{}\" D=\"{}\"",
                        prop.id,
                        prop.ddi,
                        prop.value,
                        xml_escape(&prop.designator),
                    ));
                    if prop.presentation_object_id != 0xFFFF {
                        xml.push_str(&format!(" E=\"DVP-{}\"", prop.presentation_object_id));
                    }
                    xml.push_str("/>\n");
                }
            }
        }
        xml.push_str("    </DET>\n");
        xml
    }

    fn object_exists(&self, id: ObjectID) -> bool {
        self.object_kind(id).is_some()
    }

    fn object_kind(&self, id: ObjectID) -> Option<TCObjectType> {
        if self.devices.iter().any(|d| d.id == id) {
            Some(TCObjectType::Device)
        } else if self.elements.iter().any(|e| e.id == id) {
            Some(TCObjectType::DeviceElement)
        } else if self.process_data.iter().any(|pd| pd.id == id) {
            Some(TCObjectType::DeviceProcessData)
        } else if self.properties.iter().any(|p| p.id == id) {
            Some(TCObjectType::DeviceProperty)
        } else if self.value_presentations.iter().any(|vp| vp.id == id) {
            Some(TCObjectType::DeviceValuePresentation)
        } else {
            None
        }
    }

    fn vp_exists(&self, id: ObjectID) -> bool {
        self.value_presentations.iter().any(|vp| vp.id == id)
    }
}

// ─── Deserialization helpers ──────────────────────────────────────────

fn truncated_error() -> Error {
    Error::with_message(
        ErrorCode::PoolValidation,
        "DDOP data truncated during deserialization",
    )
}

fn read_string(data: &[u8], offset: &mut usize) -> Result<String> {
    if *offset >= data.len() {
        return Err(truncated_error());
    }
    let len = data[*offset] as usize;
    *offset += 1;
    if *offset + len > data.len() {
        return Err(truncated_error());
    }
    let bytes = &data[*offset..*offset + len];
    if !bytes.iter().all(|b| b.is_ascii()) {
        return Err(Error::with_message(
            ErrorCode::PoolValidation,
            "DDOP text contains unsupported non-ASCII bytes",
        ));
    }
    let s = core::str::from_utf8(bytes)
        .map_err(|_| {
            Error::with_message(ErrorCode::PoolValidation, "DDOP text is not valid UTF-8")
        })?
        .to_owned();
    *offset += len;
    Ok(s)
}

fn parse_device(data: &[u8], offset: &mut usize, id: ObjectID) -> Result<DeviceObject> {
    let designator = read_string(data, offset)?;
    let software_version = read_string(data, offset)?;
    let serial_number = read_string(data, offset)?;
    if *offset + 14 > data.len() {
        return Err(truncated_error());
    }
    let mut structure_label = [0u8; 7];
    structure_label.copy_from_slice(&data[*offset..*offset + 7]);
    *offset += 7;
    let mut localization_label = [0u8; 7];
    localization_label.copy_from_slice(&data[*offset..*offset + 7]);
    *offset += 7;
    Ok(DeviceObject {
        id,
        designator,
        software_version,
        serial_number,
        structure_label,
        localization_label,
    })
}

fn parse_element(data: &[u8], offset: &mut usize, id: ObjectID) -> Result<DeviceElement> {
    if *offset >= data.len() {
        return Err(truncated_error());
    }
    let r#type = match data[*offset] {
        1 => DeviceElementType::Device,
        2 => DeviceElementType::Function,
        3 => DeviceElementType::Bin,
        4 => DeviceElementType::Section,
        5 => DeviceElementType::Unit,
        6 => DeviceElementType::Connector,
        7 => DeviceElementType::NavigationReference,
        _ => {
            return Err(Error::with_message(
                ErrorCode::PoolValidation,
                "unknown device element type in DDOP",
            ));
        }
    };
    *offset += 1;
    let designator = read_string(data, offset)?;
    if *offset + 6 > data.len() {
        return Err(truncated_error());
    }
    let number = ElementNumber((data[*offset] as u16) | ((data[*offset + 1] as u16) << 8));
    *offset += 2;
    let parent_id = ObjectID((data[*offset] as u16) | ((data[*offset + 1] as u16) << 8));
    *offset += 2;
    let num_children = (data[*offset] as u16) | ((data[*offset + 1] as u16) << 8);
    *offset += 2;
    let bytes_needed = num_children as usize * 2;
    if *offset + bytes_needed > data.len() {
        return Err(truncated_error());
    }
    let mut children = Vec::with_capacity(num_children as usize);
    for _ in 0..num_children {
        let cid = ObjectID((data[*offset] as u16) | ((data[*offset + 1] as u16) << 8));
        children.push(cid);
        *offset += 2;
    }
    Ok(DeviceElement {
        id,
        r#type,
        number,
        parent_id,
        designator,
        child_objects: children,
    })
}

fn parse_process_data(data: &[u8], offset: &mut usize, id: ObjectID) -> Result<DeviceProcessData> {
    if *offset + 5 > data.len() {
        return Err(truncated_error());
    }
    let ddi = DDI((data[*offset] as u16) | ((data[*offset + 1] as u16) << 8));
    *offset += 2;
    let trigger_methods = data[*offset];
    *offset += 1;
    let presentation_object_id =
        ObjectID((data[*offset] as u16) | ((data[*offset + 1] as u16) << 8));
    *offset += 2;
    let designator = read_string(data, offset)?;
    Ok(DeviceProcessData {
        id,
        ddi,
        trigger_methods,
        presentation_object_id,
        designator,
    })
}

fn parse_property(data: &[u8], offset: &mut usize, id: ObjectID) -> Result<DeviceProperty> {
    if *offset + 8 > data.len() {
        return Err(truncated_error());
    }
    let ddi = DDI((data[*offset] as u16) | ((data[*offset + 1] as u16) << 8));
    *offset += 2;
    let value = i32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    let presentation_object_id =
        ObjectID((data[*offset] as u16) | ((data[*offset + 1] as u16) << 8));
    *offset += 2;
    let designator = read_string(data, offset)?;
    Ok(DeviceProperty {
        id,
        ddi,
        value,
        presentation_object_id,
        designator,
    })
}

fn parse_value_presentation(
    data: &[u8],
    offset: &mut usize,
    id: ObjectID,
) -> Result<DeviceValuePresentation> {
    if *offset + 9 > data.len() {
        return Err(truncated_error());
    }
    let off = i32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    let scale = f32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    if !scale.is_finite() {
        return Err(Error::with_message(
            ErrorCode::PoolValidation,
            "DDOP value presentation scale is not finite",
        ));
    }
    let decimal_digits = data[*offset];
    *offset += 1;
    let unit_designator = read_string(data, offset)?;
    Ok(DeviceValuePresentation {
        id,
        offset: off,
        scale,
        decimal_digits,
        unit_designator,
    })
}

// ─── ISOXML helpers ───────────────────────────────────────────────────

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn validate_wire_text(field: &'static str, value: &str) -> Result<()> {
    if value.len() > u8::MAX as usize {
        return Err(Error::with_message(
            ErrorCode::PoolValidation,
            format!("{field} exceeds the DDOP one-byte text length limit"),
        ));
    }
    if !value.is_ascii() {
        return Err(Error::with_message(
            ErrorCode::PoolValidation,
            format!("{field} contains unsupported non-ASCII text"),
        ));
    }
    Ok(())
}

fn push_unique_id(ids: &mut Vec<ObjectID>, id: ObjectID) -> Result<()> {
    if id == ObjectID::NULL {
        return Err(Error::with_message(
            ErrorCode::PoolValidation,
            "object ID 0xFFFF is reserved",
        ));
    }
    if ids.contains(&id) {
        return Err(Error::with_message(
            ErrorCode::PoolValidation,
            "duplicate DDOP object id",
        ));
    }
    ids.push(id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_ddop() -> DDOP {
        DDOP::default()
            .with_device(
                DeviceObject::default()
                    .with_id(1)
                    .with_designator("Sprayer")
                    .with_software_version("1.0")
                    .with_serial_number("SN1"),
            )
            .with_element(
                DeviceElement::default()
                    .with_id(2)
                    .with_type(DeviceElementType::Device)
                    .with_designator("Root"),
            )
    }

    #[test]
    fn add_device_requires_designator() {
        let mut ddop = DDOP::default();
        assert!(ddop.add_device(DeviceObject::default()).is_err());
    }

    #[test]
    fn auto_assigns_ids_via_next_id() {
        let mut ddop = DDOP::default();
        let id = ddop
            .add_device(DeviceObject::default().with_designator("X"))
            .unwrap();
        assert_eq!(id, 0); // next_id starts at 0
        let id = ddop.add_element(DeviceElement::default()).unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn auto_id_allocator_skips_used_and_reserved_identifiers() {
        let mut ddop = DDOP {
            next_id: ObjectID(0xFFFE),
            ..Default::default()
        };
        ddop.add_device(DeviceObject::default().with_id(0xFFFE).with_designator("X"))
            .unwrap();

        let id = ddop.add_element(DeviceElement::default()).unwrap();
        assert_eq!(id, 0);
        assert_ne!(id, ObjectID::NULL);
        let id = ddop.add_process_data(DeviceProcessData::default()).unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn auto_id_allocator_rejects_exhausted_pool() {
        let mut ddop = DDOP::default();
        ddop.elements
            .extend((0..u16::MAX).map(|_| DeviceElement::default()));
        assert!(ddop.add_element(DeviceElement::default()).is_err());
        assert_eq!(ddop.next_id(), ObjectID::NULL);
    }

    #[test]
    fn validate_requires_devices_and_elements() {
        let ddop = DDOP::default();
        assert!(ddop.validate().is_err());

        let ddop =
            DDOP::default().with_device(DeviceObject::default().with_id(1).with_designator("D"));
        assert!(ddop.validate().is_err()); // no elements

        let ddop = dummy_ddop();
        ddop.validate().unwrap();
    }

    #[test]
    fn validate_catches_bad_parent_ref() {
        let ddop = DDOP::default()
            .with_device(DeviceObject::default().with_id(1).with_designator("D"))
            .with_element(
                DeviceElement::default().with_id(2).with_parent(999), // non-existent
            );
        assert!(ddop.validate().is_err());
    }

    #[test]
    fn validate_catches_bad_child_ref() {
        let ddop = DDOP::default()
            .with_device(DeviceObject::default().with_id(1).with_designator("D"))
            .with_element(DeviceElement::default().with_id(2).with_children(vec![999]));
        assert!(ddop.validate().is_err());
    }

    #[test]
    fn serialize_then_deserialize_round_trip() {
        let original = DDOP::default()
            .with_device(
                DeviceObject::default()
                    .with_id(1)
                    .with_designator("Sprayer")
                    .with_software_version("1.0")
                    .with_serial_number("SN-1234")
                    .with_structure_label([1, 2, 3, 4, 5, 6, 7])
                    .with_localization_label([8, 9, 10, 11, 12, 13, 14]),
            )
            .with_element(
                DeviceElement::default()
                    .with_id(2)
                    .with_type(DeviceElementType::Section)
                    .with_number(5)
                    .with_parent(1)
                    .with_designator("S1")
                    .with_children(vec![10, 20]),
            )
            .with_process_data(
                DeviceProcessData::default()
                    .with_id(10)
                    .with_ddi(0x1234)
                    .with_triggers(0x05)
                    .with_designator("PD1"),
            )
            .with_property(
                DeviceProperty::default()
                    .with_id(20)
                    .with_ddi(0xABCD)
                    .with_value(-42)
                    .with_designator("Prop1"),
            )
            .with_value_presentation(
                DeviceValuePresentation::default()
                    .with_id(30)
                    .with_offset(100)
                    .with_scale(0.001)
                    .with_decimals(3)
                    .with_unit("m"),
            );

        let bytes = original.serialize().unwrap();
        let restored = DDOP::deserialize(&bytes).unwrap();
        assert_eq!(restored.object_count(), 5);
        assert_eq!(restored.devices()[0].designator, "Sprayer");
        assert_eq!(restored.devices()[0].structure_label[0], 1);
        assert_eq!(restored.elements()[0].r#type, DeviceElementType::Section);
        assert_eq!(restored.elements()[0].number, 5);
        assert_eq!(restored.elements()[0].child_objects, vec![10, 20]);
        assert_eq!(restored.process_data()[0].ddi, 0x1234);
        assert_eq!(restored.properties()[0].value, -42);
        assert!((restored.value_presentations()[0].scale - 0.001).abs() < 1e-6);
    }

    #[test]
    fn serialize_rejects_non_finite_value_presentation_scale() {
        for scale in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let ddop = dummy_ddop().with_value_presentation(
                DeviceValuePresentation::default()
                    .with_id(30)
                    .with_scale(scale)
                    .with_unit("m"),
            );
            assert!(ddop.serialize().is_err());
            assert!(ddop.validate().is_err());
        }
    }

    #[test]
    fn deserialize_rejects_non_finite_value_presentation_scale() {
        for scale in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bytes = vec![TCObjectType::DeviceValuePresentation.as_u8(), 0x1E, 0x00];
            bytes.extend_from_slice(&0i32.to_le_bytes());
            bytes.extend_from_slice(&scale.to_le_bytes());
            bytes.push(0);
            bytes.push(1);
            bytes.push(b'm');
            assert!(DDOP::deserialize(&bytes).is_err());
        }
    }

    #[test]
    fn deserialize_truncated_errors() {
        // Header byte only — too short.
        assert!(DDOP::deserialize(&[0x00, 0x01]).is_err());
        // Unknown type tag.
        assert!(DDOP::deserialize(&[0xFF, 0x00, 0x00]).is_err());

        // Device designator declares more bytes than remain.
        assert!(
            DDOP::deserialize(&[TCObjectType::Device.as_u8(), 0x01, 0x00, 0x05, b'A']).is_err()
        );

        // Device Element child count declares far more IDs than remain; this
        // must fail before allocating a child vector from the hostile count.
        assert!(
            DDOP::deserialize(&[
                TCObjectType::DeviceElement.as_u8(),
                0x02,
                0x00,
                DeviceElementType::Device.as_u8(),
                0x00,
                0x01,
                0x00,
                0x00,
                0x00,
                0xFF,
                0xFF,
            ])
            .is_err()
        );
    }

    #[test]
    fn isoxml_smoke_test() {
        let ddop = dummy_ddop();
        let xml = ddop.to_isoxml();
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("<DVC A=\"DVC-1\""));
        assert!(xml.contains("Sprayer"));
        assert!(xml.contains("</ISO11783_TaskData>"));
    }

    #[test]
    fn xml_escape_handles_special_chars() {
        let xml = xml_escape("a&b<c>d\"e'f");
        assert_eq!(xml, "a&amp;b&lt;c&gt;d&quot;e&apos;f");
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_ddop_deserialize_arbitrary_bytes_is_bounded(
            data in proptest::collection::vec(any::<u8>(), 0..=512),
        ) {
            if let Ok(ddop) = DDOP::deserialize(&data) {
                prop_assert!(ddop.object_count() <= data.len() / 3);
                prop_assert!(ddop.value_presentations().iter().all(|vp| vp.scale.is_finite()));
                let _ = ddop.serialize();
                let _ = ddop.validate();
            }
        }
    }
}
