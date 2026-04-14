/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
#[cfg(test)]
use std::cell::Cell;

use egui_tiles::{Container, Tile, TileId, Tree};

use crate::app::workbench_layout_policy::AnchorEdge;
use crate::app::{
    GraphBrowserApp, GraphViewId, PendingConnectedOpenScope, PendingTileOpenMode, SurfaceHostId,
    ToolSurfaceReturnTarget, WorkbenchLayoutConstraint,
};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::render::radial_menu::latest_semantic_snapshot;
use crate::shell::desktop::lifecycle::webview_backpressure::{
    NodePaneAttachAttemptMetadata, take_node_pane_attach_attempt_metadata,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::{
    CommandRouteEventSequenceMetadata, OmnibarMailboxEventSequenceMetadata,
    latest_command_surface_semantic_snapshot,
};

use super::pane_model::TileRenderMode;
use super::tile_kind::TileKind;
use crate::shell::desktop::workbench::pane_model::{PaneId, PanePresentationMode, ToolPaneState};

pub(crate) const UX_TREE_SEMANTIC_SCHEMA_VERSION: u32 = 5;
pub(crate) const UX_TREE_PRESENTATION_SCHEMA_VERSION: u32 = 2;
pub(crate) const UX_TREE_TRACE_SCHEMA_VERSION: u32 = 2;
pub(crate) const UX_TREE_WORKBENCH_ROOT_ID: &str = "uxnode://workbench/root";
const GRAPH_POINT_LOD_THRESHOLD: f32 = 0.55;
const GRAPH_EXPANDED_LOD_THRESHOLD: f32 = 1.10;
const GRAPH_POINT_LOD_STATUS_LABEL: &str = "Zoom in to interact with nodes.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxGraphSemanticLodTier {
    Point,
    Compact,
    Expanded,
}

