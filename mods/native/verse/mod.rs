// Verse Native Mod: Tier 1 Direct P2P Sync
//
// Provides bilateral, zero-cost sync between trusted devices via iroh (QUIC + Noise).
// No tokens, no servers, no Tier 2 complexity.
//
// See: design_docs/verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md

use crate::registries::infrastructure::mod_loader::{
    ModCapability, ModManifest, ModType, NativeModRegistration,
};
use crate::desktop::diagnostics::{DiagnosticEvent, emit_event};
use crate::persistence::types::LogEntry;
use keyring::Entry;
use std::sync::OnceLock;

#[cfg(test)]
mod tests;

// Submodules
pub(crate) mod sync_worker;

// Re-exports for ControlPanel integration
pub use sync_worker::{SyncWorker, SyncCommand};

/// The ALPN protocol identifier for Graphshell sync
const SYNC_ALPN: &[u8] = b"graphshell-sync/1";

/// Verse mod manifest - registered at compile time via inventory
pub(crate) fn verse_manifest() -> ModManifest {
    ModManifest::new(
        "verse",
        "Verse — Direct Sync",
        ModType::Native,
        vec![
            "identity:p2p".to_string(),
            "protocol:verse".to_string(),
            "action:verse.pair_device".to_string(),
            "action:verse.sync_now".to_string(),
            "action:verse.share_workspace".to_string(),
            "action:verse.forget_device".to_string(),
        ],
        vec![
            "IdentityRegistry".to_string(),
            "ActionRegistry".to_string(),
            "ProtocolRegistry".to_string(),
            "ControlPanel".to_string(),
            "DiagnosticsRegistry".to_string(),
        ],
        vec![ModCapability::Network, ModCapability::Identity],
    )
}

// Register this mod via inventory at compile time
inventory::submit! {
    NativeModRegistration {
        manifest: verse_manifest,
    }
}

/// Verse mod activation handler — called when this mod is loaded.
pub(crate) fn activate() -> Result<(), String> {
    // Phase 2.2/2.3: When verse is activated, it can register protocol handlers,
    // identity providers, or other protocol-level capabilities.
    // For now, this is a hook point.
    log::debug!("verse: activation hook called");
    Ok(())
}

/// Register Verse protocol handlers into the provider registry.
/// Phase 5 will implement `protocol:verse` handler for P2P sync.
/// For now this is a stub (Phase 2.4 integration point).
pub(crate) fn register_protocol_handlers(providers: &mut crate::registries::atomic::ProtocolHandlerProviders) {
    // TODO: Phase 5.1 — Register protocol:verse handler
    let _ = providers; // Suppress unused warning
    log::debug!("verse: protocol handler registration stub (Phase 5 implementation pending)");
}

/// P2P Identity stored in OS keychain
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct P2PIdentitySecret {
    /// Ed25519 secret key (32 bytes)
    #[serde(with = "secret_key_serde")]
    secret_key: iroh::SecretKey,
    /// Human-readable device name
    device_name: String,
    /// When this identity was created
    #[serde(with = "system_time_serde")]
    created_at: std::time::SystemTime,
}

// Serde helpers for SecretKey (iroh's SecretKey doesn't implement Serialize directly)
mod secret_key_serde {
    use iroh::SecretKey;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(key: &SecretKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as base64 string
        let bytes = key.to_bytes();
        serializer.serialize_str(&base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        ))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SecretKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &s)
            .map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Invalid secret key length"));
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(SecretKey::from_bytes(&array))
    }
}

mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time
            .duration_since(UNIX_EPOCH)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(secs))
    }
}

// ===== Trust Store Types =====

/// Peer role in the trust model
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PeerRole {
    /// Own device — full read/write on all personal workspaces
    #[serde(rename = "self")]
    Self_,
    /// Friend — explicitly added, access is per-workspace
    #[serde(rename = "friend")]
    Friend,
}

/// Access level for workspace sharing
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AccessLevel {
    #[serde(rename = "read_only")]
    ReadOnly,
    #[serde(rename = "read_write")]
    ReadWrite,
}

/// Workspace-specific access grant
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceGrant {
    pub workspace_id: String, // TODO: Use WorkspaceId type when available
    pub access: AccessLevel,
}

/// A trusted peer (own device or friend)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrustedPeer {
    #[serde(with = "node_id_serde")]
    pub node_id: iroh::NodeId,
    pub display_name: String,
    pub role: PeerRole,
    #[serde(with = "system_time_serde")]
    pub added_at: std::time::SystemTime,
    #[serde(with = "option_system_time_serde")]
    pub last_seen: Option<std::time::SystemTime>,
    pub workspace_grants: Vec<WorkspaceGrant>,
}

