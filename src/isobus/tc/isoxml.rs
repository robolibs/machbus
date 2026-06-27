//! ISOXML / TASKDATA import (ISO 11783-10 task data interchange).
//!
//! Task data is exchanged as an XML document (`TASKDATA.XML`) whose root
//! is `ISO11783_TaskData` and whose task records are `TSK` elements. This
//! module provides:
//!
//! - [`parse_xml`] — a small, self-contained, dependency-free parser for
//!   the attribute-oriented XML subset ISOXML uses (elements, quoted
//!   attributes, nesting, self-closing tags; declarations/comments
//!   skipped). It is generic XML machinery and contains no standard
//!   content.
//! - [`TaskData`] — a thin typed view that validates the root element and
//!   exposes the document's `TSK` task records (id `A`, designator `B`),
//!   which feed the task runtime ([`crate::isobus::tc::TaskSession`]).
//!
//! Typed records are exposed for tasks (`TSK`), devices (`DVC`), device
//! elements (`DET`), partfields (`PFD`), task time-log references (`TLG`),
//! and the TimeLog header record structure (`TimeLogStructure`). The binary
//! time-log payload decode and the full attribute set are left to later
//! slices. Unknown elements and attributes are preserved generically (via
//! [`XmlElement`]) rather than dropped.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use crate::net::error::{Error, Result};

/// A parsed XML element: tag name, ordered attributes, and child elements.
/// Text content is ignored (ISOXML is attribute-based).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct XmlElement {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<XmlElement>,
}

impl XmlElement {
    /// First attribute value for `key`, if present.
    #[must_use]
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Direct children whose tag name equals `name`.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlElement> {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// First element named `name` anywhere in the subtree (self first, then
    /// a depth-first search of descendants).
    #[must_use]
    pub fn find_first(&self, name: &str) -> Option<&XmlElement> {
        if self.name == name {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find_first(name))
    }
}

/// Parse an ISOXML-subset document and return its root element.
///
/// Supports nested elements, single/double-quoted attributes, and
/// self-closing tags; skips the `<?xml …?>` declaration, `<!-- … -->`
/// comments, and `<!DOCTYPE …>`. Malformed input (unterminated tag,
/// mismatched close tag, no root) is rejected rather than panicking.
pub fn parse_xml(input: &str) -> Result<XmlElement> {
    let bytes = input.as_bytes();
    let mut pos = 0usize;
    let mut stack: Vec<XmlElement> = Vec::new();
    let mut root: Option<XmlElement> = None;

    while pos < bytes.len() {
        // Advance to the next '<'.
        match input[pos..].find('<') {
            Some(off) => pos += off,
            None => break,
        }
        let rest = &input[pos..];

        if rest.starts_with("<?") {
            pos += find_after(rest, "?>")?;
        } else if rest.starts_with("<!--") {
            pos += find_after(rest, "-->")?;
        } else if rest.starts_with("<!") {
            pos += find_after(rest, ">")?;
        } else if rest.starts_with("</") {
            // Close tag.
            let end = rest
                .find('>')
                .ok_or_else(|| xml_err("unterminated end tag"))?;
            let name = rest[2..end].trim();
            let elem = stack.pop().ok_or_else(|| xml_err("unmatched end tag"))?;
            if elem.name != name {
                return Err(xml_err("mismatched end tag"));
            }
            attach(&mut stack, &mut root, elem)?;
            pos += end + 1;
        } else {
            // Start (or self-closing) tag.
            let end = rest
                .find('>')
                .ok_or_else(|| xml_err("unterminated start tag"))?;
            let self_closing = rest[..end].ends_with('/');
            let inner = rest[1..end].trim_end_matches('/').trim();
            let elem = parse_start_tag(inner)?;
            if self_closing {
                attach(&mut stack, &mut root, elem)?;
            } else {
                stack.push(elem);
            }
            pos += end + 1;
        }
    }

    if !stack.is_empty() {
        return Err(xml_err("unclosed element(s) at end of document"));
    }
    root.ok_or_else(|| xml_err("document has no root element"))
}

/// Attach a finished element to its parent, or set it as the root.
fn attach(stack: &mut [XmlElement], root: &mut Option<XmlElement>, elem: XmlElement) -> Result<()> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(elem);
        Ok(())
    } else if root.is_none() {
        *root = Some(elem);
        Ok(())
    } else {
        Err(xml_err("multiple root elements"))
    }
}

