//! Knowledge distillation via gossip in the Planetary Brain.
//!
//! Implements soft-target distillation where a teacher network's softened
//! logits are used as training targets for a student network. The gossip
//! protocol enables distributed distillation across peers: teachers share
//! soft targets and students learn from them.

#![deny(missing_docs)]

/// Configuration parameters for knowledge distillation.
///
/// Controls whether distillation is active, the temperature of the
/// softmax, and the interpolation weight between hard-target and
/// soft-target losses.
#[derive(Debug, Clone)]
pub struct DistillationConfig {
    /// Whether distillation is enabled during training.
    pub enabled: bool,
    /// Temperature for softening the teacher's logits.
    /// Higher temperatures produce softer probability distributions.
    pub temperature: f32,
    /// Interpolation weight: `alpha` weights the soft-target loss,
    /// `(1 - alpha)` weights the hard-target (label) loss.
    pub alpha: f32,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        DistillationConfig {
            enabled: false,
            temperature: 2.0,
            alpha: 0.7,
        }
    }
}

/// A soft target produced by a teacher network.
///
/// Contains the softened activation distribution and the temperature
/// used during softening.
#[derive(Debug, Clone)]
pub struct SoftTarget {
    /// Softened activation (probability) distribution from the teacher.
    pub activations: Vec<f32>,
    /// Temperature used when generating this soft target.
    pub temperature: f32,
}

/// Compute the softmax of a vector with temperature scaling.
///
/// Each element is transformed as:
/// ```text
/// p_i = exp(logits_i / temp) / Σ_j exp(logits_j / temp)
/// ```
///
/// Uses the numerically stable trick of subtracting the maximum value
/// before exponentiating.
///
/// # Panics
///
/// Panics if `temp` is zero or negative.
pub fn softmax_with_temperature(logits: &[f32], temp: f32) -> Vec<f32> {
    assert!(temp > 0.0, "temperature must be positive");

    let scaled: Vec<f32> = logits.iter().map(|&l| l / temp).collect();
    let max_val = scaled.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    let mut exps = Vec::with_capacity(scaled.len());
    let mut sum = 0.0_f32;
    for &s in &scaled {
        let e = (s - max_val).exp();
        exps.push(e);
        sum += e;
    }

    if sum > 0.0 {
        let inv = 1.0 / sum;
        exps.iter().map(|&e| e * inv).collect()
    } else {
        // Fallback: uniform distribution.
        let n = exps.len() as f32;
        vec![1.0 / n; exps.len()]
    }
}

/// Compute the KL divergence between two probability distributions `p` and `q`.
///
/// The KL divergence is:
/// ```text
/// D_KL(p || q) = Σ_i p_i · log(p_i / q_i)
/// ```
///
/// Both `p` and `q` should be valid probability distributions (sum to ~1.0).
/// Returns 0.0 if `p` or `q` is empty.
///
/// # Panics
///
/// Panics if `p` and `q` have different lengths.
pub fn kl_divergence(p: &[f32], q: &[f32]) -> f32 {
    assert_eq!(p.len(), q.len(), "distributions must have same length");

    let mut kl = 0.0_f32;
    for (&pi, &qi) in p.iter().zip(q.iter()) {
        if pi > 0.0 && qi > 0.0 {
            kl += pi * (pi / qi).ln();
        }
    }
    kl.max(0.0)
}

/// Compute the distillation gradient for a student network.
///
/// The gradient w.r.t. the student's logits is:
/// ```text
/// ∇z_i = (1 - alpha) · (softmax(z) - y_hard)_i
///       + alpha · (softmax(z / T) - softmax(teacher / T))_i
/// ```
/// where `T` = temperature, `y_hard` is the one-hot hard target, and
/// the second term is scaled by `1 / T²` to preserve gradient magnitude.
///
/// # Arguments
///
/// * `teacher_soft` — Teacher's softened probability distribution.
/// * `student_logits` — Student's raw logits (pre-softmax).
/// * `temperature` — Temperature used for softening.
/// * `alpha` — Interpolation weight between hard and soft targets.
///
/// # Returns
///
/// A vector of gradients for the student's logits.
///
/// # Panics
///
/// Panics if the input slices have different lengths.
pub fn distill_gradient(
    teacher_soft: &[f32],
    student_logits: &[f32],
    temperature: f32,
    alpha: f32,
) -> Vec<f32> {
    assert_eq!(
        teacher_soft.len(),
        student_logits.len(),
        "teacher and student must have same output dimension"
    );

    let n = student_logits.len();
    if n == 0 {
        return Vec::new();
    }

    // Hard-target gradient: softmax(student_logits) - softmax(one_hot)
    // We approximate the one-hot hard target as the teacher's argmax.
    let student_soft = softmax_with_temperature(student_logits, 1.0);

    // Find the argmax of the teacher distribution as the hard target.
    let hard_idx = teacher_soft
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);

    // Soft-target gradient with temperature scaling.
    let student_scaled = softmax_with_temperature(student_logits, temperature);

    let mut grads = Vec::with_capacity(n);
    for i in 0..n {
        // Hard-target component.
        let hard_target = if i == hard_idx { 1.0 } else { 0.0 };
        let hard_grad = student_soft[i] - hard_target;

        // Soft-target component (scaled by 1/T² to preserve magnitude).
        let soft_grad = (student_scaled[i] - teacher_soft[i]) / (temperature * temperature);

        let grad = (1.0 - alpha) * hard_grad + alpha * soft_grad;
        grads.push(grad);
    }

    grads
}

