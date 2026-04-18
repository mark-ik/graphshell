/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Workbench-authority intent dispatch + focus-authority priming.
//!
//! Final big slice of M6 §4.1. This module owns the two-authority
//! boundary (graph reducer vs. workbench authority) — everything
//! touching the `UxDispatch*` event contract, modal-surface filtering,
//! focus-authority priming for each `WorkbenchIntent` variant, and the
//! planned-semantic-region rotation for `CycleFocusRegion`.
//!
//! Layout, from narrow to wide:
//!
//! - [`handle_tool_pane_intents`] — public entry point from
//!   `gui_orchestration`. Derives modal state and calls the interceptor.
//! - [`modal_surface_active`] / [`modal_allows_workbench_intent`] —
//!   modal-filter predicates collapsed from the former
//!   `_with_focus_authority` variants (M6 §4.1 cleanup).
//! - [`UxEventKind`], [`UxDispatchPhase`], [`UxDispatchControl`],
//!   [`UxDispatchPath`] — the diagnostic-channel vocabulary emitted
//!   during every intent's capture/target/bubble/default traversal.
//! - [`ux_event_kind_for_workbench_intent`],
//!   [`ux_dispatch_path_for_workbench_intent`] — per-intent classification.
//! - [`emit_dispatch_phase`] — the single per-phase channel emission.
//! - [`dispatch_workbench_authority_intent`] — thin wrapper around the
//!   registries' `dispatch_workbench_surface_intent`.
//! - [`prime_runtime_focus_authority_for_workbench_intent`] — the
//!   ~300-line focus-capture prologue that runs per-intent before the
//!   realizer/interceptor decides.
//! - [`refresh_runtime_focus_authority_after_workbench_intent`] /
//!   [`reconcile_focus_authority_after_realization`] — post-dispatch
//!   focus-state reconciliation.
//! - [`planned_semantic_workbench_region_from_focus_authority`] —
//!   rotation order for `CycleFocusRegion`.
//! - [`preferred_workbench_overlay_region`] /
//!   [`semantic_region_for_non_graph_pane`] — pane-to-region lookup
//!   helpers consumed by priming.
//! - [`restore_pending_transient_surface_focus`] /
//!   [`assert_workbench_intents_drained_before_reducer_apply`] — thin
//!   wrappers exposed so the interceptor + tests can reach them via a
//!   predictable path.
//!
//! The sibling [`super::workbench_intent_interceptor`] submodule
//! consumes most of these via `use super::workbench_dispatch_flow::*;`.

use std::collections::HashSet;

use egui_tiles::{Tile, Tree};

