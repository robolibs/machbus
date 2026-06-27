//! Typed per-subsystem event access (P5).
//!
//! The core produces one unified [`Event`] enum. [`SubsystemEvent`] lets callers
//! work with a single subsystem's events without matching the whole enum:
//! `session.drain::<VtEvent>()` returns just the VT events, and the driver's
//! callback layer (`on::<E>`) dispatches by type.

use crate::session::sys::{
    AuxiliaryEvent, BusEvent, ClaimEvent, DiagEvent, DmMemoryEvent, Event, FsEvent, FsServerEvent,
    GnssEvent, GuidanceEvent, HeartbeatEvent, ImplementEvent, LanguageCommandEvent,
    MaintainPowerEvent, PowertrainEvent, ScEvent, ShortcutButtonEvent, TcEvent, TcServerEvent,
    TimEvent, VtEvent, VtServerEvent,
};

/// A typed event carried inside the unified [`Event`] enum.
///
/// Implemented for every per-subsystem event type, so generic code can borrow
/// one subsystem's events from the unified stream (typed callbacks and
/// `drain::<E>()`).
pub trait SubsystemEvent: Sized {
    /// Borrow this typed event from a unified [`Event`] reference, or `None`
    /// when it belongs to another subsystem.
    fn try_ref(event: &Event) -> Option<&Self>;
}

macro_rules! subsystem_event {
    ($variant:ident, $ty:ty) => {
        impl SubsystemEvent for $ty {
            fn try_ref(event: &Event) -> Option<&Self> {
                match event {
                    Event::$variant(inner) => Some(inner),
                    _ => None,
                }
            }
        }
    };
}

subsystem_event!(AddressClaim, ClaimEvent);
subsystem_event!(Bus, BusEvent);
subsystem_event!(Diag, DiagEvent);
subsystem_event!(Gnss, GnssEvent);
subsystem_event!(Guidance, GuidanceEvent);
subsystem_event!(Vt, VtEvent);
subsystem_event!(Tc, TcEvent);
subsystem_event!(Fs, FsEvent);
subsystem_event!(Imp, ImplementEvent);
subsystem_event!(Sc, ScEvent);
subsystem_event!(Auxiliary, AuxiliaryEvent);
subsystem_event!(Tim, TimEvent);
subsystem_event!(ShortcutButton, ShortcutButtonEvent);
subsystem_event!(MaintainPower, MaintainPowerEvent);
subsystem_event!(Heartbeat, HeartbeatEvent);
subsystem_event!(LanguageCommand, LanguageCommandEvent);
subsystem_event!(Powertrain, PowertrainEvent);
subsystem_event!(DmMemory, DmMemoryEvent);
subsystem_event!(VtServer, VtServerEvent);
subsystem_event!(FsServer, FsServerEvent);
subsystem_event!(TcServer, TcServerEvent);
