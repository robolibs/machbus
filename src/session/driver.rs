//! Layer 2 — `Driver` + `Controls` over a [`Session`].
//!
//! The driver owns a [`Transport`] (the CAN boundary) and a clock, and runs the
//! pump: read frames → [`Session::feed`], [`Session::tick`], drain
//! [`Session::poll_transmit`] → transport, then surface the next
//! [`Session::poll_event`]. [`Controls`] is a cheap, cloneable command handle
//! sharing the same session, so application code keeps a `Controls` while a loop
//! owns the `Driver` — the rumqttc-style split.
//!
//! Single-threaded by design (the shared session is `Rc<RefCell<…>>`): this is
//! the sync, embedded-friendly base case. A `Send` cross-thread command channel
//! and an async driver are later, additive layers over the same core.
//!
//! ```no_run
//! use machbus::session::{Session, plugins::Diagnostics, driver::EndpointTransport};
//! # fn demo<L: machbus::net::can_adapter::Link>(
//! #     ep: machbus::net::can_adapter::CanEndpoint<L>
//! # ) -> machbus::net::Result<()> {
//! use machbus::prelude::*;
//! let (ctrl, mut driver) = Session::builder(Name::default(), 0x80)
//!     .plug(Diagnostics::every(1000))
//!     .spawn(EndpointTransport::new(0, ep))?;
//! ctrl.start()?;
//! loop {
//!     if let Some(event) = driver.poll()? { /* handle(event) */ }
//! }
//! # }
//! ```

use super::events::SubsystemEvent;
use super::{Plugin, Session, SessionBuilder};
use crate::net::can_adapter;
use crate::net::{Address, Error, Frame, Pgn, Priority, Result};
use crate::session::sys::Event;
use crate::time::Instant;
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;

/// A boxed event callback registered via [`Driver::on_event`].
type EventCallback = Box<dyn FnMut(&Event)>;

/// Registry of event callbacks, dispatched by [`Driver::pump`].
#[derive(Default)]
struct CallbackRegistry {
    next_id: u64,
    callbacks: Vec<(u64, EventCallback)>,
}

impl CallbackRegistry {
    fn add(&mut self, cb: EventCallback) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.callbacks.push((id, cb));
        id
    }

    fn remove(&mut self, id: u64) {
        self.callbacks.retain(|(cid, _)| *cid != id);
    }

    fn dispatch(&mut self, event: &Event) {
        for (_, cb) in &mut self.callbacks {
            cb(event);
        }
    }
}

/// RAII handle for a registered event callback. Drop it to unsubscribe.
#[must_use = "dropping the Subscription immediately unsubscribes the callback"]
pub struct Subscription {
    id: u64,
    registry: Weak<RefCell<CallbackRegistry>>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            registry.borrow_mut().remove(self.id);
        }
    }
}

/// The CAN boundary. A driver reads frames from and writes frames to a
/// `Transport`. Implementations are non-blocking: [`Transport::recv`] returns
/// `None` when no frame is currently available.
pub use crate::net::CanTransport as Transport;

/// Adapts a single CAN endpoint (one `port`) into a [`Transport`].
pub struct EndpointTransport<L: can_adapter::Link> {
    port: u8,
    endpoint: can_adapter::CanEndpoint<L>,
}

impl<L: can_adapter::Link> EndpointTransport<L> {
    /// Wrap `endpoint` as the transport for `port`.
    pub fn new(port: u8, endpoint: can_adapter::CanEndpoint<L>) -> Self {
        Self { port, endpoint }
    }
}

impl<L: can_adapter::Link> Transport for EndpointTransport<L> {
    type Error = Error;

    fn recv(&mut self) -> Option<(u8, Frame)> {
        while let Ok(can) = self.endpoint.recv_can() {
            if let Some(frame) = Frame::from_can_frame(&can) {
                return Some((self.port, frame));
            }
        }
        None
    }

    fn send(&mut self, _port: u8, frame: &Frame) -> Result<()> {
        self.endpoint
            .send_can(&frame.to_can_frame())
            .map_err(|e| Error::with_message(crate::net::ErrorCode::DriverError, e.to_string()))
    }
}

/// A cheap, cloneable command handle sharing a [`Session`] with its [`Driver`].
///
/// Use it to start address claiming, read status, drive a plugged subsystem for
/// fine control, or send raw messages — all between (or alongside) driver polls.
#[derive(Clone)]
pub struct Controls {
    session: Rc<RefCell<Session>>,
}

