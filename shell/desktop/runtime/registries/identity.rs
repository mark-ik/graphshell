use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use secp256k1::schnorr::Signature as SchnorrSignature;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use sha2::{Digest, Sha256};

use crate::mods::native::verse::{P2PIdentityExt, TrustedPeer};

pub(crate) const IDENTITY_ID_DEFAULT: &str = "identity:default";
pub(crate) const IDENTITY_ID_P2P: &str = "identity:p2p";
const IDENTITY_ID_LOCKED: &str = "identity:locked";
const PRESENCE_BINDING_VERSION: &str = "graphshell.presence_binding.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UserIdentityProtocol {
    LocalNostrSecp256k1,
    NostrPubkey,
    DidPlc,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct UserIdentityClaim {
    pub(crate) identity_id: String,
    pub(crate) protocol: UserIdentityProtocol,
    pub(crate) public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PresenceBindingAssertion {
    pub(crate) node_id: String,
    pub(crate) user_identity: UserIdentityClaim,
    pub(crate) issued_at_secs: u64,
    pub(crate) expires_at_secs: u64,
    pub(crate) audience: String,
    pub(crate) signature: String,
}

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
    pub(crate) verifying_key: Option<String>,
    pub(crate) succeeded: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct IdentityVerifyResult {
    pub(crate) resolution: IdentityResolution,
    pub(crate) verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyProtection {
    Unprotected,
    Ephemeral,
}

#[derive(Debug)]
pub(crate) enum IdentityKeyError {
    InvalidKeyMaterial,
    Io(String),
    PersonaLocked,
    PersonaMissing(String),
}

impl std::fmt::Display for IdentityKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidKeyMaterial => write!(f, "invalid key material"),
            Self::Io(error) => write!(f, "{error}"),
            Self::PersonaLocked => write!(f, "persona is locked"),
            Self::PersonaMissing(identity_id) => write!(f, "missing persona: {identity_id}"),
        }
    }
}

impl std::error::Error for IdentityKeyError {}

#[derive(Debug, Clone)]
struct IdentityKey {
    signing_key: Option<SigningKey>,
    verifying_key: VerifyingKey,
    archived_verifying_keys: Vec<VerifyingKey>,
    seed_path: Option<PathBuf>,
    archive_path: Option<PathBuf>,
    protection: KeyProtection,
}

#[derive(Debug, Clone)]
struct UserIdentityKey {
    secret_key: Option<SecretKey>,
    public_key: XOnlyPublicKey,
    seed_path: Option<PathBuf>,
    protection: KeyProtection,
}

impl UserIdentityKey {
    fn generate_ephemeral() -> Self {
        let secret_key = SecretKey::new(&mut secp256k1::rand::rng());
        let keypair = Keypair::from_secret_key(&Secp256k1::new(), &secret_key);
        let (public_key, _) = XOnlyPublicKey::from_keypair(&keypair);
        Self {
            secret_key: Some(secret_key),
            public_key,
            seed_path: None,
            protection: KeyProtection::Ephemeral,
        }
    }

    fn load_or_generate(identity_id: &str, store_root: &Path) -> Result<(Self, bool), IdentityKeyError> {
        let seed_path = user_seed_path_for(store_root, identity_id);
        if let Some(parent) = seed_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| IdentityKeyError::Io(format!("create user identity dir: {error}")))?;
        }

        let (secret_key, generated) = if seed_path.exists() {
            let bytes = fs::read(&seed_path)
                .map_err(|error| IdentityKeyError::Io(format!("read user identity key: {error}")))?;
            if bytes.len() != 32 {
                return Err(IdentityKeyError::InvalidKeyMaterial);
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            let secret_key =
                SecretKey::from_byte_array(seed).map_err(|_| IdentityKeyError::InvalidKeyMaterial)?;
            (secret_key, false)
        } else {
            let secret_key = SecretKey::new(&mut secp256k1::rand::rng());
            fs::write(&seed_path, secret_key.secret_bytes())
                .map_err(|error| IdentityKeyError::Io(format!("write user identity key: {error}")))?;
            (secret_key, true)
        };

        let keypair = Keypair::from_secret_key(&Secp256k1::new(), &secret_key);
        let (public_key, _) = XOnlyPublicKey::from_keypair(&keypair);
        Ok((
            Self {
                secret_key: Some(secret_key),
                public_key,
                seed_path: Some(seed_path),
                protection: KeyProtection::Unprotected,
            },
            generated,
        ))
    }

    fn sign_digest(&self, digest: &[u8; 32]) -> Option<SchnorrSignature> {
        let secret_key = self.secret_key.as_ref()?;
        let keypair = Keypair::from_secret_key(&Secp256k1::new(), secret_key);
        Some(Secp256k1::new().sign_schnorr_no_aux_rand(digest, &keypair))
    }

    fn verify_digest(&self, digest: &[u8; 32], signature: &SchnorrSignature) -> bool {
        Secp256k1::verification_only()
            .verify_schnorr(signature, digest, &self.public_key)
            .is_ok()
    }
}

