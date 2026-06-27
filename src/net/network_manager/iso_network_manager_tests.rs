#[cfg(test)]
mod tests {
    use super::super::{ErrorCode, NameFilterField, NiuConfig, Router, Side};
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use wirebit::topology::Topology;

    fn route_router_frames(
        router: &mut Router,
        tractor_ep: &mut wirebit::CanEndpoint<wirebit::ShmLink>,
        implement_ep: &mut wirebit::CanEndpoint<wirebit::ShmLink>,
        now_ms: u32,
    ) -> usize {
        let mut routed = 0usize;
        while let Ok(cf) = tractor_ep.recv_can() {
            if let Some(frame) = Frame::from_can_frame(&cf)
                && let Some(out) = router.process_frame(frame, Side::Tractor, now_ms)
            {
                implement_ep.send_can(&out.to_can_frame()).expect("route");
                routed += 1;
            }
        }
        while let Ok(cf) = implement_ep.recv_can() {
            if let Some(frame) = Frame::from_can_frame(&cf)
                && let Some(out) = router.process_frame(frame, Side::Implement, now_ms)
            {
                tractor_ep.send_can(&out.to_can_frame()).expect("route");
                routed += 1;
            }
        }
        routed
    }

    struct RouterPump<'a> {
        router: &'a mut Router,
        tractor_ep: &'a mut wirebit::CanEndpoint<wirebit::ShmLink>,
        implement_ep: &'a mut wirebit::CanEndpoint<wirebit::ShmLink>,
        built: &'a mut wirebit::topology::Built,
    }

    impl RouterPump<'_> {
        fn exchange(
            &mut self,
            tractor: &mut IsoNet<wirebit::ShmLink>,
            implement: &mut IsoNet<wirebit::ShmLink>,
            now_ms: u32,
        ) {
            for _ in 0..4 {
                self.built.pump_all().expect("pump");
                route_router_frames(self.router, self.tractor_ep, self.implement_ep, now_ms);
                self.built.pump_all().expect("pump");
                tractor.update(0);
                implement.update(0);
            }
        }

        fn pump_until<F>(
            &mut self,
            tractor: &mut IsoNet<wirebit::ShmLink>,
            implement: &mut IsoNet<wirebit::ShmLink>,
            max_ticks: u32,
            tick_ms: u32,
            mut done: F,
        ) -> u32
        where
            F: FnMut(&IsoNet<wirebit::ShmLink>, &IsoNet<wirebit::ShmLink>) -> bool,
        {
            let mut now_ms = 0u32;
            for tick in 0..max_ticks {
                now_ms = now_ms.wrapping_add(tick_ms);
                tractor.update(tick_ms);
                implement.update(tick_ms);
                self.exchange(tractor, implement, now_ms);
                if done(tractor, implement) {
                    return tick;
                }
            }
            max_ticks
        }
    }

    fn name_with(identity: u32, function_code: u8) -> Name {
        name_with_config(identity, function_code, true)
    }

    fn name_with_config(identity: u32, function_code: u8, self_configurable: bool) -> Name {
        Name::default()
            .with_identity_number(identity)
            .with_function_code(function_code)
            .with_self_configurable(self_configurable)
    }

    fn request_address_claim_frame(src: Address, dst: Address) -> Frame {
        let mut data = [0xFFu8; 8];
        data[0] = (PGN_ADDRESS_CLAIMED & 0xFF) as u8;
        data[1] = ((PGN_ADDRESS_CLAIMED >> 8) & 0xFF) as u8;
        data[2] = ((PGN_ADDRESS_CLAIMED >> 16) & 0xFF) as u8;
        Frame::new(
            Identifier::encode(Priority::Default, PGN_REQUEST, src, dst),
            data,
            8,
        )
    }

    fn malformed_request_address_claim_frame_bad_tail(src: Address, dst: Address) -> Frame {
        let mut data = [0xFFu8; 8];
        data[0] = (PGN_ADDRESS_CLAIMED & 0xFF) as u8;
        data[1] = ((PGN_ADDRESS_CLAIMED >> 8) & 0xFF) as u8;
        data[2] = ((PGN_ADDRESS_CLAIMED >> 16) & 0xFF) as u8;
        data[3] = 0x00;
        Frame::new(
            Identifier::encode(Priority::Default, PGN_REQUEST, src, dst),
            data,
            8,
        )
    }

    fn address_claim_frame(name: Name, src: Address) -> Frame {
        Frame::new(
            Identifier::encode(
                Priority::Default,
                PGN_ADDRESS_CLAIMED,
                src,
                BROADCAST_ADDRESS,
            ),
            name.to_bytes(),
            8,
        )
    }

    /// Drive both nodes' update() until the closure says stop, with a
    /// hard cap on iterations.
    fn pump_until<F>(
        a: &mut IsoNet<wirebit::ShmLink>,
        b: &mut IsoNet<wirebit::ShmLink>,
        built: &mut wirebit::topology::Built,
        max_ticks: u32,
        tick_ms: u32,
        mut done: F,
    ) -> u32
    where
        F: FnMut(&IsoNet<wirebit::ShmLink>, &IsoNet<wirebit::ShmLink>) -> bool,
    {
        for tick in 0..max_ticks {
            a.update(tick_ms);
            b.update(tick_ms);
            built.pump_all().expect("pump");
            // run again so each side sees what the other sent this tick
            a.update(0);
            b.update(0);
            built.pump_all().expect("pump");
            if done(a, b) {
                return tick;
            }
        }
        max_ticks
    }

    #[test]
    fn two_nodes_claim_distinct_addresses() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0")
            .members(&[n1, n2])
            .config(wirebit::can::CanConfig::default());
        let mut built = topo.build().expect("build");

        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let h_a = net_a
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();
        let h_b = net_b
            .create_internal(name_with(0x999, 0x80), 0, 0x80)
            .unwrap();

        net_a.start_address_claiming().unwrap();
        net_b.start_address_claiming().unwrap();

        let ticks = pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, b| {
            a.internal_cfs[0].claim_state() == ClaimState::Claimed
                && b.internal_cfs[0].claim_state() == ClaimState::Claimed
        });
        assert!(ticks < 49, "claims did not converge");

        let addr_a = net_a.internal_cf(h_a).unwrap().address();
        let addr_b = net_b.internal_cf(h_b).unwrap().address();
        assert_ne!(addr_a, addr_b, "two nodes must claim distinct addresses");
        // Lower-NAME node (n1, identity 0x100) wins the preferred 0x80.
        assert_eq!(addr_a, 0x80);
    }

    #[test]
    fn bus_state_reports_endpoint_confinement_state() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().expect("build");
        let ep = built
            .can_bus_mut("bus0")
            .unwrap()
            .take_endpoint("n1")
            .unwrap();

        let mut net = IsoNet::new(NetworkConfig::default());
        net.set_endpoint(0, ep);
        // A fresh, error-free controller is error-active.
        assert_eq!(net.bus_state(0), Some(wirebit::can::BusState::ErrorActive));
        // An unconnected port has no state.
        assert_eq!(net.bus_state(7), None);
    }

    #[test]
    fn network_statistics_count_sent_and_received_frames() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().expect("build");
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);
        net_a
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();
        assert_eq!(net_a.statistics(), NetworkStatistics::default());
        net_a.start_address_claiming().unwrap();

        pump_until(&mut net_a, &mut net_b, &mut built, 30, 50, |a, _b| {
            a.internal_cfs[0].claim_state() == ClaimState::Claimed
        });

        // net_a transmitted its address claim(s); net_b received them.
        assert!(net_a.statistics().frames_sent > 0);
        assert!(net_b.statistics().frames_received > 0);
    }

    #[test]
    fn send_before_address_claim_is_rejected() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        topo.can_bus("bus0").members(&[n1]);
        let mut built = topo.build().expect("build");
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep = bus.take_endpoint("n1").unwrap();

        let mut net = IsoNet::new(NetworkConfig::default());
        net.set_endpoint(0, ep);
        let h = net
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();

        let err = net
            .send(
                0xEF00,
                &[0xDE, 0xAD, 0xBE],
                h,
                BROADCAST_ADDRESS,
                Priority::Default,
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidState);
        assert!(err.message.contains("not claimed"));
    }

    #[test]
    fn send_rejects_invalid_pgn_before_identifier_normalization() {
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        let h = net
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();

        let err = net
            .send(
                0x4_0000,
                &[0xDE, 0xAD, 0xBE],
                h,
                BROADCAST_ADDRESS,
                Priority::Default,
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidPgn);
        assert!(err.message.contains("0x40000"));
    }

    #[test]
    fn fast_packet_registration_rejects_invalid_pgn() {
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        assert!(net.register_fast_packet_pgn(0x1_FF00).is_ok());

        let err = net.register_fast_packet_pgn(0x4_0000).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidPgn);
        assert_eq!(net.fast_packet_pgns, vec![0x1_FF00]);
    }

    #[test]
    fn pgn_callback_registration_rejects_invalid_pgn() {
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        assert!(net.register_pgn_callback(0x1_FF00, |_| {}).is_ok());

        let err = net.register_pgn_callback(0x4_0000, |_| {}).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidPgn);
        assert!(net.pgn_callbacks.contains_key(&0x1_FF00));
        assert!(!net.pgn_callbacks.contains_key(&0x4_0000));
    }

    #[test]
    fn sans_io_seam_feeds_inbound_and_buffers_outbound_without_endpoints() {
        // Sans-IO (Phase 1): a link-less IsoNet routes fed frames and buffers
        // outbound frames, proving the core works with no CanEndpoint attached.
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        net.set_capture_outbound(true);
        assert!(net.is_capturing_outbound());

        // Inbound: a fed frame reaches the PGN callback registry with no endpoint.
        let test_pgn: Pgn = 0x1_FF00;
        let hits = Rc::new(RefCell::new(0u32));
        let h = hits.clone();
        net.register_pgn_callback(test_pgn, move |_m| *h.borrow_mut() += 1)
            .unwrap();

        let frame = Frame::new(
            Identifier::encode(Priority::Default, test_pgn, 0x20, BROADCAST_ADDRESS),
            [1, 2, 3, 4, 5, 6, 7, 8],
            8,
        );
        net.feed(&frame, 0);
        assert_eq!(
            *hits.borrow(),
            1,
            "fed frame must route without an endpoint"
        );
        assert_eq!(net.statistics().frames_received, 1);

        // Outbound: send_frame buffers instead of requiring an endpoint.
        assert!(net.take_outbound().is_none());
        net.send_frame(&frame, 0)
            .expect("send_frame must buffer in capture mode without an endpoint");
        assert_eq!(net.outbound_len(), 1);
        let (port, out) = net.take_outbound().expect("buffered outbound frame");
        assert_eq!(port, 0);
        assert_eq!(out.pgn(), test_pgn);
        assert!(net.take_outbound().is_none());
    }

    #[test]
    fn send_frame_still_errors_without_endpoint_in_default_mode() {
        // The seam is opt-in: default mode preserves the existing behavior.
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        let frame = Frame::new(
            Identifier::encode(Priority::Default, 0x1_FF00, 0x20, BROADCAST_ADDRESS),
            [0xFF; 8],
            8,
        );
        assert!(net.send_frame(&frame, 0).is_err());
        assert_eq!(net.outbound_len(), 0);
    }

    #[test]
    fn create_internal_rejects_duplicate_name() {
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        let name = name_with(0x100, 0x80);

        let first = net.create_internal(name, 0, 0x80).unwrap();
        let err = net.create_internal(name, 0, 0x81).unwrap_err();

        assert_eq!(net.internal_cf(first).unwrap().address(), 0x80);
        assert_eq!(err.code, ErrorCode::AddressConflict);
        assert!(err.message.contains("duplicate internal NAME"));
    }

    #[test]
    fn duplicate_name_claim_at_different_address_fails_local_cf() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let duplicate_name = name_with(0x100, 0x80);
        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let h_a = net_a.create_internal(duplicate_name, 0, 0x80).unwrap();
        let duplicates = Rc::new(RefCell::new(Vec::<(Name, Address)>::new()));
        let seen = duplicates.clone();
        net_a
            .on_duplicate_name
            .subscribe(move |event| seen.borrow_mut().push(*event));

        net_a.start_address_claiming().unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, _b| {
            a.internal_cf(h_a).unwrap().claim_state() == ClaimState::Claimed
        });
        assert_eq!(net_a.internal_cf(h_a).unwrap().address(), 0x80);

        net_b
            .send_frame(&address_claim_frame(duplicate_name, 0x81), 0)
            .unwrap();
        let ticks = pump_until(&mut net_a, &mut net_b, &mut built, 20, 10, |a, _b| {
            a.internal_cf(h_a).unwrap().claim_state() == ClaimState::Failed
        });

        assert!(ticks < 19, "duplicate-NAME claim was not detected");
        let local = net_a.internal_cf(h_a).unwrap();
        assert_eq!(local.address(), NULL_ADDRESS);
        assert!(!local.cf().is_online());
        assert_eq!(&*duplicates.borrow(), &[(duplicate_name, 0x81)]);
    }

    #[test]
    fn global_request_for_address_claim_gets_claim_response() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let claimed_name = name_with(0x100, 0x80);
        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let _h_a = net_a.create_internal(claimed_name, 0, 0x80).unwrap();
        net_a.start_address_claiming().unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, _b| {
            a.internal_cfs[0].claim_state() == ClaimState::Claimed
        });

        let partner = net_b
            .create_partner(
                0,
                vec![NameFilter::new(NameFilterField::IdentityNumber, 0x100)],
            )
            .unwrap();
        let malformed_request =
            malformed_request_address_claim_frame_bad_tail(NULL_ADDRESS, BROADCAST_ADDRESS);
        net_b.send_frame(&malformed_request, 0).unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 10, 10, |_a, _b| false);
        assert_eq!(net_b.partner_cf(partner).unwrap().address(), NULL_ADDRESS);
        assert!(!net_b.partner_cf(partner).unwrap().cf().is_online());

        let request = request_address_claim_frame(NULL_ADDRESS, BROADCAST_ADDRESS);
        net_b.send_frame(&request, 0).unwrap();

        let ticks = pump_until(&mut net_a, &mut net_b, &mut built, 20, 10, |_a, b| {
            b.partner_cf(partner).unwrap().address() == 0x80
        });
        assert!(ticks < 19, "address-claim response did not arrive");

        let partner_cf = net_b.partner_cf(partner).unwrap();
        assert_eq!(partner_cf.address(), 0x80);
        assert_eq!(partner_cf.name(), claimed_name);
        assert!(partner_cf.cf().is_online());
    }

    #[test]
    fn specific_request_for_address_claim_only_matching_destination_responds() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let claimed_name = name_with(0x100, 0x80);
        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let _h_a = net_a.create_internal(claimed_name, 0, 0x80).unwrap();
        net_a.start_address_claiming().unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, _b| {
            a.internal_cfs[0].claim_state() == ClaimState::Claimed
        });

        let partner = net_b
            .create_partner(
                0,
                vec![NameFilter::new(NameFilterField::IdentityNumber, 0x100)],
            )
            .unwrap();

        let wrong_destination = request_address_claim_frame(NULL_ADDRESS, 0x81);
        net_b.send_frame(&wrong_destination, 0).unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 10, 10, |_a, _b| false);
        assert_eq!(net_b.partner_cf(partner).unwrap().address(), NULL_ADDRESS);
        assert!(!net_b.partner_cf(partner).unwrap().cf().is_online());

        let matching_destination = request_address_claim_frame(NULL_ADDRESS, 0x80);
        net_b.send_frame(&matching_destination, 0).unwrap();
        let ticks = pump_until(&mut net_a, &mut net_b, &mut built, 20, 10, |_a, b| {
            b.partner_cf(partner).unwrap().address() == 0x80
        });
        assert!(ticks < 19, "specific address-claim response did not arrive");

        let partner_cf = net_b.partner_cf(partner).unwrap();
        assert_eq!(partner_cf.address(), 0x80);
        assert_eq!(partner_cf.name(), claimed_name);
        assert!(partner_cf.cf().is_online());
    }

    #[test]
    fn non_claim_frame_from_claimed_source_reports_address_violation() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let _h_a = net_a
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();
        net_a.start_address_claiming().unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, _b| {
            a.internal_cfs[0].claim_state() == ClaimState::Claimed
        });

        let violations = Rc::new(RefCell::new(Vec::<Address>::new()));
        let seen = violations.clone();
        net_a
            .on_address_violation
            .subscribe(move |&addr| seen.borrow_mut().push(addr));

        let frame = Frame::new(
            Identifier::encode(Priority::Default, 0xEF00, 0x80, BROADCAST_ADDRESS),
            [0xAA, 0x55, 0xCC, 0x33, 0xFF, 0xFF, 0xFF, 0xFF],
            8,
        );
        net_b.send_frame(&frame, 0).unwrap();

        let ticks = pump_until(&mut net_a, &mut net_b, &mut built, 10, 10, |_a, _b| {
            !violations.borrow().is_empty()
        });
        assert!(ticks < 9, "address violation was not reported");
        assert_eq!(&*violations.borrow(), &[0x80]);
    }

    #[test]
    fn restart_address_claiming_after_claimed_keeps_address_stable() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let claimed_name = name_with(0x100, 0x80);
        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let h_a = net_a.create_internal(claimed_name, 0, 0x80).unwrap();
        net_a.start_address_claiming().unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, _b| {
            a.internal_cf(h_a).unwrap().claim_state() == ClaimState::Claimed
        });
        assert_eq!(net_a.internal_cf(h_a).unwrap().address(), 0x80);

        net_a.start_address_claiming().unwrap();
        assert_eq!(
            net_a.internal_cf(h_a).unwrap().claim_state(),
            ClaimState::WaitForContest
        );
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, _b| {
            a.internal_cf(h_a).unwrap().claim_state() == ClaimState::Claimed
        });

        let icf = net_a.internal_cf(h_a).unwrap();
        assert_eq!(icf.address(), 0x80);
        assert_eq!(icf.name(), claimed_name);
        assert!(icf.cf().is_online());
    }

    #[test]
    fn non_self_configurable_loser_fails_with_cannot_claim() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let h_winner = net_a
            .create_internal(name_with_config(0x100, 0x80, false), 0, 0x80)
            .unwrap();
        let h_loser = net_b
            .create_internal(name_with_config(0x999, 0x80, false), 0, 0x80)
            .unwrap();

        net_a.start_address_claiming().unwrap();
        net_b.start_address_claiming().unwrap();

        let ticks = pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, b| {
            a.internal_cf(h_winner).unwrap().claim_state() == ClaimState::Claimed
                && b.internal_cf(h_loser).unwrap().claim_state() == ClaimState::Failed
        });
        assert!(ticks < 49, "non-self-configurable loser did not fail");

        assert_eq!(net_a.internal_cf(h_winner).unwrap().address(), 0x80);
        assert_eq!(net_b.internal_cf(h_loser).unwrap().address(), NULL_ADDRESS);
    }

    #[test]
    fn claimed_node_loses_to_later_lower_name_and_reclaims() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_high = bus.take_endpoint("n1").unwrap();
        let ep_low = bus.take_endpoint("n2").unwrap();

        let mut high = IsoNet::new(NetworkConfig::default());
        let mut low = IsoNet::new(NetworkConfig::default());
        high.set_endpoint(0, ep_high);
        low.set_endpoint(0, ep_low);

        let h_high = high
            .create_internal(name_with(0x999, 0x80), 0, 0x80)
            .unwrap();
        let h_low = low
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();

        high.start_address_claiming().unwrap();
        pump_until(&mut high, &mut low, &mut built, 50, 100, |a, _b| {
            a.internal_cf(h_high).unwrap().claim_state() == ClaimState::Claimed
        });
        assert_eq!(high.internal_cf(h_high).unwrap().address(), 0x80);

        low.start_address_claiming().unwrap();
        let ticks = pump_until(&mut high, &mut low, &mut built, 50, 100, |a, b| {
            a.internal_cf(h_high).unwrap().claim_state() == ClaimState::Claimed
                && b.internal_cf(h_low).unwrap().claim_state() == ClaimState::Claimed
                && a.internal_cf(h_high).unwrap().address()
                    != b.internal_cf(h_low).unwrap().address()
        });
        assert!(ticks < 49, "later lower-NAME claim did not converge");

        assert_eq!(low.internal_cf(h_low).unwrap().address(), 0x80);
        assert_eq!(high.internal_cf(h_high).unwrap().address(), 0x81);
    }

    #[test]
    fn single_frame_pgn_round_trip() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let h_a = net_a
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();
        let _h_b = net_b
            .create_internal(name_with(0x999, 0x80), 0, 0x81)
            .unwrap();

        net_a.start_address_claiming().unwrap();
        net_b.start_address_claiming().unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, b| {
            a.internal_cfs[0].claim_state() == ClaimState::Claimed
                && b.internal_cfs[0].claim_state() == ClaimState::Claimed
        });

        // B subscribes to a custom PGN.
        let received = Rc::new(RefCell::new(Vec::<Message>::new()));
        let r = received.clone();
        net_b
            .register_pgn_callback(0xEF00, move |m| r.borrow_mut().push(m.clone()))
            .unwrap();

        // A sends a single-frame message to B.
        let dst = net_b.internal_cf(_h_b).unwrap().address();
        net_a
            .send(0xEF00, &[0xDE, 0xAD, 0xBE], h_a, dst, Priority::Default)
            .unwrap();

        pump_until(&mut net_a, &mut net_b, &mut built, 10, 10, |_a, _b| {
            !received.borrow().is_empty()
        });
        let msgs = received.borrow();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].pgn, 0xEF00);
        assert_eq!(msgs[0].source, 0x80);
        assert_eq!(msgs[0].destination, dst);
        assert_eq!(&msgs[0].data[..3], &[0xDE, 0xAD, 0xBE]);
    }

    #[test]
    fn tp_round_trip_through_isonet() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let h_a = net_a
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();
        let h_b = net_b
            .create_internal(name_with(0x999, 0x80), 0, 0x81)
            .unwrap();

        net_a.start_address_claiming().unwrap();
        net_b.start_address_claiming().unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, b| {
            a.internal_cfs[0].claim_state() == ClaimState::Claimed
                && b.internal_cfs[0].claim_state() == ClaimState::Claimed
        });

        // A 50-byte payload triggers TP CMDT.
        let payload: Vec<u8> = (0..50u32).map(|n| n as u8).collect();
        let received = Rc::new(RefCell::new(Vec::<Message>::new()));
        let r = received.clone();
        net_b
            .register_pgn_callback(0xEF11, move |m| r.borrow_mut().push(m.clone()))
            .unwrap();

        let dst = net_b.internal_cf(h_b).unwrap().address();
        net_a
            .send(0xEF11, &payload, h_a, dst, Priority::Lowest)
            .unwrap();

        pump_until(&mut net_a, &mut net_b, &mut built, 100, 50, |_a, _b| {
            !received.borrow().is_empty()
        });

        let msgs = received.borrow();
        assert_eq!(msgs.len(), 1, "expected one TP-reassembled message");
        assert_eq!(msgs[0].pgn, 0xEF11);
        assert_eq!(msgs[0].data, payload);
    }

    #[test]
    fn isonet_queues_tp_transmits_that_share_dt_endpoint_path() {
        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let mut net_a = IsoNet::new(NetworkConfig::default());
        let mut net_b = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let h_a = net_a
            .create_internal(name_with(0x100, 0x80), 0, 0x80)
            .unwrap();
        let h_b = net_b
            .create_internal(name_with(0x999, 0x80), 0, 0x81)
            .unwrap();

        net_a.start_address_claiming().unwrap();
        net_b.start_address_claiming().unwrap();
        pump_until(&mut net_a, &mut net_b, &mut built, 50, 100, |a, b| {
            a.internal_cfs[0].claim_state() == ClaimState::Claimed
                && b.internal_cfs[0].claim_state() == ClaimState::Claimed
        });

        let received = Rc::new(RefCell::new(Vec::<Message>::new()));
        let r = received.clone();
        net_b
            .register_pgn_callback(0xEF21, move |m| r.borrow_mut().push(m.clone()))
            .unwrap();
        let r = received.clone();
        net_b
            .register_pgn_callback(0xEF22, move |m| r.borrow_mut().push(m.clone()))
            .unwrap();

        let first: Vec<u8> = (0..50u32).map(|n| n as u8).collect();
        let second: Vec<u8> = (0..50u32).map(|n| 0x80 | n as u8).collect();
        let dst = net_b.internal_cf(h_b).unwrap().address();

        net_a
            .send(0xEF21, &first, h_a, dst, Priority::Default)
            .unwrap();
        net_a
            .send(0xEF22, &second, h_a, dst, Priority::Default)
            .unwrap();

        pump_until(&mut net_a, &mut net_b, &mut built, 200, 50, |_a, _b| {
            received.borrow().len() == 2
        });

        let msgs = received.borrow();
        assert_eq!(msgs.len(), 2, "both queued TP messages must be delivered");
        assert_eq!(msgs[0].pgn, 0xEF21);
        assert_eq!(msgs[0].data, first);
        assert_eq!(msgs[1].pgn, 0xEF22);
        assert_eq!(msgs[1].data, second);
    }

    #[test]
    fn routed_multibus_isonet_tp_and_etp_cross_router_translation() {
        let mut topo = Topology::new();
        let tractor_node = topo.add_node("tractor");
        let implement_node = topo.add_node("implement");
        let router_tractor_node = topo.add_node("router_tractor");
        let router_implement_node = topo.add_node("router_implement");
        topo.can_bus("tractor_bus")
            .members(&[tractor_node, router_tractor_node]);
        topo.can_bus("implement_bus")
            .members(&[implement_node, router_implement_node]);
        let mut built = topo.build().unwrap();

        let tractor_bus = built.can_bus_mut("tractor_bus").unwrap();
        let tractor_ep = tractor_bus.take_endpoint("tractor").unwrap();
        let mut router_tractor_ep = tractor_bus.take_endpoint("router_tractor").unwrap();

        let implement_bus = built.can_bus_mut("implement_bus").unwrap();
        let implement_ep = implement_bus.take_endpoint("implement").unwrap();
        let mut router_implement_ep = implement_bus.take_endpoint("router_implement").unwrap();

        let tractor_name = name_with(0x100, 0x80);
        let implement_name = name_with(0x200, 0x80);
        let mut tractor = IsoNet::new(NetworkConfig::default());
        let mut implement = IsoNet::new(NetworkConfig::default());
        tractor.set_endpoint(0, tractor_ep);
        implement.set_endpoint(0, implement_ep);

        let h_tractor = tractor.create_internal(tractor_name, 0, 0x10).unwrap();
        let h_implement = implement.create_internal(implement_name, 0, 0x30).unwrap();

        let mut router = Router::new(NiuConfig::default());
        router.niu_mut().start().unwrap();
        router.add_translation(tractor_name, 0x10, 0x20).unwrap();
        router.add_translation(implement_name, 0x40, 0x30).unwrap();

        let mut pump = RouterPump {
            router: &mut router,
            tractor_ep: &mut router_tractor_ep,
            implement_ep: &mut router_implement_ep,
            built: &mut built,
        };

        tractor.start_address_claiming().unwrap();
        implement.start_address_claiming().unwrap();
        let ticks = pump.pump_until(&mut tractor, &mut implement, 50, 100, |t, i| {
            t.internal_cf(h_tractor).unwrap().claim_state() == ClaimState::Claimed
                && i.internal_cf(h_implement).unwrap().claim_state() == ClaimState::Claimed
        });
        assert!(ticks < 49, "routed address claims did not converge");
        assert_eq!(tractor.internal_cf(h_tractor).unwrap().address(), 0x10);
        assert_eq!(implement.internal_cf(h_implement).unwrap().address(), 0x30);

        let received = Rc::new(RefCell::new(Vec::<Message>::new()));
        let seen = received.clone();
        implement
            .register_pgn_callback(0xEF22, move |m| seen.borrow_mut().push(m.clone()))
            .unwrap();
        let seen = received.clone();
        implement
            .register_pgn_callback(0xEF33, move |m| seen.borrow_mut().push(m.clone()))
            .unwrap();

        let tp_payload: Vec<u8> = (0..50u32)
            .map(|n| (n.wrapping_mul(3) & 0xFF) as u8)
            .collect();
        tractor
            .send(0xEF22, &tp_payload, h_tractor, 0x40, Priority::Lowest)
            .unwrap();
        let ticks = pump.pump_until(&mut tractor, &mut implement, 100, 50, |_t, _i| {
            received.borrow().iter().any(|m| m.pgn == 0xEF22)
        });
        assert!(ticks < 99, "routed TP message did not arrive");

        let etp_payload: Vec<u8> = (0..2000u32)
            .map(|n| (n.wrapping_mul(7).wrapping_add(11) & 0xFF) as u8)
            .collect();
        tractor
            .send(0xEF33, &etp_payload, h_tractor, 0x40, Priority::Lowest)
            .unwrap();
        let ticks = pump.pump_until(&mut tractor, &mut implement, 200, 50, |_t, _i| {
            received.borrow().iter().any(|m| m.pgn == 0xEF33)
        });
        assert!(ticks < 199, "routed ETP message did not arrive");

        let msgs = received.borrow();
        let tp = msgs.iter().find(|m| m.pgn == 0xEF22).expect("TP message");
        assert_eq!(tp.source, 0x20);
        assert_eq!(tp.destination, 0x30);
        assert_eq!(tp.data, tp_payload);

        let etp = msgs.iter().find(|m| m.pgn == 0xEF33).expect("ETP message");
        assert_eq!(etp.source, 0x20);
        assert_eq!(etp.destination, 0x30);
        assert_eq!(etp.data, etp_payload);
    }

    #[test]
    fn create_internal_returns_distinct_handles() {
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        let h1 = net
            .create_internal(name_with(0x100, 0x80), 0, 0x10)
            .unwrap();
        let h2 = net
            .create_internal(name_with(0x101, 0x80), 0, 0x11)
            .unwrap();
        assert_ne!(h1, h2);
        assert_eq!(net.internal_cf(h1).unwrap().address(), 0x10);
        assert_eq!(net.internal_cf(h2).unwrap().address(), 0x11);
    }

    #[test]
    fn start_with_no_cfs_errors() {
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        assert!(net.start_address_claiming().is_err());
    }

    #[test]
    fn inject_message_dispatches_to_callbacks() {
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        net.register_pgn_callback(0xCAFE, move |_m| *c.borrow_mut() += 1)
            .unwrap();

        let msg = Message::new(0xCAFE, vec![1, 2, 3], 0x10);
        net.inject_message(&msg);
        net.inject_message(&msg);
        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn pgn_callback_fanout_is_isolated_by_pgn() {
        let mut net = IsoNet::<wirebit::ShmLink>::new(NetworkConfig::default());
        let all_messages = Rc::new(RefCell::new(Vec::<Pgn>::new()));
        let pgn_a_first = Rc::new(RefCell::new(0u32));
        let pgn_a_second = Rc::new(RefCell::new(0u32));
        let pgn_b = Rc::new(RefCell::new(0u32));

        let all = all_messages.clone();
        net.on_message
            .subscribe(move |m| all.borrow_mut().push(m.pgn));

        let a1 = pgn_a_first.clone();
        net.register_pgn_callback(0xCAFE, move |_m| *a1.borrow_mut() += 1)
            .unwrap();

        let a2 = pgn_a_second.clone();
        net.register_pgn_callback(0xCAFE, move |_m| *a2.borrow_mut() += 1)
            .unwrap();

        let b = pgn_b.clone();
        net.register_pgn_callback(0xBEEF, move |_m| *b.borrow_mut() += 1)
            .unwrap();

        net.inject_message(&Message::new(0xCAFE, vec![1], 0x10));
        net.inject_message(&Message::new(0xBEEF, vec![2], 0x11));

        assert_eq!(&*all_messages.borrow(), &[0xCAFE, 0xBEEF]);
        assert_eq!(*pgn_a_first.borrow(), 1);
        assert_eq!(*pgn_a_second.borrow(), 1);
        assert_eq!(*pgn_b.borrow(), 1);
    }
}
