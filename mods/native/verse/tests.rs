// Tests for Verse mod Step 5.1: iroh Scaffold & Identity Bootstrap

#[cfg(test)]
mod tests {
    use super::super::{verse_manifest, P2PIdentitySecret};

    #[test]
    fn verse_manifest_declares_correct_provides() {
        let manifest = verse_manifest();
        
        assert_eq!(manifest.mod_id(), "verse");
        assert_eq!(manifest.mod_type(), crate::registries::infrastructure::mod_loader::ModType::Native);
        
        let provides = manifest.provides();
        assert!(provides.contains(&"identity:p2p".to_string()), 
            "Verse should provide identity:p2p");
        assert!(provides.contains(&"protocol:verse".to_string()), 
            "Verse should provide protocol:verse");
    }

    #[test]
    fn verse_manifest_declares_required_registries() {
        let manifest = verse_manifest();
        
        let requires = manifest.requires();
        assert!(requires.iter().any(|dep| dep.mod_id == "IdentityRegistry"), 
            "Verse should require IdentityRegistry");
        assert!(requires.iter().any(|dep| dep.mod_id == "ActionRegistry"), 
            "Verse should require ActionRegistry");
        assert!(requires.iter().any(|dep| dep.mod_id == "ProtocolRegistry"), 
            "Verse should require ProtocolRegistry");
        assert!(requires.iter().any(|dep| dep.mod_id == "DiagnosticsRegistry"), 
            "Verse should require DiagnosticsRegistry");
    }

    #[test]
    fn verse_manifest_declares_network_capability() {
        let manifest = verse_manifest();
        
        let capabilities = manifest.capabilities();
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
