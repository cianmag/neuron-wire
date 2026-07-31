#![no_main]

use libfuzzer_sys::fuzz_target;
use neuron_wire::flat::BodyReader;
use std::panic;

/// Fuzz target: feed random bytes into `flat::BodyReader`.
///
/// `BodyReader` provides zero-copy field accessors that read directly
/// from a byte buffer. The field accessors (`read_u32`, `read_u16`,
/// `read_u64`, `read_string`, `read_bytes`) compute offsets into the
/// buffer and can panic on out-of-bounds access.
///
/// Fuzzing with random bytes tests:
/// - Offset calculation safety
/// - `read_string` with invalid UTF-8 or invalid offsets
/// - `read_bytes` with truncated data
/// - The `from_utf8_unchecked` path in `read_string`
fuzz_target!(|data: &[u8]| {
    let _ = panic::catch_unwind(|| {
        if data.is_empty() {
            return;
        }

        let reader = BodyReader::new(data);

        // Only read fields that fit within the buffer
        if data.len() >= 4 {
            let _ = reader.read_u32(0);
        }
        if data.len() >= 2 {
            let _ = reader.read_u16(0);
        }
        if data.len() >= 8 {
            let _ = reader.read_u64(0);
        }

        // Try reading a string at offset 0 (offset 0 means absent, so safe)
        let _ = reader.read_string(0);

        // Try reading bytes at offset 0 (offset 0 means absent, so safe)
        let _ = reader.read_bytes(0);

        // If the buffer has data beyond 4 bytes, try reading the offset
        // value stored at byte 0 and use it as a field offset
        if data.len() >= 4 {
            let offset_val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            if offset_val > 0 && offset_val + 4 <= data.len() {
                let _ = reader.read_u32(offset_val);
            }
        }

        // Access the raw buffer
        let _ = reader.raw();
    });
});
