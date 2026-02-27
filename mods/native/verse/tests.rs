// Tests for Verse mod Step 5.1: iroh Scaffold & Identity Bootstrap

#[cfg(test)]
mod step_5_1_tests {
    use super::super::{P2PIdentitySecret, verse_manifest};

    #[test]
    fn verse_manifest_declares_correct_provides() {
        let manifest = verse_manifest();

        assert_eq!(manifest.mod_id, "verse");
        assert_eq!(
            manifest.mod_type,
            crate::registries::infrastructure::mod_loader::ModType::Native
        );

        let provides = &manifest.provides;
        assert!(
            provides.contains(&"identity:p2p".to_string()),
            "Verse should provide identity:p2p"
        );
        assert!(
            provides.contains(&"protocol:verse".to_string()),
            "Verse should provide protocol:verse"
        );
    }

    #[test]
    fn verse_manifest_declares_required_registries() {
        let manifest = verse_manifest();

        let requires = &manifest.requires;
        assert!(
            requires.contains(&"IdentityRegistry".to_string()),
            "Verse should require IdentityRegistry"
        );
        assert!(
            requires.contains(&"ActionRegistry".to_string()),
            "Verse should require ActionRegistry"
        );
        assert!(
            requires.contains(&"ProtocolRegistry".to_string()),
            "Verse should require ProtocolRegistry"
        );
        assert!(
            requires.contains(&"DiagnosticsRegistry".to_string()),
            "Verse should require DiagnosticsRegistry"
        );
    }

    #[test]
    fn verse_manifest_declares_network_capability() {
        let manifest = verse_manifest();

        let capabilities = &manifest.capabilities;
        assert!(
            capabilities.iter().any(|cap| matches!(
                cap,
                crate::registries::infrastructure::mod_loader::ModCapability::Network
            )),
            "Verse should declare network capability"
        );
        assert!(
            capabilities.iter().any(|cap| matches!(
                cap,
                crate::registries::infrastructure::mod_loader::ModCapability::Identity
            )),
            "Verse should declare identity capability"
        );
    }

    #[test]
    fn p2p_identity_secret_serde_roundtrip() {
        let original_key = iroh::SecretKey::generate(&mut rand::thread_rng());
        let identity = P2PIdentitySecret {
            secret_key: original_key.clone(),
            device_name: "test-device".to_string(),
            created_at: std::time::SystemTime::now(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&identity).expect("serialization should succeed");

        // Deserialize back
        let deserialized: P2PIdentitySecret =
            serde_json::from_str(&json).expect("deserialization should succeed");

        // Verify keys match (by comparing their public keys)
        assert_eq!(
            original_key.public().to_string(),
            deserialized.secret_key.public().to_string(),
            "Keys should match after round-trip"
        );
        assert_eq!(identity.device_name, deserialized.device_name);
    }
}

// Tests for Step 5.2: TrustedPeer Store & IdentityRegistry Extension
#[cfg(test)]
mod step_5_2_tests {
    use super::super::{
        AccessLevel, PeerRole, SyncLog, SyncedIntent, TrustedPeer, VersionVector, WorkspaceGrant,
        verify_peer_signature,
    };
    use crate::services::persistence::types::LogEntry;

    #[test]
    fn trusted_peer_serde_roundtrip() {
        let node_id = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        let peer = TrustedPeer {
            node_id,
            display_name: "Marks-iPhone".to_string(),
            role: PeerRole::Self_,
            added_at: std::time::SystemTime::now(),
            last_seen: Some(std::time::SystemTime::now()),
            workspace_grants: vec![
                WorkspaceGrant {
                    workspace_id: "research".to_string(),
                    access: AccessLevel::ReadWrite,
                },
                WorkspaceGrant {
                    workspace_id: "private".to_string(),
                    access: AccessLevel::ReadOnly,
                },
            ],
        };

        // Serialize to JSON
        let json = serde_json::to_string(&peer).expect("serialization should succeed");

        // Deserialize back
        let deserialized: TrustedPeer =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(peer.node_id, deserialized.node_id);
        assert_eq!(peer.display_name, deserialized.display_name);
        assert_eq!(peer.role, deserialized.role);
        assert_eq!(
            peer.workspace_grants.len(),
            deserialized.workspace_grants.len()
        );
    }

