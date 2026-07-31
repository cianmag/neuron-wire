//! Dynamic activation functions with learnable parameters.
//!
//! Provides a library of scalar activation functions — both fixed
//! (Tanh, ReLU, GELU, Mish, Softplus) and learnable (PReLU, Swish,
//! AdaptiveTanh, Snake) — together with their derivatives and a
//! parameter-update mechanism for gradient-based meta-learning.
//!
//! The `ActivationFn` enum dispatches `activate`, `activate_derivative`,
//! and `update_params` generically so that code using dynamic
//! activations does not need a separate match per neuron.

use crate::components::EntityId;

// ─── Activation functions ────────────────────────────────────────

/// Available activation functions.
///
/// Variants with named fields carry learnable parameters that can be
/// updated via [`update_params`].
#[derive(Debug, Clone, PartialEq)]
pub enum ActivationFn {
    /// `tanh(x)` — range [-1, 1], no learnable params.
    Tanh,
    /// `max(0, x)` — no learnable params.
    ReLU,
    /// Parametric ReLU: `max(α·x, x)` — `alpha` is learnable.
    PReLU(f32),
    /// Leaky ReLU: `max(neg_slope·x, x)` — fixed negative slope.
    LeakyReLU(f32),
    /// `x · sigmoid(β·x)` — `beta` is learnable (default ~ 1.0).
    Swish(f32),
    /// Gaussian Error Linear Unit: smooth approximation of ReLU.
    GELU,
    /// `γ · tanh(x)` — `gamma` is learnable (scale factor).
    AdaptiveTanh(f32),
    /// `ln(1 + eˣ)` — smooth, no learnable params.
    Softplus,
    /// `x · tanh(ln(1 + eˣ))` — no learnable params.
    Mish,
    /// `x + (1/α) · sin²(α·x)` — `alpha` is learnable.
    Snake(f32),
}

// ─── Activation configuration ────────────────────────────────────

/// Per-neuron and shared activation configuration.
///
/// Each neuron can have its own activation function (via the
/// `per_neuron` list) or fall back to a `shared` default.
#[derive(Debug, Clone, Default)]
pub struct ActivationConfig {
    /// Per-neuron activation assignments.
    pub per_neuron: Vec<(EntityId, ActivationFn)>,
    /// Shared default activation function for neurons not in `per_neuron`.
    pub shared: Option<ActivationFn>,
}

impl ActivationConfig {
    /// Create an empty configuration with no per-neuron assignments.
    pub fn new() -> Self {
        ActivationConfig::default()
    }

    /// Create a configuration with only a shared activation.
    pub fn with_shared(fn_: ActivationFn) -> Self {
        ActivationConfig {
            per_neuron: Vec::new(),
            shared: Some(fn_),
        }
    }

    /// Retrieve the activation function for a given neuron, falling
    /// back to the shared default or `Tanh`.
    pub fn get(&self, neuron: &EntityId) -> ActivationFn {
        for (id, fn_) in &self.per_neuron {
            if id == neuron {
                return fn_.clone();
            }
        }
        self.shared.clone().unwrap_or(ActivationFn::Tanh)
    }

    /// Assign an activation function to a specific neuron.
    pub fn set(&mut self, neuron: EntityId, fn_: ActivationFn) {
        for (id, ref mut f) in &mut self.per_neuron {
            if *id == neuron {
                *f = fn_;
                return;
            }
        }
        self.per_neuron.push((neuron, fn_));
    }
}

// ─── Activation and derivative ───────────────────────────────────

/// Evaluate the activation function `fn_` at `x`.
pub fn activate(fn_: &ActivationFn, x: f32) -> f32 {
    match fn_ {
        ActivationFn::Tanh => x.tanh(),
        ActivationFn::ReLU => {
            if x > 0.0 {
                x
            } else {
                0.0
            }
        }
        ActivationFn::PReLU(alpha) => {
            if x > 0.0 {
                x
            } else {
                alpha * x
            }
        }
        ActivationFn::LeakyReLU(slope) => {
            if x > 0.0 {
                x
            } else {
                slope * x
            }
        }
        ActivationFn::Swish(beta) => {
            let s = beta * x;
            x * sigmoid(s)
        }
        ActivationFn::GELU => gelu(x),
        ActivationFn::AdaptiveTanh(gamma) => gamma * x.tanh(),
        ActivationFn::Softplus => softplus(x),
        ActivationFn::Mish => mish(x),
        ActivationFn::Snake(alpha) => snake(x, *alpha),
    }
}

