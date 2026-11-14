// SPDX-License-Identifier: GPL-3.0-or-later

//! SplitMix64 PRNG. Hand-rolled (no `rand` dependency) so the four UI
//! spikes cannot drift apart on PRNG version/algorithm and every spike
//! generates byte-identical scenes from the same seed.

/// The standard SplitMix64 generator (as used by `java.util.SplittableRandom`
/// and the reference implementation on Rosetta Code).
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// `state` is seeded directly to `seed`; the golden-ratio increment is
    /// added on the *first* call to `next_u64`, matching the reference
    /// algorithm (not the seed itself).
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

    /// Uniform in `[lo, hi]` inclusive; `lo <= hi`. Never panics: all
    /// arithmetic is wrapping, so even a pathological `lo`/`hi` pair
    /// (e.g. spanning the full `i32` range) returns a value rather than
    /// overflow-panicking in debug builds.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (hi as i64 - lo as i64 + 1) as u64;
        let r = self.next_u64() % span;
        lo.wrapping_add(r as i32)
    }

    /// `true` with probability `numerator / denominator`. `denominator == 0`
    /// is treated as "never" rather than panicking.
    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        if denominator == 0 {
            return false;
        }
        let r = self.next_u64() % denominator as u64;
        (r as u32) < numerator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves the PRNG is the standard SplitMix64 algorithm, not a
    /// lookalike: `SplitMix64::new(1234567)` must yield the published
    /// reference sequence. Reference vector re-derived independently (not
    /// quoted from memory) by implementing the textbook SplitMix64
    /// next-state/mix steps in a throwaway script and running them for
    /// seed 1234567; the first four outputs matched
    /// `6457827717110365317, 3203168211198807973, 9817491932198370423,
    /// 4593380528125082431` exactly, confirming the Rosetta Code
    /// "Pseudo-random numbers/Splitmix64" reference vector cited in
    /// docs/adr/001-ui-toolkit.md.
    #[test]
    fn splitmix64_matches_reference_vector() {
        let mut rng = SplitMix64::new(1234567);
        let expected = [
            6457827717110365317u64,
            3203168211198807973u64,
            9817491932198370423u64,
            4593380528125082431u64,
        ];
        for want in expected {
            assert_eq!(rng.next_u64(), want);
        }
    }

    #[test]
    fn range_i32_is_inclusive_and_in_bounds() {
        let mut rng = SplitMix64::new(42);
        let mut saw_lo = false;
        let mut saw_hi = false;
        for _ in 0..10_000 {
            let v = rng.range_i32(-5, 5);
            assert!((-5..=5).contains(&v), "{v} out of [-5, 5]");
            if v == -5 {
                saw_lo = true;
            }
            if v == 5 {
                saw_hi = true;
            }
        }
        assert!(saw_lo, "range_i32 never produced the lower bound");
        assert!(saw_hi, "range_i32 never produced the upper bound");
    }
}
