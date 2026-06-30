//! Embedded-shaped session loop.
//!
//! Checked with:
//!
//! ```sh
//! cargo check --no-default-features --features embedded --example embedded_session_loop
//! ```
//!
//! The example binary itself uses `std` so it can run as a normal host example,
//! but the `machbus` crate is compiled as `no_std + alloc`. The loop shape is
//! the same one an MCU application would use: board-owned clock, board-owned
//! CAN RX/TX, and an explicitly polled protocol/session core.

use std::collections::VecDeque;

use machbus::geo::Wgs;
use machbus::isobus::{
    AuxFunctionState, AuxFunctionType, AuxNFunction, CurvatureCommandStatus, Functionalities,
    GroupFunctionMsg, GuidanceSystemCmd, PtoState, TimOption, TimOptionSet,
};
use machbus::net::{
    BROADCAST_ADDRESS, Frame, Identifier, Name, Priority, hash_to_version, parse_iop_data,
    pgn_defs::PGN_REQUEST,
};
use machbus::nmea::{GNSSPosition, NMEAInterface};
use machbus::session::{Event, Session, Transport};
use machbus::time::Instant;
use machbus::vt_storage::StoredPoolVersion;

#[derive(Default)]
struct MockCan {
    rx: VecDeque<(u8, Frame)>,
    tx: Vec<(u8, Frame)>,
}

impl MockCan {
    fn inject(&mut self, port: u8, frame: Frame) {
        self.rx.push_back((port, frame));
    }
}

impl Transport for MockCan {
    type Error = machbus::net::Error;

    fn recv(&mut self) -> Option<(u8, Frame)> {
        self.rx.pop_front()
    }

    fn send(&mut self, port: u8, frame: &Frame) -> machbus::net::Result<()> {
        self.tx.push((port, *frame));
        Ok(())
    }
}

fn local_name() -> Name {
    Name::default()
        .with_identity_number(0x12345)
        .with_function_code(0x80)
        .with_self_configurable(true)
}

fn request_for_address_claim() -> Frame {
    let payload = [0x00, 0xEE, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    Frame::new(
        Identifier::encode(Priority::Default, PGN_REQUEST, 0x20, BROADCAST_ADDRESS),
        payload,
        8,
    )
}

#[cfg(feature = "embedded")]
fn print_claim_event(event: Event) {
    if let Event::AddressClaimed { address } = event {
        println!("claimed 0x{address:02X}");
    }
}

#[cfg(feature = "default")]
fn print_claim_event(event: Event) {
    if let Event::AddressClaim(machbus::session::ClaimEvent::Claimed { address }) = event {
        println!("claimed 0x{address:02X}");
    }
}

fn main() -> machbus::net::Result<()> {
    let mut session = Session::builder(local_name(), 0x80).build()?;
    let mut can = MockCan::default();

    session.start()?;

    let gnss = GNSSPosition {
        wgs: Wgs::new(52.0, 5.0, 0.0),
        ..Default::default()
    };
    let position_payload = NMEAInterface::build_position(&gnss);
    println!(
        "embedded NMEA position payload is {} bytes",
        position_payload.len()
    );

    let iop = [0x01, 0x00, 21, 0x0D, 0xF0, 0xFE, 0xCA];
    let iop_objects = parse_iop_data(&iop)?;
    println!(
        "embedded IOP parser saw {} object(s), version {}",
        iop_objects.len(),
        hash_to_version(&iop)
    );

    let mut stored_pool = StoredPoolVersion {
        label: "POOL01".into(),
        pool_data: iop.to_vec(),
        ..Default::default()
    };
    stored_pool.update_metadata_at(6, 0);
    let stored_pool_bytes = stored_pool.to_storage_bytes().expect("valid VT pool blob");
    assert_eq!(
        StoredPoolVersion::from_storage_bytes(&stored_pool_bytes)
            .expect("decode VT pool blob")
            .label,
        "POOL01"
    );
    println!(
        "embedded VT stored-pool blob is {} bytes",
        stored_pool_bytes.len()
    );

    let aux = AuxNFunction {
        function_number: 1,
        r#type: AuxFunctionType::Type1,
        state: AuxFunctionState::Variable,
        setpoint: 42,
    };
    assert_eq!(
        AuxNFunction::decode(&machbus::net::Message::new(
            0xFDD4,
            aux.encode().to_vec(),
            0x20,
        )),
        Some(aux)
    );

    let guidance = GuidanceSystemCmd {
        commanded_curvature: 0.25,
        status: CurvatureCommandStatus::IntendedToSteer,
    };
    assert_eq!(guidance.encode().len(), 8);

    let funcs = Functionalities::new().with_min_cf(1).with_tim_client(1);
    let function_payload = funcs.serialize();
    assert!(!Functionalities::decode(&function_payload)?.is_empty());

    let group =
        GroupFunctionMsg::acknowledge(PGN_REQUEST, machbus::isobus::GroupFunctionError::NoError);
    assert_eq!(GroupFunctionMsg::decode(&group.encode()?), Some(group));

    let tim_options = TimOptionSet::from_options(&[TimOption::RearPtoEngagementCwIsSupported]);
    assert!(tim_options.contains(TimOption::RearPtoEngagementCwIsSupported));
    assert!(PtoState::default().encode()[0] != 0);

    let mut now = Instant::ZERO;
    for _ in 0..4 {
        now = now.add_millis(100);

        while let Some((port, frame)) = can.recv() {
            session.feed(port, &frame, now);
        }

        session.tick(now);

        while let Some((port, frame)) = session.poll_transmit() {
            can.send(port, &frame)?;
        }

        while let Some(event) = session.poll_event() {
            print_claim_event(event);
        }
    }

    can.inject(0, request_for_address_claim());
    while let Some((port, frame)) = can.recv() {
        session.feed(port, &frame, now);
    }
    while let Some((port, frame)) = session.poll_transmit() {
        can.send(port, &frame)?;
    }

    println!("queued {} transmitted CAN frames", can.tx.len());
    Ok(())
}
