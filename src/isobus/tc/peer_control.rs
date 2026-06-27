//! ISO 11783-10 Peer Control Assignments.
//!
//! Mirrors the C++ `machbus::isobus::tc::PeerControlInterface`.
//! Pump-style port: management methods + an `encode_assignment`
//! payload builder for the caller to ship via `IsoNet::send`.

use alloc::vec::Vec;

use super::objects::{DDI, ElementNumber};
use super::server_options::ProcessDataCommands;
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::types::Address;

/// Source element numbers are packed into the high nibble of byte 0 plus byte
/// 1 of the peer-control assignment payload. Values above 12 bits cannot be
/// represented without colliding with the process-data command nibble.
pub const MAX_PEER_CONTROL_SOURCE_ELEMENT: u16 = 0x0FFF;

/// One peer-control routing record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerControlAssignment {
    pub source_element: ElementNumber,
    pub source_ddi: DDI,
    pub destination_element: ElementNumber,
    pub destination_ddi: DDI,
    pub source_address: Address,
    pub destination_address: Address,
    pub active: bool,
}

impl Default for PeerControlAssignment {
    fn default() -> Self {
        Self {
            source_element: ElementNumber(0),
            source_ddi: DDI(0),
            destination_element: ElementNumber(0),
            destination_ddi: DDI(0),
            source_address: NULL_ADDRESS,
            destination_address: NULL_ADDRESS,
            active: false,
        }
    }
}

impl PeerControlAssignment {
    #[must_use]
    pub fn from(mut self, elem: impl Into<ElementNumber>, ddi: impl Into<DDI>) -> Self {
        self.source_element = elem.into();
        self.source_ddi = ddi.into();
        self
    }

    #[must_use]
    pub fn to(mut self, elem: impl Into<ElementNumber>, ddi: impl Into<DDI>) -> Self {
        self.destination_element = elem.into();
        self.destination_ddi = ddi.into();
        self
    }

    #[must_use]
    pub const fn with_source(mut self, addr: Address) -> Self {
        self.source_address = addr;
        self
    }

    #[must_use]
    pub const fn with_destination(mut self, addr: Address) -> Self {
        self.destination_address = addr;
        self
    }

    #[must_use]
    pub const fn source_matches(
        &self,
        source_address: Address,
        source_element: ElementNumber,
        source_ddi: DDI,
    ) -> bool {
        self.source_address == source_address
            && self.source_element.0 == source_element.0
            && self.source_ddi.0 == source_ddi.0
    }

    /// Encode the 8-byte process-data payload for
    /// `Peer Control Assignment` (low nibble of byte 0 = 0x09).
    pub fn try_encode(&self) -> Result<[u8; 8]> {
        validate_assignment(*self)?;
        Ok(self.encode_unchecked())
    }

    /// Encode the 8-byte process-data payload for
    /// `Peer Control Assignment` (low nibble of byte 0 = 0x09).
    ///
    /// This legacy infallible wrapper preserves the translated C++ surface.
    /// New code should prefer [`Self::try_encode`] so unencodable source
    /// element numbers cannot be silently truncated to the 12-bit wire field.
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        self.encode_unchecked()
    }

    /// Decode a peer-control assignment process-data payload and attach the
    /// message-level source/destination addresses to the registry record.
    pub fn decode(
        data: &[u8],
        source_address: Address,
        destination_address: Address,
    ) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "peer-control assignment payload must be exactly 8 bytes",
            ));
        }
        if (data[0] & 0x0F) != ProcessDataCommands::PeerControlAssignment.as_u8() {
            return Err(Error::invalid_data(
                "peer-control assignment command nibble is invalid",
            ));
        }
        let assignment = Self {
            source_element: ElementNumber(((data[0] >> 4) as u16) | ((data[1] as u16) << 4)),
            source_ddi: DDI((data[2] as u16) | ((data[3] as u16) << 8)),
            destination_element: ElementNumber((data[4] as u16) | ((data[5] as u16) << 8)),
            destination_ddi: DDI((data[6] as u16) | ((data[7] as u16) << 8)),
            source_address,
            destination_address,
            active: false,
        };
        validate_assignment(assignment)?;
        Ok(assignment)
    }

    fn encode_unchecked(&self) -> [u8; 8] {
        let src_elem: u16 = self.source_element.into();
        let src_ddi: u16 = self.source_ddi.into();
        let dst_elem: u16 = self.destination_element.into();
        let dst_ddi: u16 = self.destination_ddi.into();
        let mut data = [0xFFu8; 8];
        data[0] = (ProcessDataCommands::PeerControlAssignment.as_u8() & 0x0F)
            | ((src_elem as u8 & 0x0F) << 4);
        data[1] = ((src_elem >> 4) & 0xFF) as u8;
        data[2] = (src_ddi & 0xFF) as u8;
        data[3] = ((src_ddi >> 8) & 0xFF) as u8;
        data[4] = (dst_elem & 0xFF) as u8;
        data[5] = ((dst_elem >> 8) & 0xFF) as u8;
        data[6] = (dst_ddi & 0xFF) as u8;
        data[7] = ((dst_ddi >> 8) & 0xFF) as u8;
        data
    }
}

