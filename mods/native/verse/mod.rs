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
use keyring::Entry;
use std::sync::OnceLock;

#[cfg(test)]
mod tests;

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

/// Global verse state (initialized once on first access)
struct VerseState {
    /// iroh endpoint for QUIC connections
    endpoint: iroh::Endpoint,
    /// Our node identity
    identity: P2PIdentitySecret,
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

    // Store in global state
    VERSE_STATE
        .set(VerseState { endpoint, identity })
        .map_err(|_| VerseInitError::AlreadyInitialized)?;

    // Emit diagnostics
    emit_mod_loaded();

    Ok(())
}

/// Get the verse state (panics if not initialized)
fn get_verse_state() -> &'static VerseState {
    VERSE_STATE
        .get()
        .expect("Verse state not initialized - call init() first")
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

/// Get our NodeId (public key derived from secret key)
pub(crate) fn node_id() -> iroh::NodeId {
    get_verse_state().identity.secret_key.public()
}

/// Get our device name
pub(crate) fn device_name() -> &'static str {
    &get_verse_state().identity.device_name
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

// ===== Diagnostics =====

/// Diagnostics channel IDs
const CHANNEL_MOD_LOAD_SUCCEEDED: &str = "registry.mod.load_succeeded";
const CHANNEL_IDENTITY_GENERATED: &str = "verse.sync.identity_generated";

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
