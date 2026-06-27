//! Full IsoNet stack on a simulated `wirebit::Topology` two-node bus,
//! with address-claim and TP CMDT round-trip. Mirrors
//! `virtual_can_demo.cpp`.

use std::cell::RefCell;
use std::rc::Rc;

use machbus::net::{ClaimState, IsoNet, Message, Name, NetworkConfig, Priority};
use wirebit::topology::Topology;

fn main() {
    println!("=== Virtual CAN Demo ===");

    let mut topo = Topology::new();
    let n1 = topo.add_node("n1");
    let n2 = topo.add_node("n2");
    topo.can_bus("bus0").members(&[n1, n2]);
    let mut built = topo.build().unwrap();
    let bus = built.can_bus_mut("bus0").unwrap();
    let ep_a = bus.take_endpoint("n1").unwrap();
    let ep_b = bus.take_endpoint("n2").unwrap();

    let mut net_a: IsoNet<wirebit::ShmLink> = IsoNet::new(NetworkConfig::default());
    let mut net_b: IsoNet<wirebit::ShmLink> = IsoNet::new(NetworkConfig::default());
    net_a.set_endpoint(0, ep_a);
    net_b.set_endpoint(0, ep_b);

    let h_a = net_a
        .create_internal(
            Name::default()
                .with_identity_number(0x100)
                .with_function_code(0x80)
                .with_self_configurable(true),
            0,
            0x80,
        )
        .unwrap();
    let h_b = net_b
        .create_internal(
            Name::default()
                .with_identity_number(0x999)
                .with_function_code(0x80)
                .with_self_configurable(true),
            0,
            0x81,
        )
        .unwrap();

    net_a.start_address_claiming().unwrap();
    net_b.start_address_claiming().unwrap();

    // Pump until both CFs claim.
    for _ in 0..50 {
        net_a.update(100);
        net_b.update(100);
        built.pump_all().unwrap();
        net_a.update(0);
        net_b.update(0);
        built.pump_all().unwrap();
        if net_a.internal_cf(h_a).unwrap().claim_state() == ClaimState::Claimed
            && net_b.internal_cf(h_b).unwrap().claim_state() == ClaimState::Claimed
        {
            break;
        }
    }
    let addr_a = net_a.internal_cf(h_a).unwrap().address();
    let addr_b = net_b.internal_cf(h_b).unwrap().address();
    println!("[claim] net_a → 0x{addr_a:02X}, net_b → 0x{addr_b:02X}");

    // 50-byte broadcast from A → B (TP CMDT).
    let payload: Vec<u8> = (0..50u32).map(|n| n as u8).collect();
    let received = Rc::new(RefCell::new(Vec::<Message>::new()));
    let r = received.clone();
    net_b
        .register_pgn_callback(0xEF11, move |m| r.borrow_mut().push(m.clone()))
        .unwrap();

    net_a
        .send(0xEF11, &payload, h_a, addr_b, Priority::Lowest)
        .unwrap();
    println!("[TP] 50-byte payload from A → B");

    for _ in 0..50 {
        net_a.update(50);
        net_b.update(50);
        built.pump_all().unwrap();
        net_a.update(0);
        net_b.update(0);
        built.pump_all().unwrap();
        if !received.borrow().is_empty() {
            break;
        }
    }

    let msgs = received.borrow();
    assert_eq!(msgs.len(), 1, "expected exactly one delivery");
    println!(
        "[RX] reassembled {} bytes (equal? {})",
        msgs[0].data.len(),
        msgs[0].data == payload
    );
    println!(
        "[stats] bus load on net_a port 0: {:.2}%",
        net_a.bus_load(0)
    );
}
