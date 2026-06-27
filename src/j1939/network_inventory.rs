//! Diagnostic network inventory (ISO 11783-12).
//!
//! A diagnostic user interface needs a per-control-function view of the bus:
//! the claimed source address, the NAME observed from its address claim, and
//! any ECU / software / product identification it has reported. This module is
//! that queryable aggregate. It is pure state: the caller feeds it decoded
//! observations (from address claims and DM/identification responses) and reads
//! back the assembled per-CF records.

use alloc::collections::BTreeMap;

use crate::net::Name;
use crate::net::types::Address;

use super::{EcuIdentification, ProductIdentification, SoftwareIdentification};

/// What is known about one control function on the bus.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CfRecord {
    /// Claimed source address.
    pub address: Address,
    /// NAME from the control function's address claim, once observed.
    pub name: Option<Name>,
    /// ECU identification (PGN 64965), once reported.
    pub ecu_id: Option<EcuIdentification>,
    /// Software identification (PGN 65242), once reported.
    pub software_id: Option<SoftwareIdentification>,
    /// Product identification, once reported.
    pub product_id: Option<ProductIdentification>,
    /// Inventory clock value at the most recent observation for this CF.
    pub last_seen_ms: u32,
}

/// Per-control-function diagnostic inventory, keyed by source address.
#[derive(Debug, Clone, Default)]
pub struct NetworkInventory {
    by_address: BTreeMap<Address, CfRecord>,
    now_ms: u32,
}

