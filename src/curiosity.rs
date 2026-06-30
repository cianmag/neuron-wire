//! Curiosity scheduling and intrinsic motivation for the Planetary Brain.
//!
//! Provides a configurable curiosity module that computes intrinsic rewards
//! based on prediction error (novelty) and information gain. Supports several
//! annealing schedules for the curiosity bonus weight (beta) to balance
//! exploration and exploitation over the agent's lifetime.

#![deny(missing_docs)]

use std::collections::HashMap;

/// Annealing schedule for the curiosity bonus weight.
///
/// Controls how the exploration-exploitation trade-off evolves over time.
#[derive(Debug, Clone)]
pub enum CuriositySchedule {
    /// Constant beta throughout training.
    Constant(f32),
    /// Exponential decay from `beta_0` toward `beta_inf` with time constant `tau`.
    Exponential {
        /// Initial curiosity bonus weight.
        beta_0: f32,
        /// Time constant for exponential decay (in ticks).
        tau: f32,
        /// Asymptotic minimum curiosity bonus weight.
        beta_inf: f32,
    },
    /// Cosine annealing from 1.0 to `eta_min` over `t_max` ticks, then flat.
    CosineAnnealing {
        /// Number of ticks for the cosine half-cycle.
        t_max: u64,
        /// Minimum value after annealing.
        eta_min: f32,
    },
    /// Adaptive schedule: increases beta when recent average surprise drops
    /// below `threshold`, decreases when above.
    Adaptive {
        /// Surprise threshold for switching between exploration and exploitation.
        threshold: f32,
        /// Window size for computing recent average surprise.
        window: usize,
    },
}

/// A curiosity module that drives exploration via intrinsic rewards.
///
/// Maintains visit counts to discourage revisiting the same states and
/// tracks information gain to reward novel experiences. The curiosity
/// beta (exploration weight) is managed by a schedule.
#[derive(Debug, Clone)]
pub struct CuriosityModule {
    /// Weight for count-based exploration bonus.
    pub count_beta: f32,
    /// Weight for information-gain-based exploration bonus.
    pub info_beta: f32,
    /// The annealing schedule for the curiosity beta.
    pub schedule: CuriositySchedule,
    /// Visit counter per observation hash.
    pub visit_counts: HashMap<u64, u64>,
    /// Running estimate of information gain.
    pub info_gain: f32,
    /// Internal tick counter.
    tick: u64,
    /// Recent surprise values for adaptive scheduling (ring buffer).
    recent_surprises: Vec<f32>,
    /// Current position in the recent-surprise ring buffer.
    surprise_pos: usize,
}

impl CuriosityModule {
    /// Create a new `CuriosityModule` with the given parameters.
    pub fn new(count_beta: f32, info_beta: f32, schedule: CuriositySchedule) -> Self {
        let window = match &schedule {
            CuriositySchedule::Adaptive { window, .. } => *window,
            _ => 100,
        };
        CuriosityModule {
            count_beta,
            info_beta,
            schedule,
            visit_counts: HashMap::new(),
            info_gain: 0.0,
            tick: 0,
            recent_surprises: vec![0.0; window],
            surprise_pos: 0,
        }
    }

