//! Serializable deterministic PRNG for the simulator core.
//!
//! Algorithm: **xoshiro256++** (state: 4 x u64), seeded by splitmix64 expansion.
//! This replaces `rand::StdRng` in `SchemeLogic`, whose internal state cannot be
//! serialized into a snapshot. The generator state is part of the world state:
//! it is exported/imported with snapshot v2 (see `docs/project/state_inventory.md`).
//!
//! Range mapping: bounded integers use Lemire's nearly-divisionless method
//! (with the standard unbiased rejection step). **The mapping method is part
//! of the reproducibility contract** — changing it changes every random
//! sequence produced from a given seed, so do not swap it casually.

/// splitmix64: expand a single u64 seed into the 256-bit xoshiro state.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prng {
    state: [u64; 4],
}

impl Default for Prng {
    /// Default construction is the deterministic seed-0 state. There is no
    /// entropy-based construction anywhere in the simulator path.
    fn default() -> Self {
        Self::from_seed(0)
    }
}

impl Prng {
    pub fn from_seed(seed: u64) -> Self {
        let mut sm = seed;
        let mut state = [0u64; 4];
        for slot in state.iter_mut() {
            *slot = splitmix64(&mut sm);
        }
        // xoshiro256++ requires a non-all-zero state; splitmix64 expansion of
        // any seed cannot practically produce one, but guard anyway.
        if state.iter().all(|&x| x == 0) {
            state[0] = 0x9E37_79B9_7F4A_7C15;
        }
        Self { state }
    }

    /// Current internal state, for snapshot export.
    pub fn state(&self) -> [u64; 4] {
        self.state
    }

    /// Restore a previously exported state (snapshot import).
    pub fn set_state(&mut self, state: [u64; 4]) {
        // An all-zero state would freeze xoshiro at zero forever; refuse it
        // by substituting the seed-0 state instead of corrupting the stream.
        if state.iter().all(|&x| x == 0) {
            *self = Self::from_seed(0);
        } else {
            self.state = state;
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.state;
        let result = (s[0].wrapping_add(s[3])).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Uniform value in `0..n` via Lemire's method (unbiased).
    /// Panics if `n == 0`; callers use `next_range_inclusive` instead.
    pub fn next_bounded(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0);
        let mut x = self.next_u64();
        let mut m = (x as u128).wrapping_mul(n as u128);
        let mut l = m as u64;
        if l < n {
            let threshold = n.wrapping_neg() % n;
            while l < threshold {
                x = self.next_u64();
                m = (x as u128).wrapping_mul(n as u128);
                l = m as u64;
            }
        }
        (m >> 64) as u64
    }

    /// Uniform integer in `min..=max` (inclusive, order-insensitive).
    pub fn next_range_inclusive(&mut self, min: isize, max: isize) -> isize {
        let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
        if lo == hi {
            return lo;
        }
        let span = (hi as i128 - lo as i128 + 1) as u64;
        lo + self.next_bounded(span) as isize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Prng::from_seed(42);
        let mut b = Prng::from_seed(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn state_roundtrip_continues_sequence() {
        let mut a = Prng::from_seed(7);
        for _ in 0..10 {
            a.next_u64();
        }
        let saved = a.state();
        let expected: Vec<u64> = (0..10).map(|_| a.next_u64()).collect();

        let mut restored = Prng::from_seed(999);
        restored.set_state(saved);
        let actual: Vec<u64> = (0..10).map(|_| restored.next_u64()).collect();
        assert_eq!(expected, actual);
    }

    #[test]
    fn range_bounds_respected() {
        let mut p = Prng::from_seed(1);
        for _ in 0..10_000 {
            let v = p.next_range_inclusive(-5, 5);
            assert!((-5..=5).contains(&v));
        }
        assert_eq!(p.next_range_inclusive(3, 3), 3);
        // order-insensitive
        let mut q = Prng::from_seed(1);
        let v = q.next_range_inclusive(10, 1);
        assert!((1..=10).contains(&v));
    }

    #[test]
    fn default_is_seed_zero() {
        assert_eq!(Prng::default().state(), Prng::from_seed(0).state());
    }
}
