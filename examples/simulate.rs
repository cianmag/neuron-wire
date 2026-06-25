//! Simulation CLI — Multi-Trial Benchmark Runner.
//!
//! ## Paper Reproduction
//!
//! Single trial:
//! ```text
//! cargo run --example simulate -- --nodes 5 --duration 25 --seed 42
//! ```
//!
//! Multi-trial with statistical summary:
//! ```text
//! cargo run --example simulate -- --nodes 5 --duration 25 --trials 10
//! ```
//!
//! Output: `results/experiment_<timestamp>/` with per-trial subdirectories
//! and a `benchmark_table.txt` containing a LaTeX-ready summary.
//!
//! ## Flags
//!
//! - `--nodes N` — Number of nodes (default: 5)
//! - `--duration S` — Duration in seconds (default: 25)
//! - `--trials N` — Number of trials with different seeds (default: 1)
//! - `--seed S` — Base seed for deterministic runs (default: random)
//! - `--stale-ping-secs S` — DHT stale ping threshold in seconds (default: 10)
//! - `--output-dir PATH` — Output directory
//! - `--paper-mode` — Enable deterministic paper mode (seed=42, no logs)

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let config = match neuron_wire::simulator::parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[SIM] Error: {}", e);
            eprintln!("[SIM] Usage: simulate --nodes N --duration SECS [--trials N] [--seed N] [--stale-ping-secs S] [--paper-mode] [--output-dir PATH]");
            std::process::exit(1);
        }
    };

    // Extract additional CLI flags not consumed by parse_args()
    let mut trials = 1u32;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--trials" => {
                i += 1;
                trials = args.get(i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
            }
            "--output-dir" => { i += 1; /* skip value */ }
            _ => {}
        }
        i += 1;
    }

    let base_seed = if config.paper_mode && config.seed == 0 { 42 } else { config.seed };

    eprintln!("[SIM] Launching {} trials of {} nodes for {}s...",
        if trials > 1 { trials.to_string() } else { "1".into() },
        config.node_count, config.duration_secs);
    if trials > 1 {
        eprintln!("[SIM] {} TRIALS with seeds {}-{}",
            trials, base_seed, base_seed + trials as u64 - 1);
    }

    // Collect results across trials
    let mut all_results: Vec<neuron_wire::simulator::TrialResult> = Vec::new();

    for trial in 0..trials {
        let seed = if base_seed == 0 { 0 } else { base_seed + trial as u64 };

        let mut trial_config = config.clone();
        trial_config.seed = seed;

        // Create output dir
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let output_dir = std::env::args()
            .skip_while(|a| a != "--output-dir")
            .nth(1)
            .map(|p| {
                if trials > 1 {
                    format!("{}/trial_{}", p, trial)
                } else {
                    p
                }
            })
            .unwrap_or_else(|| {
                if trials > 1 {
                    format!("results/experiment_{}/trial_{}", timestamp, trial)
                } else {
                    format!("results/experiment_{}", timestamp)
                }
            });

        let output_path = PathBuf::from(&output_dir);

        if trials > 1 {
            eprintln!("[SIM] --- Trial {}/{} (seed={}) ---", trial + 1, trials, seed);
        }

        // Run the simulation
        let mut sim = neuron_wire::simulator::Simulator::new(trial_config);
        match sim.launch() {
            Ok(()) => {
                match sim.run() {
                    Ok(result) => {
                        if let Err(e) = sim.write_results(&output_path, &result) {
                            eprintln!("[SIM] Write error: {}", e);
                        }
                        all_results.push(result);
                    }
                    Err(e) => {
                        eprintln!("[SIM] Trial {} failed: {}", trial, e);
                        // Push a failure placeholder so we can still report
                        all_results.push(neuron_wire::simulator::TrialResult {
                            trial_index: trial,
                            seed,
                            node_count: config.node_count,
                            duration_secs: 0.0,
                            total_ticks: 0,
                            total_packets_recv: 0,
                            total_packets_sent: 0,
                            total_bytes_recv: 0,
                            total_bytes_sent: 0,
                            bandwidth_kbps: 0.0,
                            avg_peers: 0.0,
                            max_peers: 0,
                            total_apoptosis_deaths: 0,
                            converged: false,
                            convergence_time_secs: None,
                            failure_mode: "none".to_string(),
                            nodes_killed: 0,
                            was_partitioned: false,
                            had_malicious_node: false,
                            recovery_time_secs: None,
                            min_peers_post_failure: 0,
                            recovered: false,
                        });
                    }
                }
                sim.shutdown();
            }
            Err(e) => {
                eprintln!("[SIM] Launch failed: {}", e);
            }
        }
    }

    // Print paper-ready summary table
    if all_results.is_empty() {
        eprintln!("[SIM] No results — nothing to report.");
        std::process::exit(1);
    }

    let n = all_results.len() as f64;
    let converged_count = all_results.iter().filter(|r| r.converged).count();

    // Compute statistics
    let avg_max_peers = all_results.iter().map(|r| r.max_peers as f64).sum::<f64>() / n;
    let avg_avg_peers = all_results.iter().map(|r| r.avg_peers).sum::<f64>() / n;

    let conv_times: Vec<f64> = all_results.iter()
        .filter_map(|r| r.convergence_time_secs)
        .collect();
    let _conv_count = conv_times.len();
    let mean_conv = if !conv_times.is_empty() {
        conv_times.iter().sum::<f64>() / conv_times.len() as f64
    } else { 0.0 };
    let std_conv = if conv_times.len() > 1 {
        let m = mean_conv;
        (conv_times.iter().map(|t| (t - m).powi(2)).sum::<f64>() / (conv_times.len() - 1) as f64).sqrt()
    } else { 0.0 };

    let min_conv = conv_times.iter().cloned().fold(f64::MAX, f64::min);
    let max_conv = conv_times.iter().cloned().fold(0.0f64, f64::max);

    let avg_bw = all_results.iter().map(|r| r.bandwidth_kbps).sum::<f64>() / n;
    let avg_pkts = all_results.iter().map(|r| r.total_packets_recv as f64).sum::<f64>() / n;

    // Print (also save to file if single-run that generated an output dir)
    eprintln!();
    eprintln!("═══════════════════════════════════════════════════");
    eprintln!("  NEURON-WIRE DHT CONVERGENCE BENCHMARK");
    eprintln!("  Nodes: {} × {}s × {} trial(s)", config.node_count, config.duration_secs, trials);
    eprintln!("═══════════════════════════════════════════════════");
    eprintln!();
    eprintln!("  Convergence rate:   {}/{} ({:.0}%)", converged_count, trials,
        converged_count as f64 / n * 100.0);
    eprintln!("  Convergence time:   {:.2}s ± {:.2}s  [min={:.1}s, max={:.1}s]",
        mean_conv, std_conv, min_conv, max_conv);
    eprintln!("  Max peers:          {:.2} (of {})", avg_max_peers, config.node_count - 1);
    eprintln!("  Avg peers (steady): {:.2}", avg_avg_peers);
    eprintln!("  Bandwidth:          {:.1} kbps avg", avg_bw);
    eprintln!("  Packets/run:        {:.0} total recv", avg_pkts);
    eprintln!();

    // LaTeX-ready table
    eprintln!("--- LaTeX Table ---");
    eprintln!(r"\begin{{table}}[h]");
    eprintln!(r"\centering");
    eprintln!(r"\caption{{DHT Convergence Benchmark ({} × {}s, {} trials)}}", config.node_count, config.duration_secs, trials);
    eprintln!(r"\begin{{tabular}}{{lrrrr}}");
    eprintln!(r"\toprule");
    eprintln!("Metric & Mean & StdDev & Min & Max \\\\");
    eprintln!(r"\midrule");
    eprintln!("Convergence time (s) & {:.2} & {:.2} & {:.1} & {:.1} \\\\", mean_conv, std_conv, min_conv, max_conv);
    eprintln!("Max peers & {:.2} & -- & {} & {} \\\\", avg_max_peers,
        all_results.iter().map(|r| r.max_peers).min().unwrap_or(0),
        all_results.iter().map(|r| r.max_peers).max().unwrap_or(0));
    eprintln!("Avg peers & {:.2} & -- & {:.2} & {:.2} \\\\", avg_avg_peers,
        all_results.iter().map(|r| r.avg_peers).fold(f64::MAX, f64::min),
        all_results.iter().map(|r| r.avg_peers).fold(0.0f64, f64::max));
    eprintln!("Bandwidth (kbps) & {:.1} & -- & {:.1} & {:.1} \\\\", avg_bw,
        all_results.iter().map(|r| r.bandwidth_kbps).fold(f64::MAX, f64::min),
        all_results.iter().map(|r| r.bandwidth_kbps).fold(0.0f64, f64::max));
    eprintln!(r"\bottomrule");
    eprintln!(r"\end{{tabular}}");
    eprintln!(r"\end{{table}}");

    eprintln!();
    if converged_count == trials as usize {
        eprintln!("  ✅ ALL TRIALS CONVERGED — {:.1}% success rate", 100.0);
    } else {
        eprintln!("  ⚠️  {}/{} trials converged", converged_count, trials);
    }
    eprintln!("═══════════════════════════════════════════════════");
}
