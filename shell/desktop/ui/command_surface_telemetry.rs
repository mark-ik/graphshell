/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command-surface telemetry singleton.
//!
//! Consolidates the publish-latest-snapshot cell and the event sequence
//! counter cell that widgets emit into as the user interacts with the
//! command bar, omnibar, and command palette. Previously two parallel
//! `OnceLock<Mutex<…>>` statics in `toolbar/toolbar_ui.rs`; collapsed
//! into a single `CommandSurfaceTelemetry` struct with a methods-based
//! API so the responsibility is named in one place and a future runtime
//! slice can move ownership onto `GraphshellRuntime` without another
//! public-API break.
//!
//! The shapes exposed here (`CommandSurfaceSemanticSnapshot`,
//! `CommandSurfaceEventSequenceMetadata`, etc.) are consumed by
//! `ux_probes` diagnostics and several widget-level tests, so they
//! remain `pub(crate)`. The telemetry singleton is a transitional global
//! — not host-coupled (no egui/iced types in its surface), but not yet
//! runtime-owned either. iced and egui hosts share the same cell.

use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::app::ToolSurfaceReturnTarget;
use crate::graph::NodeKey;
use crate::shell::desktop::workbench::pane_model::PaneId;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandBarSemanticMetadata {
    pub(crate) active_pane: Option<PaneId>,
    pub(crate) focused_node: Option<NodeKey>,
    pub(crate) location_focused: bool,
    pub(crate) route_events: CommandRouteEventSequenceMetadata,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandRouteEventSequenceMetadata {
    pub(crate) resolved: u64,
    pub(crate) fallback: u64,
    pub(crate) no_target: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct OmnibarMailboxEventSequenceMetadata {
    pub(crate) request_started: u64,
    pub(crate) applied: u64,
    pub(crate) failed: u64,
    pub(crate) stale: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandSurfaceEventSequenceMetadata {
    pub(crate) route_events: CommandRouteEventSequenceMetadata,
    pub(crate) omnibar_mailbox_events: OmnibarMailboxEventSequenceMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct OmnibarSemanticMetadata {
    pub(crate) active: bool,
    pub(crate) focused: bool,
    pub(crate) query: Option<String>,
    pub(crate) match_count: usize,
    pub(crate) provider_status: Option<String>,
    pub(crate) active_pane: Option<PaneId>,
    pub(crate) focused_node: Option<NodeKey>,
    pub(crate) mailbox_events: OmnibarMailboxEventSequenceMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PaletteSurfaceSemanticMetadata {
    pub(crate) contextual_mode: bool,
    pub(crate) return_target: Option<ToolSurfaceReturnTarget>,
    pub(crate) pending_node_context_target: Option<NodeKey>,
    pub(crate) pending_frame_context_target: Option<String>,
    pub(crate) context_anchor_present: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CommandSurfaceSemanticSnapshot {
    pub(crate) command_bar: CommandBarSemanticMetadata,
    pub(crate) omnibar: OmnibarSemanticMetadata,
    pub(crate) command_palette: Option<PaletteSurfaceSemanticMetadata>,
    pub(crate) context_palette: Option<PaletteSurfaceSemanticMetadata>,
}

/// Consolidated command-surface telemetry sink.
///
/// Holds the "latest published snapshot" cell and the counter cell
/// widgets bump as they route commands and fetch omnibar suggestions.
/// Accessed via `CommandSurfaceTelemetry::global()`; all mutation flows
/// through named methods.
pub(crate) struct CommandSurfaceTelemetry {
    snapshot: Mutex<Option<CommandSurfaceSemanticSnapshot>>,
    events: Mutex<CommandSurfaceEventSequenceMetadata>,
    #[cfg(test)]
    test_lock: Mutex<()>,
}

static TELEMETRY: OnceLock<CommandSurfaceTelemetry> = OnceLock::new();

impl CommandSurfaceTelemetry {
    pub(crate) fn global() -> &'static CommandSurfaceTelemetry {
        TELEMETRY.get_or_init(|| CommandSurfaceTelemetry {
            snapshot: Mutex::new(None),
            events: Mutex::new(CommandSurfaceEventSequenceMetadata::default()),
            #[cfg(test)]
            test_lock: Mutex::new(()),
        })
    }

    pub(crate) fn latest_snapshot(&self) -> Option<CommandSurfaceSemanticSnapshot> {
        self.snapshot.lock().ok().and_then(|slot| slot.clone())
    }

    pub(crate) fn publish_snapshot(&self, snapshot: CommandSurfaceSemanticSnapshot) {
        if let Ok(mut slot) = self.snapshot.lock() {
            *slot = Some(snapshot);
        }
    }

    pub(crate) fn clear_snapshot(&self) {
        if let Ok(mut slot) = self.snapshot.lock() {
            *slot = None;
        }
        #[cfg(test)]
        self.clear_event_sequence_metadata();
    }

    pub(crate) fn latest_event_sequence_metadata(&self) -> CommandSurfaceEventSequenceMetadata {
        self.events
            .lock()
            .map(|state| *state)
            .unwrap_or_default()
    }

    fn update_events(&self, mutator: impl FnOnce(&mut CommandSurfaceEventSequenceMetadata)) {
        if let Ok(mut state) = self.events.lock() {
            mutator(&mut state);
        }
    }

    pub(crate) fn note_route_resolved(&self) {
        self.update_events(|state| {
            state.route_events.resolved = state.route_events.resolved.saturating_add(1);
        });
    }

    pub(crate) fn note_route_fallback(&self) {
        self.update_events(|state| {
            state.route_events.fallback = state.route_events.fallback.saturating_add(1);
        });
    }

    pub(crate) fn note_route_no_target(&self) {
        self.update_events(|state| {
            state.route_events.no_target = state.route_events.no_target.saturating_add(1);
        });
    }

    pub(crate) fn note_omnibar_mailbox_request_started(&self) {
        self.update_events(|state| {
            state.omnibar_mailbox_events.request_started = state
                .omnibar_mailbox_events
                .request_started
                .saturating_add(1);
        });
    }

    pub(crate) fn note_omnibar_mailbox_applied(&self) {
        self.update_events(|state| {
            state.omnibar_mailbox_events.applied =
                state.omnibar_mailbox_events.applied.saturating_add(1);
        });
    }

    pub(crate) fn note_omnibar_mailbox_failed(&self) {
        self.update_events(|state| {
            state.omnibar_mailbox_events.failed =
                state.omnibar_mailbox_events.failed.saturating_add(1);
        });
    }

    pub(crate) fn note_omnibar_mailbox_stale(&self) {
        self.update_events(|state| {
            state.omnibar_mailbox_events.stale =
                state.omnibar_mailbox_events.stale.saturating_add(1);
        });
    }

    #[cfg(test)]
    pub(crate) fn set_event_sequence_metadata_for_tests(
        &self,
        metadata: CommandSurfaceEventSequenceMetadata,
    ) {
        if let Ok(mut state) = self.events.lock() {
            *state = metadata;
        }
    }

    #[cfg(test)]
    pub(crate) fn clear_event_sequence_metadata(&self) {
        if let Ok(mut state) = self.events.lock() {
            *state = CommandSurfaceEventSequenceMetadata::default();
        }
    }

    /// Serialize test access to the telemetry singleton. Tests that
    /// publish a snapshot and read it back must hold this lock so
    /// concurrent test threads don't stomp each other's state.
    #[cfg(test)]
    pub(crate) fn lock_tests(&self) -> MutexGuard<'_, ()> {
        self.test_lock
            .lock()
            .expect("command-surface telemetry test mutex poisoned")
    }
}
