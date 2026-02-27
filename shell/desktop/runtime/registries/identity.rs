use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// Import Verse P2P types for trait implementation
use crate::mods::native::verse::{P2PIdentityExt, TrustedPeer};

pub(crate) const IDENTITY_ID_DEFAULT: &str = "identity:default";
pub(crate) const IDENTITY_ID_P2P: &str = "identity:p2p";

#[derive(Debug, Clone)]
pub(crate) struct IdentityResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) key_available: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct IdentitySignResult {
    pub(crate) resolution: IdentityResolution,
    pub(crate) signature: Option<String>,
    pub(crate) succeeded: bool,
}

pub(crate) struct IdentityRegistry {
    keys: HashMap<String, String>,
    fallback_id: String,
}

impl IdentityRegistry {
    fn has_usable_key(&self, identity_id: &str) -> bool {
        self.keys
            .get(identity_id)
            .is_some_and(|key| !key.trim().is_empty())
    }

    pub(crate) fn register_persona(&mut self, identity_id: &str, key_material: &str) {
        self.keys
            .insert(identity_id.to_ascii_lowercase(), key_material.to_string());
    }

    pub(crate) fn resolve(&self, identity_id: &str) -> IdentityResolution {
        let requested = identity_id.trim().to_ascii_lowercase();

        if requested.is_empty() {
            let key_available = self.has_usable_key(&self.fallback_id);
            return IdentityResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                key_available,
            };
        }

        if self.keys.contains_key(&requested) {
            let key_available = self.has_usable_key(&requested);
            return IdentityResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                key_available,
            };
        }

        let key_available = self.has_usable_key(&self.fallback_id);
        IdentityResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            key_available,
        }
    }

    pub(crate) fn sign(&self, identity_id: &str, payload: &[u8]) -> IdentitySignResult {
        let resolution = self.resolve(identity_id);
        let Some(key_material) = self.keys.get(&resolution.resolved_id) else {
            return IdentitySignResult {
                resolution,
                signature: None,
                succeeded: false,
            };
        };
        if key_material.trim().is_empty() {
            return IdentitySignResult {
                resolution,
                signature: None,
                succeeded: false,
            };
        };

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        resolution.resolved_id.hash(&mut hasher);
        key_material.hash(&mut hasher);
        payload.hash(&mut hasher);
        let digest = hasher.finish();

        IdentitySignResult {
            resolution,
            signature: Some(format!("sig:{digest:016x}")),
            succeeded: true,
        }
    }
}

impl Default for IdentityRegistry {
    fn default() -> Self {
        let mut registry = Self {
            keys: HashMap::new(),
            fallback_id: IDENTITY_ID_DEFAULT.to_string(),
        };
        registry.register_persona(IDENTITY_ID_DEFAULT, "local-test-key-default");
        registry.register_persona("identity:locked", "");
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_registry_signs_with_default_persona() {
        let registry = IdentityRegistry::default();
        let result = registry.sign(IDENTITY_ID_DEFAULT, b"payload");

        assert!(result.succeeded);
        assert!(result.resolution.matched);
        assert_eq!(result.resolution.resolved_id, IDENTITY_ID_DEFAULT);
        assert!(
            result
                .signature
                .as_deref()
                .is_some_and(|sig| sig.starts_with("sig:"))
        );
    }

    #[test]
    fn identity_registry_falls_back_for_unknown_persona() {
        let registry = IdentityRegistry::default();
        let result = registry.sign("identity:unknown", b"payload");

        assert!(result.succeeded);
        assert!(!result.resolution.matched);
        assert_eq!(result.resolution.resolved_id, IDENTITY_ID_DEFAULT);
    }

    #[test]
    fn identity_registry_reports_key_unavailable_when_empty() {
        let mut registry = IdentityRegistry::default();
        registry.keys.clear();

        let result = registry.sign(IDENTITY_ID_DEFAULT, b"payload");

        assert!(!result.succeeded);
        assert!(!result.resolution.key_available);
        assert_eq!(result.signature, None);
    }

    #[test]
    fn identity_registry_reports_key_unavailable_for_locked_persona() {
        let registry = IdentityRegistry::default();
        let result = registry.sign("identity:locked", b"payload");

        assert!(!result.succeeded);
        assert!(!result.resolution.key_available);
        assert_eq!(result.resolution.resolved_id, "identity:locked");
    }
}

// ===== P2PIdentityExt Trait Implementation =====

impl P2PIdentityExt for IdentityRegistry {
    fn p2p_node_id(&self) -> iroh::NodeId {
        crate::mods::native::verse::node_id()
    }

    fn sign_sync_payload(&self, payload: &[u8]) -> Vec<u8> {
        crate::mods::native::verse::sign_sync_payload(payload)
    }

    fn verify_peer_signature(&self, peer: iroh::NodeId, payload: &[u8], sig: &[u8]) -> bool {
        crate::mods::native::verse::verify_peer_signature(peer, payload, sig)
    }

    fn get_trusted_peers(&self) -> Vec<TrustedPeer> {
        crate::mods::native::verse::get_trusted_peers()
    }

    fn trust_peer(&mut self, peer: TrustedPeer) {
        crate::mods::native::verse::trust_peer(peer);
    }

    fn revoke_peer(&mut self, node_id: iroh::NodeId) {
        crate::mods::native::verse::revoke_peer(node_id);
    }
}