/// Evaluate the derivative of the activation function at `x`.
///
/// This does **not** include the derivative of any learnable
/// parameter — it is simply `df(x)/dx`.
pub fn activate_derivative(fn_: &ActivationFn, x: f32) -> f32 {
    match fn_ {
        ActivationFn::Tanh => 1.0 - x.tanh().powi(2),
        ActivationFn::ReLU => {
            if x > 0.0 {
                1.0
            } else {
                0.0
            }
        }
        ActivationFn::PReLU(alpha) => {
            if x > 0.0 {
                1.0
            } else {
                *alpha
            }
        }
        ActivationFn::LeakyReLU(slope) => {
            if x > 0.0 {
                1.0
            } else {
                *slope
            }
        }
        ActivationFn::Swish(beta) => {
            let s = sigmoid(beta * x);
            let ds = s * (1.0 - s) * beta; // dsigmoid(β·x)/dx
            s + x * ds
        }
        ActivationFn::GELU => gelu_derivative(x),
        ActivationFn::AdaptiveTanh(gamma) => gamma * (1.0 - x.tanh().powi(2)),
        ActivationFn::Softplus => sigmoid(x),
        ActivationFn::Mish => mish_derivative(x),
        ActivationFn::Snake(alpha) => {
            // d/dx [x + (1/α)·sin²(α·x)]
            // = 1 + 2·sin(α·x)·cos(α·x)
            // = 1 + sin(2·α·x)
            let s = (alpha * x).sin();
            let c = (alpha * x).cos();
            1.0 + 2.0 * s * c
        }
    }
}

/// Gradient-based update of learnable parameters in an activation function.
///
/// `grad` is the gradient of the loss with respect to the activation
/// output, and `lr` is the learning rate.
///
/// For non-learnable variants this is a no-op.
pub fn update_params(fn_: &mut ActivationFn, grad: f32, lr: f32) {
    match fn_ {
        ActivationFn::PReLU(alpha) => {
            *alpha -= lr * grad * alpha.signum().max(0.01);
        }
        ActivationFn::Swish(beta) => {
            *beta -= lr * grad * beta.max(0.01);
        }
        ActivationFn::AdaptiveTanh(gamma) => {
            *gamma -= lr * grad * gamma.signum().max(0.01);
        }
        ActivationFn::Snake(alpha) => {
            *alpha -= lr * grad * alpha.max(0.01);
        }
        _ => {}
    }
}

// ─── Elementary math helpers ─────────────────────────────────────

/// Logistic sigmoid.
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Gaussian Error Linear Unit: `x · Φ(x)` where `Φ` is the standard
/// normal CDF.  Uses the tanh approximation from the original paper.
fn gelu(x: f32) -> f32 {
    // GELU approximation: 0.5·x·(1 + tanh(√(2/π)·(x + 0.044715·x³)))
    let sqrt_2_over_pi = (2.0 / std::f32::consts::PI).sqrt();
    0.5 * x * (1.0 + (sqrt_2_over_pi * (x + 0.044715 * x.powi(3))).tanh())
}

/// Derivative of the GELU approximation.
fn gelu_derivative(x: f32) -> f32 {
    let sqrt_2_over_pi = (2.0 / std::f32::consts::PI).sqrt();
    let inner = sqrt_2_over_pi * (x + 0.044715 * x.powi(3));
    let tanh_inner = inner.tanh();
    let sech2 = 1.0 - tanh_inner.powi(2);
    0.5 * (1.0 + tanh_inner) + 0.5 * x * sech2 * sqrt_2_over_pi * (1.0 + 3.0 * 0.044715 * x.powi(2))
}

/// Softplus: `ln(1 + eˣ)`.
fn softplus(x: f32) -> f32 {
    if x > 20.0 {
        x // numerically stable approximation
    } else {
        (1.0 + x.exp()).ln()
    }
}

/// Mish: `x · tanh(softplus(x))`.
fn mish(x: f32) -> f32 {
    x * softplus(x).tanh()
}

/// Derivative of Mish.
fn mish_derivative(x: f32) -> f32 {
    let sp = softplus(x);
    let tanh_sp = sp.tanh();
    let sech2 = 1.0 - tanh_sp.powi(2);
    tanh_sp + x * sech2 * sigmoid(x)
}