impl UxGraphSemanticLodTier {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Compact => "compact",
            Self::Expanded => "expanded",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UxGraphLodParityViolation {
    pub(crate) graph_view_id: GraphViewId,
    pub(crate) expected_tier: UxGraphSemanticLodTier,
    pub(crate) actual_mode: &'static str,
    pub(crate) graph_node_count: usize,
    pub(crate) has_status_indicator: bool,
    pub(crate) zoom: f32,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum UxNodeRole {
    Workbench,
    SplitContainer,
    TabContainer,
    GraphSurface,
    GraphNode,
    StatusIndicator,
    NodePane,
    CommandBar,
    Omnibar,
    CommandPalette,
    ContextPalette,
    RadialPalette,
    RadialTierRing,
    RadialSector,
    RadialSummary,
    GraphViewLensScope,
    NavigatorProjection,
    FileTreeProjection,
    RouteOpenBoundary,
    #[cfg(feature = "diagnostics")]
    ToolPane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum UxAction {
    Focus,
    Close,
    SplitHorizontal,
    Select,
    Open,
    Navigate,
    Dismiss,
    Invoke,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UxDomainIdentity {
    Workbench,
    GraphView {
        graph_view_id: GraphViewId,
        pane_id: Option<PaneId>,
    },
    Node {
        node_key: NodeKey,
        pane_id: Option<PaneId>,
        lifecycle: NodeLifecycle,
        attach_attempt: Option<NodePaneAttachAttemptMetadata>,
    },
    CommandBar {
        active_pane: Option<crate::shell::desktop::workbench::pane_model::PaneId>,
        focused_node: Option<NodeKey>,
        location_focused: bool,
        route_events: CommandRouteEventSequenceMetadata,
    },
    Omnibar {
        active: bool,
        focused: bool,
        query: Option<String>,
        match_count: usize,
        provider_status: Option<String>,
        active_pane: Option<crate::shell::desktop::workbench::pane_model::PaneId>,
        focused_node: Option<NodeKey>,
        mailbox_events: OmnibarMailboxEventSequenceMetadata,
    },
    CommandPalette {
        contextual_mode: bool,
        return_target: Option<ToolSurfaceReturnTarget>,
        pending_node_context_target: Option<NodeKey>,
        pending_frame_context_target: Option<String>,
        context_anchor_present: bool,
    },
    #[cfg(feature = "diagnostics")]
    Tool {
        pane_id: PaneId,
        tool_kind: ToolPaneState,
    },
    RadialSector {
        action_id: String,
        enabled: bool,
        tier: u8,
        rail_position: f32,
        hover_scale: f32,
        angle_rad: f32,
        page: usize,
    },
    RadialTierRing {
        tier: u8,
        visible_count: usize,
        page: usize,
        page_count: usize,
    },
    RadialSummary {
        tier1_visible_count: usize,
        tier2_visible_count: usize,
        tier2_page: usize,
        tier2_page_count: usize,
        overflow_hidden_entries: usize,
        label_pre_collisions: usize,
        label_post_collisions: usize,
        fallback_to_palette: bool,
        fallback_reason: Option<String>,
    },
    GraphViewLensScope {
        graph_view_id: GraphViewId,
        lens_name: String,
        lens_id: Option<String>,
        physics_name: String,
        physics_source: String,
        layout_mode: String,
        layout_source: String,
        theme_name: Option<String>,
        theme_source: Option<String>,
        filter_count: usize,
        filter_source: Option<String>,
        overlay_source: Option<String>,
        dimension: String,
        position_fit_locked: bool,
        zoom_fit_locked: bool,
        focused_view: bool,
        selection_count: usize,
    },
    NavigatorProjection {
        host: SurfaceHostId,
        anchor_edge: AnchorEdge,
        form_factor: String,
        scope: String,
        projection_mode: String,
        projection_seed_source: String,
        sort_mode: String,
        root_filter: Option<String>,
        row_count: usize,
        selected_count: usize,
        expanded_count: usize,
        collapsed_count: usize,
        workbench_group_count: usize,
        workbench_member_count: usize,
        unrelated_count: usize,
        recent_count: usize,
    },
    FileTreeProjection {
        projection_seed_source: String,
        sort_mode: String,
        root_filter: Option<String>,
        row_count: usize,
        selected_count: usize,
        expanded_count: usize,
        collapsed_count: usize,
    },
    RouteOpenBoundary {
        pending_node_context_target: Option<NodeKey>,
        pending_open_node: Option<(NodeKey, String)>,
        pending_open_connected: Option<(NodeKey, String, String)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UxNodeState {
    pub(crate) focused: bool,
    pub(crate) selected: bool,
    pub(crate) blocked: bool,
    pub(crate) degraded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UxSemanticNode {
    pub(crate) ux_node_id: String,
    pub(crate) parent_ux_node_id: Option<String>,
    pub(crate) role: UxNodeRole,
    pub(crate) label: String,
    pub(crate) state: UxNodeState,
    pub(crate) allowed_actions: Vec<UxAction>,
    pub(crate) domain: UxDomainIdentity,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UxPresentationNode {
    pub(crate) ux_node_id: String,
    pub(crate) bounds: Option<[f32; 4]>,
    pub(crate) render_mode: Option<TileRenderMode>,
    pub(crate) z_pass: &'static str,
    pub(crate) style_flags: Vec<&'static str>,
    pub(crate) transient_flags: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxTraceNode {
    pub(crate) ux_node_id: String,
    pub(crate) event_route: &'static str,
    pub(crate) backend_path: &'static str,
    pub(crate) diagnostics_counter: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxTraceSummary {
    pub(crate) build_duration_us: u64,
    pub(crate) route_events_observed: u64,
    pub(crate) diagnostics_events_observed: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UxTreeSnapshot {
    pub(crate) semantic_version: u32,
    pub(crate) presentation_version: u32,
    pub(crate) trace_version: u32,
    pub(crate) semantic_nodes: Vec<UxSemanticNode>,
    pub(crate) presentation_nodes: Vec<UxPresentationNode>,
    pub(crate) trace_nodes: Vec<UxTraceNode>,
    pub(crate) trace_summary: UxTraceSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxDiffGateSeverity {
    Blocking,
    Informational,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxSnapshotDiffGate {
    pub(crate) semantic_changed: bool,
    pub(crate) presentation_changed: bool,
    pub(crate) trace_changed: bool,
    pub(crate) semantic_severity: UxDiffGateSeverity,
    pub(crate) presentation_severity: UxDiffGateSeverity,
    pub(crate) trace_severity: UxDiffGateSeverity,
    pub(crate) blocking_failure: bool,
}

pub(crate) fn classify_snapshot_diff_gate(
    baseline: &UxTreeSnapshot,
    current: &UxTreeSnapshot,
    promote_presentation: bool,
) -> UxSnapshotDiffGate {
    let semantic_changed = baseline.semantic_version != current.semantic_version
        || baseline.semantic_nodes != current.semantic_nodes;
    let presentation_changed = baseline.presentation_version != current.presentation_version
        || baseline.presentation_nodes != current.presentation_nodes;
    let trace_changed = baseline.trace_version != current.trace_version
        || baseline.trace_nodes != current.trace_nodes
        || baseline.trace_summary != current.trace_summary;

    let semantic_severity = UxDiffGateSeverity::Blocking;
    let presentation_severity = if promote_presentation {
        UxDiffGateSeverity::Blocking
    } else {
        UxDiffGateSeverity::Informational
    };
    let trace_severity = UxDiffGateSeverity::Informational;

    let blocking_failure = semantic_changed
        || (presentation_changed && matches!(presentation_severity, UxDiffGateSeverity::Blocking));

    UxSnapshotDiffGate {
        semantic_changed,
        presentation_changed,
        trace_changed,
        semantic_severity,
        presentation_severity,
        trace_severity,
        blocking_failure,
    }
}

static LATEST_UX_TREE_SNAPSHOT: OnceLock<Mutex<Option<UxTreeSnapshot>>> = OnceLock::new();
const UX_SNAPSHOT_PATH_ENV_VAR: &str = "GRAPHSHELL_UX_SNAPSHOT_PATH";

#[cfg(test)]
thread_local! {
    static FORCE_UX_TREE_BUILD_FAILURE: Cell<bool> = const { Cell::new(false) };
}

fn snapshot_cache() -> &'static Mutex<Option<UxTreeSnapshot>> {
    LATEST_UX_TREE_SNAPSHOT.get_or_init(|| Mutex::new(None))
}

pub(crate) const fn runtime_enabled() -> bool {
    cfg!(feature = "ux-semantics")
}

fn presentation_style_flags_for_mode(
    base_flags: &[&'static str],
    presentation_mode: PanePresentationMode,
) -> Vec<&'static str> {
    let mut flags = base_flags.to_vec();
    flags.push(match presentation_mode {
        PanePresentationMode::Tiled => "presentation:tiled",
        PanePresentationMode::Docked => "presentation:docked",
        PanePresentationMode::Floating => "presentation:floating",
        PanePresentationMode::Fullscreen => "presentation:fullscreen",
    });
    flags
}

pub(crate) fn publish_snapshot(snapshot: &UxTreeSnapshot) {
    if let Ok(mut slot) = snapshot_cache().lock() {
        *slot = Some(snapshot.clone());
    }
}

pub(crate) fn latest_snapshot() -> Option<UxTreeSnapshot> {
    snapshot_cache().lock().ok().and_then(|slot| slot.clone())
}

pub(crate) fn clear_snapshot() {
    if let Ok(mut slot) = snapshot_cache().lock() {
        *slot = None;
    }
}

pub(crate) fn build_snapshot(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    build_duration_us: u64,
) -> UxTreeSnapshot {
    build_snapshot_with_rects(tiles_tree, graph_app, build_duration_us, &HashMap::new())
}

fn root_snapshot(
    build_duration_us: u64,
    degraded: bool,
    transient_flags: Vec<&'static str>,
) -> UxTreeSnapshot {
    UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes: vec![UxSemanticNode {
            ux_node_id: UX_TREE_WORKBENCH_ROOT_ID.to_string(),
            parent_ux_node_id: None,
            role: UxNodeRole::Workbench,
            label: if degraded {
                "Workbench (degraded)".to_string()
            } else {
                "Workbench".to_string()
            },
            state: UxNodeState {
                focused: true,
                selected: false,
                blocked: false,
                degraded,
            },
            allowed_actions: vec![UxAction::Focus],
            domain: UxDomainIdentity::Workbench,
        }],
        presentation_nodes: vec![UxPresentationNode {
            ux_node_id: UX_TREE_WORKBENCH_ROOT_ID.to_string(),
            bounds: None,
            render_mode: None,
            z_pass: "workbench:root",
            style_flags: vec!["spine:egui_tiles"],
            transient_flags,
        }],
        trace_nodes: vec![UxTraceNode {
            ux_node_id: UX_TREE_WORKBENCH_ROOT_ID.to_string(),
            event_route: "workbench.frame_loop",
            backend_path: "egui_tiles",
            diagnostics_counter: 0,
        }],
        trace_summary: UxTraceSummary {
            build_duration_us,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    }
}

fn node_lifecycle_for_key(graph_app: &GraphBrowserApp, node_key: NodeKey) -> NodeLifecycle {
    graph_app
        .domain_graph()
        .get_node(node_key)
        .map(|node| node.lifecycle)
        .unwrap_or(NodeLifecycle::Cold)
}

fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

pub(crate) fn build_snapshot_with_rects(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    build_duration_us: u64,
    node_rects: &HashMap<NodeKey, egui::Rect>,
) -> UxTreeSnapshot {
    build_snapshot_with_rects_inner(tiles_tree, graph_app, build_duration_us, node_rects)
}

fn build_snapshot_with_rects_inner(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    build_duration_us: u64,
    node_rects: &HashMap<NodeKey, egui::Rect>,
) -> UxTreeSnapshot {
    #[cfg(test)]
    FORCE_UX_TREE_BUILD_FAILURE.with(|enabled| {
        if enabled.get() {
            panic!("forced ux tree build failure");
        }
    });

    let active: HashSet<TileId> = tiles_tree.active_tiles().into_iter().collect();
    let node_attach_attempts = take_node_pane_attach_attempt_metadata();

    let mut snapshot = root_snapshot(build_duration_us, false, Vec::new());
    let mut semantic_nodes = std::mem::take(&mut snapshot.semantic_nodes);
    let mut presentation_nodes = std::mem::take(&mut snapshot.presentation_nodes);
    let mut trace_nodes = std::mem::take(&mut snapshot.trace_nodes);

    if let Some(root) = tiles_tree.root() {
        push_nodes(
            tiles_tree,
            graph_app,
            root,
            &active,
            Some(UX_TREE_WORKBENCH_ROOT_ID),
            node_rects,
            &node_attach_attempts,
            &mut semantic_nodes,
            &mut presentation_nodes,
            &mut trace_nodes,
        );
    }

    append_radial_palette_nodes(
        &mut semantic_nodes,
        &mut presentation_nodes,
        &mut trace_nodes,
    );
    append_command_surface_nodes(
        &mut semantic_nodes,
        &mut presentation_nodes,
        &mut trace_nodes,
    );
    append_workbench_semantics_nodes(
        graph_app,
        &mut semantic_nodes,
        &mut presentation_nodes,
        &mut trace_nodes,
    );

    snapshot.semantic_nodes = semantic_nodes;
    snapshot.presentation_nodes = presentation_nodes;
    snapshot.trace_nodes = trace_nodes;
    snapshot
}

pub(crate) fn try_build_snapshot_with_rects(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    build_duration_us: u64,
    node_rects: &HashMap<NodeKey, egui::Rect>,
) -> Result<UxTreeSnapshot, String> {
    catch_unwind(AssertUnwindSafe(|| {
        build_snapshot_with_rects_inner(tiles_tree, graph_app, build_duration_us, node_rects)
    }))
    .map_err(panic_payload_message)
}

pub(crate) fn degraded_root_only_snapshot(build_duration_us: u64) -> UxTreeSnapshot {
    root_snapshot(
        build_duration_us,
        true,
        vec!["degraded:build_failure"],
    )
}

fn append_command_surface_nodes(
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    let Some(snapshot) = latest_command_surface_semantic_snapshot() else {
        return;
    };

    let command_bar_id = "uxnode://command/bar/root".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: command_bar_id.clone(),
        parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
        role: UxNodeRole::CommandBar,
        label: "Command Bar".to_string(),
        state: UxNodeState {
            focused: snapshot.command_bar.location_focused,
            selected: false,
            blocked: false,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
        domain: UxDomainIdentity::CommandBar {
            active_pane: snapshot.command_bar.active_pane,
            focused_node: snapshot.command_bar.focused_node,
            location_focused: snapshot.command_bar.location_focused,
            route_events: snapshot.command_bar.route_events,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: command_bar_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.bar",
        style_flags: vec!["surface:command-bar"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: command_bar_id.clone(),
        event_route: "command.bar_route",
        backend_path: "egui",
        diagnostics_counter: u64::from(snapshot.command_bar.location_focused),
    });

    let omnibar_id = "uxnode://command/bar/omnibar".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: omnibar_id.clone(),
        parent_ux_node_id: Some(command_bar_id.clone()),
        role: UxNodeRole::Omnibar,
        label: "Omnibar".to_string(),
        state: UxNodeState {
            focused: snapshot.omnibar.focused,
            selected: snapshot.omnibar.active,
            blocked: false,
            degraded: snapshot.omnibar.provider_status.is_some(),
        },
        allowed_actions: vec![UxAction::Focus, UxAction::Navigate, UxAction::Open],
        domain: UxDomainIdentity::Omnibar {
            active: snapshot.omnibar.active,
            focused: snapshot.omnibar.focused,
            query: snapshot.omnibar.query.clone(),
            match_count: snapshot.omnibar.match_count,
            provider_status: snapshot.omnibar.provider_status.clone(),
            active_pane: snapshot.omnibar.active_pane,
            focused_node: snapshot.omnibar.focused_node,
            mailbox_events: snapshot.omnibar.mailbox_events,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: omnibar_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.omnibar",
        style_flags: vec!["surface:omnibar"],
        transient_flags: if snapshot.omnibar.focused {
            vec!["focused"]
        } else {
            Vec::new()
        },
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: omnibar_id,
        event_route: "command.omnibar_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.omnibar.match_count as u64,
    });

    if let Some(command_palette) = snapshot.command_palette {
        let command_palette_id = "uxnode://command/palette/root".to_string();
        semantic_nodes.push(UxSemanticNode {
            ux_node_id: command_palette_id.clone(),
            parent_ux_node_id: Some(command_bar_id.clone()),
            role: UxNodeRole::CommandPalette,
            label: "Command Palette".to_string(),
            state: UxNodeState {
                focused: true,
                selected: true,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Focus, UxAction::Dismiss, UxAction::Navigate],
            domain: UxDomainIdentity::CommandPalette {
                contextual_mode: command_palette.contextual_mode,
                return_target: command_palette.return_target,
                pending_node_context_target: command_palette.pending_node_context_target,
                pending_frame_context_target: command_palette.pending_frame_context_target,
                context_anchor_present: command_palette.context_anchor_present,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: command_palette_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "command.palette",
            style_flags: vec!["surface:command-palette"],
            transient_flags: vec!["mode:palette"],
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: command_palette_id,
            event_route: "command.palette_route",
            backend_path: "egui",
            diagnostics_counter: 1,
        });
    }

    if let Some(context_palette) = snapshot.context_palette {
        let context_palette_id = "uxnode://command/context-palette/root".to_string();
        semantic_nodes.push(UxSemanticNode {
            ux_node_id: context_palette_id.clone(),
            parent_ux_node_id: Some(command_bar_id),
            role: UxNodeRole::ContextPalette,
            label: "Context Palette".to_string(),
            state: UxNodeState {
                focused: true,
                selected: true,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Focus, UxAction::Dismiss, UxAction::Navigate],
            domain: UxDomainIdentity::CommandPalette {
                contextual_mode: context_palette.contextual_mode,
                return_target: context_palette.return_target,
                pending_node_context_target: context_palette.pending_node_context_target,
                pending_frame_context_target: context_palette.pending_frame_context_target,
                context_anchor_present: context_palette.context_anchor_present,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: context_palette_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "command.context_palette",
            style_flags: vec!["surface:context-palette"],
            transient_flags: vec!["mode:contextual"],
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: context_palette_id,
            event_route: "command.context_palette_route",
            backend_path: "egui",
            diagnostics_counter: 1,
        });
    }
}

fn current_frame_tab_container_label(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
    child_count: usize,
) -> Option<String> {
    (tiles_tree.root() == Some(tile_id))
        .then(|| graph_app.current_frame_name())
        .flatten()
        .map(|frame_name| format!("Frame: {frame_name} ({child_count})"))
}

fn append_workbench_semantics_nodes(
    graph_app: &GraphBrowserApp,
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    fn policy_value_source_label(source: Option<&crate::app::PolicyValueSource>) -> Option<String> {
        source.map(|source| match source {
            crate::app::PolicyValueSource::RegistryDefault => "registry-default".to_string(),
            crate::app::PolicyValueSource::WorkspaceDefault => "workspace-default".to_string(),
            crate::app::PolicyValueSource::LensPreset(lens_id) => {
                format!("lens:{lens_id}")
            }
            crate::app::PolicyValueSource::ViewOverride => "view-override".to_string(),
            crate::app::PolicyValueSource::LegacySnapshot => "legacy-snapshot".to_string(),
        })
    }

    fn navigator_hosts_for_snapshot(graph_app: &GraphBrowserApp) -> Vec<SurfaceHostId> {
        let mut hosts = graph_app
            .workspace
            .workbench_session
            .active_layout_constraints
            .keys()
            .filter(|host| matches!(host, SurfaceHostId::Navigator(_)))
            .cloned()
            .collect::<Vec<_>>();
        if hosts.is_empty() {
            hosts.push(graph_app.preferred_default_navigator_surface_host());
        }
        hosts.sort_by_key(|host| match host {
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top) => {
                0
            }
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
            ) => 1,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Left,
            ) => 2,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            ) => 3,
            SurfaceHostId::Role(_) => 4,
        });
        hosts.dedup();
        hosts
    }

    fn default_anchor_edge_for_host(surface_host: &SurfaceHostId) -> AnchorEdge {
        match surface_host {
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top) => {
                AnchorEdge::Top
            }
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
            ) => AnchorEdge::Bottom,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Left,
            ) => AnchorEdge::Left,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            )
            | SurfaceHostId::Role(_) => AnchorEdge::Right,
        }
    }

    fn default_form_factor_for_edge(anchor_edge: AnchorEdge) -> String {
        match anchor_edge {
            AnchorEdge::Top | AnchorEdge::Bottom => "toolbar".to_string(),
            AnchorEdge::Left | AnchorEdge::Right => "sidebar".to_string(),
        }
    }

    for (view_id, view_state) in &graph_app.workspace.graph_runtime.views {
        let selection_count = graph_app.selection_for_view(*view_id).len();
        let focused_view = graph_app.workspace.graph_runtime.focused_view == Some(*view_id);
        let lens_scope_id = format!("uxnode://workbench/graph/{view_id:?}/lens-scope");
        semantic_nodes.push(UxSemanticNode {
            ux_node_id: lens_scope_id.clone(),
            parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
            role: UxNodeRole::GraphViewLensScope,
            label: format!("Graph View Lens/Scope {:?}", view_id),
            state: UxNodeState {
                focused: focused_view,
                selected: false,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Navigate],
            domain: UxDomainIdentity::GraphViewLensScope {
                graph_view_id: *view_id,
                lens_name: view_state.resolved_lens_display_name().to_string(),
                lens_id: view_state.resolved_lens_id().map(str::to_owned),
                physics_name: view_state.resolved_physics_profile().name.clone(),
                physics_source: policy_value_source_label(Some(
                    view_state.resolved_physics_source(),
                ))
                .unwrap_or_else(|| "unknown".to_string()),
                layout_mode: format!("{:?}", view_state.resolved_layout_mode()),
                layout_source: policy_value_source_label(Some(view_state.resolved_layout_source()))
                    .unwrap_or_else(|| "unknown".to_string()),
                theme_name: view_state
                    .resolved_theme()
                    .as_ref()
                    .map(|theme| crate::registries::atomic::lens::theme_data_id(theme).to_string()),
                theme_source: policy_value_source_label(view_state.resolved_theme_source()),
                filter_count: view_state.resolved_filter_count(),
                filter_source: policy_value_source_label(view_state.effective_filter_source()),
                overlay_source: policy_value_source_label(view_state.resolved_overlay_source()),
                dimension: format!("{:?}", view_state.dimension),
                position_fit_locked: view_state.position_fit_locked,
                zoom_fit_locked: view_state.zoom_fit_locked,
                focused_view,
                selection_count,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: lens_scope_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "workbench.graph.lens_scope",
            style_flags: vec!["surface:graph-lens-scope"],
            transient_flags: Vec::new(),
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: lens_scope_id,
            event_route: "graph.lens_scope_route",
            backend_path: "egui_graphs",
            diagnostics_counter: selection_count as u64,
        });
    }

    let navigator_projection = graph_app.navigator_projection_state();
    let section_projection = graph_app.navigator_section_projection();
    let workbench_group_count = section_projection.workbench_groups.len();
    let workbench_member_count = section_projection
        .workbench_groups
        .iter()
        .map(|group| group.member_keys.len())
        .sum();
    let recent_count = section_projection.recent_nodes.len();
    let unrelated_count = section_projection.unrelated_nodes.len();
    for navigator_host in navigator_hosts_for_snapshot(graph_app) {
        let (anchor_edge, form_factor) = match graph_app
            .workspace
            .workbench_session
            .active_layout_constraints
            .get(&navigator_host)
        {
            Some(WorkbenchLayoutConstraint::AnchoredSplit { anchor_edge, .. }) => {
                (*anchor_edge, default_form_factor_for_edge(*anchor_edge))
            }
            _ => {
                let anchor_edge = default_anchor_edge_for_host(&navigator_host);
                (anchor_edge, default_form_factor_for_edge(anchor_edge))
            }
        };
        let configured_scope = graph_app.navigator_host_scope(&navigator_host);
        let navigator_projection_node_id = format!(
            "uxnode://workbench/navigator/projection/{}",
            navigator_host.to_string().replace(':', "/")
        );
        semantic_nodes.push(UxSemanticNode {
            ux_node_id: navigator_projection_node_id.clone(),
            parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
            role: UxNodeRole::NavigatorProjection,
            label: format!("Navigator Projection {}", navigator_host),
            state: UxNodeState {
                focused: false,
                selected: !navigator_projection.selected_rows.is_empty(),
                blocked: navigator_projection.row_targets.is_empty(),
                degraded: false,
            },
            allowed_actions: vec![UxAction::Navigate],
            domain: UxDomainIdentity::NavigatorProjection {
                host: navigator_host.clone(),
                anchor_edge,
                form_factor,
                scope: configured_scope.as_str().to_string(),
                projection_mode: format!("{:?}", navigator_projection.mode),
                projection_seed_source: format!(
                    "{:?}",
                    navigator_projection.projection_seed_source
                ),
                sort_mode: format!("{:?}", navigator_projection.sort_mode),
                root_filter: navigator_projection.root_filter.clone(),
                row_count: navigator_projection.row_targets.len(),
                selected_count: navigator_projection.selected_rows.len(),
                expanded_count: navigator_projection.expanded_rows.len(),
                collapsed_count: navigator_projection.collapsed_rows.len(),
                workbench_group_count,
                workbench_member_count,
                unrelated_count,
                recent_count,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: navigator_projection_node_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "workbench.navigator.projection",
            style_flags: vec!["surface:navigator"],
            transient_flags: Vec::new(),
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: navigator_projection_node_id,
            event_route: "workbench.navigator_route",
            backend_path: "egui",
            diagnostics_counter: navigator_projection.row_targets.len() as u64,
        });
    }

    let route_node_id = "uxnode://workbench/route-open/boundary".to_string();
    let pending_open_node = graph_app.pending_open_node_request().map(|pending| {
        (
            pending.key,
            pending_tile_mode_label(pending.mode).to_string(),
        )
    });
    let pending_open_connected =
        graph_app
            .pending_open_connected_from()
            .map(|(source, mode, scope)| {
                (
                    source,
                    pending_tile_mode_label(mode).to_string(),
                    pending_connected_scope_label(scope).to_string(),
                )
            });
    let pending_count = usize::from(graph_app.pending_node_context_target().is_some())
        + usize::from(pending_open_node.is_some())
        + usize::from(pending_open_connected.is_some());
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: route_node_id.clone(),
        parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
        role: UxNodeRole::RouteOpenBoundary,
        label: "Route/Open Boundary".to_string(),
        state: UxNodeState {
            focused: false,
            selected: false,
            blocked: false,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Open, UxAction::Navigate],
        domain: UxDomainIdentity::RouteOpenBoundary {
            pending_node_context_target: graph_app.pending_node_context_target(),
            pending_open_node,
            pending_open_connected,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: route_node_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "workbench.route_open.boundary",
        style_flags: vec!["surface:route-open"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: route_node_id,
        event_route: "workbench.route_open_route",
        backend_path: "egui",
        diagnostics_counter: pending_count as u64,
    });
}

fn pending_tile_mode_label(mode: PendingTileOpenMode) -> &'static str {
    match mode {
        PendingTileOpenMode::Tab => "tab",
        PendingTileOpenMode::SplitHorizontal => "split-horizontal",
        PendingTileOpenMode::QuarterPane => "quarter-pane",
        PendingTileOpenMode::HalfPane => "half-pane",
    }
}

fn pending_connected_scope_label(scope: PendingConnectedOpenScope) -> &'static str {
    match scope {
        PendingConnectedOpenScope::Neighbors => "neighbors",
        PendingConnectedOpenScope::Connected => "connected",
    }
}

fn append_radial_palette_nodes(
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    let Some(snapshot) = latest_semantic_snapshot() else {
        return;
    };

    let radial_root_id = "uxnode://command/radial/root".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: radial_root_id.clone(),
        parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
        role: UxNodeRole::RadialPalette,
        label: "Radial Palette".to_string(),
        state: UxNodeState {
            focused: true,
            selected: false,
            blocked: false,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Focus, UxAction::Dismiss, UxAction::Navigate],
        domain: UxDomainIdentity::Workbench,
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: radial_root_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.radial",
        style_flags: vec!["surface:radial"],
        transient_flags: vec!["mode:radial"],
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: radial_root_id.clone(),
        event_route: "command.radial_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.sectors.len() as u64,
    });

    let tier1_ring_id = "uxnode://command/radial/tier-1-ring".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: tier1_ring_id.clone(),
        parent_ux_node_id: Some(radial_root_id.clone()),
        role: UxNodeRole::RadialTierRing,
        label: "Tier-1 Ring".to_string(),
        state: UxNodeState {
            focused: snapshot
                .sectors
                .iter()
                .any(|sector| sector.tier == 1 && sector.hover_scale > 1.0),
            selected: false,
            blocked: snapshot.summary.tier1_visible_count == 0,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Navigate],
        domain: UxDomainIdentity::RadialTierRing {
            tier: 1,
            visible_count: snapshot.summary.tier1_visible_count,
            page: 0,
            page_count: 1,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: tier1_ring_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.radial.tier1",
        style_flags: vec!["surface:radial-tier"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: tier1_ring_id.clone(),
        event_route: "command.radial_tier1_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.summary.tier1_visible_count as u64,
    });

    let tier2_ring_id = "uxnode://command/radial/tier-2-ring".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: tier2_ring_id.clone(),
        parent_ux_node_id: Some(radial_root_id.clone()),
        role: UxNodeRole::RadialTierRing,
        label: "Tier-2 Ring".to_string(),
        state: UxNodeState {
            focused: snapshot
                .sectors
                .iter()
                .any(|sector| sector.tier == 2 && sector.hover_scale > 1.0),
            selected: false,
            blocked: snapshot.summary.tier2_visible_count == 0,
            degraded: snapshot.summary.fallback_to_palette,
        },
        allowed_actions: vec![UxAction::Navigate],
        domain: UxDomainIdentity::RadialTierRing {
            tier: 2,
            visible_count: snapshot.summary.tier2_visible_count,
            page: snapshot.summary.tier2_page,
            page_count: snapshot.summary.tier2_page_count,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: tier2_ring_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.radial.tier2",
        style_flags: vec!["surface:radial-tier"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: tier2_ring_id.clone(),
        event_route: "command.radial_tier2_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.summary.tier2_visible_count as u64,
    });

    let radial_summary_id = "uxnode://command/radial/summary".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: radial_summary_id.clone(),
        parent_ux_node_id: Some(radial_root_id.clone()),
        role: UxNodeRole::RadialSummary,
        label: "Radial Layout Summary".to_string(),
        state: UxNodeState {
            focused: false,
            selected: false,
            blocked: false,
            degraded: snapshot.summary.fallback_to_palette,
        },
        allowed_actions: vec![UxAction::Navigate],
        domain: UxDomainIdentity::RadialSummary {
            tier1_visible_count: snapshot.summary.tier1_visible_count,
            tier2_visible_count: snapshot.summary.tier2_visible_count,
            tier2_page: snapshot.summary.tier2_page,
            tier2_page_count: snapshot.summary.tier2_page_count,
            overflow_hidden_entries: snapshot.summary.overflow_hidden_entries,
            label_pre_collisions: snapshot.summary.label_pre_collisions,
            label_post_collisions: snapshot.summary.label_post_collisions,
            fallback_to_palette: snapshot.summary.fallback_to_palette,
            fallback_reason: snapshot.summary.fallback_reason.clone(),
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: radial_summary_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.radial.summary",
        style_flags: vec!["surface:radial-summary"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: radial_summary_id,
        event_route: "command.radial_summary_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.summary.overflow_hidden_entries as u64,
    });

    for (idx, sector) in snapshot.sectors.iter().enumerate() {
        let sector_id = format!(
            "uxnode://command/radial/tier{}/domain/{}/sector/{}",
            sector.tier,
            sector.domain_label.to_ascii_lowercase(),
            idx
        );
        semantic_nodes.push(UxSemanticNode {
            ux_node_id: sector_id.clone(),
            parent_ux_node_id: Some(match sector.tier {
                1 => tier1_ring_id.clone(),
                _ => tier2_ring_id.clone(),
            }),
            role: UxNodeRole::RadialSector,
            label: format!("{} [{}]", sector.action_id, sector.domain_label),
            state: UxNodeState {
                focused: sector.hover_scale > 1.0,
                selected: false,
                blocked: !sector.enabled,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Invoke, UxAction::Navigate],
            domain: UxDomainIdentity::RadialSector {
                action_id: sector.action_id.clone(),
                enabled: sector.enabled,
                tier: sector.tier,
                rail_position: sector.rail_position,
                hover_scale: sector.hover_scale,
                angle_rad: sector.angle_rad,
                page: sector.page,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: sector_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "command.radial.sector",
            style_flags: vec!["surface:radial-sector"],
            transient_flags: if sector.hover_scale > 1.0 {
                vec!["hovered"]
            } else {
                Vec::new()
            },
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: sector_id,
            event_route: "command.radial_sector_route",
            backend_path: "egui",
            diagnostics_counter: u64::from(sector.enabled),
        });
    }
}

fn graph_view_zoom(graph_app: &GraphBrowserApp, graph_view_id: GraphViewId) -> f32 {
    graph_app
        .workspace
        .graph_runtime
        .graph_view_frames
        .get(&graph_view_id)
        .map(|frame| frame.zoom)
        .filter(|zoom| *zoom > 0.0)
        .or_else(|| {
            graph_app
                .workspace
                .graph_runtime
                .views
                .get(&graph_view_id)
                .map(|view| view.camera.current_zoom)
                .filter(|zoom| *zoom > 0.0)
        })
        .unwrap_or(0.8)
}

fn graph_semantic_lod_tier_for_zoom(zoom: f32) -> UxGraphSemanticLodTier {
    if zoom < GRAPH_POINT_LOD_THRESHOLD {
        UxGraphSemanticLodTier::Point
    } else if zoom < GRAPH_EXPANDED_LOD_THRESHOLD {
        UxGraphSemanticLodTier::Compact
    } else {
        UxGraphSemanticLodTier::Expanded
    }
}

fn graph_semantic_lod_tier(
    graph_app: &GraphBrowserApp,
    graph_view_id: GraphViewId,
) -> UxGraphSemanticLodTier {
    graph_semantic_lod_tier_for_zoom(graph_view_zoom(graph_app, graph_view_id))
}

fn append_graph_point_lod_status_indicator(
    graph_view_id: GraphViewId,
    pane_id: Option<PaneId>,
    parent_ux_node_id: &str,
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    let status_id = format!("uxnode://workbench/graph/{graph_view_id:?}/status/point-lod");
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: status_id.clone(),
        parent_ux_node_id: Some(parent_ux_node_id.to_string()),
        role: UxNodeRole::StatusIndicator,
        label: GRAPH_POINT_LOD_STATUS_LABEL.to_string(),
        state: UxNodeState {
            focused: false,
            selected: false,
            blocked: false,
            degraded: false,
        },
        allowed_actions: Vec::new(),
        domain: UxDomainIdentity::GraphView {
            graph_view_id,
            pane_id,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: status_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "graph.layer.status",
        style_flags: vec!["surface:status-indicator", "context:graph-lod-point"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: status_id,
        event_route: "graph.status_route",
        backend_path: "egui_graphs",
        diagnostics_counter: 1,
    });
}

fn append_graph_surface_semantics(
    graph_app: &GraphBrowserApp,
    ux_node_id: &str,
    parent_ux_node_id: Option<&str>,
    graph_view_id: GraphViewId,
    pane_id: Option<PaneId>,
    focused: bool,
    tile_selected: bool,
    style_flags: Vec<&'static str>,
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    let focused_selection = graph_app.focused_selection();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: ux_node_id.to_string(),
        parent_ux_node_id: parent_ux_node_id.map(str::to_string),
        role: UxNodeRole::GraphSurface,
        label: format!("Graph View {graph_view_id:?}"),
        state: UxNodeState {
            focused,
            selected: tile_selected,
            blocked: false,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Focus, UxAction::Close, UxAction::Navigate],
        domain: UxDomainIdentity::GraphView {
            graph_view_id,
            pane_id,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: ux_node_id.to_string(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "workbench.content",
        style_flags,
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: ux_node_id.to_string(),
        event_route: "graph.input_route",
        backend_path: "egui_graphs",
        diagnostics_counter: graph_app.domain_graph().node_count() as u64,
    });

    let graph_node_total = graph_app.domain_graph().node_count();
    if graph_node_total > 0
        && matches!(
            graph_semantic_lod_tier(graph_app, graph_view_id),
            UxGraphSemanticLodTier::Point
        )
    {
        append_graph_point_lod_status_indicator(
            graph_view_id,
            pane_id,
            ux_node_id,
            semantic_nodes,
            presentation_nodes,
            trace_nodes,
        );
        return;
    }

    for (node_key, node) in graph_app.domain_graph().nodes() {
        let graph_node_ux_id = format!(
            "uxnode://workbench/graph/{graph_view_id:?}/node/{}",
            node.id
        );
        let selected = focused_selection.contains(&node_key);
        let focused_graph_node = focused_selection.primary() == Some(node_key);
        let blocked = graph_app.runtime_block_state_for_node(node_key).is_some();
        let degraded = graph_app.runtime_crash_state_for_node(node_key).is_some();

        semantic_nodes.push(UxSemanticNode {
            ux_node_id: graph_node_ux_id.clone(),
            parent_ux_node_id: Some(ux_node_id.to_string()),
            role: UxNodeRole::GraphNode,
            label: ux_tree_node_label(graph_app, node_key, node),
            state: UxNodeState {
                focused: focused_graph_node,
                selected,
                blocked,
                degraded,
            },
            allowed_actions: vec![UxAction::Select, UxAction::Open, UxAction::Navigate],
            domain: UxDomainIdentity::Node {
                node_key,
                pane_id: None,
                lifecycle: node.lifecycle,
                attach_attempt: None,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: graph_node_ux_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "graph.layer.node",
            style_flags: vec!["surface:graph-node", "backend:egui_graphs"],
            transient_flags: Vec::new(),
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: graph_node_ux_id,
            event_route: "graph.node_route",
            backend_path: "egui_graphs",
            diagnostics_counter: u64::from(selected),
        });
    }
}

fn push_nodes(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
    active: &HashSet<TileId>,
    parent_ux_node_id: Option<&str>,
    node_rects: &HashMap<NodeKey, egui::Rect>,
    node_attach_attempts: &HashMap<NodeKey, NodePaneAttachAttemptMetadata>,
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    let Some(tile) = tiles_tree.tiles.get(tile_id) else {
        return;
    };

    let ux_node_id = ux_node_id_for_tile(tile_id, tile);
    let focused = active.contains(&tile_id);
    let tile_selected = graph_app
        .workbench_tile_selection()
        .selected_tile_ids
        .contains(&tile_id);

    match tile {
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(view_ref),
        )) => {
            append_graph_surface_semantics(
                graph_app,
                &ux_node_id,
                parent_ux_node_id,
                view_ref.graph_view_id,
                Some(view_ref.pane_id),
                focused,
                tile_selected,
                vec!["surface:graph", "backend:egui_graphs"],
                semantic_nodes,
                presentation_nodes,
                trace_nodes,
            );
        }
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        )) => {
            let focused_selection = graph_app.focused_selection();
            let blocked = graph_app.runtime_block_state_for_node(state.node).is_some();
            let degraded = matches!(state.render_mode, TileRenderMode::Placeholder);
            let selected = focused_selection.contains(&state.node);

            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::NodePane,
                label: format!("Node Pane {:?}", state.node),
                state: UxNodeState {
                    focused,
                    selected: selected || tile_selected,
                    blocked,
                    degraded,
                },
                allowed_actions: vec![
                    UxAction::Focus,
                    UxAction::Open,
                    UxAction::Close,
                    UxAction::SplitHorizontal,
                ],
                domain: UxDomainIdentity::Node {
                    node_key: state.node,
                    pane_id: Some(state.pane_id),
                    lifecycle: node_lifecycle_for_key(graph_app, state.node),
                    attach_attempt: node_attach_attempts.get(&state.node).copied(),
                },
            });
            let bounds = node_rects
                .get(&state.node)
                .map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]);
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds,
                render_mode: Some(state.render_mode),
                z_pass: "workbench.content",
                style_flags: presentation_style_flags_for_mode(
                    &["surface:node"],
                    state.presentation_mode,
                ),
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.node_route",
                backend_path: match state.render_mode {
                    TileRenderMode::CompositedTexture => "viewer.composited",
                    TileRenderMode::NativeOverlay => "viewer.native_overlay",
                    TileRenderMode::EmbeddedEgui => "viewer.embedded_egui",
                    TileRenderMode::Placeholder => "viewer.placeholder",
                },
                diagnostics_counter: u64::from(focused),
            });
        }
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool),
        )) => {
            let tool_kind = tool.title();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::ToolPane,
                label: format!("Tool Pane {tool_kind}"),
                state: UxNodeState {
                    focused,
                    selected: tile_selected,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Close],
                domain: UxDomainIdentity::Tool {
                    pane_id: tool.pane_id,
                    tool_kind: tool.kind.clone(),
                },
            });
        }
        Tile::Pane(TileKind::Graph(view_ref)) => {
            append_graph_surface_semantics(
                graph_app,
                &ux_node_id,
                parent_ux_node_id,
                view_ref.graph_view_id,
                Some(view_ref.pane_id),
                focused,
                tile_selected,
                presentation_style_flags_for_mode(
                    &["surface:graph", "backend:egui_graphs"],
                    view_ref.presentation_mode,
                ),
                semantic_nodes,
                presentation_nodes,
                trace_nodes,
            );
        }
        Tile::Pane(TileKind::Node(state)) => {
            let focused_selection = graph_app.focused_selection();
            let blocked = graph_app.runtime_block_state_for_node(state.node).is_some();
            let degraded = matches!(state.render_mode, TileRenderMode::Placeholder);
            let selected = focused_selection.contains(&state.node);

            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::NodePane,
                label: format!("Node Pane {:?}", state.node),
                state: UxNodeState {
                    focused,
                    selected: selected || tile_selected,
                    blocked,
                    degraded,
                },
                allowed_actions: vec![
                    UxAction::Focus,
                    UxAction::Open,
                    UxAction::Close,
                    UxAction::SplitHorizontal,
                ],
                domain: UxDomainIdentity::Node {
                    node_key: state.node,
                    pane_id: Some(state.pane_id),
                    lifecycle: node_lifecycle_for_key(graph_app, state.node),
                    attach_attempt: node_attach_attempts.get(&state.node).copied(),
                },
            });
            let bounds = node_rects
                .get(&state.node)
                .map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]);
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds,
                render_mode: Some(state.render_mode),
                z_pass: "workbench.content",
                style_flags: presentation_style_flags_for_mode(
                    &["surface:node"],
                    state.presentation_mode,
                ),
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.node_route",
                backend_path: match state.render_mode {
                    TileRenderMode::CompositedTexture => "viewer.composited",
                    TileRenderMode::NativeOverlay => "viewer.native_overlay",
                    TileRenderMode::EmbeddedEgui => "viewer.embedded_egui",
                    TileRenderMode::Placeholder => "viewer.placeholder",
                },
                diagnostics_counter: u64::from(focused),
            });
        }
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Tool(tool)) => {
            let tool_kind = tool.title();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::ToolPane,
                label: format!("Tool Pane {tool_kind}"),
                state: UxNodeState {
                    focused,
                    selected: tile_selected,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Close],
                domain: UxDomainIdentity::Tool {
                    pane_id: tool.pane_id,
                    tool_kind: tool.kind.clone(),
                },
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: Some(TileRenderMode::EmbeddedEgui),
                z_pass: "workbench.tool",
                style_flags: presentation_style_flags_for_mode(
                    &["surface:tool"],
                    tool.presentation_mode,
                ),
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.tool_route",
                backend_path: "egui",
                diagnostics_counter: 0,
            });
        }
        Tile::Container(Container::Tabs(tabs)) => {
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::TabContainer,
                label: current_frame_tab_container_label(
                    tiles_tree,
                    graph_app,
                    tile_id,
                    tabs.children.len(),
                )
                .unwrap_or_else(|| format!("Tabs ({})", tabs.children.len())),
                state: UxNodeState {
                    focused,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus],
                domain: UxDomainIdentity::Workbench,
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: None,
                z_pass: "workbench.tabs",
                style_flags: vec!["container:tabs"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.tabs_route",
                backend_path: "egui_tiles",
                diagnostics_counter: tabs.children.len() as u64,
            });

            for child in &tabs.children {
                push_nodes(
                    tiles_tree,
                    graph_app,
                    *child,
                    active,
                    Some(ux_node_id.as_str()),
                    node_rects,
                    node_attach_attempts,
                    semantic_nodes,
                    presentation_nodes,
                    trace_nodes,
                );
            }
        }
        Tile::Container(Container::Linear(linear)) => {
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::SplitContainer,
                label: format!("Split ({})", linear.children.len()),
                state: UxNodeState {
                    focused,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus],
                domain: UxDomainIdentity::Workbench,
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: None,
                z_pass: "workbench.split",
                style_flags: vec!["container:linear"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.split_route",
                backend_path: "egui_tiles",
                diagnostics_counter: linear.children.len() as u64,
            });

            for child in &linear.children {
                push_nodes(
                    tiles_tree,
                    graph_app,
                    *child,
                    active,
                    Some(ux_node_id.as_str()),
                    node_rects,
                    node_attach_attempts,
                    semantic_nodes,
                    presentation_nodes,
                    trace_nodes,
                );
            }
        }
        Tile::Container(Container::Grid(grid)) => {
            let grid_children_count = grid.children().count();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::SplitContainer,
                label: format!("Grid ({})", grid_children_count),
                state: UxNodeState {
                    focused,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus],
                domain: UxDomainIdentity::Workbench,
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: None,
                z_pass: "workbench.grid",
                style_flags: vec!["container:grid"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.grid_route",
                backend_path: "egui_tiles",
                diagnostics_counter: grid_children_count as u64,
            });

            for child in grid.children() {
                push_nodes(
                    tiles_tree,
                    graph_app,
                    *child,
                    active,
                    Some(ux_node_id.as_str()),
                    node_rects,
                    node_attach_attempts,
                    semantic_nodes,
                    presentation_nodes,
                    trace_nodes,
                );
            }
        }
    }
}