/// A teacher-student pair for knowledge distillation.
///
/// In the Planetary Brain, each neuron can act as either a teacher
/// (sharing soft targets) or a student (learning from received soft
/// targets). The `TeacherStudent` struct holds the role and
/// distillation configuration.
pub struct TeacherStudent {
    /// Whether this node is a teacher (`true`) or student (`false`).
    pub is_teacher: bool,
    /// Distillation configuration (temperature, alpha, enabled).
    pub config: DistillationConfig,
}

impl TeacherStudent {
    /// Create a new `TeacherStudent` with the given role and config.
    pub fn new(is_teacher: bool, config: DistillationConfig) -> Self {
        TeacherStudent { is_teacher, config }
    }

    /// Whether distillation is active (enabled and the config says so).
    pub fn is_active(&self) -> bool {
        self.config.enabled
    }

    /// Generate a soft target from raw logits.
    ///
    /// Only valid when `is_teacher` is `true`. Returns `None` for students.
    pub fn generate_soft_target(&self, logits: &[f32]) -> Option<SoftTarget> {
        if !self.is_teacher || !self.config.enabled {
            return None;
        }
        let activations = softmax_with_temperature(logits, self.config.temperature);
        Some(SoftTarget {
            activations,
            temperature: self.config.temperature,
        })
    }

    /// Compute the distillation loss gradient from a teacher's soft target.
    ///
    /// Only valid when `is_teacher` is `false` (student mode). Returns `None`
    /// for teachers.
    pub fn distill_loss_gradient(
        &self,
        teacher_soft: &[f32],
        student_logits: &[f32],
    ) -> Option<Vec<f32>> {
        if self.is_teacher || !self.config.enabled {
            return None;
        }
        Some(distill_gradient(
            teacher_soft,
            student_logits,
            self.config.temperature,
            self.config.alpha,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_softmax_temperature_uniform() {
        let logits = vec![0.0, 0.0, 0.0];
        let soft = softmax_with_temperature(&logits, 1.0);
        assert_eq!(soft.len(), 3);
        for &p in &soft {
            assert!((p - 1.0 / 3.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_softmax_temperature_sharpens() {
        let logits = vec![1.0, 0.0, 0.0];
        // Low temperature → sharper distribution.
        let soft = softmax_with_temperature(&logits, 0.5);
        assert!(soft[0] > 0.5);
    }

    #[test]
    fn test_softmax_temperature_smooths() {
        let logits = vec![5.0, 0.0, 0.0];
        // High temperature → softer distribution.
        let soft = softmax_with_temperature(&logits, 10.0);
        // All probabilities should be closer to uniform.
        assert!(soft[0] < 0.5);
    }

    #[test]
    fn test_softmax_panics_on_zero_temperature() {
        let logits = vec![1.0, 0.0];
        let result = std::panic::catch_unwind(|| softmax_with_temperature(&logits, 0.0));
        assert!(result.is_err());
    }

    #[test]
    fn test_kl_divergence_same_distribution() {
        let p = vec![0.5, 0.3, 0.2];
        let kl = kl_divergence(&p, &p);
        assert!((kl - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_kl_divergence_different() {
        let p = vec![0.9, 0.05, 0.05];
        let q = vec![0.33, 0.33, 0.34];
        let kl = kl_divergence(&p, &q);
        assert!(kl > 0.0);
    }

    #[test]
    fn test_kl_divergence_empty() {
        let p: Vec<f32> = vec![];
        let q: Vec<f32> = vec![];
        let kl = kl_divergence(&p, &q);
        assert!((kl - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_distill_gradient_returns_correct_length() {
        let teacher = softmax_with_temperature(&[2.0, 1.0, 0.1], 1.0);
        let student = vec![1.0, 0.5, 0.2];
        let grads = distill_gradient(&teacher, &student, 2.0, 0.7);
        assert_eq!(grads.len(), 3);
    }

    #[test]
    fn test_distill_gradient_alpha_zero_is_hard_only() {
        let teacher = softmax_with_temperature(&[10.0, 0.0], 1.0);
        let student = vec![0.0, 0.0];
        let grads = distill_gradient(&teacher, &student, 1.0, 0.0);
        // With alpha=0 and uniform student softmax, gradient for argmax class
        // should be negative and for others positive.
        assert!(
            grads[0] < 0.0,
            "hard gradient for argmax class should be negative"
        );
        assert!(
            grads[1] > 0.0,
            "hard gradient for non-argmax should be positive"
        );
    }

    #[test]
    fn test_teacher_student_generate_soft_target() {
        let config = DistillationConfig {
            enabled: true,
            temperature: 2.0,
            alpha: 0.7,
        };
        let teacher = TeacherStudent::new(true, config);
        let target = teacher.generate_soft_target(&[3.0, 1.0, 0.0]);
        assert!(target.is_some());
        assert_eq!(target.unwrap().activations.len(), 3);
    }

    #[test]
    fn test_student_returns_none_for_generate() {
        let config = DistillationConfig {
            enabled: true,
            temperature: 2.0,
            alpha: 0.7,
        };
        let student = TeacherStudent::new(false, config);
        assert!(student.generate_soft_target(&[1.0, 0.0]).is_none());
    }

    #[test]
    fn test_teacher_returns_none_for_distill() {
        let config = DistillationConfig {
            enabled: true,
            temperature: 2.0,
            alpha: 0.7,
        };
        let teacher = TeacherStudent::new(true, config);
        assert!(teacher
            .distill_loss_gradient(&[0.6, 0.4], &[1.0, 0.0])
            .is_none());
    }
}
