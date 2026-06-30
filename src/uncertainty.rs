//! Bayesian prediction uncertainty estimation for the Planetary Brain.
//!
//! Implements three Bayesian methods for estimating predictive uncertainty:
//!
//! - **Bayes by Backprop (BBB)**: Maintains weight means and log-variances,
//!   computes KL divergence between the approximate posterior and a standard
//!   Gaussian prior.
//! - **Deep Ensemble**: Maintains an ensemble of independent weight samples,
//!   computes prediction mean and variance across the ensemble.
//! - **MC Dropout**: Applies dropout at inference time to approximate
//!   Bayesian inference (concrete dropout probabilities).

#![deny(missing_docs)]

use std::collections::HashMap;

use rand::Rng;

use crate::components::EntityId;

/// Configuration for Bayesian uncertainty estimation.
///
/// Controls which method is active, how many ensemble members are used,
/// and the hyper-parameters for each method.
#[derive(Debug, Clone)]
pub struct BayesianConfig {
    /// Whether uncertainty estimation is enabled.
    pub enabled: bool,
    /// Which Bayesian method to use.
    pub method: BayesianMethod,
    /// Number of ensemble members (used by BBB and DeepEnsemble).
    pub ensemble_size: usize,
}

impl Default for BayesianConfig {
    fn default() -> Self {
        BayesianConfig {
            enabled: false,
            method: BayesianMethod::MCDropout { dropout_prob: 0.1 },
            ensemble_size: 10,
        }
    }
}

/// The Bayesian method used for uncertainty estimation.
#[derive(Debug, Clone)]
pub enum BayesianMethod {
    /// Bayes by Backprop: variational inference with learned variance.
    BayesByBackprop,
    /// Deep Ensemble: multiple independent forward passes.
    DeepEnsemble,
    /// Monte Carlo Dropout with a fixed dropout probability.
    MCDropout {
        /// Probability of dropping a connection at inference.
        dropout_prob: f32,
    },
}

/// A predictive distribution with decomposed uncertainty estimates.
///
/// The total predictive variance is decomposed into:
/// - **Epistemic** (model) uncertainty: reducible with more data.
/// - **Aleatoric** (data) uncertainty: inherent noise in the data.
#[derive(Debug, Clone)]
pub struct Prediction {
    /// Mean prediction value.
    pub mean: f32,
    /// Total predictive variance.
    pub variance: f32,
    /// Epistemic (model) uncertainty component of the variance.
    pub epistemic: f32,
    /// Aleatoric (data) uncertainty component of the variance.
    pub aleatoric: f32,
    /// Confidence score in `[0, 1]`, computed as `1 / (1 + variance)`.
    pub confidence: f32,
}

/// A Bayesian layer that maintains weight distributions and an ensemble.
///
/// Each connection between two `EntityId`s has a learned mean and log-variance.
/// The ensemble stores sampled weights from the posterior for ensemble-based
/// uncertainty estimation.
#[derive(Debug, Clone)]
pub struct BayesianLayer {
    /// Mean of the variational posterior for each weight (pre → post).
    pub weight_means: HashMap<(EntityId, EntityId), f32>,
    /// Log-variance of the variational posterior for each weight (pre → post).
    pub weight_log_vars: HashMap<(EntityId, EntityId), f32>,
    /// Collection of sampled weight configurations (ensemble members).
    pub ensemble: Vec<HashMap<(EntityId, EntityId), f32>>,
}

impl BayesianLayer {
    /// Create a new `BayesianLayer` with empty weight distributions.
    pub fn new() -> Self {
        BayesianLayer {
            weight_means: HashMap::new(),
            weight_log_vars: HashMap::new(),
            ensemble: Vec::new(),
        }
    }

    /// Forward pass that propagates uncertainty through the layer.
    ///
    /// Given input activations (mapping `EntityId → f32`), computes the
    /// predictive mean, variance, and decomposed uncertainties.
    ///
    /// - For **BBB**: computes the mean output via the weight means, computes
    ///   aleatoric variance from the weight log-variances, and computes
    ///   epistemic variance from the log-variance magnitude.
    /// - For **Deep Ensemble**: runs each ensemble member's weights and
    ///   computes the sample mean and variance across members.
    /// - For **MC Dropout**: runs the mean weights multiple times with
    ///   stochastic masking.
    ///
    /// The `config` parameter controls which method is used.
    pub fn forward(&self, inputs: &HashMap<EntityId, f32>, config: &BayesianConfig) -> Prediction {
        match config.method {
            BayesianMethod::BayesByBackprop => self.forward_bbb(inputs),
            BayesianMethod::DeepEnsemble => self.forward_ensemble(inputs),
            BayesianMethod::MCDropout { .. } => self.forward_mc_dropout(inputs, config),
        }
    }

