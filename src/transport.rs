//! UDP Transport Layer — reliable messaging over unreliable sockets.
//!
//! ## Wire Format
//!
//! Every UDP datagram carries a transport header followed by an NWP message:
//!
//! ```text
//! [0-3]   sequence_number: u32    = local sequence counter (monotonic)
//! [4-7]   ack_number: u32         = last contiguous seq received
//! [8-11]  ack_bitfield: u32       = bitmask of next 32 packets after ack_number
//! [12-15] timestamp: u32          = sender's local time in ms (epoch offset)
//! [16-..] payload: [u8]           = NWP frame (header + body)
//! ```
//!
//! ## ACK Bitfield Mechanics
//!
//! The bitfield acknowledges packets *after* `ack_number`:
//! - Bit 0  = packet (ack_number + 1) was received
//! - Bit 1  = packet (ack_number + 2) was received
//! - ...
//! - Bit 31 = packet (ack_number + 32) was received
//!
//! This covers 33 packets per ACK (ack_number + 32-bit bitfield).
//! Any packet before ack_number is implicitly acknowledged.
//!
//! ## Reliability Policy
//!
//! | Message Type | Retransmit | Priority |
//! |---|---|---|
//! | SPIKE | Never (fire-and-forget) | Best-effort |
//! | COMMAND | Never (predictive, re-issued) | Best-effort |
//! | READINESS | Never (stale after 1 tick) | Best-effort |
//! | DATA (gradients) | Up to 3 retries | Reliable |
//! | CONSENSUS | Up to 5 retries | Reliable |
//! | GOSSIP | Never (next cycle) | Best-effort |

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

// ─── Transport Header ──────────────────────────────────────────

/// 16-byte transport header prepended to every NWP message over UDP.
///
/// Zero-copy compatible: `repr(C)` + `packed` means you can cast
/// directly from the receive buffer (on platforms that support unaligned access).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TransportHeader {
    /// Local sequence number (monotonically increasing per sender)
    pub sequence_number: u32,
    /// Last contiguous sequence number received from the remote peer
    pub ack_number: u32,
    /// Bitmask: bit N = peer received (ack_number + 1 + N)
    pub ack_bitfield: u32,
    /// Local timestamp in milliseconds (for staleness calculation)
    pub timestamp: u32,
}

impl TransportHeader {
    pub const SIZE: usize = 16;

    /// Create a new transport header with the given sequence and ack state
    pub fn new(seq: u32, ack_num: u32, ack_bit: u32, now_ms: u32) -> Self {
        TransportHeader {
            sequence_number: seq,
            ack_number: ack_num,
            ack_bitfield: ack_bit,
            timestamp: now_ms,
        }
    }

    /// Zero-copy: interpret bytes as a TransportHeader
    /// # Safety
    /// Slice must be at least 16 bytes.
    #[inline]
    pub unsafe fn from_bytes(buf: &[u8]) -> &TransportHeader {
        assert!(buf.len() >= Self::SIZE);
        &*(buf.as_ptr() as *const TransportHeader)
    }

    /// Serialize to bytes
    #[inline]
    pub fn to_bytes(&self) -> [u8; 16] {
        unsafe { *(self as *const TransportHeader as *const [u8; 16]) }
    }
}

// ─── Bitfield Tracking ─────────────────────────────────────────

/// Tracks received packet sequence numbers for generating ACKs.
///
/// Uses a sliding window approach. We track the highest contiguous
/// sequence number received and a bitfield of up to 32 packets after it.
pub struct AckTracker {
    /// Highest contiguous sequence received
    last_contiguous: u32,
    /// Bitmask: bit N = packet (last_contiguous + 1 + N) was received
    bitfield: u32,
    /// Largest sequence number ever seen (to detect wrap)
    max_seen: u32,
}

impl AckTracker {
    pub fn new() -> Self {
        AckTracker {
            last_contiguous: 0,
            bitfield: 0,
            max_seen: 0,
        }
    }

