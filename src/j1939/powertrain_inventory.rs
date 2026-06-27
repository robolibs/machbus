//! Powertrain message inventory (ISO 11783-8 / J1939).
//!
//! GAP.md (ISO 11783-8) asks to "create a powertrain message inventory" and
//! to "decide whether machbus claims a powertrain ECU, a decoder library,
//! or selected agricultural message support." This module records both:
//! the typed inventory of the powertrain messages the crate decodes, and
//! the explicit claim ([`POWERTRAIN_CLAIM`]).
//!
//! It contains no standard prose — only repo-owned message names, their
//! group, and their backing module.

use alloc::vec::Vec;

/// The claim machbus makes about its powertrain support: a decoder library
/// for selected agricultural powertrain messages — **not** a powertrain ECU
/// application.
pub const POWERTRAIN_CLAIM: PowertrainClaim = PowertrainClaim::SelectedDecoderLibrary;

/// What the crate claims for the powertrain area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowertrainClaim {
    /// Encode/decode codecs for a selected set of messages; not an ECU.
    SelectedDecoderLibrary,
    /// A full powertrain ECU application (not claimed).
    PowertrainEcu,
}

/// Functional group of a powertrain message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowertrainGroup {
    Engine,
    Transmission,
    SpeedDistance,
}

/// One inventory row: a decoded powertrain message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowertrainMessage {
    pub name: &'static str,
    pub group: PowertrainGroup,
    pub module: &'static str,
}

use PowertrainGroup::{Engine, SpeedDistance, Transmission};

const fn m(name: &'static str, group: PowertrainGroup, module: &'static str) -> PowertrainMessage {
    PowertrainMessage {
        name,
        group,
        module,
    }
}

const ENGINE: &str = "j1939::engine";
const TRANSMISSION: &str = "j1939::transmission";
const SPEED: &str = "j1939::speed_distance";

/// The powertrain messages machbus decodes.
pub const POWERTRAIN_MESSAGES: &[PowertrainMessage] = &[
    m("EEC1", Engine, ENGINE),
    m("EEC2", Engine, ENGINE),
    m("EEC3", Engine, ENGINE),
    m("EngineTemperature1", Engine, ENGINE),
    m("EngineTemperature2", Engine, ENGINE),
    m("EngineFluidLevelPressure", Engine, ENGINE),
    m("EngineHours", Engine, ENGINE),
    m("FuelEconomy", Engine, ENGINE),
    m("TSC1", Engine, ENGINE),
    m("VEP1", Engine, ENGINE),
    m("ETC1", Transmission, TRANSMISSION),
    m("TransmissionOilTemperature", Transmission, TRANSMISSION),
    m("CruiseControl", Transmission, TRANSMISSION),
    m("SpeedAndDistance", SpeedDistance, SPEED),
];

/// Messages belonging to a group.
#[must_use]
pub fn messages_in(group: PowertrainGroup) -> Vec<PowertrainMessage> {
    POWERTRAIN_MESSAGES
        .iter()
        .copied()
        .filter(|msg| msg.group == group)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_is_a_decoder_library_not_an_ecu() {
        assert_eq!(POWERTRAIN_CLAIM, PowertrainClaim::SelectedDecoderLibrary);
        assert_ne!(POWERTRAIN_CLAIM, PowertrainClaim::PowertrainEcu);
    }

    #[test]
    fn every_message_module_matches_its_group() {
        assert!(POWERTRAIN_MESSAGES.len() >= 12);
        for msg in POWERTRAIN_MESSAGES {
            let expected = match msg.group {
                PowertrainGroup::Engine => ENGINE,
                PowertrainGroup::Transmission => TRANSMISSION,
                PowertrainGroup::SpeedDistance => SPEED,
            };
            assert_eq!(msg.module, expected, "{} module mismatch", msg.name);
        }
    }

    #[test]
    fn groups_partition_the_inventory() {
        let total = messages_in(PowertrainGroup::Engine).len()
            + messages_in(PowertrainGroup::Transmission).len()
            + messages_in(PowertrainGroup::SpeedDistance).len();
        assert_eq!(total, POWERTRAIN_MESSAGES.len());
        assert!(!messages_in(PowertrainGroup::Engine).is_empty());
    }
}
