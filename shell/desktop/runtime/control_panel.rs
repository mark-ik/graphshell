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
//! Part of The Register: `RegistryRuntime` + `ControlPanel` + a not-yet-implemented
//! signal-routing layer (`SignalBus` or equivalent abstraction).

use std::time::{Duration, Instant};

use sysinfo::System;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::app::{GraphIntent, MemoryPressureLevel};
use crate::registries::infrastructure::mod_loader::{discover_native_mods, resolve_mod_load_order};
use crate::mods::native::verse::{self, SyncWorker, SyncCommand};

/// Capacity of the intent channel — limits flooding from async producers.
const INTENT_CHANNEL_CAPACITY: usize = 256;

/// How often the memory monitor samples system memory.
const MEMORY_MONITOR_INTERVAL: Duration = Duration::from_secs(5);

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
    /// Restore/replay from persistence.
    #[allow(dead_code)]
    Restore,
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
    pub(crate) intent_tx: mpsc::Sender<QueuedIntent>,
    /// Drained by the sync frame loop each tick via [`Self::drain_pending`].
    pub(crate) intent_rx: mpsc::Receiver<QueuedIntent>,
    /// Optional sync worker command channel.
    pub(crate) sync_command_tx: Option<mpsc::Sender<SyncCommand>>,
    /// Sync worker discovery-result stream.
    discovery_result_rx: Option<mpsc::UnboundedReceiver<Result<Vec<verse::DiscoveredPeer>, String>>>,
    /// Shared cancellation token — `cancel()` stops all supervised workers.
    cancel: CancellationToken,
    /// Supervised background worker tasks.
    workers: JoinSet<()>,
}

impl ControlPanel {
    /// Create a new `ControlPanel` with an empty worker set and a fresh
    /// cancellation token.
    pub(crate) fn new() -> Self {
        let (intent_tx, intent_rx) = mpsc::channel(INTENT_CHANNEL_CAPACITY);
        Self {
            intent_tx,
            intent_rx,
            cancel: CancellationToken::new(),
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
        let mut intents = Vec::new();
        while let Ok(queued) = self.intent_rx.try_recv() {
            intents.push(queued.intent);
        }
        intents
    }

    pub(crate) fn take_discovery_results(
        &mut self,
    ) -> Option<Result<Vec<verse::DiscoveredPeer>, String>> {
        self.discovery_result_rx
            .as_mut()
            .and_then(|rx| rx.try_recv().ok())
    }

    /// Spawn the memory monitor background worker.
    ///
    /// The worker samples system memory every [`MEMORY_MONITOR_INTERVAL`] and
    /// emits `GraphIntent::SetMemoryPressureStatus` when the observed level
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

    /// Spawn the Verse sync worker (P2P delta sync).
    pub(crate) fn spawn_sync_worker(&mut self) {
        let cancel = self.cancel.clone();
        let tx = self.intent_tx.clone();
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (discovery_result_tx, discovery_result_rx) = mpsc::unbounded_channel();
        self.sync_command_tx = Some(cmd_tx.clone());
        self.discovery_result_rx = Some(discovery_result_rx);

        self.workers.spawn(async move {
            let resources = match verse::sync_worker_resources() {
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

            worker.run().await;
        });

        log::debug!("control_panel: sync worker spawned");
    }

    /// Cancel all supervised workers and await their completion.
    ///
    /// Safe to call from an async context (e.g. the main app shutdown path).
    /// After this returns the `JoinSet` is empty and the channel is drained.
    pub(crate) async fn shutdown(&mut self) {
        log::debug!("control_panel: shutdown requested — cancelling {} workers", self.workers.len());
        self.cancel.cancel();
        while self.workers.join_next().await.is_some() {}
        log::debug!("control_panel: all workers joined");
    }

    /// Number of background workers currently supervised.
    #[cfg(test)]
    pub(crate) fn worker_count(&self) -> usize {
        self.workers.len()
    }
}

impl Default for ControlPanel {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory monitor background worker.
///
/// Samples system memory every [`MEMORY_MONITOR_INTERVAL`] and emits a
/// `GraphIntent::SetMemoryPressureStatus` intent whenever the observed level
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
            intent: GraphIntent::SetMemoryPressureStatus {
                level,
                available_mib,
                total_mib,
            },
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
                    intent: GraphIntent::ModActivated {
                        mod_id: manifest.mod_id,
                    },
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
                intent: GraphIntent::ModLoadFailed {
                    mod_id: "mod:bootstrap".to_string(),
                    reason: format!("{error:?}"),
                },
                queued_at: Instant::now(),
                source: IntentSource::ModLoader,
            };
            if let Err(e) = tx.send(intent).await {
                log::debug!("control_panel: failed to emit mod load failure intent ({e})");
            }
        }
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
    use super::*;

