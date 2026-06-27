//! GNSS / NMEA 2000 as a [`Plugin`] — the first inbound-heavy subsystem port.
//!
//! Wraps the pump-style [`NMEAInterface`]: received GNSS PGNs are decoded and
//! re-emitted as [`Event::Gnss`]; outbound broadcasts requested via the
//! `broadcast_*` methods are buffered and flushed on the next tick. The cached
//! position is available for fine control via
//! `session.get::<Gnss>()?.latest_position()`.
//!
//! This establishes the reusable pattern for plugins that both *decode* inbound
//! traffic (subscribe the wrapped interface's native events into a buffer, drain
//! into the [`PluginCtx`]) and *emit on command* (buffer requests, flush in
//! [`Plugin::on_tick`]).

use crate::net::pgn_defs::{
    PGN_ATTITUDE, PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_DOPS, PGN_GNSS_POSITION_DATA,
    PGN_GNSS_POSITION_RAPID, PGN_HEADING_TRACK, PGN_MAGNETIC_VARIATION, PGN_RATE_OF_TURN,
    PGN_SYSTEM_TIME,
};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::nmea::{GNSSPosition, NMEAConfig, NMEAInterface};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, GnssEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[
    PGN_GNSS_POSITION_RAPID,
    PGN_GNSS_COG_SOG_RAPID,
    PGN_ATTITUDE,
    PGN_RATE_OF_TURN,
    PGN_GNSS_POSITION_DATA,
    PGN_GNSS_DOPS,
    PGN_HEADING_TRACK,
    PGN_MAGNETIC_VARIATION,
    PGN_SYSTEM_TIME,
];

const FAST_PACKET: &[Pgn] = &[PGN_GNSS_POSITION_DATA];

/// GNSS / NMEA 2000 plugin.
pub struct Gnss {
    iface: NMEAInterface,
    collected: Rc<RefCell<Vec<GnssEvent>>>,
    pending: Vec<(Pgn, Vec<u8>)>,
}

impl Gnss {
    /// Listen for and decode GNSS traffic with the given NMEA configuration.
    #[must_use]
    pub fn new(config: NMEAConfig) -> Self {
        let mut iface = NMEAInterface::new(config);
        let collected = Rc::new(RefCell::new(Vec::new()));
        wire_events(&mut iface, &collected);
        Self {
            iface,
            collected,
            pending: Vec::new(),
        }
    }

    /// Listen with the default NMEA configuration.
    #[must_use]
    pub fn listen() -> Self {
        Self::new(NMEAConfig::default())
    }

    /// Latest cached position, or `None` before the first fix.
    #[must_use]
    pub fn latest_position(&self) -> Option<GNSSPosition> {
        self.iface.latest_position()
    }

    /// Queue a position broadcast (`PGN_GNSS_POSITION_RAPID`), flushed on tick.
    pub fn broadcast_position(&mut self, pos: &GNSSPosition) {
        self.pending.push((
            PGN_GNSS_POSITION_RAPID,
            NMEAInterface::build_position(pos).to_vec(),
        ));
    }

    /// Queue a COG/SOG broadcast (`PGN_GNSS_COG_SOG_RAPID`), flushed on tick.
    pub fn broadcast_cog_sog(&mut self, cog_rad: f64, sog_mps: f64) {
        self.pending.push((
            PGN_GNSS_COG_SOG_RAPID,
            NMEAInterface::build_cog_sog(cog_rad, sog_mps).to_vec(),
        ));
    }

    /// Direct access to the wrapped interface for advanced configuration.
    pub fn interface_mut(&mut self) -> &mut NMEAInterface {
        &mut self.iface
    }
}

impl Plugin for Gnss {
    fn name(&self) -> &'static str {
        "gnss"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn fast_packet_pgns(&self) -> &'static [Pgn] {
        FAST_PACKET
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        self.iface.handle_message(msg);
        for event in self.collected.borrow_mut().drain(..) {
            ctx.emit(Event::Gnss(event));
        }
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        for (pgn, data) in self.pending.drain(..) {
            ctx.send(pgn, data, BROADCAST_ADDRESS, Priority::Default);
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

/// Subscribe the interface's native events into a buffer the plugin drains.
fn wire_events(iface: &mut NMEAInterface, sink: &Rc<RefCell<Vec<GnssEvent>>>) {
    let s = sink.clone();
    iface.on_position.subscribe(move |&pos| {
        s.borrow_mut().push(GnssEvent::Position(pos));
    });
    let s = sink.clone();
    iface.on_cog.subscribe(move |&v| {
        s.borrow_mut().push(GnssEvent::Cog(v));
    });
    let s = sink.clone();
    iface.on_sog.subscribe(move |&v| {
        s.borrow_mut().push(GnssEvent::Sog(v));
    });
    let s = sink.clone();
    iface.on_heading.subscribe(move |&v| {
        s.borrow_mut().push(GnssEvent::Heading(v));
    });
    let s = sink.clone();
    iface.on_magnetic_variation.subscribe(move |&v| {
        s.borrow_mut().push(GnssEvent::MagneticVariation(v));
    });
    let s = sink.clone();
    iface.on_attitude.subscribe(move |&(yaw, pitch, roll)| {
        s.borrow_mut()
            .push(GnssEvent::Attitude { yaw, pitch, roll });
    });
    let s = sink.clone();
    iface.on_gnss_dops.subscribe(move |dops| {
        s.borrow_mut().push(GnssEvent::Dops(*dops));
    });
    let s = sink.clone();
    iface.on_system_time.subscribe(move |st| {
        s.borrow_mut().push(GnssEvent::SystemTime(*st));
    });
}
