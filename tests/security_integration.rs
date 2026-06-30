//! Security subsystem integration tests.
//!
//! Tests the full pipeline: identity → signing → encryption → trust scoring → audit logging.
//! These are module-level integrations that live outside each module's unit tests.

#[cfg(test)]
mod tests {
    use neuron_wire::security::*;
    use neuron_wire::components::EntityId;

    /// Generate an IPv4-mapped EntityId for testing.
    fn test_eid(id: u8) -> EntityId {
        let mut bytes = [0u8; 32];
        bytes[0] = id;
        EntityId(bytes)
    }

    #[test]
    fn test_full_security_pipeline() {
        // ── 1. Identity: two nodes with Ed25519 keypairs ──────────
        let mut alice = NodeIdentity::new();
        let bob = NodeIdentity::new();

        assert_ne!(alice.entity_id(), bob.entity_id());
        assert!(
            !alice.public_key_bytes().iter().all(|&b| b == 0),
            "public key must be non-zero"
        );

        // ── 2. Sign and verify a packet ──────────────────────────
        let body_hash = [0xABu8; 32];
        let seq = 42;
        let ts = 1_234_567_890;
        let sig = alice.sign_packet(seq, ts, &body_hash);
        let sig_bytes: [u8; SIGNATURE_LENGTH] = sig.to_bytes();

        let verify_result = verify_packet_signature(
            &alice.public_key_bytes(),
            seq,
            ts,
            &body_hash,
            &sig_bytes,
        );
        assert!(verify_result.is_ok(), "signature must verify");

        // Wrong public key should fail
        let wrong_verify = verify_packet_signature(
            &bob.public_key_bytes(),
            seq,
            ts,
            &body_hash,
            &sig_bytes,
        );
        assert!(wrong_verify.is_err(), "wrong key must fail verification");

        // ── 3. Secure channel handshake + encryption ──────────────
        let mut alice_chan = SecureChannel::new();
        let mut bob_chan = SecureChannel::new();

        let alice_sid = alice_chan.handshake(&alice, bob.public_key_bytes());
        let bob_sid = bob_chan.handshake(&bob, alice.public_key_bytes());

        assert!(alice_chan.has_session(&alice_sid));
        assert!(bob_chan.has_session(&bob_sid));

        // Alice encrypts a message for Bob
        let msg = b"top secret gradient update";
        let (nonce, ct) = alice_chan
            .encrypt(&alice_sid, msg, b"nwp")
            .expect("encrypt should succeed");

        // Bob decrypts
        let decrypted = bob_chan
            .decrypt(&bob_sid, &nonce, &ct, b"nwp")
            .expect("decrypt should succeed");
        assert_eq!(&decrypted, msg, "decrypted message must match");

        // Replay attack: same packet rejected
        let replay = bob_chan.decrypt(&bob_sid, &nonce, &ct, b"nwp");
        assert!(replay.is_none(), "replay must be rejected");

        // Tampered ciphertext rejected
        let mut tampered_ct = ct.clone();
        if !tampered_ct.is_empty() {
            tampered_ct[0] ^= 0xFF; // Flip a bit
        }
        let tampered = bob_chan.decrypt(&bob_sid, &nonce, &tampered_ct, b"nwp");
        assert!(tampered.is_none(), "tampered ciphertext must be rejected");

        // ── 4. Trust system ──────────────────────────────────────
        let mut trust = TrustSystem::new();
        let peer_eid = test_eid(42);

        // Unknown peer starts untrusted
        assert!(!trust.is_trusted(&peer_eid));

        // Positive behaviour increases trust
        trust.record_event(peer_eid, TrustEvent::ValidSignature);
        trust.record_event(peer_eid, TrustEvent::SuccessfulDecrypt);
        trust.record_event(peer_eid, TrustEvent::SuccessfulHandshake);

        let score = trust.trust_score(&peer_eid);
        assert!(score > INITIAL_TRUST, "positive events must increase trust");

        // Invalid signature drops trust
        trust.record_event(peer_eid, TrustEvent::InvalidSignature);
        let score_after_bad = trust.trust_score(&peer_eid);
        assert!(
            score_after_bad < score,
            "invalid signature must decrease trust"
        );

        // Rate limiting
        let mut limited = false;
        for _ in 0..20 {
            if trust.check_rate_limit(&peer_eid) {
                limited = true;
                break;
            }
        }
        assert!(limited, "rate limiting must eventually trigger");

        // Stats
        let stats = trust.stats();
        assert!(stats.total_peers >= 1);

        // ── 5. Audit log ─────────────────────────────────────────
        let mut audit = AuditLog::new();

        // Log everything that happened
        audit.append(
            AuditEventType::NodeStartup,
            "Alice node initialized",
            Some(alice.entity_id()),
        );
        audit.append(
            AuditEventType::HandshakeSuccess,
            "Alice ↔ Bob handshake complete",
            Some(bob.entity_id()),
        );
        audit.append(
            AuditEventType::TrustChange,
            "Bob trust increased to 0.75",
            Some(bob.entity_id()),
        );

        // Verify hash chain integrity
        assert!(audit.verify_integrity(), "audit chain must be intact");
        assert_eq!(audit.total_entries(), 3);

        // Generate bootstrap proof
        let proof = audit.bootstrap_proof();
        assert!(audit.verify_bootstrap(&proof), "bootstrap proof must verify");

        // ── 6. EntityId derivation ──────────────────────────────
        let derived_eid = entity_id_from_public_key(&alice.public_key_bytes());
        assert_eq!(
            derived_eid,
            alice.entity_id(),
            "EntityId must match from public key"
        );

        // ── 7. Identity rotation ─────────────────────────────────
        let old_pk = alice.public_key_bytes();
        let old_eid = alice.entity_id();
        let _old_vk = alice.rotate();
        assert_ne!(
            alice.public_key_bytes(),
            old_pk,
            "key rotation must change public key"
        );
        assert_ne!(
            alice.entity_id(),
            old_eid,
            "key rotation must change EntityId"
        );

        // ── 8. Bootstrap config ──────────────────────────────────
        let bootstrap = SecureBootstrap::new("192.168.1.1:8000");
        assert!(bootstrap.validate().is_ok());
    }
}
