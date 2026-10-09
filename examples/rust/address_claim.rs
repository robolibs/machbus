//! Address-claim contention demo.
//!
//! Two CFs both prefer 0x80; the lower NAME wins arbitration and the
//! loser self-configures to the next free address. Mirrors the C++
//! `examples/address_claim.cpp` (single-CF) plus a contention twist.
//!
//! Run with `cargo run --example address_claim`.

use machbus::net::{AddressClaimer, ClaimState, InternalCf, Name};

fn main() {
    println!("=== Address Claim Demo ===");

    let low_name = Name::default()
        .with_identity_number(0x100)
        .with_manufacturer_code(42)
        .with_function_code(25)
        .with_self_configurable(true);
    let high_name = Name::default()
        .with_identity_number(0x999)
        .with_manufacturer_code(99)
        .with_function_code(25)
        .with_self_configurable(true);

    println!("low.raw  = 0x{:016X}", low_name.raw);
    println!(
        "high.raw = 0x{:016X}  (higher → loses arbitration)",
        high_name.raw
    );

    let mut cf_low = InternalCf::new(low_name, 0, 0x80);
    let mut cf_high = InternalCf::new(high_name, 0, 0x80);
    let mut clm_low = AddressClaimer::new(0);
    let mut clm_high = AddressClaimer::new(50);

    cf_low
        .on_address_claimed
        .subscribe(|&a| println!("[low ] claimed 0x{a:02X}"));
    cf_high
        .on_address_claimed
        .subscribe(|&a| println!("[high] claimed 0x{a:02X}"));
    cf_high
        .on_address_lost
        .subscribe(|_| println!("[high] lost — re-claiming after 50ms"));

    let _ = clm_low.start(&mut cf_low);
    let _ = clm_high.start(&mut cf_high);

    // Each CF sees the other's claim.
    let _ = clm_low.handle_claim(&mut cf_low, 0x80, high_name);
    let _ = clm_high.handle_claim(&mut cf_high, 0x80, low_name);

    for _ in 0..6 {
        let _ = clm_low.update(&mut cf_low, 60);
        let _ = clm_high.update(&mut cf_high, 60);
    }

    println!(
        "[low ] state={:?}  addr=0x{:02X}",
        cf_low.claim_state(),
        cf_low.address()
    );
    println!(
        "[high] state={:?}  addr=0x{:02X}",
        cf_high.claim_state(),
        cf_high.address()
    );
    assert_eq!(cf_low.claim_state(), ClaimState::Claimed);
    assert_eq!(cf_high.claim_state(), ClaimState::Claimed);
    assert_eq!(cf_low.address(), 0x80);
    assert_eq!(cf_high.address(), 0x81);
    println!("\n✓ winner kept 0x80; loser shifted to 0x81");
}
