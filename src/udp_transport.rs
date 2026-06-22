//! UDP Transport Layer for NWP v2
//!
//! Provides application-level reliability over UDP with minimal overhead.
//! Each UDP datagram carries a 16-byte transport header + framed NWP message.
//!
//! ## Datagram Layout
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │ sequence_number: u32   (4 bytes)                 │
//! │ ack_number: u32        (4 bytes)                 │
//! │ ack_bitfield: u32      (4 bytes)                 │
//! │ packet_timestamp: u32  (4 bytes)                 │  ← 16B Transport Header
//! ├──────────────────────────────────────────────────┤
//! │ frame_len: u32         (4 bytes)                 │
//! │ NWP Header             (16 bytes)                │  ← NWP Frame (from header::build_frame)
//! │ NWP Body               (N bytes)                 │
//! └──────────────────────────────────────────────────┘
//! ```
//!
//! Maximum datagram size: 16 + 4 + 16 + MAX_BODY = well under 1500 MTU for most exchanges.

use std::net::UdpSocket;
use std::time::{Duration, Instant};

// ─── Transport Header ───────────────────────────────────────────

/// Minimal 16-byte UDP transport header prepended to every NWP datagram.
///
/// All fields are little-endian u32.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TransportHeader {
    /// Monotonically increasing sequence number for this sender's outgoing packets
    pub sequence_number: u32,
    /// Highest contiguous sequence number received from the remote peer
    pub ack_number: u32,
    /// Bitmask: bit N = 1 means packet (ack_number - 1 - N) was received
    pub ack_bitfield: u32,
    /// Local millisecond timestamp (for RTT calculation and gradient staleness)
    pub packet_timestamp: u32,
}

impl TransportHeader {
    /// Size of the transport header in bytes
    pub const SIZE: usize = 16;

    /// Maximum number of packets tracked by the bitfield
    pub const BITFIELD_WINDOW: u32 = 32;

    /// Create a new transport header
    pub fn new(seq: u32, ack: u32, bitfield: u32, ts: u32) -> Self {
        TransportHeader {
            sequence_number: seq,
            ack_number: ack,
            ack_bitfield: bitfield,
            packet_timestamp: ts,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.sequence_number.to_le_bytes());
        buf[4..8].copy_from_slice(&self.ack_number.to_le_bytes());
        buf[8..12].copy_from_slice(&self.ack_bitfield.to_le_bytes());
        buf[12..16].copy_from_slice(&self.packet_timestamp.to_le_bytes());
        buf
    }

    /// Deserialize from bytes (zero-copy pointer cast)
    pub fn from_bytes(buf: &[u8]) -> &Self {
        assert!(buf.len() >= Self::SIZE);
        unsafe { &*(buf.as_ptr() as *const TransportHeader) }
    }

    /// Check if a packet with the given sequence number is acknowledged.
    /// `ack_number` is the highest contiguous received sequence.
    /// `ack_bitfield` tracks the 32 packets before that.
    pub fn is_acked(seq: u32, ack_number: u32, ack_bitfield: u32) -> bool {
        if seq > ack_number {
            return false; // Future packet, can't be acked yet
        }
        let diff = ack_number - seq;
        if diff == 0 {
            return true; // This is the ack_number itself
        }
        if diff > Self::BITFIELD_WINDOW {
            return false; // Too old to be in the window
        }
        // Bit (diff - 1) in the bitfield
        (ack_bitfield >> (diff - 1)) & 1 != 0
    }
}

// ─── Full Datagram ──────────────────────────────────────────────

/// A complete UDP datagram = TransportHeader + framed NWP message.
pub struct Datagram {
    /// Transport header
    pub transport: TransportHeader,
    /// The NWP frame (built by header::build_frame — includes its own 4B frame_len + header + body)
    pub nwp_frame: Vec<u8>,
}

impl Datagram {
    /// Serialize to a flat byte buffer ready for UDP send.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(TransportHeader::SIZE + self.nwp_frame.len());
        buf.extend_from_slice(&self.transport.to_bytes());
        buf.extend_from_slice(&self.nwp_frame);
        buf
    }

    /// Parse a received UDP datagram buffer.
    /// Returns (transport_header_ref, nwp_frame_slice).
    pub fn parse(buf: &[u8]) -> Option<(&TransportHeader, &[u8])> {
        if buf.len() < TransportHeader::SIZE {
            return None;
        }
        let transport = TransportHeader::from_bytes(buf);
        let nwp_frame = &buf[TransportHeader::SIZE..];
        Some((transport, nwp_frame))
    }

    /// Total datagram size
    pub fn total_size(&self) -> usize {
        TransportHeader::SIZE + self.nwp_frame.len()
    }
}