    /// Record receipt of a packet with the given sequence number.
    /// Returns true if this is a new (not duplicate) packet.
    pub fn record(&mut self, seq: u32) -> bool {
        if seq <= self.last_contiguous {
            // Already ACKed — duplicate
            return false;
        }

        let offset = seq - self.last_contiguous;

        if offset <= 32 {
            // Within the bitfield window
            if offset == 1 {
                // Exactly the next packet — advance contiguous window
                // All existing bit positions shift right by 1
                // (the base moved, so old bit 1 is now bit 0)
                self.last_contiguous = seq;
                self.bitfield >>= 1;
                self.advance_window();
            } else {
                // Packet in the future (gap)
                let bit_pos = (offset - 1) as u32;
                if self.bitfield & (1 << bit_pos) != 0 {
                    return false; // already received (rare with u32 bitfield)
                }
                self.bitfield |= 1 << bit_pos;
            }
        } else if offset > 32 {
            // Packet is beyond the bitfield window — major gap
            // Shift window forward, mark everything between as missing
            // This is a simplification: we move the window to this packet
            // and mark it as received, leaving a gap in the bitfield
            self.last_contiguous = seq - 32;
            self.bitfield = 1 << 31; // mark only this packet
        }

        if seq > self.max_seen {
            self.max_seen = seq;
        }

        true
    }

    /// After advancing last_contiguous by 1, check if more packets
    /// in the bitfield can now be covered by the contiguous window.
    fn advance_window(&mut self) {
        loop {
            // Check if the first bit in the bitfield corresponds to
            // the next contiguous packet
            if self.bitfield & 1 != 0 {
                // Next packet was already received out-of-order
                self.last_contiguous += 1;
                self.bitfield >>= 1;
            } else {
                break;
            }
        }
    }

    /// Build the ack state to send: (ack_number, ack_bitfield)
    pub fn ack_state(&self) -> (u32, u32) {
        (self.last_contiguous, self.bitfield)
    }

    /// Check if a specific sequence number has been acknowledged
    pub fn is_acked(&self, seq: u32) -> bool {
        if seq <= self.last_contiguous {
            return true;
        }
        let offset = seq - self.last_contiguous;
        if offset <= 32 {
            return (self.bitfield >> (offset - 1)) & 1 != 0;
        }
        false
    }

    /// Number of packets received (approx)
    pub fn total_seen(&self) -> u32 {
        self.max_seen
    }
}

// ─── Reliable Send Queue ───────────────────────────────────────

/// A packet in the reliable send queue, tracking retransmission state
struct ReliablePacket {
    /// The raw bytes to send (transport header + NWP message)
    data: Vec<u8>,
    /// When this packet was first sent
    first_sent: Instant,
    /// Number of times we've retransmitted
    retries: u32,
    /// Max retries before dropping
    max_retries: u32,
    /// Half-life for gradient weight calculation (ms)
    half_life_ms: f32,
}

/// Handles reliable delivery with retransmission and expiry.
pub struct ReliableQueue {
    packets: HashMap<u32, ReliablePacket>,
}

impl ReliableQueue {
    pub fn new() -> Self {
        ReliableQueue {
            packets: HashMap::new(),
        }
    }

    /// Enqueue a packet for reliable delivery
    pub fn enqueue(&mut self, seq: u32, data: Vec<u8>, max_retries: u32, half_life_ms: f32) {
        self.packets.insert(seq, ReliablePacket {
            data,
            first_sent: Instant::now(),
            retries: 0,
            max_retries,
            half_life_ms,
        });
    }

    /// Process an incoming ACK: remove acknowledged packets
    pub fn process_ack(&mut self, ack_number: u32, ack_bitfield: u32) -> Vec<u32> {
        let mut to_remove = Vec::new();
        for (&seq, _packet) in &self.packets {
            if seq <= ack_number {
                to_remove.push(seq);
            } else {
                let offset = (seq - ack_number) as usize;
                if offset <= 32 && (ack_bitfield >> (offset - 1)) & 1 != 0 {
                    to_remove.push(seq);
                }
            }
        }
        for seq in &to_remove {
            self.packets.remove(seq);
        }
        to_remove
    }

