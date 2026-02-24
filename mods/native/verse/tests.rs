// Tests for Verse mod Step 5.1: iroh Scaffold & Identity Bootstrap

#[cfg(test)]
mod step_5_1_tests {
    use super::super::{verse_manifest, P2PIdentitySecret};

    #[test]
    fn verse_manifest_declares_correct_provides() {
        let manifest = verse_manifest();
        
        assert_eq!(manifest.mod_id, "verse");
        assert_eq!(manifest.mod_type, crate::registries::infrastructure::mod_loader::ModType::Native);
        
        let provides = &manifest.provides;
        assert!(provides.contains(&"identity:p2p".to_string()), 
            "Verse should provide identity:p2p");
        assert!(provides.contains(&"protocol:verse".to_string()), 
            "Verse should provide protocol:verse");
    }

    #[test]
    fn verse_manifest_declares_required_registries() {
        let manifest = verse_manifest();
        
        let requires = &manifest.requires;
        assert!(requires.contains(&"IdentityRegistry".to_string()), 
            "Verse should require IdentityRegistry");
        assert!(requires.contains(&"ActionRegistry".to_string()), 
            "Verse should require ActionRegistry");
        assert!(requires.contains(&"ProtocolRegistry".to_string()), 
            "Verse should require ProtocolRegistry");
        assert!(requires.contains(&"DiagnosticsRegistry".to_string()), 
            "Verse should require DiagnosticsRegistry");
    }

    #[test]
    fn verse_manifest_declares_network_capability() {
        let manifest = verse_manifest();
        
        let capabilities = &manifest.capabilities;
        assert!(capabilities.iter().any(|cap| matches!(cap, crate::registries::infrastructure::mod_loader::ModCapability::Network)), 
            "Verse should declare network capability");
        assert!(capabilities.iter().any(|cap| matches!(cap, crate::registries::infrastructure::mod_loader::ModCapability::Identity)), 
            "Verse should declare identity capability");
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
        let deserialized: P2PIdentitySecret = serde_json::from_str(&json)
            .expect("deserialization should succeed");

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
        TrustedPeer, PeerRole, AccessLevel, WorkspaceGrant, VersionVector, SyncLog, SyncedIntent,
        sign_sync_payload, verify_peer_signature,
    };

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
        let deserialized: TrustedPeer = serde_json::from_str(&json)
            .expect("deserialization should succeed");

        assert_eq!(peer.node_id, deserialized.node_id);
        assert_eq!(peer.display_name, deserialized.display_name);
        assert_eq!(peer.role, deserialized.role);
        assert_eq!(peer.workspace_grants.len(), deserialized.workspace_grants.len());
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
        
        assert!(vv_newer.dominates(&vv_older), "Newer VV should dominate older");
        assert!(!vv_older.dominates(&vv_newer), "Older VV should not dominate newer");
    }

    #[test]
    fn sync_log_encryption_roundtrip() {
        let secret_key = iroh::SecretKey::generate(&mut rand::thread_rng());
        let node_id = secret_key.public();
        
        let mut sync_log = SyncLog::new("test-workspace".to_string());
        sync_log.version_vector.increment(node_id);
        sync_log.intents.push(SyncedIntent {
            intent_json: r#"{"type":"AddNode","node_id":"abc123"}"#.to_string(),
            authored_by: node_id,
            authored_at_secs: 1708732800,
            sequence: 1,
        });
        
        // Encrypt
        let plaintext = sync_log.to_bytes();
        use sha2::{Sha256, Digest};
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
        let deserialized: WorkspaceGrant = serde_json::from_str(&json)
            .expect("deserialization should succeed");
        
        assert_eq!(grant.workspace_id, deserialized.workspace_id);
        assert_eq!(grant.access, deserialized.access);
    }

    #[test]
    fn peer_role_serialization() {
        let self_role = PeerRole::Self_;
        let friend_role = PeerRole::Friend;
        
        let self_json = serde_json::to_string(&self_role).expect("serialization should succeed");
        let friend_json = serde_json::to_string(&friend_role).expect("serialization should succeed");
        
        assert_eq!(self_json, r#""self""#);
        assert_eq!(friend_json, r#""friend""#);
    }
}