use crate::app::{GraphBrowserApp, WorkbenchIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_DISPATCH_PHASE, CHANNEL_UX_FOCUS_REALIZATION_MISMATCH,
    CHANNEL_UX_NAVIGATION_VIOLATION,
};
use crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState;
use crate::shell::desktop::workbench::pane_model::{PaneViewState, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;

// ---------------------------------------------------------------------------
// UX event taxonomy
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UxEventKind {
    PointerDown,
    PointerUp,
    PointerMove,
    PointerEnter,
    PointerLeave,
    KeyDown,
    KeyUp,
    Scroll,
    PinchZoom,
    FocusIn,
    FocusOut,
    Focus,
    Blur,
    Action,
    UxBridgeCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UxDispatchPhase {
    Capture = 1,
    Target = 2,
    Bubble = 3,
    Default = 4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct UxDispatchControl {
    pub(super) stop_propagation: bool,
    pub(super) stop_immediate_propagation: bool,
    pub(super) prevent_default: bool,
}

#[derive(Clone, Debug)]
pub(super) struct UxDispatchPath {
    pub(super) nodes: Vec<u64>,
}

impl UxDispatchPath {
    pub(super) fn is_valid(&self) -> bool {
        if self.nodes.len() < 2 {
            return false;
        }
        if self.nodes.first().copied() != Some(0) {
            return false;
        }
        let mut seen = HashSet::new();
        self.nodes.iter().all(|node| seen.insert(*node))
    }
}

pub(super) const UX_DISPATCH_NODE_ROOT: u64 = 0;
pub(super) const UX_DISPATCH_NODE_WORKBENCH: u64 = 1;
pub(super) const UX_DISPATCH_NODE_COMMAND_SURFACE: u64 = 2;
pub(super) const UX_DISPATCH_NODE_TOOL_SURFACE: u64 = 3;
pub(super) const UX_DISPATCH_NODE_GRAPH_SURFACE: u64 = 4;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Intercept workbench-authority intents before they reach `apply_intents()`.
///
/// ## Two-authority model
///
/// The architecture has two distinct mutation authorities:
///
/// - **Graph Reducer** (`apply_intents` in `app.rs`): authoritative for the graph
///   data model, node/edge lifecycle, WAL journal, and traversal history.
///   Always synchronous, always logged, always testable.
///
/// - **Workbench Authority** (this function + `tile_view_ops.rs`): authoritative
///   for tile-tree shape mutations (`egui_tiles` splits, tabs, pane open/close/
///   focus). The tile tree is a layout construct — not graph state — and must
///   not flow through the graph reducer or the WAL.
///
/// Intents tagged as workbench-authority (`OpenToolPane`, `SplitPane`,
/// `DetachNodeToSplit`, `SwapViewerBackend`, `SetPaneView`, `OpenNodeInPane`,
/// tool-surface toggles/settings URLs) must be drained here, before
/// `apply_intents` is called. Any that leak through will trip reducer
/// hardening (panic in debug/test, warning in release for non-layout
/// authority leaks).
///
/// Modal state is derived from `graph_app`; callers that need to override
/// it (e.g. simulating a clear-data-confirm dialog in tests) should call
/// [`super::workbench_intent_interceptor::handle_tool_pane_intents_with_modal_state_and_focus_authority`]
/// directly.
pub(crate) fn handle_tool_pane_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    workbench_intents: &mut Vec<WorkbenchIntent>,
) {
    super::workbench_intent_interceptor::handle_tool_pane_intents_with_modal_state_and_focus_authority(
        graph_app,
        tiles_tree,
        None,
        workbench_intents,
        modal_surface_active(graph_app, None),
        None,
    );
}

// ---------------------------------------------------------------------------
// Modal-filter predicates
// ---------------------------------------------------------------------------

/// Whether an overlay / modal surface is currently active (command
/// palette, help panel, radial menu, clear-data-confirm, etc.).
/// Collapsed from `modal_surface_active` + `modal_surface_active_with_focus_authority`
/// in M6 §4.1 — callers pass `None` for `focus_authority` when they
/// don't have one.
pub(super) fn modal_surface_active(
    graph_app: &GraphBrowserApp,
    focus_authority: Option<&RuntimeFocusAuthorityState>,
) -> bool {
    crate::shell::desktop::ui::gui::workspace_runtime_focus_state(
        graph_app,
        focus_authority,
        None,
        false,
    )
    .overlay_active()
}

/// Does the currently active modal allow the given workbench intent to
/// still run? Collapsed from the `_with_focus_authority` pair.
pub(super) fn modal_allows_workbench_intent(
    graph_app: &GraphBrowserApp,
    intent: &WorkbenchIntent,
    focus_authority: Option<&RuntimeFocusAuthorityState>,
) -> bool {
    let focus_state = crate::shell::desktop::ui::gui::workspace_runtime_focus_state(
        graph_app,
        focus_authority,
        None,
        false,
    );
    matches!(
        (intent, &focus_state.semantic_region),
        (
            WorkbenchIntent::CloseCommandPalette,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::CommandPalette
                | crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ContextPalette
        ) | (
            WorkbenchIntent::ToggleCommandPalette,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::CommandPalette
                | crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ContextPalette
        ) | (
            WorkbenchIntent::CloseHelpPanel,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::HelpPanel
        ) | (
            WorkbenchIntent::ToggleHelpPanel,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::HelpPanel
        ) | (
            WorkbenchIntent::CloseRadialMenu,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::RadialPalette
        ) | (
            WorkbenchIntent::ToggleRadialMenu,
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::RadialPalette
        )
    )
}

// ---------------------------------------------------------------------------
// Per-intent event + path classification
// ---------------------------------------------------------------------------

pub(super) fn ux_event_kind_for_workbench_intent(intent: &WorkbenchIntent) -> UxEventKind {
    match intent {
        WorkbenchIntent::CycleFocusRegion | WorkbenchIntent::FocusGraphView { .. } => {
            UxEventKind::FocusIn
        }
        _ => UxEventKind::Action,
    }
}

pub(super) fn ux_dispatch_path_for_workbench_intent(intent: &WorkbenchIntent) -> UxDispatchPath {
    let leaf = match intent {
        WorkbenchIntent::OpenCommandPalette
        | WorkbenchIntent::CloseCommandPalette
        | WorkbenchIntent::ToggleCommandPalette
        | WorkbenchIntent::CloseHelpPanel
        | WorkbenchIntent::ToggleHelpPanel
        | WorkbenchIntent::CloseRadialMenu
        | WorkbenchIntent::ToggleRadialMenu => UX_DISPATCH_NODE_COMMAND_SURFACE,
        WorkbenchIntent::OpenToolPane { .. }
        | WorkbenchIntent::SetWorkbenchOverlayVisible { .. }
        | WorkbenchIntent::SetWorkbenchDisplayMode { .. }
        | WorkbenchIntent::SetWorkbenchPinned { .. }
        | WorkbenchIntent::SetLayoutConstraintDraft { .. }
        | WorkbenchIntent::CommitLayoutConstraintDraft { .. }
        | WorkbenchIntent::DiscardLayoutConstraintDraft { .. }
        | WorkbenchIntent::SetNavigatorHostScope { .. }
        | WorkbenchIntent::SetFirstUsePolicy { .. }
        | WorkbenchIntent::SuppressFirstUsePromptForSession { .. }
        | WorkbenchIntent::DismissFrameSplitOfferForSession { .. }
        | WorkbenchIntent::RenameFrame { .. }
        | WorkbenchIntent::DeleteFrame { .. }
        | WorkbenchIntent::SaveFrameSnapshotNamed { .. }
        | WorkbenchIntent::SaveCurrentFrame
        | WorkbenchIntent::PruneEmptyFrames
        | WorkbenchIntent::RestoreFrame { .. }
        | WorkbenchIntent::ClosePane { .. }
        | WorkbenchIntent::DismissTile { .. }
        | WorkbenchIntent::CloseToolPane { .. }
        | WorkbenchIntent::OpenSettingsUrl { .. }
        | WorkbenchIntent::OpenFrameUrl { .. }
        | WorkbenchIntent::OpenToolUrl { .. }
        | WorkbenchIntent::OpenViewUrl { .. }
        | WorkbenchIntent::OpenGraphUrl { .. }
        | WorkbenchIntent::OpenGraphViewPane { .. }
        | WorkbenchIntent::FocusGraphView { .. }
        | WorkbenchIntent::OpenFrameAsSplit { .. }
        | WorkbenchIntent::SetFrameSplitOfferSuppressed { .. }
        | WorkbenchIntent::MoveFrameLayoutHint { .. }
        | WorkbenchIntent::RemoveFrameLayoutHint { .. }
        | WorkbenchIntent::SetNavigatorSpecialtyView { .. }
        | WorkbenchIntent::TransferSelectedNodesToGraphView { .. }
        | WorkbenchIntent::ToggleOverviewPlane
        | WorkbenchIntent::OpenNoteUrl { .. }
        | WorkbenchIntent::OpenNodeUrl { .. }
        | WorkbenchIntent::OpenClipUrl { .. }
        | WorkbenchIntent::SwapViewerBackend { .. }
        | WorkbenchIntent::SetPaneView { .. }
        | WorkbenchIntent::SetPanePresentationMode { .. }
        | WorkbenchIntent::PromoteEphemeralPane { .. }
        | WorkbenchIntent::SplitPane { .. }
        | WorkbenchIntent::ApplyLayoutConstraint { .. }
        | WorkbenchIntent::SetSurfaceConfigMode { .. }
        | WorkbenchIntent::DetachNodeToSplit { .. }
        | WorkbenchIntent::OpenNodeInPane { .. }
        | WorkbenchIntent::SelectNavigatorNode { .. }
        | WorkbenchIntent::ActivateNavigatorNode { .. }
        | WorkbenchIntent::DismissNavigatorNode { .. }
        | WorkbenchIntent::SwitchNavigatorNodeSurface { .. }
        | WorkbenchIntent::ReconcileGraphletTiles { .. }
        | WorkbenchIntent::RestorePaneToSemanticTabGroup { .. }
        | WorkbenchIntent::CollapseSemanticTabGroupToPaneRest { .. }
        | WorkbenchIntent::SelectPane { .. }
        | WorkbenchIntent::UpdatePaneSelection { .. }
        | WorkbenchIntent::ClearTileSelection
        | WorkbenchIntent::GroupSelectedTiles
        | WorkbenchIntent::CycleFocusRegion => UX_DISPATCH_NODE_TOOL_SURFACE,
    };

    UxDispatchPath {
        nodes: vec![UX_DISPATCH_NODE_ROOT, UX_DISPATCH_NODE_WORKBENCH, leaf],
    }
}

pub(super) fn emit_dispatch_phase(phase: UxDispatchPhase) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UX_DISPATCH_PHASE,
        byte_len: phase as usize,
    });
}