// ─── Per-Connection State Machine ───────────────────────────────

/// Tracks the send/receive state for one UDP peer connection.
pub struct ConnectionState {
    /// Peer address (opaque identifier)
    pub peer_id: u64,
    /// Next sequence number to use for outgoing packets
    pub next_seq: u32,
    /// Highest contiguous sequence received from this peer
    pub ack_number: u32,
    /// Bitfield for the 32 packets before ack_number
    pub ack_bitfield: u32,
    /// Packets we sent but haven't received ack for yet (for retransmit)
    sent_packets: Vec<PendingPacket>,
    /// Sequence numbers we've received (for duplicate detection)
    received_seqs: sliding_window::SlidingWindow,
    /// Last time we sent an ACK (to avoid flooding)
    last_ack_send: Instant,
    /// Round-trip time estimate (ms)
    pub rtt_ms: f64,
}

mod sliding_window {
    /// Fixed-size bitfield for duplicate packet detection.
    pub struct SlidingWindow {
        base: u32,
        bits: u64,
    }

    impl SlidingWindow {
        pub fn new() -> Self {
            SlidingWindow { base: 0, bits: 0 }
        }

        /// Returns true if this sequence was ALREADY seen (duplicate).
        /// Returns false if this is a new sequence (not seen before).
        pub fn check_and_mark(&mut self, seq: u32) -> bool {
            if seq > self.base + 63 {
                // New base — shift window
                let shift = (seq - self.base - 64).min(63);
                self.base = self.base + shift;
                self.bits >>= shift;
            }

            if seq < self.base {
                return true; // Too old, likely duplicate
            }

            let offset = (seq - self.base) as usize;
            if offset > 63 {
                // Far future, unlikely but possible
                self.base = seq.saturating_sub(63);
                self.bits = 0;
                return false;
            }

            let mask = 1u64 << offset;
            if self.bits & mask != 0 {
                true // Already seen
            } else {
                self.bits |= mask; // Mark as seen
                false
            }
        }
    }
}

struct PendingPacket {
    seq: u32,
    data: Vec<u8>,
    sent_at: Instant,
    retransmit_count: u32,
    is_spike: bool, // SPIKE frames are never retransmitted
}

impl ConnectionState {
    pub fn new(peer_id: u64) -> Self {
        ConnectionState {
            peer_id,
            next_seq: 1,
            ack_number: 0,
            ack_bitfield: 0,
            sent_packets: Vec::with_capacity(64),
            received_seqs: sliding_window::SlidingWindow::new(),
            last_ack_send: Instant::now(),
            rtt_ms: 0.0,
        }
    }

    /// Build a transport header for an outgoing packet.
    pub fn build_header(&self) -> TransportHeader {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u32;

        TransportHeader::new(
            self.next_seq,
            self.ack_number,
            self.ack_bitfield,
            ts,
        )
    }

    /// Record that we sent a packet (for potential retransmit).
    pub fn record_send(&mut self, data: Vec<u8>, is_spike: bool) {
        let seq = self.next_seq;
        self.next_seq += 1;

        // Only track non-spike packets for retransmit
        if !is_spike && self.sent_packets.len() < 256 {
            self.sent_packets.push(PendingPacket {
                seq,
                data,
                sent_at: Instant::now(),
                retransmit_count: 0,
                is_spike: false,
            });
        }
    }

