#![no_main]

use libfuzzer_sys::fuzz_target;
use std::panic;

/// Fuzz target: feed random bytes into `header::parse_frame()`.
///
/// `parse_frame` expects a buffer with at least a 16-byte `MessageHeader`
/// followed by the body. Random bytes will exercise:
/// - Short-buffer detection
/// - Magic byte validation
/// - Version check
/// - CRC verification
/// - Body length bounds check
/// - The unsafe pointer cast in `from_bytes`
fuzz_target!(|data: &[u8]| {
    // catch_unwind ensures the fuzzer doesn't die on panics from
    // unchecked arithmetic or unsafe code paths
    let _ = panic::catch_unwind(|| {
        let _ = neuron_wire::header::parse_frame(data);
    });
});