    /// Get packets that need retransmission (not expired, retries remaining)
    pub fn get_retransmit_batch(&mut self, now_ms: u32) -> Vec<(u32, Vec<u8>)> {
        let mut batch = Vec::new();
        let mut to_remove = Vec::new();

        for (&seq, packet) in &mut self.packets {
            let age_ms = now_ms.saturating_sub(packet.first_sent.elapsed().as_millis() as u32);

            // Calculate gradient weight — if effectively 0, drop it
            let weight = calculate_gradient_weight(age_ms, packet.half_life_ms);
            if weight < 0.001 {
                to_remove.push(seq);
                continue;
            }

            if packet.retries < packet.max_retries {
                packet.retries += 1;
                batch.push((seq, packet.data.clone()));
            } else {
                to_remove.push(seq);
            }
        }

        for seq in to_remove {
            self.packets.remove(&seq);
        }

        batch
    }

    /// Number of packets still waiting for ACK
    pub fn pending_count(&self) -> usize {
        self.packets.len()
    }

    /// Clean up expired packets
    pub fn cleanup(&mut self, now_ms: u32) {
        let mut to_remove = Vec::new();
        for (&seq, packet) in &self.packets {
            let age_ms = now_ms.saturating_sub(packet.first_sent.elapsed().as_millis() as u32);
            if calculate_gradient_weight(age_ms, packet.half_life_ms) < 0.001 {
                to_remove.push(seq);
            }
        }
        for seq in to_remove {
            self.packets.remove(&seq);
        }
    }
}

// ─── Gradient Weight Calculation ───────────────────────────────

/// Calculate the time-decay weight for a stale gradient.
///
/// Uses exponential decay:
///     weight = e^(-ln(2) * delta_t / half_life)
///
/// At delta_t = half_life: weight = 0.5
/// At delta_t = 10 * half_life: weight ≈ 0.001
pub fn calculate_gradient_weight(age_ms: u32, half_life_ms: f32) -> f32 {
    if half_life_ms <= 0.0 {
        return 0.0;
    }
    let delta_t = age_ms as f32;
    // e^(-ln(2) * dt / half_life)
    (-0.69314718 * delta_t / half_life_ms).exp()
}

// ─── Full UDP Transport ────────────────────────────────────────

/// Complete UDP transport layer combining header, ack tracking, and reliable queue.
///
/// This is the event loop core. Each node runs one of these per peer connection
/// (or one per socket in a multi-peer setup).
pub struct UdpTransport {
    pub socket: UdpSocket,
    /// Local sequence counter (atomic for cross-thread increment)
    local_seq: AtomicU32,
    /// Timestamp offset: milliseconds since this transport was created
    start: Instant,
    /// Incoming ACK tracking
    pub ack_tracker: AckTracker,
    /// Outgoing reliable queue
    pub reliable_queue: ReliableQueue,
    /// Buffer for receiving
    recv_buf: Vec<u8>,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_recv: u64,
    /// Packets sent
    pub packets_sent: u64,
    /// Packets received
    pub packets_recv: u64,
}

impl UdpTransport {
    /// Create a new UDP transport bound to the given address
    pub fn bind(addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(UdpTransport {
            socket,
            local_seq: AtomicU32::new(1),
            start: Instant::now(),
            ack_tracker: AckTracker::new(),
            reliable_queue: ReliableQueue::new(),
            recv_buf: vec![0u8; 65535],
            bytes_sent: 0,
            bytes_recv: 0,
            packets_sent: 0,
            packets_recv: 0,
        })
    }

    /// Current local timestamp in milliseconds
    pub fn now_ms(&self) -> u32 {
        self.start.elapsed().as_millis() as u32
    }

