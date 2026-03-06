use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use ed25519_dalek::Signer;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

use crate::registries::infrastructure::mod_loader::runtime_has_capability;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};

use super::{
    CHANNEL_NOSTR_CAPABILITY_DENIED, CHANNEL_NOSTR_INTENT_REJECTED,
    CHANNEL_NOSTR_RELAY_PUBLISH_FAILED, CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
    CHANNEL_NOSTR_SECURITY_VIOLATION, CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NostrFilterSet {
    pub(crate) kinds: Vec<u16>,
    pub(crate) authors: Vec<String>,
    pub(crate) hashtags: Vec<String>,
    pub(crate) relay_urls: Vec<String>,
}

impl NostrFilterSet {
    pub(crate) fn is_empty(&self) -> bool {
        self.kinds.is_empty() && self.authors.is_empty() && self.hashtags.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NostrUnsignedEvent {
    pub(crate) kind: u16,
    pub(crate) content: String,
    pub(crate) tags: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NostrSignedEvent {
    pub(crate) event_id: String,
    pub(crate) pubkey: String,
    pub(crate) signature: String,
    pub(crate) kind: u16,
    pub(crate) content: String,
    pub(crate) tags: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NostrSubscriptionHandle {
    pub(crate) id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NostrPublishReceipt {
    pub(crate) accepted: bool,
    pub(crate) relay_count: usize,
    pub(crate) note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NostrCoreError {
    CapabilityDenied {
        capability: &'static str,
        reason: String,
    },
    ValidationFailed(String),
    BackendUnavailable(String),
    QuotaExceeded(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Nip46DelegateConfig {
    pub(crate) relay_url: String,
    pub(crate) signer_pubkey: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NostrSignerBackend {
    LocalHostKey,
    Nip46Delegated(Nip46DelegateConfig),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum RelayPolicyProfile {
    /// Only explicitly allowlisted relays are accepted.
    Strict,
    /// Community default relays + allowlist are accepted.
    #[default]
    Community,
    /// Any relay except explicitly blocked relays is accepted.
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NostrRelayPolicy {
    pub(crate) profile: RelayPolicyProfile,
    pub(crate) allowlist: Vec<String>,
    pub(crate) blocklist: Vec<String>,
    pub(crate) default_relays: Vec<String>,
    pub(crate) max_subscriptions_per_caller: usize,
    pub(crate) max_publishes_per_caller: usize,
}

impl Default for NostrRelayPolicy {
    fn default() -> Self {
        Self {
            profile: RelayPolicyProfile::Community,
            allowlist: Vec::new(),
            blocklist: Vec::new(),
            default_relays: vec!["wss://relay.damus.io".to_string()],
            max_subscriptions_per_caller: 32,
            max_publishes_per_caller: 256,
        }
    }
}

trait NostrRelayService {
    fn subscribe(
        &mut self,
        caller_id: &str,
        requested_id: Option<&str>,
        filters: NostrFilterSet,
        resolved_relays: &[String],
        id_counter: &AtomicU64,
    ) -> Result<NostrSubscriptionHandle, NostrCoreError>;
    fn unsubscribe(&mut self, caller_id: &str, handle: &NostrSubscriptionHandle) -> bool;
    fn publish(
        &mut self,
        caller_id: &str,
        signed: &NostrSignedEvent,
        resolved_relays: &[String],
    ) -> Result<NostrPublishReceipt, NostrCoreError>;
}

#[derive(Default)]
struct InProcessRelayService {
    subscriptions: HashMap<String, (String, NostrFilterSet, Vec<String>)>,
}

impl NostrRelayService for InProcessRelayService {
    fn subscribe(
        &mut self,
        caller_id: &str,
        requested_id: Option<&str>,
        filters: NostrFilterSet,
        resolved_relays: &[String],
        id_counter: &AtomicU64,
    ) -> Result<NostrSubscriptionHandle, NostrCoreError> {
        if filters.is_empty() {
            return Err(NostrCoreError::ValidationFailed(
                "filter set must not be empty".to_string(),
            ));
        }

        let id = requested_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| {
                let seq = id_counter.fetch_add(1, Ordering::Relaxed);
                format!("nostr-sub-{seq}")
            });

        self.subscriptions
            .insert(id.clone(), (caller_id.to_string(), filters, resolved_relays.to_vec()));
        Ok(NostrSubscriptionHandle { id })
    }

    fn unsubscribe(&mut self, caller_id: &str, handle: &NostrSubscriptionHandle) -> bool {
        if let Some((owner, _, _)) = self.subscriptions.get(&handle.id)
            && owner != caller_id
        {
            return false;
        }
        self.subscriptions.remove(&handle.id).is_some()
    }

    fn publish(
        &mut self,
        _caller_id: &str,
        signed: &NostrSignedEvent,
        resolved_relays: &[String],
    ) -> Result<NostrPublishReceipt, NostrCoreError> {
        if signed.signature.trim().is_empty() {
            return Err(NostrCoreError::ValidationFailed(
                "signed event signature must not be empty".to_string(),
            ));
        }

        // Scaffold behavior: acknowledge publish intent with no live relay transport yet.
        Ok(NostrPublishReceipt {
            accepted: true,
            relay_count: resolved_relays.len(),
            note: "accepted by scaffold host (relay transport pending)".to_string(),
        })
    }
}

struct NostrCoreState {
    relay_service: InProcessRelayService,
    signer_backend: NostrSignerBackend,
    local_signing_key: ed25519_dalek::SigningKey,
    relay_policy: NostrRelayPolicy,
    caller_subscription_count: HashMap<String, usize>,
    caller_publish_count: HashMap<String, usize>,
}

impl Default for NostrCoreState {
    fn default() -> Self {
        Self {
            relay_service: InProcessRelayService::default(),
            signer_backend: NostrSignerBackend::LocalHostKey,
            local_signing_key: ed25519_dalek::SigningKey::generate(&mut OsRng),
            relay_policy: NostrRelayPolicy::default(),
            caller_subscription_count: HashMap::new(),
            caller_publish_count: HashMap::new(),
        }
    }
}

#[derive(Default)]
pub(crate) struct NostrCoreRegistry {
    state: Mutex<NostrCoreState>,
    next_subscription_id: AtomicU64,
}

impl NostrCoreRegistry {
    pub(crate) fn set_relay_policy_profile(&self, profile: RelayPolicyProfile) {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.relay_policy.profile = profile;
    }

    pub(crate) fn set_relay_allowlist(&self, relays: Vec<String>) {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.relay_policy.allowlist = normalize_relays(relays);
    }

    pub(crate) fn set_relay_blocklist(&self, relays: Vec<String>) {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.relay_policy.blocklist = normalize_relays(relays);
    }

    pub(crate) fn set_default_relays(&self, relays: Vec<String>) {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.relay_policy.default_relays = normalize_relays(relays);
    }

    pub(crate) fn set_caller_quotas(&self, max_subscriptions: usize, max_publishes: usize) {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.relay_policy.max_subscriptions_per_caller = max_subscriptions.max(1);
        state.relay_policy.max_publishes_per_caller = max_publishes.max(1);
    }

    pub(crate) fn use_local_signer(&self) {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.signer_backend = NostrSignerBackend::LocalHostKey;
    }

    pub(crate) fn use_nip46_signer(
        &self,
        relay_url: &str,
        signer_pubkey: &str,
    ) -> Result<(), NostrCoreError> {
        let relay_url = relay_url.trim();
        let signer_pubkey = signer_pubkey.trim();
        if relay_url.is_empty() || signer_pubkey.is_empty() {
            return Err(NostrCoreError::ValidationFailed(
                "relay_url and signer_pubkey must be non-empty".to_string(),
            ));
        }

        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.signer_backend = NostrSignerBackend::Nip46Delegated(Nip46DelegateConfig {
            relay_url: relay_url.to_string(),
            signer_pubkey: signer_pubkey.to_string(),
        });
        Ok(())
    }

    fn ensure_capability(
        &self,
        capability: &'static str,
        channel: &'static str,
    ) -> Result<(), NostrCoreError> {
        if runtime_has_capability(capability) {
            return Ok(());
        }

        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_NOSTR_CAPABILITY_DENIED,
            byte_len: capability.len(),
        });
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: channel,
            byte_len: capability.len(),
        });
        Err(NostrCoreError::CapabilityDenied {
            capability,
            reason: format!("missing runtime capability: {capability}"),
        })
    }

    pub(crate) fn sign_event(
        &self,
        persona: &str,
        unsigned: &NostrUnsignedEvent,
    ) -> Result<NostrSignedEvent, NostrCoreError> {
        self.ensure_capability("identity:nostr-sign", CHANNEL_NOSTR_SIGN_REQUEST_DENIED)?;

        if persona.trim().is_empty() {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                byte_len: 0,
            });
            return Err(NostrCoreError::ValidationFailed(
                "persona must not be empty".to_string(),
            ));
        }

        let canonical = canonical_event_bytes(unsigned);
        let event_id = to_hex(Sha256::digest(&canonical).as_slice());

        let state = self.state.lock().expect("nostr core lock poisoned");
        let (signature, pubkey) = match &state.signer_backend {
            NostrSignerBackend::LocalHostKey => {
                let payload_digest = Sha256::digest(&canonical);
                let signature = state.local_signing_key.sign(payload_digest.as_slice());
                (
                    to_hex(signature.to_bytes().as_slice()),
                    to_hex(state.local_signing_key.verifying_key().as_bytes()),
                )
            }
            NostrSignerBackend::Nip46Delegated(config) => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                    byte_len: config.relay_url.len() + config.signer_pubkey.len(),
                });
                return Err(NostrCoreError::BackendUnavailable(
                    "NIP-46 delegated signer not implemented yet".to_string(),
                ));
            }
        };

        Ok(NostrSignedEvent {
            event_id,
            pubkey,
            signature,
            kind: unsigned.kind,
            content: unsigned.content.clone(),
            tags: unsigned.tags.clone(),
        })
    }

    pub(crate) fn relay_subscribe(
        &self,
        caller_id: &str,
        requested_id: Option<&str>,
        filters: NostrFilterSet,
    ) -> Result<NostrSubscriptionHandle, NostrCoreError> {
        self.ensure_capability(
            "nostr:relay-subscribe",
            CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
        )?;

        let caller_id = caller_id.trim().to_ascii_lowercase();
        if caller_id.is_empty() {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
                byte_len: 0,
            });
            return Err(NostrCoreError::ValidationFailed(
                "caller_id must not be empty".to_string(),
            ));
        }

        let mut state = self.state.lock().expect("nostr core lock poisoned");
        if !self.within_subscription_quota(&state, &caller_id) {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
                byte_len: caller_id.len(),
            });
            return Err(NostrCoreError::QuotaExceeded(format!(
                "subscription quota exceeded for caller '{caller_id}'"
            )));
        }

        let resolved_relays = self.resolve_and_validate_relays(&state.relay_policy, &filters)?;
        let handle = state
            .relay_service
            .subscribe(
                &caller_id,
                requested_id,
                filters,
                &resolved_relays,
                &self.next_subscription_id,
            )
            .inspect_err(|_| {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
                    byte_len: requested_id.map(str::len).unwrap_or(0),
                });
            })?;

        *state
            .caller_subscription_count
            .entry(caller_id)
            .or_insert(0usize) += 1;
        Ok(handle)
    }

    pub(crate) fn relay_unsubscribe(&self, caller_id: &str, handle: &NostrSubscriptionHandle) -> bool {
        let caller_id = caller_id.trim().to_ascii_lowercase();
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        let removed = state.relay_service.unsubscribe(&caller_id, handle);
        if removed
            && let Some(count) = state.caller_subscription_count.get_mut(&caller_id)
        {
            *count = count.saturating_sub(1);
        }
        removed
    }

    pub(crate) fn relay_publish(
        &self,
        caller_id: &str,
        signed: &NostrSignedEvent,
    ) -> Result<NostrPublishReceipt, NostrCoreError> {
        self.relay_publish_internal(caller_id, signed, None)
    }

    pub(crate) fn relay_publish_to_relays(
        &self,
        caller_id: &str,
        signed: &NostrSignedEvent,
        relay_urls: &[String],
    ) -> Result<NostrPublishReceipt, NostrCoreError> {
        self.relay_publish_internal(caller_id, signed, Some(relay_urls))
    }

    fn relay_publish_internal(
        &self,
        caller_id: &str,
        signed: &NostrSignedEvent,
        requested_relays: Option<&[String]>,
    ) -> Result<NostrPublishReceipt, NostrCoreError> {
        self.ensure_capability("nostr:relay-publish", CHANNEL_NOSTR_RELAY_PUBLISH_FAILED)?;

        let caller_id = caller_id.trim().to_ascii_lowercase();
        if caller_id.is_empty() {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_RELAY_PUBLISH_FAILED,
                byte_len: 0,
            });
            return Err(NostrCoreError::ValidationFailed(
                "caller_id must not be empty".to_string(),
            ));
        }

        let mut state = self.state.lock().expect("nostr core lock poisoned");
        if !self.within_publish_quota(&state, &caller_id) {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_RELAY_PUBLISH_FAILED,
                byte_len: caller_id.len(),
            });
            return Err(NostrCoreError::QuotaExceeded(format!(
                "publish quota exceeded for caller '{caller_id}'"
            )));
        }

        let resolved_relays =
            self.resolve_and_validate_publish_relays(&state.relay_policy, requested_relays)?;
        let result = state
            .relay_service
            .publish(&caller_id, signed, &resolved_relays)
            .inspect_err(|err| {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_RELAY_PUBLISH_FAILED,
                    byte_len: signed.event_id.len(),
                });
                if matches!(err, NostrCoreError::ValidationFailed(_)) {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_NOSTR_SECURITY_VIOLATION,
                        byte_len: signed.event_id.len(),
                    });
                }
            })?;

        *state.caller_publish_count.entry(caller_id).or_insert(0usize) += 1;
        Ok(result)
    }

    pub(crate) fn report_intent_rejected(&self, byte_len: usize) {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_NOSTR_INTENT_REJECTED,
            byte_len,
        });
    }

    fn within_subscription_quota(&self, state: &NostrCoreState, caller_id: &str) -> bool {
        state
            .caller_subscription_count
            .get(caller_id)
            .copied()
            .unwrap_or(0)
            < state.relay_policy.max_subscriptions_per_caller
    }

    fn within_publish_quota(&self, state: &NostrCoreState, caller_id: &str) -> bool {
        state.caller_publish_count.get(caller_id).copied().unwrap_or(0)
            < state.relay_policy.max_publishes_per_caller
    }

    fn resolve_and_validate_publish_relays(
        &self,
        policy: &NostrRelayPolicy,
        requested_relays: Option<&[String]>,
    ) -> Result<Vec<String>, NostrCoreError> {
        let requested = match requested_relays {
            Some(relays) if !relays.is_empty() => relays.to_vec(),
            _ => {
                if policy.default_relays.is_empty() {
                    policy.allowlist.clone()
                } else {
                    policy.default_relays.clone()
                }
            }
        };

        let normalized = normalize_relays(requested);
        if normalized.is_empty() {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_RELAY_PUBLISH_FAILED,
                byte_len: 0,
            });
            return Err(NostrCoreError::ValidationFailed(
                "no publish relays available after normalization".to_string(),
            ));
        }

        for relay in &normalized {
            if !relay_allowed(policy, relay) {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_RELAY_PUBLISH_FAILED,
                    byte_len: relay.len(),
                });
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_SECURITY_VIOLATION,
                    byte_len: relay.len(),
                });
                return Err(NostrCoreError::ValidationFailed(format!(
                    "relay '{relay}' denied by policy"
                )));
            }
        }

        Ok(normalized)
    }

    fn resolve_and_validate_relays(
        &self,
        policy: &NostrRelayPolicy,
        filters: &NostrFilterSet,
    ) -> Result<Vec<String>, NostrCoreError> {
        let requested = if filters.relay_urls.is_empty() {
            if policy.default_relays.is_empty() {
                policy.allowlist.clone()
            } else {
                policy.default_relays.clone()
            }
        } else {
            filters.relay_urls.clone()
        };

        let normalized = normalize_relays(requested);
        if normalized.is_empty() {
            return Err(NostrCoreError::ValidationFailed(
                "no relay candidates after normalization".to_string(),
            ));
        }

        for relay in &normalized {
            if !relay_allowed(policy, relay) {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
                    byte_len: relay.len(),
                });
                return Err(NostrCoreError::ValidationFailed(format!(
                    "relay '{relay}' denied by policy"
                )));
            }
        }

        Ok(normalized)
    }
}

