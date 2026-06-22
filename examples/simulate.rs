//! Simulation CLI Entry Point.
//!
//! Reproduce any experiment from the paper:
//!
//! ```text
//! cargo run --example simulate -- --paper-mode --nodes 10 --duration 300
//! ```
//!
//! Or use a pre-configured experiment:
//!
//! ```text
//! cargo run --example simulate -- --config experiments/figure4.toml
//! ```

use std::path::PathBuf;

fn main() {
    let config = match neuron_wire::simulator::parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[SIM] Error: {}", e);
            eprintln!("[SIM] Usage: simulate --nodes N --duration SECS [--paper-mode] [--seed N] [--config PATH] [--output-dir PATH]");
            std::process::exit(1);
        }
    };

    eprintln!("[SIM] Launching {} nodes for {} seconds...", config.node_count, config.duration_secs);
    if config.paper_mode {
        eprintln!("[SIM] PAPER MODE: seed={} (deterministic)", config.seed);
    }

    // Create output directory
    let output_dir = PathBuf::from(
        std::env::args()
            .skip_while(|a| a != "--output-dir")
            .nth(1)
            .unwrap_or_else(|| {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                format!("results/experiment_{}", ts)
            }),
    );

    // Launch and run
    let mut sim = neuron_wire::simulator::Simulator::new(config);
    match sim.launch() {
        Ok(()) => eprintln!("[SIM] All nodes launched. Running..."),
        Err(e) => {
            eprintln!("[SIM] Launch failed: {}", e);
            std::process::exit(1);
        }
    }

    let result = match sim.run() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[SIM] Run failed: {}", e);
            sim.shutdown();
            std::process::exit(1);
        }
    };

    // Write results
    if let Err(e) = sim.write_results(&output_dir, &result) {
        eprintln!("[SIM] Failed to write results: {}", e);
    }

    // Shutdown
    sim.shutdown();

    eprintln!("[SIM] Done. Elapsed={:.1}s ticks={} packets_rx={} packets_tx={}",
        result.duration_secs, result.total_ticks,
        result.total_packets_recv, result.total_packets_sent);
    eprintln!("[SIM] Results → {}", output_dir.display());
}
