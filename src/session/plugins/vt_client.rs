//! Virtual Terminal client as a [`Plugin`]. Wraps the pump-style [`VTClient`] +
//! [`VTClientStateTracker`] + [`AuxCapabilityDiscovery`]: routes `PGN_VT_TO_ECU`
//! and `PGN_LANGUAGE_COMMAND`, ships handshake/command frames on tick, and
//! surfaces VT activity as [`VtEvent`].
//!
//! Commands (`show`/`set_value`/…) buffer their encoded frame and flush on the
//! next tick; `connect_to` arms the connect FSM (the self-address is resolved on
//! tick, once claimed).

use crate::isobus::vt::{
    AuxCapabilities, AuxCapabilityDiscovery, ClientOutbound, ObjectID, ObjectPool, VTClient,
    VTClientConfig, VTClientStateTracker, VTState, WorkingSet,
};
use crate::net::pgn_defs::{PGN_ECU_TO_VT, PGN_LANGUAGE_COMMAND, PGN_VT_TO_ECU};
use crate::net::{Address, BROADCAST_ADDRESS, Error, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, VtEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_VT_TO_ECU, PGN_LANGUAGE_COMMAND];

/// Virtual Terminal client plugin.
pub struct VtClient {
    client: VTClient,
    tracker: VTClientStateTracker,
    aux_discovery: AuxCapabilityDiscovery,
    vt_address: Option<Address>,
    connect_requested: bool,
    events: Rc<RefCell<Vec<VtEvent>>>,
    pending: Vec<(Pgn, Vec<u8>, Address)>,
}

impl VtClient {
    /// Create with a config, object pool, and working set.
    #[must_use]
    pub fn new(config: VTClientConfig, pool: ObjectPool, ws: WorkingSet) -> Self {
        let mut client = VTClient::new(config);
        client.set_object_pool(pool);
        client.set_working_set(ws);
        let events = Rc::new(RefCell::new(Vec::new()));
        wire_events(&mut client, &events);
        Self {
            client,
            tracker: VTClientStateTracker::new(),
            aux_discovery: AuxCapabilityDiscovery::new(),
            vt_address: None,
            connect_requested: false,
            events,
            pending: Vec::new(),
        }
    }

    /// Target a VT server address and arm the connect handshake (started on the
    /// next tick, once an address is claimed).
    pub fn connect_to(&mut self, server: Address) {
        self.vt_address = Some(server);
        self.connect_requested = true;
    }

    /// Disconnect from the VT.
    ///
    /// # Errors
    /// Propagates client disconnect errors.
    pub fn disconnect(&mut self) -> Result<()> {
        self.client.disconnect()
    }

    /// Current FSM state.
    #[must_use]
    pub fn state(&self) -> VTState {
        self.client.state()
    }

    /// Whether the upload + activation handshake has completed.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self.state(), VTState::Connected)
    }

    /// Show an object.
    ///
    /// # Errors
    /// Propagates encode errors.
    pub fn show(&mut self, id: impl Into<ObjectID>) -> Result<()> {
        let out = self.client.hide_show(id.into(), true)?;
        self.queue(out);
        Ok(())
    }

    /// Hide an object.
    ///
    /// # Errors
    /// Propagates encode errors.
    pub fn hide(&mut self, id: impl Into<ObjectID>) -> Result<()> {
        let out = self.client.hide_show(id.into(), false)?;
        self.queue(out);
        Ok(())
    }

    /// Enable an object.
    ///
    /// # Errors
    /// Propagates encode errors.
    pub fn enable(&mut self, id: impl Into<ObjectID>) -> Result<()> {
        let out = self.client.enable_disable(id.into(), true)?;
        self.queue(out);
        Ok(())
    }

    /// Disable an object.
    ///
    /// # Errors
    /// Propagates encode errors.
    pub fn disable(&mut self, id: impl Into<ObjectID>) -> Result<()> {
        let out = self.client.enable_disable(id.into(), false)?;
        self.queue(out);
        Ok(())
    }

    /// Push a numeric value (updates the local tracker).
    ///
    /// # Errors
    /// Propagates encode errors.
    pub fn set_value(&mut self, id: impl Into<ObjectID>, value: u32) -> Result<()> {
        let id = id.into();
        let out = self.client.change_numeric_value(id, value)?;
        self.tracker.set_numeric_value(id, value);
        self.queue(out);
        Ok(())
    }

    /// Push a string value (updates the local tracker).
    ///
    /// # Errors
    /// Propagates encode errors.
    pub fn set_string(&mut self, id: impl Into<ObjectID>, value: &str) -> Result<()> {
        let id = id.into();
        let out = self.client.change_string_value(id, value)?;
        self.tracker.set_string_value(id, value);
        self.queue(out);
        Ok(())
    }

    /// Switch the active data mask of a working-set object.
    ///
    /// # Errors
    /// Propagates encode errors.
    pub fn change_active_mask(
        &mut self,
        ws: impl Into<ObjectID>,
        mask: impl Into<ObjectID>,
    ) -> Result<()> {
        let out = self.client.change_active_mask(ws.into(), mask.into())?;
        self.queue(out);
        Ok(())
    }

    /// Request VT v5 auxiliary capabilities from the configured VT.
    ///
    /// # Errors
    /// Returns an error if `connect_to` has not set a VT address.
    pub fn request_aux_capabilities(&mut self) -> Result<()> {
        let dest = self
            .vt_address
            .ok_or_else(|| Error::invalid_state("VT auxiliary discovery requires connect_to"))?;
        self.request_aux_capabilities_from(dest)
    }

    /// Request VT v5 auxiliary capabilities from `server` directly.
    ///
    /// # Errors
    /// Propagates encode errors.
    pub fn request_aux_capabilities_from(&mut self, server: Address) -> Result<()> {
        self.vt_address = Some(server);
        let payload = self.aux_discovery.request_capabilities()?.to_vec();
        self.pending.push((PGN_ECU_TO_VT, payload, server));
        Ok(())
    }

    /// Last accepted auxiliary capability response.
    #[must_use]
    pub fn aux_capabilities(&self) -> &AuxCapabilities {
        self.aux_discovery.capabilities()
    }

    /// Cached numeric value, if seen.
    #[must_use]
    pub fn numeric_value(&self, id: impl Into<ObjectID>) -> Option<u32> {
        self.tracker.numeric_value(id.into())
    }

    /// Cached string value, if seen.
    #[must_use]
    pub fn string_value(&self, id: impl Into<ObjectID>) -> Option<String> {
        self.tracker.string_value(id.into()).map(str::to_string)
    }

    /// Direct access to the underlying client.
    pub fn client_mut(&mut self) -> &mut VTClient {
        &mut self.client
    }

    fn queue(&mut self, out: ClientOutbound) {
        let dest = out.dest.unwrap_or(BROADCAST_ADDRESS);
        self.pending.push((out.pgn, out.data, dest));
    }

    fn drain_events(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.events.borrow_mut().drain(..) {
            ctx.emit(Event::Vt(event));
        }
    }
}

