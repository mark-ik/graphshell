/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Control Panel — async adapter layer for multi-producer intent queuing.
//!
//! Bridges concurrent background workers (memory monitor, mod loader, prefetch
//! scheduler, P2P sync) to the synchronous two-phase reducer without
//! compromising determinism or testability.
//!
//! The reducer stays 100% synchronous; workers communicate exclusively through
//! the [`QueuedIntent`] channel. Each frame, the caller drains the channel via
//! [`ControlPanel::drain_pending`] before calling `apply_intents`.
//!
//! Part of The Register: `RegistryRuntime` + `ControlPanel` + future signal
//! routing. `SignalBus` remains an architecture term only in this phase; no
//! dedicated runtime bus type is implemented here yet.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sysinfo::System;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::app::{GraphIntent, LifecycleCause, MemoryPressureLevel};
use crate::graph::NodeKey;
use crate::mods::native::verse::{self, SyncCommand, SyncWorker};
use crate::registries::infrastructure::mod_loader::{discover_native_mods, resolve_mod_load_order};
use crate::shell::desktop::runtime::protocol_probe::ContentTypeProber;
use crate::shell::desktop::runtime::registries::agent::{Agent, AgentContext};
use crate::shell::desktop::runtime::registries::nostr_core::{
    NostrRelayWorker, RelayWorkerCommand,
};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_SYSTEM_TASK_BUDGET_WORKER_RESUMED, CHANNEL_SYSTEM_TASK_BUDGET_WORKER_SUSPENDED,
    RegistryRuntime,
};

/// Capacity of the intent channel — limits flooding from async producers.
const INTENT_CHANNEL_CAPACITY: usize = 256;

/// How often the memory monitor samples system memory.
const MEMORY_MONITOR_INTERVAL: Duration = Duration::from_secs(5);
const PREFETCH_MIN_INTERVAL: Duration = Duration::from_secs(2);
const PREFETCH_MAX_INTERVAL: Duration = Duration::from_secs(30);

/// Default idle threshold (seconds) when no preference is configured.
const DEFAULT_WORKER_IDLE_THRESHOLD_SECS: u64 = 120;

/// Worker tier classification per the Runtime Task Budget policy.
///
/// Tier 0 workers (Servo render pipeline, intent drain) are not managed by
/// ControlPanel. Tier 1 workers are session-scoped and may be suspended when
/// the user is idle. Tier 2 workers are on-demand and managed by their owning
/// mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkerTier {
    /// P2P sync worker (iroh). Session-scoped; suspends on idle.
    Tier1P2pSync,
    /// Nostr relay pool. Session-scoped; suspends on idle.
    Tier1NostrRelay,
    /// Matrix client worker. Session-scoped; stub/no-op until MatrixCore lands.
    Tier1MatrixCore,
}

/// CP3 policy channel payload for prefetch scheduling behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LifecyclePolicy {
    pub(crate) prefetch_enabled: bool,
    pub(crate) prefetch_interval: Duration,
    pub(crate) prefetch_target: Option<NodeKey>,
    pub(crate) memory_pressure_level: MemoryPressureLevel,
}

impl Default for LifecyclePolicy {
    fn default() -> Self {
        Self {
            prefetch_enabled: false,
            prefetch_interval: Duration::from_secs(10),
            prefetch_target: None,
            memory_pressure_level: MemoryPressureLevel::Unknown,
        }
    }
}

/// Intent with source tracking for the async intent queue.
///
/// Intents from async producers are drained into the synchronous frame loop
/// each tick and sorted by causality before `apply_intents` runs.
#[derive(Debug, Clone)]
pub(crate) struct QueuedIntent {
    pub(crate) intent: GraphIntent,
    #[allow(dead_code)]
    pub(crate) queued_at: Instant,
    #[allow(dead_code)]
    pub(crate) source: IntentSource,
}

/// Source of a queued intent — used for causality ordering and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum IntentSource {
    /// User keyboard/mouse input (applied first, implicit clock 0).
    #[allow(dead_code)]
    LocalUI,
    /// Servo browser delegate (navigation, load completion, etc.).
    #[allow(dead_code)]
    ServoDelegate,
    /// Memory/system pressure monitor.
    MemoryMonitor,
    /// Mod loader lifecycle events.
    ModLoader,
    /// Background prefetch scheduler (Phase CP3).
    #[allow(dead_code)]
    PrefetchScheduler,
    /// P2P sync worker (Phase CP4).
    P2pSync,
    /// Background content-type probe worker (Sector A).
    ProtocolProbe,
    /// Register-supervised application agent.
    Agent,
    /// Restore/replay from persistence.
    #[allow(dead_code)]
    Restore,
    /// Inbound event from a Nostr relay subscription.
    NostrRelay,
}

fn intent_source_priority(source: IntentSource) -> u8 {
    match source {
        IntentSource::LocalUI => 0,
        IntentSource::ServoDelegate => 1,
        IntentSource::MemoryMonitor => 2,
        IntentSource::ProtocolProbe => 3,
        IntentSource::Agent => 4,
        IntentSource::ModLoader => 5,
        IntentSource::PrefetchScheduler => 6,
        IntentSource::P2pSync => 7,
        IntentSource::NostrRelay => 7,
        IntentSource::Restore => 8,
    }
}