fn canonical_event_bytes(unsigned: &NostrUnsignedEvent) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(format!("kind:{}\n", unsigned.kind).as_bytes());
    buf.extend_from_slice(format!("content:{}\n", unsigned.content).as_bytes());
    for (k, v) in &unsigned.tags {
        buf.extend_from_slice(format!("tag:{}={}\n", k, v).as_bytes());
    }
    buf
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn normalize_relays(relays: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for relay in relays {
        let trimmed = relay.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with("wss://") {
            continue;
        }
        if !out.iter().any(|existing| existing == &trimmed) {
            out.push(trimmed);
        }
    }
    out
}

fn relay_allowed(policy: &NostrRelayPolicy, relay: &str) -> bool {
    if policy.blocklist.iter().any(|blocked| blocked == relay) {
        return false;
    }
    match policy.profile {
        RelayPolicyProfile::Strict => policy.allowlist.iter().any(|allowed| allowed == relay),
        RelayPolicyProfile::Community => {
            policy.default_relays.iter().any(|default| default == relay)
                || policy.allowlist.iter().any(|allowed| allowed == relay)
        }
        RelayPolicyProfile::Open => true,
    }
}

#[cfg(test)]
mod tests {
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;

    use super::*;

    fn channel_message_count(snapshot: &serde_json::Value, channel_id: &str) -> u64 {
        snapshot["channels"]["message_counts"][channel_id]
            .as_u64()
            .unwrap_or(0)
    }

