//! Incremental scaling benchmark — saves CSV after every trial.
use std::path::PathBuf;
use std::time::Instant;
use std::fs::{self, OpenOptions};
use std::io::Write;
use neuron_wire::simulator::{SimulationConfig, Simulator};

fn main() {
    // node_count, duration_secs, trials
    let configs = [
        (5u32,  10u64, 25u32),
        (10u32, 15u64, 20u32),
        (25u32, 20u64, 15u32),
        (50u32, 20u64, 6u32),   // shorter duration, fewer trials at 50
    ];

    let base_seed: u64 = 42;
    let output_dir = std::env::args().nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("results/bench-scale"));

    fs::create_dir_all(&output_dir).ok();
    let csv_path = output_dir.join("scaling_results.csv");

    // Write header (overwrite)
    fs::write(&csv_path, "node_count,trial,seed,converged,convergence_time_s,max_peers,avg_peers,bandwidth_kbps,packets_recv,total_ticks,apoptosis_deaths\n")
        .expect("Failed to write CSV header");

    let total_runs: u32 = configs.iter().map(|c| c.2).sum();
    let mut run = 0u32;
    let start_all = Instant::now();

    eprintln!("═══ SCALING BENCH ═══ {} runs, writing to {}", total_runs, csv_path.display());

    for &(node_count, duration, trials) in &configs {
        eprintln!("─── {}n × {}t ({}s) ───", node_count, trials, duration);
        for trial in 0..trials {
            run += 1;
            let seed = base_seed + run as u64;

            let config = SimulationConfig {
                node_count,
                duration_secs: duration,
                seed,
                paper_mode: true,
                ..SimulationConfig::default()
            };

            let mut sim = Simulator::new(config);
            let start = Instant::now();
            if let Err(e) = sim.launch() {
                eprintln!("  [ERR] Launch t{}: {}", trial, e);
                // Write error row
                append_csv(&csv_path, &format!("{},{},{},{},{},{},{},{},{},{},{}\n",
                    node_count, trial, seed, false, "NULL", 0, 0.0, 0.0, 0, 0, 0));
                continue;
            }
            let result = match sim.run() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("  [ERR] Run t{}: {}", trial, e);
                    sim.shutdown();
                    append_csv(&csv_path, &format!("{},{},{},{},{},{},{},{},{},{},{}\n",
                        node_count, trial, seed, false, "NULL", 0, 0.0, 0.0, 0, 0, 0));
                    continue;
                }
            };
            sim.shutdown();

            let elapsed = start.elapsed();
            let conv_str = result.convergence_time_secs
                .map(|t| format!("{:.4}", t))
                .unwrap_or_else(|| "NULL".to_string());

            let line = format!(
                "{},{},{},{},{},{},{},{:.4},{},{},{}\n",
                node_count, trial, seed, result.converged, conv_str,
                result.max_peers, result.avg_peers, result.bandwidth_kbps,
                result.total_packets_recv, result.total_ticks, result.total_apoptosis_deaths,
            );
            append_csv(&csv_path, &line);

            eprintln!(
                "  [{:>2}/{}] {}n t{} {} ct={:.1}s pk={:.0} bw={:.0}kpbs el={:.0}s",
                trial + 1, trials, node_count, trial,
                if result.converged { "✅" } else { "❌" },
                conv_str.parse::<f64>().unwrap_or(0.0),
                result.avg_peers, result.bandwidth_kbps, elapsed.as_secs_f64(),
            );
        }
    }

    let total_elapsed = start_all.elapsed();
    eprintln!("\n═══ DONE → {} ({}s total)", csv_path.display(), total_elapsed.as_secs_f64() as u64);
}

fn append_csv(path: &std::path::Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}