/// Parse `name attr="v" attr2='v2'` into an element with no children.
fn parse_start_tag(inner: &str) -> Result<XmlElement> {
    let inner = inner.trim();
    let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
    let name = inner[..name_end].to_string();
    if name.is_empty() {
        return Err(xml_err("empty tag name"));
    }
    let mut attributes = Vec::new();
    let mut rest = inner[name_end..].trim_start();
    while !rest.is_empty() {
        let eq = rest
            .find('=')
            .ok_or_else(|| xml_err("attribute without '='"))?;
        let key = rest[..eq].trim().to_string();
        let after = rest[eq + 1..].trim_start();
        let quote = after
            .chars()
            .next()
            .filter(|c| *c == '"' || *c == '\'')
            .ok_or_else(|| xml_err("attribute value not quoted"))?;
        let after = &after[1..];
        let close = after
            .find(quote)
            .ok_or_else(|| xml_err("unterminated attribute value"))?;
        attributes.push((key, after[..close].to_string()));
        rest = after[close + 1..].trim_start();
    }
    Ok(XmlElement {
        name,
        attributes,
        children: Vec::new(),
    })
}

/// Position just past the first occurrence of `needle` in `hay`.
fn find_after(hay: &str, needle: &str) -> Result<usize> {
    hay.find(needle)
        .map(|i| i + needle.len())
        .ok_or_else(|| xml_err("unterminated markup"))
}

fn xml_err(msg: &str) -> Error {
    Error::invalid_data(format!("malformed ISOXML: {msg}"))
}

/// The ISOXML root element name.
const ROOT_ELEMENT: &str = "ISO11783_TaskData";
/// Element tags. Repo-owned structural identifiers (the TASKDATA file
/// format), not standard prose.
const TASK_ELEMENT: &str = "TSK";
const DEVICE_ELEMENT: &str = "DVC";
const DEVICE_ELEMENT_ELEMENT: &str = "DET";
const PARTFIELD_ELEMENT: &str = "PFD";
const TIMELOG_ELEMENT: &str = "TLG";
/// Common id attribute (`@A`) shared by TSK / DVC / DET / PFD records, and
/// the filename attribute of TLG records.
const ATTR_ID: &str = "A";
/// Designator attribute per element: TSK and DVC use `@B`, PFD and DET use
/// their own letters below.
const ATTR_DESIGNATOR: &str = "B";
const ATTR_PARTFIELD_DESIGNATOR: &str = "C";
const ATTR_DEVICE_ELEMENT_DESIGNATOR: &str = "D";
/// TimeLog header elements: Time / Position / DataLogValue.
const TIME_ELEMENT: &str = "TIM";
const POSITION_ELEMENT: &str = "PTN";
const DATA_LOG_VALUE_ELEMENT: &str = "DLV";
/// DataLogValue attributes: `@A` process-data DDI, `@C` device-element ref.
const ATTR_DLV_ELEMENT_REF: &str = "C";

/// One task record from a TASKDATA document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Task {
    /// Task id (`TSK@A`, e.g. `"TSK1"`).
    pub id: String,
    /// Human-readable designator (`TSK@B`).
    pub designator: String,
}

/// One device record (`DVC`) from a TASKDATA document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Device {
    /// Device id (`DVC@A`, e.g. `"DVC1"`).
    pub id: String,
    /// Device designator (`DVC@B`).
    pub designator: String,
}