/// The Control Panel: async adapter layer that bridges concurrent background
/// producers to the synchronous two-phase reducer.
///
/// Owns:
/// - an intent `mpsc` channel (capacity [`INTENT_CHANNEL_CAPACITY`])
/// - a shared [`CancellationToken`] for graceful worker shutdown
/// - a [`JoinSet`] supervising all background tasks
pub(crate) struct ControlPanel {
    /// Cloned to each background worker for intent submission.
    intent_tx: mpsc::Sender<QueuedIntent>,
    /// Drained by the sync frame loop each tick via [`Self::drain_pending`].
    intent_rx: mpsc::Receiver<QueuedIntent>,
    /// Optional sync worker command channel.
    sync_command_tx: Option<mpsc::Sender<SyncCommand>>,
    /// Sync worker discovery-result stream.
    discovery_result_rx:
        Option<mpsc::UnboundedReceiver<Result<Vec<verse::DiscoveredPeer>, String>>>,
    /// Shared cancellation token — `cancel()` stops all supervised workers.
    cancel: CancellationToken,
    /// CP3 lifecycle policy watch sender consumed by scheduler workers.
    lifecycle_policy_tx: watch::Sender<LifecyclePolicy>,
    /// Active content-type probe tokens keyed by node for cancellation/replacement.
    active_protocol_probes: Arc<Mutex<HashMap<NodeKey, (u64, CancellationToken)>>>,
    /// Monotonic nonce for protocol probe replacement.
    next_protocol_probe_nonce: u64,
    /// Per-Tier-1-worker suspension watch senders.
    ///
    /// `true` = suspended (low-frequency mode); `false` = active.
    /// Workers hold the corresponding `watch::Receiver<bool>` and decide
    /// internally what "low-frequency" means.
    p2p_sync_suspended_tx: watch::Sender<bool>,
    nostr_relay_suspended_tx: watch::Sender<bool>,
    /// Monotonic timestamp (ms since UNIX epoch) of the last user gesture.
    ///
    /// Shared with the idle-watcher task via an `Arc<AtomicU64>`.
    last_user_gesture_ms: Arc<AtomicU64>,
    /// Whether we are currently in the idle state (avoids duplicate signals).
    currently_idle: bool,
    /// Idle threshold in milliseconds, sourced from `AppPreferences` at
    /// construction time (`worker_idle_threshold_secs`).
    worker_idle_threshold_ms: u64,
    /// Live worker count per tier — incremented at spawn, never decremented
    /// (workers are supervised through JoinSet; this is a spawn-site record,
    /// not a live-task count). Used by the §4 concurrency budget query surface.
    registered_tiers: HashMap<WorkerTier, usize>,
    /// Supervised background worker tasks.
    workers: JoinSet<()>,
}

impl ControlPanel {
    /// Create a new `ControlPanel` with an empty worker set and a fresh
    /// cancellation token.
    ///
    /// `worker_idle_threshold_secs` is sourced from `AppPreferences`; pass
    /// `None` to use the built-in default of 120 s.
    pub(crate) fn new(worker_idle_threshold_secs: Option<u64>) -> Self {
        let (intent_tx, intent_rx) = mpsc::channel(INTENT_CHANNEL_CAPACITY);
        let (lifecycle_policy_tx, _lifecycle_policy_rx) =
            watch::channel(LifecyclePolicy::default());
        let (p2p_sync_suspended_tx, _) = watch::channel(false);
        let (nostr_relay_suspended_tx, _) = watch::channel(false);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let worker_idle_threshold_ms =
            worker_idle_threshold_secs.unwrap_or(DEFAULT_WORKER_IDLE_THRESHOLD_SECS) * 1000;
        Self {
            intent_tx,
            intent_rx,
            cancel: CancellationToken::new(),
            lifecycle_policy_tx,
            active_protocol_probes: Arc::new(Mutex::new(HashMap::new())),
            next_protocol_probe_nonce: 0,
            p2p_sync_suspended_tx,
            nostr_relay_suspended_tx,
            last_user_gesture_ms: Arc::new(AtomicU64::new(now_ms)),
            currently_idle: false,
            worker_idle_threshold_ms,
            registered_tiers: HashMap::new(),
            workers: JoinSet::new(),
            sync_command_tx: None,
            discovery_result_rx: None,
        }
    }

    /// Drain all pending intents from async producers (non-blocking).
    ///
    /// Call once per frame before `apply_intents`. Returns all intents
    /// currently buffered in the channel; returns an empty `Vec` if none.
    pub(crate) fn drain_pending(&mut self) -> Vec<GraphIntent> {
        let mut queued = Vec::new();
        while let Ok(item) = self.intent_rx.try_recv() {
            queued.push(item);
        }
        queued.sort_by_key(|item| (intent_source_priority(item.source), item.queued_at));
        queued.into_iter().map(|item| item.intent).collect()
    }

    /// Clone the sync command sender, when the sync worker is available.
    pub(crate) fn sync_command_sender(&self) -> Option<mpsc::Sender<SyncCommand>> {
        self.sync_command_tx.clone()
    }

    pub(crate) fn take_discovery_results(
        &mut self,
    ) -> Option<Result<Vec<verse::DiscoveredPeer>, String>> {
        self.discovery_result_rx
            .as_mut()
            .and_then(|rx| rx.try_recv().ok())
    }

    /// Enqueue a nearby-peer discovery command for the sync worker.
    pub(crate) fn request_discover_nearby_peers(&self, timeout_secs: u64) -> Result<(), String> {
        let Some(tx) = self.sync_command_tx.clone() else {
            return Err("sync worker command channel unavailable".to_string());
        };
        tx.try_send(SyncCommand::DiscoverNearby { timeout_secs })
            .map_err(|e| format!("failed to enqueue discovery command: {e}"))
    }

