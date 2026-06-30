//! Differentiable external memory module for the Planetary Brain.
//!
//! Provides a content-addressable read/write memory that uses cosine-similarity
//! attention over stored keys to retrieve weighted combinations of stored values.
//! LRU eviction keeps the memory bounded when capacity is exhausted.

#![deny(missing_docs)]

use crate::components::EntityId;

/// A differentiable external memory with cosine-similarity attention and LRU eviction.
///
/// The memory stores `(key, value)` pairs. Reading performs attention over all
/// stored keys and returns a weighted sum of the corresponding values. Writing
/// inserts or replaces an entry, evicting the least-used entry when at capacity.
///
/// All internal state uses `Vec<Vec<f32>>` which is `Sync`, making the struct
/// itself `Send` + `Sync`.
#[derive(Debug, Clone)]
pub struct MemoryModule {
    /// Whether the memory module is enabled.
    pub enabled: bool,
    /// Maximum number of key-value pairs the memory can hold.
    pub capacity: usize,
    /// Dimensionality of each key vector.
    pub key_dim: usize,
    /// Dimensionality of each value vector.
    pub value_dim: usize,
    /// Softmax temperature for read attention. Lower values sharpen attention.
    pub temperature: f32,
    /// Stored key vectors, indexed by slot.
    pub keys: Vec<Vec<f32>>,
    /// Stored value vectors, indexed by slot.
    pub values: Vec<Vec<f32>>,
    /// Per-slot usage counter for LRU eviction (higher = more recently used).
    pub usage: Vec<u64>,
    /// Internal tick counter.
    tick: u64,
}

impl MemoryModule {
    /// Create a new `MemoryModule` with the given capacity and dimensionalities.
    ///
    /// The memory starts empty. `temperature` controls the sharpness of the
    /// read attention: lower values concentrate weight on the single best match,
    /// higher values spread attention across many keys.
    pub fn new(capacity: usize, key_dim: usize, value_dim: usize, temperature: f32) -> Self {
        MemoryModule {
            enabled: true,
            capacity,
            key_dim,
            value_dim,
            temperature,
            keys: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
            usage: Vec::with_capacity(capacity),
            tick: 0,
        }
    }

    /// Observe an entity's activation at a given tick.
    ///
    /// Stores a (key, value) pair where the key is derived from the entity ID
    /// and tick, and the value is the activation value.
    pub fn observe(&mut self, entity: EntityId, value: f32, tick: u64) {
        self.tick = tick;
        // Use entity bytes + tick as a simple feature key.
        let mut key = Vec::with_capacity(self.key_dim);
        for &b in entity.0.iter().take(self.key_dim.min(32)) {
            key.push(b as f32 / 255.0);
        }
        // Pad or truncate to key_dim.
        while key.len() < self.key_dim {
            key.push((tick & 0xFF) as f32 / 255.0);
        }
        key.truncate(self.key_dim);

        let val = vec![value];
        self.write(&key, &val);
    }

    /// Write a `(key, value)` pair into memory.
    ///
    /// If the memory is at capacity, the entry with the lowest usage counter
    /// is evicted (LRU policy). The new entry is assigned a usage counter of 1.
    ///
    /// # Panics
    ///
    /// Panics if `key.len() != self.key_dim` or `value.len() != self.value_dim`.
    pub fn write(&mut self, key: &[f32], value: &[f32]) {
        assert_eq!(key.len(), self.key_dim, "key dimension mismatch");
        assert_eq!(value.len(), self.value_dim, "value dimension mismatch");

        if self.keys.len() < self.capacity {
            self.keys.push(key.to_vec());
            self.values.push(value.to_vec());
            self.usage.push(1);
        } else {
            // Find the least-used slot (LRU eviction).
            let mut min_idx = 0;
            let mut min_usage = self.usage[0];
            for (i, &u) in self.usage.iter().enumerate().skip(1) {
                if u < min_usage {
                    min_usage = u;
                    min_idx = i;
                }
            }
            // Replace the evicted slot.
            self.keys[min_idx] = key.to_vec();
            self.values[min_idx] = value.to_vec();
            self.usage[min_idx] = 1;
        }
    }

