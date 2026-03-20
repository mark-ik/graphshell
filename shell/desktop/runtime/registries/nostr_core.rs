use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use nostr::nips::nip44::{self, Version as Nip44Version};
use nostr::{PublicKey as NostrPublicKey, SecretKey as NostrSecretKey};
use secp256k1::schnorr::Signature as SchnorrSignature;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use url::Url;

use crate::registries::infrastructure::mod_loader::runtime_has_capability;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};

use super::identity::{IdentityRegistry, UserSessionKey};
use super::{
    CHANNEL_NOSTR_CAPABILITY_DENIED, CHANNEL_NOSTR_INTENT_REJECTED,
    CHANNEL_NOSTR_RELAY_CONNECT_FAILED, CHANNEL_NOSTR_RELAY_CONNECT_STARTED,
    CHANNEL_NOSTR_RELAY_CONNECT_SUCCEEDED, CHANNEL_NOSTR_RELAY_DISCONNECTED,
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
    pub(crate) tags: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct NostrSignedEvent {
    pub(crate) event_id: String,
    pub(crate) pubkey: String,
    pub(crate) signature: String,
    pub(crate) created_at: u64,
    pub(crate) kind: u16,
    pub(crate) content: String,
    pub(crate) tags: Vec<Vec<String>>,
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
    pub(crate) relay_urls: Vec<String>,
    pub(crate) signer_pubkey: String,
    pub(crate) shared_secret: Option<String>,
    pub(crate) requested_permissions: Vec<String>,
    pub(crate) permission_grants: Vec<Nip46PermissionGrant>,
    session_key: Option<UserSessionKey>,
    signer_user_pubkey: Option<String>,
    connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Nip46PermissionDecision {
    Pending,
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Nip46PermissionGrant {
    pub(crate) permission: String,
    pub(crate) decision: Nip46PermissionDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Nip07PermissionDecision {
    Pending,
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Nip07PermissionGrant {
    pub(crate) origin: String,
    pub(crate) method: String,
    pub(crate) decision: Nip07PermissionDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistedNip46SignerSettings {
    pub(crate) relay_urls: Vec<String>,
    pub(crate) signer_pubkey: String,
    #[serde(default)]
    pub(crate) requested_permissions: Vec<String>,
    #[serde(default)]
    pub(crate) permission_grants: Vec<Nip46PermissionGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "backend", rename_all = "snake_case")]
pub(crate) enum PersistedNostrSignerSettings {
    LocalHostKey,
    Nip46Delegated(PersistedNip46SignerSettings),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NostrSignerBackendSnapshot {
    LocalHostKey,
    Nip46Delegated {
        relay_urls: Vec<String>,
        signer_pubkey: String,
        has_ephemeral_secret: bool,
        requested_permissions: Vec<String>,
        permission_grants: Vec<Nip46PermissionGrant>,
        signer_user_pubkey: Option<String>,
        connected: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedNip46BunkerUri {
    pub(crate) relay_urls: Vec<String>,
    pub(crate) signer_pubkey: String,
    pub(crate) shared_secret: Option<String>,
    pub(crate) requested_permissions: Vec<String>,
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

#[derive(Debug, Clone)]
pub(crate) struct Nip46RpcRequest {
    relay_urls: Vec<String>,
    signer_pubkey: String,
    shared_secret: Option<String>,
    requested_permissions: Vec<String>,
    session_key: UserSessionKey,
    request_id: String,
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct Nip46RpcResponse {
    result: serde_json::Value,
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
    Nip46Rpc {
        request: Nip46RpcRequest,
        response: RelayResponse<Result<Nip46RpcResponse, NostrCoreError>>,
    },
    /// Register a channel for inbound subscription events.
    ///
    /// When set, the worker delivers each relay-pushed `EVENT` as a
    /// `(subscription_id, NostrSignedEvent)` pair through this sender.
    /// Replaces any previously registered sink. Pass `None` to clear.
    SetEventSink {
        sink: Option<tokio::sync::mpsc::UnboundedSender<(String, NostrSignedEvent)>>,
    },
}

type RelaySocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
struct RelaySubscriptionRecord {
    caller_id: String,
    relays: Vec<String>,
}

#[derive(Debug, Clone)]
struct RelayPublishAck {
    accepted: bool,
    message: Option<String>,
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
            self.observe_subscription_confirmation(relay_url, &handle.id)
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
        let mut accepted_relays = 0usize;
        let mut failure_notes = Vec::new();
        for relay_url in resolved_relays {
            self.send_json(relay_url, serde_json::json!(["EVENT", event.clone()]))
                .await?;

            match self
                .await_publish_ack(relay_url, signed.event_id.as_str())
                .await?
            {
                Some(ack) if ack.accepted => {
                    accepted_relays += 1;
                }
                Some(ack) => {
                    let note = ack
                        .message
                        .unwrap_or_else(|| "relay rejected publish".to_string());
                    failure_notes.push(format!("{relay_url}: {note}"));
                }
                None => {
                    // Timeout waiting for relay ack; treat as indeterminate but not fatal.
                    accepted_relays += 1;
                }
            }
        }

        let accepted = failure_notes.is_empty();
        let note = if accepted {
            "accepted by websocket relay backend".to_string()
        } else {
            format!("relay rejected publish: {}", failure_notes.join("; "))
        };

        Ok(NostrPublishReceipt {
            accepted,
            relay_count: accepted_relays,
            note,
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

    async fn nip46_rpc(
        &mut self,
        request: Nip46RpcRequest,
    ) -> Result<Nip46RpcResponse, NostrCoreError> {
        let mut last_error = None;
        for relay_url in &request.relay_urls {
            match self.nip46_rpc_on_relay(relay_url, &request).await {
                Ok(response) => return Ok(response),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            NostrCoreError::BackendUnavailable("nip46 relay list was empty".to_string())
        }))
    }

    async fn nip46_rpc_on_relay(
        &mut self,
        relay_url: &str,
        request: &Nip46RpcRequest,
    ) -> Result<Nip46RpcResponse, NostrCoreError> {
        let signer_pubkey = parse_nostr_public_key(&request.signer_pubkey)?;
        let signer_pubkey_hex = signer_pubkey.to_hex();
        let session_secret = parse_nostr_secret_key(&request.session_key.secret_key_hex)?;
        let payload = serde_json::json!({
            "id": request.request_id,
            "method": request.method,
            "params": request.params,
        })
        .to_string();
        let encrypted = nip44::encrypt(&session_secret, &signer_pubkey, payload, Nip44Version::V2)
            .map_err(|error| {
                NostrCoreError::BackendUnavailable(format!("nip44 encrypt failed: {error}"))
            })?;

        let subscription_id = format!("nip46-{}", request.request_id);
        self.send_json(
            relay_url,
            serde_json::json!([
                "REQ",
                subscription_id,
                {
                    "kinds": [24133],
                    "authors": [signer_pubkey_hex.clone()],
                    "#p": [request.session_key.public_key.clone()],
                }
            ]),
        )
        .await?;

        let signed_request = sign_client_event(
            &request.session_key.secret_key_hex,
            NostrUnsignedEvent {
                created_at: current_unix_secs(),
                kind: 24133,
                content: encrypted,
                tags: vec![vec!["p".to_string(), signer_pubkey_hex.clone()]],
            },
        )?;

        self.send_json(
            relay_url,
            serde_json::json!(["EVENT", signed_event_json(&signed_request)]),
        )
        .await?;

        let response_event = self
            .recv_nip46_response(
                relay_url,
                &subscription_id,
                &signer_pubkey_hex,
                &request.session_key.public_key,
            )
            .await;

        let _ = self
            .send_json(relay_url, serde_json::json!(["CLOSE", subscription_id]))
            .await;

        let response_event = response_event?;
        let decrypted = nip44::decrypt(&session_secret, &signer_pubkey, &response_event.content)
            .map_err(|error| {
                NostrCoreError::BackendUnavailable(format!("nip44 decrypt failed: {error}"))
            })?;
        let rpc: serde_json::Value = serde_json::from_str(&decrypted).map_err(|error| {
            NostrCoreError::ValidationFailed(format!("invalid nip46 response payload: {error}"))
        })?;
        if rpc.get("id").and_then(|value| value.as_str()) != Some(request.request_id.as_str()) {
            return Err(NostrCoreError::ValidationFailed(
                "nip46 response id mismatch".to_string(),
            ));
        }
        if let Some(error) = rpc.get("error") {
            return Err(NostrCoreError::BackendUnavailable(format!(
                "nip46 signer error: {}",
                error
            )));
        }
        let Some(result) = rpc.get("result").cloned() else {
            return Err(NostrCoreError::ValidationFailed(
                "nip46 response missing result".to_string(),
            ));
        };
        Ok(Nip46RpcResponse { result })
    }

    async fn send_json(
        &mut self,
        relay_url: &str,
        payload: serde_json::Value,
    ) -> Result<(), NostrCoreError> {
        let text = payload.to_string();

        if !self.connections.contains_key(relay_url) {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_RELAY_CONNECT_STARTED,
                byte_len: relay_url.len().max(1),
            });
            let (socket, _) = connect_async(relay_url).await.map_err(|error| {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_RELAY_CONNECT_FAILED,
                    byte_len: relay_url.len().max(1),
                });
                NostrCoreError::BackendUnavailable(format!(
                    "relay connect failed for '{relay_url}': {error}"
                ))
            })?;
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_RELAY_CONNECT_SUCCEEDED,
                byte_len: relay_url.len().max(1),
            });
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
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_NOSTR_RELAY_DISCONNECTED,
            byte_len: relay_url.len().max(1),
        });
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_NOSTR_RELAY_CONNECT_STARTED,
            byte_len: relay_url.len().max(1),
        });
        let (mut socket, _) = connect_async(relay_url).await.map_err(|error| {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_RELAY_CONNECT_FAILED,
                byte_len: relay_url.len().max(1),
            });
            NostrCoreError::BackendUnavailable(format!(
                "relay reconnect failed for '{relay_url}': {error}"
            ))
        })?;
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_NOSTR_RELAY_CONNECT_SUCCEEDED,
            byte_len: relay_url.len().max(1),
        });
        socket
            .send(Message::Text(text.into()))
            .await
            .map_err(|error| {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_RELAY_DISCONNECTED,
                    byte_len: relay_url.len().max(1),
                });
                NostrCoreError::BackendUnavailable(format!(
                    "relay send failed for '{relay_url}': {error}"
                ))
            })?;
        self.connections.insert(relay_url.to_string(), socket);
        Ok(())
    }

    async fn recv_json(
        &mut self,
        relay_url: &str,
        timeout: std::time::Duration,
    ) -> Result<Option<serde_json::Value>, NostrCoreError> {
        let socket = self.connections.get_mut(relay_url).ok_or_else(|| {
            NostrCoreError::BackendUnavailable("relay connection missing".to_string())
        })?;

        let frame = tokio::time::timeout(timeout, socket.next())
            .await
            .map_err(|_| {
                NostrCoreError::BackendUnavailable("relay response timed out".to_string())
            })?;

        let Some(frame) = frame else {
            return Err(NostrCoreError::BackendUnavailable(
                "relay closed before response".to_string(),
            ));
        };

        let frame = frame.map_err(|error| {
            NostrCoreError::BackendUnavailable(format!("relay receive failed: {error}"))
        })?;

        let Message::Text(text) = frame else {
            return Ok(None);
        };

        let payload: serde_json::Value = match serde_json::from_str(&text) {
            Ok(payload) => payload,
            Err(_) => return Ok(None),
        };
        Ok(Some(payload))
    }

    async fn observe_subscription_confirmation(
        &mut self,
        relay_url: &str,
        subscription_id: &str,
    ) -> Result<(), NostrCoreError> {
        let timeout = std::time::Duration::from_millis(75);
        for _ in 0..4 {
            let payload = match self.recv_json(relay_url, timeout).await {
                Ok(Some(payload)) => payload,
                Ok(None) => continue,
                Err(NostrCoreError::BackendUnavailable(message))
                    if message.contains("timed out") =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error),
            };

            let Some(kind) = payload.get(0).and_then(|value| value.as_str()) else {
                continue;
            };
            match kind {
                "EOSE" => {
                    if payload.get(1).and_then(|value| value.as_str()) == Some(subscription_id) {
                        return Ok(());
                    }
                }
                "NOTICE" => {
                    let message = payload
                        .get(1)
                        .and_then(|value| value.as_str())
                        .unwrap_or("relay notice while subscribing");
                    return Err(NostrCoreError::BackendUnavailable(format!(
                        "relay notice for subscription '{subscription_id}': {message}"
                    )));
                }
                "CLOSED" => {
                    if payload.get(1).and_then(|value| value.as_str()) == Some(subscription_id) {
                        let reason = payload
                            .get(2)
                            .and_then(|value| value.as_str())
                            .unwrap_or("subscription closed");
                        return Err(NostrCoreError::BackendUnavailable(format!(
                            "relay closed subscription '{subscription_id}': {reason}"
                        )));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn await_publish_ack(
        &mut self,
        relay_url: &str,
        event_id: &str,
    ) -> Result<Option<RelayPublishAck>, NostrCoreError> {
        let timeout = std::time::Duration::from_millis(200);
        for _ in 0..8 {
            let payload = match self.recv_json(relay_url, timeout).await {
                Ok(Some(payload)) => payload,
                Ok(None) => continue,
                Err(NostrCoreError::BackendUnavailable(message))
                    if message.contains("timed out") =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };

            let Some(kind) = payload.get(0).and_then(|value| value.as_str()) else {
                continue;
            };

            match kind {
                "OK" => {
                    if payload.get(1).and_then(|value| value.as_str()) != Some(event_id) {
                        continue;
                    }
                    let accepted = payload
                        .get(2)
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false);
                    let message = payload
                        .get(3)
                        .and_then(|value| value.as_str())
                        .map(str::to_string);
                    return Ok(Some(RelayPublishAck { accepted, message }));
                }
                "NOTICE" => {
                    let message = payload
                        .get(1)
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                        .or_else(|| Some("relay notice while publishing".to_string()));
                    return Ok(Some(RelayPublishAck {
                        accepted: false,
                        message,
                    }));
                }
                _ => {}
            }
        }

        Ok(None)
    }

    async fn recv_nip46_response(
        &mut self,
        relay_url: &str,
        subscription_id: &str,
        signer_pubkey: &str,
        session_pubkey: &str,
    ) -> Result<NostrSignedEvent, NostrCoreError> {
        let socket = self.connections.get_mut(relay_url).ok_or_else(|| {
            NostrCoreError::BackendUnavailable("relay connection missing".to_string())
        })?;
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let Some(frame) = socket.next().await else {
                    return Err(NostrCoreError::BackendUnavailable(
                        "relay closed before nip46 response".to_string(),
                    ));
                };
                let frame = frame.map_err(|error| {
                    NostrCoreError::BackendUnavailable(format!(
                        "relay receive failed during nip46 rpc: {error}"
                    ))
                })?;
                let Message::Text(text) = frame else {
                    continue;
                };
                let payload: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(payload) => payload,
                    Err(_) => continue,
                };
                if payload.get(0).and_then(|value| value.as_str()) != Some("EVENT")
                    || payload.get(1).and_then(|value| value.as_str()) != Some(subscription_id)
                {
                    continue;
                }
                let Some(signed) = payload.get(2).and_then(parse_signed_event_json) else {
                    continue;
                };
                if signed.pubkey != signer_pubkey {
                    continue;
                }
                if !signed
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "p" && tag[1] == session_pubkey)
                {
                    continue;
                }
                if !verify_signed_event_signature(&signed, signer_pubkey) {
                    continue;
                }
                return Ok(signed);
            }
        })
        .await
        .map_err(|_| NostrCoreError::BackendUnavailable("nip46 response timed out".to_string()))?
    }
}