/// Pump-style peer-control registry.
pub struct PeerControlInterface {
    assignments: Vec<PeerControlAssignment>,
    pub on_assignment_added: Event<PeerControlAssignment>,
    pub on_assignment_removed: Event<PeerControlAssignment>,
    pub on_assignment_state_changed: Event<PeerControlAssignment>,
}

impl Default for PeerControlInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerControlInterface {
    #[must_use]
    pub fn new() -> Self {
        Self {
            assignments: Vec::new(),
            on_assignment_added: Event::new(),
            on_assignment_removed: Event::new(),
            on_assignment_state_changed: Event::new(),
        }
    }

    pub fn add_assignment(&mut self, assignment: PeerControlAssignment) -> Result<()> {
        validate_assignment(assignment)?;
        if self.assignments.iter().any(|a| {
            a.source_matches(
                assignment.source_address,
                assignment.source_element,
                assignment.source_ddi,
            )
        }) {
            return Err(Error::invalid_state("duplicate peer control assignment"));
        }
        self.on_assignment_added.emit(&assignment);
        self.assignments.push(assignment);
        Ok(())
    }

    pub fn remove_assignment(
        &mut self,
        source_element: impl Into<ElementNumber>,
        source_ddi: impl Into<DDI>,
    ) -> Result<()> {
        let source_element = source_element.into();
        let source_ddi = source_ddi.into();
        let pos = self.find_unique_source_position(source_element, source_ddi)?;
        self.remove_assignment_at(pos);
        Ok(())
    }

    pub fn remove_assignment_from(
        &mut self,
        source_address: Address,
        source_element: impl Into<ElementNumber>,
        source_ddi: impl Into<DDI>,
    ) -> Result<()> {
        let pos =
            self.find_source_position(source_address, source_element.into(), source_ddi.into())?;
        self.remove_assignment_at(pos);
        Ok(())
    }

    pub fn activate_assignment(
        &mut self,
        source_element: impl Into<ElementNumber>,
        source_ddi: impl Into<DDI>,
        active: bool,
    ) -> Result<()> {
        let source_element = source_element.into();
        let source_ddi = source_ddi.into();
        let pos = self.find_unique_source_position(source_element, source_ddi)?;
        self.set_assignment_active(pos, active);
        Ok(())
    }

    pub fn activate_assignment_from(
        &mut self,
        source_address: Address,
        source_element: impl Into<ElementNumber>,
        source_ddi: impl Into<DDI>,
        active: bool,
    ) -> Result<()> {
        let pos =
            self.find_source_position(source_address, source_element.into(), source_ddi.into())?;
        self.set_assignment_active(pos, active);
        Ok(())
    }

    pub fn clear_assignments(&mut self) {
        for removed in self.assignments.drain(..) {
            self.on_assignment_removed.emit(&removed);
        }
    }

    #[must_use]
    pub fn assignments(&self) -> &[PeerControlAssignment] {
        &self.assignments
    }