pub(super) fn dispatch_workbench_authority_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    intent: WorkbenchIntent,
) -> Option<WorkbenchIntent> {
    crate::shell::desktop::runtime::registries::dispatch_workbench_surface_intent(
        graph_app, tiles_tree, graph_tree, intent,
    )
}

// ---------------------------------------------------------------------------
// Focus-authority pre/post reconciliation
// ---------------------------------------------------------------------------

pub(super) fn refresh_runtime_focus_authority_after_workbench_intent(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    modal_surface_active: bool,
) {
    crate::shell::desktop::ui::gui::refresh_realized_runtime_focus_state(
        focus_authority,
        graph_app,
        tiles_tree,
        None,
        false,
    );
    let _ = modal_surface_active;
}

/// After the focus authority reducer and realizer have run for an authority-handled
/// intent, reconcile by syncing return targets and comparing desired vs observed
/// semantic region. Unlike `refresh_*`, this does NOT overwrite the authority's
/// `semantic_region` — the authority remains the source of truth. Mismatches
/// produce a `ux:focus_realization_mismatch` diagnostic.
pub(super) fn reconcile_focus_authority_after_realization(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    modal_surface_active: bool,
) {
    crate::shell::desktop::ui::gui::refresh_realized_runtime_focus_state(
        focus_authority,
        graph_app,
        tiles_tree,
        None,
        false,
    );
    let _ = modal_surface_active;

    let desired_focus = crate::shell::desktop::ui::gui::desired_runtime_focus_state(
        graph_app,
        focus_authority,
        None,
        false,
    );
    if focus_authority
        .realized_focus_state
        .as_ref()
        .is_some_and(|realized| desired_focus.semantic_region != realized.semantic_region)
    {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_FOCUS_REALIZATION_MISMATCH,
            byte_len: 1,
        });
    }
}