    /// Bayes by Backprop forward pass.
    fn forward_bbb(&self, inputs: &HashMap<EntityId, f32>) -> Prediction {
        let mut mean = 0.0_f32;
        let mut aleatoric = 0.0_f32;

        for (key, w_mean) in &self.weight_means {
            let (pre, _post) = key;
            if let Some(&x) = inputs.get(pre) {
                mean += w_mean * x;
            }
        }

        for (key, log_var) in &self.weight_log_vars {
            let (pre, _post) = key;
            if let Some(&x) = inputs.get(pre) {
                let var = log_var.exp();
                aleatoric += var * x * x;
            }
        }

        let n_vars = self.weight_log_vars.len().max(1) as f32;
        let avg_log_var: f32 = self.weight_log_vars.values().sum::<f32>() / n_vars;
        let epistemic = (avg_log_var * 0.5).exp().min(10.0).max(0.0);

        let variance = aleatoric + epistemic;
        let confidence = 1.0 / (1.0 + variance);

        Prediction {
            mean,
            variance,
            epistemic,
            aleatoric,
            confidence,
        }
    }

    /// Deep Ensemble forward pass.
    fn forward_ensemble(&self, inputs: &HashMap<EntityId, f32>) -> Prediction {
        let n = self.ensemble.len();
        if n == 0 {
            return Prediction {
                mean: 0.0,
                variance: 0.0,
                epistemic: 0.0,
                aleatoric: 0.0,
                confidence: 1.0,
            };
        }

        let mut outputs = Vec::with_capacity(n);
        for member in &self.ensemble {
            let mut out = 0.0_f32;
            for (key, w) in member {
                let (pre, _post) = key;
                if let Some(&x) = inputs.get(pre) {
                    out += w * x;
                }
            }
            outputs.push(out);
        }

        let mean: f32 = outputs.iter().sum::<f32>() / n as f32;
        let variance: f32 = outputs.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / n as f32;

        let epistemic = variance;
        let aleatoric = 0.0;
        let confidence = 1.0 / (1.0 + variance);

        Prediction {
            mean,
            variance,
            epistemic,
            aleatoric,
            confidence,
        }
    }

    /// MC Dropout forward pass.
    fn forward_mc_dropout(
        &self,
        inputs: &HashMap<EntityId, f32>,
        config: &BayesianConfig,
    ) -> Prediction {
        let dropout_prob = match config.method {
            BayesianMethod::MCDropout { dropout_prob } => dropout_prob,
            _ => 0.0,
        };
        let n = config.ensemble_size.max(1);
        let mut rng = rand::thread_rng();

        let mut outputs = Vec::with_capacity(n);
        for _ in 0..n {
            let mut out = 0.0_f32;
            for (key, w_mean) in &self.weight_means {
                let (pre, _post) = key;
                let scale = 1.0 / (1.0 - dropout_prob + 1e-8);
                let mask: f32 = if rng.gen::<f32>() < dropout_prob {
                    0.0
                } else {
                    scale
                };
                if let Some(&x) = inputs.get(pre) {
                    out += w_mean * x * mask;
                }
            }
            outputs.push(out);
        }

        let mean: f32 = outputs.iter().sum::<f32>() / n as f32;
        let variance: f32 = outputs.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / n as f32;

        let aleatoric = variance * dropout_prob;
        let epistemic = variance * (1.0 - dropout_prob);
        let confidence = 1.0 / (1.0 + variance);

        Prediction {
            mean,
            variance,
            epistemic,
            aleatoric,
            confidence,
        }
    }