/// One device-element record (`DET`, nested under a `DVC`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceElement {
    /// Device-element id (`DET@A`, e.g. `"DET1"`).
    pub id: String,
    /// Device-element designator (`DET@D`).
    pub designator: String,
}

/// One partfield record (`PFD`) from a TASKDATA document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Partfield {
    /// Partfield id (`PFD@A`, e.g. `"PFD1"`).
    pub id: String,
    /// Partfield designator (`PFD@C`).
    pub designator: String,
}

/// A parsed TASKDATA document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskData {
    pub root: XmlElement,
}

impl TaskData {
    /// Parse and validate a TASKDATA.XML document.
    pub fn from_xml(input: &str) -> Result<Self> {
        let root = parse_xml(input)?;
        if root.name != ROOT_ELEMENT {
            return Err(Error::invalid_data(format!(
                "TASKDATA root element must be {ROOT_ELEMENT}, found {}",
                root.name
            )));
        }
        Ok(Self { root })
    }

    /// The document's task records (direct `TSK` children of the root).
    #[must_use]
    pub fn tasks(&self) -> Vec<Task> {
        self.root
            .children_named(TASK_ELEMENT)
            .map(|t| Task {
                id: t.attr(ATTR_ID).unwrap_or_default().to_string(),
                designator: t.attr(ATTR_DESIGNATOR).unwrap_or_default().to_string(),
            })
            .collect()
    }

    /// The document's device records (direct `DVC` children of the root).
    #[must_use]
    pub fn devices(&self) -> Vec<Device> {
        self.root
            .children_named(DEVICE_ELEMENT)
            .map(|d| Device {
                id: d.attr(ATTR_ID).unwrap_or_default().to_string(),
                designator: d.attr(ATTR_DESIGNATOR).unwrap_or_default().to_string(),
            })
            .collect()
    }

    /// The document's partfield records (direct `PFD` children of the root).
    #[must_use]
    pub fn partfields(&self) -> Vec<Partfield> {
        self.root
            .children_named(PARTFIELD_ELEMENT)
            .map(|p| Partfield {
                id: p.attr(ATTR_ID).unwrap_or_default().to_string(),
                designator: p
                    .attr(ATTR_PARTFIELD_DESIGNATOR)
                    .unwrap_or_default()
                    .to_string(),
            })
            .collect()
    }

    /// Every device-element record (`DET`), flattened across all devices.
    #[must_use]
    pub fn device_elements(&self) -> Vec<DeviceElement> {
        self.root
            .children_named(DEVICE_ELEMENT)
            .flat_map(|d| d.children_named(DEVICE_ELEMENT_ELEMENT))
            .map(|e| DeviceElement {
                id: e.attr(ATTR_ID).unwrap_or_default().to_string(),
                designator: e
                    .attr(ATTR_DEVICE_ELEMENT_DESIGNATOR)
                    .unwrap_or_default()
                    .to_string(),
            })
            .collect()
    }

    /// Binary time-log filenames (`TLG@A`) referenced by the tasks.
    #[must_use]
    pub fn time_log_filenames(&self) -> Vec<String> {
        self.root
            .children_named(TASK_ELEMENT)
            .flat_map(|t| t.children_named(TIMELOG_ELEMENT))
            .filter_map(|tlg| tlg.attr(ATTR_ID).map(str::to_string))
            .collect()
    }
}

/// One logged process-data channel in a TimeLog record (`DLV`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoggedValue {
    /// Process-data DDI (`DLV@A`, hex string e.g. `"0815"`).
    pub ddi: String,
    /// Device-element reference (`DLV@C`, e.g. `"DET1"`).
    pub device_element: String,
}

/// Which position fields a TimeLog record carries, declared by the empty
/// attributes of the header's `PTN` element (`@A` North, `@B` East,
/// `@C` Up, `@D` Status), in that wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PositionFields {
    pub north: bool,
    pub east: bool,
    pub up: bool,
    pub status: bool,
}

