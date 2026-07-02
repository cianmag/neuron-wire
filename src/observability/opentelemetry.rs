//! OpenTelemetry integration — spans, traces, and OTLP export.
#![allow(missing_docs)]
//!
//! Provides structured tracing for the engine loop, DHT operations,
//! secure channel handshakes, and ML computation. Exports to any
//! OTLP-compatible collector (Grafana Tempo, Jaeger, Datadog, etc.)
//! via OTLP gRPC or HTTP.
//!
//! ## Architecture
//!
//! Each engine tick produces a span. Child spans are created for:
//!   - Phase 1: UDP drain (recv + parse)
//!   - Phase 2: Outbound drain (send)
//!   - Phase 3: Neural computation (forward pass + hebbian + ML)
//!   - Phase 4: Retransmit
//!   - Phase 5: Cleanup
//!
//! Security operations (handshake, sign, verify) get independent spans.
//!
//! ## Usage (when opentelemetry crate is available)
//!
//! ```rust,ignore
//! use opentelemetry::{global, KeyValue};
//! use opentelemetry_otlp::WithExportConfig;
//! ```
//!
//! Currently provides the span data structures and a stub collector
//! that prints JSON-encoded spans to stderr (development mode).
//! Full OTLP export requires the `opentelemetry` and `opentelemetry_otlp`
//! crates (not added by default to keep deps minimal).

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Span Data ─────────────────────────────────────────────────

/// A single OpenTelemetry-compatible span.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Span {
    /// Span name (e.g. "engine_tick", "dht_ping", "handshake")
    pub name: String,
    /// Trace ID (hex, 32 chars for 128-bit)
    pub trace_id: String,
    /// Span ID (hex, 16 chars for 64-bit)
    pub span_id: String,
    /// Parent span ID (empty string = root span)
    pub parent_span_id: String,
    /// Start time (nanoseconds since epoch)
    pub start_time_ns: u128,
    /// End time (nanoseconds since epoch)
    pub end_time_ns: u128,
    /// Attributes (key-value pairs)
    pub attributes: HashMap<String, String>,
    /// Status: "OK", "ERROR", or "UNSET"
    pub status: String,
    /// Status description (error message, if any)
    pub status_description: String,
}

impl Span {
    pub fn new(name: &str, trace_id: &str, parent_span_id: &str) -> Self {
        Span {
            name: name.to_string(),
            trace_id: trace_id.to_string(),
            span_id: generate_span_id(),
            parent_span_id: parent_span_id.to_string(),
            start_time_ns: now_ns(),
            end_time_ns: 0,
            attributes: HashMap::new(),
            status: "UNSET".to_string(),
            status_description: String::new(),
        }
    }

    pub fn finish(&mut self) {
        self.end_time_ns = now_ns();
    }

    pub fn set_error(&mut self, msg: &str) {
        self.status = "ERROR".to_string();
        self.status_description = msg.to_string();
    }

    pub fn set_ok(&mut self) {
        self.status = "OK".to_string();
    }

    pub fn attr(&mut self, key: &str, value: &str) {
        self.attributes.insert(key.to_string(), value.to_string());
    }
}

// ─── Trace Collector ───────────────────────────────────────────

/// Simple in-process trace collector that buffers spans and optionally
/// exports them via stderr (JSON), with a hook for OTLP export.
pub struct TraceCollector {
    /// Buffered spans (ring buffer, max MAX_SPANS)
    spans: Vec<Span>,
    /// Active trace (stack of spans for nesting)
    trace_stack: Vec<Span>,
    /// Current trace ID
    trace_id: String,
}

const MAX_SPANS: usize = 10_000;

impl TraceCollector {
    pub fn new() -> Self {
        TraceCollector {
            spans: Vec::with_capacity(1024),
            trace_stack: Vec::new(),
            trace_id: generate_trace_id(),
        }
    }

    /// Start a new trace (new trace ID, clears stack).
    pub fn start_trace(&mut self) -> String {
        self.trace_id = generate_trace_id();
        self.trace_stack.clear();
        self.trace_id.clone()
    }

    /// Begin a span, pushing it onto the stack.
    /// Returns the span ID.
    pub fn begin_span(&mut self, name: &str) -> String {
        let parent_id = self
            .trace_stack
            .last()
            .map(|s| s.span_id.clone())
            .unwrap_or_default();

        let mut span = Span::new(name, &self.trace_id, &parent_id);

        // Add trace-level attributes
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        span.attr("timestamp_ns", &now.as_nanos().to_string());

        let span_id = span.span_id.clone();
        self.trace_stack.push(span);
        span_id
    }

    /// End the current span (pops from stack).
    pub fn end_span(&mut self) {
        if let Some(mut span) = self.trace_stack.pop() {
            span.finish();
            span.set_ok();
            // Buffer for export
            if self.spans.len() >= MAX_SPANS {
                self.spans.remove(0);
            }
            self.spans.push(span);
        }
    }

    /// End the current span with an error.
    pub fn end_span_error(&mut self, msg: &str) {
        if let Some(mut span) = self.trace_stack.pop() {
            span.finish();
            span.set_error(msg);
            if self.spans.len() >= MAX_SPANS {
                self.spans.remove(0);
            }
            self.spans.push(span);
        }
    }

    /// Export all buffered spans as JSON (stderr).
    /// In production, this would send via OTLP exporter.
    pub fn export_json(&self) {
        if self.spans.is_empty() {
            return;
        }
        if let Ok(json) = serde_json::to_string(&self.spans) {
            eprintln!("[OTEL] spans: {}", json);
        }
    }

    /// Get buffered spans for dashboard / debug endpoint.
    pub fn get_spans(&self) -> &[Span] {
        &self.spans
    }

    /// Number of buffered spans.
    pub fn span_count(&self) -> usize {
        self.spans.len()
    }

    /// Clear buffered spans.
    pub fn clear(&mut self) {
        self.spans.clear();
    }
}

impl Default for TraceCollector {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Helpers ───────────────────────────────────────────────────

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn generate_trace_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:032x}", nanos)
}

fn generate_span_id() -> String {
    use rand::Rng;
    let id: u64 = rand::thread_rng().gen();
    format!("{:016x}", id)
}