impl IdentityKey {
    fn generate_ephemeral() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            verifying_key: signing_key.verifying_key(),
            signing_key: Some(signing_key),
            archived_verifying_keys: Vec::new(),
            seed_path: None,
            archive_path: None,
            protection: KeyProtection::Ephemeral,
        }
    }

    fn locked() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            verifying_key: signing_key.verifying_key(),
            signing_key: None,
            archived_verifying_keys: Vec::new(),
            seed_path: None,
            archive_path: None,
            protection: KeyProtection::Ephemeral,
        }
    }

    fn load_or_generate(
        identity_id: &str,
        store_root: &Path,
    ) -> Result<(Self, bool), IdentityKeyError> {
        let seed_path = seed_path_for(store_root, identity_id);
        let archive_path = archive_path_for(store_root, identity_id);
        if let Some(parent) = seed_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| IdentityKeyError::Io(format!("create identity dir: {error}")))?;
        }

        let (signing_key, generated) = if seed_path.exists() {
            let bytes = fs::read(&seed_path)
                .map_err(|error| IdentityKeyError::Io(format!("read identity key: {error}")))?;
            if bytes.len() != 32 {
                return Err(IdentityKeyError::InvalidKeyMaterial);
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            (SigningKey::from_bytes(&seed), false)
        } else {
            let signing_key = SigningKey::generate(&mut OsRng);
            fs::write(&seed_path, signing_key.to_bytes())
                .map_err(|error| IdentityKeyError::Io(format!("write identity key: {error}")))?;
            (signing_key, true)
        };

        let archived_verifying_keys = load_archived_verifying_keys(&archive_path)?;
        Ok((
            Self {
                verifying_key: signing_key.verifying_key(),
                signing_key: Some(signing_key),
                archived_verifying_keys,
                seed_path: Some(seed_path),
                archive_path: Some(archive_path),
                protection: KeyProtection::Unprotected,
            },
            generated,
        ))
    }

    fn key_available(&self) -> bool {
        self.signing_key.is_some()
    }

    fn sign(&self, payload: &[u8]) -> Option<Signature> {
        self.signing_key.as_ref().map(|key| key.sign(payload))
    }

    fn verify(&self, payload: &[u8], signature: &Signature) -> bool {
        if self.verifying_key.verify(payload, signature).is_ok() {
            return true;
        }

        self.archived_verifying_keys
            .iter()
            .any(|key| key.verify(payload, signature).is_ok())
    }

    fn rotate(&mut self) -> Result<VerifyingKey, IdentityKeyError> {
        let Some(current) = self.signing_key.as_ref() else {
            return Err(IdentityKeyError::PersonaLocked);
        };
        self.archived_verifying_keys.push(current.verifying_key());
        self.persist_archived_verifying_keys()?;

        let next = SigningKey::generate(&mut OsRng);
        if let Some(seed_path) = &self.seed_path {
            fs::write(seed_path, next.to_bytes())
                .map_err(|error| IdentityKeyError::Io(format!("rotate identity key: {error}")))?;
        }
        self.verifying_key = next.verifying_key();
        self.signing_key = Some(next);
        Ok(self.verifying_key)
    }

    fn revoke(&mut self) -> Result<(), IdentityKeyError> {
        if let Some(current) = self.signing_key.as_ref() {
            self.archived_verifying_keys.push(current.verifying_key());
            self.persist_archived_verifying_keys()?;
        }
        if let Some(seed_path) = &self.seed_path
            && seed_path.exists()
        {
            fs::remove_file(seed_path)
                .map_err(|error| IdentityKeyError::Io(format!("remove identity key: {error}")))?;
        }
        self.signing_key = None;
        Ok(())
    }

    fn persist_archived_verifying_keys(&self) -> Result<(), IdentityKeyError> {
        let Some(archive_path) = &self.archive_path else {
            return Ok(());
        };

        let serialized = self
            .archived_verifying_keys
            .iter()
            .map(|key| encode_hex(key.as_bytes()))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(archive_path, serialized)
            .map_err(|error| IdentityKeyError::Io(format!("write archived identity keys: {error}")))
    }
}