    /// Spawn the memory monitor background worker.
    ///
    /// The worker samples system memory every [`MEMORY_MONITOR_INTERVAL`] and
    /// emits `RuntimeEvent::SetMemoryPressureStatus` when the observed level
    /// changes. Respects the shared cancellation token for graceful shutdown.
    ///
    /// Phase CP1: samples and emits level changes. Phase CP3 will extend this
    /// to also subscribe to a `LifecyclePolicy` watch channel so the worker can
    /// emit targeted demotion intents at configurable thresholds.
    pub(crate) fn spawn_memory_monitor(&mut self) {
        let cancel = self.cancel.clone();
        let tx = self.intent_tx.clone();
        self.workers.spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {
                    log::debug!("control_panel: memory monitor cancelled");
                }
                _ = memory_monitor_worker(tx) => {}
            }
        });
        log::debug!("control_panel: memory monitor spawned");
    }

    /// Spawn the mod loader background worker.
    ///
    /// Phase CP2: discovers registered native mods, resolves dependency order,
    /// and emits lifecycle intents for activation/failure events.
    pub(crate) fn spawn_mod_loader(&mut self) {
        let cancel = self.cancel.clone();
        let tx = self.intent_tx.clone();
        self.workers.spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {
                    log::debug!("control_panel: mod loader cancelled");
                }
                _ = mod_loader_worker(tx) => {}
            }
        });
        log::debug!("control_panel: mod loader spawned");
    }

    /// Update prefetch lifecycle policy for CP3 scheduler workers.
    pub(crate) fn update_lifecycle_policy(&self, policy: LifecyclePolicy) {
        if self.lifecycle_policy_tx.send(policy).is_err() {
            log::debug!("control_panel: lifecycle policy update skipped (no observers)");
        }
    }

    /// Record a user gesture timestamp (call once per frame when the user
    /// produces any input). Used by `tick_idle_watchdog` to decide whether
    /// Tier 1 workers should be suspended.
    pub(crate) fn notify_user_gesture(&self) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_user_gesture_ms
            .store(now_ms, AtomicOrdering::Relaxed);
    }

    /// Check idle state and emit `UserIdle` / `UserResumed` signals via the
    /// provided registry when the threshold is crossed.
    ///
    /// Call once per frame (cheap: one atomic load + optional watch send).
    pub(crate) fn tick_idle_watchdog(&mut self, registries: &RegistryRuntime) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_gesture_ms = self.last_user_gesture_ms.load(AtomicOrdering::Relaxed);
        let elapsed_ms = now_ms.saturating_sub(last_gesture_ms);

        if elapsed_ms >= self.worker_idle_threshold_ms && !self.currently_idle {
            self.currently_idle = true;
            let _ = self.p2p_sync_suspended_tx.send(true);
            let _ = self.nostr_relay_suspended_tx.send(true);
            registries.propagate_user_idle_signal(last_gesture_ms);
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_SYSTEM_TASK_BUDGET_WORKER_SUSPENDED,
                    byte_len: 0,
                },
            );
            log::debug!(
                "control_panel: Tier 1 workers suspended (idle {}s)",
                elapsed_ms / 1000
            );
        } else if elapsed_ms < self.worker_idle_threshold_ms && self.currently_idle {
            self.currently_idle = false;
            let _ = self.p2p_sync_suspended_tx.send(false);
            let _ = self.nostr_relay_suspended_tx.send(false);
            registries.propagate_user_resumed_signal();
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_SYSTEM_TASK_BUDGET_WORKER_RESUMED,
                    byte_len: 0,
                },
            );
            log::debug!("control_panel: Tier 1 workers resumed");
        }
    }

    /// Register a worker spawn at the given tier.
    ///
    /// Increments the spawn-site counter for the tier. Call once per
    /// `spawn_*_worker` invocation before returning.
    fn register_worker_tier(&mut self, tier: WorkerTier) {
        *self.registered_tiers.entry(tier).or_insert(0) += 1;
        log::debug!(
            "control_panel: registered {:?} worker (total for tier: {})",
            tier,
            self.registered_tiers[&tier],
        );
    }

    /// Return the number of times each tier has been spawned this session.
    ///
    /// Intended for diagnostics and future §4 budget enforcement.
    #[allow(dead_code)]
    pub(crate) fn registered_tier_counts(&self) -> &HashMap<WorkerTier, usize> {
        &self.registered_tiers
    }

    /// Spawn the CP3 prefetch scheduler worker.
    ///
    /// The scheduler emits prewarm lifecycle intents on a policy-driven
    /// cadence. Policy updates flow through `LifecyclePolicy` watch channels,
    /// including memory-pressure-aware pacing and selected-node targeting.
    pub(crate) fn spawn_prefetch_scheduler(&mut self) {
        let cancel = self.cancel.clone();
        let tx = self.intent_tx.clone();
        let policy_rx = self.lifecycle_policy_tx.subscribe();
        self.workers.spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {
                    log::debug!("control_panel: prefetch scheduler cancelled");
                }
                _ = prefetch_scheduler_worker(tx, policy_rx) => {}
            }
        });
        log::debug!("control_panel: prefetch scheduler spawned");
    }

    /// Spawn the Verse sync worker (P2P delta sync).
    pub(crate) fn spawn_p2p_sync_worker(&mut self) {
        let cancel = self.cancel.clone();
        let tx = self.intent_tx.clone();
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (discovery_result_tx, discovery_result_rx) = mpsc::unbounded_channel();
        self.sync_command_tx = Some(cmd_tx.clone());
        self.discovery_result_rx = Some(discovery_result_rx);
        let mut suspended_rx = self.p2p_sync_suspended_tx.subscribe();

        self.workers.spawn(async move {
            let resources = match verse::sync_worker_resources(
                crate::shell::desktop::runtime::registries::phase3_trusted_peers_handle(),
            ) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("control_panel: verse sync not available ({e})");
                    return;
                }
            };

            let worker = SyncWorker::new(
                resources.endpoint,
                resources.secret_key,
                resources.trusted_peers,
                resources.sync_logs,
                tx,
                cmd_rx,
                discovery_result_tx,
                cancel.clone(),
            );

            // Run the worker, but yield to the suspension watch channel when
            // suspended. When `suspended` is true the worker select! arm is
            // still running but no work is submitted (iroh socket stays open).
            tokio::select! {
                _ = cancel.cancelled() => {}
                _ = worker.run() => {}
                _ = async {
                    loop {
                        if suspended_rx.changed().await.is_err() {
                            break;
                        }
                        // Workers decide internally what low-frequency means;
                        // the suspension signal is advisory only — we let the
                        // inner run() loop handle actual throttling once it
                        // receives the flag via a future worker-side API.
                        // For now, log the transition.
                        if *suspended_rx.borrow() {
                            log::debug!("control_panel: p2p sync worker suspended (advisory)");
                        } else {
                            log::debug!("control_panel: p2p sync worker resumed");
                        }
                    }
                } => {}
            }
        });

        self.register_worker_tier(WorkerTier::Tier1P2pSync);
        log::debug!("control_panel: sync worker spawned");
    }

    pub(crate) fn spawn_nostr_relay_worker(&mut self, registries: Arc<RegistryRuntime>) {
        let cancel = self.cancel.clone();
        let mut suspended_rx = self.nostr_relay_suspended_tx.subscribe();
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        // Create the inbound event sink. The relay worker sends
        // (subscription_id, NostrSignedEvent) pairs; this task translates them
        // into GraphIntent::NostrEventReceived and queues them for the reducer.
        let (event_sink_tx, mut event_sink_rx) = tokio::sync::mpsc::unbounded_channel::<(
            String,
            crate::shell::desktop::runtime::registries::nostr_core::NostrSignedEvent,
        )>();

        // Register the event sink with the worker before it starts.
        let _ = command_tx.send(RelayWorkerCommand::SetEventSink {
            sink: Some(event_sink_tx),
        });

        registries.attach_nostr_relay_worker(command_tx);

        // Relay worker task.
        let worker_cancel = cancel.clone();
        self.workers.spawn(async move {
            NostrRelayWorker::new(command_rx, worker_cancel).run().await;
        });

        // Event dispatch task: translates inbound relay events into intents.
        let intent_tx = self.intent_tx.clone();
        self.workers.spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    item = event_sink_rx.recv() => {
                        let Some((subscription_id, event)) = item else { break };
                        let intent = GraphIntent::NostrEventReceived {
                            subscription_id,
                            event_id: event.event_id,
                            pubkey: event.pubkey,
                            created_at: event.created_at,
                            kind: event.kind,
                            content: event.content,
                            tags: event.tags,
                        };
                        let _ = intent_tx.send(QueuedIntent {
                            intent,
                            queued_at: std::time::Instant::now(),
                            source: IntentSource::NostrRelay,
                        }).await;
                    }
                }
            }
        });

        // Suspension observer task: logs advisory suspension state changes.
        // The relay worker internally decides what low-frequency mode means;
        // this task merely records transitions until NostrRelayWorker gains a
        // native suspension API.
        let suspend_cancel = self.cancel.clone();
        self.workers.spawn(async move {
            loop {
                tokio::select! {
                    _ = suspend_cancel.cancelled() => break,
                    result = suspended_rx.changed() => {
                        if result.is_err() { break; }
                        if *suspended_rx.borrow() {
                            log::debug!("control_panel: nostr relay worker suspended (advisory)");
                        } else {
                            log::debug!("control_panel: nostr relay worker resumed");
                        }
                    }
                }
            }
        });

        self.register_worker_tier(WorkerTier::Tier1NostrRelay);
        log::debug!("control_panel: nostr relay worker spawned");
    }

    pub(crate) fn spawn_agent(&mut self, agent: Box<dyn Agent>, registries: Arc<RegistryRuntime>) {
        let agent_id = agent.id().to_string();
        let agent_name = agent.display_name().to_string();
        let context = AgentContext {
            intent_tx: self.intent_tx.clone(),
            signal_rx: registries.subscribe_all_signals_async(),
            cancel: self.cancel.child_token(),
            registries: Arc::clone(&registries),
        };
        let handle = agent.spawn(context);
        self.workers.spawn(handle.task);
        registries.route_agent_spawned(&agent_id);
        log::debug!("control_panel: agent spawned ({agent_name}, {agent_id})");
    }

    /// Spawn a short-lived Shell/Register-owned task under the same
    /// supervision boundary as longer-lived background workers.
    pub(crate) fn spawn_supervised_task<F>(&mut self, label: &'static str, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.workers.spawn(task);
        log::debug!("control_panel: supervised task spawned ({label})");
    }

    /// Run a blocking host request under ControlPanel supervision and return a
    /// single-result mailbox that can be polled safely from the UI frame loop.
    pub(crate) fn spawn_blocking_host_request<T, F>(
        &mut self,
        label: &'static str,
        work: F,
    ) -> crossbeam_channel::Receiver<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let cancel = self.cancel.child_token();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.spawn_supervised_task(label, async move {
            if cancel.is_cancelled() {
                return;
            }

            let result = tokio::task::spawn_blocking(work).await;
            if cancel.is_cancelled() {
                return;
            }

            if let Ok(value) = result {
                let _ = tx.send(value);
            }
        });
        rx
    }

    pub(crate) fn spawn_registered_agent(
        &mut self,
        agent_id: &str,
        registries: Arc<RegistryRuntime>,
    ) -> Result<(), String> {
        let agent = registries
            .instantiate_agent(agent_id)
            .ok_or_else(|| format!("unknown agent: {agent_id}"))?;
        self.spawn_agent(agent, registries);
        Ok(())
    }

    /// Stub spawn point for the Matrix client worker (plan-only until MatrixCore lands).
    ///
    /// Registers `Tier1MatrixCore` so the tier classification is declared at
    /// the spawn site per §7.1. Replace the body with a real worker spawn when
    /// `MatrixCore` is implemented.
    #[allow(dead_code)]
    pub(crate) fn spawn_matrix_core_worker(&mut self) {
        self.register_worker_tier(WorkerTier::Tier1MatrixCore);
        log::debug!("control_panel: matrix core worker stub registered (not yet implemented)");
    }

    pub(crate) fn handle_protocol_probe_request(&mut self, key: NodeKey, url: Option<String>) {
        self.cancel_protocol_probe(key);

        let Some(url) = url else {
            return;
        };

        self.next_protocol_probe_nonce = self.next_protocol_probe_nonce.saturating_add(1);
        let nonce = self.next_protocol_probe_nonce;
        let cancel = self.cancel.child_token();
        self.active_protocol_probes
            .lock()
            .expect("protocol probe lock poisoned")
            .insert(key, (nonce, cancel.clone()));

        let tx = self.intent_tx.clone();
        let active_probes = Arc::clone(&self.active_protocol_probes);
        self.workers.spawn(async move {
            let prober = ContentTypeProber::default();
            let result = prober.probe(url, cancel.clone()).await;
            if let Some(result) = result {
                let _ = tx
                    .send(QueuedIntent {
                        intent: GraphIntent::UpdateNodeMimeHint {
                            key,
                            mime_hint: result.mime_hint,
                        },
                        queued_at: Instant::now(),
                        source: IntentSource::ProtocolProbe,
                    })
                    .await;
            }

            let mut guard = active_probes.lock().expect("protocol probe lock poisoned");
            if guard
                .get(&key)
                .is_some_and(|(current_nonce, _)| *current_nonce == nonce)
            {
                guard.remove(&key);
            }
        });
        log::debug!("control_panel: protocol probe spawned for node {key:?}");
    }

    pub(crate) fn cancel_protocol_probe(&mut self, key: NodeKey) {
        let cancelled = self
            .active_protocol_probes
            .lock()
            .expect("protocol probe lock poisoned")
            .remove(&key);
        if let Some((_, token)) = cancelled {
            token.cancel();
        }
    }

    /// Cancel all supervised workers and await their completion.
    ///
    /// Safe to call from an async context (e.g. the main app shutdown path).
    /// After this returns the `JoinSet` is empty and the channel is drained.
    pub(crate) async fn shutdown(&mut self) {
        log::debug!(
            "control_panel: shutdown requested — cancelling {} workers",
            self.workers.len()
        );
        self.cancel.cancel();
        while self.workers.join_next().await.is_some() {}
        log::debug!("control_panel: all workers joined");
    }

    /// Number of background workers currently supervised.
    #[cfg(test)]
    pub(crate) fn worker_count(&self) -> usize {
        self.workers.len()
    }

    #[cfg(test)]
    pub(crate) fn is_intent_channel_open_for_tests(&self) -> bool {
        !self.intent_tx.is_closed()
    }

    #[cfg(test)]
    pub(crate) fn enqueue_intent_for_tests(&self, queued: QueuedIntent) -> Result<(), String> {
        self.intent_tx
            .try_send(queued)
            .map_err(|e| format!("failed to enqueue test intent: {e}"))
    }

    #[cfg(test)]
    pub(crate) fn set_sync_command_sender_for_tests(&mut self, tx: mpsc::Sender<SyncCommand>) {
        self.sync_command_tx = Some(tx);
    }
}

