use egui_tiles::{Tile, Tree};

use super::{
    CHANNEL_UX_NAVIGATION_TRANSITION, CHANNEL_UX_NAVIGATION_VIOLATION,
    CHANNEL_UX_OPEN_DECISION_PATH, CHANNEL_UX_OPEN_DECISION_REASON,
};
use crate::app::{
    GraphBrowserApp, PendingTileOpenMode, SelectionUpdateMode, ToolSurfaceReturnTarget,
    UndoBoundaryReason, WorkbenchIntent,
};
use crate::graph::NodeKey;
pub(crate) use crate::registries::domain::layout::workbench_surface::WorkbenchSurfaceResolution;
use crate::registries::domain::layout::workbench_surface::{
    FocusHandoffPolicy, WORKBENCH_SURFACE_COMPARE, WORKBENCH_SURFACE_DEFAULT,
    WORKBENCH_SURFACE_FOCUS, WorkbenchInteractionPolicy, WorkbenchLayoutPolicy, WorkbenchLock,
    WorkbenchSurfaceProfile, WorkbenchSurfaceRegistry as DomainWorkbenchSurfaceRegistry,
};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::ui::undo_boundary::record_workspace_undo_boundary_from_tiles_tree;
use crate::shell::desktop::workbench::pane_model::{
    PaneId, PanePresentationMode, PaneViewState, SplitDirection, ToolPaneState, ViewerId,
};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;
use crate::shell::desktop::workbench::tile_view_ops::{self, TileOpenMode};

#[path = "workbench_surface/focus_routing.rs"]
mod focus_routing;
#[path = "workbench_surface/pane_ops.rs"]
mod pane_ops;
#[path = "workbench_surface/route_ops.rs"]
mod route_ops;
#[path = "workbench_surface/selection_ops.rs"]
mod selection_ops;

pub(crate) const WORKBENCH_PROFILE_DEFAULT: &str = WORKBENCH_SURFACE_DEFAULT;
pub(crate) const WORKBENCH_PROFILE_FOCUS: &str = WORKBENCH_SURFACE_FOCUS;
pub(crate) const WORKBENCH_PROFILE_COMPARE: &str = WORKBENCH_SURFACE_COMPARE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SemanticWorkbenchRegion {
    GraphSurface,
    NodePane,
    #[cfg(feature = "diagnostics")]
    ToolPane,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkbenchSurfaceDescription {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) display_name: String,
    pub(crate) lock: WorkbenchLock,
}

fn semantic_workbench_region_from_focus_state(
    focus_state: &crate::shell::desktop::ui::gui_state::RuntimeFocusState,
) -> Option<SemanticWorkbenchRegion> {
    match focus_state.semantic_region {
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::GraphSurface { .. } => {
            Some(SemanticWorkbenchRegion::GraphSurface)
        }
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::NodePane { .. } => {
            Some(SemanticWorkbenchRegion::NodePane)
        }
        crate::shell::desktop::ui::gui_state::SemanticRegionFocus::ToolPane { .. } => {
            #[cfg(feature = "diagnostics")]
            {
                Some(SemanticWorkbenchRegion::ToolPane)
            }
            #[cfg(not(feature = "diagnostics"))]
            {
                None
            }
        }
        _ => None,
    }
}

fn semantic_workbench_region_is_present(
    tiles_tree: &Tree<TileKind>,
    region: SemanticWorkbenchRegion,
) -> bool {
    tiles_tree
        .tiles
        .iter()
        .any(|(_, tile)| match (region, tile) {
            (SemanticWorkbenchRegion::GraphSurface, Tile::Pane(TileKind::Graph(_))) => true,
            (SemanticWorkbenchRegion::NodePane, Tile::Pane(TileKind::Node(_))) => true,
            #[cfg(feature = "diagnostics")]
            (SemanticWorkbenchRegion::ToolPane, Tile::Pane(TileKind::Tool(_))) => true,
            _ => false,
        })
}

fn activate_semantic_workbench_region(
    tiles_tree: &mut Tree<TileKind>,
    region: SemanticWorkbenchRegion,
) -> bool {
    match region {
        SemanticWorkbenchRegion::GraphSurface => {
            tiles_tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Graph(_))))
        }
        SemanticWorkbenchRegion::NodePane => {
            tiles_tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Node(_))))
        }
        #[cfg(feature = "diagnostics")]
        SemanticWorkbenchRegion::ToolPane => {
            tiles_tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Tool(_))))
        }
    }
}

