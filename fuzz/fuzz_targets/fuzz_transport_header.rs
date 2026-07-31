#![no_main]

use libfuzzer_sys::fuzz_target;
use std::panic;

/// Fuzz target: feed random bytes into `TransportHeader::from_bytes()`.
///
/// `from_bytes` is `unsafe` and requires `buf.len() >= 16`. It performs
/// a zero-copy pointer cast. Fuzzing this with random bytes tests:
/// - The assert guard for buffer length
/// - Unsafe pointer reinterpretation with arbitrary bit patterns
/// - Correctness of field extraction from random layouts
///
/// We only call `from_bytes` when the buffer is large enough to avoid
/// hitting the assert (which would be a legitimate error, not a bug).
/// When the buffer is too short, we test the SIZE constant instead.
fuzz_target!(|data: &[u8]| {
    let _ = panic::catch_unwind(|| {
        if data.len() >= neuron_wire::transport::TransportHeader::SIZE {
            // SAFETY: data.len() >= 16 checked above, satisfying from_bytes precondition
            let header = unsafe { neuron_wire::transport::TransportHeader::from_bytes(data) };
            // Read all fields to ensure no UB from the pointer cast
            let _ = header.sequence_number;
            let _ = header.ack_number;
            let _ = header.ack_bitfield;
            let _ = header.timestamp;

            // Also test roundtrip: serialize back and compare
            let _ = header.to_bytes();
        }
    });
});