pub(crate) struct IdentityRegistry {
    keys: HashMap<String, IdentityKey>,
    user_keys: HashMap<String, UserIdentityKey>,
    fallback_id: String,
    user_fallback_id: String,
    trust_store: Arc<RwLock<Vec<TrustedPeer>>>,
}

impl IdentityRegistry {
    fn register_generated_persona(&mut self, identity_id: &str) {
        self.keys
            .insert(identity_id.to_string(), IdentityKey::generate_ephemeral());
    }

    fn register_locked_persona(&mut self, identity_id: &str) {
        self.keys.insert(identity_id.to_string(), IdentityKey::locked());
    }

    fn generated_identities() -> [&'static str; 2] {
        [IDENTITY_ID_DEFAULT, IDENTITY_ID_P2P]
    }

    fn generated_user_identities() -> [&'static str; 1] {
        [IDENTITY_ID_DEFAULT]
    }

    fn load_persistent_personas(&mut self, store_root: &Path) {
        for identity_id in Self::generated_identities() {
            match IdentityKey::load_or_generate(identity_id, store_root) {
                Ok((identity_key, generated)) => {
                    if generated {
                        log::warn!("identity key generated for {identity_id}");
                    }
                    self.keys.insert(identity_id.to_string(), identity_key);
                }
                Err(error) => {
                    log::warn!(
                        "identity key load failed for {identity_id}: {error}; falling back to ephemeral key"
                    );
                    self.register_generated_persona(identity_id);
                }
            }
        }
    }

    fn load_persistent_user_personas(&mut self, store_root: &Path) {
        for identity_id in Self::generated_user_identities() {
            match UserIdentityKey::load_or_generate(identity_id, store_root) {
                Ok((identity_key, generated)) => {
                    if generated {
                        log::warn!("user identity key generated for {identity_id}");
                    }
                    self.user_keys.insert(identity_id.to_string(), identity_key);
                }
                Err(error) => {
                    log::warn!(
                        "user identity key load failed for {identity_id}: {error}; falling back to ephemeral key"
                    );
                    self.user_keys
                        .insert(identity_id.to_string(), UserIdentityKey::generate_ephemeral());
                }
            }
        }
    }

    pub(crate) fn resolve(&self, identity_id: &str) -> IdentityResolution {
        let requested = identity_id.trim().to_ascii_lowercase();

        if requested.is_empty() {
            let key_available = self
                .keys
                .get(&self.fallback_id)
                .is_some_and(IdentityKey::key_available);
            return IdentityResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                key_available,
            };
        }