fn cycle_semantic_workbench_region(
    graph_app: &GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) -> bool {
    let focus_before = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
        graph_app, tiles_tree, None, None, false,
    );
    let current = semantic_workbench_region_from_focus_state(&focus_before);
    let order = [
        SemanticWorkbenchRegion::GraphSurface,
        SemanticWorkbenchRegion::NodePane,
        #[cfg(feature = "diagnostics")]
        SemanticWorkbenchRegion::ToolPane,
    ];
    let start_index = current
        .and_then(|region| order.iter().position(|candidate| *candidate == region))
        .unwrap_or(order.len() - 1);

    for offset in 1..=order.len() {
        let candidate = order[(start_index + offset) % order.len()];
        if !semantic_workbench_region_is_present(tiles_tree, candidate) {
            continue;
        }
        if activate_semantic_workbench_region(tiles_tree, candidate) {
            return true;
        }
    }

    false
}

pub(crate) struct WorkbenchSurfaceRegistry {
    profiles: DomainWorkbenchSurfaceRegistry,
    active_profile_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UxOpenDecisionPath {
    SettingsUrl = 1,
    FrameUrl = 2,
    ToolUrl = 3,
    ViewUrl = 4,
    GraphUrl = 5,
    NoteUrl = 6,
    NodeUrl = 7,
    ClipUrl = 8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UxOpenDecisionReason {
    Routed = 1,
    UnresolvedRoute = 2,
    TargetMissing = 3,
}

impl Default for WorkbenchSurfaceRegistry {
    fn default() -> Self {
        Self {
            profiles: DomainWorkbenchSurfaceRegistry::default(),
            active_profile_id: WORKBENCH_PROFILE_DEFAULT.to_string(),
        }
    }
}

impl WorkbenchSurfaceRegistry {
    pub(crate) fn active_profile_id(&self) -> &str {
        &self.active_profile_id
    }

    pub(crate) fn set_active_profile(&mut self, profile_id: &str) -> WorkbenchSurfaceResolution {
        let resolution = self.profiles.resolve(profile_id);
        self.active_profile_id = resolution.resolved_id.clone();
        resolution
    }

    pub(crate) fn active_profile(&self) -> WorkbenchSurfaceResolution {
        self.profiles.resolve(&self.active_profile_id)
    }

    pub(crate) fn resolve_layout_policy(&self) -> WorkbenchLayoutPolicy {
        self.active_profile().profile.layout
    }

    pub(crate) fn resolve_interaction_policy(&self) -> WorkbenchInteractionPolicy {
        self.active_profile().profile.interaction
    }

    pub(crate) fn resolve_focus_handoff_policy(&self) -> FocusHandoffPolicy {
        self.active_profile().profile.focus_handoff
    }

    pub(crate) fn resolve_profile(&self, profile_id: Option<&str>) -> WorkbenchSurfaceResolution {
        match profile_id {
            Some(profile_id) => self.profiles.resolve(profile_id),
            None => self.active_profile(),
        }
    }

    pub(crate) fn describe_surface(&self, profile_id: Option<&str>) -> WorkbenchSurfaceDescription {
        let resolution = self.resolve_profile(profile_id);
        WorkbenchSurfaceDescription {
            requested_id: resolution.requested_id,
            resolved_id: resolution.resolved_id,
            matched: resolution.matched,
            fallback_used: resolution.fallback_used,
            display_name: resolution.profile.display_name,
            lock: resolution.profile.lock,
        }
    }

    pub(crate) fn active_lock(&self) -> WorkbenchLock {
        self.active_profile().profile.lock
    }

    pub(crate) fn active_profile_snapshot(&self) -> WorkbenchSurfaceProfile {
        self.active_profile().profile
    }

    pub(crate) fn can_mutate(lock: WorkbenchLock, intent: &WorkbenchIntent) -> bool {
        match lock {
            WorkbenchLock::None => true,
            WorkbenchLock::PreventSplit => !matches!(
                intent,
                WorkbenchIntent::SplitPane { .. } | WorkbenchIntent::DetachNodeToSplit { .. }
            ),
            WorkbenchLock::PreventClose => !matches!(
                intent,
                WorkbenchIntent::ClosePane { .. } | WorkbenchIntent::CloseToolPane { .. }
            ),
            WorkbenchLock::FullLock => !matches!(
                intent,
                WorkbenchIntent::SplitPane { .. }
                    | WorkbenchIntent::DetachNodeToSplit { .. }
                    | WorkbenchIntent::ClosePane { .. }
                    | WorkbenchIntent::CloseToolPane { .. }
                    | WorkbenchIntent::OpenToolPane { .. }
                    | WorkbenchIntent::SetPanePresentationMode { .. }
                    | WorkbenchIntent::PromoteEphemeralPane { .. }
                    | WorkbenchIntent::SetPaneView { .. }
                    | WorkbenchIntent::OpenGraphViewPane { .. }
                    | WorkbenchIntent::OpenNodeInPane { .. }
            ),
        }
    }

    pub(crate) fn dispatch_intent(
        &self,
        graph_app: &mut GraphBrowserApp,
        tiles_tree: &mut Tree<TileKind>,
        intent: WorkbenchIntent,
    ) -> Option<WorkbenchIntent> {
        if !Self::can_mutate(self.active_lock(), &intent) {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
                byte_len: 1,
            });
            return None;
        }

        let interaction_policy = self.resolve_interaction_policy();
        let focus_handoff_policy = self.resolve_focus_handoff_policy();

        match intent {
            WorkbenchIntent::OpenCommandPalette => {
                handle_open_command_palette_intent(graph_app, tiles_tree);
                None
            }
            WorkbenchIntent::ToggleCommandPalette => {
                handle_toggle_command_palette_intent(graph_app, tiles_tree, &focus_handoff_policy);
                None
            }
            WorkbenchIntent::ToggleHelpPanel => {
                graph_app.toggle_help_panel();
                None
            }
            WorkbenchIntent::ToggleRadialMenu => {
                graph_app.toggle_radial_menu();
                None
            }
            WorkbenchIntent::CycleFocusRegion => {
                if handle_cycle_focus_region_intent(
                    graph_app,
                    tiles_tree,
                    interaction_policy.keyboard_focus_cycle,
                ) {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                } else {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
                        byte_len: 1,
                    });
                }
                None
            }
            WorkbenchIntent::SelectTile { tile_id } => {
                graph_app.select_workbench_tile(tile_id);
                None
            }
            WorkbenchIntent::UpdateTileSelection { tile_id, mode } => {
                handle_update_tile_selection_intent(graph_app, tiles_tree, tile_id, mode);
                None
            }
            WorkbenchIntent::ClearTileSelection => {
                graph_app.clear_workbench_tile_selection();
                None
            }
            WorkbenchIntent::GroupSelectedTiles => {
                if handle_group_selected_tiles_intent(graph_app, tiles_tree) {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                } else {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
                        byte_len: 1,
                    });
                }
                None
            }
            WorkbenchIntent::OpenToolPane { kind } => {
                handle_open_tool_pane_intent(graph_app, tiles_tree, kind);
                None
            }
            WorkbenchIntent::ClosePane {
                pane,
                restore_previous_focus,
            } => {
                handle_close_pane_intent(
                    graph_app,
                    tiles_tree,
                    pane,
                    restore_previous_focus,
                    &focus_handoff_policy,
                );
                None
            }
            WorkbenchIntent::CloseToolPane {
                kind,
                restore_previous_focus,
            } => {
                handle_close_tool_pane_intent(
                    graph_app,
                    tiles_tree,
                    kind,
                    restore_previous_focus,
                    &focus_handoff_policy,
                );
                None
            }
            WorkbenchIntent::OpenSettingsUrl { url } => {
                handle_open_settings_url_intent(graph_app, tiles_tree, url)
            }
            WorkbenchIntent::OpenFrameUrl { url } => handle_open_frame_url_intent(graph_app, url),
            WorkbenchIntent::OpenToolUrl { url } => {
                handle_open_tool_url_intent(graph_app, tiles_tree, url)
            }
            WorkbenchIntent::OpenViewUrl { url } => {
                handle_open_view_url_intent(graph_app, tiles_tree, url)
            }
            WorkbenchIntent::OpenGraphUrl { url } => handle_open_graph_url_intent(graph_app, url),
            WorkbenchIntent::OpenGraphViewPane { view_id, mode } => {
                handle_open_graph_view_pane_intent(tiles_tree, view_id, mode);
                None
            }
            WorkbenchIntent::OpenNoteUrl { url } => handle_open_note_url_intent(graph_app, url),
            WorkbenchIntent::OpenNodeUrl { url } => {
                handle_open_node_url_intent(graph_app, tiles_tree, url)
            }
            WorkbenchIntent::OpenClipUrl { url } => {
                handle_open_clip_url_intent(graph_app, tiles_tree, url)
            }
            WorkbenchIntent::OpenNodeInPane { node, pane } => {
                handle_open_node_in_pane_intent(graph_app, tiles_tree, node, pane);
                None
            }
            WorkbenchIntent::SetPanePresentationMode { pane, mode } => {
                handle_set_pane_presentation_mode_intent(tiles_tree, pane, mode);
                None
            }
            WorkbenchIntent::PromoteEphemeralPane {
                target_tile_context,
            } => {
                handle_promote_ephemeral_pane_intent(graph_app, tiles_tree, target_tile_context);
                None
            }
            WorkbenchIntent::SwapViewerBackend {
                pane,
                node,
                viewer_id_override,
            } => {
                handle_swap_viewer_backend_intent(
                    graph_app,
                    tiles_tree,
                    pane,
                    node,
                    viewer_id_override,
                );
                None
            }
            WorkbenchIntent::SetPaneView { pane, view } => {
                handle_set_pane_view_intent(graph_app, tiles_tree, pane, view);
                None
            }
            WorkbenchIntent::SplitPane {
                source_pane,
                direction,
            } => {
                handle_split_pane_intent(tiles_tree, source_pane, direction);
                None
            }
            WorkbenchIntent::DetachNodeToSplit { key } => {
                handle_detach_node_to_split_intent(graph_app, tiles_tree, key);
                None
            }
        }
    }
}