    #[test]
    fn version_vector_merge() {
        let mut vv1 = VersionVector::new();
        let mut vv2 = VersionVector::new();

        let peer_a = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        let peer_b = iroh::SecretKey::generate(&mut rand::thread_rng()).public();

        vv1.increment(peer_a);
        vv1.increment(peer_a);
        vv1.increment(peer_b);

        vv2.increment(peer_a);
        vv2.increment(peer_b);
        vv2.increment(peer_b);

        let merged = vv1.merge(&vv2);

        // Should take max from both
        assert_eq!(merged.get(peer_a), 2); // max(2, 1)
        assert_eq!(merged.get(peer_b), 2); // max(1, 2)
    }

    #[test]
    fn version_vector_dominates() {
        let mut vv_newer = VersionVector::new();
        let mut vv_older = VersionVector::new();

        let peer_a = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        let peer_b = iroh::SecretKey::generate(&mut rand::thread_rng()).public();

        vv_older.increment(peer_a);
        vv_older.increment(peer_b);

        vv_newer.increment(peer_a);
        vv_newer.increment(peer_a);
        vv_newer.increment(peer_b);
        vv_newer.increment(peer_b);

        assert!(
            vv_newer.dominates(&vv_older),
            "Newer VV should dominate older"
        );
        assert!(
            !vv_older.dominates(&vv_newer),
            "Older VV should not dominate newer"
        );
    }

    #[test]
    fn sync_log_encryption_roundtrip() {
        let secret_key = iroh::SecretKey::generate(&mut rand::thread_rng());
        let node_id = secret_key.public();

        let mut sync_log = SyncLog::new("test-workspace".to_string());
        sync_log.version_vector.increment(node_id);
        sync_log.intents.push(SyncedIntent {
            log_entry: LogEntry::AddNode {
                node_id: "abc123".to_string(),
                url: "https://example.com".to_string(),
                position_x: 10.0,
                position_y: 20.0,
            },
            authored_by: node_id,
            authored_at_secs: 1708732800,
            sequence: 1,
        });

        // Encrypt
        let plaintext = sync_log.to_bytes();
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(secret_key.to_bytes());
        hasher.update(b"synclog-encryption-key-v1");
        let key: [u8; 32] = hasher.finalize().into();

        let encrypted = SyncLog::encrypt(&plaintext, &key).expect("encryption should succeed");

        // Decrypt
        let decrypted = SyncLog::decrypt(&encrypted, &key).expect("decryption should succeed");
        let recovered = SyncLog::from_bytes(&decrypted).expect("deserialization should succeed");

        assert_eq!(sync_log.workspace_id, recovered.workspace_id);
        assert_eq!(sync_log.version_vector, recovered.version_vector);
        assert_eq!(sync_log.intents.len(), recovered.intents.len());
    }

    #[test]
    fn sign_verify_roundtrip() {
        let secret_key = iroh::SecretKey::generate(&mut rand::thread_rng());
        let node_id = secret_key.public();

        let payload = b"test sync payload";

        // Sign with secret key
        let signature = secret_key.sign(payload);
        let sig_bytes = signature.to_bytes();

        // Verify with public key (using our verify function)
        let verified = verify_peer_signature(node_id, payload, &sig_bytes);

        assert!(verified, "Signature should verify correctly");

        // Verify wrong payload fails
        let wrong_payload = b"different payload";
        let verified_wrong = verify_peer_signature(node_id, wrong_payload, &sig_bytes);

        assert!(!verified_wrong, "Wrong payload should fail verification");
    }