        if let Some(identity_key) = self.keys.get(&requested) {
            return IdentityResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                key_available: identity_key.key_available(),
            };
        }

        let key_available = self
            .keys
            .get(&self.fallback_id)
            .is_some_and(IdentityKey::key_available);
        IdentityResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            key_available,
        }
    }

    pub(crate) fn sign(&self, identity_id: &str, payload: &[u8]) -> IdentitySignResult {
        let resolution = self.resolve(identity_id);
        let Some(identity_key) = self.keys.get(&resolution.resolved_id) else {
            return IdentitySignResult {
                resolution,
                signature: None,
                verifying_key: None,
                succeeded: false,
            };
        };

        let Some(signature) = identity_key.sign(payload) else {
            return IdentitySignResult {
                resolution,
                signature: None,
                verifying_key: Some(encode_hex(identity_key.verifying_key.as_bytes())),
                succeeded: false,
            };
        };

        IdentitySignResult {
            resolution,
            signature: Some(format!("sig:{}", encode_hex(&signature.to_bytes()))),
            verifying_key: Some(encode_hex(identity_key.verifying_key.as_bytes())),
            succeeded: true,
        }
    }

    pub(crate) fn verify(
        &self,
        identity_id: &str,
        payload: &[u8],
        signature: &str,
    ) -> IdentityVerifyResult {
        let resolution = self.resolve(identity_id);
        let Some(identity_key) = self.keys.get(&resolution.resolved_id) else {
            return IdentityVerifyResult {
                resolution,
                verified: false,
            };
        };

        let Some(signature) = parse_signature(signature) else {
            return IdentityVerifyResult {
                resolution,
                verified: false,
            };
        };

        IdentityVerifyResult {
            resolution,
            verified: identity_key.verify(payload, &signature),
        }
    }

    pub(crate) fn verifying_key_hex_for(&self, identity_id: &str) -> Option<String> {
        let resolution = self.resolve(identity_id);
        self.keys
            .get(&resolution.resolved_id)
            .map(|key| encode_hex(key.verifying_key.as_bytes()))
    }

    pub(crate) fn user_identity_claim_for(&self, identity_id: &str) -> Option<UserIdentityClaim> {
        let resolved_id = self.resolve_user_identity_id(identity_id);
        self.user_keys.get(&resolved_id).map(|key| UserIdentityClaim {
            identity_id: resolved_id,
            protocol: UserIdentityProtocol::LocalNostrSecp256k1,
            public_key: key.public_key.to_string(),
        })
    }

    pub(crate) fn default_user_identity_claim(&self) -> Option<UserIdentityClaim> {
        self.user_identity_claim_for(IDENTITY_ID_DEFAULT)
    }

    pub(crate) fn nostr_public_key_hex_for(&self, identity_id: &str) -> Option<String> {
        let resolved_id = self.resolve_user_identity_id(identity_id);
        self.user_keys
            .get(&resolved_id)
            .map(|key| key.public_key.to_string())
    }

    pub(crate) fn sign_user_digest(&self, identity_id: &str, digest: &[u8; 32]) -> Option<String> {
        let resolved_id = self.resolve_user_identity_id(identity_id);
        self.user_keys
            .get(&resolved_id)
            .and_then(|key| key.sign_digest(digest))
            .map(|signature| format!("sig:{}", encode_hex(signature.as_ref())))
    }

    pub(crate) fn create_presence_binding_assertion(
        &self,
        user_identity_id: &str,
        audience: &str,
        ttl_secs: u64,
    ) -> Option<PresenceBindingAssertion> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs();
        self.create_presence_binding_assertion_at(user_identity_id, audience, ttl_secs, now_secs)
    }

    fn create_presence_binding_assertion_at(
        &self,
        user_identity_id: &str,
        audience: &str,
        ttl_secs: u64,
        now_secs: u64,
    ) -> Option<PresenceBindingAssertion> {
        let user_identity = self.user_identity_claim_for(user_identity_id)?;
        let audience = audience.trim().to_string();
        if audience.is_empty() {
            return None;
        }

        let assertion = PresenceBindingAssertion {
            node_id: self.p2p_node_id().to_string(),
            user_identity,
            issued_at_secs: now_secs,
            expires_at_secs: now_secs.saturating_add(ttl_secs.max(1)),
            audience,
            signature: String::new(),
        };
        let payload_hash: [u8; 32] = Sha256::digest(canonical_presence_binding_bytes(&assertion)).into();
        let signature = self.sign_user_digest(user_identity_id, &payload_hash)?;

        Some(PresenceBindingAssertion {
            signature,
            ..assertion
        })
    }

    pub(crate) fn verify_presence_binding_assertion(
        &self,
        assertion: &PresenceBindingAssertion,
    ) -> bool {
        if assertion.signature.trim().is_empty() || assertion.audience.trim().is_empty() {
            return false;
        }
        if assertion.expires_at_secs < assertion.issued_at_secs {
            return false;
        }

        let payload = canonical_presence_binding_bytes(assertion);
        match assertion.user_identity.protocol {
            UserIdentityProtocol::LocalNostrSecp256k1 | UserIdentityProtocol::NostrPubkey => {
                let Ok(public_key) = assertion.user_identity.public_key.parse::<XOnlyPublicKey>() else {
                    return false;
                };
                let Some(signature) = parse_schnorr_signature(&assertion.signature) else {
                    return false;
                };
                let payload_hash: [u8; 32] = Sha256::digest(payload).into();
                Secp256k1::verification_only()
                    .verify_schnorr(&signature, &payload_hash, &public_key)
                    .is_ok()
            }
            UserIdentityProtocol::DidPlc => false,
        }
    }

    fn resolve_user_identity_id(&self, identity_id: &str) -> String {
        let requested = identity_id.trim().to_ascii_lowercase();
        if requested.is_empty() {
            return self.user_fallback_id.clone();
        }
        if self.user_keys.contains_key(&requested) {
            return requested;
        }
        self.user_fallback_id.clone()
    }

    pub(crate) fn rotate_key(&mut self, identity_id: &str) -> Result<String, IdentityKeyError> {
        let resolution = self.resolve(identity_id);
        let identity_key = self
            .keys
            .get_mut(&resolution.resolved_id)
            .ok_or_else(|| IdentityKeyError::PersonaMissing(resolution.resolved_id.clone()))?;
        let verifying_key = identity_key.rotate()?;
        Ok(encode_hex(verifying_key.as_bytes()))
    }

    pub(crate) fn revoke_key(&mut self, identity_id: &str) -> Result<(), IdentityKeyError> {
        let resolution = self.resolve(identity_id);
        let identity_key = self
            .keys
            .get_mut(&resolution.resolved_id)
            .ok_or_else(|| IdentityKeyError::PersonaMissing(resolution.resolved_id.clone()))?;
        identity_key.revoke()
    }

    pub(crate) fn protection_for(&self, identity_id: &str) -> Option<KeyProtection> {
        let resolution = self.resolve(identity_id);
        self.keys.get(&resolution.resolved_id).map(|key| key.protection)
    }

    pub(crate) fn trusted_peers(&self) -> Vec<TrustedPeer> {
        self.trust_store
            .read()
            .expect("trust store lock poisoned")
            .clone()
    }

    pub(crate) fn trusted_peers_handle(&self) -> Arc<RwLock<Vec<TrustedPeer>>> {
        Arc::clone(&self.trust_store)
    }

    pub(crate) fn trust_peer_record(&mut self, peer: TrustedPeer) {
        self.trust_peer(peer);
    }

    pub(crate) fn revoke_peer_record(&mut self, node_id: iroh::NodeId) {
        self.revoke_peer(node_id);
    }

    pub(crate) fn grant_workspace_access(
        &mut self,
        node_id: iroh::NodeId,
        workspace_id: &str,
        access: crate::mods::native::verse::AccessLevel,
    ) {
        let mut peers = self
            .trust_store
            .write()
            .expect("trust store lock poisoned");
        if let Some(peer) = peers.iter_mut().find(|peer| peer.node_id == node_id) {
            peer.workspace_grants
                .retain(|grant| grant.workspace_id != workspace_id);
            peer.workspace_grants
                .push(crate::mods::native::verse::WorkspaceGrant {
                    workspace_id: workspace_id.to_string(),
                    access,
                });
        }
        drop(peers);
        persist_trust_store(&self.trust_store);
    }

    pub(crate) fn revoke_workspace_access(
        &mut self,
        node_id: iroh::NodeId,
        workspace_id: &str,
    ) {
        let mut peers = self
            .trust_store
            .write()
            .expect("trust store lock poisoned");
        if let Some(peer) = peers.iter_mut().find(|peer| peer.node_id == node_id) {
            peer.workspace_grants
                .retain(|grant| grant.workspace_id != workspace_id);
        }
        drop(peers);
        persist_trust_store(&self.trust_store);
    }
}