impl Controls {
    /// Begin address claiming.
    ///
    /// # Errors
    /// Propagates the underlying claim-start error.
    pub fn start(&self) -> Result<()> {
        self.session.borrow_mut().start()
    }

    /// Current source address (`NULL_ADDRESS` until claimed).
    #[must_use]
    pub fn address(&self) -> Address {
        self.session.borrow().address()
    }

    /// Whether an address has been claimed.
    #[must_use]
    pub fn is_claimed(&self) -> bool {
        self.session.borrow().is_claimed()
    }

    /// Read a plugged subsystem by type. Returns `None` if not plugged.
    pub fn with<P: Plugin, R>(&self, f: impl FnOnce(&P) -> R) -> Option<R> {
        self.session.borrow().get::<P>().map(f)
    }

    /// Mutate a plugged subsystem by type for fine control. Returns `None` if
    /// not plugged.
    pub fn with_mut<P: Plugin, R>(&self, f: impl FnOnce(&mut P) -> R) -> Option<R> {
        self.session.borrow_mut().get_mut::<P>().map(f)
    }

    /// Raw escape hatch: send an arbitrary application message.
    ///
    /// # Errors
    /// Propagates [`Session::send_raw`] errors.
    pub fn send_raw(&self, pgn: Pgn, data: &[u8], dst: Address, priority: Priority) -> Result<()> {
        self.session.borrow_mut().send_raw(pgn, data, dst, priority)
    }

    /// Drain just one subsystem's events (typed per-subsystem stream).
    ///
    /// `controls.drain::<VtEvent>()` returns the queued VT events and leaves the
    /// rest for the driver / other drains.
    pub fn drain<E: SubsystemEvent + Clone>(&self) -> Vec<E> {
        self.session.borrow_mut().drain::<E>()
    }
}

/// Owns the [`Transport`] and clock, and runs the [`Session`] pump.
pub struct Driver<T: Transport> {
    session: Rc<RefCell<Session>>,
    transport: T,
    callbacks: Rc<RefCell<CallbackRegistry>>,
    origin: Option<std::time::Instant>,
}

impl<T> Driver<T>
where
    T: Transport,
    T::Error: Into<Error>,
{
    fn new(session: Rc<RefCell<Session>>, transport: T) -> Self {
        Self {
            session,
            transport,
            callbacks: Rc::new(RefCell::new(CallbackRegistry::default())),
            origin: None,
        }
    }

    /// A fresh [`Controls`] handle for this driver's session.
    #[must_use]
    pub fn controls(&self) -> Controls {
        Controls {
            session: self.session.clone(),
        }
    }

    /// Register a callback for every event. Returns an RAII [`Subscription`];
    /// drop it to unsubscribe. Callbacks fire from [`Self::pump`].
    pub fn on_event(&self, cb: impl FnMut(&Event) + 'static) -> Subscription {
        let id = self.callbacks.borrow_mut().add(Box::new(cb));
        Subscription {
            id,
            registry: Rc::downgrade(&self.callbacks),
        }
    }

    /// Register a callback for one subsystem's events, e.g.
    /// `driver.on::<VtEvent>(|e| …)`. Returns an RAII [`Subscription`].
    pub fn on<E: SubsystemEvent + 'static>(
        &self,
        mut cb: impl FnMut(&E) + 'static,
    ) -> Subscription {
        self.on_event(move |event| {
            if let Some(typed) = E::try_ref(event) {
                cb(typed);
            }
        })
    }

    /// Callback-style pump: do one IO/tick cycle and dispatch every resulting
    /// event to the registered callbacks (see [`Self::on`]). Returns how many
    /// events were dispatched. Use this instead of the [`Self::poll`] loop when
    /// you prefer callbacks.
    ///
    /// # Errors
    /// Propagates transport send failures.
    pub fn pump(&mut self) -> Result<usize> {
        let now = self.now();
        self.pump_at(now)
    }

    /// Callback-style pump at an explicit time (deterministic counterpart of
    /// [`Self::pump`]).
    ///
    /// # Errors
    /// Propagates transport send failures.
    pub fn pump_at(&mut self, now: Instant) -> Result<usize> {
        {
            let mut session = self.session.borrow_mut();
            while let Some((port, frame)) = self.transport.recv() {
                session.feed(port, &frame, now);
            }
            session.tick(now);
            while let Some((port, frame)) = session.poll_transmit() {
                self.transport.send(port, &frame).map_err(Into::into)?;
            }
        }
        let mut count = 0;
        loop {
            let event = self.session.borrow_mut().poll_event();
            let Some(event) = event else { break };
            self.callbacks.borrow_mut().dispatch(&event);
            count += 1;
        }
        Ok(count)
    }

    /// Pump once at an explicit time: drain RX → feed, tick, drain TX → transport,
    /// then return the next pending event (or `None`). This is the deterministic,
    /// `no_std`-friendly entry point — tests and embedded loops supply `now`.
    ///
    /// # Errors
    /// Propagates transport send failures.
    pub fn poll_at(&mut self, now: Instant) -> Result<Option<Event>> {
        let mut session = self.session.borrow_mut();
        while let Some((port, frame)) = self.transport.recv() {
            session.feed(port, &frame, now);
        }
        session.tick(now);
        while let Some((port, frame)) = session.poll_transmit() {
            self.transport.send(port, &frame).map_err(Into::into)?;
        }
        Ok(session.poll_event())
    }

    /// Pump once using the host monotonic clock. Convenience over
    /// [`Self::poll_at`] for std applications.
    ///
    /// # Errors
    /// Propagates transport send failures.
    pub fn poll(&mut self) -> Result<Option<Event>> {
        let now = self.now();
        self.poll_at(now)
    }

    fn now(&mut self) -> Instant {
        let origin = *self.origin.get_or_insert_with(std::time::Instant::now);
        Instant::from_micros(origin.elapsed().as_micros() as u64)
    }
}