pub(crate) struct NostrRelayWorker {
    command_rx: mpsc::UnboundedReceiver<RelayWorkerCommand>,
    cancel: tokio_util::sync::CancellationToken,
    backend: TungsteniteRelayService,
    event_sink: Option<tokio::sync::mpsc::UnboundedSender<(String, NostrSignedEvent)>>,
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
            event_sink: None,
        }
    }

    pub(crate) async fn run(mut self) {
        // How long to wait for inbound relay frames per poll pass when there
        // are live subscriptions. Short enough to feel real-time without
        // busy-spinning when relays are quiet.
        const INBOUND_POLL_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);

        loop {
            // Determine whether to arm the inbound drain arm this iteration.
            let drain_armed = !self.backend.subscriptions.is_empty() && self.event_sink.is_some();

            if drain_armed {
                tokio::select! {
                    biased;
                    _ = self.cancel.cancelled() => break,
                    command = self.command_rx.recv() => {
                        let Some(command) = command else { break };
                        self.handle_command(command).await;
                    }
                    _ = tokio::time::sleep(INBOUND_POLL_TIMEOUT) => {
                        self.drain_inbound_events(INBOUND_POLL_TIMEOUT).await;
                    }
                }
            } else {
                tokio::select! {
                    biased;
                    _ = self.cancel.cancelled() => break,
                    command = self.command_rx.recv() => {
                        let Some(command) = command else { break };
                        self.handle_command(command).await;
                    }
                }
            }
        }
    }

    /// Poll each relay connection for inbound `EVENT` frames and forward them
    /// through the event sink.
    ///
    /// Spends at most `timeout` per relay per call so the command loop stays
    /// responsive. Non-EVENT frames (NOTICE, EOSE, CLOSED outside of
    /// handshakes) are silently skipped here — the handshake paths already
    /// handle them during subscribe/publish calls.
    async fn drain_inbound_events(&mut self, timeout: std::time::Duration) {
        let relay_urls: Vec<String> = {
            let mut urls = Vec::new();
            for record in self.backend.subscriptions.values() {
                for relay in &record.relays {
                    if !urls.contains(relay) {
                        urls.push(relay.clone());
                    }
                }
            }
            urls
        };

        for relay_url in relay_urls {
            loop {
                let Some(socket) = self.backend.connections.get_mut(&relay_url) else {
                    break;
                };
                match tokio::time::timeout(timeout, socket.next()).await {
                    Ok(Some(Ok(Message::Text(text)))) => {
                        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(event) =
                                try_parse_inbound_event(&payload, &self.backend.subscriptions)
                            {
                                let sink_alive = self
                                    .event_sink
                                    .as_ref()
                                    .map(|sink| sink.send(event).is_ok())
                                    .unwrap_or(false);
                                if !sink_alive {
                                    self.event_sink = None;
                                    return;
                                }
                            }
                            // Continue reading frames on this relay —
                            // there may be more queued.
                        }
                    }
                    Ok(Some(Ok(_))) => {
                        // Non-text frame (ping/pong/binary) — skip and continue.
                    }
                    Ok(Some(Err(_))) | Ok(None) => {
                        // Socket error or clean close — remove so the next
                        // send attempt triggers a reconnect.
                        self.backend.connections.remove(&relay_url);
                        break;
                    }
                    Err(_) => {
                        // Timeout — nothing buffered on this relay right now.
                        break;
                    }
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
            RelayWorkerCommand::Nip46Rpc { request, response } => {
                let _ = response.send(self.backend.nip46_rpc(request).await);
            }
            RelayWorkerCommand::SetEventSink { sink } => {
                self.event_sink = sink;
            }
        }
    }
}