impl Default for IdentityRegistry {
    fn default() -> Self {
        let mut registry = Self {
            keys: HashMap::new(),
            user_keys: HashMap::new(),
            fallback_id: IDENTITY_ID_DEFAULT.to_string(),
            user_fallback_id: IDENTITY_ID_DEFAULT.to_string(),
            trust_store: Arc::new(RwLock::new(load_trust_store())),
        };

        #[cfg(test)]
        {
            registry.register_generated_persona(IDENTITY_ID_DEFAULT);
            registry.register_generated_persona(IDENTITY_ID_P2P);
            registry
                .user_keys
                .insert(IDENTITY_ID_DEFAULT.to_string(), UserIdentityKey::generate_ephemeral());
        }

        #[cfg(not(test))]
        {
            if let Some(store_root) = default_identity_store_dir() {
                registry.load_persistent_personas(&store_root);
                registry.load_persistent_user_personas(&store_root);
            } else {
                registry.register_generated_persona(IDENTITY_ID_DEFAULT);
                registry.register_generated_persona(IDENTITY_ID_P2P);
                registry
                    .user_keys
                    .insert(IDENTITY_ID_DEFAULT.to_string(), UserIdentityKey::generate_ephemeral());
            }
        }

        registry.register_locked_persona(IDENTITY_ID_LOCKED);
        registry
    }
}