    /// Return the current tick count for this module.
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// Compute the intrinsic reward for a state transition.
    ///
    /// This two-argument variant computes the prediction error as the mean
    /// squared difference between `state` and `next_state`, then delegates
    /// to the three-argument `intrinsic_reward`.
    ///
    /// # Arguments
    ///
    /// * `state` — The current observation/state vector.
    /// * `next_state` — The next observation/state vector.
    ///
    /// # Returns
    ///
    /// The scalar intrinsic reward for this transition.
    pub fn intrinsic_reward(&mut self, state: &[f32], next_state: &[f32]) -> f32 {
        let prediction_error = if state.len() == next_state.len() && !state.is_empty() {
            let mse: f32 = state
                .iter()
                .zip(next_state.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>()
                / state.len() as f32;
            mse.sqrt()
        } else {
            0.0
        };
        let observation_hash = simple_hash(state);
        self.tick += 1;
        self.intrinsic_reward_detailed(observation_hash, prediction_error, self.tick)
    }

    /// Compute the intrinsic reward for an observation with a known hash.
    ///
    /// The reward has two components:
    /// 1. **Count-based bonus**: `count_beta / sqrt(visit_count)`.
    /// 2. **Information gain bonus**: `info_beta * prediction_error`.
    pub fn intrinsic_reward_detailed(
        &mut self,
        observation_hash: u64,
        prediction_error: f32,
        tick: u64,
    ) -> f32 {
        self.tick = tick;

        let count = self.visit_counts.entry(observation_hash).or_insert(0);
        *count += 1;

        let count_bonus = self.count_beta * (1.0 / (*count as f32).sqrt().max(1.0));

        self.info_gain = 0.9 * self.info_gain + 0.1 * prediction_error;
        let info_bonus = self.info_beta * self.info_gain;

        let window = self.recent_surprises.len();
        if window > 0 {
            self.recent_surprises[self.surprise_pos] = prediction_error;
            self.surprise_pos = (self.surprise_pos + 1) % window;
        }

        let beta = self.curiosity_beta(tick);
        beta * (count_bonus + info_bonus)
    }

    /// Compute the curiosity beta at a given tick according to the schedule.
    pub fn curiosity_beta(&self, tick: u64) -> f32 {
        match self.schedule {
            CuriositySchedule::Constant(beta) => beta,
            CuriositySchedule::Exponential {
                beta_0,
                tau,
                beta_inf,
            } => {
                let decay = (-(tick as f32) / tau).exp();
                beta_inf + (beta_0 - beta_inf) * decay
            }
            CuriositySchedule::CosineAnnealing { t_max, eta_min } => {
                if tick >= t_max {
                    eta_min
                } else {
                    let frac = tick as f32 / t_max as f32;
                    eta_min + 0.5 * (1.0 - eta_min) * (1.0 + (std::f32::consts::PI * frac).cos())
                }
            }
            CuriositySchedule::Adaptive { threshold, .. } => {
                let n = self.recent_surprises.len().max(1);
                let avg_surprise: f32 = self.recent_surprises.iter().sum::<f32>() / n as f32;
                if avg_surprise < threshold {
                    (threshold - avg_surprise) / threshold.max(1e-8)
                } else {
                    threshold / avg_surprise.max(1e-8)
                }
            }
        }
    }

    /// Reset the curiosity module to its initial state.
    pub fn reset(&mut self) {
        self.visit_counts.clear();
        self.info_gain = 0.0;
        self.recent_surprises.fill(0.0);
        self.surprise_pos = 0;
        self.tick = 0;
    }
}

impl Default for CuriosityModule {
    fn default() -> Self {
        CuriosityModule::new(1.0, 0.5, CuriositySchedule::Constant(0.1))
    }
}

/// Compute a simple hash from a float vector using FNV-like mixing.
fn simple_hash(data: &[f32]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &x in data {
        let bits = x.to_bits();
        h ^= bits as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_schedule() {
        let mut cm = CuriosityModule::new(1.0, 0.5, CuriositySchedule::Constant(0.1));
        assert!((cm.curiosity_beta(0) - 0.1).abs() < 0.001);
        assert!((cm.curiosity_beta(1000) - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_exponential_schedule() {
        let cm = CuriosityModule::new(
            1.0,
            0.5,
            CuriositySchedule::Exponential {
                beta_0: 1.0,
                tau: 100.0,
                beta_inf: 0.01,
            },
        );
        assert!((cm.curiosity_beta(0) - 1.0).abs() < 0.01);
        assert!((cm.curiosity_beta(1000) - 0.01).abs() < 0.01);
    }

    #[test]
    fn test_intrinsic_reward_two_args() {
        let mut cm = CuriosityModule::new(2.0, 1.0, CuriositySchedule::Constant(1.0));
        let r = cm.intrinsic_reward(&[1.0, 0.0], &[1.0, 0.0]);
        // Perfect prediction, novelty bonus only
        assert!(r >= 0.0);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut cm = CuriosityModule::new(1.0, 1.0, CuriositySchedule::Constant(1.0));
        cm.intrinsic_reward(&[1.0, 0.0], &[1.1, 0.0]);
        assert!(!cm.visit_counts.is_empty());
        cm.reset();
        assert!(cm.visit_counts.is_empty());
        assert!((cm.info_gain - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_default() {
        let cm = CuriosityModule::default();
        assert!((cm.count_beta - 1.0).abs() < 0.001);
        assert!((cm.tick_count() - 0.0).abs() < 0.001);
    }
}
