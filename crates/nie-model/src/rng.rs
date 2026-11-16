use serde::{Deserialize, Serialize};

/// SplitMix64 generator. Integer-only state, so the stream is identical on every platform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetRng {
    state: u64,
}

impl DetRng {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)` with 24 bits of precision (exact in f32).
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }

    /// Uniform in `[-1, 1)`.
    pub fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    /// Uniform integer in `[lo, hi]`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        debug_assert!(lo <= hi);
        lo + (self.next_u64() % u64::from(hi - lo + 1)) as u32
    }

    /// Picks one element of a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0, items.len() as u32 - 1) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_is_stable() {
        let mut rng = DetRng::new(1);
        assert_eq!(rng.next_u64(), 0x910A_2DEC_8902_5CC1);
    }

    #[test]
    fn unit_stays_in_range() {
        let mut rng = DetRng::new(9);
        for _ in 0..10_000 {
            let v = rng.unit();
            assert!((0.0..1.0).contains(&v));
        }
    }
}