impl Plugin for VtClient {
    fn name(&self) -> &'static str {
        "vt_client"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        let local = ctx.address();
        match msg.pgn {
            PGN_VT_TO_ECU => {
                if !msg.has_usable_envelope_for_pgn(PGN_VT_TO_ECU) {
                    return;
                }
                if msg.destination != BROADCAST_ADDRESS && msg.destination != local {
                    return;
                }
                let from_configured_vt = self.vt_address.is_none_or(|a| msg.source == a);
                if from_configured_vt
                    && let Some(capabilities) = self.aux_discovery.handle_response(msg)
                {
                    ctx.emit(Event::Vt(VtEvent::AuxCapabilities {
                        source: msg.source,
                        capabilities: capabilities.clone(),
                    }));
                }
                self.client.handle_vt_message(msg);
                self.tracker.handle_vt_message(msg);
            }
            PGN_LANGUAGE_COMMAND => {
                if !msg.has_usable_envelope_for_pgn(PGN_LANGUAGE_COMMAND) {
                    return;
                }
                if msg.destination != BROADCAST_ADDRESS && msg.destination != local {
                    return;
                }
                self.client.handle_language_command(msg);
            }
            _ => {}
        }
        self.drain_events(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        self.client.set_self_address(ctx.address());
        if self.connect_requested {
            self.connect_requested = false;
            let _ = self.client.connect();
        }
        // FSM-driven frames.
        let frames: Vec<_> = self.client.update(0).into_iter().collect();
        for out in frames {
            self.queue(out);
        }
        for (pgn, data, dst) in self.pending.drain(..) {
            ctx.send(pgn, data, dst, Priority::Default);
        }
        self.drain_events(ctx);
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn wire_events(client: &mut VTClient, sink: &Rc<RefCell<Vec<VtEvent>>>) {
    let s = sink.clone();
    client
        .on_state_change
        .subscribe(move |&st| s.borrow_mut().push(VtEvent::StateChanged(st)));
    let s = sink.clone();
    client
        .on_soft_key
        .subscribe(move |&(id, code)| s.borrow_mut().push(VtEvent::SoftKey { id, code }));
    let s = sink.clone();
    client
        .on_button
        .subscribe(move |&(id, code)| s.borrow_mut().push(VtEvent::Button { id, code }));
    let s = sink.clone();
    client
        .on_numeric_value_change
        .subscribe(move |&(id, value)| {
            s.borrow_mut()
                .push(VtEvent::NumericValueChanged { id, value });
        });
    let s = sink.clone();
    client.on_string_value_change.subscribe(move |(id, value)| {
        s.borrow_mut().push(VtEvent::StringValueChanged {
            id: *id,
            value: value.clone(),
        });
    });
    let s = sink.clone();
    client
        .on_pool_error
        .subscribe(move |&code| s.borrow_mut().push(VtEvent::PoolError(code)));
    let s = sink.clone();
    client
        .on_active_ws_status
        .subscribe(move |&active| s.borrow_mut().push(VtEvent::ActiveWorkingSet(active)));
    let s = sink.clone();
    client.on_language_change.subscribe(move |&(from, to)| {
        s.borrow_mut().push(VtEvent::LanguageChanged { from, to });
    });
}