// Serde helpers for NodeId
mod node_id_serde {
    use iroh::NodeId;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(node_id: &NodeId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&node_id.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NodeId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

mod option_system_time_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &Option<SystemTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match time {
            Some(t) => {
                let duration = t
                    .duration_since(UNIX_EPOCH)
                    .map_err(serde::ser::Error::custom)?;
                serializer.serialize_some(&duration.as_secs())
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SystemTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = Option::<u64>::deserialize(deserializer)?;
        Ok(secs.map(|s| UNIX_EPOCH + std::time::Duration::from_secs(s)))
    }
}

// ===== P2PIdentityExt Trait =====

/// Extension trait for IdentityRegistry to support P2P operations
pub trait P2PIdentityExt {
    /// Get our NodeId (public key)
    fn p2p_node_id(&self) -> iroh::NodeId;
    
    /// Sign a sync payload with our private key
    fn sign_sync_payload(&self, payload: &[u8]) -> Vec<u8>;
    
    /// Verify a peer's signature on a payload
    fn verify_peer_signature(&self, peer: iroh::NodeId, payload: &[u8], sig: &[u8]) -> bool;
    
    /// Get all trusted peers
    fn get_trusted_peers(&self) -> Vec<TrustedPeer>;
    
    /// Add or update a trusted peer
    fn trust_peer(&mut self, peer: TrustedPeer);
    
    /// Revoke trust for a peer (remove from trust store)
    fn revoke_peer(&mut self, node_id: iroh::NodeId);
}

/// Global verse state (initialized once on first access)
struct VerseState {
    /// iroh endpoint for QUIC connections
    endpoint: iroh::Endpoint,
    /// Our node identity
    identity: P2PIdentitySecret,
    /// Trust store (peers we sync with)
    trusted_peers: std::sync::Arc<std::sync::RwLock<Vec<TrustedPeer>>>,
    /// mDNS service daemon (for local network discovery)
    mdns_daemon: Option<mdns_sd::ServiceDaemon>,
    /// Per-workspace sync logs
    sync_logs: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, SyncLog>>>,
}

static VERSE_STATE: OnceLock<VerseState> = OnceLock::new();

/// Initialize the Verse mod (called on app startup if mod is enabled)
pub(crate) fn init() -> Result<(), VerseInitError> {
    // Load or generate P2P identity
    let identity = load_or_generate_identity()?;

    // Create iroh endpoint (requires tokio runtime)
    let endpoint = tokio::runtime::Runtime::new()
        .map_err(|e| VerseInitError::EndpointCreate(format!("tokio runtime: {}", e)))?
        .block_on(async { create_iroh_endpoint(&identity.secret_key).await })?;

    // Load trust store (returns empty vec if none exists)
    let trusted_peers = load_trust_store().unwrap_or_default();

    // Start mDNS advertisement (non-blocking)
    let mdns_daemon = start_mdns_advertisement(&endpoint, &identity.device_name);

    // Store in global state
    VERSE_STATE
        .set(VerseState {
            endpoint,
            identity,
            trusted_peers: std::sync::Arc::new(std::sync::RwLock::new(trusted_peers)),
            mdns_daemon,
            sync_logs: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        })
        .map_err(|_| VerseInitError::AlreadyInitialized)?;

    // Emit diagnostics
    emit_mod_loaded();
    emit_p2p_key_loaded();

    Ok(())
}

/// Get the verse state (panics if not initialized)
fn get_verse_state() -> &'static VerseState {
    VERSE_STATE
        .get()
        .expect("Verse state not initialized - call init() first")
}

/// Check if Verse is initialized (safe, non-panicking)
pub(crate) fn is_initialized() -> bool {
    VERSE_STATE.get().is_some()
}

/// Load identity from OS keychain, or generate a new one if none exists
fn load_or_generate_identity() -> Result<P2PIdentitySecret, VerseInitError> {
    let entry = Entry::new("graphshell", "p2p-identity")
        .map_err(|e| VerseInitError::KeychainAccess(e.to_string()))?;

    match entry.get_password() {
        Ok(json_str) => {
            // Deserialize existing identity
            serde_json::from_str(&json_str).map_err(|e| VerseInitError::IdentityCorrupt(e.to_string()))
        }
        Err(keyring::Error::NoEntry) => {
            // Generate new identity
            let secret_key = iroh::SecretKey::generate(&mut rand::thread_rng());
            let device_name = get_device_name();
            let identity = P2PIdentitySecret {
                secret_key,
                device_name,
                created_at: std::time::SystemTime::now(),
            };

            // Store in keychain
            let json_str =
                serde_json::to_string(&identity).map_err(|e| VerseInitError::IdentitySerialize(e.to_string()))?;
            entry
                .set_password(&json_str)
                .map_err(|e| VerseInitError::KeychainAccess(e.to_string()))?;

            emit_identity_generated();
            Ok(identity)
        }
        Err(e) => Err(VerseInitError::KeychainAccess(e.to_string())),
    }
}

/// Get a human-readable device name
fn get_device_name() -> String {
    // Try to get hostname, fall back to "Unknown Device"
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Unknown Device".to_string())
}

/// Create iroh endpoint with our secret key
async fn create_iroh_endpoint(secret_key: &iroh::SecretKey) -> Result<iroh::Endpoint, VerseInitError> {
    // Create endpoint builder
    let endpoint = iroh::Endpoint::builder()
        .secret_key(secret_key.clone())
        .alpns(vec![SYNC_ALPN.to_vec()])
        .bind()
        .await
        .map_err(|e| VerseInitError::EndpointCreate(e.to_string()))?;

    Ok(endpoint)
}

// ===== Trust Store Persistence =====

/// Load trust store from user_registries.json
fn load_trust_store() -> Result<Vec<TrustedPeer>, std::io::Error> {
    // TODO: Integrate with user_registries.json once RegistryRuntime persistence is wired
    // For now, store in a dedicated file
    let trust_store_path = get_trust_store_path()?;
    
    if !trust_store_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&trust_store_path)?;
    let peers: Vec<TrustedPeer> = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    
    Ok(peers)
}

