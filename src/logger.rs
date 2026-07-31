//! Lightweight structured logger — zero external dependencies.
//!
//! Outputs JSON lines to stderr, filterable by log level. Each line:
//! ```json
//! {"ts":"2026-07-22T14:30:00Z","level":"INFO","module":"engine","msg":"peer connected","peer":"192.168.1.1:9000","event":"peer_connect"}
//! ```
//!
//! Set `NWP_LOG_LEVEL=debug` to enable debug output. Default: `info`.
//! Set `NWP_LOG_JSON=0` to disable JSON and use plain text format.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Log levels in order of severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Fatal or blocking errors.
    Error = 0,
    /// Recoverable problems worth operator attention.
    Warn = 1,
    /// Normal operational information.
    Info = 2,
    /// Verbose diagnostics for developers.
    Debug = 3,
    /// Extremely verbose per-message detail.
    Trace = 4,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }

    fn from_env(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" => Level::Error,
            "warn" | "warning" => Level::Warn,
            "info" => Level::Info,
            "debug" => Level::Debug,
            "trace" => Level::Trace,
            _ => Level::Info,
        }
    }
}

/// Global log level filter. Only messages at or below this level are emitted.
static LOG_LEVEL: AtomicU8 = AtomicU8::new(2); // Info by default
static LOG_JSON: AtomicU8 = AtomicU8::new(1); // JSON by default

/// Initialize the logger from environment variables.
/// Called once at startup (typically in main()).
pub fn init() {
    if let Ok(level_str) = std::env::var("NWP_LOG_LEVEL") {
        let level = Level::from_env(&level_str);
        LOG_LEVEL.store(level as u8, Ordering::Relaxed);
    }
    if let Ok(json_str) = std::env::var("NWP_LOG_JSON") {
        if json_str == "0" || json_str == "false" {
            LOG_JSON.store(0, Ordering::Relaxed);
        }
    }
}

/// Get the current timestamp as ISO 8601.
fn timestamp() -> String {
    let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "1970-01-01T00:00:00Z".to_string();
    };
    let secs = dur.as_secs();
    // Simple conversion — no leap seconds, good enough for logging
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Approximate date from days since 1970-01-01
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 1u32;
    for &md in &month_days {
        if remaining < md as i64 {
            break;
        }
        remaining -= md as i64;
        m += 1;
    }
    let d = remaining + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hours, minutes, seconds
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Escape a string for JSON output (minimal, just handles what we need).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 10);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Core log function. Don't call directly — use the macros.
pub fn log(
    level: Level,
    module: &str,
    msg: &str,
    peer: Option<&str>,
    event: Option<&str>,
    extra: Option<&str>,
) {
    if level > Level::from_u8(LOG_LEVEL.load(Ordering::Relaxed)) {
        return;
    }

    let use_json = LOG_JSON.load(Ordering::Relaxed) == 1;
    let ts = timestamp();

    if use_json {
        // JSON format
        let mut line = format!(
            "{{\"ts\":\"{}\",\"level\":\"{}\",\"module\":\"{}\",\"msg\":\"{}\"",
            ts,
            level.as_str(),
            json_escape(module),
            json_escape(msg),
        );
        if let Some(p) = peer {
            line.push_str(&format!(",\"peer\":\"{}\"", json_escape(p)));
        }
        if let Some(e) = event {
            line.push_str(&format!(",\"event\":\"{}\"", json_escape(e)));
        }
        if let Some(x) = extra {
            line.push_str(&format!(",\"extra\":\"{}\"", json_escape(x)));
        }
        line.push_str("}\n");
        eprint!("{}", line);
    } else {
        // Plain text format
        let prefix = match level {
            Level::Error => "\x1b[31mERROR\x1b[0m",
            Level::Warn => "\x1b[33mWARN\x1b[0m",
            Level::Info => "\x1b[32mINFO\x1b[0m",
            Level::Debug => "\x1b[36mDEBUG\x1b[0m",
            Level::Trace => "\x1b[90mTRACE\x1b[0m",
        };
        let peer_str = peer.map(|p| format!(" [{}]", p)).unwrap_or_default();
        let event_str = event.map(|e| format!(" ({})", e)).unwrap_or_default();
        eprintln!(
            "[{}] {} {}{}{} {}",
            ts, prefix, module, peer_str, event_str, msg
        );
    }
}

impl Level {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Level::Error,
            1 => Level::Warn,
            2 => Level::Info,
            3 => Level::Debug,
            4 => Level::Trace,
            _ => Level::Info,
        }
    }
}

// ─── Convenience macros ─

/// Log a error message.
#[macro_export]
macro_rules! log_error {
    ($module:expr, $msg:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Error,
            $module,
            &$msg,
            None,
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Error,
            $module,
            &$msg,
            Some($peer),
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Error,
            $module,
            &$msg,
            None,
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Error,
            $module,
            &$msg,
            None,
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Error,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Error,
            $module,
            &$msg,
            Some($peer),
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Error,
            $module,
            &$msg,
            None,
            Some($event),
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Error,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            Some($extra),
        );
    };
}

/// Log a warn message.
#[macro_export]
macro_rules! log_warn {
    ($module:expr, $msg:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Warn,
            $module,
            &$msg,
            None,
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Warn,
            $module,
            &$msg,
            Some($peer),
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Warn,
            $module,
            &$msg,
            None,
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Warn,
            $module,
            &$msg,
            None,
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Warn,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Warn,
            $module,
            &$msg,
            Some($peer),
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Warn,
            $module,
            &$msg,
            None,
            Some($event),
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Warn,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            Some($extra),
        );
    };
}

/// Log a info message.
#[macro_export]
macro_rules! log_info {
    ($module:expr, $msg:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Info,
            $module,
            &$msg,
            None,
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Info,
            $module,
            &$msg,
            Some($peer),
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Info,
            $module,
            &$msg,
            None,
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Info,
            $module,
            &$msg,
            None,
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Info,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Info,
            $module,
            &$msg,
            Some($peer),
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Info,
            $module,
            &$msg,
            None,
            Some($event),
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Info,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            Some($extra),
        );
    };
}

/// Log a debug message.
#[macro_export]
macro_rules! log_debug {
    ($module:expr, $msg:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Debug,
            $module,
            &$msg,
            None,
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Debug,
            $module,
            &$msg,
            Some($peer),
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Debug,
            $module,
            &$msg,
            None,
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Debug,
            $module,
            &$msg,
            None,
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Debug,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Debug,
            $module,
            &$msg,
            Some($peer),
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Debug,
            $module,
            &$msg,
            None,
            Some($event),
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Debug,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            Some($extra),
        );
    };
}

/// Log a trace message.
#[macro_export]
macro_rules! log_trace {
    ($module:expr, $msg:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Trace,
            $module,
            &$msg,
            None,
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Trace,
            $module,
            &$msg,
            Some($peer),
            None,
            None,
        );
    };
    ($module:expr, $msg:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Trace,
            $module,
            &$msg,
            None,
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Trace,
            $module,
            &$msg,
            None,
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Trace,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            None,
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Trace,
            $module,
            &$msg,
            Some($peer),
            None,
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Trace,
            $module,
            &$msg,
            None,
            Some($event),
            Some($extra),
        );
    };
    ($module:expr, $msg:expr, peer = $peer:expr, event = $event:expr, extra = $extra:expr) => {
        $crate::logger::log(
            $crate::logger::Level::Trace,
            $module,
            &$msg,
            Some($peer),
            Some($event),
            Some($extra),
        );
    };
}