// ---------------------------------------------------------------------------
// Planned-semantic-region rotation for CycleFocusRegion
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlannedSemanticWorkbenchRegion {
    GraphSurface,
    NodePane,
    #[cfg(feature = "diagnostics")]
    ToolPane,
}

pub(super) fn planned_semantic_workbench_region_from_focus_authority(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    focus_authority: &RuntimeFocusAuthorityState,
) -> Option<crate::shell::desktop::ui::gui_state::SemanticRegionFocus> {
    let focus_state = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
        graph_app,
        tiles_tree,
        Some(focus_authority),
        None,
        false,
    );
    let current = match focus_state.semantic_region {
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::GraphSurface { .. } => {
            Some(PlannedSemanticWorkbenchRegion::GraphSurface)
        }
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::NodePane { .. } => {
            Some(PlannedSemanticWorkbenchRegion::NodePane)
        }
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ToolPane { .. } => {
            #[cfg(feature = "diagnostics")]
            {
                Some(PlannedSemanticWorkbenchRegion::ToolPane)
            }
            #[cfg(not(feature = "diagnostics"))]
            {
                None
            }
        }
        _ => None,
    };
    let order = [
        PlannedSemanticWorkbenchRegion::GraphSurface,
        PlannedSemanticWorkbenchRegion::NodePane,
        #[cfg(feature = "diagnostics")]
        PlannedSemanticWorkbenchRegion::ToolPane,
    ];
    let start_index = current
        .and_then(|region| order.iter().position(|candidate| *candidate == region))
        .unwrap_or(order.len() - 1);

    for offset in 1..=order.len() {
        let candidate = order[(start_index + offset) % order.len()];
        let resolved = match candidate {
            PlannedSemanticWorkbenchRegion::GraphSurface => {
                tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                    egui_tiles::Tile::Pane(TileKind::Graph(view_ref)) => Some(
                        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::GraphSurface {
                            view_id: Some(view_ref.graph_view_id),
                        },
                    ),
                    _ => None,
                })
            }
            PlannedSemanticWorkbenchRegion::NodePane => {
                tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                    egui_tiles::Tile::Pane(TileKind::Node(state)) => Some(
                        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::NodePane {
                            pane_id: Some(state.pane_id),
                            node_key: Some(state.node),
                        },
                    ),
                    _ => None,
                })
            }
            #[cfg(feature = "diagnostics")]
            PlannedSemanticWorkbenchRegion::ToolPane => {
                tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                    egui_tiles::Tile::Pane(TileKind::Tool(state)) => Some(
                        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ToolPane {
                            pane_id: Some(state.pane_id),
                        },
                    ),
                    _ => None,
                })
            }
        };
        if resolved.is_some() {
            return resolved;
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Pane → semantic-region lookup helpers
// ---------------------------------------------------------------------------

pub(super) fn preferred_workbench_overlay_region(
    focus_authority: &RuntimeFocusAuthorityState,
    tiles_tree: &Tree<TileKind>,
) -> Option<crate::shell::desktop::ui::gui_state::SemanticRegionFocus> {
    focus_authority
        .last_non_graph_pane_activation
        .and_then(|pane_id| semantic_region_for_non_graph_pane(tiles_tree, pane_id))
        .or_else(|| {
            focus_authority
                .pane_activation
                .and_then(|pane_id| semantic_region_for_non_graph_pane(tiles_tree, pane_id))
        })
}

pub(super) fn semantic_region_for_non_graph_pane(
    tiles_tree: &Tree<TileKind>,
    pane_id: crate::shell::desktop::workbench::pane_model::PaneId,
) -> Option<crate::shell::desktop::ui::gui_state::SemanticRegionFocus> {
    tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        Tile::Pane(TileKind::Pane(PaneViewState::Node(state))) if state.pane_id == pane_id => Some(
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::NodePane {
                pane_id: Some(pane_id),
                node_key: Some(state.node),
            },
        ),
        Tile::Pane(TileKind::Pane(PaneViewState::Tool(tool_ref)))
            if tool_ref.pane_id == pane_id =>
        {
            Some(
                crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ToolPane {
                    pane_id: Some(pane_id),
                },
            )
        }
        Tile::Pane(TileKind::Node(state)) if state.pane_id == pane_id => Some(
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::NodePane {
                pane_id: Some(pane_id),
                node_key: Some(state.node),
            },
        ),
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Tool(tool_ref)) if tool_ref.pane_id == pane_id => Some(
            crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ToolPane {
                pane_id: Some(pane_id),
            },
        ),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Thin wrappers used by the interceptor + tests
// ---------------------------------------------------------------------------

pub(super) fn restore_pending_transient_surface_focus(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    focus_authority: &mut RuntimeFocusAuthorityState,
) {
    super::workbench_intent_interceptor::restore_pending_transient_surface_focus(
        graph_app,
        tiles_tree,
        focus_authority,
    );
}

pub(super) fn assert_workbench_intents_drained_before_reducer_apply(intents: &[WorkbenchIntent]) {
    if intents.is_empty() {
        return;
    }

    #[cfg(any(test, debug_assertions))]
    panic!(
        "workbench intents leaked past workbench-authority interception before reducer apply: {:?}",
        intents
    );

    #[cfg(not(any(test, debug_assertions)))]
    {
        log::warn!(
            "workbench intents leaked past workbench-authority interception before reducer apply; dropping {} leaked intent(s)",
            intents.len()
        );
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
            byte_len: intents.len(),
        });
    }
}