/// Save trust store to disk
fn save_trust_store(peers: &[TrustedPeer]) -> Result<(), std::io::Error> {
    let trust_store_path = get_trust_store_path()?;
    
    // Ensure parent directory exists
    if let Some(parent) = trust_store_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(peers)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    
    std::fs::write(&trust_store_path, content)?;
    Ok(())
}

/// Get path to trust store file
fn get_trust_store_path() -> Result<std::path::PathBuf, std::io::Error> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "config dir not found"))?;
    
    Ok(config_dir.join("graphshell").join("verse_trusted_peers.json"))
}

// ===== P2P Cryptographic Operations =====

/// Sign a payload with our private key (returns raw signature bytes)
pub(crate) fn sign_sync_payload(payload: &[u8]) -> Vec<u8> {
    let state = get_verse_state();
    // iroh's SecretKey provides a sign() method
    let signature = state.identity.secret_key.sign(payload);
    signature.to_bytes().to_vec()
}

/// Verify a peer's signature on a payload
pub(crate) fn verify_peer_signature(peer: iroh::NodeId, payload: &[u8], sig: &[u8]) -> bool {
    // Convert signature bytes to iroh::Signature (Ed25519 signature)
    if sig.len() != 64 {
        return false;
    }
    
    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(sig);
    
    // Use ed25519_dalek directly for verification (v2.x API)
    use ed25519_dalek::{Verifier, VerifyingKey, Signature};
    
    let public_key_bytes = peer.as_bytes();
    if public_key_bytes.len() != 32 {
        return false;
    }
    
    let Ok(verifying_key) = VerifyingKey::from_bytes(public_key_bytes) else {
        return false;
    };
    
    let Ok(signature) = Signature::try_from(&sig_array[..]) else {
        return false;
    };
    
    verifying_key.verify(payload, &signature).is_ok()
}

// ===== Public P2P Identity API =====

/// Version vector for tracking causal history across peers
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VersionVector {
    /// Maps NodeId → highest sequence number seen from that peer
    #[serde(with = "version_vector_serde")]
    pub clocks: std::collections::HashMap<iroh::NodeId, u64>,
}

// Serde helper for HashMap<NodeId, u64>
mod version_vector_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;

    pub fn serialize<S>(map: &HashMap<iroh::NodeId, u64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string_map: HashMap<String, u64> = map
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        string_map.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<iroh::NodeId, u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string_map = HashMap::<String, u64>::deserialize(deserializer)?;
        string_map
            .into_iter()
            .map(|(k, v)| {
                k.parse::<iroh::NodeId>()
                    .map(|node_id| (node_id, v))
                    .map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

impl VersionVector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge two version vectors (take max per peer)
    pub fn merge(&self, other: &VersionVector) -> VersionVector {
        let mut merged = self.clocks.clone();
        for (peer, &seq) in &other.clocks {
            merged
                .entry(*peer)
                .and_modify(|s| *s = (*s).max(seq))
                .or_insert(seq);
        }
        VersionVector { clocks: merged }
    }

    /// True if self has strictly seen more from every peer than other
    pub fn dominates(&self, other: &VersionVector) -> bool {
        other.clocks.iter().all(|(peer, &seq)| {
            self.clocks.get(peer).copied().unwrap_or(0) >= seq
        })
    }

    /// Increment sequence number for a peer
    pub fn increment(&mut self, peer: iroh::NodeId) -> u64 {
        let seq = self.clocks.entry(peer).or_insert(0);
        *seq += 1;
        *seq
    }

    /// Get current sequence number for a peer
    pub fn get(&self, peer: iroh::NodeId) -> u64 {
        self.clocks.get(&peer).copied().unwrap_or(0)
    }
}

/// Per-workspace sync log (intent history + version vector)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncLog {
    pub workspace_id: String, // TODO: Use WorkspaceId type when available
    /// Current version vector for this workspace
    pub version_vector: VersionVector,
    /// Intent history (kept in memory, persisted encrypted on disk)
    pub intents: Vec<SyncedIntent>,
    /// LWW tracking for node titles
    pub last_write_title: std::collections::HashMap<String, u64>,
    /// LWW tracking for node URLs
    pub last_write_url: std::collections::HashMap<String, u64>,
    /// Tombstones for deleted nodes (ghost-node conflict guard)
    pub tombstones: std::collections::HashMap<String, u64>,
    /// LWW tracking for tag mutations (keyed by node_id|tag)
    pub tag_last_write: std::collections::HashMap<String, u64>,
}

/// A synced intent with causal metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncedIntent {
    pub log_entry: LogEntry,
    /// Which peer originated this intent
    #[serde(with = "node_id_serde")]
    pub authored_by: iroh::NodeId,
    /// Wall clock at origin (for LWW resolution)
    pub authored_at_secs: u64, // SystemTime as unix timestamp
    /// Per-peer monotonic counter
    pub sequence: u64,
}

