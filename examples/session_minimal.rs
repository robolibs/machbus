//! Minimal demo of the **session facade** (`machbus::session`).
//!
//! Two control functions join one virtual bus through the new
//! `Session` / `Driver` / `Controls` / `Plugin` surface:
//!
//! - each is built with `Session::builder(...).plug(Diagnostics).spawn(transport)`,
//! - both claim an address (driven by `Driver::poll_at`),
//! - node A raises a DTC through `Controls::with_mut` (fine control on a plugin),
//! - the DM1 broadcast crosses the bus and surfaces on node B as a
//!   `DiagEvent::Dm1Received` event.
//!
//! Run with `cargo run --example session_minimal`.

use machbus::Instant;
use machbus::j1939::diagnostic::{Dtc, Fmi};
use machbus::net::{Name, Result};
use machbus::session::plugins::Diagnostics;
use machbus::session::{ClaimEvent, DiagEvent, Event};
use machbus::session::{EndpointTransport, Session};
use wirebit::topology::Topology;

// ANCHOR: name
fn make_name(identity: u32) -> Name {
    Name::default()
        .with_identity_number(identity)
        .with_function_code(0x80)
        .with_self_configurable(true)
}
// ANCHOR_END: name

fn main() -> Result<()> {
    println!("=== Session facade — minimal demo ===\n");

    // ─── A virtual bus with two endpoints ─────────────────────────
    let mut topo = Topology::new();
    let n1 = topo.add_node("a");
    let n2 = topo.add_node("b");
    topo.can_bus("bus0").members(&[n1, n2]);
    let mut built = topo.build().unwrap();
    let bus = built.can_bus_mut("bus0").unwrap();
    let ep_a = bus.take_endpoint("a").unwrap();
    let ep_b = bus.take_endpoint("b").unwrap();

    // ─── Build two sessions, split into (controls, driver) ────────
    // ANCHOR: build
    let (ctrl_a, mut drv_a) = Session::builder(make_name(0x100), 0x80)
        .plug(Diagnostics::every(1000))
        .spawn(EndpointTransport::new(0, ep_a))?;

    let (ctrl_b, mut drv_b) = Session::builder(make_name(0x999), 0x81)
        .plug(Diagnostics::every(1000))
        .spawn(EndpointTransport::new(0, ep_b))?;

    ctrl_a.start()?;
    ctrl_b.start()?;
    // ANCHOR_END: build

    // ─── Drive the claim handshake (capturing node A's events) ────
    // ANCHOR: claim
    let mut now = Instant::ZERO;
    let mut a_claims: Vec<ClaimEvent> = Vec::new();
    for _ in 0..40 {
        now = now.add_millis(50);
        while let Some(event) = drv_a.poll_at(now)? {
            if let Event::AddressClaim(claim) = event {
                a_claims.push(claim);
            }
        }
        while drv_b.poll_at(now)?.is_some() {}
        built.pump_all().unwrap();
        if ctrl_a.is_claimed() && ctrl_b.is_claimed() {
            break;
        }
    }
    // ANCHOR_END: claim
    println!(
        "[claim] node A → 0x{:02X}, node B → 0x{:02X}",
        ctrl_a.address(),
        ctrl_b.address()
    );
    println!(
        "[events] node A saw {} address-claim event(s)",
        a_claims.len()
    );

    // ─── Fine control: raise a DTC on node A's Diagnostics plugin ─
    // ANCHOR: finecontrol
    ctrl_a.with_mut::<Diagnostics, _>(|diag| {
        diag.raise(Dtc {
            spn: 1234,
            fmi: Fmi::BelowNormal,
            occurrence_count: 1,
        });
    });
    // ANCHOR_END: finecontrol

    // ─── Pump until node B sees the DM1 ───────────────────────────
    let mut received = false;
    for _ in 0..10 {
        now = now.add_millis(1000);
        while drv_a.poll_at(now)?.is_some() {}
        built.pump_all().unwrap();
        while let Some(event) = drv_b.poll_at(now)? {
            if let Event::Diag(DiagEvent::Dm1Received { source, active, .. }) = event {
                println!(
                    "[rx] node B received DM1 from 0x{source:02X} with {} active DTC(s)",
                    active.len()
                );
                received = true;
            }
        }
        if received {
            break;
        }
    }

    println!("\nDone.");
    Ok(())
}
