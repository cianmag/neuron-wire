//! Replay buffer for online continual learning in distributed neural networks.
//!
//! Provides a local experience replay buffer that stores observations and
//! supports periodic sharing (gossip) of buffer contents across remote peers.
//! Designed for the Planetary Brain's continual learning loop where agents
//! replay past experiences to mitigate catastrophic forgetting.

#![deny(missing_docs)]

use rand::Rng;

/// A single observation stored in the replay buffer.
///
/// Contains the feature vector, post-activation values, the network's
/// prediction, and the computed surprise signal.
#[derive(Debug, Clone)]
pub struct Observation {
    /// Input feature vector for the observation.
    pub features: Vec<f32>,
    /// Post-activation values from the forward pass.
    pub activation: Vec<f32>,
    /// The scalar prediction made by the network for this observation.
    pub prediction: f32,
    /// Surprise signal (prediction error magnitude) for this observation.
    pub surprise: f32,
}

/// A fixed-capacity replay buffer with FIFO/LRU eviction.
///
/// Stores observations in a circular buffer. When full, new observations
/// overwrite the oldest slot. The buffer supports batched sampling for
/// replay during training.
#[derive(Debug, Clone)]
pub struct ReplayBuffer {
    /// Maximum number of observations the buffer can hold.
    pub capacity: usize,
    /// Number of observations to sample per replay batch.
    pub batch_size: usize,
    /// Probability with which replay is activated on any given tick.
    pub replay_prob: f32,
    /// Circular buffer of stored observations.
    pub observations: Vec<Observation>,
    /// Current write position in the circular buffer.
    pub position: usize,
    /// Total number of elements written (capped at capacity).
    pub count: usize,
}

impl ReplayBuffer {
    /// Create a new `ReplayBuffer` with the given capacity and replay parameters.
    pub fn new(capacity: usize, batch_size: usize, replay_prob: f32) -> Self {
        ReplayBuffer {
            capacity,
            batch_size,
            replay_prob,
            observations: Vec::with_capacity(capacity),
            position: 0,
            count: 0,
        }
    }

    /// Push a new observation into the buffer.
    ///
    /// If the buffer has not yet reached capacity, the observation is appended.
    /// Otherwise the oldest slot (at `position`) is overwritten and `position`
    /// advances.
    pub fn push(&mut self, obs: Observation) {
        if self.observations.len() < self.capacity {
            self.observations.push(obs);
        } else {
            self.observations[self.position] = obs;
        }
        self.position = (self.position + 1) % self.capacity;
        self.count = self.observations.len();
    }

    /// Number of observations currently stored.
    pub fn len(&self) -> usize {
        self.observations.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }

    /// Draw a uniformly random sample of observations from the buffer.
    ///
    /// Returns up to `batch_size` distinct references. If the buffer has
    /// fewer elements than `batch_size`, all elements are returned.
    pub fn sample(&self) -> Vec<&Observation> {
        let n = self.observations.len();
        if n == 0 {
            return Vec::new();
        }
        let k = self.batch_size.min(n);
        let mut indices: Vec<usize> = (0..n).collect();
        let mut rng = rand::thread_rng();
        for i in 0..k {
            let j = rng.gen_range(i..n);
            indices.swap(i, j);
        }
        indices[..k]
            .iter()
            .map(|&i| &self.observations[i])
            .collect()
    }

    /// Decide whether replay should occur at the current tick.
    ///
    /// Returns `true` if a random draw falls below `replay_prob`.
    pub fn should_replay(&self, _tick: u64, rng: &mut impl Rng) -> bool {
        rng.gen::<f32>() < self.replay_prob
    }

    /// The fraction of the buffer currently occupied, in `[0, 1]`.
    pub fn fill_ratio(&self) -> f32 {
        if self.capacity == 0 {
            return 0.0;
        }
        self.observations.len() as f32 / self.capacity as f32
    }
}

/// A distributed replay buffer that pairs a local buffer with remote gossip.
///
/// Each agent maintains a local `ReplayBuffer`. Periodically (based on
/// `share_interval`) the agent can share its buffer contents with remote
/// peers. The `remote_beta` parameter weights the influence of remote
/// buffer contributions versus local experience.
#[derive(Debug, Clone)]
pub struct DistributedReplay {
    /// Whether this replay system is enabled.
    pub enabled: bool,
    /// Local replay buffer storing this agent's observations.
    pub local: ReplayBuffer,
    /// Minimum number of experiences required before replay is active.
    pub min_size: usize,
    /// Weight assigned to remote (gossiped) replay contributions.
    pub remote_beta: f32,
    /// Number of ticks between buffer-sharing gossip rounds.
    pub share_interval: u64,
}