impl SyncLog {
    pub fn new(workspace_id: String) -> Self {
        Self {
            workspace_id,
            version_vector: VersionVector::new(),
            intents: Vec::new(),
            last_write_title: std::collections::HashMap::new(),
            last_write_url: std::collections::HashMap::new(),
            tombstones: std::collections::HashMap::new(),
            tag_last_write: std::collections::HashMap::new(),
        }
    }

    /// True if this intent should be applied under LWW/ghost-node rules.
    pub fn should_apply(&mut self, intent: &SyncedIntent) -> bool {
        match &intent.log_entry {
            LogEntry::UpdateNodeTitle { node_id, .. } => {
                let last = self.last_write_title.get(node_id).copied().unwrap_or(0);
                let tombstone = self.tombstones.get(node_id).copied().unwrap_or(0);
                if intent.authored_at_secs >= last && intent.authored_at_secs >= tombstone {
                    self.last_write_title.insert(node_id.clone(), intent.authored_at_secs);
                    true
                } else {
                    false
                }
            }
            LogEntry::UpdateNodeUrl { node_id, .. } => {
                let last = self.last_write_url.get(node_id).copied().unwrap_or(0);
                let tombstone = self.tombstones.get(node_id).copied().unwrap_or(0);
                if intent.authored_at_secs >= last && intent.authored_at_secs >= tombstone {
                    self.last_write_url.insert(node_id.clone(), intent.authored_at_secs);
                    true
                } else {
                    false
                }
            }
            LogEntry::RemoveNode { node_id } => {
                let tombstone = self.tombstones.get(node_id).copied().unwrap_or(0);
                if intent.authored_at_secs >= tombstone {
                    self.tombstones.insert(node_id.clone(), intent.authored_at_secs);
                    true
                } else {
                    false
                }
            }
            LogEntry::AddNode { node_id, .. } => {
                let tombstone = self.tombstones.get(node_id).copied().unwrap_or(0);
                intent.authored_at_secs >= tombstone
            }
            LogEntry::TagNode { node_id, tag } | LogEntry::UntagNode { node_id, tag } => {
                let tombstone = self.tombstones.get(node_id).copied().unwrap_or(0);
                if intent.authored_at_secs < tombstone {
                    return false;
                }
                let key = format!("{}|{}", node_id, tag);
                let last = self.tag_last_write.get(&key).copied().unwrap_or(0);
                if intent.authored_at_secs >= last {
                    self.tag_last_write.insert(key, intent.authored_at_secs);
                    true
                } else {
                    false
                }
            }
            _ => true,
        }
    }

    /// Record an intent if it advances the version vector.
    pub fn record_intent(&mut self, intent: SyncedIntent) -> bool {
        let current = self.version_vector.get(intent.authored_by);
        if intent.sequence <= current {
            return false;
        }
        self.version_vector
            .clocks
            .insert(intent.authored_by, intent.sequence);
        self.intents.push(intent);
        true
    }