impl P2PIdentityExt for IdentityRegistry {
    fn p2p_node_id(&self) -> iroh::NodeId {
        let resolution = self.resolve(IDENTITY_ID_P2P);
        let Some(identity_key) = self.keys.get(&resolution.resolved_id) else {
            return iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        };
        let Some(signing_key) = identity_key.signing_key.as_ref() else {
            return iroh::SecretKey::generate(&mut rand::thread_rng()).public();
        };
        iroh::SecretKey::from_bytes(&signing_key.to_bytes()).public()
    }

    fn sign_sync_payload(&self, payload: &[u8]) -> Vec<u8> {
        let resolution = self.resolve(IDENTITY_ID_P2P);
        self.keys
            .get(&resolution.resolved_id)
            .and_then(|key| key.sign(payload))
            .map(|signature| signature.to_bytes().to_vec())
            .unwrap_or_default()
    }

    fn verify_peer_signature(&self, peer: iroh::NodeId, payload: &[u8], sig: &[u8]) -> bool {
        let Ok(verifying_key) = VerifyingKey::from_bytes(peer.as_bytes()) else {
            return false;
        };
        let Ok(signature) = Signature::from_slice(sig) else {
            return false;
        };
        verifying_key.verify(payload, &signature).is_ok()
    }

    fn get_trusted_peers(&self) -> Vec<TrustedPeer> {
        self.trust_store
            .read()
            .expect("trust store lock poisoned")
            .clone()
    }

    fn trust_peer(&mut self, peer: TrustedPeer) {
        self.revoke_peer(peer.node_id);
        self.trust_store
            .write()
            .expect("trust store lock poisoned")
            .push(peer);
        persist_trust_store(&self.trust_store);
    }

    fn revoke_peer(&mut self, node_id: iroh::NodeId) {
        self.trust_store
            .write()
            .expect("trust store lock poisoned")
            .retain(|peer| peer.node_id != node_id);
        persist_trust_store(&self.trust_store);
    }
}

fn parse_signature(signature: &str) -> Option<Signature> {
    let encoded = signature.trim().strip_prefix("sig:")?;
    let bytes = decode_hex(encoded).ok()?;
    Signature::from_slice(&bytes).ok()
}

fn parse_schnorr_signature(signature: &str) -> Option<SchnorrSignature> {
    let encoded = signature.trim().strip_prefix("sig:")?;
    let bytes = decode_hex(encoded).ok()?;
    if bytes.len() != 64 {
        return None;
    }
    let mut array = [0u8; 64];
    array.copy_from_slice(&bytes);
    Some(SchnorrSignature::from_byte_array(array))
}