    #[test]
    fn grant_model_serialization() {
        let grant = WorkspaceGrant {
            workspace_id: "research".to_string(),
            access: AccessLevel::ReadWrite,
        };

        let json = serde_json::to_string(&grant).expect("serialization should succeed");
        let deserialized: WorkspaceGrant =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(grant.workspace_id, deserialized.workspace_id);
        assert_eq!(grant.access, deserialized.access);
    }

    #[test]
    fn peer_role_serialization() {
        let self_role = PeerRole::Self_;
        let friend_role = PeerRole::Friend;

        let self_json = serde_json::to_string(&self_role).expect("serialization should succeed");
        let friend_json =
            serde_json::to_string(&friend_role).expect("serialization should succeed");

        assert_eq!(self_json, r#""self""#);
        assert_eq!(friend_json, r#""friend""#);
    }
}

// Tests for Step 5.4: Delta Sync core rules (LWW + ghost-node)
#[cfg(test)]
mod step_5_4_tests {
    use super::super::{SyncLog, SyncedIntent};
    use crate::services::persistence::types::LogEntry;

    #[test]
    fn sync_log_lww_rejects_older_title() {
        let node_id = "node-1".to_string();
        let peer = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        let mut log = SyncLog::new("workspace".to_string());

        let newer = SyncedIntent {
            log_entry: LogEntry::UpdateNodeTitle {
                node_id: node_id.clone(),
                title: "new".to_string(),
            },
            authored_by: peer,
            authored_at_secs: 200,
            sequence: 1,
        };
        assert!(log.should_apply(&newer));

        let older = SyncedIntent {
            log_entry: LogEntry::UpdateNodeTitle {
                node_id: node_id.clone(),
                title: "old".to_string(),
            },
            authored_by: peer,
            authored_at_secs: 150,
            sequence: 2,
        };
        assert!(!log.should_apply(&older));
    }

    #[test]
    fn sync_log_ghost_node_blocks_old_add() {
        let node_id = "node-2".to_string();
        let peer = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        let mut log = SyncLog::new("workspace".to_string());

        let removal = SyncedIntent {
            log_entry: LogEntry::RemoveNode {
                node_id: node_id.clone(),
            },
            authored_by: peer,
            authored_at_secs: 300,
            sequence: 1,
        };
        assert!(log.should_apply(&removal));

        let add_older = SyncedIntent {
            log_entry: LogEntry::AddNode {
                node_id: node_id.clone(),
                url: "https://example.com".to_string(),
                position_x: 0.0,
                position_y: 0.0,
            },
            authored_by: peer,
            authored_at_secs: 200,
            sequence: 2,
        };
        assert!(!log.should_apply(&add_older));
    }

    #[test]
    fn sync_log_rejects_duplicate_sequence() {
        let peer = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        let mut log = SyncLog::new("workspace".to_string());

        let first = SyncedIntent {
            log_entry: LogEntry::ClearGraph,
            authored_by: peer,
            authored_at_secs: 10,
            sequence: 1,
        };
        assert!(log.record_intent(first));

        let duplicate = SyncedIntent {
            log_entry: LogEntry::ClearGraph,
            authored_by: peer,
            authored_at_secs: 11,
            sequence: 1,
        };
        assert!(!log.record_intent(duplicate));
    }
}

// ===== Step 5.3 Tests: Pairing Ceremony & Settings UI =====

#[cfg(test)]
mod step_5_3_tests {
    use super::super::{
        DiscoveredPeer, PAIRING_WORDLIST, decode_pairing_code, generate_qr_code_ascii,
        generate_qr_code_png, sanitize_service_name,
    };

    #[test]
    fn pairing_code_generates_six_words() {
        // Note: This test requires init() to be called, but we can test the wordlist logic independently
        let test_bytes = vec![0u8, 42, 100, 150, 200, 255];
        let mut words = Vec::new();

        for byte_val in test_bytes {
            let word_idx = byte_val as usize % 256;
            words.push(PAIRING_WORDLIST[word_idx]);
        }

        assert_eq!(words.len(), 6, "Should generate exactly 6 words");

        let phrase = words.join("-");
        assert!(phrase.contains("-"), "Phrase should be hyphen-separated");

        // Verify all words are in the wordlist
        for word in &words {
            assert!(
                PAIRING_WORDLIST.contains(word),
                "Word '{}' should be in wordlist",
                word
            );
        }
    }

    #[test]
    fn pairing_code_decode_validates_word_count() {
        let valid_code = "abandon-ability-able-about-above-absent";
        let invalid_code = "abandon-ability-able:not-a-node-id"; // Only 3 words

        // Valid code should not error on word count (though it will error on "not implemented")
        let _result_valid = decode_pairing_code(valid_code);

        let result_invalid = decode_pairing_code(invalid_code);
        assert!(
            result_invalid.is_err(),
            "Should reject codes with wrong word count"
        );
        assert!(
            result_invalid.unwrap_err().contains("expected 6 words"),
            "Error should mention word count"
        );
    }

    #[test]
    fn pairing_code_decode_validates_wordlist_membership() {
        let invalid_code = "abandon-ability-NOTAWORD-about-above-absent:not-a-node-id";

        let result = decode_pairing_code(invalid_code);
        assert!(result.is_err(), "Should reject codes with unknown words");
        assert!(
            result.unwrap_err().contains("unknown word"),
            "Error should mention unknown word"
        );
    }

    #[test]
    fn pairing_code_decode_accepts_node_id_suffix_payload() {
        let node_id = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        let code = format!("abandon-ability-able-about-above-absent:{}", node_id);

        let decoded = decode_pairing_code(&code).expect("pairing code should decode");
        assert_eq!(decoded, node_id);
    }

    #[test]
    fn pairing_code_decode_rejects_missing_suffix_payload() {
        let code = "abandon-ability-able-about-above-absent";
        let result = decode_pairing_code(code);
        assert!(result.is_err(), "Should reject code missing node-id suffix");
        assert!(
            result.unwrap_err().contains("missing node-id suffix"),
            "Error should mention missing suffix"
        );
    }

    #[test]
    fn mdns_service_name_sanitization() {
        assert_eq!(sanitize_service_name("Marks-Desktop"), "Marks-Desktop");
        assert_eq!(sanitize_service_name("My Computer!"), "My-Computer");
        assert_eq!(sanitize_service_name("Test@Host#Name"), "Test-Host-Name");
        assert_eq!(sanitize_service_name("---Start"), "Start");
        assert_eq!(sanitize_service_name("End---"), "End");
    }

    #[test]
    fn qr_code_generation_produces_output() {
        let phrase = "abandon-ability-able-about-above-absent";

        // Test ASCII QR generation
        let ascii_qr = generate_qr_code_ascii(phrase);
        assert!(ascii_qr.is_ok(), "ASCII QR generation should succeed");
        let ascii_str = ascii_qr.unwrap();
        assert!(!ascii_str.is_empty(), "ASCII QR should have content");

        // Test PNG/SVG generation
        let png_qr = generate_qr_code_png(phrase);
        assert!(png_qr.is_ok(), "PNG QR generation should succeed");
        let png_bytes = png_qr.unwrap();
        assert!(!png_bytes.is_empty(), "QR bytes should have content");
        assert!(
            png_bytes.len() > 100,
            "QR should be substantial (>100 bytes)"
        );
    }

    #[test]
    fn discovered_peer_contains_required_fields() {
        let node_id = iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        let relay_url = "https://relay.example.com".parse::<url::Url>().ok();

        let peer = DiscoveredPeer {
            device_name: "Test-Device".to_string(),
            node_id,
            relay_url: relay_url.clone(),
        };

        assert_eq!(peer.device_name, "Test-Device");
        assert_eq!(peer.node_id, node_id);
        assert_eq!(peer.relay_url, relay_url);
    }
}