    /// Serialize to bytes with rkyv (zero-copy binary format)
    pub fn to_bytes(&self) -> Vec<u8> {
        // Use JSON for now (simpler, will migrate to rkyv later when wire protocol is finalized)
        serde_json::to_vec(self).expect("json serialization failed")
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("json deserialization failed: {}", e))
    }

    /// Encrypt bytes with AES-256-GCM
    pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| format!("cipher init failed: {}", e))?;
        
        // Generate random 96-bit nonce
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("encryption failed: {}", e))?;
        
        // Prepend nonce to ciphertext (nonce is not secret, just needs to be unique)
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }

    /// Decrypt bytes with AES-256-GCM
    pub fn decrypt(ciphertext_with_nonce: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
        
        if ciphertext_with_nonce.len() < 12 {
            return Err("ciphertext too short".to_string());
        }
        
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| format!("cipher init failed: {}", e))?;
        
        // Extract nonce (first 12 bytes)
        let nonce = Nonce::from_slice(&ciphertext_with_nonce[0..12]);
        let ciphertext = &ciphertext_with_nonce[12..];
        
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("decryption failed: {}", e))
    }

    /// Save to disk (encrypted with a key derived from our secret key)
    pub fn save_encrypted(&self, secret_key: &iroh::SecretKey) -> Result<(), String> {
        let plaintext = self.to_bytes();
        
        // Derive encryption key from secret key (HKDF would be better, but for now just hash it)
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(secret_key.to_bytes());
        hasher.update(b"synclog-encryption-key-v1");
        let key: [u8; 32] = hasher.finalize().into();
        
        let encrypted = Self::encrypt(&plaintext, &key)?;
        
        let path = get_sync_log_path(&self.workspace_id)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create dir failed: {}", e))?;
        }
        
        std::fs::write(&path, encrypted)
            .map_err(|e| format!("write failed: {}", e))
    }

    /// Load from disk (decrypted with key derived from our secret key)
    pub fn load_encrypted(workspace_id: String, secret_key: &iroh::SecretKey) -> Result<Self, String> {
        let path = get_sync_log_path(&workspace_id)?;
        
        let encrypted = std::fs::read(&path)
            .map_err(|e| format!("read failed: {}", e))?;
        
        // Derive same encryption key
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(secret_key.to_bytes());
        hasher.update(b"synclog-encryption-key-v1");
        let key: [u8; 32] = hasher.finalize().into();
        
        let plaintext = Self::decrypt(&encrypted, &key)?;
        Self::from_bytes(&plaintext)
    }
}

fn get_sync_log_path(workspace_id: &str) -> Result<std::path::PathBuf, String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| "config dir not found".to_string())?;
    
    Ok(config_dir
        .join("graphshell")
        .join("verse_sync_logs")
        .join(format!("{}.bin", workspace_id)))
}


/// Get our NodeId (public key derived from secret key)
pub(crate) fn node_id() -> iroh::NodeId {
    get_verse_state().identity.secret_key.public()
}

/// Get our device name
pub(crate) fn device_name() -> String {
    get_verse_state().identity.device_name.clone()
}

/// Get all trusted peers
pub(crate) fn get_trusted_peers() -> Vec<TrustedPeer> {
    get_verse_state()
        .trusted_peers
    .read()
    .expect("trust store lock poisoned")
        .clone()
}

/// Record a local mutation into the sync log (if Verse is initialized).
pub(crate) fn record_log_entry(workspace_id: &str, entry: LogEntry) -> Result<(), String> {
    let Some(state) = VERSE_STATE.get() else {
        return Ok(());
    };
    let node_id = state.identity.secret_key.public();
    let authored_at_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("clock error: {}", e))?
        .as_secs();

    let mut logs = state
        .sync_logs
        .write()
        .map_err(|_| "sync log lock poisoned".to_string())?;
    let sync_log = logs
        .entry(workspace_id.to_string())
        .or_insert_with(|| SyncLog::new(workspace_id.to_string()));

    let sequence = sync_log.version_vector.get(node_id) + 1;
    let intent = SyncedIntent {
        log_entry: entry,
        authored_by: node_id,
        authored_at_secs,
        sequence,
    };

    if sync_log.record_intent(intent) {
        let _ = sync_log.save_encrypted(&state.identity.secret_key);
    }

    Ok(())
}

/// Shared resources needed to spawn a SyncWorker.
pub(crate) struct SyncWorkerResources {
    pub endpoint: iroh::Endpoint,
    pub secret_key: iroh::SecretKey,
    pub trusted_peers: std::sync::Arc<std::sync::RwLock<Vec<TrustedPeer>>>,
    pub sync_logs: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, SyncLog>>>,
}

pub(crate) fn sync_worker_resources() -> Result<SyncWorkerResources, String> {
    let Some(state) = VERSE_STATE.get() else {
        return Err("verse not initialized".to_string());
    };
    Ok(SyncWorkerResources {
        endpoint: state.endpoint.clone(),
        secret_key: state.identity.secret_key.clone(),
        trusted_peers: state.trusted_peers.clone(),
        sync_logs: state.sync_logs.clone(),
    })
}

/// Shared handle to the sync log map.
pub(crate) fn sync_logs_handle() -> std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, SyncLog>>> {
    let state = get_verse_state();
    state.sync_logs.clone()
}

/// Add or update a trusted peer
pub(crate) fn trust_peer(peer: TrustedPeer) {
    let state = get_verse_state();
    let mut peers = state
        .trusted_peers
        .write()
        .expect("trust store lock poisoned");
    
    // Remove existing peer with same NodeId (update case)
    peers.retain(|p| p.node_id != peer.node_id);
    peers.push(peer);
    
    // Persist to disk
    if let Err(e) = save_trust_store(&peers) {
        log::error!("Failed to save trust store: {}", e);
    }
}

