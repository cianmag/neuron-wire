//! STUN (Session Traversal Utilities for NAT) — NAT discovery for NWP nodes.
//!
//! Implements a minimal STUN client (RFC 5389) for discovering the external
//! IP address and port of an NWP node behind a NAT.
//!
//! # Protocol
//!
//! 1. Send a STUN Binding Request to a STUN server (e.g., `stun.l.google.com:19302`)
//! 2. The server responds with a Binding Response containing the client's
//!    public IP:port as an XOR-MAPPED-ADDRESS attribute
//! 3. Parse the response to extract the external address
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::stun::discover_external_address;
//!
//! let addr = discover_external_address(
//!     "stun.l.google.com:19302",
//!     "0.0.0.0:0",
//!     Duration::from_secs(5),
//! );
//! match addr {
//!     Ok(Some(result)) => println!("External: {}", result.external_addr),
//!     Ok(None) => println!("No NAT — address unchanged"),
//!     Err(e) => println!("STUN failed: {e}"),
//! }
//! ```

use rand::Rng;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

// ─── STUN Protocol Constants ───────────────────────────────────

/// STUN magic cookie value (RFC 5389 §6).
const MAGIC_COOKIE: u32 = 0x2112_A442;

/// Binding request message type (RFC 5389 §6).
const BINDING_REQUEST: u16 = 0x0001;

/// Binding response message type (RFC 5389 §6).
const BINDING_RESPONSE: u16 = 0x0101;

/// XOR-MAPPED-ADDRESS attribute type (RFC 5389 §15.2).
const XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// STUN header size (20 bytes).
const STUN_HEADER_SIZE: usize = 20;

/// Transaction ID size (12 bytes).
const TRANSACTION_ID_SIZE: usize = 12;

/// Default STUN server (Google's public STUN server).
pub const DEFAULT_STUN_SERVER: &str = "stun.l.google.com:19302";

/// Timeout for STUN requests.
pub const DEFAULT_STUN_TIMEOUT: Duration = Duration::from_secs(5);

// ─── STUN Client ───────────────────────────────────────────────

/// Result of a STUN binding request.
#[derive(Debug, Clone, PartialEq)]
pub struct StunResult {
    /// The external IP address and port discovered via STUN.
    pub external_addr: SocketAddr,
    /// Round-trip time for the STUN request.
    pub rtt: Duration,
    /// The STUN server used.
    pub server: String,
}

/// Errors that can occur during STUN operations.
#[derive(Debug)]
pub enum StunError {
    /// DNS resolution of STUN server failed.
    DnsLookup(String),
    /// Connection or send/receive error.
    Transport(std::io::Error),
    /// No response received within timeout.
    Timeout,
    /// Invalid STUN response (wrong magic cookie, transaction ID, etc.).
    InvalidResponse(&'static str),
    /// Parsing failed (e.g., malformed attribute).
    ParseError,
}

impl std::fmt::Display for StunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StunError::DnsLookup(s) => write!(f, "DNS lookup failed: {s}"),
            StunError::Transport(e) => write!(f, "transport error: {e}"),
            StunError::Timeout => write!(f, "STUN request timed out"),
            StunError::InvalidResponse(s) => write!(f, "invalid STUN response: {s}"),
            StunError::ParseError => write!(f, "STUN response parse error"),
        }
    }
}

impl std::error::Error for StunError {}

impl From<std::io::Error> for StunError {
    fn from(e: std::io::Error) -> Self {
        StunError::Transport(e)
    }
}

// ─── Core STUN functions ───────────────────────────────────────

/// Build a STUN binding request packet.
///
/// Returns the packet bytes and the transaction ID for verification.
fn build_binding_request() -> (Vec<u8>, [u8; TRANSACTION_ID_SIZE]) {
    let mut packet = Vec::with_capacity(STUN_HEADER_SIZE);

    // Message Type (2 bytes): Binding Request = 0x0001
    packet.extend_from_slice(&BINDING_REQUEST.to_be_bytes());

    // Message Length (2 bytes): 0 for a simple binding request with no attributes
    packet.extend_from_slice(&0u16.to_be_bytes());

    // Magic Cookie (4 bytes)
    packet.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());

    // Transaction ID (12 bytes): random
    let mut transaction_id = [0u8; TRANSACTION_ID_SIZE];
    rand::thread_rng().fill(&mut transaction_id);
    packet.extend_from_slice(&transaction_id);

    (packet, transaction_id)
}

