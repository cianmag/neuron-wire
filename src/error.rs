//! Structured error types for the Neuron Wire Protocol.
//!
//! Replaces `String`-based errors with a proper enum hierarchy.
//! Every error variant carries structured context and implements
//! [`std::error::Error`] + [`Display`] for ergonomic handling.

use std::fmt;

// ─── Transport Errors ──────────────────────────────────────────

/// Errors specific to the UDP transport layer.
#[derive(Debug)]
pub enum TransportError {
    /// Received packet is too short to contain a transport header.
    /// Packet ended before the header could be fully read.
    PacketTooShort {
        /// Bytes actually available.
        actual: usize,
        /// Bytes required by the protocol.
        expected: usize,
    },
    /// Sequence number is invalid or out of window.
    InvalidSequence(u32),
    /// Socket bind or send/recv failure.
    Io(std::io::Error),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::PacketTooShort { actual, expected } => {
                write!(f, "packet too short: {} bytes (need {})", actual, expected)
            }
            TransportError::InvalidSequence(seq) => {
                write!(f, "invalid sequence number: {}", seq)
            }
            TransportError::Io(e) => write!(f, "transport I/O: {}", e),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TransportError::Io(e) => Some(e),
            _ => None,
        }
    }
}

// ─── Error Severity ────────────────────────────────────────────

/// Severity level for an [`NwpError`].
///
/// Used by logging and monitoring to decide whether to alert,
/// throttle, or silently drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    /// Operation cannot proceed; peer should be disconnected.
    Critical,
    /// Unexpected but recoverable; packet dropped, peer retained.
    Warning,
    /// Informational; logged but no action needed.
    Info,
}

// ─── NwpError ──────────────────────────────────────────────────

/// The unified error type for the Neuron Wire Protocol engine.
///
/// Every fallible operation returns `Result<T, NwpError>`. Variants
/// are grouped by subsystem so callers can match broadly (e.g.
/// "was this a security error?") or precisely (e.g. "was the CRC bad?").
///
/// # Examples
///
/// ```rust
/// use neuron_wire::error::{NwpError, ErrorSeverity};
///
/// let err = NwpError::ConnectionRefused { reason: "rate limited".into() };
/// assert_eq!(err.severity(), ErrorSeverity::Warning);
/// assert!(err.to_string().contains("rate limited"));
/// ```
#[derive(Debug)]
#[must_use = "NwpError should be handled, not silently discarded"]
pub enum NwpError {
    // ── Protocol errors ───────────────────────────────────────
    /// Malformed or invalid NWP header.
    Header(crate::header::HeaderError),
    /// Transport-layer error (UDP framing, sequence tracking).
    Transport(TransportError),

    // ── Security errors ───────────────────────────────────────
    /// Ed25519 signature verification failed.
    AuthenticationFailed {
        /// Human-readable reason (e.g. "bad signature from peer X").
        reason: String,
    },
    /// AEAD encryption failed (bad key, nonce overflow, etc.).
    EncryptionFailed {
        /// Human-readable reason.
        reason: String,
    },
    /// AEAD decryption failed (tampered ciphertext, bad key, replay).
    DecryptionFailed {
        /// Human-readable reason.
        reason: String,
    },
    /// Peer exceeded per-peer or global rate limit.
    RateLimited,
    /// Trust score has decayed below usable threshold.
    TrustExpired,

    // ── Connection errors ─────────────────────────────────────
    /// Node has reached maximum peer capacity.
    TooManyPeers {
        /// Configured maximum.
        max: usize,
    },
    /// Single IP address has exceeded its per-IP connection limit.
    PerIPLimit {
        /// The offending IP.
        ip: String,
        /// Configured per-IP maximum.
        max: usize,
    },
    /// Connection refused by the remote peer or local policy.
    ConnectionRefused {
        /// Human-readable reason.
        reason: String,
    },

    // ── DHT errors ────────────────────────────────────────────
    /// DHT operation failed.
    DhtError {
        /// Which DHT operation (e.g. "find_node", "ping").
        operation: String,
        /// Human-readable reason.
        reason: String,
    },

    // ── Config errors ─────────────────────────────────────────
    /// Configuration validation failed.
    ConfigError {
        /// The offending config field name.
        field: String,
        /// Human-readable reason.
        reason: String,
    },

    // ── I/O errors ────────────────────────────────────────────
    /// Standard library I/O error (socket bind, file read, etc.).
    Io(std::io::Error),

    // ── Serialization ─────────────────────────────────────────
    /// Message serialization or deserialization failed.
    Serialization(String),
}