impl Default for ControlPanel {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Memory monitor background worker.
///
/// Samples system memory every [`MEMORY_MONITOR_INTERVAL`] and emits a
/// `RuntimeEvent::SetMemoryPressureStatus` intent whenever the observed level
/// differs from the previous sample. Non-critical pressure changes are logged
/// but not emitted if the channel is full.
async fn memory_monitor_worker(tx: mpsc::Sender<QueuedIntent>) {
    let mut last_level = MemoryPressureLevel::Unknown;

    loop {
        tokio::time::sleep(MEMORY_MONITOR_INTERVAL).await;

        let (level, available_mib, total_mib) = sample_memory();

        if level == last_level {
            continue;
        }

        last_level = level;
        log::debug!(
            "control_panel: memory pressure changed to {:?} ({available_mib} MiB free of {total_mib} MiB)",
            level
        );

        let intent = QueuedIntent {
            intent: crate::app::RuntimeEvent::SetMemoryPressureStatus {
                level,
                available_mib,
                total_mib,
            }
            .into(),
            queued_at: Instant::now(),
            source: IntentSource::MemoryMonitor,
        };

        // Use try_send for non-critical updates: if the channel is full the
        // frame loop is behind; skip this sample rather than blocking.
        if let Err(e) = tx.try_send(intent) {
            log::debug!("control_panel: memory pressure intent dropped ({e})");
        }
    }
}

/// Mod loader background worker.
///
/// Discovers native mods and resolves dependency order. Emits activation
/// events on success and load-failure events on dependency/resolve errors.
async fn mod_loader_worker(tx: mpsc::Sender<QueuedIntent>) {
    let manifests = discover_native_mods();
    match resolve_mod_load_order(&manifests) {
        Ok(ordered) => {
            for manifest in ordered {
                let intent = QueuedIntent {
                    intent: crate::app::RuntimeEvent::ModActivated {
                        mod_id: manifest.mod_id,
                    }
                    .into(),
                    queued_at: Instant::now(),
                    source: IntentSource::ModLoader,
                };
                if let Err(e) = tx.send(intent).await {
                    log::debug!("control_panel: failed to emit mod activated intent ({e})");
                    break;
                }
            }
        }
        Err(error) => {
            let intent = QueuedIntent {
                intent: crate::app::RuntimeEvent::ModLoadFailed {
                    mod_id: "mod:bootstrap".to_string(),
                    reason: format!("{error:?}"),
                }
                .into(),
                queued_at: Instant::now(),
                source: IntentSource::ModLoader,
            };
            if let Err(e) = tx.send(intent).await {
                log::debug!("control_panel: failed to emit mod load failure intent ({e})");
            }
        }
    }
}

/// CP3 prefetch scheduler worker.
///
/// Backpressure policy: when the queue is congested, the worker backs off
/// exponentially up to [`PREFETCH_MAX_INTERVAL`] and retries later.
async fn prefetch_scheduler_worker(
    tx: mpsc::Sender<QueuedIntent>,
    mut policy_rx: watch::Receiver<LifecyclePolicy>,
) {
    let mut backoff = PREFETCH_MIN_INTERVAL;

    loop {
        let policy = *policy_rx.borrow_and_update();
        if !policy.prefetch_enabled {
            if policy_rx.changed().await.is_err() {
                return;
            }
            continue;
        }

        let wait_for = policy
            .prefetch_interval
            .clamp(PREFETCH_MIN_INTERVAL, PREFETCH_MAX_INTERVAL);
        let wait_for = match policy.memory_pressure_level {
            MemoryPressureLevel::Critical => PREFETCH_MAX_INTERVAL,
            MemoryPressureLevel::Warning => wait_for.max(Duration::from_secs(20)),
            MemoryPressureLevel::Normal | MemoryPressureLevel::Unknown => wait_for,
        };
        tokio::time::sleep(wait_for).await;

        let Some(target) = policy.prefetch_target else {
            continue;
        };

        let queued = QueuedIntent {
            intent: crate::app::RuntimeEvent::PromoteNodeToActive {
                key: target,
                cause: LifecycleCause::SelectedPrewarm,
            }
            .into(),
            queued_at: Instant::now(),
            source: IntentSource::PrefetchScheduler,
        };

        if tx.try_send(queued).is_ok() {
            backoff = PREFETCH_MIN_INTERVAL;
            continue;
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(PREFETCH_MAX_INTERVAL);
    }
}

/// Sample current system memory pressure.
///
/// Returns `(level, available_mib, total_mib)`. Mirrors the thresholds used
/// by `lifecycle_reconcile::sample_memory_pressure` for consistency.
fn sample_memory() -> (MemoryPressureLevel, u64, u64) {
    let mut system = System::new();
    system.refresh_memory();

    let total_bytes = system.total_memory();
    let available_bytes = system.available_memory();
    let total_mib = total_bytes / (1024 * 1024);
    let available_mib = available_bytes / (1024 * 1024);

    if total_bytes == 0 {
        return (MemoryPressureLevel::Unknown, available_mib, total_mib);
    }

    let available_pct = available_bytes as f64 / total_bytes as f64;
    let level = if available_mib <= 512 || available_pct <= 0.08 {
        MemoryPressureLevel::Critical
    } else if available_mib <= 1024 || available_pct <= 0.15 {
        MemoryPressureLevel::Warning
    } else {
        MemoryPressureLevel::Normal
    };

    (level, available_mib, total_mib)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    use super::*;
    use crate::shell::desktop::runtime::registries::agent::{
        Agent, AgentCapability, AgentContext, AgentHandle,
    };
    use crate::shell::desktop::runtime::registries::phase3_shared_runtime;

    struct CancelAwareTestAgent {
        cancelled: Arc<AtomicBool>,
    }

    impl Agent for CancelAwareTestAgent {
        fn id(&self) -> &'static str {
            "agent:test_cancel"
        }

        fn display_name(&self) -> &'static str {
            "Test cancel-aware agent"
        }

        fn declared_capabilities(&self) -> Vec<AgentCapability> {
            vec![AgentCapability::ReadNavigationSignals]
        }

        fn spawn(self: Box<Self>, context: AgentContext) -> AgentHandle {
            let cancelled = Arc::clone(&self.cancelled);
            AgentHandle::from_future(async move {
                context.cancel.cancelled().await;
                cancelled.store(true, Ordering::SeqCst);
            })
        }
    }