fn handle_cycle_focus_region_intent(
    graph_app: &GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    focus_cycle: crate::registries::domain::layout::workbench_surface::FocusCycle,
) -> bool {
    focus_routing::handle_cycle_focus_region_intent(graph_app, tiles_tree, focus_cycle)
}

fn handle_update_tile_selection_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    tile_id: egui_tiles::TileId,
    mode: SelectionUpdateMode,
) {
    selection_ops::handle_update_tile_selection_intent(graph_app, tiles_tree, tile_id, mode);
}

fn handle_group_selected_tiles_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) -> bool {
    selection_ops::handle_group_selected_tiles_intent(graph_app, tiles_tree)
}

fn handle_detach_node_to_split_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    key: NodeKey,
) {
    selection_ops::handle_detach_node_to_split_intent(graph_app, tiles_tree, key);
}

pub(crate) fn active_tool_surface_return_target(
    tiles_tree: &Tree<TileKind>,
) -> Option<ToolSurfaceReturnTarget> {
    focus_routing::active_tool_surface_return_target(tiles_tree)
}

pub(crate) fn focus_tool_surface_return_target(
    tiles_tree: &mut Tree<TileKind>,
    target: ToolSurfaceReturnTarget,
) -> bool {
    focus_routing::focus_tool_surface_return_target(tiles_tree, target)
}