    /// Sample `n` weight configurations from the variational posterior.
    pub fn sample_weights(&self, n: usize) -> Vec<HashMap<(EntityId, EntityId), f32>> {
        let mut rng = rand::thread_rng();
        let mut samples = Vec::with_capacity(n);

        let keys: Vec<&(EntityId, EntityId)> = self.weight_means.keys().collect();

        for _ in 0..n {
            let mut member = HashMap::with_capacity(keys.len());
            for &&key in &keys {
                let mu = self.weight_means.get(&key).copied().unwrap_or(0.0);
                let log_var = self.weight_log_vars.get(&key).copied().unwrap_or(-10.0);
                let sigma = (log_var * 0.5).exp();
                // Box-Muller transform for N(0,1) noise.
                let u1: f32 = rng.gen::<f32>().max(1e-8);
                let u2: f32 = rng.gen::<f32>().max(1e-8);
                let noise: f32 =
                    (-2.0_f32 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
                let w = mu + sigma * noise;
                member.insert(key, w);
            }
            samples.push(member);
        }

        samples
    }

    /// Compute the KL divergence between the variational posterior and the prior.
    pub fn kl_loss(&self) -> f32 {
        let mut kl = 0.0_f32;
        for (mu, log_var) in self
            .weight_means
            .values()
            .zip(self.weight_log_vars.values())
        {
            let sigma2 = log_var.exp();
            kl += 0.5 * (mu * mu + sigma2 - 1.0 - *log_var);
        }
        kl.max(0.0)
    }
}

impl Default for BayesianLayer {
    fn default() -> Self {
        Self::new()
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
    fn test_prediction_confidence() {
        let p = Prediction {
            mean: 0.5,
            variance: 0.0,
            epistemic: 0.0,
            aleatoric: 0.0,
            confidence: 1.0,
        };
        assert!((p.confidence - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_bayesian_layer_default() {
        let layer = BayesianLayer::default();
        assert!(layer.weight_means.is_empty());
        assert!(layer.weight_log_vars.is_empty());
        assert!(layer.ensemble.is_empty());
    }

    #[test]
    fn test_bayesian_config_default() {
        let config = BayesianConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.ensemble_size, 10);
    }

    #[test]
    fn test_bbb_forward() {
        let mut layer = BayesianLayer::new();
        let pre = eid(1);
        let post = eid(2);

        layer.weight_means.insert((pre, post), 0.5);
        layer.weight_log_vars.insert((pre, post), -4.0);

        let mut inputs = HashMap::new();
        inputs.insert(pre, 2.0);

        let config = BayesianConfig {
            enabled: true,
            method: BayesianMethod::BayesByBackprop,
            ensemble_size: 5,
        };

        let pred = layer.forward(&inputs, &config);
        assert!((pred.mean - 1.0).abs() < 0.01);
        assert!(pred.variance >= 0.0);
        assert!(pred.confidence > 0.0 && pred.confidence <= 1.0);
    }

    #[test]
    fn test_ensemble_forward() {
        let mut layer = BayesianLayer::new();
        let pre = eid(1);
        let post = eid(2);

        let mut m1 = HashMap::new();
        m1.insert((pre, post), 0.4);
        let mut m2 = HashMap::new();
        m2.insert((pre, post), 0.6);
        layer.ensemble = vec![m1, m2];

        let mut inputs = HashMap::new();
        inputs.insert(pre, 1.0);

        let config = BayesianConfig {
            enabled: true,
            method: BayesianMethod::DeepEnsemble,
            ensemble_size: 2,
        };

        let pred = layer.forward(&inputs, &config);
        assert!((pred.mean - 0.5).abs() < 0.01);
        assert!(pred.variance > 0.0);
    }

    #[test]
    fn test_kl_loss_zero_for_perfect_fit() {
        let mut layer = BayesianLayer::new();
        let pre = eid(1);
        let post = eid(2);

        layer.weight_means.insert((pre, post), 0.0);
        layer.weight_log_vars.insert((pre, post), 0.0);

        let kl = layer.kl_loss();
        assert!((kl - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_kl_loss_positive() {
        let mut layer = BayesianLayer::new();
        let pre = eid(1);
        let post = eid(2);

        layer.weight_means.insert((pre, post), 2.0);
        layer.weight_log_vars.insert((pre, post), -2.0);

        let kl = layer.kl_loss();
        assert!(kl > 0.0);
    }

    #[test]
    fn test_sample_weights() {
        let mut layer = BayesianLayer::new();
        let pre = eid(1);
        let post = eid(2);

        layer.weight_means.insert((pre, post), 1.0);
        layer.weight_log_vars.insert((pre, post), -10.0);

        let samples = layer.sample_weights(5);
        assert_eq!(samples.len(), 5);
        for s in &samples {
            let w = s.get(&(pre, post)).copied().unwrap_or(0.0);
            assert!((w - 1.0).abs() < 0.1);
        }
    }
}