    fn spawn_head_server(content_type: &'static str, delay: Duration) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listener should bind");
        let address = listener
            .local_addr()
            .expect("listener should expose address");

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nContent-Type: {content_type}\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        format!("http://{}", address)
    }

    #[tokio::test]
    async fn control_panel_new_creates_open_channel() {
        let panel = ControlPanel::new(None);
        // Channel should be open (sender not dropped)
        assert!(panel.is_intent_channel_open_for_tests());
    }

    #[tokio::test]
    async fn drain_pending_returns_empty_when_no_intents() {
        let mut panel = ControlPanel::new(None);
        assert!(panel.drain_pending().is_empty());
    }

    #[tokio::test]
    async fn drain_pending_collects_queued_intents() {
        let mut panel = ControlPanel::new(None);
        panel
            .enqueue_intent_for_tests(QueuedIntent {
                intent: GraphIntent::Noop,
                queued_at: Instant::now(),
                source: IntentSource::MemoryMonitor,
            })
            .expect("channel should accept intent");

        let drained = panel.drain_pending();
        assert_eq!(drained.len(), 1);
        assert!(matches!(drained[0], GraphIntent::Noop));
    }

    #[tokio::test]
    async fn shutdown_completes_with_no_workers() {
        let mut panel = ControlPanel::new(None);
        // Shutdown with no workers should return immediately
        panel.shutdown().await;
        assert_eq!(panel.worker_count(), 0);
    }

