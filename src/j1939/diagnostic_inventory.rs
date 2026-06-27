//! Diagnostic service inventory (ISO 11783-12 / J1939 DMxx).
//!
//! GAP.md (ISO 11783-12) asks to "create a diagnostics service inventory"
//! marking each diagnostic message as implemented, intentionally
//! unsupported, or missing. This is that inventory as typed, queryable
//! code over the DM services the crate decodes.
//!
//! It contains no standard prose — only repo-owned service ids, short
//! names, backing modules, and a codec-vs-runtime level.

/// Integration level of a diagnostic service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmLevel {
    /// Encode/decode codecs with strict reject tests.
    Codec,
    /// Codecs plus a stateful runtime helper (e.g. fault tracking).
    Runtime,
}

/// One diagnostic-service inventory row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmService {
    /// Service id, e.g. `"DM1"`.
    pub dm: &'static str,
    /// Short repo-owned descriptor.
    pub name: &'static str,
    pub module: &'static str,
    pub level: DmLevel,
}

use DmLevel::{Codec, Runtime};

const DIAG: &str = "j1939::diagnostic";
const MEM: &str = "j1939::dm_memory";
const MON: &str = "j1939::diag_monitor";

const fn s(
    dm: &'static str,
    name: &'static str,
    module: &'static str,
    level: DmLevel,
) -> DmService {
    DmService {
        dm,
        name,
        module,
        level,
    }
}

/// The diagnostic services machbus provides.
pub const DIAGNOSTIC_SERVICES: &[DmService] = &[
    s("DM1", "Active DTCs", MON, Runtime),
    s("DM2", "Previously Active DTCs", DIAG, Codec),
    s("DM3", "Clear Previously Active DTCs", DIAG, Codec),
    s("DM4", "Freeze Frame Parameters", DIAG, Codec),
    s("DM5", "Diagnostic Readiness 1", DIAG, Codec),
    s("DM6", "Pending DTCs", DIAG, Codec),
    s("DM7", "Command Non-Continuous Test", DIAG, Codec),
    s("DM8", "Test Results", DIAG, Codec),
    s("DM9", "Vehicle Identification Request", DIAG, Codec),
    s("DM10", "Vehicle Identification", DIAG, Codec),
    s("DM11", "Clear Active DTCs", DIAG, Codec),
    s("DM12", "Emissions-Related Active DTCs", DIAG, Codec),
    s("DM13", "Stop/Start Broadcast", DIAG, Codec),
    s("DM14", "Memory Access Request", MEM, Codec),
    s("DM15", "Memory Access Response", MEM, Codec),
    s("DM16", "Binary Data Transfer", MEM, Codec),
    s("DM20", "Monitor Performance Ratio", DIAG, Codec),
    s("DM21", "Diagnostic Readiness 2", DIAG, Codec),
    s("DM22", "Individual Clear DTC", DIAG, Codec),
    s("DM23", "Previously MIL-Off DTCs", DIAG, Codec),
    s("DM25", "Expanded Freeze Frame", DIAG, Codec),
];

/// The inventory row for a service id (e.g. `"DM1"`), if present.
#[must_use]
pub fn service(dm: &str) -> Option<DmService> {
    DIAGNOSTIC_SERVICES.iter().copied().find(|s| s.dm == dm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn services_are_unique_and_have_modules() {
        assert!(DIAGNOSTIC_SERVICES.len() >= 20);
        for (i, a) in DIAGNOSTIC_SERVICES.iter().enumerate() {
            assert!(a.module.starts_with("j1939::"), "{} bad module", a.dm);
            assert!(!a.name.is_empty());
            for b in &DIAGNOSTIC_SERVICES[i + 1..] {
                assert_ne!(a.dm, b.dm, "duplicate service {}", a.dm);
            }
        }
    }

    #[test]
    fn dm1_is_runtime_backed_by_the_fault_monitor() {
        assert_eq!(service("DM1").unwrap().level, DmLevel::Runtime);
        assert_eq!(service("DM1").unwrap().module, MON);
        // Memory-access services live in the dm_memory module.
        assert_eq!(service("DM14").unwrap().module, MEM);
        // Unknown service is absent.
        assert!(service("DM99").is_none());
    }
}