    #[test]
    fn nostr_core_sign_event_produces_cryptographic_local_signature() {
        let registry = NostrCoreRegistry::default();
        let unsigned = NostrUnsignedEvent {
            kind: 1,
            content: "hello nostr".to_string(),
            tags: vec![("t".to_string(), "graph".to_string())],
        };

        let signed = registry.sign_event("default", &unsigned);
        assert!(signed.is_ok());
        let signed = signed.expect("signed event should be produced");
        assert_eq!(signed.signature.len(), 128);
        assert_eq!(signed.event_id.len(), 64);
        assert_eq!(signed.kind, 1);
        assert_eq!(signed.content, "hello nostr");
    }

    #[test]
    fn nostr_core_nip46_backend_is_explicit_stub() {
        let registry = NostrCoreRegistry::default();
        registry
            .use_nip46_signer("wss://relay.example", "npub1delegate")
            .expect("nip46 config should be accepted");

        let unsigned = NostrUnsignedEvent {
            kind: 1,
            content: "hello".to_string(),
            tags: Vec::new(),
        };

        let result = registry.sign_event("default", &unsigned);
        assert!(matches!(result, Err(NostrCoreError::BackendUnavailable(_))));
    }

    #[test]
    fn nostr_core_subscribe_unsubscribe_roundtrip() {
        let registry = NostrCoreRegistry::default();
        let filters = NostrFilterSet {
            kinds: vec![1, 30023],
            authors: vec!["npub1example".to_string()],
            hashtags: vec!["graphshell".to_string()],
            relay_urls: vec!["wss://relay.damus.io".to_string()],
        };

        let handle = registry
            .relay_subscribe("test:c1", Some("timeline"), filters)
            .expect("subscription should be accepted");
        assert_eq!(handle.id, "timeline");
        assert!(registry.relay_unsubscribe("test:c1", &handle));
        assert!(!registry.relay_unsubscribe("test:c1", &handle));
    }