    /// Allocate the next local sequence number
    pub fn next_seq(&self) -> u32 {
        self.local_seq.fetch_add(1, Ordering::Relaxed)
    }

    /// Send an NWP message with best-effort delivery (no retransmit).
    /// Returns the sequence number used.
    pub fn send_best_effort(&mut self, payload: &[u8], dst: &std::net::SocketAddr) -> std::io::Result<u32> {
        let seq = self.next_seq();
        let (ack_num, ack_bit) = self.ack_tracker.ack_state();
        let header = TransportHeader::new(seq, ack_num, ack_bit, self.now_ms());

        // Build datagram: [header][payload]
        let mut datagram = Vec::with_capacity(TransportHeader::SIZE + payload.len());
        datagram.extend_from_slice(&header.to_bytes());
        datagram.extend_from_slice(payload);

        let sent = self.socket.send_to(&datagram, dst)?;
        self.bytes_sent += sent as u64;
        self.packets_sent += 1;
        Ok(seq)
    }

    /// Send an NWP message with reliable delivery (retransmit on loss).
    /// Also enqueues it in the reliable queue.
    pub fn send_reliable(
        &mut self,
        payload: &[u8],
        dst: &std::net::SocketAddr,
        max_retries: u32,
        half_life_ms: f32,
    ) -> std::io::Result<u32> {
        let seq = self.next_seq();
        let (ack_num, ack_bit) = self.ack_tracker.ack_state();
        let header = TransportHeader::new(seq, ack_num, ack_bit, self.now_ms());

        let mut datagram = Vec::with_capacity(TransportHeader::SIZE + payload.len());
        datagram.extend_from_slice(&header.to_bytes());
        datagram.extend_from_slice(payload);

        let sent = self.socket.send_to(&datagram, dst)?;
        self.bytes_sent += sent as u64;
        self.packets_sent += 1;

        // Enqueue for retransmission
        self.reliable_queue.enqueue(seq, datagram, max_retries, half_life_ms);

        Ok(seq)
    }

    /// Try to receive a message. Non-blocking.
    /// Returns None if no message is available.
    pub fn try_recv(&mut self) -> std::io::Result<Option<ReceivedMessage>> {
        match self.socket.recv_from(&mut self.recv_buf) {
            Ok((len, src)) => {
                self.bytes_recv += len as u64;
                self.packets_recv += 1;

                if len < TransportHeader::SIZE {
                    return Ok(None); // too small
                }

                // Zero-copy parse the transport header
                let header = unsafe { TransportHeader::from_bytes(&self.recv_buf[..len]) };

                // Record the sequence number in our ACK tracker
                self.ack_tracker.record(header.sequence_number);

                // Process the ACK this packet carries
                self.reliable_queue.process_ack(header.ack_number, header.ack_bitfield);

                // The payload starts after the transport header
                let payload = &self.recv_buf[TransportHeader::SIZE..len];

                Ok(Some(ReceivedMessage {
                    header: *header,
                    payload: payload.to_vec(),
                    src,
                }))
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// Retransmit any un-ACKed reliable packets that still have retries remaining
    /// and haven't hit 0% utility. Call this periodically (~100ms).
    pub fn retransmit_stale(&mut self, dst: &std::net::SocketAddr) -> std::io::Result<u32> {
        let now_ms = self.now_ms();
        let batch = self.reliable_queue.get_retransmit_batch(now_ms);
        let count = batch.len() as u32;
        for (_seq, data) in batch {
            self.socket.send_to(&data, dst)?;
            self.packets_sent += 1;
            self.bytes_sent += data.len() as u64;
        }
        Ok(count)
    }

    /// Periodic cleanup of expired packets. Call ~every 1s.
    pub fn cleanup_expired(&mut self) {
        let now_ms = self.now_ms();
        self.reliable_queue.cleanup(now_ms);
    }
}

/// A received message with parsed transport header
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    pub header: TransportHeader,
    pub payload: Vec<u8>,
    pub src: std::net::SocketAddr,
}

// ─── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_tracker_contiguous() {
        let mut tracker = AckTracker::new();
        assert!(tracker.record(1));
        assert!(tracker.record(2));
        assert!(tracker.record(3));
        assert_eq!(tracker.ack_state(), (3, 0));
    }

