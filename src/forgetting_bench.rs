//! Catastrophic forgetting benchmark framework for the Planetary Brain.
//!
//! Provides tools to define a sequence of tasks, track per-task accuracy
//! over the learning timeline, and compute standard continual-learning
//! metrics: Backward Transfer (BWT), Forward Transfer (FWT), forgetting
//! rate, and stability.

#![deny(missing_docs)]

/// A sequence of tasks used in continual learning evaluation.
///
/// Each task is learned sequentially. After each task, accuracy is
/// measured on all tasks seen so far to detect catastrophic forgetting.
#[derive(Debug, Clone)]
pub struct TaskSequence {
    /// Name identifying this task sequence (e.g., "Permuted MNIST").
    pub name: String,
    /// Ordered list of tasks in the sequence.
    pub tasks: Vec<TaskSpec>,
}

/// Specification for a single task in a continual learning benchmark.
#[derive(Debug, Clone)]
pub struct TaskSpec {
    /// Zero-based task index within the sequence.
    pub id: usize,
    /// Human-readable name for this task.
    pub name: String,
    /// Number of training examples for this task.
    pub n_train: usize,
    /// Number of test examples for this task.
    pub n_test: usize,
}

/// Aggregated metrics for catastrophic forgetting evaluation.
///
/// Computed from an accuracy matrix where `accuracy_matrix[t][i]` is the
/// accuracy on task `i` measured **after** learning task `t` (where `t ≥ i`).
#[derive(Debug, Clone)]
pub struct ForgettingMetrics {
    /// Per-task accuracy after each learning step.
    /// `per_task_accuracy[t][i]` = accuracy on task i after learning task t.
    pub per_task_accuracy: Vec<Vec<f32>>,
    /// Backward Transfer (BWT): average impact on previously learned tasks.
    /// Positive values indicate transfer, negative values indicate forgetting.
    pub bwt: f32,
    /// Forward Transfer (FWT): average impact on future (unlearned) tasks.
    /// Positive values indicate forward knowledge transfer.
    pub fwt: f32,
    /// Average forgetting rate: how much accuracy drops on earlier tasks
    /// as new tasks are learned. Range is `[0, 1]`, higher = more forgetting.
    pub forgetting_rate: f32,
    /// Stability: the inverse of the variance in per-task accuracies
    /// after their respective learning steps. Higher = more stable.
    pub stability: f32,
}

impl ForgettingMetrics {
    /// Create a new `ForgettingMetrics` with zero-initialized fields.
    fn new(tasks: usize) -> Self {
        ForgettingMetrics {
            per_task_accuracy: Vec::with_capacity(tasks),
            bwt: 0.0,
            fwt: 0.0,
            forgetting_rate: 0.0,
            stability: 1.0,
        }
    }
}

/// A complete continual learning benchmark.
///
/// Defines a sequence of tasks, accumulates accuracy measurements, and
/// computes standard forgetting metrics.
pub struct ContinualBenchmark {
    /// Name of this benchmark (e.g., "Split CIFAR-100").
    pub name: String,
    /// The sequence of tasks in this benchmark.
    pub tasks: Vec<TaskSpec>,
    /// Computed metrics, or `None` if not yet computed.
    pub results: Option<ForgettingMetrics>,
}

impl ContinualBenchmark {
    /// Create a new empty `ContinualBenchmark` with the given name.
    pub fn new(name: &str) -> Self {
        ContinualBenchmark {
            name: name.to_string(),
            tasks: Vec::new(),
            results: None,
        }
    }

    /// Add a task specification to the benchmark.
    pub fn add_task(&mut self, task: TaskSpec) {
        self.tasks.push(task);
    }