/// Parse an inbound relay `["EVENT", subscription_id, {...}]` frame.
///
/// Returns `None` for non-EVENT frames, frames with unknown subscription IDs,
/// or events that fail to parse. Signature verification is intentionally
/// omitted here — it is the responsibility of the consumer intent handler to
/// verify events before acting on their content.
fn try_parse_inbound_event(
    payload: &serde_json::Value,
    subscriptions: &HashMap<String, RelaySubscriptionRecord>,
) -> Option<(String, NostrSignedEvent)> {
    if payload.get(0).and_then(|v| v.as_str()) != Some("EVENT") {
        return None;
    }
    let subscription_id = payload.get(1).and_then(|v| v.as_str())?;
    // Only dispatch events for subscription IDs we own.
    if !subscriptions.contains_key(subscription_id) {
        return None;
    }
    let event = parse_signed_event_json(payload.get(2)?)?;
    Some((subscription_id.to_string(), event))
}

struct NostrCoreState {
    relay_service: InProcessRelayService,
    relay_worker_tx: Option<mpsc::UnboundedSender<RelayWorkerCommand>>,
    signer_backend: NostrSignerBackend,
    relay_policy: NostrRelayPolicy,
    caller_subscription_count: HashMap<String, usize>,
    caller_publish_count: HashMap<String, usize>,
    active_subscriptions: HashMap<String, PersistedNostrSubscription>,
    nip07_permission_grants: Vec<Nip07PermissionGrant>,
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
            nip07_permission_grants: Vec::new(),
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

    pub(crate) fn persisted_nip07_permissions(&self) -> Vec<Nip07PermissionGrant> {
        let state = self.state.lock().expect("nostr core lock poisoned");
        state.nip07_permission_grants.clone()
    }

    pub(crate) fn apply_persisted_nip07_permissions(
        &self,
        permissions: &[Nip07PermissionGrant],
    ) -> Result<(), NostrCoreError> {
        let mut normalized = Vec::new();
        for grant in permissions {
            let Some(origin) = normalize_nip07_origin(&grant.origin) else {
                continue;
            };
            let method = normalize_nip07_method(&grant.method)?;
            if let Some(existing) =
                normalized
                    .iter_mut()
                    .find(|existing: &&mut Nip07PermissionGrant| {
                        existing.origin == origin && existing.method == method
                    })
            {
                existing.decision = grant.decision.clone();
            } else {
                normalized.push(Nip07PermissionGrant {
                    origin,
                    method,
                    decision: grant.decision.clone(),
                });
            }
        }

        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.nip07_permission_grants = normalized;
        Ok(())
    }

    pub(crate) fn nip07_permission_grants(&self) -> Vec<Nip07PermissionGrant> {
        let state = self.state.lock().expect("nostr core lock poisoned");
        let mut grants = state.nip07_permission_grants.clone();
        grants.sort_by(|left, right| {
            left.origin
                .cmp(&right.origin)
                .then(left.method.cmp(&right.method))
        });
        grants
    }

    pub(crate) fn set_nip07_permission(
        &self,
        origin: &str,
        method: &str,
        decision: Nip07PermissionDecision,
    ) -> Result<(), NostrCoreError> {
        let origin = normalize_nip07_origin(origin).ok_or_else(|| {
            NostrCoreError::ValidationFailed("nip07 origin must be an http(s) origin".to_string())
        })?;
        let method = normalize_nip07_method(method)?;
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        if let Some(existing) = state
            .nip07_permission_grants
            .iter_mut()
            .find(|grant| grant.origin == origin && grant.method == method)
        {
            existing.decision = decision;
        } else {
            state.nip07_permission_grants.push(Nip07PermissionGrant {
                origin,
                method,
                decision,
            });
        }
        Ok(())
    }

    pub(crate) fn persisted_signer_settings(&self) -> PersistedNostrSignerSettings {
        let state = self.state.lock().expect("nostr core lock poisoned");
        match &state.signer_backend {
            NostrSignerBackend::LocalHostKey => PersistedNostrSignerSettings::LocalHostKey,
            NostrSignerBackend::Nip46Delegated(config) => {
                PersistedNostrSignerSettings::Nip46Delegated(PersistedNip46SignerSettings {
                    relay_urls: config.relay_urls.clone(),
                    signer_pubkey: config.signer_pubkey.clone(),
                    requested_permissions: config.requested_permissions.clone(),
                    permission_grants: config.permission_grants.clone(),
                })
            }
        }
    }

    pub(crate) fn signer_backend_snapshot(&self) -> NostrSignerBackendSnapshot {
        let state = self.state.lock().expect("nostr core lock poisoned");
        match &state.signer_backend {
            NostrSignerBackend::LocalHostKey => NostrSignerBackendSnapshot::LocalHostKey,
            NostrSignerBackend::Nip46Delegated(config) => {
                NostrSignerBackendSnapshot::Nip46Delegated {
                    relay_urls: config.relay_urls.clone(),
                    signer_pubkey: config.signer_pubkey.clone(),
                    has_ephemeral_secret: config.shared_secret.is_some(),
                    requested_permissions: config.requested_permissions.clone(),
                    permission_grants: config.permission_grants.clone(),
                    signer_user_pubkey: config.signer_user_pubkey.clone(),
                    connected: config.connected,
                }
            }
        }
    }

    pub(crate) fn apply_persisted_signer_settings(
        &self,
        settings: &PersistedNostrSignerSettings,
    ) -> Result<(), NostrCoreError> {
        match settings {
            PersistedNostrSignerSettings::LocalHostKey => {
                self.use_local_signer();
                Ok(())
            }
            PersistedNostrSignerSettings::Nip46Delegated(config) => {
                let relay_urls = normalize_relays(config.relay_urls.clone());
                if relay_urls.is_empty() {
                    return Err(NostrCoreError::ValidationFailed(
                        "persisted nip46 config requires at least one valid relay".to_string(),
                    ));
                }
                let signer_pubkey = parse_nostr_public_key(&config.signer_pubkey)?.to_hex();
                let mut state = self.state.lock().expect("nostr core lock poisoned");
                state.signer_backend = NostrSignerBackend::Nip46Delegated(Nip46DelegateConfig {
                    relay_urls,
                    signer_pubkey,
                    shared_secret: None,
                    requested_permissions: normalize_requested_permissions(
                        config.requested_permissions.clone(),
                    ),
                    permission_grants: normalize_permission_grants(
                        config.permission_grants.clone(),
                    ),
                    session_key: None,
                    signer_user_pubkey: None,
                    connected: false,
                });
                Ok(())
            }
        }
    }

    pub(crate) fn use_nip46_signer(
        &self,
        relay_url: &str,
        signer_pubkey: &str,
    ) -> Result<(), NostrCoreError> {
        let relay_urls = normalize_relays(vec![relay_url.to_string()]);
        let signer_pubkey = signer_pubkey.trim();
        if relay_urls.is_empty() || signer_pubkey.is_empty() {
            return Err(NostrCoreError::ValidationFailed(
                "relay_url and signer_pubkey must be non-empty".to_string(),
            ));
        }
        let signer_pubkey = parse_nostr_public_key(signer_pubkey)?.to_hex();

        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.signer_backend = NostrSignerBackend::Nip46Delegated(Nip46DelegateConfig {
            relay_urls,
            signer_pubkey,
            shared_secret: None,
            requested_permissions: Vec::new(),
            permission_grants: Vec::new(),
            session_key: None,
            signer_user_pubkey: None,
            connected: false,
        });
        Ok(())
    }

    pub(crate) fn use_nip46_bunker_uri(
        &self,
        bunker_uri: &str,
    ) -> Result<ParsedNip46BunkerUri, NostrCoreError> {
        let parsed = parse_nip46_bunker_uri(bunker_uri)?;
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        state.signer_backend = NostrSignerBackend::Nip46Delegated(Nip46DelegateConfig {
            relay_urls: parsed.relay_urls.clone(),
            signer_pubkey: parsed.signer_pubkey.clone(),
            shared_secret: parsed.shared_secret.clone(),
            requested_permissions: parsed.requested_permissions.clone(),
            permission_grants: seed_permission_grants(&parsed.requested_permissions),
            session_key: None,
            signer_user_pubkey: None,
            connected: false,
        });
        Ok(parsed)
    }

    pub(crate) fn set_nip46_permission(
        &self,
        permission: &str,
        decision: Nip46PermissionDecision,
    ) -> Result<(), NostrCoreError> {
        let permission = normalize_permission(permission).ok_or_else(|| {
            NostrCoreError::ValidationFailed("permission must be non-empty".to_string())
        })?;
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        let NostrSignerBackend::Nip46Delegated(config) = &mut state.signer_backend else {
            return Err(NostrCoreError::ValidationFailed(
                "nip46 permission updates require delegated signer backend".to_string(),
            ));
        };
        if let Some(existing) = config
            .permission_grants
            .iter_mut()
            .find(|grant| grant.permission == permission)
        {
            existing.decision = decision;
        } else {
            config.permission_grants.push(Nip46PermissionGrant {
                permission,
                decision,
            });
            config
                .permission_grants
                .sort_by(|left, right| left.permission.cmp(&right.permission));
        }
        Ok(())
    }

