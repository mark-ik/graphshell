use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::graph::NodeKey;

/// Topic families used by the Register signal routing layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SignalTopic {
    Navigation,
    Lifecycle,
    Sync,
    RegistryEvent,
    InputEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NavigationSignal {
    Resolved {
        uri: String,
        viewer_id: String,
    },
    MimeResolved {
        key: NodeKey,
        uri: String,
        mime_hint: Option<String>,
        viewer_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LifecycleSignal {
    SemanticIndexUpdated {
        indexed_nodes: usize,
    },
    MimeResolved {
        node_key: NodeKey,
        mime: String,
    },
    WorkflowActivated {
        workflow_id: String,
    },
    MemoryPressureChanged {
        level: String,
        available_mib: u64,
        total_mib: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncSignal {
    RemoteEntriesQueued,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RegistryEventSignal {
    ThemeChanged {
        new_theme_id: String,
    },
    LensChanged {
        new_lens_id: String,
    },
    WorkflowChanged {
        new_workflow_id: String,
    },
    PhysicsProfileChanged {
        new_profile_id: String,
    },
    CanvasProfileChanged {
        new_profile_id: String,
    },
    WorkbenchSurfaceChanged {
        new_profile_id: String,
    },
    SemanticIndexUpdated {
        indexed_nodes: usize,
    },
    ModLoaded {
        mod_id: String,
    },
    ModUnloaded {
        mod_id: String,
    },
    AgentSpawned {
        agent_id: String,
    },
    IdentityRotated {
        identity_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputEventSignal {
    ContextChanged {
        new_context: String,
    },
    BindingRemapped {
        action_id: String,
    },
    BindingsReset,
}

/// Typed signal kinds emitted through Register-owned routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SignalKind {
    Navigation(NavigationSignal),
    Lifecycle(LifecycleSignal),
    Sync(SyncSignal),
    RegistryEvent(RegistryEventSignal),
    InputEvent(InputEventSignal),
}

impl SignalKind {
    pub(crate) fn topic(&self) -> SignalTopic {
        match self {
            Self::Navigation(..) => SignalTopic::Navigation,
            Self::Lifecycle(..) => SignalTopic::Lifecycle,
            Self::Sync(..) => SignalTopic::Sync,
            Self::RegistryEvent(..) => SignalTopic::RegistryEvent,
            Self::InputEvent(..) => SignalTopic::InputEvent,
        }
    }
}

/// Producer identity for tracing and causality debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignalSource {
    RegistryRuntime,
    ControlPanel,
}

/// Typed signal envelope with source metadata and optional causality stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignalEnvelope {
    pub(crate) kind: SignalKind,
    pub(crate) source: SignalSource,
    pub(crate) emitted_at: Instant,
    pub(crate) causality_stamp: Option<u64>,
}

impl SignalEnvelope {
    pub(crate) fn new(
        kind: SignalKind,
        source: SignalSource,
        causality_stamp: Option<u64>,
    ) -> Self {
        Self {
            kind,
            source,
            emitted_at: Instant::now(),
            causality_stamp,
        }
    }
}

/// Stable identifier for a registered observer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ObserverId(u64);

pub(crate) type SyncObserverCallback =
    Arc<dyn Fn(&SignalEnvelope) -> Result<(), String> + Send + Sync>;

#[derive(Clone)]
struct SignalObserver {
    id: ObserverId,
    callback: SyncObserverCallback,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SignalRoutingDiagnostics {
    pub(crate) published_signals: u64,
    pub(crate) routed_deliveries: u64,
    pub(crate) unrouted_signals: u64,
    pub(crate) observer_failures: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SignalPublishReport {
    pub(crate) observers_notified: usize,
    pub(crate) observer_failures: usize,
}

#[derive(Default)]
struct SignalRoutingState {
    next_observer_id: u64,
    observers: HashMap<SignalTopic, Vec<SignalObserver>>,
    diagnostics: SignalRoutingDiagnostics,
}

/// SR2/SR3 transitional Register-owned signal routing facade and in-process fabric.
#[derive(Default, Clone)]
pub(crate) struct SignalRoutingLayer {
    state: Arc<Mutex<SignalRoutingState>>,
}

impl SignalRoutingLayer {
    pub(crate) fn subscribe(
        &self,
        topic: SignalTopic,
        callback: impl Fn(&SignalEnvelope) -> Result<(), String> + Send + Sync + 'static,
    ) -> ObserverId {
        let mut guard = self.state.lock().expect("signal routing lock poisoned");
        guard.next_observer_id = guard.next_observer_id.saturating_add(1);
        let id = ObserverId(guard.next_observer_id);
        let observer = SignalObserver {
            id,
            callback: Arc::new(callback),
        };
        guard.observers.entry(topic).or_default().push(observer);
        id
    }

    pub(crate) fn unsubscribe(&self, topic: SignalTopic, id: ObserverId) -> bool {
        let mut guard = self.state.lock().expect("signal routing lock poisoned");
        let Some(observers) = guard.observers.get_mut(&topic) else {
            return false;
        };
        let len_before = observers.len();
        observers.retain(|entry| entry.id != id);
        len_before != observers.len()
    }

    pub(crate) fn publish(&self, envelope: SignalEnvelope) -> SignalPublishReport {
        let topic = envelope.kind.topic();
        let callbacks = {
            let mut guard = self.state.lock().expect("signal routing lock poisoned");
            guard.diagnostics.published_signals =
                guard.diagnostics.published_signals.saturating_add(1);
            let Some(observers) = guard.observers.get(&topic) else {
                guard.diagnostics.unrouted_signals =
                    guard.diagnostics.unrouted_signals.saturating_add(1);
                return SignalPublishReport {
                    observers_notified: 0,
                    observer_failures: 0,
                };
            };
            observers
                .iter()
                .map(|entry| entry.callback.clone())
                .collect::<Vec<_>>()
        };

        let mut failures = 0usize;
        for callback in &callbacks {
            if callback(&envelope).is_err() {
                failures = failures.saturating_add(1);
            }
        }

        let mut guard = self.state.lock().expect("signal routing lock poisoned");
        guard.diagnostics.routed_deliveries = guard
            .diagnostics
            .routed_deliveries
            .saturating_add(callbacks.len() as u64);
        guard.diagnostics.observer_failures = guard
            .diagnostics
            .observer_failures
            .saturating_add(failures as u64);

        SignalPublishReport {
            observers_notified: callbacks.len(),
            observer_failures: failures,
        }
    }

    pub(crate) fn diagnostics_snapshot(&self) -> SignalRoutingDiagnostics {
        self.state
            .lock()
            .expect("signal routing lock poisoned")
            .diagnostics
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn signal_routing_notifies_two_observers_for_single_producer_publish() {
        let layer = SignalRoutingLayer::default();
        let observer_a = Arc::new(AtomicUsize::new(0));
        let observer_b = Arc::new(AtomicUsize::new(0));

        {
            let observer_a = Arc::clone(&observer_a);
            layer.subscribe(SignalTopic::Navigation, move |_| {
                observer_a.fetch_add(1, Ordering::Relaxed);
                Ok(())
            });
        }

        {
            let observer_b = Arc::clone(&observer_b);
            layer.subscribe(SignalTopic::Navigation, move |_| {
                observer_b.fetch_add(1, Ordering::Relaxed);
                Ok(())
            });
        }

        let report = layer.publish(SignalEnvelope::new(
            SignalKind::Navigation(NavigationSignal::Resolved {
                uri: "https://example.com".to_string(),
                viewer_id: "viewer:webview".to_string(),
            }),
            SignalSource::RegistryRuntime,
            Some(7),
        ));

        assert_eq!(report.observers_notified, 2);
        assert_eq!(report.observer_failures, 0);
        assert_eq!(observer_a.load(Ordering::Relaxed), 1);
        assert_eq!(observer_b.load(Ordering::Relaxed), 1);

        let diagnostics = layer.diagnostics_snapshot();
        assert_eq!(diagnostics.published_signals, 1);
        assert_eq!(diagnostics.routed_deliveries, 2);
        assert_eq!(diagnostics.unrouted_signals, 0);
        assert_eq!(diagnostics.observer_failures, 0);
    }

    #[test]
    fn signal_routing_tracks_unrouted_and_failed_deliveries() {
        let layer = SignalRoutingLayer::default();

        let unrouted = layer.publish(SignalEnvelope::new(
            SignalKind::Lifecycle(LifecycleSignal::MemoryPressureChanged {
                level: "warning".to_string(),
                available_mib: 512,
                total_mib: 2048,
            }),
            SignalSource::ControlPanel,
            None,
        ));
        assert_eq!(unrouted.observers_notified, 0);

        layer.subscribe(SignalTopic::Sync, |_| Err("forced failure".to_string()));
        let failed = layer.publish(SignalEnvelope::new(
            SignalKind::Sync(SyncSignal::RemoteEntriesQueued),
            SignalSource::ControlPanel,
            None,
        ));
        assert_eq!(failed.observers_notified, 1);
        assert_eq!(failed.observer_failures, 1);

        let diagnostics = layer.diagnostics_snapshot();
        assert_eq!(diagnostics.published_signals, 2);
        assert_eq!(diagnostics.routed_deliveries, 1);
        assert_eq!(diagnostics.unrouted_signals, 1);
        assert_eq!(diagnostics.observer_failures, 1);
    }
}
