//! Neuron Protocol Demo — Zero-Copy FlatBuffer Pipeline
//!
//! Demonstrates the full message lifecycle:
//!   1. Build a COMMAND body using BodyBuilder (FlatBuffer-style)
//!   2. Frame it with MessageHeader
//!   3. Parse it zero-copy using BodyReader
//!   4. Build a READINESS response
//!   5. Build a DATA message with payload
//!   6. Verify CRC integrity
//!
//! Run: cargo run --example zero_copy_demo

use neuron_wire::*;

fn main() {
    println!("═══ NEURON PROTOCOL v2 — ZERO-COPY FLATBUFFER DEMO ═══\n");

    // ─── Step 1: Build a COMMAND message ──────────────────────
    println!("[COMMAND BRAIN] Building prediction command...");

    let mut bb = flat::BodyBuilder::new(types::cmd::SIZE);

    // Write fixed-size scalar fields at their known offsets
    bb.write_u32(types::cmd::COMMAND_ID, 42);
    bb.write_u32(types::cmd::PREDICTION_CODE, types::prediction::CODE);
    bb.write_u32(types::cmd::CONFIDENCE, types::conf_to_raw(0.92));
    bb.write_u32(types::cmd::CONTEXT_HASH, 0xDEAD_BEEF);
    bb.write_u32(types::cmd::DEADLINE_US, 500_000);
    bb.write_u64(types::cmd::SOURCE_ID, 0x0000_0000_0000_0001);
    bb.write_u32(
        types::cmd::TARGET_MASK,
        types::regions::REASONING | types::regions::LANGUAGE,
    );

    // Optional: add a name string to the data area
    let name_offset = bb.push_data(b"code_generation_task");
    bb.write_u32(types::cmd::NAME_OFFSET, name_offset);

    let body = bb.finish();
    assert_eq!(body.len() as u32, types::cmd::SIZE as u32 + 4 + 20); // fixed + length prefix + "code_generation_task"

    // Frame it
    let frame = header::build_frame(types::MsgType::Command as u8, body, 0);

    println!("  Frame size: {} bytes", frame.len());
    println!("  Command built ✅\n");

    // ─── Step 2: Zero-copy parse ──────────────────────────────
    println!("[REGION] Receiving frame — zero-copy parsing...");

    let (hdr, body_bytes) = header::parse_frame(&frame[4..]).unwrap(); // skip frame length prefix
    let reader = flat::BodyReader::new(body_bytes);

    let cmd_id = reader.read_u32(types::cmd::COMMAND_ID);
    let pred_code = reader.read_u32(types::cmd::PREDICTION_CODE);
    let confidence = types::conf_from_raw(reader.read_u32(types::cmd::CONFIDENCE));
    let source = reader.read_u64(types::cmd::SOURCE_ID);
    let target_mask = reader.read_u32(types::cmd::TARGET_MASK);
    let name = reader.read_string(types::cmd::NAME_OFFSET);

    println!("  Message type:   {:?}", hdr.msg_type);
    println!("  Command ID:     {}", cmd_id);
    println!(
        "  Prediction:     {} ({})",
        types::prediction::name(pred_code),
        pred_code
    );
    println!("  Confidence:     {:.1}%", confidence * 100.0);
    println!("  Source neuron:  0x{:016X}", source);
    println!("  Target mask:    0x{:08X}", target_mask);
    println!("  Name:           {:?}", name);
    println!("  Parsed with ZERO allocations ✅\n");

    // ─── Step 3: Region reports readiness ─────────────────────
    println!("[REGION] Reporting readiness...");

    let mut rb = flat::BodyBuilder::new(types::readiness::SIZE);
    rb.write_u64(types::readiness::NEURON_ID, 0x0000_0000_0000_0002);
    rb.write_u32(types::readiness::COMMAND_ID, cmd_id);
    rb.write_u32(types::readiness::LATENCY_US, 5_000);
    rb.write_u32(types::readiness::CACHE_HIT, 1); // true

    let r_body = rb.finish();
    let r_frame = header::build_frame(types::MsgType::Readiness as u8, r_body, 0);

    // Parse readiness zero-copy
    let (_, r_bytes) = header::parse_frame(&r_frame[4..]).unwrap();
    let r_reader = flat::BodyReader::new(r_bytes);

    println!(
        "  Neuron:         0x{:016X}",
        r_reader.read_u64(types::readiness::NEURON_ID)
    );
    println!(
        "  Command:        {}",
        r_reader.read_u32(types::readiness::COMMAND_ID)
    );
    println!(
        "  Latency:        {}μs",
        r_reader.read_u32(types::readiness::LATENCY_US)
    );
    println!(
        "  Cache hit:      {}\n",
        r_reader.read_u32(types::readiness::CACHE_HIT) != 0
    );

    // ─── Step 4: Send data payload ────────────────────────────
    println!("[COMMAND BRAIN] Sending activation data...");

    let payload = b"Hello from Neuron 0x0001 - activating language region";
    let payload_len = payload.len() as u32;

    let mut db = flat::BodyBuilder::new(types::data::HEADER_SIZE);
    db.write_u64(types::data::SENDER_ID, 0x0000_0000_0000_0001);

    // Compute CRC of the payload
    let data_crc = crc::crc32(payload);
    db.write_u32(types::data::DATA_HASH, data_crc);
    db.write_u16(types::data::CONTENT_TYPE, types::content_type::TEXT);
    db.write_u16(types::data::COMPRESSION, types::compression::NONE);
    db.write_u32(types::data::ORIGINAL_LEN, payload_len);
    db.write_u32(types::data::PAYLOAD_LEN, payload_len);

    // In a real message, the payload bytes would follow the fixed header in the body buffer.
    // For this demo, we append them manually after the fixed header.
    let mut d_body = db.finish();
    d_body.extend_from_slice(payload);

    let d_frame = header::build_frame(types::MsgType::Data as u8, d_body, 0);

    // Parse data message zero-copy
    let (_, d_bytes) = header::parse_frame(&d_frame[4..]).unwrap();
    let d_reader = flat::BodyReader::new(d_bytes);

    // DataHeader fields
    let sender = d_reader.read_u64(types::data::SENDER_ID);
    let stored_crc = d_reader.read_u32(types::data::DATA_HASH);
    let ct = d_reader.read_u16(types::data::CONTENT_TYPE);
    let original_len = d_reader.read_u32(types::data::ORIGINAL_LEN);
    let actual_len = d_reader.read_u32(types::data::PAYLOAD_LEN);

    // The payload starts right after the fixed header
    let payload_slice =
        &d_bytes[types::data::HEADER_SIZE..types::data::HEADER_SIZE + actual_len as usize];

    // Verify CRC
    let computed_crc = crc::crc32(payload_slice);
    let crc_valid = computed_crc == stored_crc;

    println!("  Sender:         0x{:016X}", sender);
    println!("  Content type:   {}", ct);
    println!(
        "  Payload:        {} bytes (original: {})",
        actual_len, original_len
    );
    println!(
        "  Data:           \"{}\"",
        std::str::from_utf8(payload_slice).unwrap()
    );
    println!("  CRC stored:     0x{:08X}", stored_crc);
    println!("  CRC computed:   0x{:08X}", computed_crc);
    println!("  CRC valid:      {}\n", crc_valid);

    // ─── Step 5: Consensus demo ──────────────────────────────
    println!("[NETWORK] Consensus vote...");

    let mut cb = flat::BodyBuilder::new(types::consensus::SIZE);
    cb.write_u64(types::consensus::PROPOSAL_ID, 0x1234);
    cb.write_u64(types::consensus::VOTER_ID, 0x0000_0000_0000_0002);
    cb.write_u32(types::consensus::CONFIDENCE, types::conf_to_raw(0.95));
    cb.write_u32(types::consensus::FLAGS, 1); // vote YES

    let c_body = cb.finish();
    let c_frame = header::build_frame(types::MsgType::Consensus as u8, c_body, 0);

    let (_, c_bytes) = header::parse_frame(&c_frame[4..]).unwrap();
    let c_reader = flat::BodyReader::new(c_bytes);

    println!(
        "  Proposal:       0x{:016X}",
        c_reader.read_u64(types::consensus::PROPOSAL_ID)
    );
    println!(
        "  Voter:          0x{:016X}",
        c_reader.read_u64(types::consensus::VOTER_ID)
    );
    println!(
        "  Confidence:     {:.1}%",
        types::conf_from_raw(c_reader.read_u32(types::consensus::CONFIDENCE)) * 100.0
    );
    println!(
        "  Vote:           {}\n",
        c_reader.read_u32(types::consensus::FLAGS)
    );

    // ─── Summary ─────────────────────────────────────────────
    println!("═══ SUMMARY ═══");
    println!("Protocol:      NWP v{}", VERSION);
    println!("Messages:      COMMAND + READINESS + DATA + CONSENSUS");
    println!("Zero-copy:     ✅ — all field reads are offset computations into flat buffer");
    println!("Allocations:   0 during parsing — buffers built once, read in-place");
    println!("Integrity:     ✅ — CRC32 validated on header and data payload");
    println!("Frame format:  [4B len][16B header][N B body] — FrameBuffer-style");
    println!("═══ DEMO COMPLETE ═══");
}