fn ux_node_id_for_tile(tile_id: TileId, tile: &Tile<TileKind>) -> String {
    match tile {
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(view_ref),
        )) => {
            format!(
                "uxnode://workbench/tile/{tile_id:?}/graph/{:?}",
                view_ref.graph_view_id
            )
        }
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        )) => {
            format!("uxnode://workbench/tile/{tile_id:?}/node/{:?}", state.node)
        }
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool),
        )) => {
            format!("uxnode://workbench/tile/{tile_id:?}/tool/{}", tool.title())
        }
        Tile::Pane(TileKind::Graph(view_ref)) => {
            format!(
                "uxnode://workbench/tile/{tile_id:?}/graph/{:?}",
                view_ref.graph_view_id
            )
        }
        Tile::Pane(TileKind::Node(state)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/node/{:?}", state.node)
        }
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Tool(tool)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/tool/{}", tool.title())
        }
        Tile::Container(Container::Tabs(_)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/tabs")
        }
        Tile::Container(Container::Linear(_)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/split")
        }
        Tile::Container(Container::Grid(_)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/grid")
        }
    }
}

pub(crate) fn semantic_ids(snapshot: &UxTreeSnapshot) -> HashSet<&str> {
    snapshot
        .semantic_nodes
        .iter()
        .map(|node| node.ux_node_id.as_str())
        .collect()
}