    /// Compute all forgetting metrics from an accuracy matrix.
    ///
    /// The `accuracy_matrix` is a rectangular matrix where:
    /// - `accuracy_matrix[t][i]` = accuracy on task `i` measured after
    ///   learning task `t`.
    /// - The upper triangle (`t < i`) represents zero-shot performance on
    ///   tasks not yet learned. The diagonal (`t == i`) represents
    ///   within-task accuracy after learning.
    ///
    /// ## Metrics Computed
    ///
    /// ### BWT (Backward Transfer)
    ///
    /// Average change in accuracy on previously learned tasks after learning
    /// new tasks:
    /// ```text
    /// BWT = (1 / (N-1)) · Σ_{i=1}^{N-1} (accuracy_{N,i} - accuracy_{i,i})
    /// ```
    /// where `N` is the total number of tasks. Positive BWT indicates
    /// that learning new tasks **improves** performance on old tasks
    /// (positive transfer). Negative BWT indicates **forgetting**.
    ///
    /// ### FWT (Forward Transfer)
    ///
    /// Average accuracy on future tasks before they are learned:
    /// ```text
    /// FWT = (1 / (N-1)) · Σ_{i=1}^{N-1} (accuracy_{i-1,i} - random_perf)
    /// ```
    /// where `random_perf` is the expected random accuracy (assumed 0.01
    /// for classification tasks).
    ///
    /// ### Forgetting Rate
    ///
    /// Average drop in accuracy on each task from its peak after learning:
    /// ```text
    /// forgetting_rate = (1 / (N-1)) · Σ_{i=0}^{N-2} (max_{t ∈ [i,N-1]} acc_{t,i} - acc_{N-1,i})
    /// ```
    ///
    /// ### Stability
    ///
    /// `1 / (1 + variance_on_diagonal)`, where the diagonal entries are
    /// the within-task accuracies `accuracy_{i,i}`.
    ///
    /// # Panics
    ///
    /// Panics if the matrix is empty or rows have inconsistent lengths.
    #[allow(clippy::needless_range_loop)] // index math is clearer here
    pub fn compute_metrics(&mut self, accuracy_matrix: Vec<Vec<f32>>) {
        assert!(
            !accuracy_matrix.is_empty(),
            "accuracy_matrix must not be empty"
        );
        let n_tasks = accuracy_matrix.len();
        for row in &accuracy_matrix {
            assert_eq!(
                row.len(),
                n_tasks,
                "each row must have length equal to number of tasks"
            );
        }

        // Store raw matrix.
        let mut metrics = ForgettingMetrics::new(n_tasks);
        metrics.per_task_accuracy = accuracy_matrix.clone();

        // --- BWT: average impact on previous tasks ---
        // BWT = (1/(N-1)) * Σ_{i=0}^{N-2} (acc_{N-1,i} - acc_{i,i})
        if n_tasks > 1 {
            let mut bwt_sum = 0.0_f32;
            for i in 0..(n_tasks - 1) {
                let acc_after = accuracy_matrix[n_tasks - 1][i];
                let acc_at_learn = accuracy_matrix[i][i];
                bwt_sum += acc_after - acc_at_learn;
            }
            metrics.bwt = bwt_sum / (n_tasks - 1) as f32;
        }

        // --- FWT: average zero-shot performance on future tasks ---
        // FWT = (1/(N-1)) * Σ_{i=1}^{N-1} (acc_{i-1,i} - random_perf)
        let random_perf = 0.01_f32; // assumed random accuracy floor.
        if n_tasks > 1 {
            let mut fwt_sum = 0.0_f32;
            for i in 1..n_tasks {
                let acc_before_learning = accuracy_matrix[i - 1][i];
                fwt_sum += acc_before_learning - random_perf;
            }
            metrics.fwt = fwt_sum / (n_tasks - 1) as f32;
        }

        // --- Forgetting Rate ---
        // Average drop from each task's peak accuracy to the final accuracy.
        if n_tasks > 1 {
            let mut forget_sum = 0.0_f32;
            for i in 0..(n_tasks - 1) {
                // Peak accuracy on task i after it was learned.
                let mut peak = accuracy_matrix[i][i];
                for t in (i + 1)..n_tasks {
                    if accuracy_matrix[t][i] > peak {
                        peak = accuracy_matrix[t][i];
                    }
                }
                let final_acc = accuracy_matrix[n_tasks - 1][i];
                forget_sum += peak - final_acc;
            }
            metrics.forgetting_rate = (forget_sum / (n_tasks - 1) as f32).max(0.0);
        }

        // --- Stability ---
        // Normalised inverse of the diagonal variance. Accuracy values live in
        // [0, 1], so the maximum possible variance is 0.25 (balanced 0/1 split);
        // normalising by that bound keeps stability in [0, 1] and makes
        // "high variance ⇒ low stability" meaningful across task counts.
        if n_tasks > 0 {
            let diag_mean: f32 =
                (0..n_tasks).map(|i| accuracy_matrix[i][i]).sum::<f32>() / n_tasks as f32;
            let diag_var: f32 = (0..n_tasks)
                .map(|i| (accuracy_matrix[i][i] - diag_mean).powi(2))
                .sum::<f32>()
                / n_tasks as f32;
            metrics.stability = (1.0 - diag_var / 0.25).clamp(0.0, 1.0);
        }

        self.results = Some(metrics);
    }