/// Parse a STUN binding response and extract the XOR-MAPPED-ADDRESS.
///
/// # Arguments
///
/// * `data` - The raw UDP datagram received from the STUN server.
/// * `expected_tid` - The transaction ID we sent in the request.
///
/// # Returns
///
/// The external `SocketAddr` (IP:port) as reported by the STUN server.
fn parse_binding_response(
    data: &[u8],
    expected_tid: &[u8; TRANSACTION_ID_SIZE],
) -> Result<SocketAddr, StunError> {
    if data.len() < STUN_HEADER_SIZE {
        return Err(StunError::InvalidResponse("packet too short"));
    }

    // Verify message type is binding response
    let msg_type = u16::from_be_bytes([data[0], data[1]]);
    if msg_type != BINDING_RESPONSE {
        return Err(StunError::InvalidResponse("not a binding response"));
    }

    // Verify magic cookie
    let cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if cookie != MAGIC_COOKIE {
        // RFC 5389 §6: if magic cookie is wrong, it's RFC 3489 (old STUN)
        // We don't support RFC 3489 compatibility mode.
        return Err(StunError::InvalidResponse("wrong magic cookie"));
    }

    // Verify transaction ID
    if &data[8..20] != expected_tid.as_slice() {
        return Err(StunError::InvalidResponse("transaction ID mismatch"));
    }

    // Message length (excluding header)
    let msg_length = u16::from_be_bytes([data[2], data[3]]) as usize;

    // Parse attributes (start after 20-byte header)
    let attr_end = STUN_HEADER_SIZE + msg_length;
    if data.len() < attr_end {
        return Err(StunError::InvalidResponse("truncated attributes"));
    }

    let mut pos = STUN_HEADER_SIZE;
    while pos + 4 <= attr_end {
        let attr_type = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let attr_length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;

        if attr_type == XOR_MAPPED_ADDRESS {
            // Parse XOR-MAPPED-ADDRESS (RFC 5389 §15.2)
            // Format: 1 byte reserved (0x00) | 1 byte family | 2 bytes XOR'd port | N bytes XOR'd address
            let attr_start = pos + 4;
            if attr_start + 4 >= data.len() {
                return Err(StunError::ParseError);
            }

            let _reserved = data[attr_start];
            let family = data[attr_start + 1];

            // Port is XOR'd with the first 2 bytes of the magic cookie (0x2112)
            let xor_port = u16::from_be_bytes([data[attr_start + 2], data[attr_start + 3]]);
            let port = xor_port ^ (MAGIC_COOKIE as u16);

            match family {
                0x01 => {
                    // IPv4: 4 bytes, XOR'd with MAGIC_COOKIE
                    if attr_start + 8 > data.len() {
                        return Err(StunError::ParseError);
                    }
                    let cookie_bytes = MAGIC_COOKIE.to_be_bytes();

                    let mut ip_bytes = [0u8; 4];
                    for i in 0..4 {
                        ip_bytes[i] = data[attr_start + 4 + i] ^ cookie_bytes[i];
                    }

                    let ip = std::net::Ipv4Addr::from(ip_bytes);
                    return Ok(SocketAddr::V4(std::net::SocketAddrV4::new(ip, port)));
                }
                0x02 => {
                    // IPv6: 16 bytes, XOR'd with (magic_cookie || transaction_id)
                    if attr_start + 20 > data.len() {
                        return Err(StunError::ParseError);
                    }
                    let mut xor_key = [0u8; 16];
                    xor_key[..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
                    xor_key[4..].copy_from_slice(expected_tid);

                    let mut ip_bytes = [0u8; 16];
                    for i in 0..16 {
                        ip_bytes[i] = data[attr_start + 4 + i] ^ xor_key[i];
                    }

                    let ip = std::net::Ipv6Addr::from(ip_bytes);
                    return Ok(SocketAddr::V6(std::net::SocketAddrV6::new(ip, port, 0, 0)));
                }
                _ => {
                    return Err(StunError::ParseError);
                }
            }
        }

        // Advance past this attribute (pad to 4-byte boundary)
        pos += 4 + ((attr_length + 3) & !3);
    }

    Err(StunError::InvalidResponse(
        "no XOR-MAPPED-ADDRESS attribute",
    ))
}

/// Discover the external (public) IP address and port via STUN.
///
/// Sends a STUN binding request to the given server and parses the
/// response to extract the mapped address.
///
/// # Arguments
///
/// * `stun_server` - STUN server address in "host:port" format
///   (e.g., "stun.l.google.com:19302").
/// * `bind_addr` - Local address to bind the temporary socket to
///   (e.g., "0.0.0.0:0" for any available port).
/// * `timeout` - How long to wait for a response.
///
/// # Returns
///
/// * `Ok(Some(result))` — Successfully discovered external address.
/// * `Ok(None)` — STUN server responded but reported the address
///   is the same as the local address (no NAT).
/// * `Err(StunError)` — STUN failed (timeout, DNS, parse, etc.).
pub fn discover_external_address(
    stun_server: &str,
    bind_addr: &str,
    timeout: Duration,
) -> Result<Option<StunResult>, StunError> {
    // Resolve STUN server address
    let server_addr = stun_server
        .to_socket_addrs()
        .map_err(|e| StunError::DnsLookup(e.to_string()))?
        .next()
        .ok_or_else(|| StunError::DnsLookup("no addresses resolved".to_string()))?;

    // Create a temporary UDP socket
    let socket = UdpSocket::bind(bind_addr)?;
    socket.set_read_timeout(Some(timeout))?;
    let local_addr = socket.local_addr()?;

    let start = Instant::now();

    // Build and send the binding request
    let (request, tid) = build_binding_request();
    socket.send_to(&request, server_addr)?;

    // Wait for the response
    let mut buf = [0u8; 1024];
    let (received, responder) = match socket.recv_from(&mut buf) {
        Ok((n, addr)) => (n, addr),
        Err(ref e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            return Err(StunError::Timeout);
        }
        Err(e) => return Err(StunError::Transport(e)),
    };

    let rtt = start.elapsed();

    // Verify the response came from the STUN server
    if responder != server_addr {
        // Accept if IP matches even if ephemeral port differs (some servers use different source ports)
        let responder_ip = responder.ip();
        let server_ip = server_addr.ip();
        if responder_ip != server_ip {
            return Err(StunError::InvalidResponse("response from wrong server"));
        }
    }

    // Parse the response
    let external = parse_binding_response(&buf[..received], &tid)?;

    // If the external address equals the local address, there's no NAT
    if external.ip() == local_addr.ip() && external.port() == local_addr.port() {
        return Ok(None);
    }

    Ok(Some(StunResult {
        external_addr: external,
        rtt,
        server: stun_server.to_string(),
    }))
}

/// Convenience wrapper using the default STUN server.
///
/// Uses `stun.l.google.com:19302` with a 5-second timeout.
pub fn discover_default() -> Result<Option<StunResult>, StunError> {
    discover_external_address(DEFAULT_STUN_SERVER, "0.0.0.0:0", DEFAULT_STUN_TIMEOUT)
}

// ─── Integration helpers for UdpTransport ──────────────────────

/// NAT type classification based on STUN behavior.
///
/// A full RFC 3489 classification requires multiple STUN requests with
/// different destinations. This module provides a simplified check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NatType {
    /// No NAT — host has a public IP.
    None,
    /// Full-cone NAT or address-restricted cone.
    Cone,
    /// Symmetric NAT (port-restricted, each destination gets a different mapping).
    Symmetric,
    /// Unknown or unable to determine.
    Unknown,
}