fn maybe_capture_tool_surface_return_target(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    let active_target = active_tool_surface_return_target(tiles_tree);
    let active_is_control_surface = matches!(
        active_target,
        Some(ToolSurfaceReturnTarget::Tool(ToolPaneState::Settings))
            | Some(ToolSurfaceReturnTarget::Tool(ToolPaneState::HistoryManager))
    );
    if !active_is_control_surface {
        graph_app.set_pending_tool_surface_return_target(active_target);
    }
}

fn maybe_capture_transient_surface_return_target(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    if graph_app
        .pending_transient_surface_return_target()
        .is_none()
    {
        graph_app.set_pending_transient_surface_return_target(active_tool_surface_return_target(
            tiles_tree,
        ));
    }
}

fn maybe_capture_command_surface_return_target(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    if graph_app.pending_command_surface_return_target().is_none() {
        graph_app.set_pending_command_surface_return_target(active_tool_surface_return_target(
            tiles_tree,
        ));
    }
}

fn restore_command_surface_return_target_or_ensure_active_tile(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    focus_handoff: &FocusHandoffPolicy,
) -> bool {
    let target = graph_app.take_pending_command_surface_return_target();
    restore_focus_target_or_ensure_active_tile(
        graph_app,
        tiles_tree,
        target,
        !matches!(
            focus_handoff.pane_to_canvas_trigger,
            crate::registries::domain::layout::workbench_surface::FocusTrigger::Click
        ),
    )
}

fn handle_open_command_palette_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    if !graph_app.workspace.show_command_palette && !graph_app.workspace.show_context_palette {
        maybe_capture_command_surface_return_target(graph_app, tiles_tree);
    }
    graph_app.open_command_palette();
}

fn handle_toggle_command_palette_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    focus_handoff: &FocusHandoffPolicy,
) {
    if graph_app.workspace.show_command_palette || graph_app.workspace.show_context_palette {
        graph_app.toggle_command_palette();
        let _ = restore_command_surface_return_target_or_ensure_active_tile(
            graph_app,
            tiles_tree,
            focus_handoff,
        );
    } else {
        maybe_capture_command_surface_return_target(graph_app, tiles_tree);
        graph_app.toggle_command_palette();
    }
}