/// Revoke trust for a peer (remove from trust store)
pub(crate) fn revoke_peer(node_id: iroh::NodeId) {
    let state = get_verse_state();
    let mut peers = state
        .trusted_peers
        .write()
        .expect("trust store lock poisoned");
    
    peers.retain(|p| p.node_id != node_id);
    
    // Persist to disk
    if let Err(e) = save_trust_store(&peers) {
        log::error!("Failed to save trust store: {}", e);
    }
}

/// Grant workspace access for a peer
pub(crate) fn grant_workspace_access(node_id: iroh::NodeId, workspace_id: String, access: AccessLevel) {
    let state = get_verse_state();
    let mut peers = state
        .trusted_peers
        .write()
        .expect("trust store lock poisoned");
    
    if let Some(peer) = peers.iter_mut().find(|p| p.node_id == node_id) {
        // Update or insert grant
        if let Some(grant) = peer.workspace_grants.iter_mut().find(|g| g.workspace_id == workspace_id) {
            grant.access = access;
        } else {
            peer.workspace_grants.push(WorkspaceGrant {
                workspace_id,
                access,
            });
        }
        
        // Persist to disk
        if let Err(e) = save_trust_store(&peers) {
            log::error!("Failed to save trust store: {}", e);
        }
    } else {
        log::warn!("grant_workspace_access: peer not found: {}", node_id);
    }
}

/// Revoke workspace access for a peer
pub(crate) fn revoke_workspace_access(node_id: iroh::NodeId, workspace_id: String) {
    let state = get_verse_state();
    let mut peers = state
        .trusted_peers
        .write()
        .expect("trust store lock poisoned");
    
    if let Some(peer) = peers.iter_mut().find(|p| p.node_id == node_id) {
        peer.workspace_grants.retain(|g| g.workspace_id != workspace_id);
        
        // Persist to disk
        if let Err(e) = save_trust_store(&peers) {
            log::error!("Failed to save trust store: {}", e);
        }
    } else {
        log::warn!("revoke_workspace_access: peer not found: {}", node_id);
    }
}

/// Errors that can occur during Verse initialization
#[derive(Debug)]
pub(crate) enum VerseInitError {
    KeychainAccess(String),
    IdentityCorrupt(String),
    IdentitySerialize(String),
    EndpointCreate(String),
    AlreadyInitialized,
}

impl std::fmt::Display for VerseInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeychainAccess(e) => write!(f, "keychain access failed: {}", e),
            Self::IdentityCorrupt(e) => write!(f, "stored identity corrupt: {}", e),
            Self::IdentitySerialize(e) => write!(f, "identity serialization failed: {}", e),
            Self::EndpointCreate(e) => write!(f, "iroh endpoint creation failed: {}", e),
            Self::AlreadyInitialized => write!(f, "verse already initialized"),
        }
    }
}

impl std::error::Error for VerseInitError {}

// ===== Pairing Code Generation (Step 5.3) =====

/// BIP-39 word list (subset for 6-word pairing codes)
/// Full list has 2048 words (11 bits per word). We use first 256 words (8 bits per word).
pub(crate) const PAIRING_WORDLIST: &[&str] = &[
    "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract",
    "absurd", "abuse", "access", "accident", "account", "accuse", "achieve", "acid",
    "acoustic", "acquire", "across", "act", "action", "actor", "actress", "actual",
    "adapt", "add", "addict", "address", "adjust", "admit", "adult", "advance",
    "advice", "aerobic", "affair", "afford", "afraid", "again", "age", "agent",
    "agree", "ahead", "aim", "air", "airport", "aisle", "alarm", "album",
    "alcohol", "alert", "alien", "all", "alley", "allow", "almost", "alone",
    "alpha", "already", "also", "alter", "always", "amateur", "amazing", "among",
    "amount", "amused", "analyst", "anchor", "ancient", "anger", "angle", "angry",
    "animal", "ankle", "announce", "annual", "another", "answer", "antenna", "antique",
    "anxiety", "any", "apart", "apology", "appear", "apple", "approve", "april",
    "arch", "arctic", "area", "arena", "argue", "arm", "armed", "armor",
    "army", "around", "arrange", "arrest", "arrive", "arrow", "art", "artefact",
    "artist", "artwork", "ask", "aspect", "assault", "asset", "assist", "assume",
    "asthma", "athlete", "atom", "attack", "attend", "attitude", "attract", "auction",
    "audit", "august", "aunt", "author", "auto", "autumn", "average", "avocado",
    "avoid", "awake", "aware", "away", "awesome", "awful", "awkward", "axis",
    "baby", "bachelor", "bacon", "badge", "bag", "balance", "balcony", "ball",
    "bamboo", "banana", "banner", "bar", "barely", "bargain", "barrel", "base",
    "basic", "basket", "battle", "beach", "bean", "beauty", "because", "become",
    "beef", "before", "begin", "behave", "behind", "believe", "below", "belt",
    "bench", "benefit", "best", "betray", "better", "between", "beyond", "bicycle",
    "bid", "bike", "bind", "biology", "bird", "birth", "bitter", "black",
    "blade", "blame", "blanket", "blast", "bleak", "bless", "blind", "blood",
    "blossom", "blouse", "blue", "blur", "blush", "board", "boat", "body",
    "boil", "bomb", "bone", "bonus", "book", "boost", "border", "boring",
    "borrow", "boss", "bottom", "bounce", "box", "boy", "bracket", "brain",
    "brand", "brass", "brave", "bread", "breeze", "brick", "bridge", "brief",
    "bright", "bring", "brisk", "broccoli", "broken", "bronze", "broom", "brother",
    "brown", "brush", "bubble", "buddy", "budget", "buffalo", "build", "bulb",
    "bulk", "bullet", "bundle", "bunker", "burden", "burger", "burst", "bus",
    "business", "busy", "butter", "buyer", "buzz", "cabbage", "cabin", "cable",
];