    /// Produce a formatted statistical summary of the benchmark results.
    ///
    /// Returns a multi-line string with per-task accuracies and aggregated
    /// metrics. Returns a placeholder message if `compute_metrics` has not
    /// been called yet.
    pub fn report(&self) -> String {
        let metrics = match &self.results {
            Some(m) => m,
            None => {
                return format!(
                    "[Benchmark: {}] No metrics computed yet. Call compute_metrics().",
                    self.name
                );
            }
        };

        let n_tasks = self.tasks.len();
        let mut lines = Vec::new();

        lines.push(format!(
            "╔══ Continual Learning Benchmark: {} ══╗",
            self.name
        ));
        lines.push(format!("Tasks: {}", n_tasks));

        // Per-task summary.
        for (i, task) in self.tasks.iter().enumerate() {
            let final_acc =
                if i < metrics.per_task_accuracy.len() && !metrics.per_task_accuracy.is_empty() {
                    let last = metrics.per_task_accuracy.len() - 1;
                    if i < metrics.per_task_accuracy[last].len() {
                        format!("{:.2}%", metrics.per_task_accuracy[last][i] * 100.0)
                    } else {
                        "N/A".to_string()
                    }
                } else {
                    "N/A".to_string()
                };
            lines.push(format!(
                "  Task {}: {} (train={}, test={}) → final acc: {}",
                task.id, task.name, task.n_train, task.n_test, final_acc
            ));
        }

        // Accuracy matrix summary.
        if !metrics.per_task_accuracy.is_empty() {
            lines.push("".to_string());
            lines.push("Accuracy Matrix (rows=after task, cols=on task):".to_string());
            for (t, row) in metrics.per_task_accuracy.iter().enumerate() {
                let row_str: Vec<String> = row.iter().map(|&v| format!("{:.3}", v)).collect();
                lines.push(format!("  Task {}: [{}]", t, row_str.join(", ")));
            }
        }

        // Aggregated metrics.
        lines.push("".to_string());
        lines.push("Aggregated Metrics:".to_string());
        lines.push(format!(
            "  BWT (Backward Transfer):      {:.6}  {}",
            metrics.bwt,
            if metrics.bwt >= 0.0 {
                "(positive → transfer/improvement)"
            } else {
                "(negative → forgetting)"
            }
        ));
        lines.push(format!(
            "  FWT (Forward Transfer):       {:.6}  {}",
            metrics.fwt,
            if metrics.fwt >= 0.0 {
                "(positive → forward knowledge transfer)"
            } else {
                "(negative → no forward transfer)"
            }
        ));
        lines.push(format!(
            "  Forgetting Rate:              {:.6}  {}",
            metrics.forgetting_rate,
            if metrics.forgetting_rate < 0.1 {
                "(low forgetting)"
            } else if metrics.forgetting_rate < 0.3 {
                "(moderate forgetting)"
            } else {
                "(high forgetting)"
            }
        ));
        lines.push(format!(
            "  Stability:                    {:.6}  {}",
            metrics.stability,
            if metrics.stability > 0.8 {
                "(high stability)"
            } else if metrics.stability > 0.5 {
                "(moderate stability)"
            } else {
                "(low stability)"
            }
        ));

        lines.push(format!("╚{}╝", "═".repeat(self.name.len() + 36)));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: usize, name: &str, n_train: usize, n_test: usize) -> TaskSpec {
        TaskSpec {
            id,
            name: name.to_string(),
            n_train,
            n_test,
        }
    }

