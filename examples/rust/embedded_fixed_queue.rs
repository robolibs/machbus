//! Embedded fixed-capacity transport queue sketch.
//!
//! Checked with:
//!
//! ```sh
//! cargo check --no-default-features --features embedded --example embedded_fixed_queue
//! ```
//!
//! The session core is still the heap-backed `embedded` profile, but the board
//! transport buffers here are fixed-capacity and allocation-free. This is the
//! first narrow step toward the later fixed-capacity profile in `PLAN.md`.

use machbus::fixed::{FixedFrameQueue, FixedMessage};
use machbus::net::{
    BROADCAST_ADDRESS, Error, EtpCmdtTx, EtpRxFixed, Frame, Identifier, Name, Priority, TpRxFixed,
    TransportProtocol,
};
use machbus::session::{Session, Transport};
use machbus::time::Instant;

struct FixedCan<const RX: usize, const TX: usize> {
    rx: FixedFrameQueue<RX>,
    tx: FixedFrameQueue<TX>,
}

impl<const RX: usize, const TX: usize> Default for FixedCan<RX, TX> {
    fn default() -> Self {
        Self {
            rx: FixedFrameQueue::new(),
            tx: FixedFrameQueue::new(),
        }
    }
}

impl<const RX: usize, const TX: usize> FixedCan<RX, TX> {
    fn inject(&mut self, port: u8, frame: Frame) -> Result<(), (u8, Frame)> {
        self.rx.push_back((port, frame))
    }

    fn transmitted(&self) -> usize {
        self.tx.len()
    }
}

impl<const RX: usize, const TX: usize> Transport for FixedCan<RX, TX> {
    type Error = Error;

    fn recv(&mut self) -> Option<(u8, Frame)> {
        self.rx.pop_front()
    }

    fn send(&mut self, port: u8, frame: &Frame) -> machbus::net::Result<()> {
        self.tx
            .push_back((port, *frame))
            .map_err(|_| Error::invalid_state("fixed TX queue is full"))
    }
}

fn local_name() -> Name {
    Name::default()
        .with_identity_number(0x45678)
        .with_function_code(0x80)
        .with_self_configurable(true)
}

fn request_for_address_claim() -> Frame {
    Frame::new(
        Identifier::encode(
            Priority::Default,
            machbus::net::pgn_defs::PGN_REQUEST,
            0x20,
            BROADCAST_ADDRESS,
        ),
        [0x00, 0xEE, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        8,
    )
}

fn main() -> machbus::net::Result<()> {
    let mut session = Session::builder(local_name(), 0x80).build()?;
    let mut can = FixedCan::<4, 8>::default();
    let mut now = Instant::ZERO;

    session.start()?;
    can.inject(0, request_for_address_claim())
        .expect("RX queue has capacity");

    for _ in 0..4 {
        now = now.add_millis(100);

        while let Some((port, frame)) = can.recv() {
            let msg = FixedMessage::<8>::from_frame(&frame).expect("single-frame request fits");
            assert_eq!(msg.to_frame()?.payload(), frame.payload());
            session.feed(port, &frame, now);
        }

        session.tick(now);

        while let Some((port, frame)) = session.poll_transmit() {
            can.send(port, &frame)?;
        }

        #[cfg(feature = "default")]
        while session.poll_event().is_some() {}
        #[cfg(feature = "embedded")]
        while session.poll_fixed_event::<8>().is_some() {}
    }

    let mut tp_tx = TransportProtocol::new();
    let bam = tp_tx.send_bam_fixed::<4>(
        machbus::net::pgn_defs::PGN_REQUEST,
        &[0x42; 20],
        0x80,
        Priority::Default,
    )?;
    let mut tp_rx = TpRxFixed::<32>::new();
    let mut completed = None;
    for frame in bam.iter() {
        let outcome = tp_rx.process_frame(frame)?;
        if let Some(response) = outcome.response {
            can.send(0, &response)?;
        }
        completed = outcome.message.or(completed);
    }
    let completed = completed.expect("fixed TP receiver completes BAM payload");
    assert_eq!(completed.data.as_slice(), &[0x42; 20]);

    let etp_payload = [0x24; 1792];
    let mut etp_tx = EtpCmdtTx::new(0xFECA, &etp_payload, 0x80, 0x90)?;
    let mut etp_rx = EtpRxFixed::<1792>::new();
    can.send(0, &etp_tx.rts())?;
    let accept = etp_rx.process_frame(&etp_tx.rts())?;
    assert!(accept.response.is_some());
    etp_tx.set_window(1, 2)?;
    for frame in etp_tx.pending_data_frames_fixed::<3>()?.iter() {
        let _ = etp_rx.process_frame(frame)?;
        can.send(0, frame)?;
    }

    println!(
        "fixed transport queued {} transmitted frame(s)",
        can.transmitted()
    );
    Ok(())
}
