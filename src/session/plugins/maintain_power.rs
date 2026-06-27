//! Maintain Power (ISO 11783-9) as a [`Plugin`]. Wraps the pure [`PowerManager`]
//! lifecycle state machine; exchanges `PGN_MAINTAIN_POWER`, caches peer status,
//! and surfaces lifecycle transitions.

use crate::j1939::{MaintainPowerData, PowerManager, PowerRole, PowerState};
use crate::net::pgn_defs::PGN_MAINTAIN_POWER;
use crate::net::{Address, BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, MaintainPowerEvent};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_MAINTAIN_POWER];

/// Maintain Power plugin.
pub struct MaintainPower {
    manager: PowerManager,
    last: Option<(Address, MaintainPowerData)>,
    last_state: PowerState,
    last_tick: Option<Instant>,
}

impl MaintainPower {
    /// Create for the given role (`Tecu` source or consumer `Cf`).
    #[must_use]
    pub fn new(role: PowerRole) -> Self {
        let manager = PowerManager::new(role);
        let last_state = manager.state();
        Self {
            manager,
            last: None,
            last_state,
            last_tick: None,
        }
    }

    /// Local role.
    #[must_use]
    pub fn role(&self) -> PowerRole {
        self.manager.role()
    }

    /// Current lifecycle state.
    #[must_use]
    pub fn state(&self) -> PowerState {
        self.manager.state()
    }

    /// Last peer status observed.
    #[must_use]
    pub fn last(&self) -> Option<(Address, MaintainPowerData)> {
        self.last
    }

    /// TECU key-off input.
    pub fn key_off(&mut self) {
        self.manager.key_off();
    }

    /// TECU key-on input.
    pub fn key_on(&mut self) {
        self.manager.key_on();
    }

    /// CF power-extension request input.
    pub fn request_power(&mut self, need_power: bool) {
        self.manager.request_power(need_power);
    }

    fn emit_state_changes(&mut self, ctx: &mut PluginCtx<'_>) {
        let after = self.manager.state();
        if after != self.last_state {
            ctx.emit(Event::MaintainPower(MaintainPowerEvent::StateChanged(
                after,
            )));
            if matches!(after, PowerState::PowerOff) {
                ctx.emit(Event::MaintainPower(MaintainPowerEvent::PowerOff));
            }
            self.last_state = after;
        }
    }
}

impl Plugin for MaintainPower {
    fn name(&self) -> &'static str {
        "maintain_power"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_MAINTAIN_POWER) {
            return;
        }
        let Some(data) = MaintainPowerData::from_message(msg) else {
            return;
        };
        self.last = Some((msg.source, data));
        self.manager.handle_message(msg);
        ctx.emit(Event::MaintainPower(MaintainPowerEvent::Status {
            source: msg.source,
            data,
        }));
        self.emit_state_changes(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        self.last_tick = Some(now);

        let out = self.manager.update(elapsed);
        self.emit_state_changes(ctx);
        for data in out {
            ctx.send(
                PGN_MAINTAIN_POWER,
                data.encode().to_vec(),
                BROADCAST_ADDRESS,
                Priority::Default,
            );
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