impl fmt::Display for NwpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Protocol
            NwpError::Header(e) => write!(f, "header error: {}", e),
            NwpError::Transport(e) => write!(f, "transport error: {}", e),

            // Security
            NwpError::AuthenticationFailed { reason } => {
                write!(f, "authentication failed: {}", reason)
            }
            NwpError::EncryptionFailed { reason } => {
                write!(f, "encryption failed: {}", reason)
            }
            NwpError::DecryptionFailed { reason } => {
                write!(f, "decryption failed: {}", reason)
            }
            NwpError::RateLimited => write!(f, "rate limited"),
            NwpError::TrustExpired => write!(f, "trust expired"),

            // Connection
            NwpError::TooManyPeers { max } => {
                write!(f, "too many peers (max: {})", max)
            }
            NwpError::PerIPLimit { ip, max } => {
                write!(f, "per-IP limit reached for {} (max: {})", ip, max)
            }
            NwpError::ConnectionRefused { reason } => {
                write!(f, "connection refused: {}", reason)
            }

            // DHT
            NwpError::DhtError { operation, reason } => {
                write!(f, "DHT {} failed: {}", operation, reason)
            }

            // Config
            NwpError::ConfigError { field, reason } => {
                write!(f, "config error in '{}': {}", field, reason)
            }

            // I/O
            NwpError::Io(e) => write!(f, "I/O error: {}", e),

            // Serialization
            NwpError::Serialization(msg) => write!(f, "serialization error: {}", msg),
        }
    }
}

impl std::error::Error for NwpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NwpError::Header(e) => Some(e),
            NwpError::Transport(e) => Some(e),
            NwpError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl NwpError {
    /// Returns the severity level of this error.
    ///
    /// Severity drives logging and monitoring decisions:
    /// - **Critical**: protocol violation or security breach → disconnect peer
    /// - **Warning**: recoverable issue → drop packet, retain peer
    /// - **Info**: expected condition → log only
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: protocol corruption or security failures
            NwpError::Header(_) => ErrorSeverity::Critical,
            NwpError::AuthenticationFailed { .. } => ErrorSeverity::Critical,
            NwpError::EncryptionFailed { .. } => ErrorSeverity::Critical,
            NwpError::DecryptionFailed { .. } => ErrorSeverity::Critical,
            NwpError::TrustExpired => ErrorSeverity::Critical,
            NwpError::Serialization(_) => ErrorSeverity::Critical,
            NwpError::Io(_) => ErrorSeverity::Critical,

            // Warning: recoverable, packet dropped
            NwpError::Transport(_) => ErrorSeverity::Warning,
            NwpError::RateLimited => ErrorSeverity::Warning,
            NwpError::TooManyPeers { .. } => ErrorSeverity::Warning,
            NwpError::PerIPLimit { .. } => ErrorSeverity::Warning,
            NwpError::ConnectionRefused { .. } => ErrorSeverity::Warning,
            NwpError::DhtError { .. } => ErrorSeverity::Warning,

            // Info: expected conditions
            NwpError::ConfigError { .. } => ErrorSeverity::Info,
        }
    }
}

// ─── From Conversions ──────────────────────────────────────────

// Implement std::error::Error for HeaderError so it can serve as a source.
// HeaderError is a local crate type, so we can add this trait impl here.
impl std::error::Error for crate::header::HeaderError {}

impl From<crate::header::HeaderError> for NwpError {
    fn from(e: crate::header::HeaderError) -> Self {
        NwpError::Header(e)
    }
}

impl From<TransportError> for NwpError {
    fn from(e: TransportError) -> Self {
        NwpError::Transport(e)
    }
}

impl From<std::io::Error> for NwpError {
    fn from(e: std::io::Error) -> Self {
        NwpError::Io(e)
    }
}

// ─── Convenience Constructors ──────────────────────────────────

impl NwpError {
    /// Shorthand for a [`ConnectionRefused`](NwpError::ConnectionRefused) error.
    pub fn connection_refused(reason: impl Into<String>) -> Self {
        NwpError::ConnectionRefused {
            reason: reason.into(),
        }
    }