impl PositionFields {
    /// `true` if any position field is logged.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.north || self.east || self.up || self.status
    }

    /// Bytes a position occupies per record: North/East/Up are 32-bit,
    /// Status is one byte.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        (self.north as usize) * 4
            + (self.east as usize) * 4
            + (self.up as usize) * 4
            + (self.status as usize)
    }
}

/// The record structure declared by a TimeLog header XML file: which
/// position fields are logged per record, and the ordered list of logged
/// process-data channels.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimeLogStructure {
    pub position: PositionFields,
    pub values: Vec<LoggedValue>,
}

/// One decoded binary TimeLog record (ISO 11783-10 Table 3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimeLogRecord {
    /// Local-time-zone milliseconds since midnight.
    pub time_ms: u32,
    /// Days since the epoch (1980-01-01).
    pub date_days: u16,
    pub north_1e7_deg: Option<i32>,
    pub east_1e7_deg: Option<i32>,
    pub up_mm: Option<i32>,
    pub status: Option<u8>,
    /// One signed 32-bit value per logged channel, in header order.
    pub values: Vec<i32>,
}

impl TimeLogStructure {
    /// Parse a TimeLog header XML document into its record structure.
    /// Errors if the document is malformed or has no `Time` element.
    pub fn from_header_xml(input: &str) -> Result<Self> {
        let root = parse_xml(input)?;
        let time = root
            .find_first(TIME_ELEMENT)
            .ok_or_else(|| Error::invalid_data("TimeLog header has no Time (TIM) element"))?;
        let position = time
            .find_first(POSITION_ELEMENT)
            .map_or(PositionFields::default(), |ptn| PositionFields {
                north: ptn.attr(ATTR_ID).is_some(),
                east: ptn.attr(ATTR_DESIGNATOR).is_some(),
                up: ptn.attr(ATTR_DLV_ELEMENT_REF).is_some(),
                status: ptn.attr(ATTR_DEVICE_ELEMENT_DESIGNATOR).is_some(),
            });
        let values = time
            .children_named(DATA_LOG_VALUE_ELEMENT)
            .map(|dlv| LoggedValue {
                ddi: dlv.attr(ATTR_ID).unwrap_or_default().to_string(),
                device_element: dlv
                    .attr(ATTR_DLV_ELEMENT_REF)
                    .unwrap_or_default()
                    .to_string(),
            })
            .collect();
        Ok(Self { position, values })
    }

    /// `true` if records carry any position field.
    #[must_use]
    pub const fn has_position(&self) -> bool {
        self.position.any()
    }

    /// Size in bytes of one binary record under this structure:
    /// time (u32) + date (u16) + position fields + one i32 per channel.
    #[must_use]
    pub fn record_size(&self) -> usize {
        4 + 2 + self.position.byte_len() + self.values.len() * 4
    }

    /// Decode one binary record (ISO 11783-10 Table 3 field order: time,
    /// date, North/East/Up/Status, then each channel value). Returns `None`
    /// if `data` is shorter than [`record_size`](Self::record_size).
    #[must_use]
    pub fn decode_record(&self, data: &[u8]) -> Option<TimeLogRecord> {
        if data.len() < self.record_size() {
            return None;
        }
        let rd_u32 =
            |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        let rd_i32 =
            |o: usize| i32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);