/// Pairing code that expires after 5 minutes
#[derive(Debug, Clone)]
pub struct PairingCode {
    /// The 6-word mnemonic phrase
    pub phrase: String,
    /// iroh::NodeAddr encoded in the code
    pub node_addr: iroh::NodeAddr,
    /// When the code expires
    pub expires_at: std::time::SystemTime,
}

/// Encode our NodeAddr into a 6-word pairing code
pub fn generate_pairing_code() -> Result<PairingCode, String> {
    let state = get_verse_state();
    
    // Get NodeId directly (NodeAddr requires async, but NodeId is synchronous)
    let node_id = state.endpoint.node_id();
    
    // For Step 5.3: encode just the NodeId into 6 words (32 bytes → 6 words using first 6 bytes)
    // Step 5.4 will add relay addresses and full NodeAddr serialization with proper encoding
    let node_id_bytes = node_id.as_bytes();
    
    // Take first 6 bytes and encode as 6 words (8 bits per word)
    let mut words = Vec::with_capacity(6);
    for i in 0..6 {
        let byte_val = node_id_bytes[i];
        let word_idx = byte_val as usize % 256;
        words.push(PAIRING_WORDLIST[word_idx]);
    }
    
    let phrase = words.join("-");
    let expires_at = std::time::SystemTime::now() + std::time::Duration::from_secs(5 * 60);
    
    // Note: For Step 5.3, we create a minimal NodeAddr with just the NodeId
    // Step 5.4 will include full relay and direct addresses
    let node_addr = iroh::NodeAddr::new(node_id);
    
    Ok(PairingCode {
        phrase,
        node_addr,
        expires_at,
    })
}

/// Decode a 6-word pairing code back into a NodeId (simplified for Step 5.3)
/// Full NodeAddr reconstruction requires relay + direct addresses, which we'll add in Step 5.4.
pub fn decode_pairing_code(phrase: &str) -> Result<iroh::NodeId, String> {
    let words: Vec<&str> = phrase.split('-').collect();
    if words.len() != 6 {
        return Err(format!("expected 6 words, got {}", words.len()));
    }
    
    // Decode words back to bytes
    let mut bytes = Vec::with_capacity(6);
    for word in words {
        let word_idx = PAIRING_WORDLIST
            .iter()
            .position(|&w| w == word)
            .ok_or_else(|| format!("unknown word: {}", word))?;
        bytes.push(word_idx as u8);
    }
    
    // For Step 5.3: just reconstruct the NodeId from the first 32 bytes
    // (This is a simplified version; Step 5.4 will use full NodeAddr serialization)
    if bytes.len() < 6 {
        return Err("code too short".to_string());
    }
    
    // Reconstruct node_id from the encoded bytes
    // For now, we'll use a placeholder that extracts partial info
    // In Step 5.4, we'll properly serialize/deserialize the full NodeAddr
    Err("decode_pairing_code not yet fully implemented - requires relay address info from Step 5.4".to_string())
}

/// Generate a QR code for the pairing phrase (returns ASCII art for terminal display)
pub(crate) fn generate_qr_code_ascii(phrase: &str) -> Result<String, String> {
    use qrcode::{QrCode, render::unicode};
    
    let code = QrCode::new(phrase.as_bytes())
        .map_err(|e| format!("QR generation failed: {}", e))?;
    
    let string = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();
    
    Ok(string)
}

/// Generate QR code image data (PNG bytes) for UI display
pub(crate) fn generate_qr_code_png(phrase: &str) -> Result<Vec<u8>, String> {
    use qrcode::{QrCode, render::svg};
    
    // Generate SVG (we'll convert to PNG in the UI layer if needed)
    let code = QrCode::new(phrase.as_bytes())
        .map_err(|e| format!("QR generation failed: {}", e))?;
    
    let svg_string = code
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    
    // For now, return SVG as bytes (UI can render directly)
    Ok(svg_string.into_bytes())
}