    /// Shorthand for a [`DhtError`](NwpError::DhtError).
    pub fn dht(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        NwpError::DhtError {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// Shorthand for a [`ConfigError`](NwpError::ConfigError).
    pub fn config(field: impl Into<String>, reason: impl Into<String>) -> Self {
        NwpError::ConfigError {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Shorthand for a [`Serialization`](NwpError::Serialization) error.
    pub fn serialization(msg: impl Into<String>) -> Self {
        NwpError::Serialization(msg.into())
    }
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_all_variants() {
        let cases: Vec<(NwpError, &str)> = vec![
            (
                NwpError::Header(crate::header::HeaderError::BadCrc),
                "header error: CRC mismatch",
            ),
            (
                NwpError::Transport(TransportError::PacketTooShort {
                    actual: 8,
                    expected: 16,
                }),
                "transport error: packet too short: 8 bytes (need 16)",
            ),
            (
                NwpError::AuthenticationFailed {
                    reason: "bad sig".into(),
                },
                "authentication failed: bad sig",
            ),
            (
                NwpError::EncryptionFailed {
                    reason: "nonce overflow".into(),
                },
                "encryption failed: nonce overflow",
            ),
            (
                NwpError::DecryptionFailed {
                    reason: "tag mismatch".into(),
                },
                "decryption failed: tag mismatch",
            ),
            (NwpError::RateLimited, "rate limited"),
            (NwpError::TrustExpired, "trust expired"),
            (
                NwpError::TooManyPeers { max: 500 },
                "too many peers (max: 500)",
            ),
            (
                NwpError::PerIPLimit {
                    ip: "1.2.3.4".into(),
                    max: 10,
                },
                "per-IP limit reached for 1.2.3.4 (max: 10)",
            ),
            (
                NwpError::ConnectionRefused {
                    reason: "shutdown".into(),
                },
                "connection refused: shutdown",
            ),
            (
                NwpError::DhtError {
                    operation: "find_node".into(),
                    reason: "timeout".into(),
                },
                "DHT find_node failed: timeout",
            ),
            (
                NwpError::ConfigError {
                    field: "max_peers".into(),
                    reason: "must be > 0".into(),
                },
                "config error in 'max_peers': must be > 0",
            ),
            (
                NwpError::Serialization("bad frame".into()),
                "serialization error: bad frame",
            ),
        ];

        for (err, expected) in &cases {
            let s = format!("{}", err);
            assert!(
                s.contains(expected),
                "Display mismatch: got '{}' expected '{}'",
                s,
                expected
            );
        }
    }

    #[test]
    fn test_severity_levels() {
        assert_eq!(
            NwpError::Header(crate::header::HeaderError::BadCrc).severity(),
            ErrorSeverity::Critical
        );
        assert_eq!(
            NwpError::AuthenticationFailed { reason: "x".into() }.severity(),
            ErrorSeverity::Critical
        );
        assert_eq!(NwpError::RateLimited.severity(), ErrorSeverity::Warning);
        assert_eq!(
            NwpError::TooManyPeers { max: 1 }.severity(),
            ErrorSeverity::Warning
        );
        assert_eq!(
            NwpError::ConfigError {
                field: "x".into(),
                reason: "y".into()
            }
            .severity(),
            ErrorSeverity::Info
        );
    }

    #[test]
    fn test_from_header_error() {
        let header_err = crate::header::HeaderError::BadCrc;
        let nwp_err: NwpError = header_err.into();
        assert!(matches!(nwp_err, NwpError::Header(_)));
        assert_eq!(nwp_err.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe");
        let nwp_err: NwpError = io_err.into();
        assert!(matches!(nwp_err, NwpError::Io(_)));
    }

    #[test]
    fn test_from_transport_error() {
        let t_err = TransportError::PacketTooShort {
            actual: 4,
            expected: 16,
        };
        let nwp_err: NwpError = t_err.into();
        assert!(matches!(nwp_err, NwpError::Transport(_)));
    }

    #[test]
    fn test_convenience_constructors() {
        let e = NwpError::connection_refused("nope");
        assert!(matches!(e, NwpError::ConnectionRefused { .. }));

        let e = NwpError::dht("ping", "timeout");
        assert!(matches!(e, NwpError::DhtError { .. }));

        let e = NwpError::config("bind_addr", "invalid");
        assert!(matches!(e, NwpError::ConfigError { .. }));

        let e = NwpError::serialization("truncated");
        assert!(matches!(e, NwpError::Serialization(_)));
    }

    #[test]
    fn test_must_use_compile() {
        // This test verifies that NwpError has #[must_use].
        // If #[must_use] is removed, the compiler will emit a warning.
        let _err = NwpError::RateLimited;
    }

    #[test]
    fn test_error_trait_source() {
        use std::error::Error;

        let io_err = std::io::Error::other("test");
        let nwp_err: NwpError = io_err.into();
        assert!(nwp_err.source().is_some());

        let header_err = NwpError::Header(crate::header::HeaderError::BadCrc);
        assert!(header_err.source().is_some());

        let rate = NwpError::RateLimited;
        assert!(rate.source().is_none());
    }
}
