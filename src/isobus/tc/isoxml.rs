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

/// GuidanceAllocation (`<GAN>`) AST node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuidanceAllocation {
    pub id: String,
    pub designator: String,
}

/// GuidancePattern (`<GPN>`) AST node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuidancePattern {
    pub id: String,
    pub designator: String,
}

/// Grid (`<GRD>`) AST node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Grid {
    pub id: String,
    pub grid_type: u8,
}

/// Point (`<PNT>`) AST node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Point {
    pub id: String,
    pub point_type: u8,
}

/// Polygon (`<PLN>`) AST node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Polygon {
    pub id: String,
    pub polygon_type: u8,
}

/// LineString (`<LSG>`) AST node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineString {
    pub id: String,
    pub line_string_type: u8,
}

/// TreatmentZone (`<TZN>`) AST node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TreatmentZone {
    pub code: u8,
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
    /// PTN E — PDOP, 2 bytes.
    pub pdop: bool,
    /// PTN F — HDOP, 2 bytes.
    pub hdop: bool,
    /// PTN G — number of satellites, 1 byte.
    pub satellites: bool,
    /// PTN H — GPS UTC time, 4 bytes.
    pub gps_utc_time: bool,
    /// PTN I — GPS UTC date, 2 bytes.
    pub gps_utc_date: bool,
}

impl PositionFields {
    /// `true` if any position field is logged.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.north
            || self.east
            || self.up
            || self.status
            || self.pdop
            || self.hdop
            || self.satellites
            || self.gps_utc_time
            || self.gps_utc_date
    }

    /// Bytes a position occupies per record: North/East/Up are 32-bit,
    /// Status is one byte.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        (self.north as usize) * 4
            + (self.east as usize) * 4
            + (self.up as usize) * 4
            + (self.status as usize)
            + (self.pdop as usize) * 2
            + (self.hdop as usize) * 2
            + (self.satellites as usize)
            + (self.gps_utc_time as usize) * 4
            + (self.gps_utc_date as usize) * 2
    }
}

/// The record structure declared by a TimeLog header XML file: which
/// position fields are logged per record, and the ordered list of logged
/// process-data channels.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimeLogStructure {
    /// Whether the TimeStart time/date prefix is per-record (§8.6.3).
    pub has_time: bool,
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
    /// Whether this record carries the TimeStart time/date prefix.
    ///
    /// §8.6.3: "All attributes in the header file that contain values are
    /// specified as having these constant values for all records of the binary
    /// file." A header carrying `A="12345"` fixes the time for every record, so
    /// it is *not* in the binary.
    pub has_time: bool,
    /// `(DLVn, ProcessDataValue)` per logged channel.
    ///
    /// Table 3 requires an ordering byte per value — "Ordering number of PDV to
    /// follow, starting with 0 for first DataLogValue definition" — preceded by
    /// `#DLV`, "Number of PDV to follow". Both were missing, so every record
    /// was one byte short of the count and one byte short per channel, and the
    /// decoder walked a stride that no conformant writer produces.
    pub values: Vec<(u8, i32)>,
}

