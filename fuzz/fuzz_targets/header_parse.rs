#![no_main]

use libfuzzer_sys::fuzz_target;
use neuron_wire::header::MessageHeader;

/// Fuzz target: feed random bytes into MessageHeader::from_bytes.
/// The function should never panic — any input is handled gracefully via Result.
fuzz_target!(|data: &[u8]| {
    let _ = MessageHeader::from_bytes(data);
});