// ---------------------------------------------------------------------------
// Focus-authority priming per WorkbenchIntent variant
// ---------------------------------------------------------------------------

pub(super) fn prime_runtime_focus_authority_for_workbench_intent(
    focus_authority: &mut RuntimeFocusAuthorityState,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    intent: &WorkbenchIntent,
) {
    match intent {
        WorkbenchIntent::OpenCommandPalette => {
            let contextual_mode = matches!(
                focus_authority.semantic_region,
                Some(crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ContextPalette)
            );
            let return_target = if focus_authority.command_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.command_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterCommandPalette {
                    contextual_mode,
                    return_target,
                },
            );
            crate::shell::desktop::ui::gui::capture_command_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::CloseCommandPalette => {
            crate::shell::desktop::ui::gui::seed_command_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitCommandPalette,
            );
        }
        WorkbenchIntent::ToggleCommandPalette
            if graph_app.workspace.chrome_ui.show_command_palette
                || graph_app.workspace.chrome_ui.show_context_palette =>
        {
            crate::shell::desktop::ui::gui::seed_command_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitCommandPalette,
            );
        }
        WorkbenchIntent::ToggleCommandPalette => {
            let contextual_mode = matches!(
                focus_authority.semantic_region,
                Some(crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ContextPalette)
            );
            let return_target = if focus_authority.command_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.command_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterCommandPalette {
                    contextual_mode,
                    return_target,
                },
            );
            crate::shell::desktop::ui::gui::capture_command_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::ToggleHelpPanel if graph_app.workspace.chrome_ui.show_help_panel => {
            crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitTransientSurface {
                    surface: crate::shell::desktop::ui::gui_state::FocusCaptureSurface::HelpPanel,
                    restore_target: focus_authority.transient_surface_return_target.clone(),
                },
            );
        }
        WorkbenchIntent::ToggleHelpPanel => {
            let return_target = if focus_authority.transient_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.transient_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterTransientSurface {
                    surface: crate::shell::desktop::ui::gui_state::FocusCaptureSurface::HelpPanel,
                    return_target,
                },
            );
        }
        WorkbenchIntent::CloseHelpPanel => {
            crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitTransientSurface {
                    surface: crate::shell::desktop::ui::gui_state::FocusCaptureSurface::HelpPanel,
                    restore_target: focus_authority.transient_surface_return_target.clone(),
                },
            );
        }
        WorkbenchIntent::ToggleRadialMenu if graph_app.workspace.chrome_ui.show_radial_menu => {
            crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitTransientSurface {
                    surface:
                        crate::shell::desktop::ui::gui_state::FocusCaptureSurface::RadialPalette,
                    restore_target: focus_authority.transient_surface_return_target.clone(),
                },
            );
        }
        WorkbenchIntent::ToggleRadialMenu => {
            let return_target = if focus_authority.transient_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.transient_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterTransientSurface {
                    surface:
                        crate::shell::desktop::ui::gui_state::FocusCaptureSurface::RadialPalette,
                    return_target,
                },
            );
        }
        WorkbenchIntent::CloseRadialMenu => {
            crate::shell::desktop::ui::gui::seed_transient_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitTransientSurface {
                    surface:
                        crate::shell::desktop::ui::gui_state::FocusCaptureSurface::RadialPalette,
                    restore_target: focus_authority.transient_surface_return_target.clone(),
                },
            );
        }
        WorkbenchIntent::CycleFocusRegion => {
            if let Some(region) = planned_semantic_workbench_region_from_focus_authority(
                graph_app,
                tiles_tree,
                focus_authority,
            ) {
                crate::shell::desktop::ui::gui::apply_focus_command(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCommand::SetSemanticRegion {
                        region,
                    },
                );
            }
        }
        WorkbenchIntent::SetWorkbenchOverlayVisible { visible: true } => {
            let return_target = if focus_authority.tool_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.tool_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterToolPane { return_target },
            );
            if let Some(region) = preferred_workbench_overlay_region(focus_authority, tiles_tree) {
                crate::shell::desktop::ui::gui::apply_focus_command(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCommand::SetSemanticRegion {
                        region,
                    },
                );
            }
            crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::SetWorkbenchOverlayVisible { visible: false } => {
            crate::shell::desktop::ui::gui::seed_tool_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitToolPane {
                    restore_target: focus_authority.tool_surface_return_target.clone(),
                },
            );
        }
        WorkbenchIntent::OpenToolPane { kind }
            if matches!(
                kind,
                ToolPaneState::Settings
                    | ToolPaneState::HistoryManager
                    | ToolPaneState::Diagnostics
            ) =>
        {
            let return_target = if focus_authority.tool_surface_return_target.is_none() {
                crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
            } else {
                focus_authority.tool_surface_return_target.clone()
            };
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::EnterToolPane { return_target },
            );
            crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::OpenSettingsUrl { url } => {
            if crate::shell::desktop::runtime::registries::workbench_surface::settings_url_targets_overlay(
                graph_app,
                tiles_tree,
                url,
            ) {
                let return_target = if focus_authority.transient_surface_return_target.is_none() {
                    crate::shell::desktop::runtime::registries::workbench_surface::active_tool_surface_return_target(tiles_tree)
                } else {
                    focus_authority.transient_surface_return_target.clone()
                };
                crate::shell::desktop::ui::gui::apply_focus_command(
                    focus_authority,
                    crate::shell::desktop::ui::gui_state::FocusCommand::EnterTransientSurface {
                        surface:
                            crate::shell::desktop::ui::gui_state::FocusCaptureSurface::SettingsOverlay,
                        return_target,
                    },
                );
            } else {
                crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                    focus_authority,
                    tiles_tree,
                );
            }
        }
        WorkbenchIntent::OpenClipUrl { .. } => {
            crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                focus_authority,
                tiles_tree,
            );
        }
        WorkbenchIntent::OpenToolUrl { url } => {
            if matches!(
                GraphBrowserApp::resolve_tool_route(url),
                Some(
                    ToolPaneState::Settings
                        | ToolPaneState::HistoryManager
                        | ToolPaneState::Diagnostics
                )
            ) {
                crate::shell::desktop::ui::gui::capture_tool_surface_return_target_in_authority(
                    focus_authority,
                    tiles_tree,
                );
            }
        }
        WorkbenchIntent::ClosePane {
            restore_previous_focus: true,
            ..
        }
        | WorkbenchIntent::CloseToolPane {
            restore_previous_focus: true,
            ..
        } => {
            crate::shell::desktop::ui::gui::apply_focus_command(
                focus_authority,
                crate::shell::desktop::ui::gui_state::FocusCommand::ExitToolPane {
                    restore_target: focus_authority.tool_surface_return_target.clone(),
                },
            );
            crate::shell::desktop::ui::gui::seed_tool_surface_return_target_from_authority(
                focus_authority,
                graph_app,
            );
        }
        _ => {}
    }
}