impl SessionBuilder {
    /// Build the session and split it into a [`Controls`] handle and a [`Driver`]
    /// bound to `transport`.
    ///
    /// # Errors
    /// Propagates [`SessionBuilder::build`] errors (e.g. duplicate plugin types).
    pub fn spawn<T>(self, transport: T) -> Result<(Controls, Driver<T>)>
    where
        T: Transport,
        T::Error: Into<Error>,
    {
        let session = Rc::new(RefCell::new(self.build()?));
        let controls = Controls {
            session: session.clone(),
        };
        let driver = Driver::new(session, transport);
        Ok((controls, driver))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::j1939::diagnostic::{DiagnosticLamps, DmDtcList, Dtc, Fmi};
    use crate::net::pgn_defs::PGN_DM1;
    use crate::net::{BROADCAST_ADDRESS, Identifier, Name};
    use crate::session::Session;
    use crate::session::plugins::Diagnostics;
    use crate::session::sys::{ClaimEvent, DiagEvent};
    use std::collections::VecDeque;

    /// In-memory transport with shared handles so a test can inject RX frames
    /// and observe TX frames after the transport is moved into the driver.
    #[derive(Clone, Default)]
    struct MemTransport {
        rx: Rc<RefCell<VecDeque<(u8, Frame)>>>,
        tx: Rc<RefCell<Vec<(u8, Frame)>>>,
    }

    impl Transport for MemTransport {
        type Error = Error;

        fn recv(&mut self) -> Option<(u8, Frame)> {
            self.rx.borrow_mut().pop_front()
        }
        fn send(&mut self, port: u8, frame: &Frame) -> Result<()> {
            self.tx.borrow_mut().push((port, *frame));
            Ok(())
        }
    }

    fn test_name(identity: u32) -> Name {
        Name::default()
            .with_identity_number(identity)
            .with_function_code(0x80)
            .with_self_configurable(true)
    }

    fn pump_until_claimed(driver: &mut Driver<MemTransport>, ctrl: &Controls) {
        let mut now = Instant::ZERO;
        for _ in 0..40 {
            now = now.add_millis(100);
            while driver.poll_at(now).unwrap().is_some() {}
            if ctrl.is_claimed() {
                return;
            }
        }
        panic!("driver should claim an address with no contention");
    }

    #[test]
    fn driver_claims_and_surfaces_event_through_transport() {
        let mem = MemTransport::default();
        let (ctrl, mut driver) = Session::builder(test_name(1), 0x80)
            .plug(Diagnostics::every(1000))
            .spawn(mem.clone())
            .unwrap();
        ctrl.start().unwrap();

        let mut now = Instant::ZERO;
        let mut claimed_event = false;
        for _ in 0..40 {
            now = now.add_millis(100);
            while let Some(ev) = driver.poll_at(now).unwrap() {
                if matches!(ev, Event::AddressClaim(ClaimEvent::Claimed { .. })) {
                    claimed_event = true;
                }
            }
            if ctrl.is_claimed() {
                break;
            }
        }
        assert!(ctrl.is_claimed());
        assert!(
            claimed_event,
            "Claimed event must reach the driver consumer"
        );
        // Address-claim frames must have gone out through the transport.
        assert!(!mem.tx.borrow().is_empty());
    }

    #[test]
    fn controls_drive_plugin_and_driver_transmits_dm1() {
        let mem = MemTransport::default();
        let (ctrl, mut driver) = Session::builder(test_name(2), 0x80)
            .plug(Diagnostics::every(1000))
            .spawn(mem.clone())
            .unwrap();
        ctrl.start().unwrap();
        pump_until_claimed(&mut driver, &ctrl);
        mem.tx.borrow_mut().clear();

        // Fine control through the cheap handle.
        let raised = ctrl.with_mut::<Diagnostics, _>(|d| {
            d.raise(Dtc {
                spn: 1234,
                fmi: Fmi::BelowNormal,
                occurrence_count: 1,
            });
            d.active().len()
        });
        assert_eq!(raised, Some(1));

        // Advance past the broadcast interval; the driver must transmit a DM1.
        let now = Instant::from_millis(10_000);
        while driver.poll_at(now).unwrap().is_some() {}
        assert!(
            mem.tx.borrow().iter().any(|(_, f)| f.pgn() == PGN_DM1),
            "driver must transmit the plugin's DM1 broadcast"
        );
    }

    #[test]
    fn driver_feeds_inbound_and_returns_plugin_event() {
        let mem = MemTransport::default();
        let (ctrl, mut driver) = Session::builder(test_name(3), 0x80)
            .plug(Diagnostics::every(1000))
            .spawn(mem.clone())
            .unwrap();
        ctrl.start().unwrap();
        pump_until_claimed(&mut driver, &ctrl);

        // Inject a peer DM1 onto the transport's RX queue.
        let list = DmDtcList {
            lamps: DiagnosticLamps::default(),
            dtcs: vec![Dtc {
                spn: 2000,
                fmi: Fmi::ConditionExists,
                occurrence_count: 1,
            }],
        };
        let mut payload = [0xFFu8; 8];
        let encoded = list.encode();
        let n = encoded.len().min(8);
        payload[..n].copy_from_slice(&encoded[..n]);
        let frame = Frame::new(
            Identifier::encode(Priority::Default, PGN_DM1, 0x20, BROADCAST_ADDRESS),
            payload,
            8,
        );
        mem.rx.borrow_mut().push_back((0, frame));

        let now = Instant::from_millis(11_000);
        let mut got = false;
        while let Some(ev) = driver.poll_at(now).unwrap() {
            if matches!(ev, Event::Diag(DiagEvent::Dm1Received { source, .. }) if source == 0x20) {
                got = true;
            }
        }
        assert!(got, "peer DM1 fed via transport must surface as an event");
    }

    #[test]
    fn callbacks_fire_via_pump_and_unsubscribe_on_drop() {
        use crate::session::sys::ClaimEvent;

        let mem = MemTransport::default();
        let (ctrl, mut driver) = Session::builder(test_name(4), 0x80)
            .plug(Diagnostics::every(1000))
            .spawn(mem)
            .unwrap();
        ctrl.start().unwrap();

        let claimed = Rc::new(RefCell::new(0u32));
        let seen = claimed.clone();
        let sub = driver.on::<ClaimEvent>(move |_| *seen.borrow_mut() += 1);

        let mut now = Instant::ZERO;
        for _ in 0..40 {
            now = now.add_millis(100);
            driver.pump_at(now).unwrap();
            if ctrl.is_claimed() {
                break;
            }
        }
        assert!(ctrl.is_claimed());
        assert_eq!(
            *claimed.borrow(),
            1,
            "claim callback must fire exactly once"
        );

        // Drop the subscription; further events must not reach the callback.
        drop(sub);
        let before = *claimed.borrow();
        for _ in 0..5 {
            now = now.add_millis(100);
            driver.pump_at(now).unwrap();
        }
        assert_eq!(
            *claimed.borrow(),
            before,
            "dropped subscription must not fire"
        );
    }
}