impl TimeLogStructure {
    /// Parse a TimeLog header XML document into its record structure.
    /// Errors if the document is malformed or has no `Time` element.
    pub fn from_header_xml(input: &str) -> Result<Self> {
        let root = parse_xml(input)?;
        let time = root
            .find_first(TIME_ELEMENT)
            .ok_or_else(|| Error::invalid_data("TimeLog header has no Time (TIM) element"))?;
        // F8 — §8.6.3: "Inside the TimeLog XML header file, the XML elements
        // shall include attributes without any values to define the record
        // structure of the binary log file. All attributes with non-empty value
        // definitions in the TimeLog XML header file contain fixed values which
        // are valid for all binary-coded records ... Only the values of the
        // attributes without a value in the header file are stored."
        //
        // Presence was tested with `is_some()`, which is true for a fixed value
        // too — so a header like `<PTN A="520000000" B="" D=""/>` (constant
        // north, varying east and status) made the decoder expect a 4-byte
        // North field that is not in the binary, misaligning every record.
        let logged = |node: &XmlElement, attr: &str| node.attr(attr).is_some_and(str::is_empty);
        let has_time = logged(time, ATTR_ID);
        let position = time
            .find_first(POSITION_ELEMENT)
            .map_or(PositionFields::default(), |ptn| PositionFields {
                north: logged(ptn, "A"),
                east: logged(ptn, "B"),
                up: logged(ptn, "C"),
                status: logged(ptn, "D"),
                pdop: logged(ptn, "E"),
                hdop: logged(ptn, "F"),
                satellites: logged(ptn, "G"),
                gps_utc_time: logged(ptn, "H"),
                gps_utc_date: logged(ptn, "I"),
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
        Ok(Self {
            has_time,
            position,
            values,
        })
    }

    /// `true` if records carry any position field.
    #[must_use]
    pub const fn has_position(&self) -> bool {
        self.position.any()
    }

    /// The *minimum* size of one binary record: the fixed prefix plus the
    /// `#DLV` count byte.
    ///
    /// It is no longer an exact stride. Table 3 gives each value an ordering
    /// byte and the count is per record, so a record's length depends on how
    /// many PDVs it actually carries — which is the point of `#DLV`, "to allow
    /// a dynamic set of DLVs in the binary record". Use
    /// [`decode_record`](Self::decode_record), which reports what it consumed.
    #[must_use]
    pub fn record_size(&self) -> usize {
        self.prefix_len() + 1
    }

    /// Bytes before the `#DLV` count: the optional TimeStart time/date and the
    /// position fields the header declared as per-record.
    #[must_use]
    pub fn prefix_len(&self) -> usize {
        usize::from(self.has_time) * 6 + self.position.byte_len()
    }

    /// Encode a single binary record into ISO 11783-10 `.BIN` format matching this structure.
    #[must_use]
    pub fn encode_record(&self, rec: &TimeLogRecord) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.record_size());
        if self.has_time {
            out.extend_from_slice(&rec.time_ms.to_le_bytes());
            out.extend_from_slice(&rec.date_days.to_le_bytes());
        }
        if self.position.north {
            out.extend_from_slice(&rec.north_1e7_deg.unwrap_or(0).to_le_bytes());
        }
        if self.position.east {
            out.extend_from_slice(&rec.east_1e7_deg.unwrap_or(0).to_le_bytes());
        }
        if self.position.up {
            out.extend_from_slice(&rec.up_mm.unwrap_or(0).to_le_bytes());
        }
        if self.position.status {
            out.push(rec.status.unwrap_or(0));
        }
        // Table 3: `#DLV` then, per value, `DLVn` and the 32-bit PDV. §8.6.3:
        // "This means that a time entry can have a maximum of 255 PDVs."
        let count = u8::try_from(rec.values.len()).unwrap_or(u8::MAX);
        out.push(count);
        for &(index, value) in rec.values.iter().take(count as usize) {
            out.push(index);
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// Decode one binary record, returning it and the bytes consumed.
    ///
    /// Table 3 order: the optional TimeStart time/date, the declared position
    /// fields, then `#DLV` and that many `(DLVn, ProcessDataValue)` pairs.
    /// Records are not a fixed stride — `#DLV` exists precisely "to allow a
    /// dynamic set of DLVs in the binary record" (§8.6.3).
    #[must_use]
    pub fn decode_record(&self, data: &[u8]) -> Option<(TimeLogRecord, usize)> {
        if data.len() < self.record_size() {
            return None;
        }
        let rd_i32 = |o: usize| i32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);

        let mut o = 0usize;
        let (time_ms, date_days) = if self.has_time {
            let t = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            let d = u16::from_le_bytes([data[4], data[5]]);
            o = 6;
            (t, d)
        } else {
            (0, 0)
        };

        let take_i32 = |present: bool, o: &mut usize| -> Option<i32> {
            present.then(|| {
                let v = rd_i32(*o);
                *o += 4;
                v
            })
        };
        let north_1e7_deg = take_i32(self.position.north, &mut o);
        let east_1e7_deg = take_i32(self.position.east, &mut o);
        let up_mm = take_i32(self.position.up, &mut o);
        let status = self.position.status.then(|| {
            let v = data[o];
            o += 1;
            v
        });
        // The remaining Table 3 position columns are skipped rather than
        // surfaced: nothing in the crate consumes them yet, but their widths
        // have to be honoured or every following field misaligns.
        o += usize::from(self.position.pdop) * 2
            + usize::from(self.position.hdop) * 2
            + usize::from(self.position.satellites)
            + usize::from(self.position.gps_utc_time) * 4
            + usize::from(self.position.gps_utc_date) * 2;
        if o >= data.len() {
            return None;
        }

        let count = data[o] as usize;
        o += 1;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            if o + 4 >= data.len() {
                return None;
            }
            let index = data[o];
            // An index outside the header's definitions cannot be resolved.
            if usize::from(index) >= self.values.len() {
                return None;
            }
            values.push((index, rd_i32(o + 1)));
            o += 5;
        }

        Some((
            TimeLogRecord {
                time_ms,
                date_days,
                has_time: self.has_time,
                north_1e7_deg,
                east_1e7_deg,
                up_mm,
                status,
                values,
            },
            o,
        ))
    }