fn handle_open_tool_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    kind: ToolPaneState,
) {
    pane_ops::handle_open_tool_pane_intent(graph_app, tiles_tree, kind);
}

fn handle_set_pane_presentation_mode_intent(
    tiles_tree: &mut Tree<TileKind>,
    pane: PaneId,
    mode: PanePresentationMode,
) {
    for tile in tiles_tree.tiles.iter_mut().filter_map(|(_, tile)| match tile {
        Tile::Pane(kind) => Some(kind),
        _ => None,
    }) {
        if tile.pane_id() == pane {
            match tile {
                TileKind::Pane(view) => view.set_presentation_mode(mode),
                TileKind::Graph(graph_ref) => graph_ref.presentation_mode = mode,
                TileKind::Node(node_state) => node_state.presentation_mode = mode,
                #[cfg(feature = "diagnostics")]
                TileKind::Tool(tool_ref) => tool_ref.presentation_mode = mode,
            }
            break;
        }
    }
}

fn handle_close_tool_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    kind: ToolPaneState,
    restore_previous_focus: bool,
    focus_handoff: &FocusHandoffPolicy,
) {
    pane_ops::handle_close_tool_pane_intent(
        graph_app,
        tiles_tree,
        kind,
        restore_previous_focus,
        focus_handoff,
    );
}

fn handle_close_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    pane: PaneId,
    restore_previous_focus: bool,
    focus_handoff: &FocusHandoffPolicy,
) {
    pane_ops::handle_close_pane_intent(
        graph_app,
        tiles_tree,
        pane,
        restore_previous_focus,
        focus_handoff,
    );
}

fn restore_tool_surface_focus_or_ensure_active_tile(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    focus_handoff: &FocusHandoffPolicy,
) -> bool {
    let target = graph_app.take_pending_tool_surface_return_target();
    restore_focus_target_or_ensure_active_tile(
        graph_app,
        tiles_tree,
        target,
        !matches!(
            focus_handoff.pane_to_canvas_trigger,
            crate::registries::domain::layout::workbench_surface::FocusTrigger::Click
        ),
    )
}

pub(crate) fn restore_focus_target_or_ensure_active_tile(
    graph_app: &GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    target: Option<ToolSurfaceReturnTarget>,
    allow_ensure_active_tile: bool,
) -> bool {
    focus_routing::restore_focus_target_or_ensure_active_tile(
        graph_app,
        tiles_tree,
        target,
        allow_ensure_active_tile,
    )
}

fn handle_open_settings_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    route_ops::handle_open_settings_url_intent(graph_app, tiles_tree, url)
}

fn handle_open_frame_url_intent(
    graph_app: &mut GraphBrowserApp,
    url: String,
) -> Option<WorkbenchIntent> {
    route_ops::handle_open_frame_url_intent(graph_app, url)
}

fn handle_open_tool_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    route_ops::handle_open_tool_url_intent(graph_app, tiles_tree, url)
}

fn handle_open_view_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    route_ops::handle_open_view_url_intent(graph_app, tiles_tree, url)
}

fn handle_open_graph_url_intent(
    graph_app: &mut GraphBrowserApp,
    url: String,
) -> Option<WorkbenchIntent> {
    route_ops::handle_open_graph_url_intent(graph_app, url)
}

fn handle_open_note_url_intent(
    graph_app: &mut GraphBrowserApp,
    url: String,
) -> Option<WorkbenchIntent> {
    route_ops::handle_open_note_url_intent(graph_app, url)
}

fn handle_open_node_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    route_ops::handle_open_node_url_intent(graph_app, tiles_tree, url)
}

fn handle_open_clip_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    route_ops::handle_open_clip_url_intent(graph_app, tiles_tree, url)
}

fn handle_open_graph_view_pane_intent(
    tiles_tree: &mut Tree<TileKind>,
    view_id: crate::app::GraphViewId,
    mode: PendingTileOpenMode,
) {
    let tile_mode = match mode {
        PendingTileOpenMode::Tab => TileOpenMode::Tab,
        PendingTileOpenMode::SplitHorizontal => TileOpenMode::SplitHorizontal,
        PendingTileOpenMode::QuarterPane => TileOpenMode::QuarterPane,
        PendingTileOpenMode::HalfPane => TileOpenMode::HalfPane,
    };
    tile_view_ops::open_or_focus_graph_pane_with_mode(tiles_tree, view_id, tile_mode);
}

