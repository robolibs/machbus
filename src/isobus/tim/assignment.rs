//! AEF 023 TIM function assignment (§5.5, Annexes B.5.1 / B.5.2).
//!
//! Assignment is **per TIM function**, exclusively, to one client. The crate's
//! `TimAuthorityArbiter` held a single `Option<Address>`: one client owned
//! everything or nothing, so one client holding the rear hitch while another
//! held a valve was unrepresentable. It was also never wired into the plugin.
//!
//! This module adds the request/response pair and a per-function assignment
//! table with the 1500 ms response timeout the spec requires.

use alloc::vec::Vec;

use super::functions::TimFunctionId;

/// Message code shared by request and response (A.2.3).
pub const MSG_CODE_ASSIGNMENT: u8 = 0xF5;

/// A response must arrive within this window (B.5.2).
pub const ASSIGNMENT_TIMEOUT_MS: u32 = 1500;

/// A request may not be repeated more often than this (B.5.1).
pub const ASSIGNMENT_MIN_INTERVAL_MS: u32 = 100;

/// What a client wants done with a function, byte `2i+2` bits 8-6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum RequestType {
    /// Release this client's assignment.
    Release = 0x0,
    /// Take exclusive assignment.
    Assign = 0x1,
    Error = 0x6,
    /// Query the current assignment without changing it.
    #[default]
    DontCare = 0x7,
}

impl RequestType {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x07 {
            0x0 => Some(Self::Release),
            0x1 => Some(Self::Assign),
            0x6 => Some(Self::Error),
            0x7 => Some(Self::DontCare),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Outcome for one function, byte `2i+2` bits 8-6 of the response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AssignmentStatus {
    NotAssignedToRequester = 0x0,
    AssignedToRequester = 0x1,
    NotSuccessful = 0x5,
    Error = 0x6,
    #[default]
    NotAvailable = 0x7,
}

impl AssignmentStatus {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x07 {
            0x0 => Some(Self::NotAssignedToRequester),
            0x1 => Some(Self::AssignedToRequester),
            0x5 => Some(Self::NotSuccessful),
            0x6 => Some(Self::Error),
            0x7 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Why an assignment did not succeed, byte `2i+2` bits 5-1 of the response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AssignmentReason {
    #[default]
    AllClear = 0x00,
    /// The server does not implement this function.
    FunctionNotSupported = 0x01,
    /// Implemented, but not available right now.
    FunctionNotAvailable = 0x02,
    UnknownRequestType = 0x03,
    /// e.g. the function count does not match the patterns that follow.
    ErrorInRequestMessage = 0x04,
    /// The client's certificate does not cover this function.
    ClientNotCertified = 0x05,
    /// The server is already handling another assignment request.
    ServerBusy = 0x06,
    /// No facility on either side matches the requested function.
    NoMatchingFacility = 0x07,
    AnyOtherError = 0x1E,
    NotAvailable = 0x1F,
}

impl AssignmentReason {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x1F {
            0x00 => Some(Self::AllClear),
            0x01 => Some(Self::FunctionNotSupported),
            0x02 => Some(Self::FunctionNotAvailable),
            0x03 => Some(Self::UnknownRequestType),
            0x04 => Some(Self::ErrorInRequestMessage),
            0x05 => Some(Self::ClientNotCertified),
            0x06 => Some(Self::ServerBusy),
            0x07 => Some(Self::NoMatchingFacility),
            0x1E => Some(Self::AnyOtherError),
            0x1F => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// `TIM_FunctionsAssignmentRequest` (B.5.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssignmentRequest {
    pub entries: Vec<(TimFunctionId, RequestType)>,
}

impl AssignmentRequest {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(2 + self.entries.len() * 2);
        data.push(MSG_CODE_ASSIGNMENT);
        data.push(self.entries.len() as u8);
        for (id, request) in &self.entries {
            data.push(id.as_u8());
            // Bits 5-1 are reserved and travel as ones.
            data.push((request.as_u8() << 5) | 0x1F);
        }
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 || data[0] != MSG_CODE_ASSIGNMENT {
            return None;
        }
        let count = data[1] as usize;
        // "Error in request message (e.g. number of functions does not match
        // number of patterns that follow)" — B.5.2 reason 0x04.
        if data.len() < 2 + count * 2 {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let id = TimFunctionId::from_u8(data[2 + i * 2])?;
            let request = RequestType::from_u8(data[3 + i * 2] >> 5)?;
            entries.push((id, request));
        }
        Some(Self { entries })
    }
}

/// `TIM_FunctionsAssignmentResponse` (B.5.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssignmentResponse {
    pub entries: Vec<(TimFunctionId, AssignmentStatus, AssignmentReason)>,
}

impl AssignmentResponse {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(2 + self.entries.len() * 2);
        data.push(MSG_CODE_ASSIGNMENT);
        data.push(self.entries.len() as u8);
        for (id, status, reason) in &self.entries {
            data.push(id.as_u8());
            data.push((status.as_u8() << 5) | (reason.as_u8() & 0x1F));
        }
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 || data[0] != MSG_CODE_ASSIGNMENT {
            return None;
        }
        let count = data[1] as usize;
        if data.len() < 2 + count * 2 {
            return None;
        }
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let id = TimFunctionId::from_u8(data[2 + i * 2])?;
            let packed = data[3 + i * 2];
            entries.push((
                TimFunctionId::from_u8(id.as_u8())?,
                AssignmentStatus::from_u8(packed >> 5)?,
                AssignmentReason::from_u8(packed & 0x1F)?,
            ));
        }
        Some(Self { entries })
    }
}

/// Server-side per-function exclusive assignment table (§5.5.1).
#[derive(Debug, Default)]
pub struct AssignmentTable {
    /// One owner per function. A `BTreeMap` would do; functions are few enough
    /// that a sorted vector avoids the dependency and keeps `no_std` simple.
    owners: Vec<(TimFunctionId, u8)>,
    /// Functions this server implements at all.
    supported: Vec<TimFunctionId>,
    /// A request is being processed; further ones get `ServerBusy` (§5.5.3).
    busy: bool,
}

impl AssignmentTable {
    #[must_use]
    pub fn new(supported: &[TimFunctionId]) -> Self {
        Self {
            owners: Vec::new(),
            supported: supported.to_vec(),
            busy: false,
        }
    }