    #[test]
    fn test_ack_tracker_out_of_order() {
        let mut tracker = AckTracker::new();
        assert!(tracker.record(1));
        assert!(tracker.record(3)); // gap at 2
        let (ack_num, bitfield) = tracker.ack_state();
        assert_eq!(ack_num, 1);
        // bit 1 should be set (packet 3 = ack_num + 2)
        assert!(bitfield & (1 << 1) != 0);

        // Now receive packet 2 — should fill the gap
        assert!(tracker.record(2));
        assert_eq!(tracker.ack_state(), (3, 0));
    }

    #[test]
    fn test_ack_tracker_duplicate() {
        let mut tracker = AckTracker::new();
        assert!(tracker.record(1));
        assert!(!tracker.record(1)); // duplicate
    }

    #[test]
    fn test_ack_tracker_jump_ahead() {
        let mut tracker = AckTracker::new();
        // Major jump
        assert!(tracker.record(50));
        let (ack_num, bitfield) = tracker.ack_state();
        // Window shifted: ack_num should be 50-32=18
        assert_eq!(ack_num, 18);
        // bit 31 should be set (packet 50 = ack_num + 32)
        assert!(bitfield & (1 << 31) != 0);
    }

    #[test]
    fn test_gradient_weight() {
        // At half-life: 0.5
        let w = calculate_gradient_weight(100, 100.0);
        assert!((w - 0.5).abs() < 0.01);

        // At time = 0: 1.0
        let w = calculate_gradient_weight(0, 100.0);
        assert!((w - 1.0).abs() < 0.001);

        // At 10 * half-life: ~0.001
        let w = calculate_gradient_weight(1000, 100.0);
        assert!(w < 0.002);
    }

    #[test]
    fn test_transport_header_roundtrip() {
        let h = TransportHeader::new(42, 10, 0xFF, 12345);
        let bytes = h.to_bytes();
        let h2 = unsafe { TransportHeader::from_bytes(&bytes) };
        assert_eq!(h.sequence_number, h2.sequence_number);
        assert_eq!(h.ack_number, h2.ack_number);
        assert_eq!(h.ack_bitfield, h2.ack_bitfield);
        assert_eq!(h.timestamp, h2.timestamp);
    }

    #[test]
    fn test_reliable_queue_ack() {
        let mut queue = ReliableQueue::new();
        queue.enqueue(1, vec![1, 2, 3], 3, 100.0);
        queue.enqueue(2, vec![4, 5, 6], 3, 100.0);
        queue.enqueue(5, vec![7, 8, 9], 3, 100.0);

        // ACK up to 2
        let acked = queue.process_ack(2, 0);
        assert_eq!(acked.len(), 2);
        assert!(acked.contains(&1));
        assert!(acked.contains(&2));
        assert_eq!(queue.pending_count(), 1); // seq 5 still pending
    }

    #[test]
    fn test_reliable_queue_retransmit() {
        let mut queue = ReliableQueue::new();
        queue.enqueue(1, vec![1, 2, 3], 2, 1000.0);

        // First retransmit batch
        let batch = queue.get_retransmit_batch(0);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].0, 1);

        // Should still be pending (retries left)
        assert_eq!(queue.pending_count(), 1);

        // Second retransmit
        let batch = queue.get_retransmit_batch(0);
        assert_eq!(batch.len(), 1);

        // Third retransmit — should max out (max_retries=2)
        let batch = queue.get_retransmit_batch(0);
        assert_eq!(batch.len(), 0);
        assert_eq!(queue.pending_count(), 0); // evicted
    }
}