    #[test]
    fn nostr_core_unsubscribe_rejects_non_owner_caller() {
        let registry = NostrCoreRegistry::default();
        let handle = registry
            .relay_subscribe(
                "test:owner",
                Some("owned"),
                NostrFilterSet {
                    kinds: vec![1],
                    authors: vec!["npub1example".to_string()],
                    hashtags: vec![],
                    relay_urls: vec!["wss://relay.damus.io".to_string()],
                },
            )
            .expect("owner subscription should be accepted");

        assert!(!registry.relay_unsubscribe("test:other", &handle));
        assert!(registry.relay_unsubscribe("test:owner", &handle));
    }

    #[test]
    fn nostr_core_enforces_caller_subscription_quota() {
        let registry = NostrCoreRegistry::default();
        registry.set_caller_quotas(1, 5);

        let first = registry.relay_subscribe(
            "quota:c1",
            Some("first"),
            NostrFilterSet {
                kinds: vec![1],
                authors: vec!["npub1example".to_string()],
                hashtags: vec![],
                relay_urls: vec!["wss://relay.damus.io".to_string()],
            },
        );
        assert!(first.is_ok());

        let second = registry.relay_subscribe(
            "quota:c1",
            Some("second"),
            NostrFilterSet {
                kinds: vec![1],
                authors: vec!["npub1example".to_string()],
                hashtags: vec![],
                relay_urls: vec!["wss://relay.damus.io".to_string()],
            },
        );
        assert!(matches!(second, Err(NostrCoreError::QuotaExceeded(_))));
    }