pub(crate) fn presentation_ids(snapshot: &UxTreeSnapshot) -> HashSet<&str> {
    snapshot
        .presentation_nodes
        .iter()
        .map(|node| node.ux_node_id.as_str())
        .collect()
}

pub(crate) fn trace_ids(snapshot: &UxTreeSnapshot) -> HashSet<&str> {
    snapshot
        .trace_nodes
        .iter()
        .map(|node| node.ux_node_id.as_str())
        .collect()
}

pub(crate) fn presentation_id_consistency_violation(snapshot: &UxTreeSnapshot) -> Option<String> {
    let semantic = semantic_ids(snapshot);
    for node in &snapshot.presentation_nodes {
        if !semantic.contains(node.ux_node_id.as_str()) {
            return Some(format!(
                "uxtree invariant failed: presentation ux_node_id '{}' missing from semantic layer",
                node.ux_node_id
            ));
        }
    }
    None
}

pub(crate) fn trace_id_consistency_violation(snapshot: &UxTreeSnapshot) -> Option<String> {
    let semantic = semantic_ids(snapshot);
    for node in &snapshot.trace_nodes {
        if !semantic.contains(node.ux_node_id.as_str()) {
            return Some(format!(
                "uxtree invariant failed: trace ux_node_id '{}' missing from semantic layer",
                node.ux_node_id
            ));
        }
    }
    None
}

