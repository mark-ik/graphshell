use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::SinkExt;
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::registries::infrastructure::mod_loader::runtime_has_capability;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};

use super::identity::IdentityRegistry;
use super::{
    CHANNEL_NOSTR_CAPABILITY_DENIED, CHANNEL_NOSTR_INTENT_REJECTED,
    CHANNEL_NOSTR_RELAY_PUBLISH_FAILED, CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
    CHANNEL_NOSTR_SECURITY_VIOLATION, CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct NostrUnsignedEvent {
    pub(crate) created_at: u64,
    pub(crate) kind: u16,
    pub(crate) content: String,
    pub(crate) tags: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct NostrSignedEvent {
    pub(crate) event_id: String,
    pub(crate) pubkey: String,
    pub(crate) signature: String,
    pub(crate) created_at: u64,
    pub(crate) kind: u16,
    pub(crate) content: String,
    pub(crate) tags: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NostrSubscriptionHandle {
    pub(crate) id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistedNostrSubscription {
    pub(crate) caller_id: String,
    pub(crate) requested_id: Option<String>,
    pub(crate) filters: NostrFilterSet,
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

impl std::fmt::Display for NostrCoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapabilityDenied { reason, .. } => f.write_str(reason),
            Self::ValidationFailed(message)
            | Self::BackendUnavailable(message)
            | Self::QuotaExceeded(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for NostrCoreError {}

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

        self.subscriptions.insert(
            id.clone(),
            (caller_id.to_string(), filters, resolved_relays.to_vec()),
        );
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

#[derive(Debug, Clone)]
pub(crate) struct RelaySubscriptionRequest {
    caller_id: String,
    subscription_id: String,
    filters: NostrFilterSet,
    resolved_relays: Vec<String>,
}

type RelayResponse<T> = std::sync::mpsc::Sender<T>;

#[derive(Debug)]
pub(crate) enum RelayWorkerCommand {
    Subscribe {
        request: RelaySubscriptionRequest,
        response: RelayResponse<Result<NostrSubscriptionHandle, NostrCoreError>>,
    },
    Unsubscribe {
        caller_id: String,
        handle: NostrSubscriptionHandle,
        response: RelayResponse<bool>,
    },
    Publish {
        caller_id: String,
        signed: NostrSignedEvent,
        resolved_relays: Vec<String>,
        response: RelayResponse<Result<NostrPublishReceipt, NostrCoreError>>,
    },
    ReplaceSubscriptions {
        subscriptions: Vec<RelaySubscriptionRequest>,
        response: RelayResponse<Result<usize, NostrCoreError>>,
    },
}

type RelaySocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
struct RelaySubscriptionRecord {
    caller_id: String,
    relays: Vec<String>,
}

#[derive(Default)]
struct TungsteniteRelayService {
    connections: HashMap<String, RelaySocket>,
    subscriptions: HashMap<String, RelaySubscriptionRecord>,
}

impl TungsteniteRelayService {
    async fn subscribe(
        &mut self,
        request: RelaySubscriptionRequest,
    ) -> Result<NostrSubscriptionHandle, NostrCoreError> {
        if request.filters.is_empty() {
            return Err(NostrCoreError::ValidationFailed(
                "filter set must not be empty".to_string(),
            ));
        }

        let handle = NostrSubscriptionHandle {
            id: request.subscription_id,
        };

        let filter_json = nostr_filter_json(&request.filters);
        for relay_url in &request.resolved_relays {
            self.send_json(
                relay_url,
                serde_json::json!(["REQ", handle.id, filter_json.clone()]),
            )
            .await?;
        }
        self.subscriptions.insert(
            handle.id.clone(),
            RelaySubscriptionRecord {
                caller_id: request.caller_id,
                relays: request.resolved_relays,
            },
        );
        Ok(handle)
    }

    async fn unsubscribe(
        &mut self,
        caller_id: &str,
        handle: &NostrSubscriptionHandle,
    ) -> Result<bool, NostrCoreError> {
        let Some(record) = self.subscriptions.get(&handle.id).cloned() else {
            return Ok(false);
        };
        if record.caller_id != caller_id {
            return Ok(false);
        }

        for relay_url in &record.relays {
            self.send_json(relay_url, serde_json::json!(["CLOSE", handle.id]))
                .await?;
        }
        self.subscriptions.remove(&handle.id);
        Ok(true)
    }

    async fn publish(
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

        let event = serde_json::json!({
            "id": signed.event_id,
            "pubkey": signed.pubkey,
            "created_at": signed.created_at,
            "kind": signed.kind,
            "tags": signed.tags,
            "content": signed.content,
            "sig": signed.signature,
        });
        for relay_url in resolved_relays {
            self.send_json(relay_url, serde_json::json!(["EVENT", event.clone()]))
                .await?;
        }

        Ok(NostrPublishReceipt {
            accepted: true,
            relay_count: resolved_relays.len(),
            note: "accepted by websocket relay backend".to_string(),
        })
    }

    async fn replace_subscriptions(
        &mut self,
        subscriptions: Vec<RelaySubscriptionRequest>,
    ) -> Result<usize, NostrCoreError> {
        self.connections.clear();
        self.subscriptions.clear();

        let mut restored = 0usize;
        for request in subscriptions {
            self.subscribe(request).await?;
            restored += 1;
        }
        Ok(restored)
    }

    async fn send_json(
        &mut self,
        relay_url: &str,
        payload: serde_json::Value,
    ) -> Result<(), NostrCoreError> {
        let text = payload.to_string();

        if !self.connections.contains_key(relay_url) {
            let (socket, _) = connect_async(relay_url).await.map_err(|error| {
                NostrCoreError::BackendUnavailable(format!(
                    "relay connect failed for '{relay_url}': {error}"
                ))
            })?;
            self.connections.insert(relay_url.to_string(), socket);
        }

        let send_result = {
            let socket = self
                .connections
                .get_mut(relay_url)
                .expect("relay connection inserted");
            socket.send(Message::Text(text.clone().into())).await
        };
        if send_result.is_ok() {
            return Ok(());
        }

        self.connections.remove(relay_url);
        let (mut socket, _) = connect_async(relay_url).await.map_err(|error| {
            NostrCoreError::BackendUnavailable(format!(
                "relay reconnect failed for '{relay_url}': {error}"
            ))
        })?;
        socket
            .send(Message::Text(text.into()))
            .await
            .map_err(|error| {
                NostrCoreError::BackendUnavailable(format!(
                    "relay send failed for '{relay_url}': {error}"
                ))
            })?;
        self.connections.insert(relay_url.to_string(), socket);
        Ok(())
    }
}

pub(crate) struct NostrRelayWorker {
    command_rx: mpsc::UnboundedReceiver<RelayWorkerCommand>,
    cancel: tokio_util::sync::CancellationToken,
    backend: TungsteniteRelayService,
}

impl NostrRelayWorker {
    pub(crate) fn new(
        command_rx: mpsc::UnboundedReceiver<RelayWorkerCommand>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Self {
        Self {
            command_rx,
            cancel,
            backend: TungsteniteRelayService::default(),
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                command = self.command_rx.recv() => {
                    let Some(command) = command else {
                        break;
                    };
                    self.handle_command(command).await;
                }
            }
        }
    }

    async fn handle_command(&mut self, command: RelayWorkerCommand) {
        match command {
            RelayWorkerCommand::Subscribe { request, response } => {
                let _ = response.send(self.backend.subscribe(request).await);
            }
            RelayWorkerCommand::Unsubscribe {
                caller_id,
                handle,
                response,
            } => {
                let result = self
                    .backend
                    .unsubscribe(&caller_id, &handle)
                    .await
                    .unwrap_or(false);
                let _ = response.send(result);
            }
            RelayWorkerCommand::Publish {
                caller_id,
                signed,
                resolved_relays,
                response,
            } => {
                let _ = response.send(
                    self.backend
                        .publish(&caller_id, &signed, &resolved_relays)
                        .await,
                );
            }
            RelayWorkerCommand::ReplaceSubscriptions {
                subscriptions,
                response,
            } => {
                let _ = response.send(self.backend.replace_subscriptions(subscriptions).await);
            }
        }
    }
}

struct NostrCoreState {
    relay_service: InProcessRelayService,
    relay_worker_tx: Option<mpsc::UnboundedSender<RelayWorkerCommand>>,
    signer_backend: NostrSignerBackend,
    relay_policy: NostrRelayPolicy,
    caller_subscription_count: HashMap<String, usize>,
    caller_publish_count: HashMap<String, usize>,
    active_subscriptions: HashMap<String, PersistedNostrSubscription>,
}

impl Default for NostrCoreState {
    fn default() -> Self {
        Self {
            relay_service: InProcessRelayService::default(),
            relay_worker_tx: None,
            signer_backend: NostrSignerBackend::LocalHostKey,
            relay_policy: NostrRelayPolicy::default(),
            caller_subscription_count: HashMap::new(),
            caller_publish_count: HashMap::new(),
            active_subscriptions: HashMap::new(),
        }
    }
}

#[derive(Default)]
pub(crate) struct NostrCoreRegistry {
    state: Mutex<NostrCoreState>,
    next_subscription_id: AtomicU64,
}

impl NostrCoreRegistry {
    pub(crate) fn attach_supervised_relay_worker(
        &self,
        relay_worker_tx: mpsc::UnboundedSender<RelayWorkerCommand>,
    ) {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.relay_worker_tx = Some(relay_worker_tx);
    }

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
        identity: &IdentityRegistry,
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

        let state = self.state.lock().expect("nostr core lock poisoned");
        let (event_id, signature, pubkey) = match &state.signer_backend {
            NostrSignerBackend::LocalHostKey => {
                let Some(pubkey) = identity.verifying_key_hex_for(persona) else {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                        byte_len: persona.len(),
                    });
                    return Err(NostrCoreError::ValidationFailed(format!(
                        "verifying key unavailable for persona '{persona}'"
                    )));
                };

                let event_hash = canonical_event_hash(&pubkey, unsigned);
                let signed = identity.sign(persona, &event_hash);
                let Some(signature) = signed.signature else {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                        byte_len: persona.len(),
                    });
                    return Err(NostrCoreError::ValidationFailed(format!(
                        "identity key unavailable for persona '{persona}'"
                    )));
                };
                (
                    to_hex(&event_hash),
                    signature.trim_start_matches("sig:").to_string(),
                    pubkey,
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
            created_at: unsigned.created_at,
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

        let (resolved_relays, relay_worker_tx) = {
            let state = self.state.lock().expect("nostr core lock poisoned");
            if !self.within_subscription_quota(&state, &caller_id) {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
                    byte_len: caller_id.len(),
                });
                return Err(NostrCoreError::QuotaExceeded(format!(
                    "subscription quota exceeded for caller '{caller_id}'"
                )));
            }
            (
                self.resolve_and_validate_relays(&state.relay_policy, &filters)?,
                state.relay_worker_tx.clone(),
            )
        };

        let persisted_filters = filters.clone();
        let subscription_id = self.allocate_subscription_id(requested_id);
        let handle = if let Some(relay_worker_tx) = relay_worker_tx {
            match self.request_worker_subscribe(
                &relay_worker_tx,
                RelaySubscriptionRequest {
                    caller_id: caller_id.clone(),
                    subscription_id: subscription_id.clone(),
                    filters,
                    resolved_relays: resolved_relays.clone(),
                },
            )
            {
                Ok(handle) => handle,
                Err(error) if Self::is_worker_unavailable(&error) => {
                    let mut state = self.state.lock().expect("nostr core lock poisoned");
                    state
                        .relay_service
                        .subscribe(
                            &caller_id,
                            Some(subscription_id.as_str()),
                            persisted_filters.clone(),
                            &resolved_relays,
                            &self.next_subscription_id,
                        )
                        .inspect_err(|_| {
                            emit_event(DiagnosticEvent::MessageSent {
                                channel_id: CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
                                byte_len: requested_id.map(str::len).unwrap_or(0),
                            });
                        })?
                }
                Err(error) => {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
                        byte_len: requested_id.map(str::len).unwrap_or(0),
                    });
                    return Err(error);
                }
            }
        } else {
            let mut state = self.state.lock().expect("nostr core lock poisoned");
            state
                .relay_service
                .subscribe(
                    &caller_id,
                    Some(subscription_id.as_str()),
                    filters,
                    &resolved_relays,
                    &self.next_subscription_id,
                )
                .inspect_err(|_| {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_NOSTR_RELAY_SUBSCRIPTION_FAILED,
                        byte_len: requested_id.map(str::len).unwrap_or(0),
                    });
                })?
        };

        let mut state = self.state.lock().expect("nostr core lock poisoned");
        *state
            .caller_subscription_count
            .entry(caller_id.clone())
            .or_insert(0usize) += 1;
        state.active_subscriptions.insert(
            handle.id.clone(),
            PersistedNostrSubscription {
                caller_id: caller_id.clone(),
                requested_id: requested_id.map(str::to_string),
                filters: persisted_filters,
            },
        );
        Ok(handle)
    }

    pub(crate) fn relay_unsubscribe(
        &self,
        caller_id: &str,
        handle: &NostrSubscriptionHandle,
    ) -> bool {
        let caller_id = caller_id.trim().to_ascii_lowercase();
        let relay_worker_tx = {
            let state = self.state.lock().expect("nostr core lock poisoned");
            state.relay_worker_tx.clone()
        };
        let removed = if let Some(relay_worker_tx) = relay_worker_tx {
            match self.request_worker_unsubscribe(&relay_worker_tx, &caller_id, handle) {
                Ok(removed) => removed,
                Err(error) if Self::is_worker_unavailable(&error) => {
                    let mut state = self.state.lock().expect("nostr core lock poisoned");
                    state.relay_service.unsubscribe(&caller_id, handle)
                }
                Err(_) => false,
            }
        } else {
            let mut state = self.state.lock().expect("nostr core lock poisoned");
            state.relay_service.unsubscribe(&caller_id, handle)
        };
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        if removed && let Some(count) = state.caller_subscription_count.get_mut(&caller_id) {
            *count = count.saturating_sub(1);
        }
        if removed {
            state.active_subscriptions.remove(&handle.id);
        }
        removed
    }

    pub(crate) fn persisted_subscriptions(&self) -> Vec<PersistedNostrSubscription> {
        self.state
            .lock()
            .expect("nostr core lock poisoned")
            .active_subscriptions
            .values()
            .cloned()
            .collect()
    }

    pub(crate) fn restore_persisted_subscriptions(
        &self,
        subscriptions: &[PersistedNostrSubscription],
    ) -> Result<usize, NostrCoreError> {
        let (relay_policy, relay_worker_tx) = {
            let state = self.state.lock().expect("nostr core lock poisoned");
            (state.relay_policy.clone(), state.relay_worker_tx.clone())
        };

        let mut normalized = Vec::new();
        let mut persisted = Vec::new();
        for entry in subscriptions {
            let caller_id = entry.caller_id.trim().to_ascii_lowercase();
            if caller_id.is_empty() || entry.filters.is_empty() {
                continue;
            }
            let resolved_relays = self.resolve_and_validate_relays(&relay_policy, &entry.filters)?;
            let subscription_id = self.allocate_subscription_id(entry.requested_id.as_deref());
            normalized.push(RelaySubscriptionRequest {
                caller_id: caller_id.clone(),
                subscription_id: subscription_id.clone(),
                filters: entry.filters.clone(),
                resolved_relays,
            });
            persisted.push((subscription_id, entry.clone(), caller_id));
        }

        let restored = if let Some(relay_worker_tx) = relay_worker_tx {
            match self.request_worker_replace_subscriptions(&relay_worker_tx, normalized.clone()) {
                Ok(restored) => restored,
                Err(error) if Self::is_worker_unavailable(&error) => {
                    let mut state = self.state.lock().expect("nostr core lock poisoned");
                    state.relay_service = InProcessRelayService::default();
                    let mut restored = 0usize;
                    for request in &normalized {
                        state.relay_service.subscribe(
                            &request.caller_id,
                            Some(request.subscription_id.as_str()),
                            request.filters.clone(),
                            &request.resolved_relays,
                            &self.next_subscription_id,
                        )?;
                        restored += 1;
                    }
                    restored
                }
                Err(error) => return Err(error),
            }
        } else {
            let mut state = self.state.lock().expect("nostr core lock poisoned");
            state.relay_service = InProcessRelayService::default();
            let mut restored = 0usize;
            for request in &normalized {
                state.relay_service.subscribe(
                    &request.caller_id,
                    Some(request.subscription_id.as_str()),
                    request.filters.clone(),
                    &request.resolved_relays,
                    &self.next_subscription_id,
                )?;
                restored += 1;
            }
            restored
        };

        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.caller_subscription_count.clear();
        state.active_subscriptions.clear();
        for (handle_id, entry, caller_id) in persisted {
            *state
                .caller_subscription_count
                .entry(caller_id.clone())
                .or_insert(0usize) += 1;
            state.active_subscriptions.insert(
                handle_id,
                PersistedNostrSubscription {
                    caller_id,
                    requested_id: entry.requested_id,
                    filters: entry.filters,
                },
            );
        }
        Ok(restored)
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

        let (resolved_relays, relay_worker_tx) = {
            let state = self.state.lock().expect("nostr core lock poisoned");
            if !self.within_publish_quota(&state, &caller_id) {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_RELAY_PUBLISH_FAILED,
                    byte_len: caller_id.len(),
                });
                return Err(NostrCoreError::QuotaExceeded(format!(
                    "publish quota exceeded for caller '{caller_id}'"
                )));
            }
            (
                self.resolve_and_validate_publish_relays(&state.relay_policy, requested_relays)?,
                state.relay_worker_tx.clone(),
            )
        };
        let result = if let Some(relay_worker_tx) = relay_worker_tx {
            match self.request_worker_publish(&relay_worker_tx, &caller_id, signed, &resolved_relays)
            {
                Ok(result) => Ok(result),
                Err(error) if Self::is_worker_unavailable(&error) => {
                    let mut state = self.state.lock().expect("nostr core lock poisoned");
                    state.relay_service.publish(&caller_id, signed, &resolved_relays)
                }
                Err(error) => Err(error),
            }
        } else {
            let mut state = self.state.lock().expect("nostr core lock poisoned");
            state.relay_service.publish(&caller_id, signed, &resolved_relays)
        }
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

        let mut state = self.state.lock().expect("nostr core lock poisoned");
        *state
            .caller_publish_count
            .entry(caller_id)
            .or_insert(0usize) += 1;
        Ok(result)
    }

    pub(crate) fn report_intent_rejected(&self, byte_len: usize) {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_NOSTR_INTENT_REJECTED,
            byte_len,
        });
    }

    fn is_worker_unavailable(error: &NostrCoreError) -> bool {
        matches!(error, NostrCoreError::BackendUnavailable(message) if message.contains("worker"))
    }

    fn allocate_subscription_id(&self, requested_id: Option<&str>) -> String {
        requested_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| {
                let seq = self.next_subscription_id.fetch_add(1, Ordering::Relaxed);
                format!("nostr-sub-{seq}")
            })
    }

    fn request_worker_subscribe(
        &self,
        relay_worker_tx: &mpsc::UnboundedSender<RelayWorkerCommand>,
        request: RelaySubscriptionRequest,
    ) -> Result<NostrSubscriptionHandle, NostrCoreError> {
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        relay_worker_tx
            .send(RelayWorkerCommand::Subscribe {
                request,
                response: response_tx,
            })
            .map_err(|_| {
                NostrCoreError::BackendUnavailable(
                    "nostr relay worker unavailable for subscribe".to_string(),
                )
            })?;
        response_rx.recv().map_err(|_| {
            NostrCoreError::BackendUnavailable(
                "nostr relay worker did not respond to subscribe".to_string(),
            )
        })?
    }

    fn request_worker_unsubscribe(
        &self,
        relay_worker_tx: &mpsc::UnboundedSender<RelayWorkerCommand>,
        caller_id: &str,
        handle: &NostrSubscriptionHandle,
    ) -> Result<bool, NostrCoreError> {
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        relay_worker_tx
            .send(RelayWorkerCommand::Unsubscribe {
                caller_id: caller_id.to_string(),
                handle: handle.clone(),
                response: response_tx,
            })
            .map_err(|_| {
                NostrCoreError::BackendUnavailable(
                    "nostr relay worker unavailable for unsubscribe".to_string(),
                )
            })?;
        response_rx.recv().map_err(|_| {
            NostrCoreError::BackendUnavailable(
                "nostr relay worker did not respond to unsubscribe".to_string(),
            )
        })
    }

    fn request_worker_publish(
        &self,
        relay_worker_tx: &mpsc::UnboundedSender<RelayWorkerCommand>,
        caller_id: &str,
        signed: &NostrSignedEvent,
        resolved_relays: &[String],
    ) -> Result<NostrPublishReceipt, NostrCoreError> {
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        relay_worker_tx
            .send(RelayWorkerCommand::Publish {
                caller_id: caller_id.to_string(),
                signed: signed.clone(),
                resolved_relays: resolved_relays.to_vec(),
                response: response_tx,
            })
            .map_err(|_| {
                NostrCoreError::BackendUnavailable(
                    "nostr relay worker unavailable for publish".to_string(),
                )
            })?;
        response_rx.recv().map_err(|_| {
            NostrCoreError::BackendUnavailable(
                "nostr relay worker did not respond to publish".to_string(),
            )
        })?
    }

    fn request_worker_replace_subscriptions(
        &self,
        relay_worker_tx: &mpsc::UnboundedSender<RelayWorkerCommand>,
        subscriptions: Vec<RelaySubscriptionRequest>,
    ) -> Result<usize, NostrCoreError> {
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        relay_worker_tx
            .send(RelayWorkerCommand::ReplaceSubscriptions {
                subscriptions,
                response: response_tx,
            })
            .map_err(|_| {
                NostrCoreError::BackendUnavailable(
                    "nostr relay worker unavailable for restore".to_string(),
                )
            })?;
        response_rx.recv().map_err(|_| {
            NostrCoreError::BackendUnavailable(
                "nostr relay worker did not respond to restore".to_string(),
            )
        })?
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
        state
            .caller_publish_count
            .get(caller_id)
            .copied()
            .unwrap_or(0)
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

fn canonical_event_hash(pubkey: &str, unsigned: &NostrUnsignedEvent) -> [u8; 32] {
    let canonical = serde_json::json!([
        0,
        pubkey,
        unsigned.created_at,
        unsigned.kind,
        unsigned.tags,
        unsigned.content,
    ]);
    Sha256::digest(canonical.to_string().as_bytes()).into()
}

fn nostr_filter_json(filters: &NostrFilterSet) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if !filters.kinds.is_empty() {
        map.insert("kinds".to_string(), serde_json::json!(filters.kinds));
    }
    if !filters.authors.is_empty() {
        map.insert("authors".to_string(), serde_json::json!(filters.authors));
    }
    if !filters.hashtags.is_empty() {
        map.insert("#t".to_string(), serde_json::json!(filters.hashtags));
    }
    serde_json::Value::Object(map)
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
        let allow_non_tls_local = trimmed.starts_with("ws://127.0.0.1")
            || trimmed.starts_with("ws://localhost");
        if !trimmed.starts_with("wss://") && !allow_non_tls_local {
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
    use std::time::Duration;

    use futures_util::StreamExt;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc as tokio_mpsc;
    use tokio_tungstenite::accept_async;

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
        let identity = IdentityRegistry::default();
        let unsigned = NostrUnsignedEvent {
            created_at: 1_710_000_001,
            kind: 1,
            content: "hello nostr".to_string(),
            tags: vec![("t".to_string(), "graph".to_string())],
        };

        let signed = registry.sign_event(&identity, "default", &unsigned);
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
        let identity = IdentityRegistry::default();
        registry
            .use_nip46_signer("wss://relay.example", "npub1delegate")
            .expect("nip46 config should be accepted");

        let unsigned = NostrUnsignedEvent {
            created_at: 1_710_000_002,
            kind: 1,
            content: "hello".to_string(),
            tags: Vec::new(),
        };

        let result = registry.sign_event(&identity, "default", &unsigned);
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
            created_at: 1_710_000_003,
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
            created_at: 1_710_000_004,
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
                created_at: 1_710_000_005,
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

    #[test]
    fn nostr_core_restore_persisted_subscriptions_roundtrip() {
        let registry = NostrCoreRegistry::default();
        let _first = registry
            .relay_subscribe(
                "test:alpha",
                Some("timeline"),
                NostrFilterSet {
                    kinds: vec![1],
                    authors: vec!["npub1alpha".to_string()],
                    hashtags: vec![],
                    relay_urls: vec!["wss://relay.damus.io".to_string()],
                },
            )
            .expect("first subscription should succeed");
        let _second = registry
            .relay_subscribe(
                "test:beta",
                Some("mentions"),
                NostrFilterSet {
                    kinds: vec![42],
                    authors: vec![],
                    hashtags: vec!["graphshell".to_string()],
                    relay_urls: vec![],
                },
            )
            .expect("second subscription should succeed");

        let persisted = registry.persisted_subscriptions();
        assert_eq!(persisted.len(), 2);

        let restored_registry = NostrCoreRegistry::default();
        let restored = restored_registry
            .restore_persisted_subscriptions(&persisted)
            .expect("restoring persisted subscriptions should succeed");
        assert_eq!(restored, 2);

        let mut restored_persisted = restored_registry.persisted_subscriptions();
        restored_persisted.sort_by(|left, right| left.requested_id.cmp(&right.requested_id));

        let mut expected = persisted;
        expected.sort_by(|left, right| left.requested_id.cmp(&right.requested_id));
        assert_eq!(restored_persisted, expected);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nostr_relay_worker_emits_req_event_and_close_over_websocket() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test websocket listener should bind");
        let relay_url = format!("ws://{}", listener.local_addr().unwrap());
        let (message_tx, mut message_rx) = tokio_mpsc::unbounded_channel::<serde_json::Value>();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("server should accept");
            let mut websocket = accept_async(stream)
                .await
                .expect("server should upgrade websocket");
            while let Some(frame) = websocket.next().await {
                let Ok(frame) = frame else {
                    break;
                };
                if let Message::Text(text) = frame {
                    let payload: serde_json::Value =
                        serde_json::from_str(&text).expect("frame should contain json");
                    let _ = message_tx.send(payload);
                }
            }
        });

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let cancel = tokio_util::sync::CancellationToken::new();
        let worker = tokio::spawn(NostrRelayWorker::new(command_rx, cancel.clone()).run());

        let (subscribe_tx, subscribe_rx) = std::sync::mpsc::channel();
        command_tx
            .send(RelayWorkerCommand::Subscribe {
                request: RelaySubscriptionRequest {
                    caller_id: "test:relay".to_string(),
                    subscription_id: "timeline".to_string(),
                    filters: NostrFilterSet {
                        kinds: vec![1],
                        authors: vec!["npub1example".to_string()],
                        hashtags: vec!["graphshell".to_string()],
                        relay_urls: vec![relay_url.clone()],
                    },
                    resolved_relays: vec![relay_url.clone()],
                },
                response: subscribe_tx,
            })
            .expect("subscribe command should send");
        let handle = subscribe_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("subscribe response should arrive")
            .expect("subscribe should succeed");
        assert_eq!(handle.id, "timeline");

        let (publish_tx, publish_rx) = std::sync::mpsc::channel();
        command_tx
            .send(RelayWorkerCommand::Publish {
                caller_id: "test:relay".to_string(),
                signed: NostrSignedEvent {
                    event_id: "evt-100".to_string(),
                    pubkey: "pk".to_string(),
                    signature: "sig".to_string(),
                    created_at: 1_710_000_111,
                    kind: 1,
                    content: "hello".to_string(),
                    tags: vec![("t".to_string(), "graphshell".to_string())],
                },
                resolved_relays: vec![relay_url.clone()],
                response: publish_tx,
            })
            .expect("publish command should send");
        let publish = publish_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("publish response should arrive")
            .expect("publish should succeed");
        assert!(publish.accepted);

        let (unsubscribe_tx, unsubscribe_rx) = std::sync::mpsc::channel();
        command_tx
            .send(RelayWorkerCommand::Unsubscribe {
                caller_id: "test:relay".to_string(),
                handle,
                response: unsubscribe_tx,
            })
            .expect("unsubscribe command should send");
        assert!(
            unsubscribe_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("unsubscribe response should arrive")
        );

        let first = tokio::time::timeout(Duration::from_secs(2), message_rx.recv())
            .await
            .expect("req frame should arrive")
            .expect("req frame should be present");
        let second = tokio::time::timeout(Duration::from_secs(2), message_rx.recv())
            .await
            .expect("event frame should arrive")
            .expect("event frame should be present");
        let third = tokio::time::timeout(Duration::from_secs(2), message_rx.recv())
            .await
            .expect("close frame should arrive")
            .expect("close frame should be present");

        assert_eq!(first[0], "REQ");
        assert_eq!(first[1], "timeline");
        assert_eq!(second[0], "EVENT");
        assert_eq!(second[1]["id"], "evt-100");
        assert_eq!(second[1]["created_at"], 1_710_000_111u64);
        assert_eq!(third[0], "CLOSE");
        assert_eq!(third[1], "timeline");

        cancel.cancel();
        worker.await.expect("worker should shut down cleanly");
        server.await.expect("server should shut down cleanly");
    }
}
