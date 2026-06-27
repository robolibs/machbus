//! Remote ECU we explicitly track via NAME filtering: [`PartnerCf`].
//!
//! Mirrors the C++ `machbus::net::PartnerCF`. Filters are AND-combined:
//! a remote NAME matches if **every** filter matches.

use alloc::vec::Vec;

use super::control_function::{CfState, CfType, ControlFunction};
use super::event::Event;
use super::name::Name;
use super::types::Address;

/// Time allowed for an online partner CF to answer a request-for-address-claim
/// before the observer treats it as offline.
pub const PARTNER_ADDRESS_CLAIM_RESPONSE_TIMEOUT_MS: u32 = 2_000;

/// Which NAME field a [`NameFilter`] inspects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameFilterField {
    IdentityNumber,
    ManufacturerCode,
    EcuInstance,
    FunctionInstance,
    FunctionCode,
    DeviceClass,
    DeviceClassInstance,
    IndustryGroup,
}

/// One AND term used to match remote NAMEs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameFilter {
    pub field: NameFilterField,
    pub value: u32,
}

impl NameFilter {
    #[must_use]
    pub const fn new(field: NameFilterField, value: u32) -> Self {
        Self { field, value }
    }

    /// `true` if `name`'s field equals [`Self::value`] (with the
    /// field's natural width).
    #[must_use]
    pub fn matches(&self, name: &Name) -> bool {
        match self.field {
            NameFilterField::IdentityNumber => name.identity_number() == self.value,
            NameFilterField::ManufacturerCode => name.manufacturer_code() as u32 == self.value,
            NameFilterField::EcuInstance => name.ecu_instance() as u32 == self.value,
            NameFilterField::FunctionInstance => name.function_instance() as u32 == self.value,
            NameFilterField::FunctionCode => name.function_code() as u32 == self.value,
            NameFilterField::DeviceClass => name.device_class() as u32 == self.value,
            NameFilterField::DeviceClassInstance => {
                name.device_class_instance() as u32 == self.value
            }
            NameFilterField::IndustryGroup => name.industry_group() as u32 == self.value,
        }
    }
}

/// Remote ECU tracked via NAME filtering.
pub struct PartnerCf {
    cf: ControlFunction,
    filters: Vec<NameFilter>,
    claim_validation_elapsed_ms: u32,
    claim_validation_pending: bool,

    /// Fires the address as soon as a matching NAME claims it.
    pub on_partner_found: Event<Address>,
    /// Fires when the partner goes Offline.
    pub on_partner_lost: Event<()>,
}

