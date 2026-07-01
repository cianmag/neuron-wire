//! Observability suite — metrics, Prometheus, OpenTelemetry, and dashboard.
#![allow(missing_docs)]
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use neuron_wire::observability::{MetricsRegistry, spawn_dashboard, DashboardConfig};
//!
//! let metrics = MetricsRegistry::new();
//! spawn_dashboard(DashboardConfig::default(), metrics.clone(), trace_collector, shutdown);
//! ```

pub mod dashboard;
pub mod metrics;
pub mod opentelemetry;
pub mod prometheus;

pub use dashboard::{spawn_dashboard, DashboardConfig, DashboardState};
pub use metrics::{MetricsRegistry, MetricsSnapshot, PacketEvent, PeerLatencyStats, MAX_HISTORY, SAMPLE_INTERVAL_TICKS};
pub use opentelemetry::{Span, TraceCollector};