/// Snake activation: `x + (1/α) · sin²(α·x)`.
fn snake(x: f32, alpha: f32) -> f32 {
    let s = (alpha * x).sin();
    x + s * s / alpha
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tanh() {
        let a = activate(&ActivationFn::Tanh, 0.0);
        assert!((a - 0.0).abs() < 1e-6);
        let d = activate_derivative(&ActivationFn::Tanh, 0.0);
        assert!((d - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_relu() {
        let a_pos = activate(&ActivationFn::ReLU, 2.0);
        assert!((a_pos - 2.0).abs() < 1e-6);
        let a_neg = activate(&ActivationFn::ReLU, -1.0);
        assert!((a_neg - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_prelu() {
        let fn_ = ActivationFn::PReLU(0.1);
        let a_pos = activate(&fn_, 2.0);
        assert!((a_pos - 2.0).abs() < 1e-6);
        let a_neg = activate(&fn_, -2.0);
        assert!((a_neg - (-0.2)).abs() < 1e-6);
    }

    #[test]
    fn test_leaky_relu() {
        let fn_ = ActivationFn::LeakyReLU(0.01);
        let a_neg = activate(&fn_, -1.0);
        assert!((a_neg - (-0.01)).abs() < 1e-6);
    }

    #[test]
    fn test_swish() {
        let fn_ = ActivationFn::Swish(1.0);
        let a = activate(&fn_, 0.0);
        assert!((a - 0.0).abs() < 1e-6);
        let a_pos = activate(&fn_, 2.0);
        assert!(a_pos > 0.0);
    }

    #[test]
    fn test_gelu() {
        let a = activate(&ActivationFn::GELU, 0.0);
        assert!((a - 0.0).abs() < 1e-6);
        let a_pos = activate(&ActivationFn::GELU, 1.0);
        assert!((a_pos - 0.841).abs() < 0.01);
    }

    #[test]
    fn test_adaptive_tanh() {
        let fn_ = ActivationFn::AdaptiveTanh(2.0);
        let a = activate(&fn_, 0.5);
        // 2.0 * tanh(0.5) ~ 2.0 * 0.4621 = 0.9242
        assert!((a - 0.9242).abs() < 0.01);
    }

    #[test]
    fn test_softplus() {
        let a = activate(&ActivationFn::Softplus, 0.0);
        assert!((a - std::f32::consts::LN_2).abs() < 0.001);
    }

    #[test]
    fn test_mish() {
        let a = activate(&ActivationFn::Mish, 0.0);
        assert!((a - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_snake() {
        let fn_ = ActivationFn::Snake(0.5);
        let a = activate(&fn_, 0.0);
        assert!((a - 0.0).abs() < 1e-6);
        let a_odd = activate(&fn_, std::f32::consts::PI);
        // snake(π, 0.5) = π + 2·sin²(0.5π) = π + 2
        assert!((a_odd - (std::f32::consts::PI + 2.0)).abs() < 0.01);
    }

    #[test]
    fn test_derivatives_non_negative() {
        // Most common activation derivatives are >= 0 for all inputs
        for fn_ in &[
            ActivationFn::Tanh,
            ActivationFn::ReLU,
            ActivationFn::LeakyReLU(0.01),
            ActivationFn::Swish(1.0),
            ActivationFn::Softplus,
        ] {
            for x in [-10.0, -1.0, 0.0, 1.0, 10.0] {
                let d = activate_derivative(fn_, x);
                assert!(d >= -1e-6, "negative derivative for {:?} at x={}", fn_, x);
            }
        }
    }

    #[test]
    fn test_update_params_prelu() {
        let mut fn_ = ActivationFn::PReLU(0.1);
        update_params(&mut fn_, 0.5, 0.01);
        if let ActivationFn::PReLU(alpha) = fn_ {
            assert!((alpha - 0.095).abs() < 0.01);
        } else {
            panic!("not PReLU");
        }
    }

    #[test]
    fn test_activation_config() {
        let mut cfg = ActivationConfig::with_shared(ActivationFn::ReLU);
        let n1 = EntityId([1u8; 32]);
        let n2 = EntityId([2u8; 32]);
        cfg.set(n1, ActivationFn::Tanh);
        assert_eq!(cfg.get(&n1), ActivationFn::Tanh);
        assert_eq!(cfg.get(&n2), ActivationFn::ReLU);
    }
}
