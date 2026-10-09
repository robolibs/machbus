//! Tiny non-crypto PRNG for the `gen` command.
//!
//! A xorshift64 seeded from wall-clock nanoseconds + the PID. It avoids a
//! dependency on the `rand` crate while being more than random enough for
//! synthetic CAN traffic generation.

use std::time::{SystemTime, UNIX_EPOCH};

/// A xorshift64 generator.
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Seed from the current monotonic/real time and process id.
    pub fn new_seeded() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let pid = unsafe { libc::getpid() } as u64;
        // xorshift64 must never start at zero.
        let mut state = nanos ^ (pid << 17) ^ 0xA5A5_A5A5_A5A5_A5A5;
        if state == 0 {
            state = 0xA5A5_A5A5_A5A5_A5A5;
        }
        Self { state }
    }

    /// Next raw 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Uniform `u32` in `0..=max`.
    pub fn below(&mut self, max: u32) -> u32 {
        if max == 0 {
            return 0;
        }
        (self.next_u64() as u32) % (max + 1)
    }

    /// Fill `buf` with random bytes.
    pub fn fill(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        while i + 8 <= buf.len() {
            let v = self.next_u64();
            buf[i..i + 8].copy_from_slice(&v.to_le_bytes());
            i += 8;
        }
        if i < buf.len() {
            let v = self.next_u64();
            for (j, b) in buf[i..].iter_mut().enumerate() {
                *b = v.to_le_bytes()[j];
            }
        }
    }
}