fn canonical_presence_binding_bytes(assertion: &PresenceBindingAssertion) -> Vec<u8> {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        PRESENCE_BINDING_VERSION,
        assertion.user_identity.identity_id,
        user_identity_protocol_label(assertion.user_identity.protocol),
        assertion.user_identity.public_key,
        assertion.node_id,
        assertion.issued_at_secs,
        assertion.expires_at_secs,
        assertion.audience,
    )
    .into_bytes()
}

fn user_identity_protocol_label(protocol: UserIdentityProtocol) -> &'static str {
    match protocol {
        UserIdentityProtocol::LocalNostrSecp256k1 => "local-nostr-secp256k1",
        UserIdentityProtocol::NostrPubkey => "nostr-pubkey",
        UserIdentityProtocol::DidPlc => "did-plc",
    }
}

fn seed_path_for(store_root: &Path, identity_id: &str) -> PathBuf {
    store_root.join(format!("{}.seed", identity_id.replace(':', "__")))
}

fn user_seed_path_for(store_root: &Path, identity_id: &str) -> PathBuf {
    store_root.join(format!("{}.user.seed", identity_id.replace(':', "__")))
}

fn archive_path_for(store_root: &Path, identity_id: &str) -> PathBuf {
    store_root.join(format!("{}.archive", identity_id.replace(':', "__")))
}

fn load_archived_verifying_keys(path: &Path) -> Result<Vec<VerifyingKey>, IdentityKeyError> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| IdentityKeyError::Io(format!("read archived identity keys: {error}")))?;
    let mut keys = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let bytes = decode_hex(line).map_err(|_| IdentityKeyError::InvalidKeyMaterial)?;
        if bytes.len() != 32 {
            return Err(IdentityKeyError::InvalidKeyMaterial);
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        let verifying_key =
            VerifyingKey::from_bytes(&array).map_err(|_| IdentityKeyError::InvalidKeyMaterial)?;
        keys.push(verifying_key);
    }
    Ok(keys)
}

#[cfg(not(test))]
fn default_identity_store_dir() -> Option<PathBuf> {
    let mut dir = dirs::config_dir()?;
    dir.push("graphshell");
    dir.push("identity");
    Some(dir)
}

#[cfg(test)]
fn default_identity_store_dir() -> Option<PathBuf> {
    None
}

fn trust_store_path() -> Option<PathBuf> {
    let mut path = dirs::config_dir()?;
    path.push("graphshell");
    path.push("verse_trusted_peers.json");
    Some(path)
}

#[cfg(not(test))]
fn load_trust_store() -> Vec<TrustedPeer> {
    let Some(path) = trust_store_path() else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

#[cfg(test)]
fn load_trust_store() -> Vec<TrustedPeer> {
    Vec::new()
}

#[cfg(not(test))]
fn persist_trust_store(trust_store: &Arc<RwLock<Vec<TrustedPeer>>>) {
    let Some(path) = trust_store_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let peers = trust_store
        .read()
        .expect("trust store lock poisoned")
        .clone();
    let Ok(content) = serde_json::to_string_pretty(&peers) else {
        return;
    };
    let _ = fs::write(path, content);
}

#[cfg(test)]
fn persist_trust_store(_trust_store: &Arc<RwLock<Vec<TrustedPeer>>>) {}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(nibble_to_hex(byte >> 4));
        output.push(nibble_to_hex(byte & 0x0f));
    }
    output
}

fn decode_hex(encoded: &str) -> Result<Vec<u8>, ()> {
    if encoded.len() % 2 != 0 {
        return Err(());
    }

    let mut output = Vec::with_capacity(encoded.len() / 2);
    let mut chars = encoded.as_bytes().iter().copied();
    while let (Some(high), Some(low)) = (chars.next(), chars.next()) {
        let high = hex_to_nibble(high)?;
        let low = hex_to_nibble(low)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => unreachable!("nibble_to_hex only accepts 0..=15"),
    }
}