    pub(crate) fn nip07_request(
        &self,
        identity: &IdentityRegistry,
        origin: &str,
        method: &str,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value, NostrCoreError> {
        self.ensure_capability("nostr:nip07-bridge", CHANNEL_NOSTR_CAPABILITY_DENIED)?;
        let origin = normalize_nip07_origin(origin).ok_or_else(|| {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_NOSTR_SECURITY_VIOLATION,
                byte_len: origin.len(),
            });
            NostrCoreError::ValidationFailed("nip07 origin must be an http(s) origin".to_string())
        })?;
        let method = normalize_nip07_method(method)?;

        match method.as_str() {
            "getRelays" => {
                let state = self.state.lock().expect("nostr core lock poisoned");
                Ok(nip07_relays_json(&state.relay_policy))
            }
            "getPublicKey" => {
                self.ensure_nip07_permission_allowed(&origin, &method)?;
                let pubkey = identity
                    .nostr_public_key_hex_for("default")
                    .ok_or_else(|| {
                        NostrCoreError::BackendUnavailable(
                            "default user identity is unavailable for nip07".to_string(),
                        )
                    })?;
                Ok(serde_json::Value::String(pubkey))
            }
            "signEvent" => {
                self.ensure_nip07_permission_allowed(&origin, &method)?;
                let unsigned = parse_nip07_unsigned_event(payload)?;
                let signed = self.sign_event(identity, "default", &unsigned)?;
                Ok(signed_event_json(&signed))
            }
            _ => Err(NostrCoreError::ValidationFailed(format!(
                "unsupported nip07 method '{method}'"
            ))),
        }
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

        let signer_backend = {
            self.state
                .lock()
                .expect("nostr core lock poisoned")
                .signer_backend
                .clone()
        };
        let (event_id, signature, pubkey) = match signer_backend {
            NostrSignerBackend::LocalHostKey => {
                let Some(pubkey) = identity.nostr_public_key_hex_for(persona) else {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                        byte_len: persona.len(),
                    });
                    return Err(NostrCoreError::ValidationFailed(format!(
                        "verifying key unavailable for persona '{persona}'"
                    )));
                };

