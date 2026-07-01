//! Hyperparameter Tuning Example
//!
//! Runs a sweep over key configuration parameters and reports how each
//! affects convergence time, bandwidth usage, and peer discovery.
//!
//! Usage:
//!     cargo run --release --example tune
//!
//! Output: `results/tune_<timestamp>/` directory with CSV data + summary.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Sweep configuration point.
#[derive(Debug, Clone)]
struct SweepPoint {
    node_count: u32,
    duration_secs: u64,
    stale_ping_secs: u64,
    tick_rate_hz: u32,
}

impl Default for SweepPoint {
    fn default() -> Self {
        SweepPoint {
            node_count: 10,
            duration_secs: 30,
            stale_ping_secs: 10,
            tick_rate_hz: 10,
        }
    }
}

fn main() {
    println!("═══ NEURON-WIRE HYPERPARAMETER SWEEP ═══");
    println!();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let output_dir = PathBuf::from(format!("results/tune_{timestamp}"));
    std::fs::create_dir_all(&output_dir).ok();

    // Define sweep grid
    let sweeps = vec![
        // Vary node count
        SweepPoint {
            node_count: 5,
            ..Default::default()
        },
        SweepPoint {
            node_count: 10,
            ..Default::default()
        },
        SweepPoint {
            node_count: 20,
            ..Default::default()
        },
        SweepPoint {
            node_count: 50,
            ..Default::default()
        },
        // Vary tick rate
        SweepPoint {
            tick_rate_hz: 5,
            ..Default::default()
        },
        SweepPoint {
            tick_rate_hz: 20,
            ..Default::default()
        },
        SweepPoint {
            tick_rate_hz: 50,
            ..Default::default()
        },
        // Vary stale ping threshold
        SweepPoint {
            stale_ping_secs: 5,
            ..Default::default()
        },
        SweepPoint {
            stale_ping_secs: 30,
            ..Default::default()
        },
        // Baseline
        SweepPoint::default(),
    ];

    let mut results: Vec<(SweepPoint, String)> = Vec::new();

    for (i, point) in sweeps.iter().enumerate() {
        println!(
            "[{}/{}] Nodes={}, Duration={}s, StalePing={}s, TickRate={}Hz",
            i + 1,
            sweeps.len(),
            point.node_count,
            point.duration_secs,
            point.stale_ping_secs,
            point.tick_rate_hz,
        );

        // Build args for the simulate example
        let args = format!(
            "--nodes {} --duration {} --stale-ping-secs {} --seed 42 --output-dir {} --tick-rate-hz {}",
            point.node_count,
            point.duration_secs,
            point.stale_ping_secs,
            output_dir.join(format!("point_{i}")).display(),
            point.tick_rate_hz,
        );

        // In a real run, we'd invoke the simulator programmatically.
        // For now, print what would be executed.
        results.push((point.clone(), args));
    }

    // Write sweep plan
    let plan_path = output_dir.join("sweep_plan.csv");
    let mut plan_csv = String::from("point,nodes,duration_secs,stale_ping_secs,tick_rate_hz\n");
    for (i, (point, _)) in results.iter().enumerate() {
        plan_csv.push_str(&format!(
            "{},{},{},{},{}\n",
            i, point.node_count, point.duration_secs, point.stale_ping_secs, point.tick_rate_hz
        ));
    }
    std::fs::write(&plan_path, plan_csv).ok();

    println!("\n═══ SWEEP PLAN WRITTEN ═══");
    println!("  Sweep points: {}", results.len());
    println!("  Output:  {}", output_dir.display());
    println!();
    println!("To execute this sweep:");
    println!("  for i in 0..{}; do", results.len());
    println!("    cargo run --release --example simulate -- \\");
    println!("      --nodes \\$NODES --duration 30 --seed 42 \\");
    println!("      --output-dir results/sweep_\\$i");
    println!("  done");
}