fn hex_to_nibble(value: u8) -> Result<u8, ()> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn identity_registry_signs_and_verifies_with_default_persona() {
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
        let verify = registry.verify(
            IDENTITY_ID_DEFAULT,
            b"payload",
            result.signature.as_deref().unwrap_or_default(),
        );
        assert!(verify.verified);
    }

    #[test]
    fn identity_registry_verify_rejects_wrong_payload() {
        let registry = IdentityRegistry::default();
        let signed = registry
            .sign(IDENTITY_ID_DEFAULT, b"payload")
            .signature
            .expect("signature should be produced");

        let verify = registry.verify(IDENTITY_ID_DEFAULT, b"wrong", &signed);
        assert!(!verify.verified);
    }

    #[test]
    fn identity_registry_load_or_generate_persists_seed_material() {
        let tempdir = TempDir::new().expect("temp dir should be created");
        let (generated, created) = IdentityKey::load_or_generate(IDENTITY_ID_DEFAULT, tempdir.path())
            .expect("identity key should be generated");
        assert!(created);

        let (reloaded, created_again) = IdentityKey::load_or_generate(IDENTITY_ID_DEFAULT, tempdir.path())
            .expect("identity key should reload");
        assert!(!created_again);
        assert_eq!(
            generated.verifying_key.as_bytes(),
            reloaded.verifying_key.as_bytes()
        );
    }

    #[test]
    fn identity_registry_rotation_archives_previous_verifying_key() {
        let mut registry = IdentityRegistry::default();
        let original = registry
            .verifying_key_hex_for(IDENTITY_ID_DEFAULT)
            .expect("verifying key should exist");
        let rotated = registry
            .rotate_key(IDENTITY_ID_DEFAULT)
            .expect("key rotation should succeed");
        assert_ne!(original, rotated);
    }

    #[test]
    fn identity_registry_reports_key_unavailable_for_locked_persona() {
        let registry = IdentityRegistry::default();
        let result = registry.sign(IDENTITY_ID_LOCKED, b"payload");

        assert!(!result.succeeded);
        assert!(!result.resolution.key_available);
        assert_eq!(result.resolution.resolved_id, IDENTITY_ID_LOCKED);
    }

    #[test]
    fn identity_registry_builds_signed_presence_binding_assertion() {
        let registry = IdentityRegistry::default();

        let assertion = registry
            .create_presence_binding_assertion_at(IDENTITY_ID_DEFAULT, "local:mdns", 60, 100)
            .expect("presence binding should be created");

        assert_eq!(assertion.node_id, registry.p2p_node_id().to_string());
        assert_eq!(assertion.user_identity.identity_id, IDENTITY_ID_DEFAULT);
        assert_eq!(
            assertion.user_identity.protocol,
            UserIdentityProtocol::LocalNostrSecp256k1
        );
        assert_eq!(assertion.issued_at_secs, 100);
        assert_eq!(assertion.expires_at_secs, 160);
        assert!(assertion.signature.starts_with("sig:"));
        assert!(registry.verify_presence_binding_assertion(&assertion));
    }

    #[test]
    fn identity_registry_default_user_claim_uses_secp256k1_protocol() {
        let registry = IdentityRegistry::default();
        let claim = registry
            .default_user_identity_claim()
            .expect("default user claim should exist");

        assert_eq!(claim.identity_id, IDENTITY_ID_DEFAULT);
        assert_eq!(claim.protocol, UserIdentityProtocol::LocalNostrSecp256k1);
        assert_eq!(claim.public_key.len(), 64);
    }

    #[test]
    fn identity_registry_rejects_tampered_presence_binding_assertion() {
        let registry = IdentityRegistry::default();
        let mut assertion = registry
            .create_presence_binding_assertion_at(IDENTITY_ID_DEFAULT, "local:mdns", 60, 100)
            .expect("presence binding should be created");

        assertion.node_id = iroh::SecretKey::generate(&mut rand::thread_rng())
            .public()
            .to_string();

        assert!(!registry.verify_presence_binding_assertion(&assertion));
    }
}
