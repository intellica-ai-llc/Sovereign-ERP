//! Deterministic runtime abstractions (arc42 section 8.23, ADR 046).
//!
//! Kernel code must not call the wall clock or OS entropy directly. It receives a [`Clock`]
//! and an [`Rng`] instead, so every failing run can be replayed from a seed.
//! This crate has no dependencies on purpose; madsim/turmoil integration comes later.

/// `SplitMix64`: tiny, fast and fully deterministic, which is all test-case generation needs.
///
/// Not cryptographically secure. Never use it for anything secret.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator whose whole output sequence is determined by `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64 pseudo-random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`. Modulo bias is irrelevant for test generation.
    ///
    /// Returns `0` when `n == 0`, so callers never have to special-case an empty range.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }

    /// `true` with probability `numerator / denominator`.
    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator) < numerator
    }
}

/// Source of time. Production code gets a real clock at the edge; kernel code only sees this trait.
pub trait Clock {
    /// Milliseconds since an arbitrary, fixed epoch.
    fn now_millis(&self) -> u64;
}

/// A clock that only moves when the test says so.
#[derive(Clone, Debug, Default)]
pub struct SimClock {
    now: u64,
}

impl SimClock {
    /// A clock showing `start_millis`.
    #[must_use]
    pub fn new(start_millis: u64) -> Self {
        Self { now: start_millis }
    }

    /// Moves time forward by `millis`, saturating at `u64::MAX`.
    pub fn advance(&mut self, millis: u64) {
        self.now = self.now.saturating_add(millis);
    }
}

impl Clock for SimClock {
    fn now_millis(&self) -> u64 {
        self.now
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, Rng, SimClock};

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
    fn below_stays_in_range_and_tolerates_zero() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            assert!(r.below(10) < 10);
        }
        assert_eq!(r.below(0), 0);
    }

    #[test]
    fn clock_moves_only_when_told() {
        let mut c = SimClock::new(10);
        assert_eq!(c.now_millis(), 10);
        c.advance(5);
        assert_eq!(c.now_millis(), 15);
        c.advance(u64::MAX);
        assert_eq!(c.now_millis(), u64::MAX);
    }
}
