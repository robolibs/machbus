//! Diagnostics (J1939 DM1) as a [`Plugin`] — the first subsystem ported to the
//! plugin model, proving the [`Session`](crate::session::Session) vertical slice.
//!
//! Behavior:
//! - broadcasts DM1 (active DTC list + lamp status) on a fixed cadence while any
//!   DTC is active;
//! - responds to a PGN-request for DM1 with the current list;
//! - emits [`DiagEvent::Dm1Received`] when a peer broadcasts DM1.
//!
//! Fine control: hold it via `session.get_mut::<Diagnostics>()` and call
//! [`Diagnostics::raise`] / [`Diagnostics::clear`].

use crate::j1939::diagnostic::{DiagnosticLamps, DmDtcList, Dtc};
use crate::net::pgn_defs::{PGN_DM1, PGN_REQUEST};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{DiagEvent, Event};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_DM1, PGN_REQUEST];

/// DM1 diagnostics plugin.
pub struct Diagnostics {
    interval_ms: u32,
    next_broadcast: Option<Instant>,
    lamps: DiagnosticLamps,
    active: Vec<Dtc>,
}

impl Diagnostics {
    /// Broadcast active DTCs every `interval_ms` milliseconds.
    #[must_use]
    pub fn every(interval_ms: u32) -> Self {
        Self {
            interval_ms,
            next_broadcast: None,
            lamps: DiagnosticLamps::default(),
            active: Vec::new(),
        }
    }

    /// Add a DTC to the active list (deduplicated by SPN+FMI).
    pub fn raise(&mut self, dtc: Dtc) {
        if !self
            .active
            .iter()
            .any(|d| d.spn == dtc.spn && d.fmi == dtc.fmi)
        {
            self.active.push(dtc);
        }
    }

    /// Clear all active DTCs.
    pub fn clear(&mut self) {
        self.active.clear();
    }

    /// Current active DTC list.
    #[must_use]
    pub fn active(&self) -> &[Dtc] {
        &self.active
    }

    fn dm1_payload(&self) -> Vec<u8> {
        DmDtcList {
            lamps: self.lamps,
            dtcs: self.active.clone(),
        }
        .encode()
    }
}

impl Plugin for Diagnostics {
    fn name(&self) -> &'static str {
        "diagnostics"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        match msg.pgn {
            PGN_DM1 => {
                if let Some(list) = DmDtcList::decode(&msg.data) {
                    ctx.emit(Event::Diag(DiagEvent::Dm1Received {
                        source: msg.source,
                        active: list.dtcs,
                        lamps: list.lamps,
                    }));
                }
            }
            PGN_REQUEST => {
                if msg.data.len() >= 3 {
                    let requested = u32::from(msg.data[0])
                        | (u32::from(msg.data[1]) << 8)
                        | (u32::from(msg.data[2]) << 16);
                    if requested == PGN_DM1 {
                        ctx.send(
                            PGN_DM1,
                            self.dm1_payload(),
                            BROADCAST_ADDRESS,
                            Priority::Default,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let due = self.next_broadcast.is_none_or(|t| now >= t);
        if due {
            if !self.active.is_empty() {
                ctx.send(
                    PGN_DM1,
                    self.dm1_payload(),
                    BROADCAST_ADDRESS,
                    Priority::Default,
                );
            }
            self.next_broadcast = Some(now.add_millis(u64::from(self.interval_ms)));
        }
        self.next_broadcast
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
