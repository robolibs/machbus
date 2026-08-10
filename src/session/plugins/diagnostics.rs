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
use crate::j1939::acknowledgment::Acknowledgment;
use crate::net::pgn_defs::{PGN_ACKNOWLEDGMENT, PGN_DM1, PGN_REQUEST};
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
    /// Whether the last broadcast carried at least one DTC.
    ///
    /// ISO 11783-12: a DTC that "has been active for 1 s or longer, and then
    /// becomes inactive" requires one DM1 reflecting the change, "after that,
    /// the DM1 is discontinued". Simply falling silent left the fault latched
    /// in every receiver on the bus, with no way to learn it had cleared.
    announced_active: bool,
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
            announced_active: false,
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
        // ISO 11783-12 form: bytes 1-2 are reserved and set to 0xFF. This
        // plugin speaks on an ISO 11783 network, not a bare J1939 one.
        DmDtcList {
            lamps: self.lamps,
            dtcs: self.active.clone(),
        }
        .encode_iso()
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
                    } else if msg.destination != BROADCAST_ADDRESS {
                        // ISO 11783-3 §5.4.x: "A response is always required
                        // from a specified destination (not global), even if it
                        // is a NACK indicating that the particular PGN value is
                        // not supported" — and conversely "A global request
                        // shall not be responded to with a NACK". Staying
                        // silent left the requester waiting out its timeout with
                        // no way to tell "unsupported" from "no reply".
                        let nack = Acknowledgment::nack(requested, ctx.address());
                        if let Ok(payload) = nack.encode() {
                            ctx.send(
                                PGN_ACKNOWLEDGMENT,
                                payload.to_vec(),
                                msg.source,
                                Priority::Default,
                            );
                        }
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
            let has_active = !self.active.is_empty();
            // Broadcast while faults are active, and exactly once more on the
            // transition to none so receivers can clear what they are holding.
            if has_active || self.announced_active {
                ctx.send(
                    PGN_DM1,
                    self.dm1_payload(),
                    BROADCAST_ADDRESS,
                    Priority::Default,
                );
            }
            self.announced_active = has_active;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::j1939::diagnostic::Fmi;
    use crate::net::Name;
    use crate::session::Session;

    fn node() -> Session {
        let name = Name::default()
            .with_identity_number(0x31)
            .with_function_code(0x80)
            .with_self_configurable(true);
        let mut s = Session::builder(name, 0x80)
            .plug(Diagnostics::every(1000))
            .build()
            .unwrap();
        s.start().unwrap();
        let mut now = Instant::ZERO;
        for _ in 0..40 {
            now = now.add_millis(100);
            s.tick(now);
            while s.poll_transmit().is_some() {}
            if s.is_claimed() {
                break;
            }
        }
        s
    }

    fn drain_dm1(s: &mut Session) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Some((_, frame)) = s.poll_transmit() {
            if frame.id.pgn() == PGN_DM1 {
                out.push(frame.data.to_vec());
            }
        }
        out
    }

    /// H37 — ISO 11783-3: "A response is always required from a specified
    /// destination (not global), even if it is a NACK indicating that the
    /// particular PGN value is not supported", and "A global request shall not
    /// be responded to with a NACK when a particular PGN is not supported".
    /// Staying silent left the requester waiting out its timeout, unable to
    /// tell "unsupported" from "no reply".
    #[test]
    fn an_unsupported_diagnostic_request_is_nacked_only_when_addressed() {
        use crate::j1939::acknowledgment::{AckControl, Acknowledgment};
        use crate::net::pgn_defs::{PGN_ACKNOWLEDGMENT, PGN_DM2};
        use crate::net::{Address, Frame, Identifier, Priority};

        fn request(dest: Address, pgn: Pgn) -> Frame {
            Frame::new(
                Identifier::encode(Priority::Default, PGN_REQUEST, 0x26, dest),
                [
                    (pgn & 0xFF) as u8,
                    ((pgn >> 8) & 0xFF) as u8,
                    ((pgn >> 16) & 0xFF) as u8,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                ],
                8,
            )
        }

        let mut s = node();
        let now = Instant::from_millis(10_000);
        let our_address = s.address();

        // Addressed to us, for a PGN this plugin does not serve.
        s.feed(0, &request(our_address, PGN_DM2), now);
        let mut nack = None;
        while let Some((_, frame)) = s.poll_transmit() {
            if frame.id.pgn() == PGN_ACKNOWLEDGMENT {
                nack = Acknowledgment::decode(&frame.data);
            }
        }
        let nack = nack.expect("an addressed request for an unsupported PGN must be answered");
        assert_eq!(nack.control, AckControl::NegativeAck);
        assert_eq!(nack.acknowledged_pgn, PGN_DM2);

        // The same request sent globally must not be NACKed.
        s.feed(0, &request(BROADCAST_ADDRESS, PGN_DM2), now.add_millis(10));
        let mut saw_ack = false;
        while let Some((_, frame)) = s.poll_transmit() {
            saw_ack |= frame.id.pgn() == PGN_ACKNOWLEDGMENT;
        }
        assert!(!saw_ack, "a NACK is not permitted for a global request");
    }

    /// H36 — ISO 11783-12: a DTC that "has been active for 1 s or longer, and
    /// then becomes inactive" requires one DM1 reflecting the change, "after
    /// that, the DM1 is discontinued". Falling silent instead left the fault
    /// latched in every receiver on the bus with no way to learn it cleared.
    #[test]
    fn clearing_the_last_dtc_is_announced_exactly_once() {
        let mut s = node();
        let mut now = Instant::from_millis(10_000);
        s.get_mut::<Diagnostics>().unwrap().raise(Dtc {
            spn: 1234,
            fmi: Fmi::AboveNormal,
            occurrence_count: 1,
            conversion_method: false,
        });

        now = now.add_millis(1_000);
        s.tick(now);
        assert_eq!(drain_dm1(&mut s).len(), 1, "the active fault is broadcast");

        s.get_mut::<Diagnostics>().unwrap().clear();
        now = now.add_millis(1_000);
        s.tick(now);
        let cleared = drain_dm1(&mut s);
        assert_eq!(
            cleared.len(),
            1,
            "the transition to no active faults must be broadcast"
        );
        let decoded = DmDtcList::decode(&cleared[0]).expect("a well-formed DM1");
        assert!(
            decoded.dtcs.is_empty(),
            "the clearing DM1 carries no active DTCs"
        );

        // And then it stops: nothing further while the list stays empty.
        for _ in 0..5 {
            now = now.add_millis(1_000);
            s.tick(now);
        }
        assert!(
            drain_dm1(&mut s).is_empty(),
            "after the state change the DM1 is discontinued"
        );
    }
}