    /// Process an incoming transport header from a received datagram.
    pub fn receive_header(&mut self, hdr: &TransportHeader) {
        // Update ACK tracking for what THEY have received from US
        let mut i = 0;
        while i < self.sent_packets.len() {
            let p = &self.sent_packets[i];
            if TransportHeader::is_acked(p.seq, hdr.ack_number, hdr.ack_bitfield) {
                // Peer received this — remove from retransmit queue
                // Calculate RTT
                let rtt = p.sent_at.elapsed().as_secs_f64() * 1000.0;
                // Exponential moving average
                self.rtt_ms = self.rtt_ms * 0.9 + rtt * 0.1;
                self.sent_packets.swap_remove(i);
            } else {
                i += 1;
            }
        }

        // Track THEIR sequence number for our outgoing ACKs
        if hdr.sequence_number > self.ack_number {
            // Update ack window
            let diff = hdr.sequence_number - self.ack_number;
            if diff > TransportHeader::BITFIELD_WINDOW + 1 {
                // Big jump — shift the window
                self.ack_bitfield = 0;
            } else if diff > 1 {
                // Shift the bitfield
                self.ack_bitfield <<= diff - 1;
            }
            // Mark this seq as received
            if hdr.sequence_number == self.ack_number + 1 {
                self.ack_number = hdr.sequence_number;
                // Also try to advance ack_number using the bitfield
                self.try_advance_ack();
            } else {
                // Gap — set the bit for this sequence
                let gap = hdr.sequence_number - self.ack_number - 1;
                self.ack_bitfield |= 1 << (gap - 1);
            }
        } else {
            // Old or duplicate sequence — might fill a gap
            let diff = self.ack_number - hdr.sequence_number;
            if diff > 0 && diff <= TransportHeader::BITFIELD_WINDOW {
                self.ack_bitfield |= 1 << (diff - 1);
                self.try_advance_ack();
            }
        }
    }

    /// Try to advance ack_number by scanning the bitfield
    fn try_advance_ack(&mut self) {
        while self.ack_bitfield & 1 != 0 {
            self.ack_number += 1;
            self.ack_bitfield >>= 1;
        }
    }

    /// Check for packets that need retransmit.
    /// Returns a list of packet bodies to resend.
    pub fn get_retransmits(&mut self, _timeout_ms: u64) -> Vec<(u32, Vec<u8>)> {
        let mut retransmits = Vec::new();
        let now = Instant::now();

        // Use RTT-based timeout (3x RTT, min 100ms)
        let timeout = Duration::from_millis(
            (self.rtt_ms * 3.0).max(100.0) as u64
        );

        self.sent_packets.retain(|p| {
            if p.is_spike {
                return false; // Never retransmit spikes
            }
            if p.retransmit_count >= 3 {
                return false; // Max 3 retransmits
            }
            if now - p.sent_at > timeout {
                // Time to retransmit
                retransmits.push((p.seq, p.data.clone()));
                return false; // Remove from pending, will be re-added on actual send
            }
            true
        });

        retransmits
    }

    /// Process a received datagram buffer.
    /// Returns the (nwp_frame_bytes, is_duplicate).
    pub fn receive_datagram<'a>(&mut self, buf: &'a [u8]) -> Option<(&'a [u8], bool)> {
        let (hdr, nwp_frame) = Datagram::parse(buf)?;

        // Check for duplicate
        let is_duplicate = self.received_seqs.check_and_mark(hdr.sequence_number);

        // Process ack info
        self.receive_header(hdr);

        Some((nwp_frame, is_duplicate))
    }
}

// ─── UDP Send/Receive ──────────────────────────────────────────

/// Send a framed NWP message over UDP with reliability tracking.
pub fn udp_send(
    socket: &UdpSocket,
    dest: &std::net::SocketAddr,
    state: &mut ConnectionState,
    nwp_frame: Vec<u8>,
    is_spike: bool,
) -> std::io::Result<()> {
    let header = state.build_header();
    let datagram = Datagram {
        transport: header,
        nwp_frame,
    };

    let bytes = datagram.to_bytes();
    let _seq = state.next_seq;

    // Record for potential retransmit
    state.record_send(datagram.nwp_frame, is_spike);

    socket.send_to(&bytes, dest)?;
    Ok(())
}

