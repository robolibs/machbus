//! DM14/DM15/DM16 + ECU/Software/Product Identification as a [`Plugin`].
//! Responds to identification requests, routes DM14/15/16, and caches the last
//! seen values. Wraps the pure `crate::j1939::dm_memory` codecs.

use crate::j1939::{
    Dm14Request, Dm15Response, Dm16Transfer, EcuIdentification, ProductIdentification,
    SoftwareIdentification, encode_request, requested_pgn,
};
use crate::net::pgn_defs::{
    PGN_DM14, PGN_DM15, PGN_DM16, PGN_ECU_IDENTIFICATION, PGN_PRODUCT_IDENTIFICATION, PGN_REQUEST,
    PGN_SOFTWARE_ID,
};
use crate::net::{Address, BROADCAST_ADDRESS, Message, NULL_ADDRESS, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{DmMemoryEvent, Event};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[
    PGN_REQUEST,
    PGN_DM14,
    PGN_DM15,
    PGN_DM16,
    PGN_ECU_IDENTIFICATION,
    PGN_SOFTWARE_ID,
    PGN_PRODUCT_IDENTIFICATION,
];

/// DM memory / identification plugin.
#[derive(Default)]
pub struct DmMemory {
    ecu_identification: Option<EcuIdentification>,
    software_identification: Option<SoftwareIdentification>,
    product_identification: Option<ProductIdentification>,
    last_dm14: Option<(Address, Dm14Request)>,
    last_dm15: Option<(Address, Dm15Response)>,
    last_dm16: Option<(Address, Dm16Transfer)>,
    last_ecu_identification: Option<(Address, EcuIdentification)>,
    pending: Vec<(Pgn, Vec<u8>, Address)>,
}

impl DmMemory {
    /// Create, optionally pre-loaded with the local ECU Identification.
    #[must_use]
    pub fn new(ecu_identification: Option<EcuIdentification>) -> Self {
        Self {
            ecu_identification,
            ..Self::default()
        }
    }

    /// Configure the ECU Identification answered to requests.
    pub fn set_ecu_identification(&mut self, id: Option<EcuIdentification>) {
        self.ecu_identification = id;
    }

    /// Configure the Software Identification answered to requests.
    pub fn set_software_identification(&mut self, id: Option<SoftwareIdentification>) {
        self.software_identification = id;
    }

    /// Configure the Product Identification answered to requests.
    pub fn set_product_identification(&mut self, id: Option<ProductIdentification>) {
        self.product_identification = id;
    }

    /// Last DM14 request seen.
    #[must_use]
    pub fn last_dm14(&self) -> Option<(Address, Dm14Request)> {
        self.last_dm14
    }

    /// Last DM15 response seen.
    #[must_use]
    pub fn last_dm15(&self) -> Option<(Address, Dm15Response)> {
        self.last_dm15
    }

    /// Queue a destination-specific request for ECU Identification.
    ///
    /// # Errors
    /// Propagates request-encoding errors.
    pub fn request_ecu_identification(&mut self, destination: Address) -> Result<()> {
        self.request(PGN_ECU_IDENTIFICATION, destination)
    }

    /// Queue a destination-specific request for Software Identification.
    ///
    /// # Errors
    /// Propagates request-encoding errors.
    pub fn request_software_identification(&mut self, destination: Address) -> Result<()> {
        self.request(PGN_SOFTWARE_ID, destination)
    }

    /// Queue a DM14 send.
    ///
    /// # Errors
    /// Propagates DM14 encoding errors.
    pub fn send_dm14(&mut self, destination: Address, request: &Dm14Request) -> Result<()> {
        self.pending
            .push((PGN_DM14, request.encode()?.to_vec(), destination));
        Ok(())
    }

    fn request(&mut self, pgn: Pgn, destination: Address) -> Result<()> {
        self.pending
            .push((PGN_REQUEST, encode_request(pgn)?.to_vec(), destination));
        Ok(())
    }

    fn respond_to_request(&self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        let Some(requested) = requested_pgn(msg) else {
            return;
        };
        let payload = match requested {
            PGN_ECU_IDENTIFICATION => self
                .ecu_identification
                .as_ref()
                .and_then(|i| i.encode().ok()),
            PGN_SOFTWARE_ID => self
                .software_identification
                .as_ref()
                .and_then(|i| i.encode().ok()),
            PGN_PRODUCT_IDENTIFICATION => self
                .product_identification
                .as_ref()
                .and_then(|i| i.encode().ok()),
            _ => return,
        };
        if let Some(payload) = payload {
            ctx.send(requested, payload, msg.source, Priority::Default);
        }
    }
}

impl Plugin for DmMemory {
    fn name(&self) -> &'static str {
        "dm_memory"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_source() || msg.destination == NULL_ADDRESS {
            return;
        }
        let local = ctx.address();
        if msg.destination != BROADCAST_ADDRESS && msg.destination != local {
            return;
        }
        match msg.pgn {
            PGN_REQUEST => self.respond_to_request(msg, ctx),
            PGN_DM14 => {
                if let Some(request) = Dm14Request::from_message(msg) {
                    self.last_dm14 = Some((msg.source, request));
                    ctx.emit(Event::DmMemory(DmMemoryEvent::Dm14Request {
                        source: msg.source,
                        request,
                    }));
                }
            }
            PGN_DM15 => {
                if let Some(response) = Dm15Response::decode(&msg.data) {
                    self.last_dm15 = Some((msg.source, response));
                    ctx.emit(Event::DmMemory(DmMemoryEvent::Dm15Response {
                        source: msg.source,
                        response,
                    }));
                }
            }
            PGN_DM16 => {
                if let Some(transfer) = Dm16Transfer::decode(&msg.data) {
                    self.last_dm16 = Some((msg.source, transfer.clone()));
                    ctx.emit(Event::DmMemory(DmMemoryEvent::Dm16Transfer {
                        source: msg.source,
                        transfer,
                    }));
                }
            }
            PGN_ECU_IDENTIFICATION => {
                if let Some(id) = EcuIdentification::decode(&msg.data) {
                    self.last_ecu_identification = Some((msg.source, id.clone()));
                    ctx.emit(Event::DmMemory(DmMemoryEvent::EcuIdentification {
                        source: msg.source,
                        identification: id,
                    }));
                }
            }
            _ => {}
        }
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        for (pgn, data, dst) in self.pending.drain(..) {
            ctx.send(pgn, data, dst, Priority::Default);
        }
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