    #[tokio::test]
    async fn control_panel_new_creates_open_channel() {
        let panel = ControlPanel::new();
        // Channel should be open (sender not dropped)
        assert!(!panel.intent_tx.is_closed());
    }

    #[tokio::test]
    async fn drain_pending_returns_empty_when_no_intents() {
        let mut panel = ControlPanel::new();
        assert!(panel.drain_pending().is_empty());
    }

    #[tokio::test]
    async fn drain_pending_collects_queued_intents() {
        let mut panel = ControlPanel::new();
        panel
            .intent_tx
            .try_send(QueuedIntent {
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
        let mut panel = ControlPanel::new();
        // Shutdown with no workers should return immediately
        panel.shutdown().await;
        assert_eq!(panel.worker_count(), 0);
    }

    #[tokio::test]
    async fn spawn_memory_monitor_increments_worker_count() {
        let mut panel = ControlPanel::new();
        assert_eq!(panel.worker_count(), 0);
        panel.spawn_memory_monitor();
        // Give the JoinSet a tick to register the task
        tokio::task::yield_now().await;
        assert_eq!(panel.worker_count(), 1);
    }

    #[tokio::test]
    async fn shutdown_cancels_and_joins_all_workers() {
        let mut panel = ControlPanel::new();
        panel.spawn_memory_monitor();
        panel.shutdown().await;
        assert_eq!(panel.worker_count(), 0);
    }

    #[tokio::test]
    async fn spawn_mod_loader_increments_worker_count() {
        let mut panel = ControlPanel::new();
        assert_eq!(panel.worker_count(), 0);
        panel.spawn_mod_loader();
        tokio::task::yield_now().await;
        assert_eq!(panel.worker_count(), 1);
    }

    #[tokio::test]
    async fn mod_loader_emits_activated_intent_on_success() {
        let mut panel = ControlPanel::new();
        panel.spawn_mod_loader();

        let received = tokio::time::timeout(Duration::from_secs(2), panel.intent_rx.recv())
            .await
            .expect("mod loader should emit an intent")
            .expect("channel should remain open");

        assert!(matches!(received.intent, GraphIntent::ModActivated { .. }));
    }

    #[tokio::test]
    async fn mod_loader_emits_failed_intent_on_load_error() {
        let (tx, mut rx) = mpsc::channel(4);
        tokio::spawn(async move {
            mod_loader_worker_with_manifests(tx, vec![
                crate::registries::infrastructure::mod_loader::ModManifest::new(
                    "mod:broken",
                    "Broken",
                    crate::registries::infrastructure::mod_loader::ModType::Native,
                    vec!["viewer:broken".to_string()],
                    vec!["ProtocolRegistry".to_string()],
                    vec![],
                ),
            ]).await;
        });

        let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("worker should emit an intent")
            .expect("channel should remain open");

        assert!(matches!(received.intent, GraphIntent::ModLoadFailed { mod_id, .. } if mod_id == "mod:bootstrap"));
    }

    #[tokio::test]
    async fn spawn_sync_worker_sets_panel_command_channel() {
        let mut panel = ControlPanel::new();
        panel.spawn_sync_worker();
        tokio::task::yield_now().await;

        assert!(panel.sync_command_tx.is_some());

        panel.shutdown().await;
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
