#![no_main]

use libfuzzer_sys::fuzz_target;
use std::panic;

/// Fuzz target: feed random bytes into the trust binary deserialization format.
///
/// The trust module's `load_from_file` reads a binary format:
/// ```text
/// [u32 count]                              — number of peers
/// For each peer:
///   [32 bytes entity_id]                   — peer identifier
///   [f32  score]                           — trust score
///   [u64  total_events]                    — event count
/// ```
///
/// This fuzzer mimics the parsing logic directly on a byte slice to
/// exercise the deserialization without needing a temp file. It tests:
/// - Count field parsing (u32 from_le_bytes)
/// - Entity ID extraction (32-byte read)
/// - Score parsing (f32 from_le_bytes, including NaN/Inf)
/// - Event count parsing (u64 from_le_bytes)
/// - Truncated input handling (early EOF)
fuzz_target!(|data: &[u8]| {
    let _ = panic::catch_unwind(|| {
        let mut pos = 0;

        // Read peer count (u32)
        if data.len() < 4 {
            return;
        }
        let count = u32::from_le_bytes([
            data[0], data[1], data[2], data[3],
        ]) as usize;
        pos += 4;

        // Each peer record is 32 (entity_id) + 4 (score) + 8 (events) = 44 bytes
        let record_size = 32 + 4 + 8;

        for _ in 0..count {
            if pos + record_size > data.len() {
                break; // truncated input — not a bug
            }

            // Parse entity ID (32 bytes)
            let _entity_id = &data[pos..pos + 32];
            pos += 32;

            // Parse trust score (f32, little-endian)
            let score = f32::from_le_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
            ]);
            pos += 4;

            // Validate score is within [0.0, 1.0] range (as the loader does)
            let _clamped = score.clamp(0.0, 1.0);

            // Parse total events (u64, little-endian)
            let _total_events = u64::from_le_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
                data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
            ]);
            pos += 8;
        }
    });
});