impl PartnerCf {
    #[must_use]
    pub fn new(port: u8, filters: Vec<NameFilter>) -> Self {
        let cf = ControlFunction {
            can_port: port,
            r#type: CfType::Partnered,
            ..Default::default()
        };
        Self {
            cf,
            filters,
            claim_validation_elapsed_ms: 0,
            claim_validation_pending: false,
            on_partner_found: Event::new(),
            on_partner_lost: Event::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn cf(&self) -> &ControlFunction {
        &self.cf
    }
    #[inline]
    pub fn cf_mut(&mut self) -> &mut ControlFunction {
        &mut self.cf
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> Name {
        self.cf.name
    }
    #[inline]
    #[must_use]
    pub fn address(&self) -> Address {
        self.cf.address
    }
    #[inline]
    #[must_use]
    pub fn port(&self) -> u8 {
        self.cf.can_port
    }
    #[inline]
    #[must_use]
    pub fn filters(&self) -> &[NameFilter] {
        &self.filters
    }

    #[inline]
    pub fn set_address(&mut self, addr: Address) {
        self.cf.address = addr;
    }
    #[inline]
    pub fn set_name(&mut self, name: Name) {
        self.cf.name = name;
    }
    #[inline]
    pub fn set_state(&mut self, state: CfState) {
        self.cf.state = state;
    }

    pub fn note_address_claim_seen(&mut self, name: Name, address: Address) {
        self.set_name(name);
        self.set_address(address);
        self.set_state(CfState::Online);
        self.claim_validation_pending = false;
        self.claim_validation_elapsed_ms = 0;
        self.on_partner_found.emit(&address);
    }

    pub fn note_cannot_claim_seen(&mut self, name: Name) {
        let was_online = self.cf.is_online() || self.cf.address_valid();
        self.set_name(name);
        self.set_address(super::constants::NULL_ADDRESS);
        self.set_state(CfState::Offline);
        self.claim_validation_pending = false;
        self.claim_validation_elapsed_ms = 0;
        if was_online {
            self.on_partner_lost.emit(&());
        }
    }

    pub fn begin_claim_validation(&mut self) {
        if self.cf.is_online() && self.cf.address_valid() {
            self.claim_validation_pending = true;
            self.claim_validation_elapsed_ms = 0;
        }
    }

    pub fn update_claim_validation(&mut self, elapsed_ms: u32) {
        if !self.claim_validation_pending {
            return;
        }
        self.claim_validation_elapsed_ms =
            self.claim_validation_elapsed_ms.saturating_add(elapsed_ms);
        if self.claim_validation_elapsed_ms < PARTNER_ADDRESS_CLAIM_RESPONSE_TIMEOUT_MS {
            return;
        }

        self.claim_validation_pending = false;
        self.claim_validation_elapsed_ms = 0;
        self.cf.address = super::constants::NULL_ADDRESS;
        self.cf.state = CfState::Offline;
        self.on_partner_lost.emit(&());
    }

    /// `true` if `name` satisfies every filter (empty filter set
    /// matches every name).
    #[must_use]
    pub fn matches_name(&self, name: &Name) -> bool {
        self.filters.iter().all(|f| f.matches(name))
    }
}

impl core::fmt::Debug for PartnerCf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PartnerCf")
            .field("address", &format_args!("0x{:02X}", self.cf.address))
            .field("port", &self.cf.can_port)
            .field("filters", &self.filters)
            .field("state", &self.cf.state)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_with_function(fc: u8) -> Name {
        Name::default()
            .with_function_code(fc)
            .with_manufacturer_code(0x100)
    }

    #[test]
    fn empty_filter_set_matches_anything() {
        let p = PartnerCf::new(0, vec![]);
        assert!(p.matches_name(&Name::default()));
        assert!(p.matches_name(&name_with_function(0x42)));
    }

    #[test]
    fn single_filter_matches_only_when_field_equals() {
        let p = PartnerCf::new(
            0,
            vec![NameFilter::new(NameFilterField::FunctionCode, 0x42)],
        );
        assert!(p.matches_name(&name_with_function(0x42)));
        assert!(!p.matches_name(&name_with_function(0x43)));
    }

    #[test]
    fn multiple_filters_are_anded() {
        let p = PartnerCf::new(
            0,
            vec![
                NameFilter::new(NameFilterField::FunctionCode, 0x42),
                NameFilter::new(NameFilterField::ManufacturerCode, 0x100),
            ],
        );
        let matching = name_with_function(0x42);
        assert!(p.matches_name(&matching));
        let wrong_mfg = matching.with_manufacturer_code(0x200);
        assert!(!p.matches_name(&wrong_mfg));
    }

    #[test]
    fn each_field_is_addressable() {
        for &field in &[
            NameFilterField::IdentityNumber,
            NameFilterField::ManufacturerCode,
            NameFilterField::EcuInstance,
            NameFilterField::FunctionInstance,
            NameFilterField::FunctionCode,
            NameFilterField::DeviceClass,
            NameFilterField::DeviceClassInstance,
            NameFilterField::IndustryGroup,
        ] {
            let f = NameFilter::new(field, 0);
            // Default Name has all-zero fields, so every filter against 0 matches.
            assert!(f.matches(&Name::default()));
        }
    }

    #[test]
    fn claim_validation_times_out_or_refreshes() {
        let name = name_with_function(0x42);
        let mut p = PartnerCf::new(
            0,
            vec![NameFilter::new(NameFilterField::FunctionCode, 0x42)],
        );

        p.note_address_claim_seen(name, 0x79);
        p.begin_claim_validation();
        p.update_claim_validation(PARTNER_ADDRESS_CLAIM_RESPONSE_TIMEOUT_MS - 1);
        assert!(p.cf().is_online());
        assert_eq!(p.address(), 0x79);

        p.note_address_claim_seen(name, 0x79);
        p.update_claim_validation(PARTNER_ADDRESS_CLAIM_RESPONSE_TIMEOUT_MS);
        assert!(
            p.cf().is_online(),
            "matching claim refresh cancels validation"
        );
        assert_eq!(p.address(), 0x79);

        p.begin_claim_validation();
        p.update_claim_validation(PARTNER_ADDRESS_CLAIM_RESPONSE_TIMEOUT_MS);
        assert!(!p.cf().is_online());
        assert_eq!(p.address(), crate::net::constants::NULL_ADDRESS);
    }
}
