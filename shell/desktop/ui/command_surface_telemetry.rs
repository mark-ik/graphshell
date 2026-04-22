/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command-surface telemetry sink.
//!
//! Consolidates the publish-latest-snapshot cell and the event sequence
//! counter cell that widgets emit into as the user interacts with the
//! command bar, omnibar, and command palette.
//!
//! M4 slice 6 (2026-04-22) migrated ownership onto `GraphshellRuntime`.
//! The `OnceLock<CommandSurfaceTelemetry>` crate-global is gone;
//! production call sites receive a `&CommandSurfaceTelemetry`
//! reference (either from the runtime via the phase pipeline, or
//! through the workbench snapshot builders' optional parameter).
//! Tests construct per-test instances via
//! [`CommandSurfaceTelemetry::new`] — each test's sink is naturally
//! isolated without the previous `test_lock: Mutex<()>` serialisation.
//!
//! The snapshot/metadata shapes exposed here
//! (`CommandSurfaceSemanticSnapshot`,
//! `CommandSurfaceEventSequenceMetadata`, etc.) depend on
//! shell-specific types (`PaneId`, `ToolSurfaceReturnTarget`), so the
//! whole module stays shell-side until those move to
//! `graphshell-core`. The `Mutex`-wrapped interior is still a
//! portability concern on `wasm32-unknown-unknown`, but that moves
//! with the rest of the module when it migrates.

use std::sync::Mutex;

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
/// Owned by `GraphshellRuntime` (M4 slice 6, 2026-04-22); production
/// call sites receive a `&CommandSurfaceTelemetry` reference rather
/// than reaching a crate-global singleton.
#[derive(Default)]
pub(crate) struct CommandSurfaceTelemetry {
    snapshot: Mutex<Option<CommandSurfaceSemanticSnapshot>>,
    events: Mutex<CommandSurfaceEventSequenceMetadata>,
}

impl CommandSurfaceTelemetry {
    pub(crate) fn new() -> Self {
        Self::default()
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
}
