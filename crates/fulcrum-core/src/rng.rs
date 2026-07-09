//! [`SimRng`]: the deterministic simulation random number generator.

use std::ops::Range;

use bevy_ecs::prelude::Resource;
use rand_core::{Rng, SeedableRng};
use rand_pcg::Pcg32;

/// The simulation RNG, seeded from [`FulcrumConfig::seed`](crate::FulcrumConfig::seed) and
/// inserted automatically.
///
/// All randomness that affects simulation state MUST come from this resource (or a
/// [`fork`](Self::fork) of it) — never `std` hashing, thread RNGs, or time-based seeds. Same
/// seed + same inputs = same rolls, which is what makes replays possible. See
/// `docs/determinism.md`.
#[derive(Resource)]
pub struct SimRng(Pcg32);

impl SimRng {
    /// An RNG with an explicit seed.
    pub fn seeded(seed: u64) -> Self {
        Self(Pcg32::seed_from_u64(seed))
    }

    /// Next raw 32-bit value.
    pub fn u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    /// Next raw 64-bit value.
    pub fn u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    /// Uniform `f32` in `[0, 1)`.
    pub fn unit_f32(&mut self) -> f32 {
        // 24 mantissa-safe bits.
        (self.0.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform `f32` in `[range.start, range.end)`.
    pub fn range_f32(&mut self, range: Range<f32>) -> f32 {
        range.start + self.unit_f32() * (range.end - range.start)
    }

    /// Uniform `i32` in `[range.start, range.end)`. Panics if the range is empty.
    pub fn range_i32(&mut self, range: Range<i32>) -> i32 {
        assert!(range.start < range.end, "empty range");
        let span = (range.end as i64 - range.start as i64) as u64;
        // Lemire multiply-shift; slight bias is irrelevant at game scales, determinism is not.
        let value = (u64::from(self.0.next_u32()) * span) >> 32;
        range.start + value as i32
    }

    /// `true` with probability `p` (clamped to `0..=1`).
    pub fn chance(&mut self, p: f32) -> bool {
        self.unit_f32() < p
    }

    /// Split off an independent child RNG (for a subsystem or mod), advancing this one once.
    /// Forked streams stay deterministic and don't interleave with the parent's rolls.
    pub fn fork(&mut self) -> SimRng {
        SimRng::seeded(self.u64())
    }
}