impl NetworkInventory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn entry(&mut self, address: Address) -> &mut CfRecord {
        let now = self.now_ms;
        let rec = self.by_address.entry(address).or_insert_with(|| CfRecord {
            address,
            ..CfRecord::default()
        });
        // Every observation refreshes the freshness stamp.
        rec.last_seen_ms = now;
        rec
    }

    /// Record the NAME a control function claimed at `address`.
    pub fn observe_address_claim(&mut self, address: Address, name: Name) {
        self.entry(address).name = Some(name);
    }

    /// Record an ECU-identification response from `address`.
    pub fn observe_ecu_id(&mut self, address: Address, ecu_id: EcuIdentification) {
        self.entry(address).ecu_id = Some(ecu_id);
    }

    /// Record a software-identification response from `address`.
    pub fn observe_software_id(&mut self, address: Address, software_id: SoftwareIdentification) {
        self.entry(address).software_id = Some(software_id);
    }

    /// Record a product-identification response from `address`.
    pub fn observe_product_id(&mut self, address: Address, product_id: ProductIdentification) {
        self.entry(address).product_id = Some(product_id);
    }

    /// The record for a control function, if any has been observed.
    #[must_use]
    pub fn get(&self, address: Address) -> Option<&CfRecord> {
        self.by_address.get(&address)
    }

    /// Drop a control function (e.g. after it loses its address). Returns the
    /// removed record, if present.
    pub fn forget(&mut self, address: Address) -> Option<CfRecord> {
        self.by_address.remove(&address)
    }

    /// Advance the inventory clock (used for staleness tracking).
    pub fn tick(&mut self, elapsed_ms: u32) {
        self.now_ms = self.now_ms.saturating_add(elapsed_ms);
    }

    /// The inventory-clock value when `address` was last observed, if known.
    #[must_use]
    pub fn last_seen_ms(&self, address: Address) -> Option<u32> {
        self.by_address.get(&address).map(|r| r.last_seen_ms)
    }

    /// Remove control functions not observed within `timeout_ms` of the current
    /// clock. Returns the number pruned.
    pub fn prune_stale(&mut self, timeout_ms: u32) -> usize {
        let now = self.now_ms;
        let before = self.by_address.len();
        self.by_address
            .retain(|_, r| now.saturating_sub(r.last_seen_ms) <= timeout_ms);
        before - self.by_address.len()
    }

    /// All known control functions, ordered by source address.
    pub fn control_functions(&self) -> impl Iterator<Item = &CfRecord> {
        self.by_address.values()
    }

    /// The control function whose observed NAME equals `name`, if any. NAME is
    /// stable across address changes, so this is the address-independent lookup
    /// a diagnostic UI uses to track a CF.
    #[must_use]
    pub fn find_by_name(&self, name: Name) -> Option<&CfRecord> {
        self.by_address.values().find(|r| r.name == Some(name))
    }

    /// Every control function whose observed NAME carries `function_code`,
    /// ordered by source address (a function code may appear on several CFs).
    pub fn find_by_function_code(&self, function_code: u8) -> impl Iterator<Item = &CfRecord> {
        self.by_address
            .values()
            .filter(move |r| r.name.is_some_and(|n| n.function_code() == function_code))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_address.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_address.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_aggregates_per_cf_observations() {
        let mut inv = NetworkInventory::new();
        let name = Name::default().with_identity_number(0x123);

        inv.observe_address_claim(0x80, name);
        let ecu = EcuIdentification {
            ecu_part_number: "PN-1".to_string(),
            ..EcuIdentification::default()
        };
        inv.observe_ecu_id(0x80, ecu.clone());
        inv.observe_address_claim(0x81, Name::default().with_identity_number(0x456));

        assert_eq!(inv.len(), 2);
        let rec = inv.get(0x80).unwrap();
        assert_eq!(rec.address, 0x80);
        assert_eq!(rec.name, Some(name));
        assert_eq!(rec.ecu_id.as_ref().unwrap().ecu_part_number, "PN-1");
        assert!(rec.software_id.is_none());

        // Records are ordered by address and droppable on address loss.
        let addrs: Vec<_> = inv.control_functions().map(|r| r.address).collect();
        assert_eq!(addrs, vec![0x80, 0x81]);
        assert!(inv.forget(0x80).is_some());
        assert!(inv.get(0x80).is_none());
        assert_eq!(inv.len(), 1);
    }

    #[test]
    fn cfs_are_findable_by_stable_name_and_function_code() {
        let mut inv = NetworkInventory::new();
        let tecu = Name::default()
            .with_function_code(0)
            .with_identity_number(0x111);
        let vt = Name::default()
            .with_function_code(29)
            .with_identity_number(0x222);
        let vt2 = Name::default()
            .with_function_code(29)
            .with_identity_number(0x333);
        inv.observe_address_claim(0x26, tecu);
        inv.observe_address_claim(0x80, vt);
        inv.observe_address_claim(0x81, vt2);

        // Exact NAME lookup is address-independent.
        assert_eq!(inv.find_by_name(vt).unwrap().address, 0x80);
        assert!(
            inv.find_by_name(Name::default().with_identity_number(0x999))
                .is_none()
        );

        // Function-code lookup returns every matching CF, ordered by address.
        let vts: Vec<_> = inv.find_by_function_code(29).map(|r| r.address).collect();
        assert_eq!(vts, vec![0x80, 0x81]);
        assert_eq!(inv.find_by_function_code(0).count(), 1);
    }

    #[test]
    fn stale_control_functions_are_pruned_after_timeout() {
        let mut inv = NetworkInventory::new();
        inv.observe_address_claim(0x80, Name::default());
        inv.tick(1_000);
        // 0x81 seen later than 0x80.
        inv.observe_address_claim(0x81, Name::default());
        assert_eq!(inv.last_seen_ms(0x80), Some(0));
        assert_eq!(inv.last_seen_ms(0x81), Some(1_000));

        // Advance so 0x80 is stale (age 1500) but 0x81 is fresh (age 500).
        inv.tick(500);
        assert_eq!(inv.prune_stale(1_000), 1);
        assert!(inv.get(0x80).is_none());
        assert!(inv.get(0x81).is_some());

        // A fresh observation refreshes the stamp and prevents pruning.
        inv.observe_software_id(0x81, SoftwareIdentification::default());
        assert_eq!(inv.last_seen_ms(0x81), Some(1_500));
        assert_eq!(inv.prune_stale(0), 0);
    }
}
