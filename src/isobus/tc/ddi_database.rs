//! ISO 11783-11 Data Dictionary (DDI) database (760 entries).
//!
//! Mirrors the C++ `machbus::isobus::tc::ddi_database.hpp` (Version
//! `2025121001`).
//!
//! The database is generated from the ISO 11783-11 online database;
//! see the C++ source for the licence notice (data is supplied by
//! ISO without liability and may not be redistributed except as part
//! of an implementation).
//!
//! The standard itself carries no DDI values to check these against. ISO
//! 11783-11:2011 is three pages: §4.1 says the process-data variables "shall be
//! as defined in the ISOBUS Data Dictionary, accessible at the ISOBUS website",
//! maintained by VDMA as the ISO-appointed maintenance agency. What §4.2 *does*
//! fix is the shape of every entry — "identification number; process data
//! element definition; range of the process data element; resolution of the
//! process data element; units of the process data element" — which is exactly
//! [`DDIDefinition`]'s `ddi` / `name` / `min_value`-`max_value` / `resolution` /
//! `unit`. Individual DDI ranges and semantics are therefore sourced from the
//! online dictionary, not from any document in the ISO 11783 series.
//!
//! Naming follows the C++ exactly. Data layout is byte-compatible.

#![allow(missing_docs)]
#![allow(clippy::excessive_precision)]

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("ddi_database/ddi_entry_types.rs");
include!("ddi_database/ddi_generated_table.rs");
include!("ddi_database/ddi_lookup_and_fingerprint.rs");