impl std::fmt::Display for NatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatType::None => write!(f, "No NAT (public IP)"),
            NatType::Cone => write!(f, "Cone NAT"),
            NatType::Symmetric => write!(f, "Symmetric NAT"),
            NatType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Perform a simple NAT classification using two STUN servers.
///
/// If the XOR-MAPPED-ADDRESS differs between two STUN servers, the NAT
/// is likely symmetric. If it's the same, it's likely a cone NAT.
pub fn classify_nat(
    primary_server: &str,
    secondary_server: &str,
    bind_addr: &str,
    timeout: Duration,
) -> Result<NatType, StunError> {
    let result1 = discover_external_address(primary_server, bind_addr, timeout);
    let result2 = discover_external_address(secondary_server, bind_addr, timeout);

    match (result1, result2) {
        (Ok(Some(r1)), Ok(Some(r2))) => {
            if r1.external_addr == r2.external_addr {
                Ok(NatType::Cone)
            } else {
                Ok(NatType::Symmetric)
            }
        }
        (Ok(None), _) | (_, Ok(None)) => Ok(NatType::None),
        (Ok(Some(_)), Err(_)) => Ok(NatType::Cone), // At least one worked
        (Err(_), Ok(Some(_))) => Ok(NatType::Cone),
        (Err(_), Err(_)) => Ok(NatType::Unknown),
    }
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_binding_response_ipv4() {
        // Build a synthetic STUN binding response with XOR-MAPPED-ADDRESS
        let tid: [u8; 12] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
        ];
        let magic_bytes = MAGIC_COOKIE.to_be_bytes();

        // IP = 192.168.1.1, Port = 54321
        let test_ip = [192, 168, 1, 1];
        let test_port = 54321u16;

        // Build the XOR-MAPPED-ADDRESS attribute
        // XOR'd port = port ^ (MAGIC_COOKIE as u16)
        let xor_port = test_port ^ (MAGIC_COOKIE as u16);
        // XOR'd IP bytes for IPv4
        let mut xor_ip = [0u8; 4];
        for i in 0..4 {
            xor_ip[i] = test_ip[i] ^ magic_bytes[i];
        }

        let mut packet = Vec::new();

        // STUN header (20 bytes)
        packet.extend_from_slice(&BINDING_RESPONSE.to_be_bytes()); // msg type
        packet.extend_from_slice(&8u16.to_be_bytes()); // length (1 attr of 8 bytes)
        packet.extend_from_slice(&magic_bytes); // magic cookie
        packet.extend_from_slice(&tid); // transaction ID

        // XOR-MAPPED-ADDRESS attribute (type 0x0020, length 8)
        packet.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
        packet.extend_from_slice(&8u16.to_be_bytes()); // length
        packet.push(0x00); // reserved
        packet.push(0x01); // family = IPv4
        packet.extend_from_slice(&xor_port.to_be_bytes()); // XOR'd port
        packet.extend_from_slice(&xor_ip); // XOR'd IP

        let result = parse_binding_response(&packet, &tid).expect("should parse");
        assert_eq!(result.port(), test_port, "port mismatch");
        assert_eq!(result.ip().to_string(), "192.168.1.1", "IP mismatch");
    }

    #[test]
    fn test_parse_binding_response_ipv6() {
        let tid: [u8; 12] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
        ];

        // IPv6 address: 2001:db8::1
        let test_ip = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let test_port = 12345u16;

        // XOR key for IPv6: MAGIC_COOKIE (4 bytes) || transaction_id (12 bytes)
        let mut xor_key = [0u8; 16];
        xor_key[..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
        xor_key[4..].copy_from_slice(&tid);

        let xor_port = test_port ^ (MAGIC_COOKIE as u16);
        let mut xor_ip = [0u8; 16];
        for i in 0..16 {
            xor_ip[i] = test_ip[i] ^ xor_key[i];
        }

        let mut packet = Vec::new();
        packet.extend_from_slice(&BINDING_RESPONSE.to_be_bytes());
        packet.extend_from_slice(&20u16.to_be_bytes()); // 20 bytes (1 IPv6 attr)
        packet.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        packet.extend_from_slice(&tid);

        // XOR-MAPPED-ADDRESS for IPv6
        packet.extend_from_slice(&XOR_MAPPED_ADDRESS.to_be_bytes());
        packet.extend_from_slice(&20u16.to_be_bytes()); // length
        packet.push(0x00);
        packet.push(0x02); // family = IPv6
        packet.extend_from_slice(&xor_port.to_be_bytes());
        packet.extend_from_slice(&xor_ip);

        let result = parse_binding_response(&packet, &tid).expect("should parse IPv6");
        assert_eq!(result.port(), test_port, "IPv6 port mismatch");
        assert_eq!(result.ip().to_string(), "2001:db8::1", "IPv6 IP mismatch");
    }

    #[test]
    fn test_rejects_wrong_tid() {
        let tid: [u8; 12] = [1; 12];
        let wrong_tid: [u8; 12] = [2; 12];
        let magic_bytes = MAGIC_COOKIE.to_be_bytes();

        let mut packet = Vec::new();
        packet.extend_from_slice(&BINDING_RESPONSE.to_be_bytes());
        packet.extend_from_slice(&0u16.to_be_bytes());
        packet.extend_from_slice(&magic_bytes);
        packet.extend_from_slice(&tid);

        let result = parse_binding_response(&packet, &wrong_tid);
        assert!(result.is_err(), "wrong TID should be rejected");
        assert!(
            matches!(result, Err(StunError::InvalidResponse(_))),
            "should be InvalidResponse"
        );
    }

    #[test]
    fn test_rejects_truncated_packet() {
        let tid: [u8; 12] = [1; 12];
        let result = parse_binding_response(&[0u8; 10], &tid);
        assert!(result.is_err(), "truncated should be rejected");
    }

    #[test]
    fn test_build_binding_request_structure() {
        let (packet, tid) = build_binding_request();
        assert_eq!(packet.len(), STUN_HEADER_SIZE, "header size");

        // Verify message type
        let msg_type = u16::from_be_bytes([packet[0], packet[1]]);
        assert_eq!(msg_type, BINDING_REQUEST, "should be binding request");

        // Verify length field = 0
        let length = u16::from_be_bytes([packet[2], packet[3]]);
        assert_eq!(length, 0, "length should be 0");

        // Verify magic cookie
        let cookie = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
        assert_eq!(cookie, MAGIC_COOKIE, "magic cookie");

        // Verify transaction ID is non-zero (unlikely to be all zeros)
        assert_ne!(tid, [0u8; 12], "TID should be random");

        // Verify transaction ID is in packet
        assert_eq!(&packet[8..20], &tid[..], "TID in packet");
    }
}