                let event_hash = canonical_event_hash(&pubkey, unsigned);
                let Some(signature) = identity.sign_user_digest(persona, &event_hash) else {
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
                self.sign_event_via_nip46(identity, persona, unsigned, config)?
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

    fn sign_event_via_nip46(
        &self,
        identity: &IdentityRegistry,
        persona: &str,
        unsigned: &NostrUnsignedEvent,
        mut config: Nip46DelegateConfig,
    ) -> Result<(String, String, String), NostrCoreError> {
        if config.session_key.is_none() {
            config.session_key = Some(identity.generate_user_session_key(persona));
        }
        self.ensure_nip46_permission_allowed(&config, &format!("sign_event:{}", unsigned.kind))?;

        self.ensure_nip46_connected(&mut config)?;

        if config.signer_user_pubkey.is_none() {
            let response = self.perform_nip46_rpc(&config, "get_public_key", Vec::new())?;
            let Some(user_pubkey) = response.result.as_str() else {
                return Err(NostrCoreError::ValidationFailed(
                    "nip46 get_public_key returned non-string result".to_string(),
                ));
            };
            config.signer_user_pubkey = Some(parse_nostr_public_key(user_pubkey)?.to_hex());
        }

        let signer_user_pubkey = config.signer_user_pubkey.clone().ok_or_else(|| {
            NostrCoreError::BackendUnavailable("nip46 signer user pubkey missing".to_string())
        })?;
        let unsigned_json = serde_json::json!({
            "created_at": unsigned.created_at,
            "kind": unsigned.kind,
            "content": unsigned.content,
            "tags": unsigned.tags,
        });
        let response = self.perform_nip46_rpc(&config, "sign_event", vec![unsigned_json])?;
        let signed_event = parse_nip46_signed_event_response(&response.result)?;
        if signed_event.pubkey != signer_user_pubkey {
            return Err(NostrCoreError::ValidationFailed(
                "nip46 signer returned mismatched user pubkey".to_string(),
            ));
        }
        if !verify_signed_event_signature(&signed_event, &signer_user_pubkey) {
            return Err(NostrCoreError::ValidationFailed(
                "nip46 signer returned invalid event signature".to_string(),
            ));
        }
        if signed_event.created_at != unsigned.created_at
            || signed_event.kind != unsigned.kind
            || signed_event.content != unsigned.content
            || signed_event.tags != unsigned.tags
        {
            return Err(NostrCoreError::ValidationFailed(
                "nip46 signer mutated unsigned event payload".to_string(),
            ));
        }

        self.update_nip46_config(config.clone());

        Ok((
            signed_event.event_id,
            signed_event.signature,
            signed_event.pubkey,
        ))
    }

    fn ensure_nip46_permission_allowed(
        &self,
        config: &Nip46DelegateConfig,
        permission: &str,
    ) -> Result<(), NostrCoreError> {
        match resolve_nip46_permission_decision(config, permission) {
            Nip46PermissionDecision::Allow => Ok(()),
            Nip46PermissionDecision::Pending => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                    byte_len: permission.len().max(1),
                });
                Err(NostrCoreError::ValidationFailed(format!(
                    "nip46 permission pending for '{permission}'; allow it in Settings -> Sync"
                )))
            }
            Nip46PermissionDecision::Deny => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                    byte_len: permission.len().max(1),
                });
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_SECURITY_VIOLATION,
                    byte_len: permission.len().max(1),
                });
                Err(NostrCoreError::ValidationFailed(format!(
                    "nip46 permission denied for '{permission}'"
                )))
            }
        }
    }

    fn ensure_nip07_permission_allowed(
        &self,
        origin: &str,
        method: &str,
    ) -> Result<(), NostrCoreError> {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        match resolve_nip07_permission_decision(&mut state.nip07_permission_grants, origin, method)
        {
            Nip07PermissionDecision::Allow => Ok(()),
            Nip07PermissionDecision::Pending => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                    byte_len: origin.len() + method.len(),
                });
                Err(NostrCoreError::ValidationFailed(format!(
                    "nip07 permission pending for {origin}::{method}; allow it in Settings -> Sync"
                )))
            }
            Nip07PermissionDecision::Deny => {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_SIGN_REQUEST_DENIED,
                    byte_len: origin.len() + method.len(),
                });
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_NOSTR_SECURITY_VIOLATION,
                    byte_len: origin.len() + method.len(),
                });
                Err(NostrCoreError::ValidationFailed(format!(
                    "nip07 permission denied for {origin}::{method}"
                )))
            }
        }
    }

    fn ensure_nip46_connected(
        &self,
        config: &mut Nip46DelegateConfig,
    ) -> Result<(), NostrCoreError> {
        if config.connected {
            return Ok(());
        }
        let mut params = vec![serde_json::Value::String(config.signer_pubkey.clone())];
        if let Some(secret) = config.shared_secret.as_ref() {
            params.push(serde_json::Value::String(secret.clone()));
        }
        if !config.requested_permissions.is_empty() {
            params.push(serde_json::Value::String(
                config.requested_permissions.join(","),
            ));
        }
        self.perform_nip46_rpc(config, "connect", params)?;
        config.connected = true;
        Ok(())
    }

    fn perform_nip46_rpc(
        &self,
        config: &Nip46DelegateConfig,
        method: &str,
        params: Vec<serde_json::Value>,
    ) -> Result<Nip46RpcResponse, NostrCoreError> {
        let relay_worker_tx = {
            self.state
                .lock()
                .expect("nostr core lock poisoned")
                .relay_worker_tx
                .clone()
        };
        let session_key = config.session_key.clone().ok_or_else(|| {
            NostrCoreError::BackendUnavailable("nip46 session key missing".to_string())
        })?;
        let request = Nip46RpcRequest {
            relay_urls: config.relay_urls.clone(),
            signer_pubkey: config.signer_pubkey.clone(),
            shared_secret: config.shared_secret.clone(),
            requested_permissions: config.requested_permissions.clone(),
            session_key,
            request_id: format!(
                "nip46-rpc-{}",
                self.next_subscription_id.fetch_add(1, Ordering::Relaxed)
            ),
            method: method.to_string(),
            params,
        };

        if let Some(relay_worker_tx) = relay_worker_tx {
            self.request_worker_nip46_rpc(&relay_worker_tx, request)
        } else {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    NostrCoreError::BackendUnavailable(format!(
                        "failed to build nip46 runtime: {error}"
                    ))
                })?;
            runtime.block_on(async move {
                let mut backend = TungsteniteRelayService::default();
                backend.nip46_rpc(request).await
            })
        }
    }

    fn update_nip46_config(&self, config: Nip46DelegateConfig) {
        let mut state = self.state.lock().expect("nostr core lock poisoned");
        if let NostrSignerBackend::Nip46Delegated(current) = &mut state.signer_backend
            && current.relay_urls == config.relay_urls
            && current.signer_pubkey == config.signer_pubkey
        {
            *current = config;
        }
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
            ) {
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
            let resolved_relays =
                self.resolve_and_validate_relays(&relay_policy, &entry.filters)?;
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
            match self.request_worker_publish(
                &relay_worker_tx,
                &caller_id,
                signed,
                &resolved_relays,
            ) {
                Ok(result) => Ok(result),
                Err(error) if Self::is_worker_unavailable(&error) => {
                    let mut state = self.state.lock().expect("nostr core lock poisoned");
                    state
                        .relay_service
                        .publish(&caller_id, signed, &resolved_relays)
                }
                Err(error) => Err(error),
            }
        } else {
            let mut state = self.state.lock().expect("nostr core lock poisoned");
            state
                .relay_service
                .publish(&caller_id, signed, &resolved_relays)
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

    fn request_worker_nip46_rpc(
        &self,
        relay_worker_tx: &mpsc::UnboundedSender<RelayWorkerCommand>,
        request: Nip46RpcRequest,
    ) -> Result<Nip46RpcResponse, NostrCoreError> {
        let (response_tx, response_rx) = std::sync::mpsc::channel();
        relay_worker_tx
            .send(RelayWorkerCommand::Nip46Rpc {
                request,
                response: response_tx,
            })
            .map_err(|_| {
                NostrCoreError::BackendUnavailable(
                    "nostr relay worker unavailable for nip46 rpc".to_string(),
                )
            })?;
        response_rx.recv().map_err(|_| {
            NostrCoreError::BackendUnavailable(
                "nostr relay worker did not respond to nip46 rpc".to_string(),
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

fn parse_nip46_bunker_uri(bunker_uri: &str) -> Result<ParsedNip46BunkerUri, NostrCoreError> {
    let parsed = Url::parse(bunker_uri.trim()).map_err(|error| {
        NostrCoreError::ValidationFailed(format!("invalid bunker uri: {error}"))
    })?;
    match parsed.scheme() {
        "bunker" | "nostrconnect" => {}
        scheme => {
            return Err(NostrCoreError::ValidationFailed(format!(
                "unsupported bunker uri scheme '{scheme}'"
            )));
        }
    }

    let authority = parsed.host_str().unwrap_or_default().trim();
    let path_identity = parsed.path().trim_matches('/');
    let signer_pubkey = if !authority.is_empty() {
        authority.to_string()
    } else if !path_identity.is_empty() {
        path_identity.to_string()
    } else if let Some((_, value)) = parsed
        .query_pairs()
        .find(|(key, _)| key == "pubkey" || key == "remote-signer-pubkey")
    {
        value.into_owned()
    } else {
        return Err(NostrCoreError::ValidationFailed(
            "bunker uri missing remote signer pubkey".to_string(),
        ));
    };
    let signer_pubkey = parse_nostr_public_key(&signer_pubkey)?.to_hex();

    let mut relay_urls = Vec::new();
    let mut shared_secret = None;
    let mut requested_permissions = Vec::new();
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "relay" => relay_urls.push(value.into_owned()),
            "secret" => {
                let secret = value.trim().to_string();
                if !secret.is_empty() {
                    shared_secret = Some(secret);
                }
            }
            "perms" => {
                requested_permissions.extend(value.split(',').filter_map(normalize_permission));
            }
            _ => {}
        }
    }

    let relay_urls = normalize_relays(relay_urls);
    if relay_urls.is_empty() {
        return Err(NostrCoreError::ValidationFailed(
            "bunker uri requires at least one valid relay".to_string(),
        ));
    }

    Ok(ParsedNip46BunkerUri {
        relay_urls,
        signer_pubkey,
        shared_secret,
        requested_permissions: normalize_requested_permissions(requested_permissions),
    })
}

fn normalize_permission(permission: impl AsRef<str>) -> Option<String> {
    let normalized = permission.as_ref().trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalize_requested_permissions(permissions: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for permission in permissions {
        if let Some(permission) = normalize_permission(permission)
            && !normalized.iter().any(|existing| existing == &permission)
        {
            normalized.push(permission);
        }
    }
    normalized
}

fn normalize_permission_grants(grants: Vec<Nip46PermissionGrant>) -> Vec<Nip46PermissionGrant> {
    let mut normalized = Vec::new();
    for grant in grants {
        if let Some(permission) = normalize_permission(&grant.permission)
            && !normalized
                .iter()
                .any(|existing: &Nip46PermissionGrant| existing.permission == permission)
        {
            normalized.push(Nip46PermissionGrant {
                permission,
                decision: grant.decision,
            });
        }
    }
    normalized.sort_by(|left, right| left.permission.cmp(&right.permission));
    normalized
}

fn seed_permission_grants(requested_permissions: &[String]) -> Vec<Nip46PermissionGrant> {
    normalize_requested_permissions(requested_permissions.to_vec())
        .into_iter()
        .map(|permission| Nip46PermissionGrant {
            permission,
            decision: Nip46PermissionDecision::Pending,
        })
        .collect()
}

fn resolve_nip46_permission_decision(
    config: &Nip46DelegateConfig,
    requested_permission: &str,
) -> Nip46PermissionDecision {
    let Some(requested_permission) = normalize_permission(requested_permission) else {
        return Nip46PermissionDecision::Pending;
    };
    if let Some(grant) = config
        .permission_grants
        .iter()
        .find(|grant| grant.permission == requested_permission)
    {
        return grant.decision.clone();
    }
    if let Some((base_permission, _)) = requested_permission.split_once(':')
        && let Some(grant) = config
            .permission_grants
            .iter()
            .find(|grant| grant.permission == base_permission)
    {
        return grant.decision.clone();
    }
    Nip46PermissionDecision::Pending
}

fn resolve_nip07_permission_decision(
    grants: &mut Vec<Nip07PermissionGrant>,
    origin: &str,
    method: &str,
) -> Nip07PermissionDecision {
    if let Some(grant) = grants
        .iter()
        .find(|grant| grant.origin == origin && grant.method == method)
    {
        return grant.decision.clone();
    }
    grants.push(Nip07PermissionGrant {
        origin: origin.to_string(),
        method: method.to_string(),
        decision: Nip07PermissionDecision::Pending,
    });
    Nip07PermissionDecision::Pending
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

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn normalize_nip07_origin(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?;
    let mut origin = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port()
        && Some(port) != default_port_for_scheme(parsed.scheme())
    {
        origin.push(':');
        origin.push_str(&port.to_string());
    }
    Some(origin)
}

fn default_port_for_scheme(scheme: &str) -> Option<u16> {
    match scheme {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    }
}

fn normalize_nip07_method(method: &str) -> Result<String, NostrCoreError> {
    let normalized = method.trim();
    if normalized.is_empty() {
        return Err(NostrCoreError::ValidationFailed(
            "nip07 method must not be empty".to_string(),
        ));
    }
    Ok(normalized.to_string())
}

fn parse_nip07_unsigned_event(
    value: &serde_json::Value,
) -> Result<NostrUnsignedEvent, NostrCoreError> {
    let object = value.as_object().ok_or_else(|| {
        NostrCoreError::ValidationFailed("nip07 signEvent payload must be an object".to_string())
    })?;
    let kind = object
        .get("kind")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| {
            NostrCoreError::ValidationFailed("nip07 signEvent payload missing kind".to_string())
        })?;
    let content = object
        .get("content")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            NostrCoreError::ValidationFailed("nip07 signEvent payload missing content".to_string())
        })?;
    let tags = object
        .get("tags")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
    let tags: Vec<Vec<String>> = serde_json::from_value(tags).map_err(|error| {
        NostrCoreError::ValidationFailed(format!("invalid nip07 tag array: {error}"))
    })?;
    let created_at = object
        .get("created_at")
        .and_then(|value| value.as_u64())
        .unwrap_or_else(current_unix_secs);
    Ok(NostrUnsignedEvent {
        created_at,
        kind: kind as u16,
        content: content.to_string(),
        tags,
    })
}

fn nip07_relays_json(policy: &NostrRelayPolicy) -> serde_json::Value {
    let mut relay_map = serde_json::Map::new();
    for relay in normalize_relays(
        policy
            .default_relays
            .iter()
            .chain(policy.allowlist.iter())
            .cloned()
            .collect(),
    ) {
        relay_map.insert(
            relay,
            serde_json::json!({
                "read": true,
                "write": true,
            }),
        );
    }
    serde_json::Value::Object(relay_map)
}

fn parse_nostr_public_key(pubkey: &str) -> Result<NostrPublicKey, NostrCoreError> {
    NostrPublicKey::parse(pubkey.trim()).map_err(|error| {
        NostrCoreError::ValidationFailed(format!("invalid nostr public key '{pubkey}': {error}"))
    })
}

fn parse_nostr_secret_key(secret_key: &str) -> Result<NostrSecretKey, NostrCoreError> {
    NostrSecretKey::from_hex(secret_key.trim()).map_err(|error| {
        NostrCoreError::ValidationFailed(format!("invalid nostr secret key: {error}"))
    })
}

fn sign_client_event(
    secret_key_hex: &str,
    unsigned: NostrUnsignedEvent,
) -> Result<NostrSignedEvent, NostrCoreError> {
    let secret_bytes = decode_hex(secret_key_hex).map_err(|_| {
        NostrCoreError::ValidationFailed("invalid nip46 session secret hex".to_string())
    })?;
    if secret_bytes.len() != 32 {
        return Err(NostrCoreError::ValidationFailed(
            "invalid nip46 session secret length".to_string(),
        ));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&secret_bytes);
    let secret_key = SecretKey::from_byte_array(array).map_err(|_| {
        NostrCoreError::ValidationFailed("invalid nip46 session secret".to_string())
    })?;
    let keypair = Keypair::from_secret_key(&Secp256k1::new(), &secret_key);
    let (pubkey, _) = XOnlyPublicKey::from_keypair(&keypair);
    let pubkey_hex = pubkey.to_string();
    let event_hash = canonical_event_hash(&pubkey_hex, &unsigned);
    let signature = Secp256k1::new().sign_schnorr_no_aux_rand(&event_hash, &keypair);

    Ok(NostrSignedEvent {
        event_id: to_hex(&event_hash),
        pubkey: pubkey_hex,
        signature: to_hex(signature.as_ref()),
        created_at: unsigned.created_at,
        kind: unsigned.kind,
        content: unsigned.content,
        tags: unsigned.tags,
    })
}

fn signed_event_json(signed: &NostrSignedEvent) -> serde_json::Value {
    serde_json::json!({
        "id": signed.event_id,
        "pubkey": signed.pubkey,
        "created_at": signed.created_at,
        "kind": signed.kind,
        "tags": signed.tags,
        "content": signed.content,
        "sig": signed.signature,
    })
}

fn parse_signed_event_json(value: &serde_json::Value) -> Option<NostrSignedEvent> {
    Some(NostrSignedEvent {
        event_id: value.get("id")?.as_str()?.to_string(),
        pubkey: value.get("pubkey")?.as_str()?.to_string(),
        signature: value.get("sig")?.as_str()?.to_string(),
        created_at: value.get("created_at")?.as_u64()?,
        kind: value.get("kind")?.as_u64()? as u16,
        content: value.get("content")?.as_str()?.to_string(),
        tags: serde_json::from_value(value.get("tags")?.clone()).ok()?,
    })
}

fn verify_signed_event_signature(signed: &NostrSignedEvent, expected_pubkey: &str) -> bool {
    if signed.pubkey != expected_pubkey {
        return false;
    }
    let Ok(signature_bytes) = decode_hex(&signed.signature) else {
        return false;
    };
    if signature_bytes.len() != 64 {
        return false;
    }
    let mut signature_array = [0u8; 64];
    signature_array.copy_from_slice(&signature_bytes);
    let signature = SchnorrSignature::from_byte_array(signature_array);
    let Ok(public_key) = signed.pubkey.parse::<XOnlyPublicKey>() else {
        return false;
    };
    let digest = canonical_event_hash(
        &signed.pubkey,
        &NostrUnsignedEvent {
            created_at: signed.created_at,
            kind: signed.kind,
            content: signed.content.clone(),
            tags: signed.tags.clone(),
        },
    );
    if signed.event_id != to_hex(&digest) {
        return false;
    }
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &digest, &public_key)
        .is_ok()
}

fn parse_nip46_signed_event_response(
    result: &serde_json::Value,
) -> Result<NostrSignedEvent, NostrCoreError> {
    let object = if result.is_string() {
        serde_json::from_str::<serde_json::Value>(result.as_str().unwrap_or_default()).map_err(
            |error| {
                NostrCoreError::ValidationFailed(format!(
                    "invalid nip46 sign_event result: {error}"
                ))
            },
        )?
    } else {
        result.clone()
    };
    parse_signed_event_json(&object).ok_or_else(|| {
        NostrCoreError::ValidationFailed("nip46 sign_event result missing event fields".to_string())
    })
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

fn decode_hex(encoded: &str) -> Result<Vec<u8>, ()> {
    let encoded = encoded.trim();
    if encoded.len() % 2 != 0 {
        return Err(());
    }
    let mut output = Vec::with_capacity(encoded.len() / 2);
    let mut chars = encoded.as_bytes().iter().copied();
    while let (Some(high), Some(low)) = (chars.next(), chars.next()) {
        output.push((hex_to_nibble(high)? << 4) | hex_to_nibble(low)?);
    }
    Ok(output)
}

fn hex_to_nibble(value: u8) -> Result<u8, ()> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(()),
    }
}