    #[test]
    fn test_benchmark_new() {
        let bm = ContinualBenchmark::new("TestBench");
        assert_eq!(bm.name, "TestBench");
        assert!(bm.tasks.is_empty());
        assert!(bm.results.is_none());
    }

    #[test]
    fn test_add_task() {
        let mut bm = ContinualBenchmark::new("Test");
        bm.add_task(make_task(0, "A", 100, 50));
        bm.add_task(make_task(1, "B", 200, 100));
        assert_eq!(bm.tasks.len(), 2);
    }

    #[test]
    fn test_compute_metrics_no_forgetting() {
        let mut bm = ContinualBenchmark::new("NoForget");
        bm.add_task(make_task(0, "A", 100, 50));
        bm.add_task(make_task(1, "B", 100, 50));
        bm.add_task(make_task(2, "C", 100, 50));

        // Perfect retention: accuracy on each task stays constant after learning.
        let matrix = vec![
            vec![0.9, 0.01, 0.01],
            vec![0.9, 0.85, 0.01],
            vec![0.9, 0.85, 0.80],
        ];

        bm.compute_metrics(matrix);
        let metrics = bm.results.unwrap();
        // No forgetting: BWT >= 0 (actually 0 since everything stays same)
        assert!((metrics.bwt - 0.0).abs() < 0.01);
        // Forgetting rate should be ~0.
        assert!((metrics.forgetting_rate - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_metrics_with_forgetting() {
        let mut bm = ContinualBenchmark::new("Forgetting");
        bm.add_task(make_task(0, "A", 100, 50));
        bm.add_task(make_task(1, "B", 100, 50));

        // Task 0 accuracy drops after learning task 1.
        let matrix = vec![vec![0.9, 0.01], vec![0.6, 0.85]];

        bm.compute_metrics(matrix);
        let metrics = bm.results.unwrap();
        // BWT should be negative: acc on task 0 went from 0.9 to 0.6.
        assert!(metrics.bwt < -0.2);
        // Forgetting rate should be positive.
        assert!(metrics.forgetting_rate > 0.2);
    }

    #[test]
    fn test_report_no_metrics() {
        let bm = ContinualBenchmark::new("Test");
        let report = bm.report();
        assert!(report.contains("No metrics computed yet"));
    }

    #[test]
    fn test_report_with_metrics() {
        let mut bm = ContinualBenchmark::new("TestReport");
        bm.add_task(make_task(0, "X", 100, 50));
        bm.add_task(make_task(1, "Y", 100, 50));

        let matrix = vec![vec![0.8, 0.02], vec![0.75, 0.82]];
        bm.compute_metrics(matrix);

        let report = bm.report();
        assert!(report.contains("BWT"));
        assert!(report.contains("FWT"));
        assert!(report.contains("Forgetting Rate"));
        assert!(report.contains("Stability"));
    }

    #[test]
    fn test_stability_low_with_high_variance() {
        let mut bm = ContinualBenchmark::new("Unstable");
        bm.add_task(make_task(0, "A", 100, 50));
        bm.add_task(make_task(1, "B", 100, 50));
        bm.add_task(make_task(2, "C", 100, 50));

        // High variance on diagonal.
        let matrix = vec![
            vec![1.0, 0.01, 0.01],
            vec![1.0, 0.0, 0.01],
            vec![1.0, 0.0, 0.0],
        ];

        bm.compute_metrics(matrix);
        let metrics = bm.results.unwrap();
        assert!(metrics.stability < 0.8);
    }

    #[test]
    fn test_single_task_metrics() {
        let mut bm = ContinualBenchmark::new("Single");
        bm.add_task(make_task(0, "Solo", 100, 50));

        let matrix = vec![vec![0.85]];
        bm.compute_metrics(matrix);
        let metrics = bm.results.unwrap();
        assert!((metrics.bwt - 0.0).abs() < 0.001);
        assert!((metrics.fwt - 0.0).abs() < 0.001);
        assert!((metrics.forgetting_rate - 0.0).abs() < 0.001);
        assert!(metrics.stability > 0.0);
    }
}