    #[tokio::test]
    async fn spawn_memory_monitor_increments_worker_count() {
        let mut panel = ControlPanel::new(None);
        assert_eq!(panel.worker_count(), 0);
        panel.spawn_memory_monitor();
        // Give the JoinSet a tick to register the task
        tokio::task::yield_now().await;
        assert_eq!(panel.worker_count(), 1);
    }

    #[tokio::test]
    async fn shutdown_cancels_and_joins_all_workers() {
        let mut panel = ControlPanel::new(None);
        panel.spawn_memory_monitor();
        panel.shutdown().await;
        assert_eq!(panel.worker_count(), 0);
    }

    #[tokio::test]
    async fn spawn_agent_is_supervised_and_shutdown_cancels_it() {
        let runtime = phase3_shared_runtime();
        let mut panel = ControlPanel::new(None);
        let cancelled = Arc::new(AtomicBool::new(false));

        panel.spawn_agent(
            Box::new(CancelAwareTestAgent {
                cancelled: Arc::clone(&cancelled),
            }),
            runtime,
        );
        tokio::task::yield_now().await;
        assert_eq!(panel.worker_count(), 1);

        panel.shutdown().await;

        assert!(cancelled.load(Ordering::SeqCst));
        assert_eq!(panel.worker_count(), 0);
    }

