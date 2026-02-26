// Verse SyncWorker - Handles P2P sync protocol
//
// The SyncWorker is a ControlPanel-supervised tokio task that:
// - Accepts incoming QUIC connections from trusted peers
// - Performs bidirectional delta sync using version vectors
// - Injects remote intents into the app pipeline
// - Enforces workspace access grants

use crate::app::GraphIntent;
use crate::shell::desktop::runtime::control_panel::{IntentSource as ControlIntentSource, QueuedIntent};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::mods::native::verse::{
    VersionVector, SyncLog, SyncedIntent, TrustedPeer, WorkspaceGrant, AccessLevel,
};
use iroh::{Endpoint, NodeAddr, NodeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Commands sent to the SyncWorker
#[derive(Debug)]
#[allow(dead_code)]
pub enum SyncCommand {
    /// Initiate sync with a peer for a specific workspace
    SyncWorkspace {
        peer: NodeId,
        workspace_id: String,
    },
    /// Accept an incoming connection
    AcceptIncoming {
        conn_info: String, // Connection metadata
    },
    /// Update access grant for a peer
    UpdateGrant {
        peer: NodeId,
        grant: WorkspaceGrant,
    },
    /// Revoke all access for a peer
    RevokeAccess {
        peer: NodeId,
    },
    /// Discover nearby peers via mDNS without blocking the UI thread.
    DiscoverNearby {
        timeout_secs: u64,
    },
    /// Shutdown the worker
    Shutdown,
}

/// SyncUnit wire format for P2P exchange
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncUnit {
    pub workspace_id: String,
    /// Sender's version vector
    pub version_vector: VersionVector,
    /// Batch of intents since last sync
    pub intents: Vec<SyncedIntent>,
    /// Optional full snapshot (for new peers or large gaps)
    pub snapshot: Option<WorkspaceSnapshot>,
}

/// Full workspace snapshot (for fast-forward)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceSnapshot {
    pub workspace_id: String,
    pub nodes: Vec<SnapshotNode>,
    pub edges: Vec<SnapshotEdge>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotNode {
    pub key: u64, // NodeKey as u64
    pub url: String,
    pub position_x: f32,
    pub position_y: f32,
    pub is_pinned: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnapshotEdge {
    pub from: u64,
    pub to: u64,
    pub edge_type: String, // EdgeType as string
}

/// SyncWorker state
pub struct SyncWorker {
    /// iroh endpoint (QUIC + Noise)
    endpoint: Endpoint,
    /// Our secret key (for signing)
    our_secret_key: iroh::SecretKey,
    /// Per-workspace sync logs (in-memory, persisted to disk)
    sync_logs: Arc<RwLock<HashMap<String, SyncLog>>>,
    /// Trusted peers (read from IdentityRegistry/trust store)
    trusted_peers: Arc<RwLock<Vec<TrustedPeer>>>,
    /// Channel to send intents into the app pipeline
    intent_tx: mpsc::Sender<QueuedIntent>,
    /// Command receiver
    command_rx: mpsc::Receiver<SyncCommand>,
    /// Discovery result sender back to control-panel owner.
    discovery_result_tx: mpsc::UnboundedSender<Result<Vec<crate::mods::native::verse::DiscoveredPeer>, String>>,
    /// Cancellation token for graceful shutdown
    cancel: CancellationToken,
}

impl SyncWorker {
    /// Create a new SyncWorker
    pub fn new(
        endpoint: Endpoint,
        our_secret_key: iroh::SecretKey,
        trusted_peers: Arc<RwLock<Vec<TrustedPeer>>>,
        sync_logs: Arc<RwLock<HashMap<String, SyncLog>>>,
        intent_tx: mpsc::Sender<QueuedIntent>,
        command_rx: mpsc::Receiver<SyncCommand>,
        discovery_result_tx: mpsc::UnboundedSender<Result<Vec<crate::mods::native::verse::DiscoveredPeer>, String>>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            endpoint,
            our_secret_key,
            sync_logs,
            trusted_peers,
            intent_tx,
            command_rx,
            discovery_result_tx,
            cancel,
        }
    }

    /// Run the sync worker (accept loop + command handler)
    pub async fn run(mut self) {
        log::info!("SyncWorker started (NodeId: {})", self.endpoint.node_id());
        
        loop {
            tokio::select! {
                biased;
                
                // Check cancellation first
                _ = self.cancel.cancelled() => {
                    log::info!("SyncWorker shutting down (cancellation requested)");
                    break;
                }
                
                // Handle incoming connections
                Some(conn_result) = self.endpoint.accept() => {
                    match conn_result.accept() {
                        Ok(connecting) => {
                            let worker_handle = SyncWorkerHandle {
                                sync_logs: self.sync_logs.clone(),
                                trusted_peers: self.trusted_peers.clone(),
                                intent_tx: self.intent_tx.clone(),
                                our_secret_key: self.our_secret_key.clone(),
                                our_node_id: self.endpoint.node_id(),
                            };
                            tokio::spawn(async move {
                                if let Err(e) = worker_handle.handle_incoming(connecting).await {
                                    log::warn!("Failed to handle incoming connection: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            log::warn!("Failed to accept connection: {}", e);
                        }
                    }
                }
                
                // Handle commands
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        SyncCommand::Shutdown => {
                            log::info!("SyncWorker shutting down (shutdown command)");
                            break;
                        }
                        SyncCommand::SyncWorkspace { peer, workspace_id } => {
                            let handle = SyncWorkerHandle {
                                sync_logs: self.sync_logs.clone(),
                                trusted_peers: self.trusted_peers.clone(),
                                intent_tx: self.intent_tx.clone(),
                                our_secret_key: self.our_secret_key.clone(),
                                our_node_id: self.endpoint.node_id(),
                            };
                            let endpoint = self.endpoint.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle.initiate_sync(endpoint, peer, workspace_id).await {
                                    log::warn!("Failed to sync with peer: {}", e);
                                }
                            });
                        }
                        SyncCommand::UpdateGrant { peer, grant } => {
                            log::info!("Updated grant for peer {}: {:?}", peer, grant);
                            // Grants are managed in the trust store, just log for now
                        }
                        SyncCommand::RevokeAccess { peer } => {
                            log::info!("Revoked access for peer {}", peer);
                            // Remove peer from trust store (handled externally)
                        }
                        SyncCommand::DiscoverNearby { timeout_secs } => {
                            let discovery_result_tx = self.discovery_result_tx.clone();
                            tokio::spawn(async move {
                                let result = tokio::task::spawn_blocking(move || {
                                    crate::mods::native::verse::discover_nearby_peers(timeout_secs)
                                })
                                .await
                                .map_err(|join_err| format!("discovery worker join failed: {join_err}"))
                                .and_then(|res| res);

                                let _ = discovery_result_tx.send(result);
                            });
                        }
                        SyncCommand::AcceptIncoming { conn_info } => {
                            log::debug!("Accepting incoming: {}", conn_info);
                        }
                    }
                }
            }
        }
        
        // Graceful shutdown: close endpoint
        let _ = self.endpoint.close().await;
        log::info!("SyncWorker stopped");
    }
}

/// Cloneable handle for spawned tasks
#[derive(Clone)]
struct SyncWorkerHandle {
    sync_logs: Arc<RwLock<HashMap<String, SyncLog>>>,
    trusted_peers: Arc<RwLock<Vec<TrustedPeer>>>,
    intent_tx: mpsc::Sender<QueuedIntent>,
    our_secret_key: iroh::SecretKey,
    #[allow(dead_code)]
    our_node_id: NodeId,
}

impl SyncWorkerHandle {
    /// Handle an incoming connection from a peer
    async fn handle_incoming(
        &self,
        connecting: iroh::endpoint::Connecting,
    ) -> Result<(), String> {
        let conn = connecting.await.map_err(|e| format!("connection failed: {}", e))?;
        let peer_id = iroh::endpoint::get_remote_node_id(&conn)
            .map_err(|e| format!("no peer node ID: {}", e))?;
        
        log::info!("Incoming connection from peer: {}", peer_id);
        
        // Verify peer is trusted
        let is_trusted = {
            let peers = self.trusted_peers.read().expect("trusted peer lock poisoned");
            peers.iter().any(|p| p.node_id == peer_id)
        };
        
        if !is_trusted {
            log::warn!("Rejecting connection from untrusted peer: {}", peer_id);
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_CONNECTION_REJECTED,
                latency_us: 0,
            });
            return Err("untrusted peer".to_string());
        }
        
        // Accept bidirectional stream
        let (mut send, mut recv) = conn
            .accept_bi()
            .await
            .map_err(|e| format!("accept_bi failed: {}", e))?;
        
        // Read incoming SyncUnit
        let buf = recv.read_to_end(1024 * 1024) // 1MB max
            .await
            .map_err(|e| format!("read failed: {}", e))?;
        
        if buf.is_empty() {
            return Err("empty payload".to_string());
        }
        
        // Decompress (zstd)
        let decompressed = zstd::decode_all(&buf[..])
            .map_err(|e| format!("zstd decompress failed: {}", e))?;
        
        // Deserialize SyncUnit
        let sync_unit: SyncUnit = serde_json::from_slice(&decompressed)
            .map_err(|e| format!("deserialization failed: {}", e))?;
        
        log::info!("Received SyncUnit from {}: {} intents", peer_id, sync_unit.intents.len());
        
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_SYNC_UNIT_RECEIVED,
            latency_us: buf.len() as u64,
        });
        
        let workspace_id = sync_unit.workspace_id.clone();
        let peer_vv = sync_unit.version_vector.clone();

        // Process incoming intents
        self.process_incoming_sync_unit(peer_id, sync_unit).await?;
        
        // Send our delta back (bidirectional sync)
        let our_sync_unit = self
            .build_outgoing_sync_unit(&workspace_id, &peer_vv)
            .await?;
        let serialized = serde_json::to_vec(&our_sync_unit)
            .map_err(|e| format!("serialization failed: {}", e))?;
        let compressed = zstd::encode_all(&serialized[..], 3)
            .map_err(|e| format!("zstd compress failed: {}", e))?;
        
        send.write_all(&compressed)
            .await
            .map_err(|e| format!("write failed: {}", e))?;
        send.finish().ok();
        
        log::info!("Sent SyncUnit to {}: {} intents", peer_id, our_sync_unit.intents.len());
        
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_SYNC_UNIT_SENT,
            byte_len: compressed.len(),
        });
        
        Ok(())
    }
    
    /// Initiate sync with a peer (outbound)
    async fn initiate_sync(
        &self,
        endpoint: Endpoint,
        peer_id: NodeId,
        workspace_id: String,
    ) -> Result<(), String> {
        log::info!("Initiating sync with peer {} for workspace {}", peer_id, workspace_id);
        
        // Get peer's NodeAddr (simplified - in production we'd use mDNS or relay discovery)
        let node_addr = NodeAddr::new(peer_id);
        
        // Connect to peer
        let conn = endpoint
            .connect(node_addr, b"graphshell-sync")
            .await
            .map_err(|e| format!("connect failed: {}", e))?;
        
        // Open bidirectional stream
        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| format!("open_bi failed: {}", e))?;
        
        // Build and send our SyncUnit
        let empty_peer_vv = VersionVector::new();
        let our_sync_unit = self
            .build_outgoing_sync_unit(&workspace_id, &empty_peer_vv)
            .await?;
        let serialized = serde_json::to_vec(&our_sync_unit)
            .map_err(|e| format!("serialization failed: {}", e))?;
        let compressed = zstd::encode_all(&serialized[..], 3)
            .map_err(|e| format!("zstd compress failed: {}", e))?;
        
        send.write_all(&compressed)
            .await
            .map_err(|e| format!("write failed: {}", e))?;
        send.finish().ok();
        
        log::info!("Sent SyncUnit to {}", peer_id);
        
        // Read response
        let buf = recv.read_to_end(1024 * 1024)
            .await
            .map_err(|e| format!("read response failed: {}", e))?;
        
        if !buf.is_empty() {
            let decompressed = zstd::decode_all(&buf[..])
                .map_err(|e| format!("decompress response failed: {}", e))?;
            let sync_unit: SyncUnit = serde_json::from_slice(&decompressed)
                .map_err(|e| format!("deserialize response failed: {}", e))?;
            
            log::info!("Received response from {}: {} intents", peer_id, sync_unit.intents.len());
            self.process_incoming_sync_unit(peer_id, sync_unit).await?;
        }
        
        Ok(())
    }
    
    /// Build a SyncUnit for outbound sync
    async fn build_outgoing_sync_unit(
        &self,
        workspace_id: &str,
        peer_vv: &VersionVector,
    ) -> Result<SyncUnit, String> {
        let logs = self.sync_logs.read().expect("sync log lock poisoned");
        let sync_log = logs.get(workspace_id).ok_or_else(|| {
            format!("no sync log for workspace {}", workspace_id)
        })?;
        
        let intents: Vec<SyncedIntent> = sync_log
            .intents
            .iter()
            .filter(|intent| {
                intent.sequence > peer_vv.get(intent.authored_by)
            })
            .cloned()
            .collect();

        let sync_unit = SyncUnit {
            workspace_id: workspace_id.to_string(),
            version_vector: sync_log.version_vector.clone(),
            intents,
            snapshot: None, // TODO: generate snapshot for new peers
        };
        
        Ok(sync_unit)
    }
    
    /// Process an incoming SyncUnit and apply intents
    async fn process_incoming_sync_unit(
        &self,
        peer_id: NodeId,
        sync_unit: SyncUnit,
    ) -> Result<(), String> {
        // Check workspace grants
        let workspace_id = sync_unit.workspace_id.clone();
        let access = {
            let peers = self.trusted_peers.read().expect("trusted peer lock poisoned");
            resolve_peer_grant(&peers, peer_id, &workspace_id)
        };

        let Some(access) = access else {
            log::warn!("Peer {} has no workspace grants - rejecting sync", peer_id);
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_ACCESS_DENIED,
                latency_us: 0,
            });
            return Err("access denied".to_string());
        };

        if access == AccessLevel::ReadOnly && !sync_unit.intents.is_empty() {
            log::warn!("Peer {} is read-only for {} - rejecting mutations", peer_id, workspace_id);
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_ACCESS_DENIED,
                latency_us: 0,
            });
            return Err("read-only access".to_string());
        }
        
        // Merge version vectors
        let mut entries_to_apply = Vec::new();
        let mut should_persist = false;
        {
            let mut logs = self.sync_logs.write().expect("sync log lock poisoned");
            let sync_log = logs.entry(workspace_id.to_string())
                .or_insert_with(|| SyncLog::new(workspace_id.to_string()));
            sync_log.version_vector = sync_log.version_vector.merge(&sync_unit.version_vector);

            for intent in sync_unit.intents.iter() {
                if sync_log.should_apply(intent) {
                    entries_to_apply.push(intent.log_entry.clone());
                }
                if sync_log.record_intent(intent.clone()) {
                    should_persist = true;
                }
            }

            if should_persist {
                let _ = sync_log.save_encrypted(&self.our_secret_key);
            }
        }
        
        let intent_count = entries_to_apply.len();

        if !entries_to_apply.is_empty() {
            let entries_bytes = serde_json::to_vec(&entries_to_apply)
                .unwrap_or_default();
            
            let delta_intent = GraphIntent::ApplyRemoteLogEntries {
                entries: entries_bytes,
            };
            let queued = QueuedIntent {
                intent: delta_intent,
                queued_at: std::time::Instant::now(),
                source: ControlIntentSource::P2pSync,
            };
            let _ = self.intent_tx.try_send(queued);
        }
        
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_INTENT_APPLIED,
            latency_us: intent_count as u64,
        });
        
        Ok(())
    }
}

pub(crate) fn resolve_peer_grant(
    peers: &[TrustedPeer],
    peer_id: NodeId,
    workspace_id: &str,
) -> Option<AccessLevel> {
    peers
        .iter()
        .find(|p| p.node_id == peer_id)
        .and_then(|peer| {
            peer.workspace_grants
                .iter()
                .find(|g| g.workspace_id == workspace_id)
                .map(|g| g.access)
        })
}

/// Diagnostics channel IDs
const CHANNEL_SYNC_UNIT_SENT: &str = "verse.sync.unit_sent";
const CHANNEL_SYNC_UNIT_RECEIVED: &str = "verse.sync.unit_received";
const CHANNEL_INTENT_APPLIED: &str = "verse.sync.intent_applied";
const CHANNEL_ACCESS_DENIED: &str = "verse.sync.access_denied";
const CHANNEL_CONNECTION_REJECTED: &str = "verse.sync.connection_rejected";
