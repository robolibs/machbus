//! ISO 11783-5 NAME Management (`PGN 0x9300`) as a [`Plugin`]. Answers NAME
//! Management requests, adopts a commanded pending NAME (re-claiming), and
//! handles targeted RequestAddressClaim. Wraps the pure
//! [`crate::net::NameManager`] state machine.

use crate::net::pgn_defs::PGN_NAME_MANAGEMENT;
use crate::net::{BROADCAST_ADDRESS, Message, NULL_ADDRESS, Name, NameManager, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_NAME_MANAGEMENT];

/// NAME Management plugin.
pub struct NameManagement {
    manager: NameManager,
    adopted_name: Rc<RefCell<Option<Name>>>,
    request_address_claim: Rc<RefCell<bool>>,
}

impl NameManagement {
    /// Create a NAME Management responder.
    #[must_use]
    pub fn new() -> Self {
        let mut manager = NameManager::new();
        let adopted_name = Rc::new(RefCell::new(None));
        let sink = adopted_name.clone();
        manager
            .on_name_changed
            .subscribe(move |name| *sink.borrow_mut() = Some(*name));
        let request_address_claim = Rc::new(RefCell::new(false));
        let req_sink = request_address_claim.clone();
        manager
            .on_request_address_claim
            .subscribe(move |_| *req_sink.borrow_mut() = true);
        Self {
            manager,
            adopted_name,
            request_address_claim,
        }
    }

    /// Read the underlying manager.
    #[must_use]
    pub fn manager(&self) -> &NameManager {
        &self.manager
    }

    /// Mutate the underlying manager.
    pub fn manager_mut(&mut self) -> &mut NameManager {
        &mut self.manager
    }
}

impl Default for NameManagement {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for NameManagement {
    fn name(&self) -> &'static str {
        "name_management"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_source() || msg.destination == NULL_ADDRESS {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }

        let current_name = ctx.name();
        let reply = self.manager.handle_name_management(msg, current_name);
        let adopted = self.adopted_name.borrow_mut().take();
        let requested_address_claim = {
            let mut flag = self.request_address_claim.borrow_mut();
            core::mem::replace(&mut *flag, false)
        };

        if let Some(reply) = reply {
            ctx.send(
                PGN_NAME_MANAGEMENT,
                reply.msg.encode().to_vec(),
                reply.destination,
                Priority::Default,
            );
        }

        if let Some(new_name) = adopted {
            ctx.set_name(new_name);
        } else if requested_address_claim {
            ctx.send_address_claim_responses();
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