    #[test]
    fn nostr_core_strict_policy_denies_unallowlisted_relay() {
        let registry = NostrCoreRegistry::default();
        registry.set_relay_policy_profile(RelayPolicyProfile::Strict);
        registry.set_relay_allowlist(vec!["wss://relay.allowed".to_string()]);

        let result = registry.relay_subscribe(
            "policy:c1",
            Some("strict"),
            NostrFilterSet {
                kinds: vec![1],
                authors: vec!["npub1example".to_string()],
                hashtags: vec![],
                relay_urls: vec!["wss://relay.denied".to_string()],
            },
        );
        assert!(matches!(
            result,
            Err(NostrCoreError::ValidationFailed(message))
                if message.contains("denied by policy")
        ));
    }

    #[test]
    fn nostr_core_publish_rejects_missing_signature() {
        let registry = NostrCoreRegistry::default();
        let signed = NostrSignedEvent {
            event_id: "evt-1".to_string(),
            pubkey: "pk".to_string(),
            signature: String::new(),
            kind: 1,
            content: "hello".to_string(),
            tags: Vec::new(),
        };

        let result = registry.relay_publish("test:c1", &signed);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(NostrCoreError::ValidationFailed(message))
                if message.contains("signature")
        ));
    }

    #[test]
    fn nostr_core_publish_profile_transition_matrix_applies_policy_rules() {
        let registry = NostrCoreRegistry::default();
        registry.set_default_relays(vec!["wss://relay.default".to_string()]);
        registry.set_relay_allowlist(vec!["wss://relay.allowed".to_string()]);
        let signed = NostrSignedEvent {
            event_id: "evt-2".to_string(),
            pubkey: "pk".to_string(),
            signature: "sig".to_string(),
            kind: 1,
            content: "hello".to_string(),
            tags: Vec::new(),
        };

        registry.set_relay_policy_profile(RelayPolicyProfile::Strict);
        let strict_denied = registry.relay_publish_to_relays(
            "policy:c1",
            &signed,
            &["wss://relay.default".to_string()],
        );
        assert!(matches!(
            strict_denied,
            Err(NostrCoreError::ValidationFailed(message))
                if message.contains("denied by policy")
        ));
        let strict_allowed = registry.relay_publish_to_relays(
            "policy:c1",
            &signed,
            &["wss://relay.allowed".to_string()],
        );
        assert!(strict_allowed.is_ok());

        registry.set_relay_policy_profile(RelayPolicyProfile::Community);
        let community_default = registry.relay_publish_to_relays(
            "policy:c1",
            &signed,
            &["wss://relay.default".to_string()],
        );
        assert!(community_default.is_ok());
        let community_allowlisted = registry.relay_publish_to_relays(
            "policy:c1",
            &signed,
            &["wss://relay.allowed".to_string()],
        );
        assert!(community_allowlisted.is_ok());
        let community_denied = registry.relay_publish_to_relays(
            "policy:c1",
            &signed,
            &["wss://relay.unknown".to_string()],
        );
        assert!(matches!(
            community_denied,
            Err(NostrCoreError::ValidationFailed(message))
                if message.contains("denied by policy")
        ));

        registry.set_relay_policy_profile(RelayPolicyProfile::Open);
        let open_any = registry.relay_publish_to_relays(
            "policy:c1",
            &signed,
            &["wss://relay.unknown".to_string()],
        );
        assert!(open_any.is_ok());
    }

    #[test]
    fn nostr_core_emits_diagnostics_on_quota_and_policy_denials() {
        let mut diagnostics = DiagnosticsState::new();
        let registry = NostrCoreRegistry::default();
        registry.set_caller_quotas(1, 5);

        let _ = registry.relay_subscribe(
            "diag:c1",
            Some("first"),
            NostrFilterSet {
                kinds: vec![1],
                authors: vec!["npub1example".to_string()],
                hashtags: vec![],
                relay_urls: vec!["wss://relay.damus.io".to_string()],
            },
        );
        let _ = registry.relay_subscribe(
            "diag:c1",
            Some("second"),
            NostrFilterSet {
                kinds: vec![1],
                authors: vec!["npub1example".to_string()],
                hashtags: vec![],
                relay_urls: vec!["wss://relay.damus.io".to_string()],
            },
        );

        registry.set_relay_policy_profile(RelayPolicyProfile::Strict);
        registry.set_relay_allowlist(vec!["wss://relay.allowed".to_string()]);
        let _ = registry.relay_publish_to_relays(
            "diag:c2",
            &NostrSignedEvent {
                event_id: "evt-3".to_string(),
                pubkey: "pk".to_string(),
                signature: "sig".to_string(),
                kind: 1,
                content: "hello".to_string(),
                tags: Vec::new(),
            },
            &["wss://relay.denied".to_string()],
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert!(
            channel_message_count(&snapshot, CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED) >= 1,
            "expected subscription failure channel to be emitted"
        );
        assert!(
            channel_message_count(&snapshot, CHANNEL_NOSTR_RELAY_PUBLISH_FAILED) >= 1,
            "expected publish failure channel to be emitted"
        );
        assert!(
            channel_message_count(&snapshot, CHANNEL_NOSTR_SECURITY_VIOLATION) >= 1,
            "expected security violation channel to be emitted"
        );
    }
}