    pub fn update(&mut self, _elapsed_ms: u32) {}

    fn find_unique_source_position(
        &self,
        source_element: ElementNumber,
        source_ddi: DDI,
    ) -> Result<usize> {
        let mut matches = self
            .assignments
            .iter()
            .enumerate()
            .filter(|(_, a)| a.source_element == source_element && a.source_ddi == source_ddi)
            .map(|(pos, _)| pos);
        let Some(pos) = matches.next() else {
            return Err(Error::invalid_state("assignment not found"));
        };
        if matches.next().is_some() {
            return Err(Error::invalid_state(
                "assignment source is ambiguous; include source address",
            ));
        }
        Ok(pos)
    }

    fn find_source_position(
        &self,
        source_address: Address,
        source_element: ElementNumber,
        source_ddi: DDI,
    ) -> Result<usize> {
        self.assignments
            .iter()
            .position(|a| a.source_matches(source_address, source_element, source_ddi))
            .ok_or_else(|| Error::invalid_state("assignment not found"))
    }

    fn remove_assignment_at(&mut self, pos: usize) {
        let removed = self.assignments.remove(pos);
        self.on_assignment_removed.emit(&removed);
    }

    fn set_assignment_active(&mut self, pos: usize, active: bool) {
        let assignment = &mut self.assignments[pos];
        if assignment.active == active {
            return;
        }
        assignment.active = active;
        let snapshot = *assignment;
        self.on_assignment_state_changed.emit(&snapshot);
    }
}