    /// Read from memory using cosine-similarity attention over all stored keys.
    ///
    /// Returns a weighted sum of all stored values, where the weight for each
    /// slot is `softmax(cosine_sim(query, key_i) / temperature)`.
    ///
    /// If memory is empty, returns a zero vector of length `value_dim`.
    ///
    /// # Panics
    ///
    /// Panics if `query.len() != self.key_dim`.
    pub fn read(&self, query: &[f32]) -> Vec<f32> {
        assert_eq!(query.len(), self.key_dim, "query dimension mismatch");

        let n = self.keys.len();
        if n == 0 {
            return vec![0.0; self.value_dim];
        }

        // Compute cosine similarities.
        let query_norm = vector_norm(query);
        let mut similarities = Vec::with_capacity(n);
        let mut max_sim = f32::NEG_INFINITY;

        for key in &self.keys {
            let sim = cosine_similarity(query, query_norm, key);
            similarities.push(sim);
            if sim > max_sim {
                max_sim = sim;
            }
        }

        // Numerically stable softmax: subtract max before exp.
        let temp = self.temperature.max(1e-8);
        let mut weights = Vec::with_capacity(n);
        let mut total = 0.0;
        for &s in &similarities {
            let w = ((s - max_sim) / temp).exp();
            weights.push(w);
            total += w;
        }

        let inv_total = if total > 0.0 { 1.0 / total } else { 0.0 };

        // Weighted sum of values.
        let mut result = vec![0.0; self.value_dim];
        for (i, value) in self.values.iter().enumerate() {
            let w = weights[i] * inv_total;
            for j in 0..self.value_dim {
                result[j] += w * value[j];
            }
        }

        result
    }

    /// Clear all stored entries from memory.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.values.clear();
        self.usage.clear();
    }
}

impl Default for MemoryModule {
    fn default() -> Self {
        MemoryModule {
            enabled: true,
            capacity: 1000,
            key_dim: 32,
            value_dim: 1,
            temperature: 1.0,
            keys: Vec::new(),
            values: Vec::new(),
            usage: Vec::new(),
            tick: 0,
        }
    }
}

/// Compute the L2 norm of a vector.
fn vector_norm(v: &[f32]) -> f32 {
    let dot: f32 = v.iter().map(|&x| x * x).sum();
    dot.sqrt()
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(query: &[f32], query_norm: f32, key: &[f32]) -> f32 {
    let mut dot = 0.0;
    for (a, b) in query.iter().zip(key.iter()) {
        dot += a * b;
    }
    let key_norm = vector_norm(key);
    let denom = query_norm * key_norm;
    if denom < 1e-12 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eid(b: u8) -> EntityId {
        let mut a = [0u8; 32];
        a[31] = b;
        EntityId(a)
    }

    #[test]
    fn test_new_memory_default() {
        let mem = MemoryModule::default();
        assert_eq!(mem.capacity, 1000);
        assert!(mem.enabled);
    }

    #[test]
    fn test_write_and_read() {
        let mut mem = MemoryModule::new(5, 3, 2, 1.0);
        mem.write(&[1.0, 0.0, 0.0], &[0.5, 0.5]);
        mem.write(&[0.0, 1.0, 0.0], &[1.0, 0.0]);
        mem.write(&[0.0, 0.0, 1.0], &[0.0, 1.0]);

        assert_eq!(mem.keys.len(), 3);

        let result = mem.read(&[1.0, 0.0, 0.0]);
        assert_eq!(result.len(), 2);
        assert!(result[0] > 0.0);
        assert!(result[1] > 0.0);
    }

    #[test]
    fn test_observe() {
        let mut mem = MemoryModule::default();
        mem.observe(eid(42), 0.75, 100);
        assert_eq!(mem.keys.len(), 1);
        assert_eq!(mem.values[0], vec![0.75]);
    }

    #[test]
    fn test_clear() {
        let mut mem = MemoryModule::new(5, 3, 2, 1.0);
        mem.write(&[1.0, 0.0, 0.0], &[0.5, 0.5]);
        mem.clear();
        assert!(mem.keys.is_empty());
    }

    #[test]
    fn test_lru_eviction() {
        let mut mem = MemoryModule::new(2, 2, 1, 1.0);
        mem.write(&[1.0, 0.0], &[10.0]);
        mem.write(&[0.0, 1.0], &[20.0]);
        mem.read(&[1.0, 0.0]);
        mem.write(&[0.5, 0.5], &[30.0]);
        assert_eq!(mem.keys.len(), 2);
        let r = mem.read(&[1.0, 0.0]);
        assert!((r[0] - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_read_empty_returns_zeros() {
        let mem: MemoryModule = MemoryModule::new(5, 3, 2, 1.0);
        let result = mem.read(&[1.0, 0.0, 0.0]);
        assert_eq!(result, vec![0.0, 0.0]);
    }
}
