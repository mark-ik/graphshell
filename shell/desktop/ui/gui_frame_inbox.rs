/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shell-side spawn wiring for the host-neutral [`FrameInboxState`].
//!
//! Ungated — used by both the egui-host (Servo) and iced-host launch
//! paths.  The portable receiver state and drain helpers live in
//! [`graphshell_runtime::frame_inbox`].  This module owns only the
//! ControlPanel-driven subscription wiring that translates
//! registry/lifecycle signals into receiver sends.

use std::sync::Arc;

use futures_util::StreamExt;
use graphshell_core::signal_router::{
    LifecycleSignal, RegistrySignal, SignalEnvelope, SignalRouter, SignalTopic,
};
use graphshell_runtime::FrameInboxState;

use crate::shell::desktop::runtime::control_panel::ControlPanel;

/// Shell-side alias for the frame inbox type used by both hosts.
pub(crate) type GuiFrameInbox = FrameInboxState;

/// Spawn the registry/lifecycle relays on `control_panel` and return a
/// frame inbox bound to their receivers.
pub(crate) fn spawn_gui_frame_inbox(
    control_panel: &mut ControlPanel,
    signal_router: Arc<dyn SignalRouter>,
) -> GuiFrameInbox {
    let (semantic_index_updates_tx, semantic_index_updates_rx) = std::sync::mpsc::channel();
    let (workbench_projection_refreshes_tx, workbench_projection_refreshes_rx) =
        std::sync::mpsc::channel();
    let (settings_route_requests_tx, settings_route_requests_rx) = std::sync::mpsc::channel();
    let (profile_invalidations_tx, profile_invalidations_rx) = std::sync::mpsc::channel();

    let lifecycle_router = Arc::clone(&signal_router);
    control_panel.spawn_shell_signal_relay("shell_frame_inbox_lifecycle", async move {
        let mut subscription = lifecycle_router.subscribe(SignalTopic::Lifecycle);
        while let Some(signal) = subscription.next().await {
            if let SignalEnvelope::Lifecycle(LifecycleSignal::SemanticIndexUpdated {
                indexed_nodes,
            }) = signal
            {
                let _ = semantic_index_updates_tx.send(indexed_nodes);
            }
        }
    });

    let registry_router = Arc::clone(&signal_router);
    control_panel.spawn_shell_signal_relay("shell_frame_inbox_registry", async move {
        let mut subscription = registry_router.subscribe(SignalTopic::RegistryEvent);
        while let Some(signal) = subscription.next().await {
            if let SignalEnvelope::RegistryEvent(registry_signal) = signal {
                match registry_signal {
                    RegistrySignal::WorkbenchProjectionRefreshRequested => {
                        let _ = workbench_projection_refreshes_tx.send(());
                    }
                    RegistrySignal::SettingsRouteRequested { url } => {
                        let _ = settings_route_requests_tx.send(url);
                    }
                    RegistrySignal::ThemeChanged
                    | RegistrySignal::LensChanged
                    | RegistrySignal::PhysicsProfileChanged
                    | RegistrySignal::CanvasProfileChanged
                    | RegistrySignal::WorkbenchSurfaceChanged => {
                        let _ = profile_invalidations_tx.send(());
                    }
                }
            }
        }
    });

    FrameInboxState::new(
        semantic_index_updates_rx,
        workbench_projection_refreshes_rx,
        settings_route_requests_rx,
        profile_invalidations_rx,
    )
}

#[cfg(test)]
mod tests {
    use super::spawn_gui_frame_inbox;
    use std::sync::Arc;

    use crate::shell::desktop::runtime::control_panel::{ControlPanel, WorkerTier};
    use crate::shell::desktop::runtime::registry_signal_router::RegistrySignalRouter;

    #[test]
    fn frame_inbox_spawn_uses_control_panel_runtime_handle_outside_ambient_context() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime should initialize");
        let mut panel = ControlPanel::new_with_runtime(None, runtime.handle().clone());
        let signal_router = Arc::new(RegistrySignalRouter);

        let _inbox = spawn_gui_frame_inbox(&mut panel, signal_router);

        assert_eq!(panel.worker_count(), 2);
        assert_eq!(
            panel
                .registered_tier_counts()
                .get(&WorkerTier::Tier1ShellSignalRelay),
            Some(&2)
        );

        runtime.block_on(async {
            tokio::task::yield_now().await;
            panel.shutdown().await;
        });
        assert_eq!(panel.worker_count(), 0);
    }
}