fn validate_assignment(assignment: PeerControlAssignment) -> Result<()> {
    if assignment.source_element.0 > MAX_PEER_CONTROL_SOURCE_ELEMENT {
        return Err(Error::invalid_data(
            "peer-control source element exceeds 12-bit wire field",
        ));
    }
    if assignment.source_address == NULL_ADDRESS {
        return Err(Error::invalid_address(assignment.source_address));
    }
    if assignment.source_address == BROADCAST_ADDRESS {
        return Err(Error::invalid_address(assignment.source_address));
    }
    if assignment.destination_address == NULL_ADDRESS {
        return Err(Error::invalid_address(assignment.destination_address));
    }
    if assignment.destination_address == BROADCAST_ADDRESS {
        return Err(Error::invalid_address(assignment.destination_address));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assignment() -> PeerControlAssignment {
        PeerControlAssignment::default()
            .from(1, 0x1234)
            .to(2, 0xABCD)
            .with_source(0x10)
            .with_destination(0x20)
    }

    #[test]
    fn encode_layout() {
        let bytes = assignment().encode();
        assert_eq!(
            bytes[0] & 0x0F,
            ProcessDataCommands::PeerControlAssignment.as_u8()
        );
        assert_eq!(bytes[2..4], [0x34, 0x12]);
        assert_eq!(bytes[4..6], [2, 0]);
        assert_eq!(bytes[6..8], [0xCD, 0xAB]);
    }

    #[test]
    fn try_encode_and_decode_round_trip_assignment_payload() {
        let original = assignment();
        let bytes = original.try_encode().unwrap();
        let decoded = PeerControlAssignment::decode(&bytes, 0x10, 0x20).unwrap();
        assert_eq!(decoded, original);
        assert_eq!(decoded.try_encode().unwrap(), bytes);
    }

    #[test]
    fn decode_rejects_malformed_payloads_and_invalid_sources() {
        let bytes = assignment().try_encode().unwrap();
        assert!(PeerControlAssignment::decode(&bytes[..7], 0x10, 0x20).is_err());
        let mut overlong = bytes.to_vec();
        overlong.push(0xFF);
        assert!(PeerControlAssignment::decode(&overlong, 0x10, 0x20).is_err());
        let mut wrong_command = bytes;
        wrong_command[0] = (wrong_command[0] & 0xF0) | ProcessDataCommands::Value.as_u8();
        assert!(PeerControlAssignment::decode(&wrong_command, 0x10, 0x20).is_err());
        assert!(PeerControlAssignment::decode(&bytes, NULL_ADDRESS, 0x20).is_err());
        assert!(PeerControlAssignment::decode(&bytes, BROADCAST_ADDRESS, 0x20).is_err());
        assert!(PeerControlAssignment::decode(&bytes, 0x10, NULL_ADDRESS).is_err());
    }

    #[test]
    fn try_encode_rejects_unencodable_source_element() {
        let bad = assignment().from(MAX_PEER_CONTROL_SOURCE_ELEMENT + 1, 0x1234);
        assert!(bad.try_encode().is_err());

        let mut p = PeerControlInterface::new();
        assert!(p.add_assignment(bad).is_err());
        assert!(p.assignments().is_empty());
    }

    #[test]
    fn add_dedups_by_source() {
        let mut p = PeerControlInterface::new();
        p.add_assignment(assignment()).unwrap();
        assert!(p.add_assignment(assignment()).is_err());
        // Different source DDI should be allowed.
        let other = assignment().from(1, 0x5678);
        p.add_assignment(other).unwrap();
        // Same source element/DDI from a different source CF is also distinct.
        p.add_assignment(assignment().with_source(0x11)).unwrap();
        assert_eq!(p.assignments().len(), 3);
        assert!(p.add_assignment(PeerControlAssignment::default()).is_err());
    }

    #[test]
    fn remove_and_activate() {
        let mut p = PeerControlInterface::new();
        p.add_assignment(assignment()).unwrap();
        p.activate_assignment_from(0x10, 1, 0x1234, true).unwrap();
        assert!(p.assignments()[0].active);
        p.remove_assignment_from(0x10, 1, 0x1234).unwrap();
        assert!(p.assignments().is_empty());
        assert!(p.remove_assignment(1, 0x1234).is_err());
    }

    #[test]
    fn legacy_source_only_lookup_rejects_ambiguous_matches() {
        let mut p = PeerControlInterface::new();
        p.add_assignment(assignment()).unwrap();
        p.add_assignment(assignment().with_source(0x11)).unwrap();
        assert!(p.activate_assignment(1, 0x1234, true).is_err());
        assert!(p.remove_assignment(1, 0x1234).is_err());
        p.activate_assignment_from(0x11, 1, 0x1234, true).unwrap();
        assert!(p.assignments()[1].active);
    }

    #[test]
    fn clear_drops_all() {
        let mut p = PeerControlInterface::new();
        p.add_assignment(assignment()).unwrap();
        p.add_assignment(assignment().from(2, 0x9999)).unwrap();
        p.clear_assignments();
        assert!(p.assignments().is_empty());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_peer_control_decode_accepts_only_canonical_payloads(
            data in proptest::collection::vec(any::<u8>(), 0..=16),
            source in any::<u8>(),
            destination in any::<u8>(),
        ) {
            match PeerControlAssignment::decode(&data, source, destination) {
                Ok(decoded) => {
                    prop_assert_eq!(data.len(), 8);
                    prop_assert_eq!(data[0] & 0x0F, ProcessDataCommands::PeerControlAssignment.as_u8());
                    prop_assert_ne!(source, NULL_ADDRESS);
                    prop_assert_ne!(source, BROADCAST_ADDRESS);
                    prop_assert_ne!(destination, NULL_ADDRESS);
                    prop_assert_ne!(destination, BROADCAST_ADDRESS);
                    prop_assert!(decoded.source_element.0 <= MAX_PEER_CONTROL_SOURCE_ELEMENT);
                    let encoded = decoded.try_encode().unwrap();
                    prop_assert_eq!(encoded.as_slice(), data.as_slice());
                    prop_assert!(!decoded.active);
                }
                Err(_) => {
                    prop_assert!(
                        data.len() != 8
                            || (data[0] & 0x0F) != ProcessDataCommands::PeerControlAssignment.as_u8()
                            || source == NULL_ADDRESS
                            || source == BROADCAST_ADDRESS
                            || destination == NULL_ADDRESS
                            || destination == BROADCAST_ADDRESS
                    );
                }
            }
        }
    }
}