        let time_ms = rd_u32(0);
        let date_days = u16::from_le_bytes([data[4], data[5]]);
        let mut o = 6usize;
        let mut take_i32 = |present: bool| -> Option<i32> {
            present.then(|| {
                let v = rd_i32(o);
                o += 4;
                v
            })
        };
        let north_1e7_deg = take_i32(self.position.north);
        let east_1e7_deg = take_i32(self.position.east);
        let up_mm = take_i32(self.position.up);
        let status = self.position.status.then(|| {
            let v = data[o];
            o += 1;
            v
        });
        let values = (0..self.values.len())
            .map(|_| {
                let v = rd_i32(o);
                o += 4;
                v
            })
            .collect();
        Some(TimeLogRecord {
            time_ms,
            date_days,
            north_1e7_deg,
            east_1e7_deg,
            up_mm,
            status,
            values,
        })
    }

    /// Decode a whole binary TimeLog file into records. Trailing bytes that
    /// do not form a full record are ignored.
    #[must_use]
    pub fn decode_records(&self, data: &[u8]) -> Vec<TimeLogRecord> {
        let size = self.record_size();
        data.chunks_exact(size)
            .filter_map(|chunk| self.decode_record(chunk))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attributes_nesting_and_self_closing() {
        let xml = r#"<?xml version="1.0"?>
            <root a="1" b='two'>
                <child x="9"/>
                <wrap><leaf/></wrap>
            </root>"#;
        let root = parse_xml(xml).unwrap();
        assert_eq!(root.name, "root");
        assert_eq!(root.attr("a"), Some("1"));
        assert_eq!(root.attr("b"), Some("two"));
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].name, "child");
        assert_eq!(root.children[0].attr("x"), Some("9"));
        assert_eq!(root.children[1].children[0].name, "leaf");
    }

    #[test]
    fn skips_comments_and_doctype() {
        let xml = r#"<!-- a comment --><!DOCTYPE foo><root><a/></root>"#;
        let root = parse_xml(xml).unwrap();
        assert_eq!(root.name, "root");
        assert_eq!(root.children.len(), 1);
    }

    #[test]
    fn rejects_malformed_documents() {
        assert!(parse_xml("").is_err()); // no root
        assert!(parse_xml("<a><b></a>").is_err()); // mismatched
        assert!(parse_xml("<a>").is_err()); // unclosed
        assert!(parse_xml("<a x=1></a>").is_err()); // unquoted attr
        assert!(parse_xml("<a></a><b></b>").is_err()); // two roots
    }

    #[test]
    fn taskdata_validates_root_and_enumerates_tasks() {
        let xml = r#"<ISO11783_TaskData VersionMajor="4">
            <TSK A="TSK1" B="North Field"/>
            <TSK A="TSK2" B="South Field"></TSK>
        </ISO11783_TaskData>"#;
        let td = TaskData::from_xml(xml).unwrap();
        let tasks = td.tasks();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "TSK1");
        assert_eq!(tasks[0].designator, "North Field");
        assert_eq!(tasks[1].id, "TSK2");
        assert_eq!(tasks[1].designator, "South Field");
    }

    #[test]
    fn taskdata_rejects_wrong_root() {
        assert!(TaskData::from_xml("<NotTaskData/>").is_err());
    }

    #[test]
    fn taskdata_enumerates_devices_and_partfields() {
        let xml = r#"<ISO11783_TaskData VersionMajor="4">
            <DVC A="DVC1" B="Sprayer ECU"/>
            <DVC A="DVC2" B="Tractor ECU"/>
            <PFD A="PFD1" C="North 40" D="404686"/>
            <TSK A="TSK1" B="Spray North" DvcRef="DVC1" PfdRef="PFD1"/>
        </ISO11783_TaskData>"#;
        let td = TaskData::from_xml(xml).unwrap();

        let devices = td.devices();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "DVC1");
        assert_eq!(devices[0].designator, "Sprayer ECU");
        assert_eq!(devices[1].designator, "Tractor ECU");

        let partfields = td.partfields();
        assert_eq!(partfields.len(), 1);
        assert_eq!(partfields[0].id, "PFD1");
        assert_eq!(partfields[0].designator, "North 40");

        // Tasks still enumerate alongside devices/partfields.
        assert_eq!(td.tasks().len(), 1);
    }

    #[test]
    fn taskdata_enumerates_device_elements_and_time_logs() {
        let xml = r#"<ISO11783_TaskData VersionMajor="4">
            <DVC A="DVC1" B="Sprayer ECU">
                <DET A="DET1" C="1" D="Boom"/>
                <DET A="DET2" C="4" D="Section 1"/>
            </DVC>
            <TSK A="TSK1" B="Spray North">
                <TLG A="TLG00001" C="1"/>
                <TLG A="TLG00002" C="1"/>
            </TSK>
        </ISO11783_TaskData>"#;
        let td = TaskData::from_xml(xml).unwrap();

        let elements = td.device_elements();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].id, "DET1");
        assert_eq!(elements[0].designator, "Boom");
        assert_eq!(elements[1].designator, "Section 1");

        assert_eq!(
            td.time_log_filenames(),
            vec!["TLG00001".to_string(), "TLG00002".to_string()]
        );
    }

    #[test]
    fn time_log_header_structure_is_parsed() {
        // A TimeLog header: empty-value attrs declare the binary record
        // structure (TimeStart, Position, two logged DDIs).
        let header = r#"<TIM A="" D="4">
            <PTN A="" B="" D=""/>
            <DLV A="0815" B="" C="DET1"/>
            <DLV A="0816" B="" C="DET2"/>
        </TIM>"#;
        let s = TimeLogStructure::from_header_xml(header).unwrap();
        assert!(s.has_position());
        assert!(s.position.north && s.position.east && s.position.status);
        assert!(!s.position.up);
        assert_eq!(s.values.len(), 2);
        assert_eq!(s.values[0].ddi, "0815");
        assert_eq!(s.values[0].device_element, "DET1");
        assert_eq!(s.values[1].ddi, "0816");

        // No-position header.
        let no_pos = r#"<TIM A=""><DLV A="0001" B="" C="DET1"/></TIM>"#;
        let s = TimeLogStructure::from_header_xml(no_pos).unwrap();
        assert!(!s.has_position());
        assert_eq!(s.values.len(), 1);

        // Missing Time element is rejected.
        assert!(TimeLogStructure::from_header_xml("<root/>").is_err());
    }

    #[test]
    fn time_log_binary_records_decode_per_table_3() {
        // Structure: position North+East+Status + one value channel.
        let header = r#"<TIM A="">
            <PTN A="" B="" D=""/>
            <DLV A="0001" B="" C="DET1"/>
        </TIM>"#;
        let s = TimeLogStructure::from_header_xml(header).unwrap();
        // record = time(4) + date(2) + North(4) + East(4) + Status(1) + value(4) = 19
        assert_eq!(s.record_size(), 19);

        let mut buf = Vec::new();
        buf.extend_from_slice(&1_000_u32.to_le_bytes()); // time_ms
        buf.extend_from_slice(&45_u16.to_le_bytes()); // date_days
        buf.extend_from_slice(&520_000_000_i32.to_le_bytes()); // North 52.0°
        buf.extend_from_slice(&53_000_000_i32.to_le_bytes()); // East 5.3°
        buf.push(2); // Status = DGNSS
        buf.extend_from_slice(&1234_i32.to_le_bytes()); // value

        let rec = s.decode_record(&buf).unwrap();
        assert_eq!(rec.time_ms, 1_000);
        assert_eq!(rec.date_days, 45);
        assert_eq!(rec.north_1e7_deg, Some(520_000_000));
        assert_eq!(rec.east_1e7_deg, Some(53_000_000));
        assert_eq!(rec.up_mm, None);
        assert_eq!(rec.status, Some(2));
        assert_eq!(rec.values, vec![1234]);

        // Two concatenated records decode; a short tail is ignored.
        let mut two = buf.clone();
        two.extend_from_slice(&buf);
        two.push(0xFF); // partial trailing byte
        assert_eq!(s.decode_records(&two).len(), 2);

        // Too-short buffer yields None.
        assert!(s.decode_record(&buf[..10]).is_none());
    }
}