fn handle_promote_ephemeral_pane_intent(
    graph_app: &GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    target_tile_context: crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext,
) {
    let mode = match target_tile_context {
        crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext::Split => {
            TileOpenMode::SplitHorizontal
        }
        crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext::TabGroup
        | crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext::BareGraph => {
            TileOpenMode::Tab
        }
    };
    let _ = tile_view_ops::promote_floating_node_pane(tiles_tree, graph_app, mode);
}

fn open_settings_route_target(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    route: crate::app::SettingsRouteTarget,
) {
    match route {
        crate::app::SettingsRouteTarget::History => {
            open_or_focus_tool_pane_if_available(tiles_tree, ToolPaneState::HistoryManager);
        }
        crate::app::SettingsRouteTarget::Settings(page) => {
            graph_app.workspace.settings_tool_page = page;
            open_or_focus_tool_pane_if_available(tiles_tree, ToolPaneState::Settings);
        }
    }
}

#[cfg(feature = "diagnostics")]
fn open_or_focus_tool_pane_if_available(tiles_tree: &mut Tree<TileKind>, kind: ToolPaneState) {
    tile_view_ops::open_or_focus_tool_pane(tiles_tree, kind);
}

#[cfg(not(feature = "diagnostics"))]
fn open_or_focus_tool_pane_if_available(_tiles_tree: &mut Tree<TileKind>, _kind: ToolPaneState) {}

fn handle_open_node_in_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    node: NodeKey,
    pane: PaneId,
) {
    log::debug!(
        "workbench intent OpenNodeInPane ignored pane target {}; opening node pane directly",
        pane
    );
    pane_ops::handle_open_node_in_pane_intent(graph_app, tiles_tree, node, pane);
}

fn handle_set_pane_view_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    pane: PaneId,
    view: PaneViewState,
) {
    pane_ops::handle_set_pane_view_intent(graph_app, tiles_tree, pane, view);
}

fn handle_swap_viewer_backend_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    pane: PaneId,
    node: NodeKey,
    viewer_id_override: Option<ViewerId>,
) {
    pane_ops::handle_swap_viewer_backend_intent(
        graph_app,
        tiles_tree,
        pane,
        node,
        viewer_id_override,
    );
}

fn handle_split_pane_intent(
    tiles_tree: &mut Tree<TileKind>,
    source_pane: PaneId,
    direction: SplitDirection,
) {
    pane_ops::handle_split_pane_intent(tiles_tree, source_pane, direction);
}