    #[tokio::test]
    async fn spawn_blocking_host_request_is_supervised_and_returns_result() {
        let mut panel = ControlPanel::new(None);

        let rx = panel.spawn_blocking_host_request("test_blocking_request", || 42usize);
        tokio::task::yield_now().await;

        assert_eq!(panel.worker_count(), 1);
        let value = tokio::task::spawn_blocking(move || {
            rx.recv_timeout(Duration::from_secs(2))
                .expect("blocking host request should return a value")
        })
        .await
        .expect("join should succeed");
        assert_eq!(value, 42);

        panel.shutdown().await;
        assert_eq!(panel.worker_count(), 0);
    }

    #[tokio::test]
    async fn spawn_mod_loader_increments_worker_count() {
        let mut panel = ControlPanel::new(None);
        assert_eq!(panel.worker_count(), 0);
        panel.spawn_mod_loader();
        tokio::task::yield_now().await;
        assert_eq!(panel.worker_count(), 1);
    }

    #[tokio::test]
    async fn mod_loader_emits_activated_intent_on_success() {
        let (tx, mut rx) = mpsc::channel(4);
        tokio::spawn(async move {
            mod_loader_worker_with_manifests(
                tx,
                vec![
                    crate::registries::infrastructure::mod_loader::ModManifest::new(
                        "mod:test",
                        "Test",
                        crate::registries::infrastructure::mod_loader::ModType::Native,
                        vec!["viewer:test".to_string()],
                        vec![],
                        vec![],
                    ),
                ],
            )
            .await;
        });

        let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("mod loader should emit an intent")
            .expect("channel should remain open");

        assert!(
            matches!(received.intent, GraphIntent::ModActivated { mod_id } if mod_id == "mod:test")
        );
    }

    #[tokio::test]
    async fn mod_loader_emits_failed_intent_on_load_error() {
        let (tx, mut rx) = mpsc::channel(4);
        tokio::spawn(async move {
            mod_loader_worker_with_manifests(
                tx,
                vec![
                    crate::registries::infrastructure::mod_loader::ModManifest::new(
                        "mod:broken",
                        "Broken",
                        crate::registries::infrastructure::mod_loader::ModType::Native,
                        vec!["viewer:broken".to_string()],
                        vec!["ProtocolRegistry".to_string()],
                        vec![],
                    ),
                ],
            )
            .await;
        });

        let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("worker should emit an intent")
            .expect("channel should remain open");

        assert!(
            matches!(received.intent, GraphIntent::ModLoadFailed { mod_id, .. } if mod_id == "mod:bootstrap")
        );
    }

    #[tokio::test]
    async fn spawn_prefetch_scheduler_increments_worker_count() {
        let mut panel = ControlPanel::new(None);
        assert_eq!(panel.worker_count(), 0);
        panel.spawn_prefetch_scheduler();
        tokio::task::yield_now().await;
        assert_eq!(panel.worker_count(), 1);
    }

    #[tokio::test]
    async fn prefetch_scheduler_emits_intent_when_enabled() {
        let (tx, mut rx) = mpsc::channel(2);
        let target = NodeKey::new(7);
        let (policy_tx, policy_rx) = watch::channel(LifecyclePolicy {
            prefetch_enabled: true,
            prefetch_interval: Duration::from_millis(5),
            prefetch_target: Some(target),
            memory_pressure_level: MemoryPressureLevel::Normal,
        });
        let _keep_policy_alive = policy_tx;

        let worker = tokio::spawn(async move {
            prefetch_scheduler_worker(tx, policy_rx).await;
        });

        let queued = tokio::time::timeout(Duration::from_secs(4), rx.recv())
            .await
            .expect("prefetch worker should queue an intent")
            .expect("channel should remain open");

        assert!(matches!(
            queued.intent,
            GraphIntent::PromoteNodeToActive {
                key,
                cause: LifecycleCause::SelectedPrewarm,
            } if key == target
        ));
        assert_eq!(queued.source, IntentSource::PrefetchScheduler);

        worker.abort();
        let _ = worker.await;
    }

    #[tokio::test]
    async fn spawn_p2p_sync_worker_sets_panel_command_channel() {
        let mut panel = ControlPanel::new(None);
        panel.spawn_p2p_sync_worker();
        tokio::task::yield_now().await;

        assert!(panel.sync_command_sender().is_some());

        panel.shutdown().await;
    }

    #[tokio::test]
    async fn spawn_nostr_relay_worker_is_supervised() {
        let runtime = phase3_shared_runtime();
        let mut panel = ControlPanel::new(None);
        assert_eq!(panel.worker_count(), 0);

        panel.spawn_nostr_relay_worker(runtime);
        tokio::task::yield_now().await;

        // spawn_nostr_relay_worker spawns three tasks: the relay worker, the
        // event dispatch bridge that translates inbound events into intents,
        // and the suspension observer that logs advisory suspension state changes.
        assert_eq!(panel.worker_count(), 3);

        panel.shutdown().await;
        assert_eq!(panel.worker_count(), 0);
    }