// ===== mDNS Advertisement & Discovery (Step 5.3) =====

/// Start advertising our device via mDNS on the local network
fn start_mdns_advertisement(endpoint: &iroh::Endpoint, device_name: &str) -> Option<mdns_sd::ServiceDaemon> {
    match mdns_sd::ServiceDaemon::new() {
        Ok(daemon) => {
            let node_id = endpoint.node_id();
            
            // Service type: _graphshell-sync._udp.local
            let service_type = "_graphshell-sync._udp.local.";
            
            // Instance name: device name
            let instance_name = sanitize_service_name(device_name);
            
            // TXT records: node_id (as hex string)
            let mut properties = std::collections::HashMap::new();
            properties.insert("node_id".to_string(), node_id.to_string());
            
            // Note: We'll add relay URL in Step 5.4 when we handle full NodeAddr encoding
            // For Step 5.3, just advertise NodeId for local network discovery
            
            // Note: Port 0 because iroh uses QUIC with Magic Sockets (not a fixed TCP/UDP port)
            let service_info = mdns_sd::ServiceInfo::new(
                service_type,
                &instance_name,
                &format!("{}.local.", instance_name),
                (), // Empty address (we use relay URLs, not direct IP)
                0,  // Port 0 (iroh handles connectivity)
                Some(properties),
            );
            
            if let Ok(service_info) = service_info {
                if let Err(e) = daemon.register(service_info) {
                    log::warn!("mDNS registration failed: {}", e);
                    return None;
                }
                log::info!("mDNS advertisement started for '{}'", device_name);
                Some(daemon)
            } else {
                log::warn!("mDNS service info creation failed");
                None
            }
        }
        Err(e) => {
            log::warn!("mDNS daemon creation failed: {} - local discovery disabled", e);
            None
        }
    }
}

/// Sanitize device name for mDNS service name (alphanumeric + hyphens only)
pub(crate) fn sanitize_service_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Discovered peer from mDNS
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub device_name: String,
    pub node_id: iroh::NodeId,
    pub relay_url: Option<url::Url>,
}

/// Browse for nearby devices on the local network (blocking for up to timeout_secs)
pub fn discover_nearby_peers(timeout_secs: u64) -> Result<Vec<DiscoveredPeer>, String> {
    let daemon = mdns_sd::ServiceDaemon::new()
        .map_err(|e| format!("mDNS daemon creation failed: {}", e))?;
    
    let service_type = "_graphshell-sync._udp.local.";
    let receiver = daemon.browse(service_type)
        .map_err(|e| format!("mDNS browse failed: {}", e))?;
    
    let mut discovered = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    
    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(event) => {
                if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                    // Extract node_id from TXT records
                    if let Some(node_id_str) = info.get_property_val_str("node_id") {
                        if let Ok(node_id) = node_id_str.parse::<iroh::NodeId>() {
                            let relay_url = info.get_property_val_str("relay")
                                .and_then(|s| s.parse::<url::Url>().ok());
                            
                            discovered.push(DiscoveredPeer {
                                device_name: info.get_fullname().to_string(),
                                node_id,
                                relay_url,
                            });
                        }
                    }
                }
            }
            Err(e) if e.to_string().contains("timeout") || e.to_string().contains("Timeout") => continue,
            Err(_) => break, // Disconnected or other error
        }
    }
    
    Ok(discovered)
}

// ===== Diagnostics =====

/// Diagnostics channel IDs
const CHANNEL_MOD_LOAD_SUCCEEDED: &str = "registry.mod.load_succeeded";
const CHANNEL_IDENTITY_GENERATED: &str = "verse.sync.identity_generated";
const CHANNEL_P2P_KEY_LOADED: &str = "registry.identity.p2p_key_loaded";
const CHANNEL_PAIRING_SUCCEEDED: &str = "verse.sync.pairing_succeeded";
const CHANNEL_PAIRING_FAILED: &str = "verse.sync.pairing_failed";

fn emit_mod_loaded() {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_MOD_LOAD_SUCCEEDED,
        byte_len: "verse".len(),
    });
    log::info!("Verse mod loaded successfully");
}

fn emit_identity_generated() {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_IDENTITY_GENERATED,
        byte_len: 0,
    });
    log::info!("Generated new P2P identity");
}

fn emit_p2p_key_loaded() {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_P2P_KEY_LOADED,
        byte_len: 32, // Ed25519 public key size
    });
}

pub(crate) fn emit_pairing_succeeded(peer_name: &str) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_PAIRING_SUCCEEDED,
        byte_len: peer_name.len(),
    });
    log::info!("Pairing succeeded with {}", peer_name);
}

pub(crate) fn emit_pairing_failed(error: &str) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_PAIRING_FAILED,
        byte_len: error.len(),
    });
    log::warn!("Pairing failed: {}", error);
}