/// Receive a UDP datagram and process it through the connection state.
/// Returns (nwp_frame, is_duplicate) if valid.
pub fn udp_recv<'a>(
    socket: &UdpSocket,
    buf: &'a mut [u8],
    state: &'a mut ConnectionState,
) -> std::io::Result<Option<(&'a [u8], bool)>> {
    let (len, _src) = socket.recv_from(buf)?;
    let data = &buf[..len];

    Ok(state.receive_datagram(data))
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_header_roundtrip() {
        let hdr = TransportHeader::new(42, 10, 0xDEAD, 12345678);
        let bytes = hdr.to_bytes();
        let parsed = TransportHeader::from_bytes(&bytes);
        assert_eq!(parsed.sequence_number, 42);
        assert_eq!(parsed.ack_number, 10);
        assert_eq!(parsed.ack_bitfield, 0xDEAD);
        assert_eq!(parsed.packet_timestamp, 12345678);
    }

    #[test]
    fn test_ack_tracking() {
        // ack_number = 100, ack_bitfield indicates packets 99, 98, 96 received (bit 0 marks 99)
        let ack = 100u32;
        let bitfield = 0b101u32; // Bit 0 (ack-1=99) = received, Bit 1 (ack-2=98) = dropped, Bit 2 (ack-3=97) = received? No, 0b101 = bits 0 and 2

        // Wait, let me be more careful:
        // bit N = (ack_number - 1 - N)
        // bit 0 = ack - 1 = 99
        // bit 1 = ack - 2 = 98
        // bit 2 = ack - 3 = 97

        // 0b101 = bits 0 and 2 set
        // So 99 is acked, 98 is NOT, 97 IS acked, 96 is NOT

        assert!(TransportHeader::is_acked(100, ack, bitfield)); // ack_number itself
        assert!(TransportHeader::is_acked(99, ack, bitfield));  // bit 0 set
        assert!(!TransportHeader::is_acked(98, ack, bitfield)); // bit 1 clear
        assert!(TransportHeader::is_acked(97, ack, bitfield));  // bit 2 set
        assert!(!TransportHeader::is_acked(96, ack, bitfield)); // bit 3 clear
        assert!(!TransportHeader::is_acked(101, ack, bitfield)); // future
    }

    #[test]
    fn test_datagram_roundtrip() {
        let nwp_frame = vec![0x4E, 0x57, 0x50, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let datagram = Datagram {
            transport: TransportHeader::new(1, 0, 0, 1000),
            nwp_frame: nwp_frame.clone(),
        };

        let bytes = datagram.to_bytes();
        assert_eq!(bytes.len(), TransportHeader::SIZE + nwp_frame.len());

        let (parsed_hdr, parsed_frame) = Datagram::parse(&bytes).unwrap();
        assert_eq!(parsed_hdr.sequence_number, 1);
        assert_eq!(parsed_frame, &nwp_frame[..]);
    }

    #[test]
    fn test_connection_ack_advance() {
        let mut conn = ConnectionState::new(0x42);
        assert_eq!(conn.ack_number, 0);

        // Receive seq 1
        conn.receive_header(&TransportHeader::new(1, 0, 0, 0));
        assert_eq!(conn.ack_number, 1);

        // Receive seq 2
        conn.receive_header(&TransportHeader::new(2, 0, 0, 0));
        assert_eq!(conn.ack_number, 2);

        // Receive seq 4 (gap, 3 missing)
        conn.receive_header(&TransportHeader::new(4, 0, 0, 0));
        assert_eq!(conn.ack_number, 2); // Can't advance past gap

        // Receive seq 3 (fills gap)
        conn.receive_header(&TransportHeader::new(3, 0, 0, 0));
        assert_eq!(conn.ack_number, 4); // Now advanced!
    }

    #[test]
    fn test_duplicate_detection() {
        let mut conn = ConnectionState::new(0x42);

        // First time seeing seq 5
        let (_, dup1) = conn.receive_datagram(&make_test_datagram(5)).unwrap();
        assert!(!dup1); // Not a duplicate

        // Second time
        let _ = conn.receive_datagram(&make_test_datagram(5));
        let (_, dup2) = conn.receive_datagram(&make_test_datagram(5)).unwrap();
        assert!(dup2); // Now a duplicate
    }

    fn make_test_datagram(seq: u32) -> Vec<u8> {
        let hdr = TransportHeader::new(seq, 0, 0, 0);
        let nwp_data = [0x4E, 0x57, 0x50, 0x00, 0x02, 0x00, 0x00, 0x00]; // minimal NWP header
        let mut buf = Vec::with_capacity(TransportHeader::SIZE + nwp_data.len());
        buf.extend_from_slice(&hdr.to_bytes());
        buf.extend_from_slice(&nwp_data);
        buf
    }
}