impl DistributedReplay {
    /// Create a new `DistributedReplay` from a local buffer and gossip parameters.
    pub fn new(local: ReplayBuffer, remote_beta: f32, share_interval: u64) -> Self {
        DistributedReplay {
            enabled: true,
            local,
            min_size: 1,
            remote_beta,
            share_interval,
        }
    }

    /// Push an observation into the local replay buffer.
    pub fn push(&mut self, obs: Observation) {
        self.local.push(obs);
    }

    /// Number of observations currently stored.
    pub fn len(&self) -> usize {
        self.local.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.local.is_empty()
    }

    /// Sample a batch of observations from the local buffer.
    pub fn sample(&self) -> Vec<&Observation> {
        self.local.sample()
    }

    /// Decide whether replay should occur based on tick and random chance.
    pub fn should_replay(&self, tick: u64, rng: &mut impl Rng) -> bool {
        self.local.should_replay(tick, rng)
    }

    /// The effective remote weight at the current tick.
    pub fn remote_weight(&self) -> f32 {
        self.remote_beta * (1.0 - self.local.fill_ratio())
    }

    /// Whether a gossip-sharing tick should occur based on `share_interval`.
    pub fn should_share(&self, tick: u64) -> bool {
        self.share_interval > 0 && tick.is_multiple_of(self.share_interval)
    }
}

impl Default for DistributedReplay {
    fn default() -> Self {
        let local = ReplayBuffer::new(1000, 32, 0.5);
        DistributedReplay {
            enabled: false,
            local,
            min_size: 100,
            remote_beta: 0.3,
            share_interval: 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::mock::StepRng;

    fn make_obs(val: f32) -> Observation {
        Observation {
            features: vec![val, val + 1.0],
            activation: vec![val * 0.5],
            prediction: val,
            surprise: 0.0,
        }
    }

    #[test]
    fn test_replay_buffer_push_and_count() {
        let mut buf = ReplayBuffer::new(5, 2, 0.5);
        assert_eq!(buf.count, 0);
        for i in 0..3 {
            buf.push(make_obs(i as f32));
        }
        assert_eq!(buf.count, 3);
    }

    #[test]
    fn test_replay_buffer_circular_eviction() {
        let mut buf = ReplayBuffer::new(3, 2, 0.5);
        for i in 0..5 {
            buf.push(make_obs(i as f32));
        }
        assert_eq!(buf.observations.len(), 3);
        assert_eq!(buf.position, 2);
    }

    #[test]
    fn test_sample_returns_correct_batch_size() {
        let mut buf = ReplayBuffer::new(10, 3, 0.5);
        for i in 0..7 {
            buf.push(make_obs(i as f32));
        }
        let sample = buf.sample();
        assert_eq!(sample.len(), 3);
    }

    #[test]
    fn test_sample_empty_buffer() {
        let buf: ReplayBuffer = ReplayBuffer::new(5, 2, 0.5);
        let sample = buf.sample();
        assert!(sample.is_empty());
    }

    #[test]
    fn test_should_replay_deterministic() {
        let buf = ReplayBuffer::new(10, 2, 1.0);
        let mut rng = StepRng::new(0, 1);
        assert!(buf.should_replay(0, &mut rng));

        let buf2 = ReplayBuffer::new(10, 2, 0.0);
        let mut rng2 = StepRng::new(0, 1);
        assert!(!buf2.should_replay(0, &mut rng2));
    }

    #[test]
    fn test_fill_ratio() {
        let mut buf = ReplayBuffer::new(10, 2, 0.5);
        assert!((buf.fill_ratio() - 0.0).abs() < 0.001);
        for i in 0..5 {
            buf.push(make_obs(i as f32));
        }
        assert!((buf.fill_ratio() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_distributed_replay_default() {
        let dr = DistributedReplay::default();
        assert!(!dr.enabled);
        assert_eq!(dr.min_size, 100);
    }

    #[test]
    fn test_should_share() {
        let local = ReplayBuffer::new(10, 2, 0.5);
        let dr = DistributedReplay::new(local, 0.3, 5);
        assert!(dr.should_share(5));
        assert!(dr.should_share(10));
        assert!(!dr.should_share(3));
    }
}