pub(crate) fn semantic_parent_link_violation(snapshot: &UxTreeSnapshot) -> Option<String> {
    let semantic = semantic_ids(snapshot);
    snapshot.semantic_nodes.iter().find_map(|node| {
        let parent_id = node.parent_ux_node_id.as_deref()?;
        (!semantic.contains(parent_id)).then(|| {
            format!(
                "uxtree invariant failed: semantic ux_node_id '{}' references missing parent '{}'",
                node.ux_node_id, parent_id
            )
        })
    })
}

pub(crate) fn interactive_label_presence_violation(snapshot: &UxTreeSnapshot) -> Option<String> {
    snapshot.semantic_nodes.iter().find_map(|node| {
        (!node.allowed_actions.is_empty() && node.label.trim().is_empty()).then(|| {
            format!(
                "uxtree invariant failed: interactive ux_node_id '{}' has empty label",
                node.ux_node_id
            )
        })
    })
}

pub(crate) fn semantic_focus_uniqueness_violation(snapshot: &UxTreeSnapshot) -> Option<String> {
    let focused_nodes = snapshot
        .semantic_nodes
        .iter()
        .filter(|node| node.state.focused)
        .map(|node| node.ux_node_id.as_str())
        .collect::<Vec<_>>();

    (focused_nodes.len() > 1).then(|| {
        format!(
            "uxtree invariant failed: multiple focused semantic nodes advertised simultaneously: {}",
            focused_nodes.join(", ")
        )
    })
}

pub(crate) fn semantic_id_uniqueness_violation(snapshot: &UxTreeSnapshot) -> Option<String> {
    let mut seen = HashSet::new();
    snapshot.semantic_nodes.iter().find_map(|node| {
        (!seen.insert(node.ux_node_id.as_str())).then(|| {
            format!(
                "uxtree invariant failed: duplicate semantic ux_node_id '{}' detected in one snapshot",
                node.ux_node_id
            )
        })
    })
}

pub(crate) fn node_pane_tombstone_lifecycle_violation(
    snapshot: &UxTreeSnapshot,
) -> Option<(String, String)> {
    snapshot.semantic_nodes.iter().find_map(|node| {
        if node.role != UxNodeRole::NodePane {
            return None;
        }

        match &node.domain {
            UxDomainIdentity::Node {
                node_key,
                lifecycle: NodeLifecycle::Tombstone,
                ..
            } => Some((
                node.ux_node_id.clone(),
                format!(
                    "uxtree invariant failed: NodePane '{}' projected tombstoned node {:?}",
                    node.ux_node_id, node_key
                ),
            )),
            _ => None,
        }
    })
}

pub(crate) fn interactive_bounds_violation(
    snapshot: &UxTreeSnapshot,
) -> Option<(String, String)> {
    let presentation_bounds: HashMap<&str, [f32; 4]> = snapshot
        .presentation_nodes
        .iter()
        .filter_map(|node| node.bounds.map(|bounds| (node.ux_node_id.as_str(), bounds)))
        .collect();

    snapshot.semantic_nodes.iter().find_map(|node| {
        if node.allowed_actions.is_empty() {
            return None;
        }

        let bounds = presentation_bounds.get(node.ux_node_id.as_str())?;
        let width = (bounds[2] - bounds[0]).max(0.0);
        let height = (bounds[3] - bounds[1]).max(0.0);

        ((width < 32.0) || (height < 32.0)).then(|| {
            (
                node.ux_node_id.clone(),
                format!(
                    "uxtree invariant failed: interactive ux_node_id '{}' has bounds {:.1}x{:.1} below 32x32 minimum",
                    node.ux_node_id, width, height
                ),
            )
        })
    })
}

pub(crate) fn radial_sector_count_violation(snapshot: &UxTreeSnapshot) -> Option<String> {
    let has_radial_palette = snapshot
        .semantic_nodes
        .iter()
        .any(|node| node.role == UxNodeRole::RadialPalette);
    if !has_radial_palette {
        return None;
    }

    let sector_count = snapshot
        .semantic_nodes
        .iter()
        .filter(|node| node.role == UxNodeRole::RadialSector)
        .count();

    (!(1..=8).contains(&sector_count)).then(|| {
        format!(
            "uxtree invariant failed: radial palette projected {} radial sectors outside the 1..=8 bound",
            sector_count
        )
    })
}