fn emit_open_decision(path: UxOpenDecisionPath, reason: UxOpenDecisionReason) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UX_OPEN_DECISION_PATH,
        byte_len: path as usize,
    });
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UX_OPEN_DECISION_REASON,
        byte_len: reason as usize,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphViewId;
    use crate::graph::{EdgeType, NodeKey};
    use crate::shell::desktop::workbench::pane_model::{
        GraphPaneRef, NodePaneState, PaneId, SplitDirection,
    };
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use crate::util::VersoAddress;
    use egui_tiles::{Tile, Tiles, Tree};
    use euclid::default::Point2D;

    #[test]
    fn registry_defaults_to_default_profile() {
        let registry = WorkbenchSurfaceRegistry::default();

        assert_eq!(registry.active_profile_id(), WORKBENCH_PROFILE_DEFAULT);
        assert_eq!(
            registry.resolve_layout_policy().default_split_direction,
            crate::registries::domain::layout::workbench_surface::SplitDirection::Horizontal
        );
    }

    #[test]
    fn registry_switches_profiles_with_fallback() {
        let mut registry = WorkbenchSurfaceRegistry::default();

        let compare = registry.set_active_profile(WORKBENCH_PROFILE_COMPARE);
        assert!(compare.matched);
        assert_eq!(registry.active_profile_id(), WORKBENCH_PROFILE_COMPARE);

        let fallback = registry.set_active_profile("workbench_surface:missing");
        assert!(fallback.fallback_used);
        assert_eq!(registry.active_profile_id(), WORKBENCH_PROFILE_DEFAULT);
    }

    #[test]
    fn describe_surface_reports_resolution_metadata() {
        let registry = WorkbenchSurfaceRegistry::default();

        let description = registry.describe_surface(Some(WORKBENCH_PROFILE_FOCUS));
        assert_eq!(description.display_name, "Focus");
        assert_eq!(description.lock, WorkbenchLock::PreventSplit);
        assert!(description.matched);
    }

    #[test]
    fn mutation_guard_respects_lock_modes() {
        let split = WorkbenchIntent::SplitPane {
            source_pane: PaneId::new(),
            direction: SplitDirection::Horizontal,
        };
        let close = WorkbenchIntent::ClosePane {
            pane: PaneId::new(),
            restore_previous_focus: true,
        };

        assert!(!WorkbenchSurfaceRegistry::can_mutate(
            WorkbenchLock::PreventSplit,
            &split
        ));
        assert!(WorkbenchSurfaceRegistry::can_mutate(
            WorkbenchLock::PreventSplit,
            &close
        ));
        assert!(!WorkbenchSurfaceRegistry::can_mutate(
            WorkbenchLock::PreventClose,
            &close
        ));
        assert!(!WorkbenchSurfaceRegistry::can_mutate(
            WorkbenchLock::FullLock,
            &split
        ));
    }

    #[test]
    fn dispatch_blocks_split_when_active_profile_prevents_split() {
        let mut registry = WorkbenchSurfaceRegistry::default();
        registry.set_active_profile(WORKBENCH_PROFILE_FOCUS);

        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("focus_lock", graph, tiles);
        let source_pane = match tree.tiles.get(graph) {
            Some(Tile::Pane(TileKind::Graph(graph_ref))) => graph_ref.pane_id,
            other => panic!("expected graph pane, got {other:?}"),
        };

        let result = registry.dispatch_intent(
            &mut app,
            &mut tree,
            WorkbenchIntent::SplitPane {
                source_pane,
                direction: SplitDirection::Horizontal,
            },
        );

        assert!(result.is_none());
        let graph_pane_count = tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))))
            .count();
        assert_eq!(graph_pane_count, 1);
    }

    #[test]
    fn dispatch_cycles_tabs_when_profile_requests_tab_focus_cycle() {
        let mut registry = WorkbenchSurfaceRegistry::default();
        registry.set_active_profile(WORKBENCH_PROFILE_FOCUS);

        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let graph_a = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let graph_b = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let root = tiles.insert_tab_tile(vec![graph_a, graph_b]);
        if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) = tiles.get_mut(root) {
            tabs.set_active(graph_a);
        }
        let mut tree = Tree::new("focus_tabs", root, tiles);

        let result =
            registry.dispatch_intent(&mut app, &mut tree, WorkbenchIntent::CycleFocusRegion);

        assert!(result.is_none());
        assert!(
            tree.active_tiles()
                .into_iter()
                .any(|tile_id| tile_id == graph_b)
        );
    }

    #[test]
    fn dispatch_cycles_focus_between_pane_kinds_when_profile_requests_pane_cycle() {
        let mut registry = WorkbenchSurfaceRegistry::default();
        registry.set_active_profile(WORKBENCH_PROFILE_COMPARE);

        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(NodeKey::new(42))));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) = tiles.get_mut(root) {
            tabs.set_active(graph);
        }
        let mut tree = Tree::new("compare_panes", root, tiles);

        let result =
            registry.dispatch_intent(&mut app, &mut tree, WorkbenchIntent::CycleFocusRegion);

        assert!(result.is_none());
        assert!(
            tree.active_tiles()
                .into_iter()
                .any(|tile_id| tile_id == node)
        );
    }

    #[test]
    fn update_tile_selection_intent_tracks_selected_tiles_independent_of_active_focus() {
        let registry = WorkbenchSurfaceRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(NodeKey::new(7))));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) = tiles.get_mut(root) {
            tabs.set_active(graph);
        }
        let mut tree = Tree::new("workbench_tile_selection", root, tiles);

        registry.dispatch_intent(
            &mut app,
            &mut tree,
            WorkbenchIntent::UpdateTileSelection {
                tile_id: graph,
                mode: SelectionUpdateMode::Replace,
            },
        );
        registry.dispatch_intent(
            &mut app,
            &mut tree,
            WorkbenchIntent::UpdateTileSelection {
                tile_id: node,
                mode: SelectionUpdateMode::Add,
            },
        );

        assert_eq!(
            app.workbench_tile_selection().selected_tile_ids,
            std::collections::HashSet::from([graph, node])
        );
        assert_eq!(app.workbench_tile_selection().primary_tile_id, Some(node));
        assert!(
            tree.active_tiles()
                .into_iter()
                .any(|tile_id| tile_id == graph),
            "selection should not replace active focus"
        );
    }

    #[test]
    fn group_selected_tiles_moves_original_tiles_into_new_group() {
        let registry = WorkbenchSurfaceRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(NodeKey::new(11))));
        let graph_leaf = tiles.insert_tab_tile(vec![graph]);
        let node_leaf = tiles.insert_tab_tile(vec![node]);
        let root = tiles.insert_horizontal_tile(vec![graph_leaf, node_leaf]);
        let mut tree = Tree::new("group_selected_tiles", root, tiles);

        app.select_workbench_tile(graph);
        app.update_workbench_tile_selection(node, SelectionUpdateMode::Add);

        registry.dispatch_intent(&mut app, &mut tree, WorkbenchIntent::GroupSelectedTiles);

        let selected = &app.workbench_tile_selection().selected_tile_ids;
        assert_eq!(selected.len(), 2);
        assert_eq!(*selected, std::collections::HashSet::from([graph, node]));
        assert_eq!(app.workbench_tile_selection().primary_tile_id, Some(graph));
        let graph_count = tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))))
            .count();
        let node_count = tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(_))))
            .count();
        assert_eq!(graph_count, 1);
        assert_eq!(node_count, 1);
    }

    #[test]
    fn group_selected_tiles_persists_tile_group_node_and_member_edges() {
        let registry = WorkbenchSurfaceRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let left = app.add_node_and_sync("https://left.example".into(), Point2D::new(0.0, 0.0));
        let right = app.add_node_and_sync("https://right.example".into(), Point2D::new(40.0, 0.0));
        let mut tiles = Tiles::default();
        let left_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left)));
        let right_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right)));
        let left_leaf = tiles.insert_tab_tile(vec![left_tile]);
        let right_leaf = tiles.insert_tab_tile(vec![right_tile]);
        let root = tiles.insert_horizontal_tile(vec![left_leaf, right_leaf]);
        let mut tree = Tree::new("persist_tile_group", root, tiles);

        app.select_workbench_tile(left_tile);
        app.update_workbench_tile_selection(right_tile, SelectionUpdateMode::Add);

        registry.dispatch_intent(&mut app, &mut tree, WorkbenchIntent::GroupSelectedTiles);

        let tile_group_nodes: Vec<_> = app
            .domain_graph()
            .nodes()
            .filter(|(_, node)| {
                matches!(
                    VersoAddress::parse(&node.url),
                    Some(VersoAddress::TileGroup(_))
                )
            })
            .collect();
        assert_eq!(tile_group_nodes.len(), 1);
        let group_key = tile_group_nodes[0].0;
        let group_node = tile_group_nodes[0].1;
        assert_eq!(group_node.title, "Tile Group");
        assert!(app.domain_graph().edges().any(|edge| {
            edge.edge_type == EdgeType::ArrangementRelation(
                crate::graph::ArrangementSubKind::TileGroup,
            ) && edge.from == group_key
                && edge.to == left
        }));
        assert!(app.domain_graph().edges().any(|edge| {
            edge.edge_type == EdgeType::ArrangementRelation(
                crate::graph::ArrangementSubKind::TileGroup,
            ) && edge.from == group_key
                && edge.to == right
        }));
    }

    #[test]
    fn group_selected_tiles_ensures_graph_view_member_node_identity() {
        let registry = WorkbenchSurfaceRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key =
            app.add_node_and_sync("https://member.example".into(), Point2D::new(20.0, 0.0));
        let view_id = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let graph_leaf = tiles.insert_tab_tile(vec![graph_tile]);
        let node_leaf = tiles.insert_tab_tile(vec![node_tile]);
        let root = tiles.insert_horizontal_tile(vec![graph_leaf, node_leaf]);
        let mut tree = Tree::new("persist_graph_member", root, tiles);

        app.select_workbench_tile(graph_tile);
        app.update_workbench_tile_selection(node_tile, SelectionUpdateMode::Add);

        registry.dispatch_intent(&mut app, &mut tree, WorkbenchIntent::GroupSelectedTiles);

        let view_url = VersoAddress::view(view_id.as_uuid().to_string()).to_string();
        let (view_member_key, view_member_node) = app
            .domain_graph()
            .get_node_by_url(&view_url)
            .expect("graph view member node should be created");
        assert_eq!(view_member_node.title, "Graph View");

        let (group_key, _) = app
            .domain_graph()
            .nodes()
            .find(|(_, node)| {
                matches!(
                    VersoAddress::parse(&node.url),
                    Some(VersoAddress::TileGroup(_))
                )
            })
            .expect("tile group node should be created");
        assert!(app.domain_graph().edges().any(|edge| {
            edge.edge_type == EdgeType::ArrangementRelation(
                crate::graph::ArrangementSubKind::TileGroup,
            ) && edge.from == group_key
                && edge.to == view_member_key
        }));
    }
}