    #[tokio::test]
    async fn request_discover_nearby_requires_sync_worker_channel() {
        let panel = ControlPanel::new(None);
        let result = panel.request_discover_nearby_peers(2);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn request_discover_nearby_enqueues_after_sync_worker_spawn() {
        let mut panel = ControlPanel::new(None);
        let (tx, mut rx) = mpsc::channel(1);
        panel.set_sync_command_sender_for_tests(tx);

        panel
            .request_discover_nearby_peers(2)
            .expect("discovery request should enqueue");

        let command = rx
            .recv()
            .await
            .expect("command should be received by test channel");
        assert!(matches!(
            command,
            SyncCommand::DiscoverNearby { timeout_secs: 2 }
        ));
    }

    #[tokio::test]
    async fn protocol_probe_request_emits_update_node_mime_hint_intent() {
        let mut panel = ControlPanel::new(None);
        let key = NodeKey::new(41);
        let url = spawn_head_server("text/csv; charset=utf-8", Duration::ZERO);

        panel.handle_protocol_probe_request(key, Some(url));

        let drained = tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                let drained = panel.drain_pending();
                if !drained.is_empty() {
                    return drained;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("protocol probe should emit an intent");

        assert!(matches!(
            drained.first(),
            Some(GraphIntent::UpdateNodeMimeHint { key: emitted_key, mime_hint })
                if *emitted_key == key && mime_hint.as_deref() == Some("text/csv")
        ));
    }

    #[tokio::test]
    async fn protocol_probe_cancellation_prevents_mime_intent_delivery() {
        let mut panel = ControlPanel::new(None);
        let key = NodeKey::new(42);
        let url = spawn_head_server("application/pdf", Duration::from_millis(150));

        panel.handle_protocol_probe_request(key, Some(url));
        tokio::time::sleep(Duration::from_millis(20)).await;
        panel.handle_protocol_probe_request(key, None);

        tokio::time::sleep(Duration::from_millis(220)).await;
        assert!(panel.drain_pending().is_empty());
    }

    #[tokio::test]
    async fn drain_pending_sorts_by_causality_priority_then_time() {
        let mut panel = ControlPanel::new(None);
        let now = Instant::now();

        panel
            .enqueue_intent_for_tests(QueuedIntent {
                intent: GraphIntent::Noop,
                queued_at: now + Duration::from_millis(30),
                source: IntentSource::ModLoader,
            })
            .expect("channel should accept first queued intent");
        panel
            .enqueue_intent_for_tests(QueuedIntent {
                intent: GraphIntent::Undo,
                queued_at: now + Duration::from_millis(10),
                source: IntentSource::LocalUI,
            })
            .expect("channel should accept second queued intent");
        panel
            .enqueue_intent_for_tests(QueuedIntent {
                intent: GraphIntent::Redo,
                queued_at: now + Duration::from_millis(20),
                source: IntentSource::ServoDelegate,
            })
            .expect("channel should accept third queued intent");

        let drained = panel.drain_pending();
        assert_eq!(drained.len(), 3);
        assert!(matches!(drained[0], GraphIntent::Undo));
        assert!(matches!(drained[1], GraphIntent::Redo));
        assert!(matches!(drained[2], GraphIntent::Noop));
    }

    #[tokio::test]
    async fn drain_pending_is_deterministic_under_concurrent_producers() {
        let mut panel = ControlPanel::new(None);
        let base = Instant::now();

        let tx_a = panel.intent_tx.clone();
        let tx_b = panel.intent_tx.clone();
        let tx_c = panel.intent_tx.clone();

        let a = tokio::spawn(async move {
            let _ = tx_a
                .send(QueuedIntent {
                    intent: GraphIntent::Noop,
                    queued_at: base + Duration::from_millis(3),
                    source: IntentSource::PrefetchScheduler,
                })
                .await;
        });
        let b = tokio::spawn(async move {
            let _ = tx_b
                .send(QueuedIntent {
                    intent: GraphIntent::Undo,
                    queued_at: base + Duration::from_millis(1),
                    source: IntentSource::LocalUI,
                })
                .await;
        });
        let c = tokio::spawn(async move {
            let _ = tx_c
                .send(QueuedIntent {
                    intent: GraphIntent::Redo,
                    queued_at: base + Duration::from_millis(2),
                    source: IntentSource::ServoDelegate,
                })
                .await;
        });

        let _ = tokio::join!(a, b, c);

        let drained = panel.drain_pending();
        assert_eq!(drained.len(), 3);
        assert!(matches!(drained[0], GraphIntent::Undo));
        assert!(matches!(drained[1], GraphIntent::Redo));
        assert!(matches!(drained[2], GraphIntent::Noop));
    }

    async fn mod_loader_worker_with_manifests(
        tx: mpsc::Sender<QueuedIntent>,
        manifests: Vec<crate::registries::infrastructure::mod_loader::ModManifest>,
    ) {
        match resolve_mod_load_order(&manifests) {
            Ok(ordered) => {
                for manifest in ordered {
                    let _ = tx
                        .send(QueuedIntent {
                            intent: GraphIntent::ModActivated {
                                mod_id: manifest.mod_id,
                            },
                            queued_at: Instant::now(),
                            source: IntentSource::ModLoader,
                        })
                        .await;
                }
            }
            Err(error) => {
                let _ = tx
                    .send(QueuedIntent {
                        intent: GraphIntent::ModLoadFailed {
                            mod_id: "mod:bootstrap".to_string(),
                            reason: format!("{error:?}"),
                        },
                        queued_at: Instant::now(),
                        source: IntentSource::ModLoader,
                    })
                    .await;
            }
        }
    }
}