fn command_surface_capture_owners(snapshot: &UxTreeSnapshot) -> Vec<&'static str> {
    snapshot
        .semantic_nodes
        .iter()
        .filter_map(|node| match (&node.role, &node.domain) {
            (
                UxNodeRole::Omnibar,
                UxDomainIdentity::Omnibar {
                    focused: true,
                    active: true,
                    ..
                },
            ) => Some("omnibar"),
            (UxNodeRole::CommandPalette, UxDomainIdentity::CommandPalette { .. })
                if node.state.focused || node.state.selected =>
            {
                Some("command_palette")
            }
            (UxNodeRole::ContextPalette, UxDomainIdentity::CommandPalette { .. })
                if node.state.focused || node.state.selected =>
            {
                Some("context_palette")
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn command_surface_capture_owner_violation(
    snapshot: &UxTreeSnapshot,
) -> Option<String> {
    let has_command_bar = snapshot
        .semantic_nodes
        .iter()
        .any(|node| node.role == UxNodeRole::CommandBar);
    if !has_command_bar {
        return None;
    }

    let capture_owners = command_surface_capture_owners(snapshot);
    (capture_owners.len() > 1).then(|| {
        format!(
            "uxtree invariant failed: multiple command-surface capture owners advertised semantic focus: {}",
            capture_owners.join(", ")
        )
    })
}

fn command_bar_has_restore_anchor(snapshot: &UxTreeSnapshot) -> bool {
    snapshot.semantic_nodes.iter().any(|node| {
        matches!(
            &node.domain,
            UxDomainIdentity::CommandBar {
                active_pane,
                focused_node,
                ..
            } if active_pane.is_some() || focused_node.is_some()
        )
    })
}

pub(crate) fn command_surface_return_target_violation(
    snapshot: &UxTreeSnapshot,
) -> Option<String> {
    let has_fallback_anchor = command_bar_has_restore_anchor(snapshot);
    snapshot.semantic_nodes.iter().find_map(|node| {
        let UxDomainIdentity::CommandPalette {
            return_target,
            pending_node_context_target,
            pending_frame_context_target,
            context_anchor_present,
            ..
        } = &node.domain
        else {
            return None;
        };

        let visible_palette = matches!(node.role, UxNodeRole::CommandPalette | UxNodeRole::ContextPalette)
            && (node.state.focused || node.state.selected);
        if !visible_palette {
            return None;
        }

        let has_restore_path = return_target.is_some()
            || pending_node_context_target.is_some()
            || pending_frame_context_target.is_some()
            || *context_anchor_present
            || has_fallback_anchor;

        (!has_restore_path).then(|| {
            format!(
                "uxtree invariant failed: visible {} has no return target or fallback anchor",
                match node.role {
                    UxNodeRole::CommandPalette => "command palette",
                    UxNodeRole::ContextPalette => "context palette",
                    _ => "command surface",
                }
            )
        })
    })
}

pub(crate) fn graph_lod_semantic_emission_violation(
    snapshot: &UxTreeSnapshot,
    graph_app: &GraphBrowserApp,
) -> Option<UxGraphLodParityViolation> {
    let focused_view = graph_app.workspace.graph_runtime.focused_view;
    let graph_surface = snapshot
        .semantic_nodes
        .iter()
        .find(|node| {
            node.role == UxNodeRole::GraphSurface
                && matches!(
                    node.domain,
                    UxDomainIdentity::GraphView { graph_view_id, .. }
                        if Some(graph_view_id) == focused_view
                )
        })
        .or_else(|| {
            snapshot
                .semantic_nodes
                .iter()
                .find(|node| node.role == UxNodeRole::GraphSurface)
        })?;

    let UxDomainIdentity::GraphView { graph_view_id, .. } = graph_surface.domain else {
        return None;
    };

    let graph_node_count = snapshot
        .semantic_nodes
        .iter()
        .filter(|node| {
            node.role == UxNodeRole::GraphNode
                && node.parent_ux_node_id.as_deref() == Some(graph_surface.ux_node_id.as_str())
        })
        .count();
    let has_status_indicator = snapshot.semantic_nodes.iter().any(|node| {
        node.role == UxNodeRole::StatusIndicator
            && node.parent_ux_node_id.as_deref() == Some(graph_surface.ux_node_id.as_str())
            && node.label == GRAPH_POINT_LOD_STATUS_LABEL
    });
    let zoom = graph_view_zoom(graph_app, graph_view_id);
    let expected_tier = graph_semantic_lod_tier_for_zoom(zoom);
    let total_graph_nodes = graph_app.domain_graph().node_count();
    let actual_mode = if graph_node_count > 0 {
        "graph_nodes"
    } else if has_status_indicator {
        "status_indicator"
    } else {
        "empty"
    };

    let mismatch = match expected_tier {
        UxGraphSemanticLodTier::Point => {
            if total_graph_nodes > 0 {
                graph_node_count > 0 || !has_status_indicator
            } else {
                graph_node_count > 0 || has_status_indicator
            }
        }
        UxGraphSemanticLodTier::Compact | UxGraphSemanticLodTier::Expanded => {
            (total_graph_nodes > 0 && graph_node_count == 0) || has_status_indicator
        }
    };

    mismatch.then(|| UxGraphLodParityViolation {
        graph_view_id,
        expected_tier,
        actual_mode,
        graph_node_count,
        has_status_indicator,
        zoom,
        message: format!(
            "uxtree invariant failed: graph view {graph_view_id:?} semantic emission mismatched active {} LOD at zoom {:.2} (mode={actual_mode}, graph_nodes={graph_node_count}, status_indicator={has_status_indicator})",
            expected_tier.as_str(),
            zoom,
        ),
    })
}

pub(crate) fn node_pane_bounds_missing_count(snapshot: &UxTreeSnapshot) -> usize {
    let presentation_has_bounds: std::collections::HashMap<&str, bool> = snapshot
        .presentation_nodes
        .iter()
        .map(|n| (n.ux_node_id.as_str(), n.bounds.is_some()))
        .collect();
    snapshot
        .semantic_nodes
        .iter()
        .filter(|n| n.role == UxNodeRole::NodePane)
        .filter(|n| {
            !presentation_has_bounds
                .get(n.ux_node_id.as_str())
                .copied()
                .unwrap_or(false)
        })
        .count()
}

pub(crate) struct CoverageReport {
    /// Number of adjacent tile pairs with a gap > 1 px between their edges.
    pub(crate) gutter_pair_count: usize,
    /// Number of tile pairs whose rects have a non-zero area intersection.
    pub(crate) overlap_pair_count: usize,
}

/// Pure coverage analysis over a set of laid-out tile rects.
///
/// A *gutter* is a gap of more than 1 px between two rects that share an
/// axis-aligned edge direction (i.e. one rect's right edge is close to another
/// rect's left edge, or top/bottom equivalents) but do not actually touch.
///
/// An *overlap* is any pair of rects whose intersection has positive area.
pub(crate) fn run_coverage_analysis(
    rects: &std::collections::HashMap<crate::graph::NodeKey, egui::Rect>,
) -> CoverageReport {
    const GAP_THRESHOLD: f32 = 1.0;

    let rects: Vec<egui::Rect> = rects.values().copied().collect();
    let mut gutter_pair_count = 0usize;
    let mut overlap_pair_count = 0usize;

    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let a = rects[i];
            let b = rects[j];

            // Overlap: intersection has positive area.
            let inter = a.intersect(b);
            if inter.width() > 0.0 && inter.height() > 0.0 {
                overlap_pair_count += 1;
                continue;
            }

            // Gutter: rects do not overlap but are "neighbours" along one axis
            // with a gap > GAP_THRESHOLD on the shared axis.
            //
            // Horizontal neighbours: x-projections overlap or are adjacent,
            // one rect's right is close to the other's left.
            let x_gap = if a.max.x <= b.min.x {
                b.min.x - a.max.x
            } else if b.max.x <= a.min.x {
                a.min.x - b.max.x
            } else {
                0.0
            };
            let y_gap = if a.max.y <= b.min.y {
                b.min.y - a.max.y
            } else if b.max.y <= a.min.y {
                a.min.y - b.max.y
            } else {
                0.0
            };

            // Only consider pairs that are neighbours along exactly one axis
            // (the other axis has overlapping or touching projections).
            let x_proj_overlap =
                a.min.x < b.max.x + GAP_THRESHOLD && b.min.x < a.max.x + GAP_THRESHOLD;
            let y_proj_overlap =
                a.min.y < b.max.y + GAP_THRESHOLD && b.min.y < a.max.y + GAP_THRESHOLD;

            if x_gap > GAP_THRESHOLD && y_proj_overlap {
                gutter_pair_count += 1;
            } else if y_gap > GAP_THRESHOLD && x_proj_overlap {
                gutter_pair_count += 1;
            }
        }
    }

    CoverageReport {
        gutter_pair_count,
        overlap_pair_count,
    }
}

fn ux_tree_node_label(
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
    node: &crate::graph::Node,
) -> String {
    graph_app.user_visible_node_title(node_key).unwrap_or_else(|| {
        if node.title.is_empty() {
            node.url().to_string()
        } else {
            node.title.clone()
        }
    })
}

pub(crate) fn snapshot_json(snapshot: &UxTreeSnapshot) -> serde_json::Value {
    serde_json::json!({
        "semantic_version": snapshot.semantic_version,
        "presentation_version": snapshot.presentation_version,
        "trace_version": snapshot.trace_version,
        "semantic_nodes": snapshot.semantic_nodes.iter().map(|node| serde_json::json!({
            "ux_node_id": node.ux_node_id,
            "parent_ux_node_id": node.parent_ux_node_id,
            "role": format!("{:?}", node.role),
            "label": node.label,
            "focused": node.state.focused,
            "selected": node.state.selected,
            "blocked": node.state.blocked,
            "degraded": node.state.degraded,
            "allowed_actions": node.allowed_actions.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>(),
            "domain": format!("{:?}", node.domain),
        })).collect::<Vec<_>>(),
        "presentation_nodes": snapshot.presentation_nodes.iter().map(|node| serde_json::json!({
            "ux_node_id": node.ux_node_id,
            "z_pass": node.z_pass,
            "style_flags": node.style_flags,
            "transient_flags": node.transient_flags,
            "render_mode": node.render_mode.map(|mode| format!("{:?}", mode)),
        })).collect::<Vec<_>>(),
        "trace_nodes": snapshot.trace_nodes.iter().map(|node| serde_json::json!({
            "ux_node_id": node.ux_node_id,
            "event_route": node.event_route,
            "backend_path": node.backend_path,
            "diagnostics_counter": node.diagnostics_counter,
        })).collect::<Vec<_>>(),
        "trace_summary": {
            "build_duration_us": snapshot.trace_summary.build_duration_us,
            "route_events_observed": snapshot.trace_summary.route_events_observed,
            "diagnostics_events_observed": snapshot.trace_summary.diagnostics_events_observed,
        }
    })
}

pub(crate) fn write_snapshot_to_path(
    snapshot: &UxTreeSnapshot,
    path: &Path,
) -> Result<usize, String> {
    let payload = serde_json::to_vec_pretty(&snapshot_json(snapshot))
        .map_err(|error| format!("failed to serialize ux snapshot: {error}"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create ux snapshot directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    fs::write(path, &payload).map_err(|error| {
        format!(
            "failed to write ux snapshot to '{}': {error}",
            path.display()
        )
    })?;
    Ok(payload.len())
}

pub(crate) fn write_snapshot_if_configured(
    snapshot: &UxTreeSnapshot,
) -> Result<Option<(PathBuf, usize)>, String> {
    let Some(path) = std::env::var_os(UX_SNAPSHOT_PATH_ENV_VAR).map(PathBuf::from) else {
        return Ok(None);
    };
    let bytes_written = write_snapshot_to_path(snapshot, &path)?;
    Ok(Some((path, bytes_written)))
}

#[cfg(test)]
pub(crate) fn snapshot_json_for_tests(snapshot: &UxTreeSnapshot) -> serde_json::Value {
    snapshot_json(snapshot)
}

#[cfg(test)]
fn set_force_build_failure_for_tests(enabled: bool) {
        FORCE_UX_TREE_BUILD_FAILURE.with(|flag| flag.set(enabled));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        NavigatorProjectionSeedSource, PendingConnectedOpenScope, PendingTileOpenMode,
    };
    use crate::render::radial_menu::{
        RadialPaletteSemanticSnapshot, RadialPaletteSemanticSummary, RadialSectorSemanticMetadata,
        clear_semantic_snapshot, publish_semantic_snapshot,
    };
    use crate::shell::desktop::ui::toolbar::toolbar_ui::{
        CommandBarSemanticMetadata, CommandRouteEventSequenceMetadata,
        CommandSurfaceSemanticSnapshot, OmnibarMailboxEventSequenceMetadata,
        OmnibarSemanticMetadata, PaletteSurfaceSemanticMetadata,
        clear_command_surface_semantic_snapshot, publish_command_surface_semantic_snapshot,
    };
    use crate::shell::desktop::tests::harness::TestRegistry;

    #[test]
    fn snapshot_uses_single_canonical_id_space_across_layers() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-id.example");
        harness.open_node_tab(node);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 12);
        let semantic = semantic_ids(&snapshot);
        let presentation = presentation_ids(&snapshot);
        let trace = trace_ids(&snapshot);

        assert!(
            presentation.is_subset(&semantic),
            "presentation ids must be subset of semantic ids"
        );
        assert!(trace.is_subset(&semantic), "trace ids must be subset of semantic ids");
    }

    #[test]
    fn consistency_probe_flags_missing_semantic_id_for_presentation_node() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-probe.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        snapshot.presentation_nodes.push(UxPresentationNode {
            ux_node_id: "uxnode://orphan/presentation".to_string(),
            bounds: None,
            render_mode: None,
            z_pass: "workbench.orphan",
            style_flags: Vec::new(),
            transient_flags: Vec::new(),
        });

        let violation = presentation_id_consistency_violation(&snapshot)
            .expect("probe should detect orphan presentation node");
        assert!(violation.contains("orphan/presentation"));
    }

    #[test]
    fn consistency_probe_flags_missing_semantic_id_for_trace_node() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-trace-probe.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        snapshot.trace_nodes.push(UxTraceNode {
            ux_node_id: "uxnode://orphan/trace".to_string(),
            event_route: "orphan.trace_route",
            backend_path: "egui",
            diagnostics_counter: 0,
        });

        let violation = trace_id_consistency_violation(&snapshot)
            .expect("probe should detect orphan trace node");
        assert!(violation.contains("orphan/trace"));
    }

    #[test]
    fn semantic_parent_link_violation_flags_missing_parent_node() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-orphan-parent.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        let graph_surface = snapshot
            .semantic_nodes
            .iter_mut()
            .find(|entry| entry.role == UxNodeRole::GraphSurface)
            .expect("graph surface should be present");
        graph_surface.parent_ux_node_id = Some("uxnode://missing/parent".to_string());

        let violation = semantic_parent_link_violation(&snapshot)
            .expect("probe should detect orphan semantic parent link");
        assert!(violation.contains("missing/parent"));
    }

    #[test]
    fn interactive_label_presence_violation_flags_empty_interactive_label() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-empty-label.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        let graph_surface = snapshot
            .semantic_nodes
            .iter_mut()
            .find(|entry| entry.role == UxNodeRole::GraphSurface)
            .expect("graph surface should be present");
        graph_surface.label = "   ".to_string();

        let violation = interactive_label_presence_violation(&snapshot)
            .expect("probe should detect empty label on interactive node");
        assert!(violation.contains("empty label"));
    }

    #[test]
    fn semantic_focus_uniqueness_violation_flags_multiple_focused_nodes() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-focus-uniqueness.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        let mut focused_nodes = snapshot
            .semantic_nodes
            .iter_mut()
            .filter(|entry| !entry.allowed_actions.is_empty())
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(focused_nodes.len(), 2, "expected two interactive nodes to mark focused");
        focused_nodes[0].state.focused = true;
        focused_nodes[1].state.focused = true;

        let violation = semantic_focus_uniqueness_violation(&snapshot)
            .expect("probe should detect duplicate focus ownership");
        assert!(violation.contains("multiple focused semantic nodes"));
    }

    #[test]
    fn semantic_id_uniqueness_violation_flags_duplicate_semantic_ids() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-duplicate-id.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        let duplicate = snapshot
            .semantic_nodes
            .first()
            .expect("snapshot should contain a root semantic node")
            .clone();
        snapshot.semantic_nodes.push(duplicate.clone());

        let violation = semantic_id_uniqueness_violation(&snapshot)
            .expect("probe should detect duplicate semantic node ids");
        assert!(violation.contains(duplicate.ux_node_id.as_str()));
    }

    #[test]
    fn snapshot_projects_node_lifecycle_into_node_domains() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-node-lifecycle.example");
        harness.open_node_tab(node);

        let expected_lifecycle = harness
            .app
            .domain_graph()
            .get_node(node)
            .expect("node should exist in the graph")
            .lifecycle;
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        let node_pane = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| entry.role == UxNodeRole::NodePane)
            .expect("snapshot should contain a node pane semantic node");

        assert!(matches!(
            node_pane.domain,
            UxDomainIdentity::Node {
                node_key,
                lifecycle,
                ..
            } if node_key == node && lifecycle == expected_lifecycle
        ));
    }

    #[test]
    fn node_pane_tombstone_lifecycle_violation_flags_tombstoned_node_pane() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-tombstone-pane.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        let node_pane_id = {
            let node_pane = snapshot
                .semantic_nodes
                .iter_mut()
                .find(|entry| entry.role == UxNodeRole::NodePane)
                .expect("snapshot should contain a node pane semantic node");
            node_pane.domain = UxDomainIdentity::Node {
                node_key: node,
                pane_id: Some(crate::shell::desktop::workbench::pane_model::PaneId::new()),
                lifecycle: NodeLifecycle::Tombstone,
                attach_attempt: None,
            };
            node_pane.ux_node_id.clone()
        };

        let (node_path, violation) = node_pane_tombstone_lifecycle_violation(&snapshot)
            .expect("probe should detect tombstoned node panes");
        assert_eq!(node_path, node_pane_id);
        assert!(violation.contains("tombstoned node"));
    }

    #[test]
    fn interactive_bounds_violation_flags_subminimum_interactive_bounds() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-bounds.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        let interactive_id = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                !entry.allowed_actions.is_empty()
                    && snapshot
                        .presentation_nodes
                        .iter()
                        .any(|presentation| presentation.ux_node_id == entry.ux_node_id)
            })
            .expect("snapshot should contain an interactive semantic node with presentation")
            .ux_node_id
            .clone();
        let presentation = snapshot
            .presentation_nodes
            .iter_mut()
            .find(|entry| entry.ux_node_id == interactive_id)
            .expect("interactive node should have presentation metadata");
        presentation.bounds = Some([0.0, 0.0, 24.0, 18.0]);

        let (node_path, violation) = interactive_bounds_violation(&snapshot)
            .expect("probe should detect undersized interactive bounds");
        assert_eq!(node_path, interactive_id);
        assert!(violation.contains("24.0x18.0"));
    }

    #[test]
    fn radial_sector_count_violation_flags_overfull_radial_palette() {
        publish_semantic_snapshot(RadialPaletteSemanticSnapshot {
            sectors: (0..9)
                .map(|index| RadialSectorSemanticMetadata {
                    tier: 1,
                    domain_label: format!("Domain {index}"),
                    action_id: format!("action.{index}"),
                    enabled: true,
                    page: 0,
                    rail_position: index as f32 / 9.0,
                    angle_rad: index as f32,
                    hover_scale: 1.0,
                })
                .collect(),
            summary: RadialPaletteSemanticSummary {
                tier1_visible_count: 9,
                tier2_visible_count: 0,
                tier2_page: 0,
                tier2_page_count: 0,
                overflow_hidden_entries: 1,
                label_pre_collisions: 0,
                label_post_collisions: 0,
                fallback_to_palette: false,
                fallback_reason: None,
            },
        });

        let harness = TestRegistry::new();
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);
        clear_semantic_snapshot();

        let violation = radial_sector_count_violation(&snapshot)
            .expect("probe should detect overfull radial palette sector count");
        assert!(violation.contains("9 radial sectors"));
    }

    #[test]
    fn try_build_snapshot_with_rects_reports_forced_builder_failure() {
        let harness = TestRegistry::new();
        set_force_build_failure_for_tests(true);

        let result = try_build_snapshot_with_rects(
            &harness.tiles_tree,
            &harness.app,
            5,
            &HashMap::new(),
        );

        set_force_build_failure_for_tests(false);
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("forced ux tree build failure"));
    }

    #[test]
    fn degraded_root_only_snapshot_marks_root_as_degraded() {
        let snapshot = degraded_root_only_snapshot(11);

        assert_eq!(snapshot.semantic_nodes.len(), 1);
        assert!(snapshot.semantic_nodes[0].state.degraded);
        assert!(snapshot.presentation_nodes[0]
            .transient_flags
            .contains(&"degraded:build_failure"));
    }

    #[test]
    fn write_snapshot_to_path_writes_json_payload() {
        let harness = TestRegistry::new();
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);
        let temp_path = std::env::temp_dir().join(format!(
            "graphshell-ux-snapshot-{}.json",
            std::process::id()
        ));

        let bytes_written = write_snapshot_to_path(&snapshot, &temp_path)
            .expect("snapshot export should succeed");
        let body = fs::read_to_string(&temp_path).expect("snapshot file should be readable");
        let _ = fs::remove_file(&temp_path);

        assert!(bytes_written > 0);
        assert!(body.contains("semantic_version"));
        assert!(body.contains(UX_TREE_WORKBENCH_ROOT_ID));
    }

    #[test]
    fn snapshot_projects_stable_parent_links_for_graph_surface_hierarchy() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-parent-links.example");
        harness.open_node_tab(node);
        harness.app.select_node(node, false);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);
        let graph_surface = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| entry.role == UxNodeRole::GraphSurface)
            .expect("graph surface should be present");
        let graph_surface_parent = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                Some(entry.ux_node_id.as_str()) == graph_surface.parent_ux_node_id.as_deref()
            })
            .expect("graph surface parent should be projected");
        assert_eq!(graph_surface_parent.role, UxNodeRole::TabContainer);

        let graph_node = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| entry.role == UxNodeRole::GraphNode && matches!(entry.domain, UxDomainIdentity::Node { node_key, .. } if node_key == node))
            .expect("selected graph node should be projected");
        assert_eq!(
            graph_node.parent_ux_node_id.as_deref(),
            Some(graph_surface.ux_node_id.as_str())
        );
    }

    #[test]
    fn snapshot_labels_root_tab_container_as_active_frame_when_frame_handle_is_open() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-frame.example");
        harness.open_node_tab(node);
        harness.app.note_frame_activated("alpha", [node]);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 7);

        let frame_tabs = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                entry.role == UxNodeRole::TabContainer && entry.label.contains("Frame: alpha")
            })
            .expect("active frame tab container should be labeled explicitly");
        assert_eq!(
            frame_tabs.parent_ux_node_id.as_deref(),
            Some(UX_TREE_WORKBENCH_ROOT_ID)
        );
    }

    #[test]
    fn diff_gate_semantic_changes_are_blocking_by_default() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-diff-semantic.example");
        harness.open_node_tab(node);

        let baseline = build_snapshot(&harness.tiles_tree, &harness.app, 10);
        let mut current = baseline.clone();
        current.semantic_nodes[0].label = "Workbench Renamed".to_string();

        let gate = classify_snapshot_diff_gate(&baseline, &current, false);
        assert!(gate.semantic_changed);
        assert!(gate.blocking_failure);
        assert_eq!(gate.semantic_severity, UxDiffGateSeverity::Blocking);
    }

    #[test]
    fn diff_gate_presentation_changes_are_informational_by_default() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-diff-presentation.example");
        harness.open_node_tab(node);

        let baseline = build_snapshot(&harness.tiles_tree, &harness.app, 10);
        let mut current = baseline.clone();
        current.presentation_nodes[0]
            .transient_flags
            .push("anim:pulse");

        let gate = classify_snapshot_diff_gate(&baseline, &current, false);
        assert!(!gate.semantic_changed);
        assert!(gate.presentation_changed);
        assert!(!gate.blocking_failure);
        assert_eq!(
            gate.presentation_severity,
            UxDiffGateSeverity::Informational
        );
    }

    #[test]
    fn diff_gate_can_promote_presentation_changes_to_blocking() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-diff-promotion.example");
        harness.open_node_tab(node);

        let baseline = build_snapshot(&harness.tiles_tree, &harness.app, 10);
        let mut current = baseline.clone();
        current.presentation_nodes[0]
            .style_flags
            .push("promoted-style");

        let gate = classify_snapshot_diff_gate(&baseline, &current, true);
        assert!(gate.presentation_changed);
        assert!(gate.blocking_failure);
        assert_eq!(gate.presentation_severity, UxDiffGateSeverity::Blocking);
    }

    #[test]
    fn snapshot_projects_graph_nodes_into_semantic_layer() {
        let mut harness = TestRegistry::new();
        let node_a = harness.add_node("https://ux-tree-graph-a.example");
        let node_b = harness.add_node("https://ux-tree-graph-b.example");
        harness.open_node_tab(node_a);
        harness.app.select_node(node_b, false);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);
        let graph_nodes = snapshot
            .semantic_nodes
            .iter()
            .filter(|entry| entry.role == UxNodeRole::GraphNode)
            .collect::<Vec<_>>();

        assert_eq!(graph_nodes.len(), 2);
        assert!(graph_nodes.iter().any(|entry| entry.state.selected));
    }

    #[test]
    fn snapshot_omits_graph_nodes_and_projects_status_indicator_at_point_lod() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-point-lod.example");
        harness.open_node_tab(node);

        let initial_snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);
        let graph_view_id = initial_snapshot
            .semantic_nodes
            .iter()
            .find_map(|entry| match entry.domain {
                UxDomainIdentity::GraphView { graph_view_id, .. }
                    if entry.role == UxNodeRole::GraphSurface =>
                {
                    Some(graph_view_id)
                }
                _ => None,
            })
            .expect("graph surface should advertise a graph view id");
        harness.app.workspace.graph_runtime.graph_view_frames.insert(
            graph_view_id,
            crate::app::GraphViewFrame {
                zoom: 0.4,
                pan_x: 0.0,
                pan_y: 0.0,
            },
        );

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);

        assert!(
            snapshot
                .semantic_nodes
                .iter()
                .all(|entry| entry.role != UxNodeRole::GraphNode),
            "point LOD should suppress graph-node semantic children"
        );
        assert!(snapshot.semantic_nodes.iter().any(|entry| {
            entry.role == UxNodeRole::StatusIndicator
                && entry.label == GRAPH_POINT_LOD_STATUS_LABEL
        }));
        assert!(graph_lod_semantic_emission_violation(&snapshot, &harness.app).is_none());
    }

    #[test]
    fn snapshot_projects_graph_nodes_without_status_indicator_at_compact_lod() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-compact-lod.example");
        harness.open_node_tab(node);

        let initial_snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);
        let graph_view_id = initial_snapshot
            .semantic_nodes
            .iter()
            .find_map(|entry| match entry.domain {
                UxDomainIdentity::GraphView { graph_view_id, .. }
                    if entry.role == UxNodeRole::GraphSurface =>
                {
                    Some(graph_view_id)
                }
                _ => None,
            })
            .expect("graph surface should advertise a graph view id");
        harness.app.workspace.graph_runtime.graph_view_frames.insert(
            graph_view_id,
            crate::app::GraphViewFrame {
                zoom: 0.8,
                pan_x: 0.0,
                pan_y: 0.0,
            },
        );

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);

        assert!(snapshot
            .semantic_nodes
            .iter()
            .any(|entry| entry.role == UxNodeRole::GraphNode));
        assert!(snapshot
            .semantic_nodes
            .iter()
            .all(|entry| entry.role != UxNodeRole::StatusIndicator));
        assert!(graph_lod_semantic_emission_violation(&snapshot, &harness.app).is_none());
    }

    #[test]
    fn graph_lod_semantic_emission_violation_detects_point_lod_mismatch() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-point-mismatch.example");
        harness.open_node_tab(node);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);
        let graph_view_id = snapshot
            .semantic_nodes
            .iter()
            .find_map(|entry| match entry.domain {
                UxDomainIdentity::GraphView { graph_view_id, .. }
                    if entry.role == UxNodeRole::GraphSurface =>
                {
                    Some(graph_view_id)
                }
                _ => None,
            })
            .expect("graph surface should advertise a graph view id");
        harness.app.workspace.graph_runtime.graph_view_frames.insert(
            graph_view_id,
            crate::app::GraphViewFrame {
                zoom: 0.4,
                pan_x: 0.0,
                pan_y: 0.0,
            },
        );

        let violation = graph_lod_semantic_emission_violation(&snapshot, &harness.app)
            .expect("point-tier mismatch should be detected");

        assert_eq!(violation.graph_view_id, graph_view_id);
        assert_eq!(violation.expected_tier, UxGraphSemanticLodTier::Point);
        assert_eq!(violation.actual_mode, "graph_nodes");
        assert!(violation.message.contains("mismatched active point LOD"));
    }

    #[test]
    fn snapshot_projects_radial_sector_metadata_when_available() {
        clear_semantic_snapshot();
        publish_semantic_snapshot(RadialPaletteSemanticSnapshot {
            sectors: vec![RadialSectorSemanticMetadata {
                tier: 2,
                domain_label: "Node".to_string(),
                action_id: "NodeDelete".to_string(),
                enabled: true,
                page: 0,
                rail_position: 0.15,
                angle_rad: 1.2,
                hover_scale: 1.5,
            }],
            summary: RadialPaletteSemanticSummary {
                tier1_visible_count: 4,
                tier2_visible_count: 1,
                tier2_page: 0,
                tier2_page_count: 1,
                overflow_hidden_entries: 0,
                label_pre_collisions: 2,
                label_post_collisions: 0,
                fallback_to_palette: false,
                fallback_reason: None,
            },
        });

        let harness = TestRegistry::new();
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 7);

        assert!(
            snapshot
                .semantic_nodes
                .iter()
                .any(|node| node.role == UxNodeRole::RadialPalette),
            "snapshot should include radial palette root when metadata is available"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.role == UxNodeRole::RadialSector
                    && matches!(
                        &node.domain,
                        UxDomainIdentity::RadialSector {
                            action_id,
                            enabled,
                            tier,
                            ..
                        } if action_id == "NodeDelete" && *enabled && *tier == 2
                    )
            }),
            "snapshot should include radial sector action metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.role == UxNodeRole::RadialTierRing
                    && matches!(
                        &node.domain,
                        UxDomainIdentity::RadialTierRing {
                            tier,
                            visible_count,
                            ..
                        } if *tier == 1 && *visible_count == 4
                    )
            }),
            "snapshot should include explicit tier-1 ring container metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.role == UxNodeRole::RadialSummary
                    && matches!(
                        &node.domain,
                        UxDomainIdentity::RadialSummary {
                            label_pre_collisions,
                            label_post_collisions,
                            ..
                        } if *label_pre_collisions == 2 && *label_post_collisions == 0
                    )
            }),
            "snapshot should include radial overflow/readability summary metadata"
        );

        clear_semantic_snapshot();
    }

    #[test]
    fn snapshot_projects_command_surface_probe_receipts() {
        let _guard = crate::shell::desktop::ui::toolbar::toolbar_ui::lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: Some(crate::shell::desktop::workbench::pane_model::PaneId::new()),
                focused_node: Some(NodeKey::new(17)),
                location_focused: true,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: true,
                focused: true,
                query: Some("rust graph".to_string()),
                match_count: 4,
                provider_status: Some("Suggestions: loading...".to_string()),
                active_pane: None,
                focused_node: Some(NodeKey::new(17)),
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: Some(PaletteSurfaceSemanticMetadata {
                contextual_mode: false,
                return_target: Some(ToolSurfaceReturnTarget::Graph(GraphViewId::new())),
                pending_node_context_target: None,
                pending_frame_context_target: None,
                context_anchor_present: false,
            }),
            context_palette: None,
        });

        let harness = TestRegistry::new();
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 7);

        assert!(
            snapshot
                .semantic_nodes
                .iter()
                .any(|node| node.role == UxNodeRole::CommandBar),
            "snapshot should include command bar semantic root"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.role == UxNodeRole::Omnibar
                    && matches!(
                        &node.domain,
                        UxDomainIdentity::Omnibar {
                            active,
                            focused,
                            query,
                            match_count,
                            focused_node,
                            ..
                        } if *active
                            && *focused
                            && query.as_deref() == Some("rust graph")
                            && *match_count == 4
                            && *focused_node == Some(NodeKey::new(17))
                    )
            }),
            "snapshot should include omnibar probe metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.role == UxNodeRole::CommandPalette
                    && matches!(
                        &node.domain,
                        UxDomainIdentity::CommandPalette {
                            contextual_mode,
                            return_target,
                            ..
                        } if !*contextual_mode
                            && matches!(return_target, Some(ToolSurfaceReturnTarget::Graph(_)))
                    )
            }),
            "snapshot should include command palette return-target metadata"
        );

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn command_surface_capture_owner_violation_detects_conflicting_owners() {
        let _guard = crate::shell::desktop::ui::toolbar::toolbar_ui::lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: Some(crate::shell::desktop::workbench::pane_model::PaneId::new()),
                focused_node: Some(NodeKey::new(9)),
                location_focused: true,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: true,
                focused: true,
                query: Some("graphshell".to_string()),
                match_count: 2,
                provider_status: None,
                active_pane: None,
                focused_node: Some(NodeKey::new(9)),
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: Some(PaletteSurfaceSemanticMetadata {
                contextual_mode: false,
                return_target: Some(ToolSurfaceReturnTarget::Graph(GraphViewId::new())),
                pending_node_context_target: None,
                pending_frame_context_target: None,
                context_anchor_present: false,
            }),
            context_palette: None,
        });

        let harness = TestRegistry::new();
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 7);

        let violation = command_surface_capture_owner_violation(&snapshot)
            .expect("expected capture-owner conflict to be detected");
        assert!(violation.contains("omnibar"));
        assert!(violation.contains("command_palette"));

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn command_surface_return_target_violation_detects_missing_restore_anchor() {
        let _guard = crate::shell::desktop::ui::toolbar::toolbar_ui::lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: None,
                focused_node: None,
                location_focused: false,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: false,
                focused: false,
                query: None,
                match_count: 0,
                provider_status: None,
                active_pane: None,
                focused_node: None,
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: Some(PaletteSurfaceSemanticMetadata {
                contextual_mode: false,
                return_target: None,
                pending_node_context_target: None,
                pending_frame_context_target: None,
                context_anchor_present: false,
            }),
            context_palette: None,
        });

        let harness = TestRegistry::new();
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 7);

        let violation = command_surface_return_target_violation(&snapshot)
            .expect("expected missing command-surface restore anchor to be detected");
        assert!(violation.contains("command palette"));

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn command_surface_return_target_violation_accepts_command_bar_fallback_anchor() {
        let _guard = crate::shell::desktop::ui::toolbar::toolbar_ui::lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: Some(crate::shell::desktop::workbench::pane_model::PaneId::new()),
                focused_node: None,
                location_focused: false,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: false,
                focused: false,
                query: None,
                match_count: 0,
                provider_status: None,
                active_pane: None,
                focused_node: None,
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: Some(PaletteSurfaceSemanticMetadata {
                contextual_mode: false,
                return_target: None,
                pending_node_context_target: None,
                pending_frame_context_target: None,
                context_anchor_present: false,
            }),
            context_palette: None,
        });

        let harness = TestRegistry::new();
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 7);

        assert!(command_surface_return_target_violation(&snapshot).is_none());

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn snapshot_projects_lens_scope_navigator_and_route_open_boundary_nodes() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-route-open.example");
        harness.open_node_tab(node);

        let view_id = GraphViewId::default();
        harness.app.ensure_graph_view_registered(view_id);
        if let Some(view) = harness.app.workspace.graph_runtime.views.get_mut(&view_id) {
            view.lens_state.display_name = "Research Lens".to_string();
            view.layout_policy.mode = crate::registries::atomic::lens::LayoutMode::Free;
            view.filter_policy.legacy_filters = vec!["tag:#clip".to_string()];
        }
        harness.app.workspace.graph_runtime.focused_view = Some(view_id);

        harness.app.set_navigator_host_scope(
            crate::app::workbench_layout_policy::default_navigator_surface_host(),
            crate::app::workbench_layout_policy::NavigatorHostScope::WorkbenchOnly,
        );
        harness.app.set_navigator_projection_seed_source(
            NavigatorProjectionSeedSource::SavedViewCollections,
        );
        harness
            .app
            .set_navigator_projection_mode(crate::app::NavigatorProjectionMode::Workbench);
        let row_key = format!("view:{}", view_id.as_uuid());
        harness.app.set_navigator_selected_rows([row_key]);

        harness.app.set_pending_node_context_target(Some(node));
        harness
            .app
            .request_open_node_tile_mode(node, PendingTileOpenMode::SplitHorizontal);
        harness.app.request_open_connected_from(
            node,
            PendingTileOpenMode::Tab,
            PendingConnectedOpenScope::Neighbors,
        );

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 9);

        assert!(
            snapshot.semantic_nodes.iter().any(|entry| {
                entry.role == UxNodeRole::GraphViewLensScope
                    && matches!(
                        &entry.domain,
                        UxDomainIdentity::GraphViewLensScope {
                            graph_view_id,
                            lens_name,
                            filter_count,
                            layout_source,
                            physics_source,
                            focused_view,
                            ..
                        } if *graph_view_id == view_id
                            && lens_name == "Research Lens"
                            && *filter_count == 1
                            && layout_source == "registry-default"
                            && physics_source == "registry-default"
                            && *focused_view
                    )
            }),
            "snapshot should include graph view lens/scope metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|entry| {
                entry.role == UxNodeRole::NavigatorProjection
                    && matches!(
                        &entry.domain,
                        UxDomainIdentity::NavigatorProjection {
                            host,
                            anchor_edge,
                            form_factor,
                            scope,
                            projection_mode,
                            projection_seed_source,
                            selected_count,
                            row_count,
                            ..
                        } if *host == crate::app::workbench_layout_policy::default_navigator_surface_host()
                            && *anchor_edge == AnchorEdge::Left
                            && form_factor == "sidebar"
                            && scope == "workbench"
                            && projection_mode == "Workbench"
                            && projection_seed_source == "SavedViewCollections"
                            && *selected_count == 1
                            && *row_count >= 1
                    )
            }),
            "snapshot should include navigator projection metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|entry| {
                entry.role == UxNodeRole::RouteOpenBoundary
                    && matches!(
                        &entry.domain,
                        UxDomainIdentity::RouteOpenBoundary {
                            pending_node_context_target,
                            pending_open_node,
                            pending_open_connected,
                        } if *pending_node_context_target == Some(node)
                            && pending_open_node
                                .as_ref()
                                .is_some_and(|(key, mode)| *key == node && mode == "split-horizontal")
                            && pending_open_connected
                                .as_ref()
                                .is_some_and(|(source, mode, scope)| {
                                    *source == node && mode == "tab" && scope == "neighbors"
                                })
                    )
            }),
            "snapshot should include route/open boundary pending intent metadata"
        );
    }

    #[test]
    fn snapshot_projects_multiple_navigator_hosts_from_profile_constraints() {
        let mut harness = TestRegistry::new();
        let view_id = GraphViewId::default();
        harness.app.ensure_graph_view_registered(view_id);
        harness.app.set_workbench_layout_constraint(
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top),
            WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Top,
                ),
                anchor_edge: AnchorEdge::Top,
                anchor_size_fraction: 0.15,
                cross_axis_margin_start_px: 12.0,
                cross_axis_margin_end_px: 18.0,
                resizable: true,
            },
        );
        harness.app.set_workbench_layout_constraint(
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Bottom),
            WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
                ),
                anchor_edge: AnchorEdge::Bottom,
                anchor_size_fraction: 0.14,
                cross_axis_margin_start_px: 0.0,
                cross_axis_margin_end_px: 0.0,
                resizable: false,
            },
        );

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 11);

        let projected_hosts = snapshot
            .semantic_nodes
            .iter()
            .filter_map(|entry| match &entry.domain {
                UxDomainIdentity::NavigatorProjection { host, .. } => Some(host.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(projected_hosts.contains(&SurfaceHostId::Navigator(
            crate::app::workbench_layout_policy::NavigatorHostId::Top,
        )));
        assert!(projected_hosts.contains(&SurfaceHostId::Navigator(
            crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
        )));
    }
}