    /// Decode a whole binary TimeLog file. Trailing bytes that do not form a
    /// full record are ignored.
    ///
    /// Advances by what each record actually consumed rather than a fixed
    /// stride: `#DLV` makes the length per-record.
    #[must_use]
    pub fn decode_records(&self, data: &[u8]) -> Vec<TimeLogRecord> {
        let mut out = Vec::new();
        let mut offset = 0usize;
        while offset < data.len() {
            let Some((record, used)) = self.decode_record(&data[offset..]) else {
                break;
            };
            debug_assert!(used > 0, "a record must consume input");
            offset += used;
            out.push(record);
        }
        out
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
        // F3 — §8.6.3 states the record shape outright:
        // "(TimeStart,PositionNorth,PositionEast,PositionStatus,#DLV,DLV0,PDV0,
        //   DLV1,PDV1,DLV2,PDV2)"
        // with `#DLV` = "Number of PDV to follow" and `DLVn` = "Ordering number
        // of PDV to follow, starting with 0 for first DataLogValue definition".
        // Both bytes were missing, so a record was one byte short of the count
        // and one short per channel, and the decoder walked a stride no
        // conformant writer produces.
        let header = r#"<TIM A="">
            <PTN A="" B="" D=""/>
            <DLV A="0001" B="" C="DET1"/>
            <DLV A="0002" B="" C="DET1"/>
        </TIM>"#;
        let s = TimeLogStructure::from_header_xml(header).unwrap();
        // The minimum is the fixed prefix plus #DLV; the rest is per-record.
        assert_eq!(s.prefix_len(), 4 + 2 + 4 + 4 + 1);
        assert_eq!(s.record_size(), s.prefix_len() + 1);

        let mut buf = Vec::new();
        buf.extend_from_slice(&1_000_u32.to_le_bytes()); // time_ms
        buf.extend_from_slice(&45_u16.to_le_bytes()); // date_days
        buf.extend_from_slice(&520_000_000_i32.to_le_bytes()); // North 52.0°
        buf.extend_from_slice(&53_000_000_i32.to_le_bytes()); // East 5.3°
        buf.push(2); // Status = DGNSS
        buf.push(2); // #DLV
        buf.push(0); // DLV0
        buf.extend_from_slice(&1234_i32.to_le_bytes());
        buf.push(1); // DLV1
        buf.extend_from_slice(&5678_i32.to_le_bytes());

        let (rec, used) = s.decode_record(&buf).unwrap();
        assert_eq!(used, buf.len());
        assert_eq!(rec.time_ms, 1_000);
        assert_eq!(rec.date_days, 45);
        assert_eq!(rec.north_1e7_deg, Some(520_000_000));
        assert_eq!(rec.east_1e7_deg, Some(53_000_000));
        assert_eq!(rec.up_mm, None);
        assert_eq!(rec.status, Some(2));
        assert_eq!(rec.values, vec![(0, 1234), (1, 5678)]);
        assert_eq!(s.encode_record(&rec), buf, "and it round trips");

        // "To allow a dynamic set of DLVs in the binary record": the next
        // record may carry a different subset, so the stride is not fixed.
        let mut sparse = buf[..s.prefix_len()].to_vec();
        sparse.push(1); // #DLV
        sparse.push(1); // only DLV1 this time
        sparse.extend_from_slice(&99_i32.to_le_bytes());
        let (rec, used) = s.decode_record(&sparse).unwrap();
        assert_eq!(used, sparse.len());
        assert_eq!(rec.values, vec![(1, 99)]);

        // Two records of *different* lengths still both decode.
        let mut two = buf.clone();
        two.extend_from_slice(&sparse);
        two.push(0xFF); // partial trailing byte
        assert_eq!(s.decode_records(&two).len(), 2);

        // Too-short buffer yields None.
        assert!(s.decode_record(&buf[..10]).is_none());
        // A DLVn outside the header's definitions cannot be resolved.
        let mut bad_index = buf[..s.prefix_len()].to_vec();
        bad_index.push(1);
        bad_index.push(7);
        bad_index.extend_from_slice(&1_i32.to_le_bytes());
        assert!(s.decode_record(&bad_index).is_none());
    }

    /// F8 — §8.6.3: "All attributes with non-empty value definitions in the
    /// TimeLog XML header file contain fixed values which are valid for all
    /// binary-coded records ... Only the values of the attributes without a
    /// value in the header file are stored."
    ///
    /// Presence was tested with `is_some()`, which is true for a fixed value
    /// too, so a header carrying constants made the decoder expect fields that
    /// are not in the binary at all.
    #[test]
    fn time_log_header_distinguishes_fixed_values_from_logged_fields() {
        // Constant north, varying east and status; the time is fixed too.
        let header = r#"<TIM A="12345">
            <PTN A="520000000" B="" D=""/>
            <DLV A="0001" B="" C="DET1"/>
        </TIM>"#;
        let s = TimeLogStructure::from_header_xml(header).unwrap();
        assert!(!s.has_time, "a fixed TimeStart is not per-record");
        assert!(!s.position.north, "a fixed north is not per-record");
        assert!(s.position.east);
        assert!(s.position.status);
        assert_eq!(s.prefix_len(), 4 + 1, "East(4) + Status(1) only");
    }
}
