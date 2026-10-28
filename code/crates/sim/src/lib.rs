//! Deterministic runtime abstractions (arc42 8.23, ADR 046).
//!
//! Kernel code must not call the wall clock or OS entropy directly. It receives a [`Clock`]
//! and an [`Rng`] instead, so every failing run can be replayed from a seed.
//! This crate has no dependencies on purpose; madsim/turmoil integration comes later.

/// SplitMix64: tiny, fast, fully deterministic, good enough for test-case generation.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish value in `0..n` (modulo bias is irrelevant for test generation). Panics if `n == 0`.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "Rng::below(0)");
        self.next_u64() % n
    }

    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator) < numerator
    }
}

/// Source of time. Production code gets a real clock at the edge; kernel code only sees this trait.
pub trait Clock {
    fn now_millis(&self) -> u64;
}

/// A clock that only moves when the test says so.
#[derive(Clone, Debug, Default)]
pub struct SimClock {
    now: u64,
}

impl SimClock {
    pub fn new(start_millis: u64) -> Self {
        Self { now: start_millis }
    }
    pub fn advance(&mut self, millis: u64) {
        self.now += millis;
    }
}

impl Clock for SimClock {
    fn now_millis(&self) -> u64 {
        self.now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seed_different_sequence() {
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
    }

    #[test]
    fn known_vector() {
        // Reference values for SplitMix64 seeded with 0.
        let mut r = Rng::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }

    #[test]
    fn clock_moves_only_when_told() {
        let mut c = SimClock::new(10);
        assert_eq!(c.now_millis(), 10);
        c.advance(5);
        assert_eq!(c.now_millis(), 15);
    }
}