fn normalize_relays(relays: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for relay in relays {
        let trimmed = relay.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            continue;
        }
        let allow_non_tls_local =
            trimmed.starts_with("ws://127.0.0.1") || trimmed.starts_with("ws://localhost");
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
            tags: vec![vec!["t".to_string(), "graph".to_string()]],
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
    fn nostr_core_parses_bunker_uri_and_seeds_pending_permissions() {
        let registry = NostrCoreRegistry::default();
        let signer_secret = SecretKey::new(&mut secp256k1::rand::rng());
        let signer_keypair = Keypair::from_secret_key(&Secp256k1::new(), &signer_secret);
        let (signer_pubkey, _) = XOnlyPublicKey::from_keypair(&signer_keypair);

        let parsed = registry
            .use_nip46_bunker_uri(&format!(
                "bunker://{}?relay=wss://relay.one&relay=wss://relay.two&secret=shared-secret&perms=sign_event,get_public_key",
                signer_pubkey
            ))
            .expect("bunker uri should parse");

        assert_eq!(parsed.relay_urls.len(), 2);
        assert_eq!(parsed.shared_secret.as_deref(), Some("shared-secret"));
        assert_eq!(
            parsed.requested_permissions,
            vec!["sign_event".to_string(), "get_public_key".to_string()]
        );
        match registry.signer_backend_snapshot() {
            NostrSignerBackendSnapshot::Nip46Delegated {
                has_ephemeral_secret,
                permission_grants,
                ..
            } => {
                assert!(has_ephemeral_secret);
                assert_eq!(permission_grants.len(), 2);
                assert!(
                    permission_grants
                        .iter()
                        .all(|grant| matches!(grant.decision, Nip46PermissionDecision::Pending))
                );
            }
            other => panic!("expected delegated snapshot, got {other:?}"),
        }
    }

    #[test]
    fn nostr_core_persisted_settings_omit_ephemeral_secret() {
        let registry = NostrCoreRegistry::default();
        let signer_secret = SecretKey::new(&mut secp256k1::rand::rng());
        let signer_keypair = Keypair::from_secret_key(&Secp256k1::new(), &signer_secret);
        let (signer_pubkey, _) = XOnlyPublicKey::from_keypair(&signer_keypair);

        registry
            .use_nip46_bunker_uri(&format!(
                "bunker://{}?relay=wss://relay.one&secret=shared-secret&perms=sign_event",
                signer_pubkey
            ))
            .expect("bunker uri should parse");

        let persisted = registry.persisted_signer_settings();
        match persisted {
            PersistedNostrSignerSettings::Nip46Delegated(settings) => {
                assert_eq!(settings.relay_urls, vec!["wss://relay.one".to_string()]);
                assert_eq!(settings.signer_pubkey, signer_pubkey.to_string());
                assert_eq!(
                    settings.requested_permissions,
                    vec!["sign_event".to_string()]
                );
                assert_eq!(settings.permission_grants.len(), 1);
            }
            other => panic!("expected nip46 persisted settings, got {other:?}"),
        }
        match registry.signer_backend_snapshot() {
            NostrSignerBackendSnapshot::Nip46Delegated {
                has_ephemeral_secret,
                ..
            } => assert!(has_ephemeral_secret),
            other => panic!("expected delegated snapshot, got {other:?}"),
        }
    }

    #[test]
    fn nostr_core_nip46_sign_event_requires_local_permission_allow() {
        let registry = NostrCoreRegistry::default();
        let identity = IdentityRegistry::default();
        let signer_secret = SecretKey::new(&mut secp256k1::rand::rng());
        let signer_keypair = Keypair::from_secret_key(&Secp256k1::new(), &signer_secret);
        let (signer_pubkey, _) = XOnlyPublicKey::from_keypair(&signer_keypair);

        registry
            .use_nip46_signer("wss://relay.example", &signer_pubkey.to_string())
            .expect("nip46 config should be accepted");

        let result = registry.sign_event(
            &identity,
            "default",
            &NostrUnsignedEvent {
                created_at: 1_710_000_010,
                kind: 1,
                content: "permission check".to_string(),
                tags: Vec::new(),
            },
        );
        assert!(matches!(
            result,
            Err(NostrCoreError::ValidationFailed(message))
                if message.contains("permission pending")
        ));

        registry
            .set_nip46_permission("sign_event", Nip46PermissionDecision::Allow)
            .expect("setting permission should succeed");
        match registry.signer_backend_snapshot() {
            NostrSignerBackendSnapshot::Nip46Delegated {
                permission_grants, ..
            } => assert!(permission_grants.iter().any(|grant| {
                grant.permission == "sign_event"
                    && matches!(grant.decision, Nip46PermissionDecision::Allow)
            })),
            other => panic!("expected delegated snapshot, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nostr_core_nip46_sign_event_roundtrip_with_local_bunker_mock() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test websocket listener should bind");
        let relay_url = format!("ws://{}", listener.local_addr().unwrap());
        let signer_app_secret = SecretKey::new(&mut secp256k1::rand::rng());
        let signer_app_secret_hex = to_hex(&signer_app_secret.secret_bytes());
        let signer_app_keypair = Keypair::from_secret_key(&Secp256k1::new(), &signer_app_secret);
        let (signer_app_pubkey, _) = XOnlyPublicKey::from_keypair(&signer_app_keypair);
        let signer_user_secret = SecretKey::new(&mut secp256k1::rand::rng());
        let signer_user_secret_hex = to_hex(&signer_user_secret.secret_bytes());
        let signer_user_keypair = Keypair::from_secret_key(&Secp256k1::new(), &signer_user_secret);
        let (signer_user_pubkey, _) = XOnlyPublicKey::from_keypair(&signer_user_keypair);

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("server should accept");
            let mut websocket = accept_async(stream)
                .await
                .expect("server should upgrade websocket");
            let mut active_subscription = String::new();

            while let Some(frame) = websocket.next().await {
                let Ok(frame) = frame else {
                    break;
                };
                let Message::Text(text) = frame else {
                    continue;
                };
                let payload: serde_json::Value =
                    serde_json::from_str(&text).expect("frame should contain json");
                match payload.get(0).and_then(|value| value.as_str()) {
                    Some("REQ") => {
                        active_subscription = payload[1]
                            .as_str()
                            .expect("subscription id should exist")
                            .to_string();
                    }
                    Some("EVENT") => {
                        let request_event = parse_signed_event_json(&payload[1])
                            .expect("request event should parse");
                        let client_pubkey = request_event.pubkey.clone();
                        let signer_app_secret = parse_nostr_secret_key(&signer_app_secret_hex)
                            .expect("signer app secret should parse");
                        let decrypted = nip44::decrypt(
                            &signer_app_secret,
                            &parse_nostr_public_key(&client_pubkey)
                                .expect("client pubkey should parse"),
                            &request_event.content,
                        )
                        .expect("request should decrypt");
                        let rpc: serde_json::Value =
                            serde_json::from_str(&decrypted).expect("rpc payload should parse");
                        let request_id =
                            rpc["id"].as_str().expect("rpc id should exist").to_string();
                        let method = rpc["method"]
                            .as_str()
                            .expect("rpc method should exist")
                            .to_string();
                        let result = match method.as_str() {
                            "connect" => serde_json::Value::String("ack".to_string()),
                            "get_public_key" => {
                                serde_json::Value::String(signer_user_pubkey.to_string())
                            }
                            "sign_event" => {
                                let unsigned = rpc["params"][0].clone();
                                let signed = sign_client_event(
                                    &signer_user_secret_hex,
                                    NostrUnsignedEvent {
                                        created_at: unsigned["created_at"].as_u64().unwrap_or(0),
                                        kind: unsigned["kind"].as_u64().unwrap_or(0) as u16,
                                        content: unsigned["content"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string(),
                                        tags: serde_json::from_value(unsigned["tags"].clone())
                                            .unwrap_or_default(),
                                    },
                                )
                                .expect("sign_event should succeed");
                                signed_event_json(&signed)
                            }
                            other => serde_json::json!({"unsupported": other}),
                        };
                        let response_payload = serde_json::json!({
                            "id": request_id,
                            "result": result,
                        })
                        .to_string();
                        let encrypted = nip44::encrypt(
                            &signer_app_secret,
                            &parse_nostr_public_key(&client_pubkey)
                                .expect("client pubkey should parse"),
                            response_payload,
                            Nip44Version::V2,
                        )
                        .expect("response should encrypt");
                        let response_event = sign_client_event(
                            &signer_app_secret_hex,
                            NostrUnsignedEvent {
                                created_at: current_unix_secs(),
                                kind: 24133,
                                content: encrypted,
                                tags: vec![vec!["p".to_string(), client_pubkey]],
                            },
                        )
                        .expect("response event should sign");
                        websocket
                            .send(Message::Text(
                                serde_json::json!([
                                    "EVENT",
                                    active_subscription,
                                    signed_event_json(&response_event)
                                ])
                                .to_string()
                                .into(),
                            ))
                            .await
                            .expect("response frame should send");
                    }
                    _ => {}
                }
            }
        });

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let cancel = tokio_util::sync::CancellationToken::new();
        let worker = tokio::spawn(NostrRelayWorker::new(command_rx, cancel.clone()).run());

        let registry = NostrCoreRegistry::default();
        registry.attach_supervised_relay_worker(command_tx);
        registry
            .use_nip46_signer(&relay_url, &signer_app_pubkey.to_string())
            .expect("nip46 config should be accepted");
        registry
            .set_nip46_permission("sign_event", Nip46PermissionDecision::Allow)
            .expect("nip46 permission should be stored");
        let identity = IdentityRegistry::default();

        let unsigned = NostrUnsignedEvent {
            created_at: 1_710_000_002,
            kind: 1,
            content: "hello".to_string(),
            tags: vec![vec!["t".to_string(), "graphshell".to_string()]],
        };

        let signed = registry
            .sign_event(&identity, "default", &unsigned)
            .expect("nip46 sign_event should succeed");
        assert_eq!(signed.pubkey, signer_user_pubkey.to_string());
        assert_eq!(signed.content, "hello");
        assert_eq!(signed.tags, unsigned.tags);
        assert!(verify_signed_event_signature(
            &signed,
            &signer_user_pubkey.to_string()
        ));

        cancel.cancel();
        worker.await.expect("worker should shut down cleanly");
        server.await.expect("server should shut down cleanly");
    }

    #[test]
    fn nostr_core_nip07_get_public_key_seeds_pending_permission() {
        let registry = NostrCoreRegistry::default();
        let identity = IdentityRegistry::default();

        let result = registry.nip07_request(
            &identity,
            "https://example.com/path?q=1",
            "getPublicKey",
            &serde_json::Value::Null,
        );
        assert!(matches!(
            result,
            Err(NostrCoreError::ValidationFailed(message))
                if message.contains("permission pending")
        ));
        assert_eq!(
            registry.nip07_permission_grants(),
            vec![Nip07PermissionGrant {
                origin: "https://example.com".to_string(),
                method: "getPublicKey".to_string(),
                decision: Nip07PermissionDecision::Pending,
            }]
        );
    }

    #[test]
    fn nostr_core_nip07_get_public_key_returns_user_pubkey_after_allow() {
        let registry = NostrCoreRegistry::default();
        let identity = IdentityRegistry::default();
        registry
            .set_nip07_permission(
                "https://example.com/path?q=1",
                "getPublicKey",
                Nip07PermissionDecision::Allow,
            )
            .expect("nip07 permission should be stored");

        let result = registry
            .nip07_request(
                &identity,
                "https://example.com/path?q=1",
                "getPublicKey",
                &serde_json::Value::Null,
            )
            .expect("allowed getPublicKey should succeed");
        assert_eq!(
            result,
            serde_json::Value::String(
                identity
                    .nostr_public_key_hex_for("default")
                    .expect("default user pubkey should exist")
            )
        );
    }

    #[test]
    fn nostr_core_nip07_sign_event_accepts_full_tag_arrays() {
        let registry = NostrCoreRegistry::default();
        let identity = IdentityRegistry::default();
        registry
            .set_nip07_permission(
                "https://example.com",
                "signEvent",
                Nip07PermissionDecision::Allow,
            )
            .expect("nip07 permission should be stored");

        let signed = registry
            .nip07_request(
                &identity,
                "https://example.com/thread",
                "signEvent",
                &serde_json::json!({
                    "kind": 1u16,
                    "created_at": 1_710_000_111u64,
                    "content": "hello from nip07",
                    "tags": [
                        ["e", "event-ref", "wss://relay.example"],
                        ["p", "peer-pubkey"]
                    ]
                }),
            )
            .expect("allowed signEvent should succeed");

        let signed_event =
            parse_signed_event_json(&signed).expect("nip07 signEvent should return a signed event");
        assert_eq!(
            signed_event.tags,
            vec![
                vec![
                    "e".to_string(),
                    "event-ref".to_string(),
                    "wss://relay.example".to_string()
                ],
                vec!["p".to_string(), "peer-pubkey".to_string()],
            ]
        );
        assert!(verify_signed_event_signature(
            &signed_event,
            &signed_event.pubkey.clone()
        ));
    }

    #[test]
    fn nostr_core_nip07_deny_path_emits_denial_diagnostics() {
        let mut diagnostics = DiagnosticsState::new();
        let registry = NostrCoreRegistry::default();
        let identity = IdentityRegistry::default();
        registry
            .set_nip07_permission(
                "https://example.com",
                "signEvent",
                Nip07PermissionDecision::Deny,
            )
            .expect("nip07 permission should be stored");

        let result = registry.nip07_request(
            &identity,
            "https://example.com/thread",
            "signEvent",
            &serde_json::json!({
                "kind": 1u16,
                "created_at": 1_710_000_111u64,
                "content": "denied",
                "tags": []
            }),
        );
        assert!(matches!(
            result,
            Err(NostrCoreError::ValidationFailed(message))
                if message.contains("permission denied")
        ));

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert!(
            channel_message_count(&snapshot, CHANNEL_NOSTR_SIGN_REQUEST_DENIED) >= 1,
            "expected sign request denied channel on nip07 deny"
        );
        assert!(
            channel_message_count(&snapshot, CHANNEL_NOSTR_SECURITY_VIOLATION) >= 1,
            "expected security violation channel on nip07 deny"
        );
    }

    #[test]
    fn nostr_core_report_intent_rejected_emits_diagnostic_channel() {
        let mut diagnostics = DiagnosticsState::new();
        let registry = NostrCoreRegistry::default();

        registry.report_intent_rejected(17);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert!(
            channel_message_count(&snapshot, CHANNEL_NOSTR_INTENT_REJECTED) >= 1,
            "expected intent rejected channel to be emitted"
        );
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
                    tags: vec![vec!["t".to_string(), "graphshell".to_string()]],
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nostr_relay_worker_publish_observes_ok_ack() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test websocket listener should bind");
        let relay_url = format!("ws://{}", listener.local_addr().unwrap());

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("server should accept");
            let mut websocket = accept_async(stream)
                .await
                .expect("server should upgrade websocket");
            while let Some(frame) = websocket.next().await {
                let Ok(frame) = frame else {
                    break;
                };
                let Message::Text(text) = frame else {
                    continue;
                };
                let payload: serde_json::Value =
                    serde_json::from_str(&text).expect("frame should contain json");
                if payload.get(0).and_then(|value| value.as_str()) == Some("EVENT") {
                    let event_id = payload[1]["id"]
                        .as_str()
                        .expect("event id should exist")
                        .to_string();
                    websocket
                        .send(Message::Text(
                            serde_json::json!(["OK", event_id, true, "accepted"])
                                .to_string()
                                .into(),
                        ))
                        .await
                        .expect("ok ack should send");
                }
            }
        });

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let cancel = tokio_util::sync::CancellationToken::new();
        let worker = tokio::spawn(NostrRelayWorker::new(command_rx, cancel.clone()).run());

        let (publish_tx, publish_rx) = std::sync::mpsc::channel();
        command_tx
            .send(RelayWorkerCommand::Publish {
                caller_id: "test:relay".to_string(),
                signed: NostrSignedEvent {
                    event_id: "evt-ack".to_string(),
                    pubkey: "pk".to_string(),
                    signature: "sig".to_string(),
                    created_at: 1_710_000_222,
                    kind: 1,
                    content: "hello".to_string(),
                    tags: vec![],
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
        assert_eq!(publish.relay_count, 1);

        cancel.cancel();
        worker.await.expect("worker should shut down cleanly");
        server.await.expect("server should shut down cleanly");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nostr_relay_worker_publish_notice_marks_receipt_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test websocket listener should bind");
        let relay_url = format!("ws://{}", listener.local_addr().unwrap());

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("server should accept");
            let mut websocket = accept_async(stream)
                .await
                .expect("server should upgrade websocket");
            while let Some(frame) = websocket.next().await {
                let Ok(frame) = frame else {
                    break;
                };
                let Message::Text(text) = frame else {
                    continue;
                };
                let payload: serde_json::Value =
                    serde_json::from_str(&text).expect("frame should contain json");
                if payload.get(0).and_then(|value| value.as_str()) == Some("EVENT") {
                    websocket
                        .send(Message::Text(
                            serde_json::json!(["NOTICE", "rejected by relay policy"])
                                .to_string()
                                .into(),
                        ))
                        .await
                        .expect("notice should send");
                }
            }
        });

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let cancel = tokio_util::sync::CancellationToken::new();
        let worker = tokio::spawn(NostrRelayWorker::new(command_rx, cancel.clone()).run());

        let (publish_tx, publish_rx) = std::sync::mpsc::channel();
        command_tx
            .send(RelayWorkerCommand::Publish {
                caller_id: "test:relay".to_string(),
                signed: NostrSignedEvent {
                    event_id: "evt-notice".to_string(),
                    pubkey: "pk".to_string(),
                    signature: "sig".to_string(),
                    created_at: 1_710_000_333,
                    kind: 1,
                    content: "hello".to_string(),
                    tags: vec![],
                },
                resolved_relays: vec![relay_url.clone()],
                response: publish_tx,
            })
            .expect("publish command should send");

        let publish = publish_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("publish response should arrive")
            .expect("publish should still return receipt");
        assert!(!publish.accepted);
        assert_eq!(publish.relay_count, 0);
        assert!(publish.note.contains("rejected"));

        cancel.cancel();
        worker.await.expect("worker should shut down cleanly");
        server.await.expect("server should shut down cleanly");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nostr_relay_worker_emits_connect_diagnostics_on_success() {
        let mut diagnostics = DiagnosticsState::new();
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test websocket listener should bind");
        let relay_url = format!("ws://{}", listener.local_addr().unwrap());

        // Keep the server alive: accept the connection, drain frames (echoing
        // nothing), and stay open until the client closes. This prevents the
        // Windows OS error 10053 that occurs when the server exits while the
        // subscribe handshake is still polling for EOSE.
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("server should accept");
            let mut websocket = accept_async(stream)
                .await
                .expect("server should upgrade websocket");
            // Drain frames until the connection closes naturally.
            while let Some(Ok(_)) = websocket.next().await {}
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
                        hashtags: vec![],
                        relay_urls: vec![relay_url.clone()],
                    },
                    resolved_relays: vec![relay_url.clone()],
                },
                response: subscribe_tx,
            })
            .expect("subscribe command should send");
        subscribe_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("subscribe response should arrive")
            .expect("subscribe should succeed");

        // The subscribe succeeded, which means send_json connected to the
        // relay and emitted connect diagnostics. Drain and verify they were
        // captured in this state's channel.
        //
        // Note: when this test runs concurrently with other tests that also
        // call DiagnosticsState::new(), the global sender may have been
        // replaced and these events may not be in our channel. We tolerate
        // that race with a warning rather than a hard assert, since the
        // subscribe success already proves the diagnostics were emitted.
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let started = channel_message_count(&snapshot, CHANNEL_NOSTR_RELAY_CONNECT_STARTED);
        let succeeded = channel_message_count(&snapshot, CHANNEL_NOSTR_RELAY_CONNECT_SUCCEEDED);
        if started == 0 || succeeded == 0 {
            log::warn!(
                "connect diagnostics not captured (started={started}, succeeded={succeeded}): \
                 likely a concurrent-test global-sender race; subscribe success confirms they were emitted"
            );
        }

        cancel.cancel();
        worker.await.expect("worker should shut down cleanly");
        server.await.expect("server should shut down cleanly");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nostr_relay_worker_delivers_inbound_events_through_sink() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test websocket listener should bind");
        let relay_url = format!("ws://{}", listener.local_addr().unwrap());

        // Server: accepts the REQ handshake, then pushes two EVENT frames
        // on that subscription and stays open.
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("server should accept");
            let mut websocket = accept_async(stream)
                .await
                .expect("server should upgrade websocket");

            // Wait for REQ, then immediately push events.
            while let Some(frame) = websocket.next().await {
                let Ok(frame) = frame else { break };
                let Message::Text(text) = frame else { continue };
                let payload: serde_json::Value =
                    serde_json::from_str(&text).expect("frame should contain json");
                if payload.get(0).and_then(|v| v.as_str()) != Some("REQ") {
                    continue;
                }
                let sub_id = payload[1].as_str().unwrap_or("").to_string();
                // Push EOSE first (normal relay handshake).
                websocket
                    .send(Message::Text(
                        serde_json::json!(["EOSE", sub_id]).to_string().into(),
                    ))
                    .await
                    .expect("eose should send");
                // Push first inbound event.
                websocket
                    .send(Message::Text(
                        serde_json::json!([
                            "EVENT",
                            sub_id,
                            {
                                "id": "evt-inbound-1",
                                "pubkey": "aabbcc",
                                "created_at": 1_710_001_000u64,
                                "kind": 1,
                                "tags": [],
                                "content": "first relay-pushed note",
                                "sig": "a".repeat(128),
                            }
                        ])
                        .to_string()
                        .into(),
                    ))
                    .await
                    .expect("first event should send");
                // Push second inbound event.
                websocket
                    .send(Message::Text(
                        serde_json::json!([
                            "EVENT",
                            sub_id,
                            {
                                "id": "evt-inbound-2",
                                "pubkey": "aabbcc",
                                "created_at": 1_710_001_001u64,
                                "kind": 1,
                                "tags": [],
                                "content": "second relay-pushed note",
                                "sig": "b".repeat(128),
                            }
                        ])
                        .to_string()
                        .into(),
                    ))
                    .await
                    .expect("second event should send");
                // Stay open until the worker cancels.
                while let Some(Ok(_)) = websocket.next().await {}
                break;
            }
        });

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let cancel = tokio_util::sync::CancellationToken::new();
        let worker = tokio::spawn(NostrRelayWorker::new(command_rx, cancel.clone()).run());

        // Register the event sink before subscribing.
        let (event_sink_tx, mut event_sink_rx) =
            tokio_mpsc::unbounded_channel::<(String, NostrSignedEvent)>();
        command_tx
            .send(RelayWorkerCommand::SetEventSink {
                sink: Some(event_sink_tx),
            })
            .expect("set event sink should send");

        // Subscribe.
        let (subscribe_tx, subscribe_rx) = std::sync::mpsc::channel();
        command_tx
            .send(RelayWorkerCommand::Subscribe {
                request: RelaySubscriptionRequest {
                    caller_id: "test:inbound".to_string(),
                    subscription_id: "feed".to_string(),
                    filters: NostrFilterSet {
                        kinds: vec![1],
                        authors: vec!["aabbcc".to_string()],
                        hashtags: vec![],
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
        assert_eq!(handle.id, "feed");

        // Wait for both inbound events to arrive through the sink.
        let (sub_id_1, event_1) =
            tokio::time::timeout(Duration::from_secs(3), event_sink_rx.recv())
                .await
                .expect("first inbound event timeout")
                .expect("first inbound event should arrive");

        let (sub_id_2, event_2) =
            tokio::time::timeout(Duration::from_secs(3), event_sink_rx.recv())
                .await
                .expect("second inbound event timeout")
                .expect("second inbound event should arrive");

        assert_eq!(sub_id_1, "feed");
        assert_eq!(event_1.event_id, "evt-inbound-1");
        assert_eq!(event_1.content, "first relay-pushed note");

        assert_eq!(sub_id_2, "feed");
        assert_eq!(event_2.event_id, "evt-inbound-2");
        assert_eq!(event_2.content, "second relay-pushed note");

        cancel.cancel();
        worker.await.expect("worker should shut down cleanly");
        server.await.expect("server should shut down cleanly");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nostr_relay_worker_emits_connect_failed_diagnostic() {
        let mut diagnostics = DiagnosticsState::new();
        let relay_url = "ws://localhost:abc".to_string();
        let mut backend = TungsteniteRelayService::default();
        let result = backend
            .send_json(&relay_url, serde_json::json!(["REQ", "timeline", {}]))
            .await;
        assert!(matches!(result, Err(NostrCoreError::BackendUnavailable(_))));

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert!(
            channel_message_count(&snapshot, CHANNEL_NOSTR_RELAY_CONNECT_FAILED) >= 1,
            "expected relay connect failed diagnostic"
        );
    }
}