    /// Who owns `function`, if anyone.
    #[must_use]
    pub fn owner(&self, function: TimFunctionId) -> Option<u8> {
        self.owners
            .iter()
            .find(|(f, _)| *f == function)
            .map(|(_, a)| *a)
    }

    /// `true` when `client` may command `function`.
    #[must_use]
    pub fn is_assigned_to(&self, function: TimFunctionId, client: u8) -> bool {
        self.owner(function) == Some(client)
    }

    /// Mark the server as handling a request, so concurrent ones are refused.
    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    /// Release everything held by `client` — used on status timeout, heartbeat
    /// error, or a shutdown without an explicit release (§5.5.3).
    pub fn release_all(&mut self, client: u8) {
        self.owners.retain(|(_, a)| *a != client);
    }

    /// Apply a request from `client` and produce the response.
    ///
    /// One request is serialised at a time; a second concurrent one is answered
    /// `ServerBusy` rather than interleaved.
    #[must_use]
    pub fn apply(&mut self, request: &AssignmentRequest, client: u8) -> AssignmentResponse {
        let mut entries = Vec::with_capacity(request.entries.len());

        for (function, request_type) in &request.entries {
            let (status, reason) = if self.busy {
                (
                    AssignmentStatus::NotSuccessful,
                    AssignmentReason::ServerBusy,
                )
            } else if !self.supported.contains(function) {
                (
                    AssignmentStatus::NotSuccessful,
                    AssignmentReason::FunctionNotSupported,
                )
            } else {
                match request_type {
                    RequestType::Assign => match self.owner(*function) {
                        // Already ours: idempotent.
                        Some(owner) if owner == client => (
                            AssignmentStatus::AssignedToRequester,
                            AssignmentReason::AllClear,
                        ),
                        // Held by someone else: exclusivity means refusal.
                        Some(_) => (
                            AssignmentStatus::NotSuccessful,
                            AssignmentReason::FunctionNotAvailable,
                        ),
                        None => {
                            self.owners.push((*function, client));
                            (
                                AssignmentStatus::AssignedToRequester,
                                AssignmentReason::AllClear,
                            )
                        }
                    },
                    RequestType::Release => {
                        if self.owner(*function) == Some(client) {
                            self.owners.retain(|(f, _)| f != function);
                        }
                        (
                            AssignmentStatus::NotAssignedToRequester,
                            AssignmentReason::AllClear,
                        )
                    }
                    RequestType::DontCare => {
                        // A pure query never changes the assignment.
                        if self.is_assigned_to(*function, client) {
                            (
                                AssignmentStatus::AssignedToRequester,
                                AssignmentReason::AllClear,
                            )
                        } else {
                            (
                                AssignmentStatus::NotAssignedToRequester,
                                AssignmentReason::AllClear,
                            )
                        }
                    }
                    RequestType::Error => (
                        AssignmentStatus::NotSuccessful,
                        AssignmentReason::UnknownRequestType,
                    ),
                }
            };
            entries.push((*function, status, reason));
        }

        AssignmentResponse { entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT_A: u8 = 0x80;
    const CLIENT_B: u8 = 0x81;

    fn server() -> AssignmentTable {
        AssignmentTable::new(&[
            TimFunctionId::ExternalGuidance,
            TimFunctionId::VehicleSpeed,
            TimFunctionId::RearHitch,
            TimFunctionId::AuxValve(1),
        ])
    }

    #[test]
    fn assignment_is_per_function_not_per_client() {
        // The defect this replaces: a single Option<Address> meant one client
        // owned everything or nothing.
        let mut table = server();
        let hitch = AssignmentRequest {
            entries: vec![(TimFunctionId::RearHitch, RequestType::Assign)],
        };
        let valve = AssignmentRequest {
            entries: vec![(TimFunctionId::AuxValve(1), RequestType::Assign)],
        };

        let _ = table.apply(&hitch, CLIENT_A);
        let _ = table.apply(&valve, CLIENT_B);

        assert!(table.is_assigned_to(TimFunctionId::RearHitch, CLIENT_A));
        assert!(table.is_assigned_to(TimFunctionId::AuxValve(1), CLIENT_B));
        assert!(!table.is_assigned_to(TimFunctionId::RearHitch, CLIENT_B));
    }

    #[test]
    fn a_held_function_is_refused_to_a_second_client() {
        let mut table = server();
        let request = AssignmentRequest {
            entries: vec![(TimFunctionId::ExternalGuidance, RequestType::Assign)],
        };
        let _ = table.apply(&request, CLIENT_A);

        let response = table.apply(&request, CLIENT_B);
        assert_eq!(
            response.entries[0],
            (
                TimFunctionId::ExternalGuidance,
                AssignmentStatus::NotSuccessful,
                AssignmentReason::FunctionNotAvailable
            )
        );
        // The original owner keeps it.
        assert!(table.is_assigned_to(TimFunctionId::ExternalGuidance, CLIENT_A));
    }

    #[test]
    fn unsupported_functions_say_so_rather_than_failing_silently() {
        let mut table = server();
        let response = table.apply(
            &AssignmentRequest {
                entries: vec![(TimFunctionId::FrontPto, RequestType::Assign)],
            },
            CLIENT_A,
        );
        assert_eq!(response.entries[0].1, AssignmentStatus::NotSuccessful);
        assert_eq!(
            response.entries[0].2,
            AssignmentReason::FunctionNotSupported
        );
    }

    #[test]
    fn a_query_does_not_change_the_assignment() {
        let mut table = server();
        let _ = table.apply(
            &AssignmentRequest {
                entries: vec![(TimFunctionId::VehicleSpeed, RequestType::Assign)],
            },
            CLIENT_A,
        );

        let query = AssignmentRequest {
            entries: vec![(TimFunctionId::VehicleSpeed, RequestType::DontCare)],
        };
        let as_owner = table.apply(&query, CLIENT_A);
        assert_eq!(as_owner.entries[0].1, AssignmentStatus::AssignedToRequester);

        let as_other = table.apply(&query, CLIENT_B);
        assert_eq!(
            as_other.entries[0].1,
            AssignmentStatus::NotAssignedToRequester
        );
        assert!(table.is_assigned_to(TimFunctionId::VehicleSpeed, CLIENT_A));
    }

    #[test]
    fn losing_a_client_releases_everything_it_held() {
        // Section 5.5.3: status timeout, heartbeat error or a shutdown without
        // release must free the functions.
        let mut table = server();
        let _ = table.apply(
            &AssignmentRequest {
                entries: vec![
                    (TimFunctionId::ExternalGuidance, RequestType::Assign),
                    (TimFunctionId::VehicleSpeed, RequestType::Assign),
                ],
            },
            CLIENT_A,
        );
        assert!(table.is_assigned_to(TimFunctionId::ExternalGuidance, CLIENT_A));

        table.release_all(CLIENT_A);
        assert_eq!(table.owner(TimFunctionId::ExternalGuidance), None);
        assert_eq!(table.owner(TimFunctionId::VehicleSpeed), None);

        // And the functions are then available to someone else.
        let response = table.apply(
            &AssignmentRequest {
                entries: vec![(TimFunctionId::ExternalGuidance, RequestType::Assign)],
            },
            CLIENT_B,
        );
        assert_eq!(response.entries[0].1, AssignmentStatus::AssignedToRequester);
    }

    #[test]
    fn concurrent_requests_are_serialised_with_server_busy() {
        let mut table = server();
        table.set_busy(true);
        let response = table.apply(
            &AssignmentRequest {
                entries: vec![(TimFunctionId::RearHitch, RequestType::Assign)],
            },
            CLIENT_A,
        );
        assert_eq!(response.entries[0].2, AssignmentReason::ServerBusy);
        assert_eq!(table.owner(TimFunctionId::RearHitch), None);
    }

    #[test]
    fn request_and_response_round_trip_the_annex_b5_layout() {
        let request = AssignmentRequest {
            entries: vec![
                (TimFunctionId::ExternalGuidance, RequestType::Assign),
                (TimFunctionId::VehicleSpeed, RequestType::Release),
            ],
        };
        let bytes = request.encode();
        assert_eq!(bytes[0], MSG_CODE_ASSIGNMENT);
        assert_eq!(bytes[1], 2, "byte 2 is the function count");
        assert_eq!(bytes[2], 0x46, "external guidance is function 0x46");
        assert_eq!(AssignmentRequest::decode(&bytes), Some(request));

        let response = AssignmentResponse {
            entries: vec![(
                TimFunctionId::ExternalGuidance,
                AssignmentStatus::NotSuccessful,
                AssignmentReason::NoMatchingFacility,
            )],
        };
        let bytes = response.encode();
        assert_eq!(bytes[3] >> 5, 0x5, "status in bits 8-6");
        assert_eq!(bytes[3] & 0x1F, 0x7, "reason in bits 5-1");
        assert_eq!(AssignmentResponse::decode(&bytes), Some(response));
    }

    #[test]
    fn a_count_that_does_not_match_the_payload_is_rejected() {
        // B.5.2 reason 0x04 exists precisely for this.
        let truncated = [MSG_CODE_ASSIGNMENT, 3, 0x46, 0x3F];
        assert_eq!(AssignmentRequest::decode(&truncated), None);
        assert_eq!(AssignmentResponse::decode(&truncated), None);
    }
}
