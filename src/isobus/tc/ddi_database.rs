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
//! Naming follows the C++ exactly. Data layout is byte-compatible.

#![allow(missing_docs)]
#![allow(clippy::excessive_precision)]

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("ddi_database/ddi_entry_types.rs");
include!("ddi_database/ddi_generated_table.rs");
include!("ddi_database/ddi_lookup_and_fingerprint.rs");
