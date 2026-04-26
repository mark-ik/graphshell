/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{BTreeMap, HashMap, HashSet};

use egui::{Panel, RichText};
use egui_tiles::{Container, LinearDir, Tile, TileId, Tree};
use uuid::Uuid;

use crate::app::workbench_layout_policy::{AnchorEdge, FirstUseOutcome, NavigatorHostId};
use crate::app::{
    CameraCommand, GraphBrowserApp, GraphIntent, GraphViewId, NavigatorHostScope,
    PendingTileOpenMode, SurfaceFirstUsePolicy, SurfaceHostId, UxConfigMode, WorkbenchDisplayMode,
    WorkbenchIntent, WorkbenchLayoutConstraint, WorkbenchNavigationGeometry,
    user_visible_node_title_from_data, user_visible_node_url_from_data,
};
use crate::graph::{
    ArrangementSubKind, DominantEdge, FrameLayoutHint, GraphletKind, NodeKey, SplitOrientation,
};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_DISPATCH_CONSUMED, CHANNEL_UX_DISPATCH_STARTED,
    CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use crate::shell::desktop::workbench::pane_model::{
    NodePaneState, PaneId, PanePresentationMode, SplitDirection, TileRenderMode, ToolPaneState,
    ViewerId, ViewerSwitchReason,
};
use crate::shell::desktop::workbench::semantic_tabs;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_render_pass;
use crate::shell::desktop::workbench::tile_runtime;
use crate::shell::desktop::workbench::tile_view_ops;
use crate::util::CoordBridge;
use crate::util::VersoAddress;

/// Maximum workbench-host panel width as a fraction of screen width. Clamped so
/// the host panel never exceeds one-fifth of the available screen, with an
/// absolute floor of 180 px.
const HOST_PANEL_MAX_FRACTION: f32 = 0.20;
const HOST_PANEL_MAX_FLOOR: f32 = 180.0;
const HOST_PANEL_MIN_FRACTION: f32 = 0.10;
const HOST_PANEL_MARGIN_MAX: f32 = 240.0;
const HOST_PANEL_LABEL_MAX_CHARS: usize = 40;
const NAVIGATOR_RECENT_LIMIT: usize = 12;
const NAVIGATOR_GRAPH_VIEW_SWITCHER_HEIGHT: f32 = 28.0;

fn compact_host_panel_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= HOST_PANEL_LABEL_MAX_CHARS {
        return trimmed.to_string();
    }
    let shortened: String = trimmed
        .chars()
        .take(HOST_PANEL_LABEL_MAX_CHARS.saturating_sub(1))
        .collect();
    format!("{shortened}…")
}

fn show_host_contents_with_cross_axis_margins(
    ui: &mut egui::Ui,
    host_layout: &WorkbenchHostLayout,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    match host_layout.form_factor {
        WorkbenchHostFormFactor::Sidebar => {
            if host_layout.cross_axis_margin_start_px > 0.0 {
                ui.add_space(host_layout.cross_axis_margin_start_px);
            }
            add_contents(ui);
            if host_layout.cross_axis_margin_end_px > 0.0 {
                ui.add_space(host_layout.cross_axis_margin_end_px);
            }
        }
        WorkbenchHostFormFactor::Toolbar => {
            ui.horizontal(|ui| {
                if host_layout.cross_axis_margin_start_px > 0.0 {
                    ui.add_space(host_layout.cross_axis_margin_start_px);
                }
                ui.vertical(|ui| add_contents(ui));
                if host_layout.cross_axis_margin_end_px > 0.0 {
                    ui.add_space(host_layout.cross_axis_margin_end_px);
                }
            });
        }
    }
}

fn host_uses_overlay_layout(host_layout: &WorkbenchHostLayout) -> bool {
    host_layout.cross_axis_margin_start_px > 0.0 || host_layout.cross_axis_margin_end_px > 0.0
}

fn host_panel_extent(host_layout: &WorkbenchHostLayout, available_rect: egui::Rect) -> f32 {
    let (axis_extent, min_extent) = match host_layout.form_factor {
        WorkbenchHostFormFactor::Sidebar => (available_rect.width(), HOST_PANEL_MAX_FLOOR),
        WorkbenchHostFormFactor::Toolbar => (available_rect.height(), HOST_PANEL_MAX_FLOOR),
    };
    let max_extent = (axis_extent * HOST_PANEL_MAX_FRACTION).max(min_extent);
    (axis_extent * host_layout.size_fraction).clamp(min_extent, max_extent)
}

fn clamped_cross_axis_margins(
    available_extent: f32,
    start_margin: f32,
    end_margin: f32,
) -> (f32, f32) {
    let available_extent = available_extent.max(0.0);
    let max_total_margin = (available_extent - 1.0).max(0.0);
    let requested_total_margin = (start_margin.max(0.0) + end_margin.max(0.0)).max(0.0);
    if requested_total_margin <= max_total_margin || requested_total_margin <= f32::EPSILON {
        return (start_margin.max(0.0), end_margin.max(0.0));
    }

    let scale = max_total_margin / requested_total_margin;
    (start_margin.max(0.0) * scale, end_margin.max(0.0) * scale)
}

fn host_overlay_rect(
    host_layout: &WorkbenchHostLayout,
    available_rect: egui::Rect,
) -> Option<egui::Rect> {
    if !host_uses_overlay_layout(host_layout) {
        return None;
    }

    let host_extent = host_panel_extent(host_layout, available_rect);
    match host_layout.form_factor {
        WorkbenchHostFormFactor::Sidebar => {
            let (start_margin, end_margin) = clamped_cross_axis_margins(
                available_rect.height(),
                host_layout.cross_axis_margin_start_px,
                host_layout.cross_axis_margin_end_px,
            );
            let top = available_rect.top() + start_margin;
            let bottom = available_rect.bottom() - end_margin;
            let (left, right) = match host_layout.anchor_edge {
                AnchorEdge::Left => (available_rect.left(), available_rect.left() + host_extent),
                AnchorEdge::Right | AnchorEdge::Top | AnchorEdge::Bottom => {
                    (available_rect.right() - host_extent, available_rect.right())
                }
            };
            Some(egui::Rect::from_min_max(
                egui::pos2(left, top),
                egui::pos2(right, bottom.max(top + 1.0)),
            ))
        }
        WorkbenchHostFormFactor::Toolbar => {
            let (start_margin, end_margin) = clamped_cross_axis_margins(
                available_rect.width(),
                host_layout.cross_axis_margin_start_px,
                host_layout.cross_axis_margin_end_px,
            );
            let left = available_rect.left() + start_margin;
            let right = available_rect.right() - end_margin;
            let (top, bottom) = match host_layout.anchor_edge {
                AnchorEdge::Bottom => (
                    available_rect.bottom() - host_extent,
                    available_rect.bottom(),
                ),
                AnchorEdge::Top | AnchorEdge::Left | AnchorEdge::Right => {
                    (available_rect.top(), available_rect.top() + host_extent)
                }
            };
            Some(egui::Rect::from_min_max(
                egui::pos2(left, top),
                egui::pos2(right.max(left + 1.0), bottom),
            ))
        }
    }
}

fn update_workbench_navigation_geometry(
    graph_app: &mut GraphBrowserApp,
    content_rect: egui::Rect,
    occluding_host_rects: Vec<egui::Rect>,
) {
    graph_app
        .workspace
        .graph_runtime
        .workbench_navigation_geometry = Some(WorkbenchNavigationGeometry::from_content_rect(
        content_rect,
        occluding_host_rects,
    ));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchLayerState {
    GraphOnly,
    GraphOverlayActive,
    WorkbenchOverlayActive,
    WorkbenchActive,
    WorkbenchPinned,
    WorkbenchOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChromeExposurePolicy {
    GraphOnly,
    GraphWithOverlay,
    GraphPlusWorkbenchOverlay,
    GraphPlusWorkbenchHost,
    GraphPlusWorkbenchHostPinned,
    WorkbenchOnly,
}

impl WorkbenchLayerState {
    pub(crate) fn chrome_policy(self) -> ChromeExposurePolicy {
        match self {
            Self::GraphOnly => ChromeExposurePolicy::GraphOnly,
            Self::GraphOverlayActive => ChromeExposurePolicy::GraphWithOverlay,
            Self::WorkbenchOverlayActive => ChromeExposurePolicy::GraphPlusWorkbenchOverlay,
            Self::WorkbenchActive => ChromeExposurePolicy::GraphPlusWorkbenchHost,
            Self::WorkbenchPinned => ChromeExposurePolicy::GraphPlusWorkbenchHostPinned,
            Self::WorkbenchOnly => ChromeExposurePolicy::WorkbenchOnly,
        }
    }
}

pub(crate) fn workbench_layer_state_from_tree(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) -> WorkbenchLayerState {
    let has_hosted_workbench = tree_has_hosted_workbench(tiles_tree);
    if graph_app.workbench_display_mode() == WorkbenchDisplayMode::Dedicated && has_hosted_workbench
    {
        WorkbenchLayerState::WorkbenchOnly
    } else if graph_app.workbench_overlay_visible() && has_hosted_workbench {
        WorkbenchLayerState::WorkbenchOverlayActive
    } else if graph_app.workbench_host_pinned() {
        WorkbenchLayerState::WorkbenchPinned
    } else if has_hosted_workbench {
        WorkbenchLayerState::WorkbenchActive
    } else if graph_app.chrome_overlay_active() {
        WorkbenchLayerState::GraphOverlayActive
    } else {
        WorkbenchLayerState::GraphOnly
    }
}

fn tree_has_hosted_workbench(tiles_tree: &Tree<TileKind>) -> bool {
    let graph_pane_count = tiles_tree
        .tiles
        .iter()
        .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))))
        .count();

    tiles_tree
        .tiles
        .iter()
        .any(|(_, tile)| matches!(tile, Tile::Pane(kind) if !matches!(kind, TileKind::Graph(_))))
        || graph_pane_count > 1
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchPaneKind {
    Graph { view_id: GraphViewId },
    Node { node_key: NodeKey },
    Tool { kind: ToolPaneState },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkbenchPaneEntry {
    pub(crate) pane_id: PaneId,
    pub(crate) kind: WorkbenchPaneKind,
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) arrangement_memberships: Vec<String>,
    pub(crate) semantic_tab_affordance: Option<semantic_tabs::SemanticTabAffordance>,
    pub(crate) node_viewer_summary: Option<WorkbenchNodeViewerSummary>,
    pub(crate) presentation_mode: PanePresentationMode,
    pub(crate) is_active: bool,
    pub(crate) closable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkbenchNodeViewerSummary {
    pub(crate) effective_viewer_id: Option<String>,
    pub(crate) viewer_override: Option<String>,
    pub(crate) viewer_switch_reason: ViewerSwitchReason,
    pub(crate) render_mode: TileRenderMode,
    pub(crate) runtime_blocked: bool,
    pub(crate) runtime_crashed: bool,
    pub(crate) fallback_reason: Option<String>,
    pub(crate) available_viewer_ids: Vec<String>,
    /// Verso's resolved route for the node, including engine,
    /// reason, and owner. `None` means verso does not route this
    /// content (specialized non-web viewers: images, PDFs, local
    /// files) — the registry's viewer selection at
    /// `effective_viewer_id` is authoritative in that case.
    pub(crate) verso_route: Option<::verso::VersoResolvedRoute>,
}

/// A single entry in the active tile group's graphlet roster shown by the omnibar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GraphletRosterEntry {
    pub(crate) node_key: NodeKey,
    pub(crate) title: String,
    /// True when `NodeLifecycle::Cold` — shown with ○ badge; activating opens a tile.
    pub(crate) is_cold: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WorkbenchChromeProjection {
    pub(crate) layer_state: WorkbenchLayerState,
    pub(crate) chrome_policy: ChromeExposurePolicy,
    pub(crate) host_layout: WorkbenchHostLayout,
    pub(crate) host_layouts: Vec<WorkbenchHostLayout>,
    pub(crate) active_graph_view: Option<(GraphViewId, String)>,
    pub(crate) extra_graph_views: Vec<(GraphViewId, String)>,
    pub(crate) active_pane_title: Option<String>,
    pub(crate) active_frame_name: Option<String>,
    pub(crate) saved_frame_names: Vec<String>,
    pub(crate) navigator_groups: Vec<WorkbenchNavigatorGroup>,
    pub(crate) pane_entries: Vec<WorkbenchPaneEntry>,
    pub(crate) tree_root: Option<WorkbenchChromeNode>,
    /// Full graphlet roster (warm ● + cold ○) for the active node pane's graphlet.
    /// Empty when the active pane is not a node pane or when the node has no graphlet peers.
    pub(crate) active_graphlet_roster: Vec<GraphletRosterEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchHostFormFactor {
    Toolbar,
    Sidebar,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WorkbenchHostLayout {
    pub(crate) host: SurfaceHostId,
    pub(crate) anchor_edge: AnchorEdge,
    pub(crate) form_factor: WorkbenchHostFormFactor,
    pub(crate) configured_scope: NavigatorHostScope,
    pub(crate) resolved_scope: NavigatorHostScope,
    pub(crate) size_fraction: f32,
    pub(crate) cross_axis_margin_start_px: f32,
    pub(crate) cross_axis_margin_end_px: f32,
    pub(crate) resizable: bool,
}

impl WorkbenchHostLayout {
    fn default_workbench_navigator(graph_app: &GraphBrowserApp) -> Self {
        Self::default_for_host(graph_app.preferred_default_navigator_surface_host(), false)
    }

    fn default_for_host(host: SurfaceHostId, prefer_workbench_scope: bool) -> Self {
        let anchor_edge = default_anchor_edge_for_host(&host);
        let configured_scope = NavigatorHostScope::Both;
        Self {
            host,
            anchor_edge,
            form_factor: default_form_factor_for_edge(anchor_edge),
            configured_scope,
            resolved_scope: resolve_navigator_host_scope(configured_scope, prefer_workbench_scope),
            size_fraction: HOST_PANEL_MAX_FRACTION * 0.75,
            cross_axis_margin_start_px: 0.0,
            cross_axis_margin_end_px: 0.0,
            resizable: true,
        }
    }

    fn from_constraint(
        host: SurfaceHostId,
        configured_scope: NavigatorHostScope,
        prefer_workbench_scope: bool,
        constraint: &WorkbenchLayoutConstraint,
    ) -> Option<Self> {
        let WorkbenchLayoutConstraint::AnchoredSplit {
            anchor_edge,
            anchor_size_fraction,
            cross_axis_margin_start_px,
            cross_axis_margin_end_px,
            resizable,
            ..
        } = constraint
        else {
            return None;
        };
        Some(Self {
            host,
            anchor_edge: *anchor_edge,
            form_factor: default_form_factor_for_edge(*anchor_edge),
            configured_scope,
            resolved_scope: resolve_navigator_host_scope(configured_scope, prefer_workbench_scope),
            size_fraction: *anchor_size_fraction,
            cross_axis_margin_start_px: *cross_axis_margin_start_px,
            cross_axis_margin_end_px: *cross_axis_margin_end_px,
            resizable: *resizable,
        })
    }

    fn layouts_from_runtime(
        graph_app: &GraphBrowserApp,
        prefer_workbench_scope: bool,
    ) -> Vec<Self> {
        let mut layouts = graph_app
            .workspace
            .workbench_session
            .active_layout_constraints
            .iter()
            .filter_map(|(surface_host, constraint)| match surface_host {
                SurfaceHostId::Navigator(_) => Self::from_constraint(
                    surface_host.clone(),
                    graph_app.navigator_host_scope(surface_host),
                    prefer_workbench_scope,
                    constraint,
                ),
                SurfaceHostId::Role(_) => None,
            })
            .collect::<Vec<_>>();

        if layouts.is_empty() {
            layouts.push(Self::default_for_host(
                graph_app.preferred_default_navigator_surface_host(),
                prefer_workbench_scope,
            ));
        }

        layouts.sort_by_key(|layout| anchor_edge_priority(layout.anchor_edge));
        layouts
    }
}

fn resolve_navigator_host_scope(
    configured_scope: NavigatorHostScope,
    prefer_workbench_scope: bool,
) -> NavigatorHostScope {
    configured_scope.resolve(prefer_workbench_scope)
}

fn host_scope_label(scope: NavigatorHostScope) -> &'static str {
    match scope {
        NavigatorHostScope::Both => "Both",
        NavigatorHostScope::GraphOnly => "Graph",
        NavigatorHostScope::WorkbenchOnly => "Workbench",
        NavigatorHostScope::Auto => "Auto",
    }
}

fn host_shows_graph_scope(host_layout: &WorkbenchHostLayout) -> bool {
    matches!(
        host_layout.resolved_scope,
        NavigatorHostScope::Both | NavigatorHostScope::GraphOnly
    )
}

fn host_shows_workbench_scope(host_layout: &WorkbenchHostLayout) -> bool {
    matches!(
        host_layout.resolved_scope,
        NavigatorHostScope::Both | NavigatorHostScope::WorkbenchOnly
    )
}

fn graph_view_switcher_visible(projection: &WorkbenchChromeProjection) -> bool {
    projection.active_graph_view.is_some() && !projection.extra_graph_views.is_empty()
}

#[derive(Clone, Debug, PartialEq)]
struct GraphScopeControlSummary {
    view_id: GraphViewId,
    view_label: String,
    active_filter_chip_labels: Vec<String>,
    lens_label: String,
    lens_id: String,
    dimension: crate::app::ViewDimension,
    dimension_label: String,
    dimension_tooltip: String,
    semantic_depth_active: bool,
    physics_profile_id: String,
    physics_profile_label: String,
    physics_running: bool,
}

fn active_graph_scope_control_summary(
    projection: &WorkbenchChromeProjection,
    graph_app: &GraphBrowserApp,
) -> Option<GraphScopeControlSummary> {
    let (view_id, view_label) = projection.active_graph_view.as_ref()?;
    let view = graph_app.workspace.graph_runtime.views.get(view_id)?;
    let (dimension_label, dimension_tooltip, semantic_depth_active) =
        crate::app::view_dimension_summary(&view.dimension);

    Some(GraphScopeControlSummary {
        view_id: *view_id,
        view_label: view_label.clone(),
        active_filter_chip_labels: graph_scope_filter_chip_labels(view),
        lens_label: view.resolved_lens_display_name().to_string(),
        lens_id: view
            .resolved_lens_id()
            .unwrap_or(crate::registries::atomic::lens::LENS_ID_DEFAULT)
            .to_string(),
        dimension: view.dimension.clone(),
        dimension_label,
        dimension_tooltip,
        semantic_depth_active,
        physics_profile_id: view
            .resolved_physics_profile_id()
            .unwrap_or(crate::registries::atomic::lens::PHYSICS_ID_DEFAULT)
            .to_string(),
        physics_profile_label: view.resolved_physics_profile().name.clone(),
        physics_running: graph_app.workspace.graph_runtime.physics.is_running,
    })
}

fn graph_scope_filter_chip_labels(view: &crate::app::GraphViewState) -> Vec<String> {
    fn push_labels(
        expr: &crate::model::graph::filter::FacetExpr,
        labels: &mut Vec<String>,
        flatten_top_level: bool,
    ) {
        match expr {
            crate::model::graph::filter::FacetExpr::And(exprs) if flatten_top_level => {
                for child in exprs {
                    push_labels(child, labels, false);
                }
            }
            _ => labels.push(expr.display_label()),
        }
    }

    let mut labels = Vec::new();
    if let Some(expr) = view.effective_filter_expr() {
        push_labels(expr, &mut labels, true);
    }
    labels
}

fn graph_scope_slot_strip_slots(
    graph_app: &GraphBrowserApp,
) -> Vec<crate::shell::desktop::ui::overview_plane::OverviewSlotSnapshot> {
    crate::shell::desktop::ui::overview_plane::sorted_slot_snapshots(graph_app)
        .into_iter()
        .filter(|slot| !slot.archived)
        .collect()
}

fn render_view_dimension_menu(
    ui: &mut egui::Ui,
    summary: &GraphScopeControlSummary,
    actions: &mut Vec<WorkbenchHostAction>,
) {
    let two_d_selected = matches!(summary.dimension, crate::app::ViewDimension::TwoD);
    if ui
        .selectable_label(two_d_selected, "2D")
        .on_hover_text("Restore the focused graph view to the standard 2D planar mode")
        .clicked()
    {
        actions.push(WorkbenchHostAction::SetViewDimension {
            view_id: summary.view_id,
            dimension: crate::app::ViewDimension::TwoD,
        });
        ui.close();
    }

    let twopointfive_dimension =
        crate::app::default_view_dimension_for_mode(crate::app::ThreeDMode::TwoPointFive);
    if ui
        .selectable_label(summary.dimension == twopointfive_dimension, "2.5D")
        .on_hover_text("Use the generic 2.5D projection preset with recency-derived depth")
        .clicked()
    {
        actions.push(WorkbenchHostAction::SetViewDimension {
            view_id: summary.view_id,
            dimension: twopointfive_dimension,
        });
        ui.close();
    }

    let isometric_dimension =
        crate::app::default_view_dimension_for_mode(crate::app::ThreeDMode::Isometric);
    if ui
        .selectable_label(summary.dimension == isometric_dimension, "Isometric")
        .on_hover_text("Use the isometric projection preset with BFS-derived depth layering")
        .clicked()
    {
        actions.push(WorkbenchHostAction::SetViewDimension {
            view_id: summary.view_id,
            dimension: isometric_dimension,
        });
        ui.close();
    }

    let standard_dimension =
        crate::app::default_view_dimension_for_mode(crate::app::ThreeDMode::Standard);
    if ui
        .selectable_label(summary.dimension == standard_dimension, "Standard 3D")
        .on_hover_text("Persist the full standard 3D view state without semantic depth layering")
        .clicked()
    {
        actions.push(WorkbenchHostAction::SetViewDimension {
            view_id: summary.view_id,
            dimension: standard_dimension,
        });
        ui.close();
    }

    ui.separator();

    let preset_label = if summary.semantic_depth_active {
        "Restore prior view"
    } else {
        "UDC depth preset"
    };
    let preset_hover = if summary.semantic_depth_active {
        "Restore the graph view dimension that was active before the UDC depth preset was applied"
    } else {
        "Apply the reversible UDC semantic-depth preset without losing the previous view dimension"
    };
    if ui
        .selectable_label(summary.semantic_depth_active, preset_label)
        .on_hover_text(preset_hover)
        .clicked()
    {
        actions.push(WorkbenchHostAction::ToggleSemanticDepthView {
            view_id: summary.view_id,
        });
        ui.close();
    }
}

fn render_graph_view_slot_strip(
    ui: &mut egui::Ui,
    summary: &GraphScopeControlSummary,
    graph_app: &GraphBrowserApp,
    actions: &mut Vec<WorkbenchHostAction>,
) {
    let slots = graph_scope_slot_strip_slots(graph_app);
    if slots.is_empty() {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Slots").small().weak());
        for slot in slots {
            let selected = slot.view_id == summary.view_id;
            let response = ui
                .selectable_label(selected, compact_host_panel_text(slot.name.as_str()))
                .on_hover_text(format!(
                    "Click to focus {}. Double-click to open it in the workbench. Right-click for slot actions.",
                    slot.name
                ));
            let clicked = response.clicked();
            let double_clicked = response.double_clicked();
            let view_id = slot.view_id;
            let row = slot.row;
            let col = slot.col;
            response.context_menu(|ui| {
                ui.label(RichText::new(format!("{} · r{} · c{}", slot.name, row, col)).small().weak());
                if ui.button("Open in workbench").clicked() {
                    actions.push(WorkbenchHostAction::OpenGraphView(view_id));
                    ui.close();
                }
                ui.separator();
                for (label, next_row, next_col) in [
                    ("Move left", row, col - 1),
                    ("Move right", row, col + 1),
                    ("Move up", row - 1, col),
                    ("Move down", row + 1, col),
                ] {
                    if ui.button(label).clicked() {
                        actions.push(WorkbenchHostAction::MoveGraphViewSlot {
                            view_id,
                            row: next_row,
                            col: next_col,
                        });
                        ui.close();
                    }
                }
            });
            if clicked {
                actions.push(WorkbenchHostAction::FocusGraphView(view_id));
            }
            if double_clicked {
                actions.push(WorkbenchHostAction::OpenGraphView(view_id));
            }
        }
    });
}

fn render_graph_scope_filter_chips(
    ui: &mut egui::Ui,
    summary: &GraphScopeControlSummary,
    actions: &mut Vec<WorkbenchHostAction>,
) {
    if summary.active_filter_chip_labels.is_empty() {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Filters").small().weak());
        for label in &summary.active_filter_chip_labels {
            if ui
                .small_button(format!("{} x", compact_host_panel_text(label.as_str())))
                .on_hover_text("Clear the active graph-view filter")
                .clicked()
            {
                actions.push(WorkbenchHostAction::ClearGraphViewFilter {
                    view_id: summary.view_id,
                });
            }
        }
    });
}

fn graph_scope_sync_status() -> (String, egui::Color32, String) {
    // Resolve the active theme's status palette once so all three
    // sync-status branches read the same theme surface. This replaces
    // the hardcoded (140,145,150) / (210,175,70) / (110,190,130) triple
    // that used to live inline — users who swap themes now see their
    // theme's status colors applied here too.
    let theme =
        crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(None).tokens;

    if !crate::mods::verse::is_initialized() {
        return (
            "Sync off".to_string(),
            theme.status_neutral,
            "Sync is unavailable in this runtime.".to_string(),
        );
    }

    let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
    if peers.is_empty() {
        (
            "Sync ready".to_string(),
            theme.status_warning,
            "Sync is ready but there are no trusted peers connected.".to_string(),
        )
    } else {
        (
            format!("Sync {}", peers.len()),
            theme.status_success,
            format!(
                "Sync connected with {} trusted peer{}.",
                peers.len(),
                if peers.len() == 1 { "" } else { "s" }
            ),
        )
    }
}

fn rendered_graph_scope_host_exists(projection: &WorkbenchChromeProjection) -> bool {
    projection.visible() && projection.host_layouts.iter().any(host_shows_graph_scope)
}

fn graph_scope_requires_fallback_toolbar_host(projection: &WorkbenchChromeProjection) -> bool {
    projection.active_graph_view.is_some() && !rendered_graph_scope_host_exists(projection)
}

fn render_graph_view_switcher(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    projection: &WorkbenchChromeProjection,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Views").small().weak());
        if let Some((_view_id, label)) = &projection.active_graph_view {
            let _ = ui.selectable_label(true, label.as_str());
        }
        for (view_id, label) in &projection.extra_graph_views {
            if ui.selectable_label(false, label.as_str()).clicked() {
                graph_app.enqueue_workbench_intent(WorkbenchIntent::FocusGraphView {
                    view_id: *view_id,
                });
            }
        }
    });
}

fn render_graph_scope_controls(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    projection: &WorkbenchChromeProjection,
    actions: &mut Vec<WorkbenchHostAction>,
    show_clear_data_confirm: &mut bool,
    id_namespace: &'static str,
) -> bool {
    let Some(summary) = active_graph_scope_control_summary(projection, graph_app) else {
        return false;
    };

    let (sync_label, sync_color, sync_tooltip) = graph_scope_sync_status();
    let lens_input_id = egui::Id::new((id_namespace, "graph_scope_lens_input", summary.view_id));
    let mut lens_input = ui
        .ctx()
        .data_mut(|data| data.get_persisted::<String>(lens_input_id))
        .unwrap_or_else(|| summary.lens_id.clone());

    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(format!(
                "View: {}",
                compact_host_panel_text(summary.view_label.as_str())
            ))
            .small()
            .weak(),
        )
        .on_hover_text(format!("Focused graph view target: {}", summary.view_label));

        if ui
            .small_button("Fit")
            .on_hover_text("Fit the focused graph view to the current content")
            .clicked()
        {
            actions.push(WorkbenchHostAction::RequestFitToScreen);
        }

        ui.menu_button(
            format!(
                "Lens: {}",
                compact_host_panel_text(summary.lens_label.as_str())
            ),
            |ui| {
                ui.label(
                    RichText::new(format!("Current lens ID: {}", summary.lens_id))
                        .small()
                        .weak(),
                );
                ui.horizontal_wrapped(|ui| {
                    let default_selected =
                        summary.lens_id == crate::registries::atomic::lens::LENS_ID_DEFAULT;
                    if ui
                        .selectable_label(default_selected, "Default")
                        .on_hover_text(crate::registries::atomic::lens::LENS_ID_DEFAULT)
                        .clicked()
                    {
                        actions.push(WorkbenchHostAction::SetViewLensId {
                            view_id: summary.view_id,
                            lens_id: crate::registries::atomic::lens::LENS_ID_DEFAULT.to_string(),
                        });
                        lens_input = crate::registries::atomic::lens::LENS_ID_DEFAULT.to_string();
                        ui.close();
                    }
                    let semantic_selected = summary.lens_id
                        == crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY;
                    if ui
                        .selectable_label(semantic_selected, "Semantic Overlay")
                        .on_hover_text(crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY)
                        .clicked()
                    {
                        actions.push(WorkbenchHostAction::SetViewLensId {
                            view_id: summary.view_id,
                            lens_id: crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY
                                .to_string(),
                        });
                        lens_input =
                            crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY.to_string();
                        ui.close();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Lens ID").small().weak());
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut lens_input)
                            .desired_width(120.0)
                            .hint_text("lens:..."),
                    );
                    let submit_with_enter = response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    if ui.small_button("Apply").clicked() || submit_with_enter {
                        let requested = lens_input.trim();
                        if !requested.is_empty() {
                            actions.push(WorkbenchHostAction::SetViewLensId {
                                view_id: summary.view_id,
                                lens_id: requested.to_string(),
                            });
                            ui.close();
                        }
                    }
                    if ui.small_button("Reset").clicked() {
                        actions.push(WorkbenchHostAction::SetViewLensId {
                            view_id: summary.view_id,
                            lens_id: crate::registries::atomic::lens::LENS_ID_DEFAULT.to_string(),
                        });
                        lens_input = crate::registries::atomic::lens::LENS_ID_DEFAULT.to_string();
                        ui.close();
                    }
                });
            },
        );

        ui.menu_button(format!("View: {}", summary.dimension_label), |ui| {
            render_view_dimension_menu(ui, &summary, actions);
        })
        .response
        .on_hover_text(summary.dimension_tooltip.as_str());

        let semantic_chip_label = if summary.semantic_depth_active {
            "Restore"
        } else {
            "Depth"
        };
        if ui
            .selectable_label(summary.semantic_depth_active, semantic_chip_label)
            .on_hover_text(if summary.semantic_depth_active {
                "Restore the view dimension that was active before UDC depth layering"
            } else {
                "Apply the reversible UDC semantic-depth preset"
            })
            .clicked()
        {
            actions.push(WorkbenchHostAction::ToggleSemanticDepthView {
                view_id: summary.view_id,
            });
        }

        ui.menu_button(
            format!(
                "Physics: {}",
                compact_host_panel_text(summary.physics_profile_label.as_str())
            ),
            |ui| {
                let toggle_label = if summary.physics_running {
                    "Pause"
                } else {
                    "Run"
                };
                if ui
                    .button(toggle_label)
                    .on_hover_text("Toggle the shared force-directed simulation runtime")
                    .clicked()
                {
                    actions.push(WorkbenchHostAction::TogglePhysics);
                    ui.close();
                }
                if ui
                    .button("Reheat")
                    .on_hover_text("Kick the force-directed layout back into motion")
                    .clicked()
                {
                    actions.push(WorkbenchHostAction::ReheatPhysics);
                    ui.close();
                }
                ui.separator();
                for descriptor in crate::registries::atomic::lens::physics_profile_descriptors() {
                    let selected = summary.physics_profile_id == descriptor.id;
                    let response = ui.selectable_label(selected, descriptor.display_name.as_str());
                    let response = response.on_hover_text(descriptor.summary.as_str());
                    if response.clicked() {
                        actions.push(WorkbenchHostAction::SetPhysicsProfile {
                            profile_id: descriptor.id.clone(),
                        });
                        ui.close();
                    }
                }
            },
        );

        ui.label(RichText::new(sync_label).small().color(sync_color))
            .on_hover_text(sync_tooltip);

        ui.menu_button("More", |ui| {
            if ui.button("Clear graph and saved data").clicked() {
                *show_clear_data_confirm = true;
                ui.close();
            }
        });
    });

    render_graph_view_slot_strip(ui, &summary, graph_app, actions);
    render_graph_scope_filter_chips(ui, &summary, actions);

    ui.ctx()
        .data_mut(|data| data.insert_persisted(lens_input_id, lens_input));
    true
}

pub(crate) fn render_fallback_graph_scope_toolbar_host(
    _ctx: &egui::Context,
    root_ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    projection: &WorkbenchChromeProjection,
    show_clear_data_confirm: &mut bool,
) {
    if !graph_scope_requires_fallback_toolbar_host(projection) {
        return;
    }

    let mut post_host_actions = Vec::new();
    Panel::top("navigator_graph_scope_toolbar_host_fallback")
        .exact_size(NAVIGATOR_GRAPH_VIEW_SWITCHER_HEIGHT + 28.0)
        .show_inside(root_ui, |ui| {
            if graph_view_switcher_visible(projection) {
                render_graph_view_switcher(ui, graph_app, projection);
                ui.separator();
            }
            let _ = render_graph_scope_controls(
                ui,
                graph_app,
                projection,
                &mut post_host_actions,
                show_clear_data_confirm,
                "fallback_graph_scope_host",
            );
        });

    for action in post_host_actions {
        apply_workbench_host_action(action, graph_app, tiles_tree, graph_tree);
    }
}

fn anchor_edge_priority(anchor_edge: AnchorEdge) -> usize {
    match anchor_edge {
        AnchorEdge::Top => 0,
        AnchorEdge::Bottom => 1,
        AnchorEdge::Left => 2,
        AnchorEdge::Right => 3,
    }
}

fn default_anchor_edge_for_host(surface_host: &SurfaceHostId) -> AnchorEdge {
    match surface_host {
        SurfaceHostId::Navigator(NavigatorHostId::Top) => AnchorEdge::Top,
        SurfaceHostId::Navigator(NavigatorHostId::Bottom) => AnchorEdge::Bottom,
        SurfaceHostId::Navigator(NavigatorHostId::Left) => AnchorEdge::Left,
        SurfaceHostId::Navigator(NavigatorHostId::Right) | SurfaceHostId::Role(_) => {
            AnchorEdge::Right
        }
    }
}

fn default_form_factor_for_edge(anchor_edge: AnchorEdge) -> WorkbenchHostFormFactor {
    match anchor_edge {
        AnchorEdge::Top | AnchorEdge::Bottom => WorkbenchHostFormFactor::Toolbar,
        AnchorEdge::Left | AnchorEdge::Right => WorkbenchHostFormFactor::Sidebar,
    }
}

fn host_display_name(surface_host: &SurfaceHostId) -> &'static str {
    match surface_host {
        SurfaceHostId::Navigator(NavigatorHostId::Top) => "Top Navigator Host",
        SurfaceHostId::Navigator(NavigatorHostId::Bottom) => "Bottom Navigator Host",
        SurfaceHostId::Navigator(NavigatorHostId::Left) => "Left Navigator Host",
        SurfaceHostId::Navigator(NavigatorHostId::Right) => "Right Navigator Host",
        SurfaceHostId::Role(_) => "Workbench Host",
    }
}

fn host_constraint_label(host_layout: &WorkbenchHostLayout) -> String {
    format!(
        "{} - {:?} - {}%",
        host_scope_label(host_layout.resolved_scope),
        host_layout.anchor_edge,
        (host_layout.size_fraction * 100.0).round() as i32
    )
}

fn constraint_from_host_layout(host_layout: &WorkbenchHostLayout) -> WorkbenchLayoutConstraint {
    anchored_constraint_for_host_layout(
        host_layout,
        host_layout.anchor_edge,
        host_layout.size_fraction,
        host_layout.cross_axis_margin_start_px,
        host_layout.cross_axis_margin_end_px,
        host_layout.resizable,
    )
}

fn anchored_constraint_for_host_layout(
    host_layout: &WorkbenchHostLayout,
    anchor_edge: AnchorEdge,
    size_fraction: f32,
    cross_axis_margin_start_px: f32,
    cross_axis_margin_end_px: f32,
    resizable: bool,
) -> WorkbenchLayoutConstraint {
    WorkbenchLayoutConstraint::AnchoredSplit {
        surface_host: host_layout.host.clone(),
        anchor_edge,
        anchor_size_fraction: size_fraction,
        cross_axis_margin_start_px,
        cross_axis_margin_end_px,
        resizable,
    }
}

fn host_panel_id(surface_host: &SurfaceHostId) -> String {
    format!(
        "workbench_host_{}",
        surface_host.to_string().replace(':', "_").to_lowercase()
    )
}

fn is_host_configuring(graph_app: &GraphBrowserApp, surface_host: &SurfaceHostId) -> bool {
    matches!(
        &graph_app.workspace.workbench_session.ux_config_mode,
        UxConfigMode::Configuring { surface_host: configuring_host } if configuring_host == surface_host
    )
}

fn missing_navigator_hosts(host_layouts: &[WorkbenchHostLayout]) -> Vec<SurfaceHostId> {
    let present = host_layouts
        .iter()
        .map(|layout| layout.host.clone())
        .collect::<HashSet<_>>();
    [
        SurfaceHostId::Navigator(NavigatorHostId::Top),
        SurfaceHostId::Navigator(NavigatorHostId::Bottom),
        SurfaceHostId::Navigator(NavigatorHostId::Left),
        SurfaceHostId::Navigator(NavigatorHostId::Right),
    ]
    .into_iter()
    .filter(|host| !present.contains(host))
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfiguringOverlaySpec {
    edge_targets: Vec<AnchorEdge>,
    has_unconstrain_target: bool,
    has_size_slider: bool,
    margin_handle_labels: Vec<&'static str>,
}

fn configuring_overlay_spec(is_configuring: bool) -> Option<ConfiguringOverlaySpec> {
    is_configuring.then(|| ConfiguringOverlaySpec {
        edge_targets: vec![
            AnchorEdge::Top,
            AnchorEdge::Bottom,
            AnchorEdge::Left,
            AnchorEdge::Right,
        ],
        has_unconstrain_target: true,
        has_size_slider: true,
        margin_handle_labels: vec!["Start", "End"],
    })
}

fn first_use_prompt_visible(graph_app: &GraphBrowserApp, surface_host: &SurfaceHostId) -> bool {
    if graph_app.is_first_use_prompt_suppressed_for_session(surface_host) {
        return false;
    }
    if graph_app
        .workbench_profile()
        .layout_constraints
        .contains_key(surface_host)
    {
        return false;
    }

    graph_app
        .workbench_profile()
        .first_use_policies
        .get(surface_host)
        .map(|policy| {
            policy.outcome.is_none()
                || policy
                    .outcome
                    .as_ref()
                    .is_some_and(|outcome| matches!(outcome, FirstUseOutcome::ConfigureNow))
        })
        .unwrap_or(true)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkbenchNavigatorGroup {
    pub(crate) section: WorkbenchNavigatorSection,
    pub(crate) title: String,
    pub(crate) is_highlighted: bool,
    pub(crate) members: Vec<WorkbenchNavigatorMember>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchNavigatorSection {
    Workbench,
    Folders,
    Domain,
    Recent,
    Unrelated,
    Imported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkbenchNavigatorMember {
    pub(crate) node_key: NodeKey,
    pub(crate) title: String,
    pub(crate) is_selected: bool,
    pub(crate) row_key: Option<String>,
    pub(crate) target_url: Option<String>,
    /// True when the node's lifecycle is `Cold` — has graph edges but no live tile.
    /// Rendered with a ○ badge in the Navigator; double-click activates.
    pub(crate) is_cold: bool,
    /// Nesting depth in the GraphTree topology (0 = root). Used for indented
    /// tree-style rendering when the navigator is driven by GraphTree projections.
    pub(crate) depth: usize,
    /// Whether this member's children are expanded in the tree view.
    pub(crate) is_expanded: bool,
    /// Whether this member has children in the GraphTree topology.
    pub(crate) has_children: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkbenchChromeNode {
    Pane(WorkbenchPaneEntry),
    Tabs {
        tile_id: TileId,
        label: String,
        children: Vec<WorkbenchChromeNode>,
    },
    Split {
        tile_id: TileId,
        label: String,
        children: Vec<WorkbenchChromeNode>,
    },
    Grid {
        tile_id: TileId,
        label: String,
        children: Vec<WorkbenchChromeNode>,
    },
}

impl WorkbenchChromeProjection {
    pub(crate) fn from_tree(
        graph_app: &GraphBrowserApp,
        tiles_tree: &Tree<TileKind>,
        active_pane: Option<PaneId>,
    ) -> Self {
        let (active_graph_view, extra_graph_views) = graph_view_switcher_projection(graph_app);
        let projection_view_id = graph_view_id_for_navigation(graph_app, tiles_tree);
        let arrangement_memberships = pane_arrangement_memberships(graph_app);
        let mut saved_frame_names = graph_app
            .list_workspace_layout_names()
            .into_iter()
            .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
            .collect::<Vec<_>>();
        for frame_name in arrangement_memberships
            .values()
            .flatten()
            .filter_map(|membership| membership.strip_prefix("Frame: "))
        {
            if !saved_frame_names
                .iter()
                .any(|existing| existing == frame_name)
            {
                saved_frame_names.push(frame_name.to_string());
            }
        }
        saved_frame_names.sort();
        saved_frame_names.dedup();
        let pane_entries = tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(kind) => Some(pane_entry_for_tile(
                    graph_app,
                    tiles_tree,
                    kind,
                    active_pane,
                    &arrangement_memberships,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let has_hosted_workbench = tree_has_hosted_workbench(tiles_tree);
        let active_pane_prefers_workbench_scope = active_pane
            .and_then(|pane_id| pane_entries.iter().find(|entry| entry.pane_id == pane_id))
            .is_some_and(|entry| !matches!(entry.kind, WorkbenchPaneKind::Graph { .. }));
        let prefer_workbench_scope = has_hosted_workbench && active_pane_prefers_workbench_scope;
        let layer_state = workbench_layer_state_from_tree(graph_app, tiles_tree);
        let chrome_policy = layer_state.chrome_policy();
        let host_layouts =
            WorkbenchHostLayout::layouts_from_runtime(graph_app, prefer_workbench_scope);
        let host_layout = host_layouts
            .first()
            .cloned()
            .unwrap_or_else(|| WorkbenchHostLayout::default_workbench_navigator(graph_app));
        let active_pane_title = pane_entries
            .iter()
            .find(|entry| entry.is_active)
            .map(|entry| entry.title.clone());
        let active_frame_name = graph_app.current_frame_name().map(str::to_string);
        let navigator_groups =
            navigator_groups(graph_app, &arrangement_memberships, projection_view_id);
        let tree_root = tiles_tree.root().and_then(|root| {
            build_tree_node(
                graph_app,
                tiles_tree,
                root,
                active_pane,
                &arrangement_memberships,
            )
        });
        let active_graphlet_roster =
            build_active_graphlet_roster(graph_app, tiles_tree, active_pane, projection_view_id);
        Self {
            layer_state,
            chrome_policy,
            host_layout,
            host_layouts,
            active_graph_view,
            extra_graph_views,
            active_pane_title,
            active_frame_name,
            saved_frame_names,
            navigator_groups,
            pane_entries,
            tree_root,
            active_graphlet_roster,
        }
    }

    pub(crate) fn visible(&self) -> bool {
        matches!(
            self.chrome_policy,
            ChromeExposurePolicy::GraphPlusWorkbenchOverlay
                | ChromeExposurePolicy::GraphPlusWorkbenchHost
                | ChromeExposurePolicy::GraphPlusWorkbenchHostPinned
                | ChromeExposurePolicy::WorkbenchOnly
        )
    }
}

fn graph_view_switcher_projection(
    graph_app: &GraphBrowserApp,
) -> (Option<(GraphViewId, String)>, Vec<(GraphViewId, String)>) {
    let runtime = &graph_app.workspace.graph_runtime;

    let mut all_views = runtime
        .views
        .iter()
        .map(|(id, view)| {
            let name = view.name.trim().to_string();
            let label = if name.is_empty() {
                "Graph".to_string()
            } else {
                name
            };
            (*id, label)
        })
        .collect::<Vec<_>>();
    all_views.sort_by_key(|(id, _)| id.as_uuid());

    let focused_id = runtime.focused_view.or_else(|| {
        (all_views.len() == 1)
            .then(|| all_views.first().map(|(id, _)| *id))
            .flatten()
    });
    let active_view = focused_id.and_then(|focused_id| {
        all_views
            .iter()
            .find(|(id, _)| *id == focused_id)
            .map(|(id, label)| (*id, label.clone()))
    });
    let extra_views = if all_views.len() > 1 {
        all_views
            .iter()
            .filter(|(id, _)| Some(*id) != focused_id)
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    (active_view, extra_views)
}

fn navigator_groups(
    graph_app: &GraphBrowserApp,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
    graph_view_id: Option<GraphViewId>,
) -> Vec<WorkbenchNavigatorGroup> {
    let mut groups = arrangement_navigator_groups(graph_app, graph_view_id);
    let mut assigned_keys = groups
        .iter()
        .flat_map(|group| group.members.iter().map(|member| member.node_key))
        .collect::<HashSet<_>>();

    let folder_groups = containment_navigator_groups(graph_app, &assigned_keys, true);
    assigned_keys.extend(
        folder_groups
            .iter()
            .flat_map(|group| group.members.iter().map(|member| member.node_key)),
    );
    groups.extend(folder_groups);

    let domain_groups = containment_navigator_groups(graph_app, &assigned_keys, false);
    assigned_keys.extend(
        domain_groups
            .iter()
            .flat_map(|group| group.members.iter().map(|member| member.node_key)),
    );
    groups.extend(domain_groups);

    let recent_keys = recent_navigator_members(graph_app, arrangement_memberships, &assigned_keys)
        .iter()
        .map(|member| member.node_key)
        .collect::<HashSet<_>>();
    groups.extend(unrelated_navigator_group(
        graph_app,
        arrangement_memberships,
        &assigned_keys,
        &recent_keys,
    ));
    groups.extend(recent_navigator_group(
        graph_app,
        arrangement_memberships,
        &assigned_keys,
    ));
    groups.extend(imported_navigator_groups(graph_app));
    groups
}

fn arrangement_navigator_groups(
    graph_app: &GraphBrowserApp,
    graph_view_id: Option<GraphViewId>,
) -> Vec<WorkbenchNavigatorGroup> {
    graph_app
        .arrangement_projection_groups()
        .into_iter()
        .map(|group| {
            // Start with the directly-connected members (via ArrangementRelation edges).
            let mut member_keys: Vec<NodeKey> = group.member_keys.clone();

            // Extend with cold peers reachable via UserGrouped edges that are not
            // already represented by an ArrangementRelation edge. These are nodes
            // that were added to the graphlet by tile-opening (Phase 5) and later
            // dismissed (DismissTile → Cold). Their edges survive dismiss, so they
            // remain durable graphlet members but are not FrameMember-connected.
            let existing: std::collections::HashSet<NodeKey> =
                member_keys.iter().copied().collect();
            for &seed in &group.member_keys {
                for peer in graph_app.graphlet_peers_for_view(seed, graph_view_id) {
                    if !existing.contains(&peer) && !member_keys.contains(&peer) {
                        // Skip internal surface nodes (frame anchors, tool panes, etc.)
                        // that are reachable via FrameMember edges but are not user-visible
                        // content nodes.
                        if let Some(node) = graph_app.domain_graph().get_node(peer) {
                            if !is_internal_surface_node(node) {
                                member_keys.push(peer);
                            }
                        }
                    }
                }
            }

            WorkbenchNavigatorGroup {
                section: WorkbenchNavigatorSection::Workbench,
                title: match group.sub_kind {
                    ArrangementSubKind::FrameMember => format!("Frame: {}", group.title),
                    ArrangementSubKind::TileGroup => format!("Tile Group: {}", group.title),
                    ArrangementSubKind::SplitPair => format!("Split Pair: {}", group.title),
                },
                is_highlighted: matches!(group.sub_kind, ArrangementSubKind::FrameMember)
                    && (graph_app.selected_frame_name() == Some(group.title.as_str())
                        || graph_app.current_frame_name() == Some(group.title.as_str())),
                members: member_keys
                    .into_iter()
                    .filter_map(|node_key| navigator_member_for_node(graph_app, node_key, None))
                    .collect(),
            }
        })
        .collect()
}

/// Build the graphlet roster for the active node pane (Phase 3 — omnibar).
///
/// Returns all graphlet peers of the active pane's node under the resolved
/// projection, plus the node itself, sorted: warm/active members first (by
/// title), then cold members.
/// Returns an empty `Vec` if the active pane is not a node pane or the node
/// has no graphlet peers.
fn build_active_graphlet_roster(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    active_pane: Option<PaneId>,
    graph_view_id: Option<GraphViewId>,
) -> Vec<GraphletRosterEntry> {
    let active_pane = match active_pane {
        Some(p) => p,
        None => return Vec::new(),
    };

    // Find the node key for the active pane.
    let seed_node = tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        Tile::Pane(kind) if kind.pane_id() == active_pane => kind.node_state().map(|s| s.node),
        _ => None,
    });
    let seed_node = match seed_node {
        Some(k) => k,
        None => return Vec::new(),
    };

    // Collect the seed + all graphlet peers under the resolved projection.
    let mut all_nodes = vec![seed_node];
    all_nodes.extend(graph_app.graphlet_peers_for_view(seed_node, graph_view_id));
    all_nodes.sort_by_key(|k| k.index());
    all_nodes.dedup();

    if all_nodes.len() <= 1 {
        // Singleton — no roster to show.
        return Vec::new();
    }

    let mut entries: Vec<GraphletRosterEntry> = all_nodes
        .into_iter()
        .filter_map(|node_key| {
            let node = graph_app.domain_graph().get_node(node_key)?;
            let title = node_primary_label(node);
            let is_cold = node.lifecycle == crate::graph::NodeLifecycle::Cold;
            Some(GraphletRosterEntry {
                node_key,
                title,
                is_cold,
            })
        })
        .collect();

    // Sort: warm/active first (by title), then cold (by title).
    entries.sort_by(|a, b| {
        a.is_cold
            .cmp(&b.is_cold)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.node_key.index().cmp(&b.node_key.index()))
    });
    entries
}

fn containment_navigator_groups(
    graph_app: &GraphBrowserApp,
    excluded_keys: &HashSet<NodeKey>,
    folders: bool,
) -> Vec<WorkbenchNavigatorGroup> {
    let mut sections: BTreeMap<String, Vec<NodeKey>> = BTreeMap::new();
    for (node_key, node) in graph_app.domain_graph().nodes() {
        if excluded_keys.contains(&node_key) || is_internal_surface_node(node) {
            continue;
        }

        let Ok(parsed) = url::Url::parse(node.url()) else {
            continue;
        };

        let maybe_section_key = if folders {
            containment_folder_key(&parsed)
        } else {
            parsed.host_str().map(|host| host.to_ascii_lowercase())
        };

        let Some(section_key) = maybe_section_key else {
            continue;
        };
        sections.entry(section_key).or_default().push(node_key);
    }

    sections
        .into_iter()
        .filter_map(|(title, mut node_keys)| {
            node_keys.sort_by(|left, right| {
                navigator_member_sort_key(graph_app, *left)
                    .cmp(&navigator_member_sort_key(graph_app, *right))
            });
            node_keys.dedup();
            let members = node_keys
                .into_iter()
                .filter_map(|node_key| navigator_member_for_node(graph_app, node_key, None))
                .collect::<Vec<_>>();
            if members.is_empty() {
                return None;
            }
            Some(WorkbenchNavigatorGroup {
                section: if folders {
                    WorkbenchNavigatorSection::Folders
                } else {
                    WorkbenchNavigatorSection::Domain
                },
                title,
                is_highlighted: false,
                members,
            })
        })
        .collect()
}

fn containment_folder_key(parsed: &url::Url) -> Option<String> {
    if !matches!(parsed.scheme(), "http" | "https" | "file") {
        return None;
    }

    let mut parent = parsed.clone();
    parent.set_query(None);
    parent.set_fragment(None);
    let mut segments: Vec<String> = parent
        .path_segments()
        .map(|parts| {
            parts
                .filter(|segment| !segment.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if segments.is_empty() {
        return None;
    }
    segments.pop();
    let parent_path = if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}/", segments.join("/"))
    };
    parent.set_path(&parent_path);
    Some(parent.to_string())
}

fn imported_navigator_groups(graph_app: &GraphBrowserApp) -> Vec<WorkbenchNavigatorGroup> {
    let mut sections: BTreeMap<String, Vec<NodeKey>> = BTreeMap::new();
    for (node_key, node) in graph_app.domain_graph().nodes() {
        if is_internal_surface_node(node) {
            continue;
        }
        for provenance in &node.import_provenance {
            let label = provenance.source_label.trim();
            if label.is_empty() {
                continue;
            }
            sections
                .entry(label.to_string())
                .or_default()
                .push(node_key);
        }
    }

    sections
        .into_iter()
        .filter_map(|(title, mut node_keys)| {
            node_keys.sort_by(|left, right| {
                navigator_member_sort_key(graph_app, *left)
                    .cmp(&navigator_member_sort_key(graph_app, *right))
            });
            node_keys.dedup();
            let members = node_keys
                .into_iter()
                .filter_map(|node_key| navigator_member_for_node(graph_app, node_key, None))
                .collect::<Vec<_>>();
            if members.is_empty() {
                return None;
            }
            Some(WorkbenchNavigatorGroup {
                section: WorkbenchNavigatorSection::Imported,
                title,
                is_highlighted: false,
                members,
            })
        })
        .collect()
}

fn recent_navigator_group(
    graph_app: &GraphBrowserApp,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
    excluded_keys: &HashSet<NodeKey>,
) -> Option<WorkbenchNavigatorGroup> {
    let members = recent_navigator_members(graph_app, arrangement_memberships, excluded_keys);
    if members.is_empty() {
        return None;
    }
    Some(WorkbenchNavigatorGroup {
        section: WorkbenchNavigatorSection::Recent,
        title: "Recent".to_string(),
        is_highlighted: false,
        members,
    })
}

fn recent_navigator_members(
    graph_app: &GraphBrowserApp,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
    excluded_keys: &HashSet<NodeKey>,
) -> Vec<WorkbenchNavigatorMember> {
    let mut rows = graph_app
        .semantic_recent_navigation_nodes(NAVIGATOR_RECENT_LIMIT * 4)
        .into_iter()
        .filter(|(node_key, runtime)| {
            let Some(node) = graph_app.domain_graph().get_node(*node_key) else {
                return false;
            };
            runtime.visit_count > 0
                && !arrangement_memberships.contains_key(node_key)
                && !excluded_keys.contains(node_key)
                && !is_internal_surface_node(node)
        })
        .collect::<Vec<_>>();
    rows.sort_by(|(left_key, left_stats), (right_key, right_stats)| {
        right_stats
            .last_visit_at_ms
            .cmp(&left_stats.last_visit_at_ms)
            .then_with(|| right_stats.visit_count.cmp(&left_stats.visit_count))
            .then_with(|| {
                navigator_member_sort_key(graph_app, *left_key)
                    .cmp(&navigator_member_sort_key(graph_app, *right_key))
            })
    });
    rows.truncate(NAVIGATOR_RECENT_LIMIT);
    let mut members = Vec::new();
    for (node_key, runtime) in rows {
        let Some(parent) = ({
            let mut parts = vec![format!(
                "{} visit{}",
                runtime.visit_count,
                if runtime.visit_count == 1 { "" } else { "s" }
            )];
            if runtime.branch_points > 0 {
                parts.push(format!(
                    "{} fork{}",
                    runtime.branch_points,
                    if runtime.branch_points == 1 { "" } else { "s" }
                ));
            }
            if runtime.alternate_targets > 0 {
                parts.push(format!(
                    "{} alt{}",
                    runtime.alternate_targets,
                    if runtime.alternate_targets == 1 {
                        ""
                    } else {
                        "s"
                    }
                ));
            }
            let suffix = format!("({})", parts.join(", "));
            navigator_member_for_node(graph_app, node_key, Some(suffix))
        }) else {
            continue;
        };
        members.push(parent);
        members.extend(semantic_branch_target_members(graph_app, node_key));
    }
    members
}

fn unrelated_navigator_group(
    graph_app: &GraphBrowserApp,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
    excluded_keys: &HashSet<NodeKey>,
    recent_keys: &HashSet<NodeKey>,
) -> Option<WorkbenchNavigatorGroup> {
    let mut members = graph_app
        .domain_graph()
        .nodes()
        .filter(|(node_key, node)| {
            !arrangement_memberships.contains_key(node_key)
                && !excluded_keys.contains(node_key)
                && !recent_keys.contains(node_key)
                && !is_internal_surface_node(node)
        })
        .map(|(node_key, _)| node_key)
        .collect::<Vec<_>>();
    members.sort_by(|left, right| {
        navigator_member_sort_key(graph_app, *left)
            .cmp(&navigator_member_sort_key(graph_app, *right))
    });
    if members.is_empty() {
        return None;
    }
    Some(WorkbenchNavigatorGroup {
        section: WorkbenchNavigatorSection::Unrelated,
        title: "Unrelated".to_string(),
        is_highlighted: false,
        members: members
            .into_iter()
            .filter_map(|node_key| navigator_member_for_node(graph_app, node_key, None))
            .collect(),
    })
}

fn navigator_member_for_node(
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
    suffix: Option<String>,
) -> Option<WorkbenchNavigatorMember> {
    let node = graph_app.domain_graph().get_node(node_key)?;
    let mut title = node_primary_label(node);
    if let Some(suffix) = suffix {
        title.push(' ');
        title.push_str(&suffix);
    }
    let is_cold = node.lifecycle == crate::graph::NodeLifecycle::Cold;
    Some(WorkbenchNavigatorMember {
        node_key,
        title,
        is_selected: graph_app.focused_selection().contains(&node_key),
        row_key: navigator_row_key_for_node(graph_app, node_key),
        target_url: None,
        is_cold,
        depth: 0,
        is_expanded: false,
        has_children: false,
    })
}

fn semantic_branch_target_members(
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
) -> Vec<WorkbenchNavigatorMember> {
    let Some(node) = graph_app.domain_graph().get_node(node_key) else {
        return Vec::new();
    };
    let branch = node.history_branch_projection();
    if branch.visits.is_empty() {
        return Vec::new();
    }

    let row_key = navigator_row_key_for_node(graph_app, node_key);
    let mut members = Vec::new();
    let current_url = branch
        .visits
        .iter()
        .find(|visit| visit.is_current)
        .map(|visit| visit.url.clone());

    if let Some(current_url) = current_url.clone() {
        members.push(WorkbenchNavigatorMember {
            node_key,
            title: format!("Now: {}", navigator_semantic_target_label(&current_url)),
            is_selected: graph_app.focused_selection().contains(&node_key),
            row_key: row_key.clone(),
            target_url: Some(current_url),
            is_cold: false,
            depth: 1,
            is_expanded: false,
            has_children: false,
        });
    }

    let mut seen_urls = HashSet::new();
    if let Some(current_url) = current_url {
        seen_urls.insert(current_url);
    }

    'outer: for visit in branch.visits.iter().rev() {
        for alternate in visit.alternate_children.iter().rev() {
            if !seen_urls.insert(alternate.url.clone()) {
                continue;
            }
            members.push(WorkbenchNavigatorMember {
                node_key,
                title: format!("Alt: {}", navigator_semantic_target_label(&alternate.url)),
                is_selected: graph_app.focused_selection().contains(&node_key),
                row_key: row_key.clone(),
                target_url: Some(alternate.url.clone()),
                is_cold: false,
                depth: 1,
                is_expanded: false,
                has_children: false,
            });
            if members.len() >= 3 {
                break 'outer;
            }
        }
    }

    members
}

fn navigator_semantic_target_label(url: &str) -> String {
    let display = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    if display.chars().count() <= 32 {
        display.to_string()
    } else {
        let shortened: String = display.chars().take(31).collect();
        format!("{shortened}…")
    }
}

fn navigator_row_key_for_node(graph_app: &GraphBrowserApp, node_key: NodeKey) -> Option<String> {
    graph_app
        .navigator_projection_state()
        .row_targets
        .iter()
        .filter_map(|(row_key, target)| match target {
            crate::app::NavigatorProjectionTarget::Node(key) if *key == node_key => {
                Some(row_key.clone())
            }
            _ => None,
        })
        .min()
}

fn node_has_workbench_presentation(tiles_tree: &Tree<TileKind>, node_key: NodeKey) -> bool {
    tiles_tree.tiles.iter().any(|(_, tile)| {
        matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key)
            || matches!(
                tile,
                Tile::Pane(TileKind::Pane(
                    crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
                )) if state.node == node_key
            )
    })
}

fn focus_node_presentation(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) -> bool {
    tiles_tree.make_active(|_, tile| {
        matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key)
            || matches!(
                tile,
                Tile::Pane(TileKind::Pane(
                    crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
                )) if state.node == node_key
            )
    })
}

fn graph_view_id_for_navigation(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) -> Option<GraphViewId> {
    active_visible_graph_view_id(tiles_tree)
        .or_else(|| {
            tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Graph(graph_ref)) => Some(graph_ref.graph_view_id),
                Tile::Pane(TileKind::Pane(
                    crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(graph_ref),
                )) => Some(graph_ref.graph_view_id),
                _ => None,
            })
        })
        .or(graph_app.workspace.graph_runtime.focused_view)
        .or_else(|| {
            graph_app
                .workspace
                .graph_runtime
                .views
                .keys()
                .next()
                .copied()
        })
}

fn active_visible_graph_view_id(tiles_tree: &Tree<TileKind>) -> Option<GraphViewId> {
    tiles_tree.active_tiles().into_iter().find_map(|tile_id| {
        let tile = tiles_tree.tiles.get(tile_id)?;
        match tile {
            Tile::Pane(TileKind::Graph(graph_ref)) => Some(graph_ref.graph_view_id),
            Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(graph_ref),
            )) => Some(graph_ref.graph_view_id),
            _ => None,
        }
    })
}

fn offscreen_visible_graph_view_for_node(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    node_key: NodeKey,
) -> Option<GraphViewId> {
    let view_id = active_visible_graph_view_id(tiles_tree)?;
    let canvas_rect = graph_app
        .workspace
        .graph_runtime
        .graph_view_canvas_rects
        .get(&view_id)?;
    let position = graph_app.domain_graph().node_projected_position(node_key)?;
    (!canvas_rect.contains(position.to_pos2())).then_some(view_id)
}

fn navigator_member_sort_key(app: &GraphBrowserApp, key: NodeKey) -> (String, usize) {
    let label = app
        .domain_graph()
        .get_node(key)
        .map(node_primary_label)
        .unwrap_or_else(|| format!("Node {}", key.index()));
    (label, key.index())
}

fn node_primary_label(node: &crate::graph::Node) -> String {
    let title = user_visible_node_title_from_data(node);
    if !title.trim().is_empty() {
        title
    } else if !node.url().trim().is_empty() {
        node.url().to_string()
    } else {
        "Untitled node".to_string()
    }
}

fn node_pane_entry_title(key: NodeKey, node: &crate::graph::Node) -> String {
    let title = node.title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    if node.address.address_kind() == crate::graph::AddressKind::GraphshellClip {
        let visible_title = user_visible_node_title_from_data(node);
        if !visible_title.trim().is_empty() {
            return visible_title;
        }
    }
    format!("Node {}", key.index())
}

fn node_pane_entry_subtitle(node: &crate::graph::Node) -> Option<String> {
    let visible_url = user_visible_node_url_from_data(node);
    (!visible_url.trim().is_empty()).then_some(visible_url)
}

fn is_internal_surface_node(node: &crate::graph::Node) -> bool {
    matches!(
        VersoAddress::parse(node.url()),
        Some(
            VersoAddress::Frame(_)
                | VersoAddress::TileGroup(_)
                | VersoAddress::View(_)
                | VersoAddress::Tool { .. }
                | VersoAddress::Other { .. }
        )
    )
}

fn pane_entry_for_tile(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    kind: &TileKind,
    active_pane: Option<PaneId>,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
) -> WorkbenchPaneEntry {
    match kind {
        TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(
            graph_ref,
        )) => WorkbenchPaneEntry {
            pane_id: graph_ref.pane_id,
            kind: WorkbenchPaneKind::Graph {
                view_id: graph_ref.graph_view_id,
            },
            title: graph_view_title(graph_app, graph_ref.graph_view_id),
            subtitle: Some("Graph".to_string()),
            arrangement_memberships: Vec::new(),
            semantic_tab_affordance: None,
            node_viewer_summary: None,
            presentation_mode: PanePresentationMode::Tiled,
            is_active: active_pane == Some(graph_ref.pane_id),
            closable: false,
        },
        TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Node(
            state,
        )) => {
            let title = graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|node| node_pane_entry_title(state.node, node))
                .unwrap_or_else(|| format!("Node {}", state.node.index()));
            let subtitle = graph_app
                .domain_graph()
                .get_node(state.node)
                .and_then(node_pane_entry_subtitle);
            WorkbenchPaneEntry {
                pane_id: state.pane_id,
                kind: WorkbenchPaneKind::Node {
                    node_key: state.node,
                },
                title,
                subtitle,
                arrangement_memberships: arrangement_memberships
                    .get(&state.node)
                    .cloned()
                    .unwrap_or_default(),
                semantic_tab_affordance: semantic_tabs::semantic_tab_affordance_for_pane(
                    tiles_tree,
                    graph_app,
                    state.pane_id,
                ),
                node_viewer_summary: Some(build_node_viewer_summary(graph_app, state)),
                presentation_mode: state.presentation_mode,
                is_active: active_pane == Some(state.pane_id),
                closable: true,
            }
        }
        #[cfg(feature = "diagnostics")]
        TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool)) => {
            WorkbenchPaneEntry {
                pane_id: tool.pane_id,
                kind: WorkbenchPaneKind::Tool {
                    kind: tool.kind.clone(),
                },
                title: tool.title().to_string(),
                subtitle: Some("Tool".to_string()),
                arrangement_memberships: Vec::new(),
                semantic_tab_affordance: None,
                node_viewer_summary: None,
                presentation_mode: PanePresentationMode::Tiled,
                is_active: active_pane == Some(tool.pane_id),
                closable: true,
            }
        }
        TileKind::Graph(graph_ref) => WorkbenchPaneEntry {
            pane_id: graph_ref.pane_id,
            kind: WorkbenchPaneKind::Graph {
                view_id: graph_ref.graph_view_id,
            },
            title: graph_view_title(graph_app, graph_ref.graph_view_id),
            subtitle: Some("Graph".to_string()),
            arrangement_memberships: Vec::new(),
            semantic_tab_affordance: None,
            node_viewer_summary: None,
            presentation_mode: PanePresentationMode::Tiled,
            is_active: active_pane == Some(graph_ref.pane_id),
            closable: false,
        },
        TileKind::Node(state) => {
            let title = graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|node| node_pane_entry_title(state.node, node))
                .unwrap_or_else(|| format!("Node {}", state.node.index()));
            let subtitle = graph_app
                .domain_graph()
                .get_node(state.node)
                .and_then(node_pane_entry_subtitle);
            WorkbenchPaneEntry {
                pane_id: state.pane_id,
                kind: WorkbenchPaneKind::Node {
                    node_key: state.node,
                },
                title,
                subtitle,
                arrangement_memberships: arrangement_memberships
                    .get(&state.node)
                    .cloned()
                    .unwrap_or_default(),
                semantic_tab_affordance: semantic_tabs::semantic_tab_affordance_for_pane(
                    tiles_tree,
                    graph_app,
                    state.pane_id,
                ),
                node_viewer_summary: Some(build_node_viewer_summary(graph_app, state)),
                presentation_mode: state.presentation_mode,
                is_active: active_pane == Some(state.pane_id),
                closable: true,
            }
        }
        #[cfg(feature = "diagnostics")]
        TileKind::Tool(tool) => WorkbenchPaneEntry {
            pane_id: tool.pane_id,
            kind: WorkbenchPaneKind::Tool {
                kind: tool.kind.clone(),
            },
            title: tool.title().to_string(),
            subtitle: Some("Tool".to_string()),
            arrangement_memberships: Vec::new(),
            semantic_tab_affordance: None,
            node_viewer_summary: None,
            presentation_mode: PanePresentationMode::Tiled,
            is_active: active_pane == Some(tool.pane_id),
            closable: true,
        },
    }
}

fn build_node_viewer_summary(
    graph_app: &GraphBrowserApp,
    state: &NodePaneState,
) -> WorkbenchNodeViewerSummary {
    let verso_route = state
        .resolved_route
        .clone()
        .or_else(|| tile_runtime::resolve_route_for_node_pane(state, graph_app));
    WorkbenchNodeViewerSummary {
        effective_viewer_id: tile_runtime::effective_viewer_id_for_node_pane(state, graph_app),
        viewer_override: state
            .viewer_id_override
            .as_ref()
            .map(|viewer_id| viewer_id.as_str().to_string()),
        viewer_switch_reason: state.viewer_switch_reason,
        render_mode: state.render_mode,
        runtime_blocked: graph_app.runtime_block_state_for_node(state.node).is_some(),
        runtime_crashed: graph_app.runtime_crash_state_for_node(state.node).is_some(),
        fallback_reason: tile_runtime::fallback_reason_for_node_pane(state, graph_app),
        available_viewer_ids: tile_runtime::candidate_viewer_ids_for_node_pane(state, graph_app),
        verso_route,
    }
}

fn pane_arrangement_memberships(graph_app: &GraphBrowserApp) -> HashMap<NodeKey, Vec<String>> {
    let mut index: HashMap<NodeKey, Vec<String>> = HashMap::new();
    for group in graph_app.arrangement_projection_groups() {
        let label = match group.sub_kind {
            ArrangementSubKind::FrameMember => format!("Frame: {}", group.title),
            ArrangementSubKind::TileGroup => format!("Tile Group: {}", group.title),
            ArrangementSubKind::SplitPair => format!("Split Pair: {}", group.title),
        };
        for node_key in group.member_keys {
            index.entry(node_key).or_default().push(label.clone());
        }
    }
    for memberships in index.values_mut() {
        memberships.sort();
        memberships.dedup();
    }
    index
}

fn graph_view_title(graph_app: &GraphBrowserApp, view_id: GraphViewId) -> String {
    graph_app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|view| view.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Graph View".to_string())
}

fn current_frame_group_label(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    tile_id: TileId,
    child_count: usize,
) -> Option<String> {
    (tiles_tree.root() == Some(tile_id))
        .then(|| graph_app.current_frame_name())
        .flatten()
        .map(|frame_name| format!("Frame: {frame_name} ({child_count})"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FrameSplitOfferCandidate {
    node_key: NodeKey,
    frame_name: String,
    hint_count: usize,
}

fn frame_split_offer_candidate(graph_app: &GraphBrowserApp) -> Option<FrameSplitOfferCandidate> {
    let node_key = graph_app.focused_selection().primary()?;
    let mut frame_names = graph_app.sorted_frames_for_node_key(node_key);
    for group in graph_app.arrangement_projection_groups() {
        if group.sub_kind == ArrangementSubKind::FrameMember
            && group.member_keys.contains(&node_key)
            && !frame_names.contains(&group.id)
        {
            frame_names.push(group.id);
        }
    }

    for frame_name in frame_names {
        if graph_app.current_frame_name() == Some(frame_name.as_str())
            || graph_app.is_frame_split_offer_dismissed_for_session(&frame_name)
        {
            continue;
        }

        let frame_url = VersoAddress::frame(frame_name.clone()).to_string();
        let Some((frame_key, _)) = graph_app.domain_graph().get_node_by_url(&frame_url) else {
            continue;
        };
        if graph_app
            .domain_graph()
            .frame_split_offer_suppressed(frame_key)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(hints) = graph_app.domain_graph().frame_layout_hints(frame_key) else {
            continue;
        };
        if hints.is_empty() {
            continue;
        }

        return Some(FrameSplitOfferCandidate {
            node_key,
            frame_name,
            hint_count: hints.len(),
        });
    }
    None
}

fn frame_key_for_name(graph_app: &GraphBrowserApp, frame_name: &str) -> Option<NodeKey> {
    let frame_url = VersoAddress::frame(frame_name.to_string()).to_string();
    graph_app
        .domain_graph()
        .get_node_by_url(&frame_url)
        .map(|(frame_key, _)| frame_key)
}

fn frame_layout_hints_for_name(
    graph_app: &GraphBrowserApp,
    frame_name: &str,
) -> Vec<FrameLayoutHint> {
    frame_key_for_name(graph_app, frame_name)
        .and_then(|frame_key| graph_app.domain_graph().frame_layout_hints(frame_key))
        .map(|hints| hints.to_vec())
        .unwrap_or_default()
}

fn frame_layout_hint_summary(hint: &FrameLayoutHint) -> String {
    match hint {
        FrameLayoutHint::SplitHalf {
            orientation,
            first,
            second,
        } => {
            let axis = match orientation {
                SplitOrientation::Vertical => "side-by-side",
                SplitOrientation::Horizontal => "stacked",
            };
            format!("Half ({axis}): {first}, {second}")
        }
        FrameLayoutHint::SplitPamphlet {
            members,
            orientation,
        } => {
            let axis = match orientation {
                SplitOrientation::Vertical => "columns",
                SplitOrientation::Horizontal => "rows",
            };
            format!(
                "Pamphlet ({axis}): {}, {}, {}",
                members[0], members[1], members[2]
            )
        }
        FrameLayoutHint::SplitTriptych {
            dominant,
            dominant_edge,
            wings,
        } => {
            let edge = match dominant_edge {
                DominantEdge::Left => "left-dominant",
                DominantEdge::Right => "right-dominant",
                DominantEdge::Top => "top-dominant",
                DominantEdge::Bottom => "bottom-dominant",
            };
            format!("Triptych ({edge}): {dominant} | {}, {}", wings[0], wings[1])
        }
        FrameLayoutHint::SplitQuartered {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
        } => format!("Quartered: {top_left}, {top_right}, {bottom_left}, {bottom_right}"),
    }
}

fn frame_settings_target_name(
    graph_app: &GraphBrowserApp,
    projection: &WorkbenchChromeProjection,
) -> Option<String> {
    projection
        .active_frame_name
        .clone()
        .or_else(|| graph_app.selected_frame_name().map(|name| name.to_string()))
}

fn frame_split_offer_suppressed_for_name(graph_app: &GraphBrowserApp, frame_name: &str) -> bool {
    frame_key_for_name(graph_app, frame_name)
        .and_then(|frame_key| {
            graph_app
                .domain_graph()
                .frame_split_offer_suppressed(frame_key)
        })
        .unwrap_or(false)
}

fn build_tree_node(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    tile_id: TileId,
    active_pane: Option<PaneId>,
    arrangement_memberships: &HashMap<NodeKey, Vec<String>>,
) -> Option<WorkbenchChromeNode> {
    let tile = tiles_tree.tiles.get(tile_id)?;
    match tile {
        Tile::Pane(kind) => Some(WorkbenchChromeNode::Pane(pane_entry_for_tile(
            graph_app,
            tiles_tree,
            kind,
            active_pane,
            arrangement_memberships,
        ))),
        Tile::Container(Container::Tabs(tabs)) => Some(WorkbenchChromeNode::Tabs {
            tile_id,
            label: current_frame_group_label(graph_app, tiles_tree, tile_id, tabs.children.len())
                .unwrap_or_else(|| format!("Tab Group ({})", tabs.children.len())),
            children: tabs
                .children
                .iter()
                .filter_map(|child| {
                    build_tree_node(
                        graph_app,
                        tiles_tree,
                        *child,
                        active_pane,
                        arrangement_memberships,
                    )
                })
                .collect(),
        }),
        Tile::Container(Container::Linear(linear)) => {
            let dir_label = match linear.dir {
                LinearDir::Horizontal => "Split ↔",
                LinearDir::Vertical => "Split ↕",
            };
            Some(WorkbenchChromeNode::Split {
                tile_id,
                label: format!("{dir_label} ({})", linear.children.len()),
                children: linear
                    .children
                    .iter()
                    .filter_map(|child| {
                        build_tree_node(
                            graph_app,
                            tiles_tree,
                            *child,
                            active_pane,
                            arrangement_memberships,
                        )
                    })
                    .collect(),
            })
        }
        Tile::Container(Container::Grid(grid)) => Some(WorkbenchChromeNode::Grid {
            tile_id,
            label: format!("Grid ({})", grid.children().count()),
            children: grid
                .children()
                .filter_map(|child| {
                    build_tree_node(
                        graph_app,
                        tiles_tree,
                        *child,
                        active_pane,
                        arrangement_memberships,
                    )
                })
                .collect(),
        }),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_workbench_host(
    ctx: &egui::Context,
    root_ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    command_bar_focus_target: CommandBarFocusTarget,
    show_clear_data_confirm: &mut bool,
) -> WorkbenchChromeProjection {
    let mut projection = WorkbenchChromeProjection::from_tree(
        graph_app,
        tiles_tree,
        command_bar_focus_target.active_pane(),
    );

    // Phase C: Replace workbench section with GraphTree-sourced groups.
    {
        let label_fn = |node_key: NodeKey| {
            graph_app
                .domain_graph()
                .get_node(node_key)
                .map(|n| node_primary_label(n))
                .unwrap_or_else(|| format!("Node {}", node_key.index()))
        };
        crate::shell::desktop::workbench::graph_tree_projection::replace_workbench_navigator_groups(
            &mut projection.navigator_groups,
            graph_tree,
            &label_fn,
        );
    }
    if !projection.visible() {
        graph_app
            .workspace
            .graph_runtime
            .workbench_navigation_geometry = None;
        return projection;
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && matches!(
            graph_app.workspace.workbench_session.ux_config_mode,
            UxConfigMode::Configuring { .. }
        )
    {
        graph_app.enqueue_workbench_intent(WorkbenchIntent::SetSurfaceConfigMode {
            surface_host: projection.host_layout.host.clone(),
            mode: UxConfigMode::Locked,
        });
    }

    let persisted_frame_names: HashSet<String> = graph_app
        .list_workspace_layout_names()
        .into_iter()
        .collect();
    let focused_pane_pin_name = command_bar_focus_target
        .focused_node()
        .and_then(|node| frame_pin_name_for_node(node, graph_app));
    let mut post_host_actions = Vec::new();
    let host_layouts = projection.host_layouts.clone();
    let missing_hosts = missing_navigator_hosts(&host_layouts);
    let mut overlay_occlusions = Vec::new();

    for (index, host_layout) in host_layouts.iter().cloned().enumerate() {
        let host_available_rect = ctx.content_rect();
        let host_panel_max_extent = match host_layout.form_factor {
            WorkbenchHostFormFactor::Sidebar => {
                (host_available_rect.width() * HOST_PANEL_MAX_FRACTION).max(HOST_PANEL_MAX_FLOOR)
            }
            WorkbenchHostFormFactor::Toolbar => {
                (host_available_rect.height() * HOST_PANEL_MAX_FRACTION).max(HOST_PANEL_MAX_FLOOR)
            }
        };
        let host_panel_default_extent = (match host_layout.form_factor {
            WorkbenchHostFormFactor::Sidebar => {
                host_available_rect.width() * host_layout.size_fraction
            }
            WorkbenchHostFormFactor::Toolbar => {
                host_available_rect.height() * host_layout.size_fraction
            }
        })
        .clamp(HOST_PANEL_MAX_FLOOR, host_panel_max_extent);
        let panel_id = host_panel_id(&host_layout.host);
        let specialty_canvas_area_id = egui::Id::new(("navigator_specialty_canvas", &panel_id));
        let mut rendered_rect = None;
        let mut actual_panel_extent: Option<f32> = None;
        let specialty_view_id = graph_app
            .workspace
            .workbench_session
            .navigator_specialty_views
            .get(&host_layout.host)
            .map(|sv| sv.view_id);
        let mut specialty_canvas_rect: Option<egui::Rect> = None;
        let mut show_host_contents = |ui: &mut egui::Ui| {
            rendered_rect = Some(ui.max_rect());
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading(host_display_name(&host_layout.host));
                    ui.label(
                        RichText::new(host_constraint_label(&host_layout))
                            .small()
                            .weak(),
                    );
                    let configuring_host = is_host_configuring(graph_app, &host_layout.host);
                    let toggle_label = if configuring_host {
                        "Lock Layout"
                    } else {
                        "Unlock Layout"
                    };
                    if ui.small_button(toggle_label).clicked() {
                        post_host_actions.push(WorkbenchHostAction::SetSurfaceConfigMode {
                            surface_host: host_layout.host.clone(),
                            mode: if configuring_host {
                                UxConfigMode::Locked
                            } else {
                                UxConfigMode::Configuring {
                                    surface_host: host_layout.host.clone(),
                                }
                            },
                        });
                    }
                });
                if let Some(active_title) = &projection.active_pane_title {
                    ui.label(RichText::new(active_title).small().weak());
                }
                if let Some(active_frame_name) = &projection.active_frame_name {
                    ui.label(
                        RichText::new(format!("Frame: {active_frame_name}"))
                            .small()
                            .strong(),
                    );
                }
                let mut rendered_graph_scope_header = false;
                if host_shows_graph_scope(&host_layout) && graph_view_switcher_visible(&projection)
                {
                    render_graph_view_switcher(ui, graph_app, &projection);
                    rendered_graph_scope_header = true;
                }
                if host_shows_graph_scope(&host_layout)
                    && render_graph_scope_controls(
                        ui,
                        graph_app,
                        &projection,
                        &mut post_host_actions,
                        show_clear_data_confirm,
                        "workbench_host_graph_scope",
                    )
                {
                    rendered_graph_scope_header = true;
                }
                if rendered_graph_scope_header {
                    ui.separator();
                }
                if let Some(frame_name) = frame_settings_target_name(graph_app, &projection) {
                    let hints = frame_layout_hints_for_name(graph_app, &frame_name);
                    let split_offer_suppressed =
                        frame_split_offer_suppressed_for_name(graph_app, &frame_name);
                    let rename_field_id =
                        egui::Id::new("workbench_host_frame_rename").with(frame_name.clone());
                    let mut rename_value = ui
                        .ctx()
                        .data_mut(|data| data.get_persisted::<String>(rename_field_id))
                        .unwrap_or_else(|| frame_name.clone());
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().faint_bg_color)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("Frame settings: {frame_name}"))
                                    .small()
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(
                                    if projection.active_frame_name.as_deref()
                                        == Some(frame_name.as_str())
                                    {
                                        "Managing the active frame tile group."
                                    } else {
                                        "Managing the frame currently selected on the graph."
                                    },
                                )
                                .small()
                                .weak(),
                            );
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("Name").small().weak());
                                ui.add(
                                    egui::TextEdit::singleline(&mut rename_value)
                                        .desired_width(180.0),
                                );
                                let trimmed_name = rename_value.trim().to_string();
                                if ui.small_button("Reset").clicked() {
                                    rename_value = frame_name.clone();
                                }
                                if ui
                                    .add_enabled(
                                        !trimmed_name.is_empty() && trimmed_name != frame_name,
                                        egui::Button::new("Rename"),
                                    )
                                    .clicked()
                                {
                                    post_host_actions.push(WorkbenchHostAction::RenameFrame {
                                        from: frame_name.clone(),
                                        to: trimmed_name.clone(),
                                    });
                                    rename_value = trimmed_name;
                                }
                                if ui.small_button("Delete frame").clicked() {
                                    post_host_actions.push(WorkbenchHostAction::DeleteFrame(
                                        frame_name.clone(),
                                    ));
                                }
                            });
                            ui.ctx().data_mut(|data| {
                                data.insert_persisted(rename_field_id, rename_value.clone());
                            });
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .small_button(if split_offer_suppressed {
                                        "Re-enable split offer"
                                    } else {
                                        "Suppress split offer"
                                    })
                                    .clicked()
                                {
                                    post_host_actions.push(
                                        WorkbenchHostAction::SetFrameSplitOfferSuppressed {
                                            frame_name: frame_name.clone(),
                                            suppressed: !split_offer_suppressed,
                                        },
                                    );
                                }
                                ui.label(
                                    RichText::new(if split_offer_suppressed {
                                        "Split offer suppressed for this frame."
                                    } else {
                                        "Split offer currently allowed for this frame."
                                    })
                                    .small()
                                    .weak(),
                                );
                            });
                            if hints.is_empty() {
                                ui.label(
                                    RichText::new("No recorded layout hints yet.")
                                        .small()
                                        .weak(),
                                );
                            } else {
                                for (index, hint) in hints.iter().enumerate() {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{}. {}",
                                                index + 1,
                                                frame_layout_hint_summary(hint)
                                            ))
                                            .small(),
                                        );
                                        if index > 0 && ui.small_button("Up").clicked() {
                                            post_host_actions.push(
                                                WorkbenchHostAction::MoveFrameLayoutHint {
                                                    frame_name: frame_name.clone(),
                                                    from_index: index,
                                                    to_index: index - 1,
                                                },
                                            );
                                        }
                                        if index + 1 < hints.len()
                                            && ui.small_button("Down").clicked()
                                        {
                                            post_host_actions.push(
                                                WorkbenchHostAction::MoveFrameLayoutHint {
                                                    frame_name: frame_name.clone(),
                                                    from_index: index,
                                                    to_index: index + 1,
                                                },
                                            );
                                        }
                                        if ui.small_button("Delete").clicked() {
                                            post_host_actions.push(
                                                WorkbenchHostAction::RemoveFrameLayoutHint {
                                                    frame_name: frame_name.clone(),
                                                    hint_index: index,
                                                },
                                            );
                                        }
                                    });
                                }
                            }
                        });
                }
                if let Some(split_offer) = frame_split_offer_candidate(graph_app) {
                    let split_label = if split_offer.hint_count == 1 {
                        "1 recorded split".to_string()
                    } else {
                        format!("{} recorded splits", split_offer.hint_count)
                    };
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().faint_bg_color)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "Frame '{name}' has {split_label}.",
                                    name = split_offer.frame_name
                                ))
                                .small(),
                            );
                            ui.horizontal_wrapped(|ui| {
                                if ui.small_button("Open as split").clicked() {
                                    post_host_actions.push(WorkbenchHostAction::OpenFrameAsSplit {
                                        node_key: split_offer.node_key,
                                        frame_name: split_offer.frame_name.clone(),
                                    });
                                }
                                if ui.small_button("Not this session").clicked() {
                                    post_host_actions.push(
                                        WorkbenchHostAction::DismissFrameSplitOfferForSession(
                                            split_offer.frame_name.clone(),
                                        ),
                                    );
                                }
                                if ui.small_button("Never for this frame").clicked() {
                                    post_host_actions.push(
                                        WorkbenchHostAction::SetFrameSplitOfferSuppressed {
                                            frame_name: split_offer.frame_name.clone(),
                                            suppressed: true,
                                        },
                                    );
                                }
                            });
                        });
                }
                if first_use_prompt_visible(graph_app, &host_layout.host) {
                    let existing_policy = graph_app
                        .workbench_profile()
                        .first_use_policies
                        .get(&host_layout.host)
                        .cloned();
                    let awaiting_config_follow_up = existing_policy
                        .as_ref()
                        .and_then(|policy| policy.outcome.as_ref())
                        .is_some_and(|outcome| matches!(outcome, FirstUseOutcome::ConfigureNow))
                        && !is_host_configuring(graph_app, &host_layout.host);
                    if existing_policy
                        .as_ref()
                        .is_none_or(|policy| !policy.prompt_shown)
                    {
                        post_host_actions.push(WorkbenchHostAction::SetFirstUsePolicy(
                            SurfaceFirstUsePolicy {
                                surface_host: host_layout.host.clone(),
                                prompt_shown: true,
                                outcome: None,
                            },
                        ));
                    }
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        if awaiting_config_follow_up {
                            ui.label(RichText::new("Keep this Navigator host layout?").small().strong());
                            ui.label(
                                RichText::new(
                                    "Remember this edge placement, keep it for just this session, or discard the draft.",
                                )
                                .small(),
                            );
                            ui.horizontal_wrapped(|ui| {
                                if ui.small_button("Remember layout").clicked() {
                                    let remembered_constraint = graph_app
                                        .workbench_layout_constraint_for_host(&host_layout.host)
                                        .cloned()
                                        .unwrap_or_else(|| constraint_from_host_layout(&host_layout));
                                    post_host_actions.push(
                                        WorkbenchHostAction::CommitLayoutConstraintDraft(
                                            host_layout.host.clone(),
                                        ),
                                    );
                                    post_host_actions.push(WorkbenchHostAction::SetFirstUsePolicy(
                                        SurfaceFirstUsePolicy {
                                            surface_host: host_layout.host.clone(),
                                            prompt_shown: true,
                                            outcome: Some(FirstUseOutcome::RememberedConstraint(
                                                remembered_constraint,
                                            )),
                                        },
                                    ));
                                }
                                if ui.small_button("This session only").clicked() {
                                    post_host_actions.push(WorkbenchHostAction::SetFirstUsePolicy(
                                        SurfaceFirstUsePolicy {
                                            surface_host: host_layout.host.clone(),
                                            prompt_shown: true,
                                            outcome: None,
                                        },
                                    ));
                                    post_host_actions.push(
                                        WorkbenchHostAction::SuppressFirstUsePromptForSession(
                                            host_layout.host.clone(),
                                        ),
                                    );
                                }
                                if ui.small_button("Discard changes").clicked() {
                                    post_host_actions.push(
                                        WorkbenchHostAction::DiscardLayoutConstraintDraft(
                                            host_layout.host.clone(),
                                        ),
                                    );
                                    post_host_actions.push(WorkbenchHostAction::SetFirstUsePolicy(
                                        SurfaceFirstUsePolicy {
                                            surface_host: host_layout.host.clone(),
                                            prompt_shown: true,
                                            outcome: Some(FirstUseOutcome::Discarded),
                                        },
                                    ));
                                }
                            });
                        } else {
                            ui.label(RichText::new("Set up this Navigator host").small().strong());
                            ui.label(
                                RichText::new(
                                    "Pin it to an edge now, keep the default layout, or dismiss this prompt.",
                                )
                                .small(),
                            );
                            ui.horizontal_wrapped(|ui| {
                                if ui.small_button("Set up layout").clicked() {
                                    post_host_actions.push(WorkbenchHostAction::SetFirstUsePolicy(
                                        SurfaceFirstUsePolicy {
                                            surface_host: host_layout.host.clone(),
                                            prompt_shown: true,
                                            outcome: Some(FirstUseOutcome::ConfigureNow),
                                        },
                                    ));
                                    post_host_actions.push(WorkbenchHostAction::SetSurfaceConfigMode {
                                        surface_host: host_layout.host.clone(),
                                        mode: UxConfigMode::Configuring {
                                            surface_host: host_layout.host.clone(),
                                        },
                                    });
                                }
                                if ui.small_button("Use default").clicked() {
                                    post_host_actions.push(WorkbenchHostAction::SetFirstUsePolicy(
                                        SurfaceFirstUsePolicy {
                                            surface_host: host_layout.host.clone(),
                                            prompt_shown: true,
                                            outcome: Some(FirstUseOutcome::AcceptDefault),
                                        },
                                    ));
                                }
                                if ui.small_button("Dismiss").clicked() {
                                    post_host_actions.push(WorkbenchHostAction::SetFirstUsePolicy(
                                        SurfaceFirstUsePolicy {
                                            surface_host: host_layout.host.clone(),
                                            prompt_shown: true,
                                            outcome: Some(FirstUseOutcome::Dismissed),
                                        },
                                    ));
                                }
                            });
                        }
                    });
                    ui.add_space(6.0);
                }
                if is_host_configuring(graph_app, &host_layout.host) {
                    render_host_config_controls(
                        ui,
                        &host_layout,
                        &missing_hosts,
                        &mut post_host_actions,
                    );
                    ui.separator();
                }
                // Graphlet roster: ● warm peers already visible as tabs;
                // ○ cold peers shown here for discovery / one-click activation.
                if !projection.active_graphlet_roster.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for entry in &projection.active_graphlet_roster {
                            let label = if entry.is_cold {
                                format!("○ {}", entry.title)
                            } else {
                                format!("● {}", entry.title)
                            };
                            let btn = egui::Button::new(RichText::new(&label).small())
                                .frame(entry.is_cold); // framed = clickable cold peer
                            if ui
                                .add(btn)
                                .on_hover_text(if entry.is_cold {
                                    "Cold — click to open a tile"
                                } else {
                                    "Warm — already open"
                                })
                                .clicked()
                                && entry.is_cold
                            {
                                post_host_actions.push(WorkbenchHostAction::ActivateNode {
                                    node_key: entry.node_key,
                                    row_key: None,
                                });
                            }
                        }
                    });
                    ui.add_space(2.0);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(layer_state_label(projection.layer_state))
                            .small()
                            .weak(),
                    );
                    let overlay_active = graph_app.workbench_overlay_visible();
                    let dedicated_mode_active =
                        graph_app.workbench_display_mode() == WorkbenchDisplayMode::Dedicated;
                    let has_dedicated_workbench_target = projection
                        .pane_entries
                        .iter()
                        .any(|entry| !matches!(entry.kind, WorkbenchPaneKind::Graph { .. }));
                    let dedicated_label = if dedicated_mode_active {
                        "Use Split Workbench"
                    } else {
                        "Use Dedicated Workbench"
                    };
                    let dedicated_toggle = ui.add_enabled(
                        dedicated_mode_active || has_dedicated_workbench_target,
                        egui::Button::new(dedicated_label).small(),
                    );
                    if dedicated_toggle.clicked() {
                        post_host_actions.push(WorkbenchHostAction::SetWorkbenchDisplayMode(
                            if dedicated_mode_active {
                                WorkbenchDisplayMode::Split
                            } else {
                                WorkbenchDisplayMode::Dedicated
                            },
                        ));
                    }
                    let overlay_label = if overlay_active {
                        "Close Workbench Overlay"
                    } else {
                        "Open Workbench Overlay"
                    };
                    let overlay_toggle = ui.add_enabled(
                        overlay_active || (!dedicated_mode_active && has_dedicated_workbench_target),
                        egui::Button::new(overlay_label).small(),
                    );
                    if overlay_toggle.clicked() {
                        post_host_actions.push(WorkbenchHostAction::SetWorkbenchOverlayVisible(
                            !overlay_active,
                        ));
                    }
                    let pin_label = if graph_app.workbench_host_pinned() {
                            "Unpin Workbench Host"
                    } else {
                            "Pin Workbench Host"
                    };
                    if ui.small_button(pin_label).clicked() {
                        post_host_actions.push(WorkbenchHostAction::SetWorkbenchPinned(
                            !graph_app.workbench_host_pinned(),
                        ));
                    }
                });
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    render_frame_pin_controls(
                        ui,
                        true,
                        focused_pane_pin_name.as_deref(),
                        &persisted_frame_names,
                        &mut post_host_actions,
                    );
                    ui.menu_button(
                        format!("Frames ({})", projection.saved_frame_names.len()),
                        |ui| {
                            if ui.button("Save Current Frame Snapshot").clicked() {
                                post_host_actions.push(WorkbenchHostAction::SaveCurrentFrame);
                                ui.close();
                            }
                            if ui.button("Prune Empty Named Frames").clicked() {
                                post_host_actions.push(WorkbenchHostAction::PruneEmptyFrames);
                                ui.close();
                            }
                            ui.separator();
                            if projection.saved_frame_names.is_empty() {
                                ui.label(RichText::new("No saved frames").small().weak());
                                return;
                            }
                            for frame_name in &projection.saved_frame_names {
                                let split_offer_suppressed =
                                    frame_split_offer_suppressed_for_name(graph_app, frame_name);
                                ui.menu_button(frame_name, |ui| {
                                    if ui.button("Open").clicked() {
                                        post_host_actions.push(WorkbenchHostAction::RestoreFrame(
                                            frame_name.clone(),
                                        ));
                                        ui.close();
                                    }
                                    let toggle_label = if split_offer_suppressed {
                                        "Re-enable split offer"
                                    } else {
                                        "Suppress split offer"
                                    };
                                    if ui.button(toggle_label).clicked() {
                                        post_host_actions.push(
                                            WorkbenchHostAction::SetFrameSplitOfferSuppressed {
                                                frame_name: frame_name.clone(),
                                                suppressed: !split_offer_suppressed,
                                            },
                                        );
                                        ui.close();
                                    }
                                    ui.label(
                                        RichText::new(if split_offer_suppressed {
                                            "Split offer suppressed"
                                        } else {
                                            "Split offer enabled"
                                        })
                                        .small()
                                        .weak(),
                                    );
                                });
                            }
                        },
                    );
                });

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Settings").clicked() {
                        post_host_actions.push(WorkbenchHostAction::OpenTool(ToolPaneState::Settings));
                    }
                    if ui.small_button("History").clicked() {
                        post_host_actions
                            .push(WorkbenchHostAction::OpenTool(ToolPaneState::HistoryManager));
                    }
                    if ui.small_button("Navigator").clicked() {
                        post_host_actions
                            .push(WorkbenchHostAction::OpenTool(ToolPaneState::navigator_surface()));
                    }
                });
                // Specialty graphlet view controls.
                let active_specialty = graph_app
                    .workspace
                    .workbench_session
                    .navigator_specialty_views
                    .get(&host_layout.host)
                    .cloned();
                let selected_node_count = graph_app.focused_selection().len();
                let has_selected_pair = graph_app.focused_selection().ordered_pair().is_some();
                ui.horizontal_wrapped(|ui| {
                    let ego_active = matches!(
                        active_specialty,
                        Some(ref sv) if matches!(sv.kind, GraphletKind::Ego { .. })
                    );
                    let ego_label = if ego_active { "★ Ego" } else { "Ego view" };
                    if ui
                        .add_enabled(
                            selected_node_count > 0 || ego_active,
                            egui::Button::new(ego_label),
                        )
                        .on_hover_text("Derive ego-graphlet from selection and show in this host")
                        .clicked()
                    {
                        let kind = if ego_active {
                            None
                        } else {
                            Some(GraphletKind::Ego { radius: 1 })
                        };
                        post_host_actions.push(WorkbenchHostAction::SetNavigatorSpecialtyView {
                            host: host_layout.host.clone(),
                            kind,
                        });
                    }
                    let corridor_active = matches!(
                        active_specialty,
                        Some(ref sv) if matches!(sv.kind, GraphletKind::Corridor)
                    );
                    let corridor_label = if corridor_active {
                        "★ Corridor"
                    } else {
                        "Corridor"
                    };
                    if ui
                        .add_enabled(
                            has_selected_pair || corridor_active,
                            egui::Button::new(corridor_label),
                        )
                        .on_hover_text(
                            "Derive a corridor graphlet from the current two-node selection and show it in this host",
                        )
                        .clicked()
                    {
                        let kind = if corridor_active {
                            None
                        } else {
                            Some(GraphletKind::Corridor)
                        };
                        post_host_actions.push(WorkbenchHostAction::SetNavigatorSpecialtyView {
                            host: host_layout.host.clone(),
                            kind,
                        });
                    }
                    let component_active = matches!(
                        active_specialty,
                        Some(ref sv) if matches!(sv.kind, GraphletKind::Component)
                    );
                    let component_label = if component_active {
                        "★ Component"
                    } else {
                        "Component"
                    };
                    if ui
                        .add_enabled(
                            selected_node_count > 0 || component_active,
                            egui::Button::new(component_label),
                        )
                        .on_hover_text(
                            "Derive a connected-component graphlet from the current selection and show it in this host",
                        )
                        .clicked()
                    {
                        let kind = if component_active {
                            None
                        } else {
                            Some(GraphletKind::Component)
                        };
                        post_host_actions.push(WorkbenchHostAction::SetNavigatorSpecialtyView {
                            host: host_layout.host.clone(),
                            kind,
                        });
                    }
                    if active_specialty.is_some() {
                        if ui
                            .small_button("✕ Clear view")
                            .on_hover_text("Clear specialty graphlet view")
                            .clicked()
                        {
                            post_host_actions.push(WorkbenchHostAction::SetNavigatorSpecialtyView {
                                host: host_layout.host.clone(),
                                kind: None,
                            });
                        }
                    }
                });
                ui.separator();

                // Specialty graphlet view panel — rendered when a specialty view
                // is active for this host.
                if let Some(sv) = graph_app
                    .workspace
                    .workbench_session
                    .navigator_specialty_views
                    .get(&host_layout.host)
                    .cloned()
                {
                    let kind_label = navigator_specialty_kind_label(&sv.kind);
                    ui.heading(format!("Specialty: {kind_label}"));
                    // Reserve the remaining panel space for the graph canvas.
                    // The actual canvas is rendered in an egui::Area after the
                    // panel show returns so we have &mut GraphBrowserApp.
                    let canvas_size = ui.available_size();
                    let (canvas_alloc_rect, _response) =
                        ui.allocate_exact_size(canvas_size, egui::Sense::hover());
                    specialty_canvas_rect = Some(canvas_alloc_rect);
                }

                if host_shows_graph_scope(&host_layout)
                    && matches!(host_layout.form_factor, WorkbenchHostFormFactor::Sidebar)
                {
                    let swatch_actions =
                        crate::shell::desktop::ui::overview_plane::render_navigator_overview_swatch(
                            ui,
                            graph_app,
                            &projection,
                        );
                    for action in swatch_actions {
                        for mapped_action in navigator_overview_action_to_host_actions(
                            action,
                            &host_layout.host,
                        ) {
                            post_host_actions.push(mapped_action);
                        }
                    }
                    ui.separator();
                }

                if host_shows_graph_scope(&host_layout) && !projection.navigator_groups.is_empty() {
                    ui.heading("Navigator");
                    for group in &projection.navigator_groups {
                        let header = egui::CollapsingHeader::new(if group.is_highlighted {
                            RichText::new(&group.title)
                                .small()
                                .strong()
                                .color(ui.visuals().selection.stroke.color)
                        } else {
                            RichText::new(&group.title).small().strong()
                        })
                        .id_salt(("workbench_host_navigator", &group.title))
                        .default_open(!matches!(
                            group.section,
                            WorkbenchNavigatorSection::Recent | WorkbenchNavigatorSection::Imported
                        ));
                        header.show(ui, |ui| {
                            for member in &group.members {
                                let indent_px = member.depth as f32 * 12.0;
                                let lifecycle_prefix = if member.is_cold { "○ " } else { "" };
                                let label = format!("{}{}", lifecycle_prefix, member.title);
                                ui.horizontal(|ui| {
                                    if indent_px > 0.0 {
                                        ui.add_space(indent_px);
                                    }
                                    // Expand/collapse toggle button for nodes with children.
                                    if member.has_children {
                                        let arrow = if member.is_expanded { "▾" } else { "▸" };
                                        if ui.small_button(arrow).clicked() {
                                            post_host_actions.push(
                                                WorkbenchHostAction::ToggleExpandNode(member.node_key),
                                            );
                                        }
                                    } else if member.depth > 0 {
                                        // Leaf indent spacer to align with siblings.
                                        ui.add_space(ui.spacing().button_padding.x * 2.0 + 8.0);
                                    }
                                    let response = ui.selectable_label(
                                        member.is_selected,
                                        RichText::new(label).small(),
                                    );
                                    if response.double_clicked() {
                                        if let Some(target_url) = member.target_url.clone() {
                                            post_host_actions.push(
                                                WorkbenchHostAction::ActivateNodeSemanticTarget {
                                                    node_key: member.node_key,
                                                    row_key: member.row_key.clone(),
                                                    target_url,
                                                },
                                            );
                                        } else {
                                            post_host_actions.push(WorkbenchHostAction::ActivateNode {
                                                node_key: member.node_key,
                                                row_key: member.row_key.clone(),
                                            });
                                        }
                                    } else if response.clicked() {
                                        if let Some(target_url) = member.target_url.clone() {
                                            post_host_actions.push(
                                                WorkbenchHostAction::SelectNodeSemanticTarget {
                                                    node_key: member.node_key,
                                                    row_key: member.row_key.clone(),
                                                    target_url,
                                                },
                                            );
                                        } else {
                                            post_host_actions.push(WorkbenchHostAction::SelectNode {
                                                node_key: member.node_key,
                                                row_key: member.row_key.clone(),
                                            });
                                        }
                                    }
                                });
                            }
                        });
                    }
                    ui.separator();
                }

                if host_shows_workbench_scope(&host_layout) {
                    ui.label(RichText::new("Open Panes").small().strong());
                    egui::ScrollArea::vertical()
                        .id_salt(("workbench_host_pane_list", index))
                        .show(ui, |ui| {
                            for entry in &projection.pane_entries {
                                render_pane_row(ui, entry, &mut post_host_actions);
                            }
                            ui.add_space(4.0);
                            egui::CollapsingHeader::new(RichText::new("Tile Structure").small().weak())
                                .id_salt(("workbench_host_tile_structure", index))
                                .default_open(false)
                                .show(ui, |ui| {
                                    if let Some(root) = projection.tree_root.as_ref() {
                                        render_tree_node(ui, root, 0, &mut post_host_actions);
                                    }
                                });
                        });
                }
            });
        };
        if let Some(overlay_rect) = host_overlay_rect(&host_layout, host_available_rect) {
            overlay_occlusions.push(overlay_rect);
            egui::Area::new(egui::Id::new(panel_id))
                .order(egui::Order::Foreground)
                .fixed_pos(overlay_rect.min)
                .show(ctx, |ui| {
                    ui.set_min_size(overlay_rect.size());
                    ui.set_max_size(overlay_rect.size());
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.set_width(overlay_rect.width());
                            ui.set_height(overlay_rect.height());
                            show_host_contents(ui);
                        });
                });
        } else {
            match host_layout.form_factor {
                WorkbenchHostFormFactor::Sidebar => {
                    let side_panel = match host_layout.anchor_edge {
                        AnchorEdge::Left => Panel::left(panel_id),
                        AnchorEdge::Right => Panel::right(panel_id),
                        AnchorEdge::Top | AnchorEdge::Bottom => Panel::right(panel_id),
                    };
                    let panel_response = side_panel
                        .resizable(host_layout.resizable)
                        .default_size(host_panel_default_extent)
                        .min_size(HOST_PANEL_MAX_FLOOR)
                        .max_size(host_panel_max_extent)
                        .show_inside(root_ui, |ui| {
                            show_host_contents_with_cross_axis_margins(ui, &host_layout, |ui| {
                                show_host_contents(ui);
                            });
                        });
                    actual_panel_extent = Some(panel_response.response.rect.width());
                }
                WorkbenchHostFormFactor::Toolbar => {
                    let top_bottom_panel = match host_layout.anchor_edge {
                        AnchorEdge::Top => egui::Panel::top(panel_id),
                        AnchorEdge::Bottom => egui::Panel::bottom(panel_id),
                        AnchorEdge::Left | AnchorEdge::Right => egui::Panel::top(panel_id),
                    };
                    let panel_response = top_bottom_panel
                        .resizable(host_layout.resizable)
                        .default_size(host_panel_default_extent)
                        .min_size(HOST_PANEL_MAX_FLOOR)
                        .max_size(host_panel_max_extent)
                        .show_inside(root_ui, |ui| {
                            show_host_contents_with_cross_axis_margins(ui, &host_layout, |ui| {
                                show_host_contents(ui);
                            });
                        });
                    actual_panel_extent = Some(panel_response.response.rect.height());
                }
            }
        }

        // Render the specialty graphlet canvas as an overlay Area once we have
        // &mut GraphBrowserApp again (after the panel show closure completes).
        if let (Some(canvas_rect), Some(view_id)) = (specialty_canvas_rect, specialty_view_id) {
            let canvas_area_id = specialty_canvas_area_id;
            egui::Area::new(canvas_area_id)
                .order(egui::Order::Foreground)
                .fixed_pos(canvas_rect.min)
                .show(ctx, |ui| {
                    ui.set_min_size(canvas_rect.size());
                    ui.set_max_size(canvas_rect.size());
                    let intents = tile_render_pass::render_specialty_graph_in_ui(
                        ui, graph_app, tiles_tree, graph_tree, view_id,
                    );
                    graph_app.apply_reducer_intents(intents);
                });
        }

        if let Some(rect) = rendered_rect {
            let actual_extent = match host_layout.form_factor {
                WorkbenchHostFormFactor::Sidebar => rect.width(),
                WorkbenchHostFormFactor::Toolbar => rect.height(),
            };
            if (actual_extent - host_panel_default_extent).abs() > 2.0 {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT,
                    byte_len: host_layout.host.to_string().len(),
                });
            }
        }

        // Feed back resized panel dimensions to the layout constraint so the
        // stored size_fraction stays in sync with the actual panel extent.
        if let Some(extent) = actual_panel_extent {
            if host_layout.resizable && !is_host_configuring(graph_app, &host_layout.host) {
                let axis_extent = match host_layout.form_factor {
                    WorkbenchHostFormFactor::Sidebar => host_available_rect.width(),
                    WorkbenchHostFormFactor::Toolbar => host_available_rect.height(),
                };
                if axis_extent > 0.0 {
                    let new_fraction = (extent / axis_extent)
                        .clamp(HOST_PANEL_MIN_FRACTION, HOST_PANEL_MAX_FRACTION);
                    if (new_fraction - host_layout.size_fraction).abs() > 0.005 {
                        post_host_actions.push(WorkbenchHostAction::SyncHostPanelSize {
                            surface_host: host_layout.host.clone(),
                            new_size_fraction: new_fraction,
                        });
                    }
                }
            }
        }
    }

    update_workbench_navigation_geometry(graph_app, ctx.content_rect(), overlay_occlusions);

    for action in post_host_actions {
        apply_workbench_host_action(action, graph_app, tiles_tree, graph_tree);
    }

    projection
}

fn render_host_config_controls(
    ui: &mut egui::Ui,
    host_layout: &WorkbenchHostLayout,
    missing_hosts: &[SurfaceHostId],
    actions: &mut Vec<WorkbenchHostAction>,
) {
    let overlay_spec =
        configuring_overlay_spec(true).expect("configuring overlays should exist in config mode");
    ui.label(RichText::new("Layout Config").small().strong());
    ui.horizontal_wrapped(|ui| {
        for anchor_edge in overlay_spec.edge_targets.iter().copied() {
            let selected = host_layout.anchor_edge == anchor_edge;
            if ui
                .selectable_label(selected, format!("Drop {:?}", anchor_edge))
                .clicked()
            {
                actions.push(WorkbenchHostAction::SetLayoutConstraintDraft {
                    surface_host: host_layout.host.clone(),
                    constraint: anchored_constraint_for_host_layout(
                        host_layout,
                        anchor_edge,
                        host_layout.size_fraction,
                        host_layout.cross_axis_margin_start_px,
                        host_layout.cross_axis_margin_end_px,
                        host_layout.resizable,
                    ),
                });
            }
        }
        if overlay_spec.has_unconstrain_target && ui.small_button("Float").clicked() {
            actions.push(WorkbenchHostAction::SetLayoutConstraintDraft {
                surface_host: host_layout.host.clone(),
                constraint: WorkbenchLayoutConstraint::Unconstrained,
            });
        }
    });

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Scope").small());
        for scope in [
            NavigatorHostScope::Auto,
            NavigatorHostScope::Both,
            NavigatorHostScope::GraphOnly,
            NavigatorHostScope::WorkbenchOnly,
        ] {
            let selected = host_layout.configured_scope == scope;
            if ui
                .selectable_label(selected, host_scope_label(scope))
                .clicked()
            {
                actions.push(WorkbenchHostAction::SetNavigatorHostScope {
                    surface_host: host_layout.host.clone(),
                    scope,
                });
            }
        }
    });

    if overlay_spec.has_size_slider {
        let mut size_fraction = host_layout.size_fraction;
        if ui
            .add(
                egui::Slider::new(
                    &mut size_fraction,
                    HOST_PANEL_MIN_FRACTION..=HOST_PANEL_MAX_FRACTION,
                )
                .text("Size"),
            )
            .changed()
        {
            actions.push(WorkbenchHostAction::SetLayoutConstraintDraft {
                surface_host: host_layout.host.clone(),
                constraint: anchored_constraint_for_host_layout(
                    host_layout,
                    host_layout.anchor_edge,
                    size_fraction,
                    host_layout.cross_axis_margin_start_px,
                    host_layout.cross_axis_margin_end_px,
                    host_layout.resizable,
                ),
            });
        }
    }

    let mut start_margin = host_layout.cross_axis_margin_start_px;
    let mut end_margin = host_layout.cross_axis_margin_end_px;
    let mut resizable = host_layout.resizable;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Margins").small());
        let mut margin_changed = false;
        if overlay_spec
            .margin_handle_labels
            .first()
            .is_some_and(|label| {
                ui.add(
                    egui::DragValue::new(&mut start_margin)
                        .speed(1.0)
                        .range(0.0..=HOST_PANEL_MARGIN_MAX)
                        .prefix(format!("{label} ")),
                )
                .changed()
            })
        {
            margin_changed = true;
        }
        if overlay_spec
            .margin_handle_labels
            .get(1)
            .is_some_and(|label| {
                ui.add(
                    egui::DragValue::new(&mut end_margin)
                        .speed(1.0)
                        .range(0.0..=HOST_PANEL_MARGIN_MAX)
                        .prefix(format!("{label} ")),
                )
                .changed()
            })
        {
            margin_changed = true;
        }
        if margin_changed || ui.checkbox(&mut resizable, "Resizable").changed() {
            actions.push(WorkbenchHostAction::SetLayoutConstraintDraft {
                surface_host: host_layout.host.clone(),
                constraint: anchored_constraint_for_host_layout(
                    host_layout,
                    host_layout.anchor_edge,
                    host_layout.size_fraction,
                    start_margin,
                    end_margin,
                    resizable,
                ),
            });
        }
    });

    if !missing_hosts.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Add host").small());
            for host in missing_hosts {
                if ui.small_button(host_display_name(host)).clicked() {
                    actions.push(WorkbenchHostAction::SetLayoutConstraintDraft {
                        surface_host: host.clone(),
                        constraint: WorkbenchLayoutConstraint::AnchoredSplit {
                            surface_host: host.clone(),
                            anchor_edge: default_anchor_edge_for_host(host),
                            anchor_size_fraction: HOST_PANEL_MAX_FRACTION * 0.75,
                            cross_axis_margin_start_px: 0.0,
                            cross_axis_margin_end_px: 0.0,
                            resizable: true,
                        },
                    });
                }
            }
        });
    }
}

#[derive(Clone, Debug, PartialEq)]
enum WorkbenchHostAction {
    FocusPane(PaneId),
    SelectNode {
        node_key: NodeKey,
        row_key: Option<String>,
    },
    SelectNodeSemanticTarget {
        node_key: NodeKey,
        row_key: Option<String>,
        target_url: String,
    },
    ActivateNode {
        node_key: NodeKey,
        row_key: Option<String>,
    },
    ActivateNodeSemanticTarget {
        node_key: NodeKey,
        row_key: Option<String>,
        target_url: String,
    },
    SplitPane(PaneId, SplitDirection),
    RestoreSemanticTabGroup {
        pane: PaneId,
        group_id: Uuid,
    },
    CollapseSemanticTabGroup {
        group_id: Uuid,
    },
    /// Close a non-node pane (graph view, tool). Preserves webview.
    ClosePane(PaneId),
    /// Dismiss a node pane: demotes to Cold but keeps graph edges intact.
    DismissNodePane(PaneId),
    SetPanePresentationMode {
        pane: PaneId,
        mode: PanePresentationMode,
    },
    SwapViewerBackend {
        pane: PaneId,
        node: NodeKey,
        viewer_id_override: Option<ViewerId>,
    },
    OpenTool(ToolPaneState),
    SetWorkbenchDisplayMode(WorkbenchDisplayMode),
    SetWorkbenchOverlayVisible(bool),
    SetWorkbenchPinned(bool),
    SetLayoutConstraintDraft {
        surface_host: SurfaceHostId,
        constraint: WorkbenchLayoutConstraint,
    },
    CommitLayoutConstraintDraft(SurfaceHostId),
    DiscardLayoutConstraintDraft(SurfaceHostId),
    SetSurfaceConfigMode {
        surface_host: SurfaceHostId,
        mode: UxConfigMode,
    },
    SetNavigatorHostScope {
        surface_host: SurfaceHostId,
        scope: NavigatorHostScope,
    },
    SetFirstUsePolicy(SurfaceFirstUsePolicy),
    SuppressFirstUsePromptForSession(SurfaceHostId),
    OpenFrameAsSplit {
        node_key: NodeKey,
        frame_name: String,
    },
    DismissFrameSplitOfferForSession(String),
    SetFrameSplitOfferSuppressed {
        frame_name: String,
        suppressed: bool,
    },
    RenameFrame {
        from: String,
        to: String,
    },
    DeleteFrame(String),
    SaveFrameSnapshotNamed(String),
    MoveFrameLayoutHint {
        frame_name: String,
        from_index: usize,
        to_index: usize,
    },
    RemoveFrameLayoutHint {
        frame_name: String,
        hint_index: usize,
    },
    SaveCurrentFrame,
    PruneEmptyFrames,
    RestoreFrame(String),
    FocusGraphView(GraphViewId),
    OpenGraphView(GraphViewId),
    SelectNodeInGraphView {
        view_id: GraphViewId,
        node_key: NodeKey,
    },
    OpenNavigatorSpecialtyFromNode {
        host: SurfaceHostId,
        view_id: GraphViewId,
        node_key: NodeKey,
        kind: GraphletKind,
    },
    MoveGraphViewSlot {
        view_id: GraphViewId,
        row: i32,
        col: i32,
    },
    TransferSelectedNodesToGraphView {
        source_view: GraphViewId,
        destination_view: GraphViewId,
    },
    ToggleOverviewPlane,
    RequestFitToScreen,
    SetViewLensId {
        view_id: GraphViewId,
        lens_id: String,
    },
    SetViewDimension {
        view_id: GraphViewId,
        dimension: crate::app::ViewDimension,
    },
    ToggleSemanticDepthView {
        view_id: GraphViewId,
    },
    ClearGraphViewFilter {
        view_id: GraphViewId,
    },
    SetPhysicsProfile {
        profile_id: String,
    },
    TogglePhysics,
    ReheatPhysics,
    /// Set or clear a graphlet specialty view on a Navigator host.
    SetNavigatorSpecialtyView {
        host: SurfaceHostId,
        kind: Option<GraphletKind>,
    },
    /// Toggle expand/collapse of a node's children in the navigator tree view.
    /// Dispatched to `graph_tree_commands::toggle_expand` when GraphTree is available.
    ToggleExpandNode(NodeKey),
    /// Sync the stored size_fraction with the actual panel extent after a
    /// drag-resize interaction.
    SyncHostPanelSize {
        surface_host: SurfaceHostId,
        new_size_fraction: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkbenchHostActionDispatchOutcome {
    Consumed,
    ContractWarning,
}

fn workbench_host_action_diagnostic_code(action: &WorkbenchHostAction) -> usize {
    match action {
        WorkbenchHostAction::FocusPane(_) => 1,
        WorkbenchHostAction::SelectNode { .. } => 2,
        WorkbenchHostAction::ActivateNode { .. } => 3,
        WorkbenchHostAction::SelectNodeSemanticTarget { .. } => 51,
        WorkbenchHostAction::ActivateNodeSemanticTarget { .. } => 52,
        WorkbenchHostAction::SplitPane(_, _) => 4,
        WorkbenchHostAction::RestoreSemanticTabGroup { .. } => 5,
        WorkbenchHostAction::CollapseSemanticTabGroup { .. } => 6,
        WorkbenchHostAction::ClosePane(_) => 7,
        WorkbenchHostAction::DismissNodePane(_) => 8,
        WorkbenchHostAction::SetPanePresentationMode { .. } => 9,
        WorkbenchHostAction::SwapViewerBackend { .. } => 10,
        WorkbenchHostAction::OpenTool(_) => 11,
        WorkbenchHostAction::SetWorkbenchDisplayMode(_) => 12,
        WorkbenchHostAction::SetWorkbenchOverlayVisible(_) => 13,
        WorkbenchHostAction::SetWorkbenchPinned(_) => 14,
        WorkbenchHostAction::SetLayoutConstraintDraft { .. } => 15,
        WorkbenchHostAction::CommitLayoutConstraintDraft(_) => 16,
        WorkbenchHostAction::DiscardLayoutConstraintDraft(_) => 17,
        WorkbenchHostAction::SetSurfaceConfigMode { .. } => 18,
        WorkbenchHostAction::SetNavigatorHostScope { .. } => 19,
        WorkbenchHostAction::SetFirstUsePolicy(_) => 20,
        WorkbenchHostAction::SuppressFirstUsePromptForSession(_) => 21,
        WorkbenchHostAction::OpenFrameAsSplit { .. } => 22,
        WorkbenchHostAction::DismissFrameSplitOfferForSession(_) => 23,
        WorkbenchHostAction::SetFrameSplitOfferSuppressed { .. } => 24,
        WorkbenchHostAction::RenameFrame { .. } => 25,
        WorkbenchHostAction::DeleteFrame(_) => 26,
        WorkbenchHostAction::SaveFrameSnapshotNamed(_) => 27,
        WorkbenchHostAction::MoveFrameLayoutHint { .. } => 28,
        WorkbenchHostAction::RemoveFrameLayoutHint { .. } => 29,
        WorkbenchHostAction::SaveCurrentFrame => 30,
        WorkbenchHostAction::PruneEmptyFrames => 31,
        WorkbenchHostAction::RestoreFrame(_) => 32,
        WorkbenchHostAction::FocusGraphView(_) => 33,
        WorkbenchHostAction::OpenGraphView(_) => 34,
        WorkbenchHostAction::SelectNodeInGraphView { .. } => 35,
        WorkbenchHostAction::OpenNavigatorSpecialtyFromNode { .. } => 36,
        WorkbenchHostAction::MoveGraphViewSlot { .. } => 37,
        WorkbenchHostAction::TransferSelectedNodesToGraphView { .. } => 38,
        WorkbenchHostAction::ToggleOverviewPlane => 39,
        WorkbenchHostAction::RequestFitToScreen => 40,
        WorkbenchHostAction::SetViewLensId { .. } => 41,
        WorkbenchHostAction::SetViewDimension { .. } => 42,
        WorkbenchHostAction::ToggleSemanticDepthView { .. } => 43,
        WorkbenchHostAction::ClearGraphViewFilter { .. } => 44,
        WorkbenchHostAction::SetPhysicsProfile { .. } => 45,
        WorkbenchHostAction::TogglePhysics => 46,
        WorkbenchHostAction::ReheatPhysics => 47,
        WorkbenchHostAction::SetNavigatorSpecialtyView { .. } => 48,
        WorkbenchHostAction::SyncHostPanelSize { .. } => 49,
        WorkbenchHostAction::ToggleExpandNode(_) => 50,
    }
}

fn navigator_overview_action_to_host_actions(
    action: crate::shell::desktop::ui::overview_plane::NavigatorOverviewAction,
    host: &SurfaceHostId,
) -> Vec<WorkbenchHostAction> {
    match action {
        crate::shell::desktop::ui::overview_plane::NavigatorOverviewAction::Surface(surface_action) => {
            match surface_action {
                crate::shell::desktop::ui::overview_plane::OverviewSurfaceAction::FocusView(view_id) => {
                    vec![WorkbenchHostAction::FocusGraphView(view_id)]
                }
                crate::shell::desktop::ui::overview_plane::OverviewSurfaceAction::OpenView(view_id) => {
                    vec![WorkbenchHostAction::OpenGraphView(view_id)]
                }
                crate::shell::desktop::ui::overview_plane::OverviewSurfaceAction::TransferSelectionToView {
                    source_view,
                    destination_view,
                } => {
                    vec![WorkbenchHostAction::TransferSelectedNodesToGraphView {
                        source_view,
                        destination_view,
                    }]
                }
                crate::shell::desktop::ui::overview_plane::OverviewSurfaceAction::ToggleOverviewPlane => {
                    vec![WorkbenchHostAction::ToggleOverviewPlane]
                }
            }
        }
        crate::shell::desktop::ui::overview_plane::NavigatorOverviewAction::SelectGraphletAnchor {
            view_id,
            node_key,
        } => {
            vec![WorkbenchHostAction::SelectNodeInGraphView { view_id, node_key }]
        }
        crate::shell::desktop::ui::overview_plane::NavigatorOverviewAction::OpenGraphletSpecialty {
            view_id,
            node_key,
            kind,
        } => {
            vec![WorkbenchHostAction::OpenNavigatorSpecialtyFromNode {
                host: host.clone(),
                view_id,
                node_key,
                kind,
            }]
        }
    }
}

fn select_node_in_graph_view(
    graph_app: &mut GraphBrowserApp,
    view_id: GraphViewId,
    node_key: NodeKey,
) {
    graph_app.apply_reducer_intents([
        GraphIntent::FocusGraphView { view_id },
        GraphIntent::SelectNode {
            key: node_key,
            multi_select: false,
        },
    ]);
}

fn emit_workbench_host_action_started(diagnostic_code: usize) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UX_DISPATCH_STARTED,
        byte_len: diagnostic_code,
    });
}

fn emit_workbench_host_action_outcome(
    diagnostic_code: usize,
    outcome: WorkbenchHostActionDispatchOutcome,
) {
    let channel_id = match outcome {
        WorkbenchHostActionDispatchOutcome::Consumed => CHANNEL_UX_DISPATCH_CONSUMED,
        WorkbenchHostActionDispatchOutcome::ContractWarning => CHANNEL_UX_CONTRACT_WARNING,
    };
    emit_event(DiagnosticEvent::MessageSent {
        channel_id,
        byte_len: diagnostic_code,
    });
}

fn navigator_specialty_kind_label(kind: &GraphletKind) -> &'static str {
    match kind {
        GraphletKind::Ego { .. } => "Ego",
        GraphletKind::Corridor => "Corridor",
        GraphletKind::Component => "Component",
        GraphletKind::Loop => "Loop",
        GraphletKind::Frontier => "Frontier",
        GraphletKind::Facet => "Facet",
        GraphletKind::Session => "Session",
        GraphletKind::Bridge => "Bridge",
        GraphletKind::WorkbenchCorrespondence => "Workbench",
    }
}

fn layer_state_label(layer_state: WorkbenchLayerState) -> &'static str {
    match layer_state {
        WorkbenchLayerState::GraphOnly => "Graph only",
        WorkbenchLayerState::GraphOverlayActive => "Graph overlay active",
        WorkbenchLayerState::WorkbenchOverlayActive => "Workbench overlay active",
        WorkbenchLayerState::WorkbenchActive => "Workbench active",
        WorkbenchLayerState::WorkbenchPinned => "Workbench pinned",
        WorkbenchLayerState::WorkbenchOnly => "Workbench only",
    }
}

fn render_semantic_tab_affordance_button(
    ui: &mut egui::Ui,
    entry: &WorkbenchPaneEntry,
    actions: &mut Vec<WorkbenchHostAction>,
) {
    let Some(affordance) = entry.semantic_tab_affordance else {
        return;
    };

    match affordance {
        semantic_tabs::SemanticTabAffordance::Restore {
            group_id,
            member_count,
        } => {
            if ui
                .small_button("Tabs")
                .on_hover_text(format!(
                    "Restore semantic tab group with {member_count} pane{}",
                    if member_count == 1 { "" } else { "s" }
                ))
                .clicked()
            {
                actions.push(WorkbenchHostAction::RestoreSemanticTabGroup {
                    pane: entry.pane_id,
                    group_id,
                });
            }
        }
        semantic_tabs::SemanticTabAffordance::Collapse {
            group_id,
            member_count,
        } => {
            if ui
                .small_button("Rest")
                .on_hover_text(format!(
                    "Collapse semantic tab group with {member_count} pane{} to pane rest",
                    if member_count == 1 { "" } else { "s" }
                ))
                .clicked()
            {
                actions.push(WorkbenchHostAction::CollapseSemanticTabGroup { group_id });
            }
        }
    }
}

fn pane_presentation_mode_label(mode: PanePresentationMode) -> &'static str {
    match mode {
        PanePresentationMode::Tiled => "Tiled",
        PanePresentationMode::Docked => "Docked",
        PanePresentationMode::Floating => "Floating",
        PanePresentationMode::Fullscreen => "Fullscreen",
    }
}

fn pane_presentation_mode_toggle(
    mode: PanePresentationMode,
) -> Option<(PanePresentationMode, &'static str, &'static str)> {
    match mode {
        PanePresentationMode::Tiled => Some((
            PanePresentationMode::Docked,
            "Dock",
            "Reduce chrome and lock this pane in place",
        )),
        PanePresentationMode::Docked => Some((
            PanePresentationMode::Tiled,
            "Tile",
            "Restore full tile chrome and normal pane mobility",
        )),
        PanePresentationMode::Floating | PanePresentationMode::Fullscreen => None,
    }
}

fn pane_supports_split(mode: PanePresentationMode) -> bool {
    matches!(mode, PanePresentationMode::Tiled)
}

fn render_pane_mode_controls(
    ui: &mut egui::Ui,
    entry: &WorkbenchPaneEntry,
    actions: &mut Vec<WorkbenchHostAction>,
) {
    ui.label(
        RichText::new(pane_presentation_mode_label(entry.presentation_mode))
            .small()
            .weak(),
    );

    if let Some((mode, label, hover_text)) = pane_presentation_mode_toggle(entry.presentation_mode)
    {
        if ui.small_button(label).on_hover_text(hover_text).clicked() {
            actions.push(WorkbenchHostAction::SetPanePresentationMode {
                pane: entry.pane_id,
                mode,
            });
        }
    }
}

fn viewer_backend_display_name(viewer_id: &str) -> &str {
    match viewer_id {
        "viewer:webview" => "Servo",
        "viewer:wry" => "Wry",
        "viewer:middlenet" => "MiddleNet",
        "viewer:plaintext" => "Text",
        "viewer:markdown" => "Markdown",
        "viewer:pdf" => "PDF",
        "viewer:csv" => "CSV",
        "viewer:audio" => "Audio",
        "viewer:settings" => "Settings",
        _ => viewer_id,
    }
}

fn viewer_runtime_badge(summary: &WorkbenchNodeViewerSummary) -> Option<(&'static str, String)> {
    if summary.runtime_crashed {
        return Some(("Crash", "Viewer crash recorded for this node.".to_string()));
    }
    if summary.runtime_blocked {
        return Some((
            "Blocked",
            "Viewer startup or backpressure is currently blocking this pane.".to_string(),
        ));
    }
    if let Some(reason) = summary.fallback_reason.as_deref() {
        return Some(("Degraded", reason.to_string()));
    }
    if summary.render_mode == TileRenderMode::Placeholder {
        return Some((
            "Degraded",
            "Viewer currently falls back to placeholder rendering.".to_string(),
        ));
    }
    None
}

fn viewer_override_badge(
    summary: &WorkbenchNodeViewerSummary,
) -> Option<(&'static str, &'static str)> {
    summary.viewer_override.as_ref()?;
    match summary.viewer_switch_reason {
        ViewerSwitchReason::UserRequested => Some((
            "Pinned",
            "Viewer override is pinned by the user for this pane.",
        )),
        ViewerSwitchReason::RecoveryPromptAccepted => Some((
            "Recovery",
            "Viewer override was accepted from a recovery prompt.",
        )),
        ViewerSwitchReason::PolicyPinned => Some((
            "Policy",
            "Viewer override is pinned by policy for this pane.",
        )),
    }
}

/// Short human-legible label plus expanded hover text for a verso
/// route reason. Surfaced in the workbench debug surface so users
/// can tell at a glance *why* a given engine was chosen.
fn verso_route_reason_badge(route: &::verso::VersoResolvedRoute) -> (&'static str, String) {
    use ::verso::{EngineChoice, VersoRouteReason, WebEnginePreference};
    let preference_label = |preference: WebEnginePreference| match preference {
        WebEnginePreference::Servo => "Servo",
        WebEnginePreference::Wry => "Wry",
    };
    let engine_label = |engine: EngineChoice| match engine {
        EngineChoice::Middlenet => "Middlenet",
        EngineChoice::Servo => "Servo",
        EngineChoice::Wry => "Wry",
        EngineChoice::Unsupported => "Unsupported",
    };
    match route.reason() {
        VersoRouteReason::MiddlenetLane(lane) => (
            "Lane",
            format!("Middlenet routed this content to the {:?} lane.", lane),
        ),
        VersoRouteReason::WebEnginePreferred(preference) => (
            "Preferred",
            format!(
                "Routed to {} because it matches the current web engine preference.",
                preference_label(*preference)
            ),
        ),
        VersoRouteReason::WebEngineFallback { preferred, used } => (
            "Fallback",
            format!(
                "Preferred {} is unavailable on this host; fell back to {}.",
                preference_label(*preferred),
                engine_label(*used)
            ),
        ),
        VersoRouteReason::UserOverride => (
            "Override",
            "Engine chosen by an explicit user override.".to_string(),
        ),
        VersoRouteReason::Unsupported => (
            "Unsupported",
            "No engine on this host can render this content.".to_string(),
        ),
    }
}

fn render_node_viewer_status_badges(ui: &mut egui::Ui, entry: &WorkbenchPaneEntry) {
    let Some(summary) = entry.node_viewer_summary.as_ref() else {
        return;
    };

    if let Some(viewer_id) = summary.effective_viewer_id.as_deref() {
        ui.label(
            RichText::new(viewer_backend_display_name(viewer_id))
                .small()
                .weak(),
        )
        .on_hover_text(viewer_id);
    }

    if let Some((badge, hover_text)) = viewer_override_badge(summary) {
        ui.label(
            RichText::new(badge)
                .small()
                .color(egui::Color32::from_rgb(120, 180, 220)),
        )
        .on_hover_text(hover_text);
    }

    if let Some((badge, hover_text)) = viewer_runtime_badge(summary) {
        ui.label(
            RichText::new(badge)
                .small()
                .color(egui::Color32::from_rgb(220, 180, 60)),
        )
        .on_hover_text(hover_text);
    }

    if let Some(route) = summary.verso_route.as_ref() {
        let (badge, hover_text) = verso_route_reason_badge(route);
        ui.label(
            RichText::new(badge)
                .small()
                .color(egui::Color32::from_rgb(150, 170, 200)),
        )
        .on_hover_text(hover_text);
    }
}

fn open_with_picker_visible(entry: &WorkbenchPaneEntry) -> bool {
    matches!(entry.kind, WorkbenchPaneKind::Node { .. })
        && entry.presentation_mode == PanePresentationMode::Tiled
        && entry.node_viewer_summary.as_ref().is_some_and(|summary| {
            summary.available_viewer_ids.len() > 1 || summary.viewer_override.is_some()
        })
}

fn quick_switch_viewer_ids(summary: &WorkbenchNodeViewerSummary) -> Vec<&str> {
    let mut viewer_ids = Vec::new();
    if summary
        .available_viewer_ids
        .iter()
        .any(|viewer_id| viewer_id == "viewer:middlenet")
    {
        viewer_ids.push("viewer:middlenet");
    }
    viewer_ids
}

fn render_inline_viewer_quick_switches(
    ui: &mut egui::Ui,
    entry: &WorkbenchPaneEntry,
    actions: &mut Vec<WorkbenchHostAction>,
) {
    let WorkbenchPaneKind::Node { node_key } = entry.kind else {
        return;
    };
    let Some(summary) = entry.node_viewer_summary.as_ref() else {
        return;
    };
    let quick_viewers = quick_switch_viewer_ids(summary);
    if quick_viewers.is_empty() {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Quick switch:").small().weak());
        let auto_selected = summary.viewer_override.is_none();
        if ui
            .selectable_label(auto_selected, "Auto")
            .on_hover_text("Use the registry-selected viewer for this content")
            .clicked()
        {
            actions.push(WorkbenchHostAction::SwapViewerBackend {
                pane: entry.pane_id,
                node: node_key,
                viewer_id_override: None,
            });
        }

        for viewer_id in quick_viewers {
            let selected = summary.viewer_override.as_deref() == Some(viewer_id);
            if ui
                .selectable_label(selected, viewer_backend_display_name(viewer_id))
                .on_hover_text(viewer_id)
                .clicked()
            {
                actions.push(WorkbenchHostAction::SwapViewerBackend {
                    pane: entry.pane_id,
                    node: node_key,
                    viewer_id_override: Some(ViewerId::new(viewer_id.to_string())),
                });
            }
        }
    });
}

fn render_open_with_picker(
    ui: &mut egui::Ui,
    entry: &WorkbenchPaneEntry,
    actions: &mut Vec<WorkbenchHostAction>,
    id_namespace: &'static str,
) {
    let WorkbenchPaneKind::Node { node_key } = entry.kind else {
        return;
    };
    let Some(summary) = entry.node_viewer_summary.as_ref() else {
        return;
    };

    egui::CollapsingHeader::new(RichText::new("Open with…").small())
        .id_salt((id_namespace, entry.pane_id))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let auto_selected = summary.viewer_override.is_none();
                if ui
                    .selectable_label(auto_selected, "Auto")
                    .on_hover_text("Use the registry-selected viewer for this content")
                    .clicked()
                {
                    actions.push(WorkbenchHostAction::SwapViewerBackend {
                        pane: entry.pane_id,
                        node: node_key,
                        viewer_id_override: None,
                    });
                }

                for viewer_id in &summary.available_viewer_ids {
                    let selected = summary.viewer_override.as_deref() == Some(viewer_id.as_str());
                    if ui
                        .selectable_label(selected, viewer_backend_display_name(viewer_id))
                        .on_hover_text(viewer_id)
                        .clicked()
                    {
                        actions.push(WorkbenchHostAction::SwapViewerBackend {
                            pane: entry.pane_id,
                            node: node_key,
                            viewer_id_override: Some(ViewerId::new(viewer_id.clone())),
                        });
                    }
                }
            });
        });
}

fn render_tree_node(
    ui: &mut egui::Ui,
    node: &WorkbenchChromeNode,
    depth: usize,
    actions: &mut Vec<WorkbenchHostAction>,
) {
    match node {
        WorkbenchChromeNode::Pane(entry) => {
            ui.add_space((depth as f32) * 10.0);
            ui.horizontal(|ui| {
                let compact_title = compact_host_panel_text(&entry.title);
                let text = if entry.is_active {
                    RichText::new(&compact_title).strong()
                } else {
                    RichText::new(&compact_title)
                };
                let response = ui
                    .selectable_label(entry.is_active, text)
                    .on_hover_text(&entry.title);
                if response.clicked() {
                    actions.push(WorkbenchHostAction::FocusPane(entry.pane_id));
                }
                render_pane_mode_controls(ui, entry, actions);
                render_node_viewer_status_badges(ui, entry);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_semantic_tab_affordance_button(ui, entry, actions);
                    if entry.closable && ui.small_button("x").on_hover_text("Close").clicked() {
                        actions.push(match entry.kind {
                            WorkbenchPaneKind::Node { .. } => {
                                WorkbenchHostAction::DismissNodePane(entry.pane_id)
                            }
                            _ => WorkbenchHostAction::ClosePane(entry.pane_id),
                        });
                    }
                    if pane_supports_split(entry.presentation_mode) {
                        if ui
                            .small_button("V")
                            .on_hover_text("Split vertically")
                            .clicked()
                        {
                            actions.push(WorkbenchHostAction::SplitPane(
                                entry.pane_id,
                                SplitDirection::Vertical,
                            ));
                        }
                        if ui
                            .small_button("H")
                            .on_hover_text("Split horizontally")
                            .clicked()
                        {
                            actions.push(WorkbenchHostAction::SplitPane(
                                entry.pane_id,
                                SplitDirection::Horizontal,
                            ));
                        }
                    }
                });
            });
            if let Some(subtitle) = &entry.subtitle {
                let compact_subtitle = compact_host_panel_text(subtitle);
                ui.add_space((depth as f32) * 10.0 + 2.0);
                ui.label(RichText::new(compact_subtitle).small().weak())
                    .on_hover_text(subtitle);
            }
            if !entry.arrangement_memberships.is_empty() {
                ui.add_space((depth as f32) * 10.0 + 2.0);
                ui.label(
                    RichText::new(format!(
                        "Memberships: {}",
                        entry.arrangement_memberships.join(", ")
                    ))
                    .small()
                    .weak(),
                );
            }
            if open_with_picker_visible(entry) {
                ui.add_space((depth as f32) * 10.0 + 2.0);
                render_inline_viewer_quick_switches(ui, entry, actions);
                ui.add_space(2.0);
                render_open_with_picker(ui, entry, actions, "workbench_host_tree_open_with");
            }
            ui.add_space(6.0);
        }
        WorkbenchChromeNode::Tabs {
            tile_id,
            label,
            children,
        }
        | WorkbenchChromeNode::Split {
            tile_id,
            label,
            children,
        }
        | WorkbenchChromeNode::Grid {
            tile_id,
            label,
            children,
        } => {
            let compact_label = compact_host_panel_text(label);
            let header = egui::CollapsingHeader::new(RichText::new(compact_label).small().strong())
                .id_salt(("workbench_host_container", tile_id))
                .default_open(true);
            header.show(ui, |ui| {
                for child in children {
                    render_tree_node(ui, child, depth + 1, actions);
                }
            });
            ui.add_space(4.0);
        }
    }
}

fn render_pane_row(
    ui: &mut egui::Ui,
    entry: &WorkbenchPaneEntry,
    actions: &mut Vec<WorkbenchHostAction>,
) {
    ui.horizontal(|ui| {
        let compact_title = compact_host_panel_text(&entry.title);
        let text = if entry.is_active {
            RichText::new(&compact_title).strong()
        } else {
            RichText::new(&compact_title)
        };
        let response = ui.selectable_label(entry.is_active, text);
        let response = if let Some(subtitle) = &entry.subtitle {
            response.on_hover_text(subtitle)
        } else {
            response
        };
        if response.clicked() {
            actions.push(WorkbenchHostAction::FocusPane(entry.pane_id));
        }
        render_pane_mode_controls(ui, entry, actions);
        render_node_viewer_status_badges(ui, entry);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            render_semantic_tab_affordance_button(ui, entry, actions);
            if entry.closable && ui.small_button("x").on_hover_text("Close").clicked() {
                actions.push(match entry.kind {
                    WorkbenchPaneKind::Node { .. } => {
                        WorkbenchHostAction::DismissNodePane(entry.pane_id)
                    }
                    _ => WorkbenchHostAction::ClosePane(entry.pane_id),
                });
            }
            if pane_supports_split(entry.presentation_mode) {
                if ui
                    .small_button("V")
                    .on_hover_text("Split vertically")
                    .clicked()
                {
                    actions.push(WorkbenchHostAction::SplitPane(
                        entry.pane_id,
                        SplitDirection::Vertical,
                    ));
                }
                if ui
                    .small_button("H")
                    .on_hover_text("Split horizontally")
                    .clicked()
                {
                    actions.push(WorkbenchHostAction::SplitPane(
                        entry.pane_id,
                        SplitDirection::Horizontal,
                    ));
                }
            }
        });
    });
    if open_with_picker_visible(entry) {
        render_inline_viewer_quick_switches(ui, entry, actions);
        render_open_with_picker(ui, entry, actions, "workbench_host_list_open_with");
    }
    ui.add_space(2.0);
}

fn apply_workbench_host_action(
    action: WorkbenchHostAction,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
) {
    let diagnostic_code = workbench_host_action_diagnostic_code(&action);
    emit_workbench_host_action_started(diagnostic_code);
    let outcome = match action {
        WorkbenchHostAction::FocusPane(pane_id) => {
            if tile_view_ops::focus_pane(tiles_tree, pane_id) {
                WorkbenchHostActionDispatchOutcome::Consumed
            } else {
                WorkbenchHostActionDispatchOutcome::ContractWarning
            }
        }
        WorkbenchHostAction::SelectNode { node_key, row_key } => {
            if let Some(row_key) = row_key {
                graph_app.set_navigator_selected_rows([row_key]);
            }
            graph_app.apply_reducer_intents([GraphIntent::SelectNode {
                key: node_key,
                multi_select: false,
            }]);
            if let Some(view_id) =
                offscreen_visible_graph_view_for_node(graph_app, tiles_tree, node_key)
            {
                graph_app
                    .request_camera_command_for_view(Some(view_id), CameraCommand::FitSelection);
            }
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SelectNodeSemanticTarget {
            node_key,
            row_key,
            target_url,
        } => {
            if let Some(row_key) = row_key {
                graph_app.set_navigator_selected_rows([row_key]);
            }
            graph_app.apply_reducer_intents([GraphIntent::SelectNode {
                key: node_key,
                multi_select: false,
            }]);
            graph_app.apply_reducer_intents([GraphIntent::SetNodeUrl {
                key: node_key,
                new_url: target_url,
            }]);
            if let Some(view_id) =
                offscreen_visible_graph_view_for_node(graph_app, tiles_tree, node_key)
            {
                graph_app
                    .request_camera_command_for_view(Some(view_id), CameraCommand::FitSelection);
            }
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::ActivateNode { node_key, row_key } => {
            if let Some(row_key) = row_key {
                graph_app.set_navigator_selected_rows([row_key]);
            }
            // Ensure the node is selected — ActivateNode is not a toggle.
            if !graph_app.focused_selection().contains(&node_key) {
                graph_app.apply_reducer_intents([GraphIntent::SelectNode {
                    key: node_key,
                    multi_select: false,
                }]);
            }
            if node_has_workbench_presentation(tiles_tree, node_key) {
                let _ = focus_node_presentation(tiles_tree, node_key);
            } else {
                let lifecycle = graph_app
                    .domain_graph()
                    .get_node(node_key)
                    .map(|n| n.lifecycle);
                if lifecycle == Some(crate::graph::NodeLifecycle::Cold) {
                    // Cold node: open a tile in the graphlet's tab group (Phase 4).
                    // If the node belongs to a durable graphlet with a warm member,
                    // graphlet routing joins that group. Otherwise a new tile is created.
                    tile_view_ops::open_node_with_graphlet_routing(tiles_tree, graph_app, node_key);
                } else {
                    // Pre-warmed node (lifecycle Active/Warm but no tile present):
                    // focus the graph canvas and fit the selection rather than opening a tile.
                    let graph_view_id = tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                        Tile::Pane(TileKind::Graph(r)) => Some(r.graph_view_id),
                        _ => None,
                    });
                    if let Some(view_id) = graph_view_id {
                        tiles_tree.make_active(|_, tile| {
                            matches!(tile, Tile::Pane(TileKind::Graph(r)) if r.graph_view_id == view_id)
                        });
                        graph_app.request_camera_command_for_view(
                            Some(view_id),
                            CameraCommand::FitSelection,
                        );
                    }
                }
            }
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::ActivateNodeSemanticTarget {
            node_key,
            row_key,
            target_url,
        } => {
            if let Some(row_key) = row_key {
                graph_app.set_navigator_selected_rows([row_key]);
            }
            if !graph_app.focused_selection().contains(&node_key) {
                graph_app.apply_reducer_intents([GraphIntent::SelectNode {
                    key: node_key,
                    multi_select: false,
                }]);
            }
            graph_app.apply_reducer_intents([GraphIntent::SetNodeUrl {
                key: node_key,
                new_url: target_url,
            }]);
            if node_has_workbench_presentation(tiles_tree, node_key) {
                let _ = focus_node_presentation(tiles_tree, node_key);
            } else {
                let lifecycle = graph_app
                    .domain_graph()
                    .get_node(node_key)
                    .map(|n| n.lifecycle);
                if lifecycle == Some(crate::graph::NodeLifecycle::Cold) {
                    tile_view_ops::open_node_with_graphlet_routing(tiles_tree, graph_app, node_key);
                } else {
                    let graph_view_id = tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                        Tile::Pane(TileKind::Graph(r)) => Some(r.graph_view_id),
                        _ => None,
                    });
                    if let Some(view_id) = graph_view_id {
                        tiles_tree.make_active(|_, tile| {
                            matches!(tile, Tile::Pane(TileKind::Graph(r)) if r.graph_view_id == view_id)
                        });
                        graph_app.request_camera_command_for_view(
                            Some(view_id),
                            CameraCommand::FitSelection,
                        );
                    }
                }
            }
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SplitPane(source_pane, direction) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SplitPane {
                source_pane,
                direction,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::RestoreSemanticTabGroup { pane, group_id } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::RestorePaneToSemanticTabGroup {
                pane,
                group_id,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::CollapseSemanticTabGroup { group_id } => {
            graph_app.enqueue_workbench_intent(
                WorkbenchIntent::CollapseSemanticTabGroupToPaneRest { group_id },
            );
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::ClosePane(pane) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::ClosePane {
                pane,
                restore_previous_focus: true,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::DismissNodePane(pane) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::DismissTile { pane });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetPanePresentationMode { pane, mode } => {
            graph_app
                .enqueue_workbench_intent(WorkbenchIntent::SetPanePresentationMode { pane, mode });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SwapViewerBackend {
            pane,
            node,
            viewer_id_override,
        } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SwapViewerBackend {
                pane,
                node,
                viewer_id_override,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::OpenTool(kind) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane { kind });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetWorkbenchDisplayMode(mode) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SetWorkbenchDisplayMode { mode });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetWorkbenchOverlayVisible(visible) => {
            graph_app
                .enqueue_workbench_intent(WorkbenchIntent::SetWorkbenchOverlayVisible { visible });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetWorkbenchPinned(pinned) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SetWorkbenchPinned { pinned });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetLayoutConstraintDraft {
            surface_host,
            constraint,
        } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SetLayoutConstraintDraft {
                surface_host,
                constraint,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::CommitLayoutConstraintDraft(surface_host) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::CommitLayoutConstraintDraft {
                surface_host,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::DiscardLayoutConstraintDraft(surface_host) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::DiscardLayoutConstraintDraft {
                surface_host,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetSurfaceConfigMode { surface_host, mode } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SetSurfaceConfigMode {
                surface_host,
                mode,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetNavigatorHostScope {
            surface_host,
            scope,
        } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SetNavigatorHostScope {
                surface_host,
                scope,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetFirstUsePolicy(policy) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SetFirstUsePolicy { policy });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SuppressFirstUsePromptForSession(surface_host) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SuppressFirstUsePromptForSession {
                surface_host,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::OpenFrameAsSplit {
            node_key,
            frame_name,
        } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::OpenFrameAsSplit {
                node_key,
                frame_name,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::DismissFrameSplitOfferForSession(frame_name) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::DismissFrameSplitOfferForSession {
                frame_name,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetFrameSplitOfferSuppressed {
            frame_name,
            suppressed,
        } => {
            if let Some(frame_key) = frame_key_for_name(graph_app, &frame_name) {
                graph_app.enqueue_workbench_intent(WorkbenchIntent::SetFrameSplitOfferSuppressed {
                    frame: frame_key,
                    suppressed,
                });
                WorkbenchHostActionDispatchOutcome::Consumed
            } else {
                WorkbenchHostActionDispatchOutcome::ContractWarning
            }
        }
        WorkbenchHostAction::RenameFrame { from, to } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::RenameFrame { from, to });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::DeleteFrame(frame_name) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::DeleteFrame { frame_name });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SaveFrameSnapshotNamed(name) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SaveFrameSnapshotNamed { name });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::MoveFrameLayoutHint {
            frame_name,
            from_index,
            to_index,
        } => {
            if let Some(frame_key) = frame_key_for_name(graph_app, &frame_name) {
                graph_app.enqueue_workbench_intent(WorkbenchIntent::MoveFrameLayoutHint {
                    frame: frame_key,
                    from_index,
                    to_index,
                });
                WorkbenchHostActionDispatchOutcome::Consumed
            } else {
                WorkbenchHostActionDispatchOutcome::ContractWarning
            }
        }
        WorkbenchHostAction::RemoveFrameLayoutHint {
            frame_name,
            hint_index,
        } => {
            if let Some(frame_key) = frame_key_for_name(graph_app, &frame_name) {
                graph_app.enqueue_workbench_intent(WorkbenchIntent::RemoveFrameLayoutHint {
                    frame: frame_key,
                    hint_index,
                });
                WorkbenchHostActionDispatchOutcome::Consumed
            } else {
                WorkbenchHostActionDispatchOutcome::ContractWarning
            }
        }
        WorkbenchHostAction::SaveCurrentFrame => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SaveCurrentFrame);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::PruneEmptyFrames => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::PruneEmptyFrames);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::RestoreFrame(name) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::RestoreFrame { name });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::FocusGraphView(view_id) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::FocusGraphView { view_id });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::OpenGraphView(view_id) => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::OpenGraphViewPane {
                view_id,
                mode: PendingTileOpenMode::Tab,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SelectNodeInGraphView { view_id, node_key } => {
            select_node_in_graph_view(graph_app, view_id, node_key);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::OpenNavigatorSpecialtyFromNode {
            host,
            view_id,
            node_key,
            kind,
        } => {
            select_node_in_graph_view(graph_app, view_id, node_key);
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SetNavigatorSpecialtyView {
                host,
                kind: Some(kind),
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::MoveGraphViewSlot { view_id, row, col } => {
            graph_app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot { view_id, row, col }]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::TransferSelectedNodesToGraphView {
            source_view,
            destination_view,
        } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::TransferSelectedNodesToGraphView {
                source_view,
                destination_view,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::ToggleOverviewPlane => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::ToggleOverviewPlane);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::RequestFitToScreen => {
            graph_app.apply_reducer_intents([GraphIntent::RequestFitToScreen]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetViewLensId { view_id, lens_id } => {
            graph_app.apply_reducer_intents([GraphIntent::SetViewLensId { view_id, lens_id }]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetViewDimension { view_id, dimension } => {
            graph_app.apply_reducer_intents([GraphIntent::SetViewDimension { view_id, dimension }]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::ToggleSemanticDepthView { view_id } => {
            graph_app.apply_reducer_intents([GraphIntent::ToggleSemanticDepthView { view_id }]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::ClearGraphViewFilter { view_id } => {
            graph_app.apply_reducer_intents([GraphIntent::ClearViewFilter { view_id }]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetPhysicsProfile { profile_id } => {
            graph_app.apply_reducer_intents([GraphIntent::SetPhysicsProfile { profile_id }]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::TogglePhysics => {
            graph_app.apply_reducer_intents([GraphIntent::TogglePhysics]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::ReheatPhysics => {
            graph_app.apply_reducer_intents([GraphIntent::ReheatPhysics]);
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SetNavigatorSpecialtyView { host, kind } => {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SetNavigatorSpecialtyView {
                host,
                kind,
            });
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::SyncHostPanelSize {
            surface_host,
            new_size_fraction,
        } => {
            if let Some(WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: sh,
                anchor_edge,
                cross_axis_margin_start_px,
                cross_axis_margin_end_px,
                resizable,
                ..
            }) = graph_app
                .workbench_layout_constraint_for_host(&surface_host)
                .cloned()
            {
                graph_app.set_workbench_layout_constraint(
                    surface_host,
                    WorkbenchLayoutConstraint::AnchoredSplit {
                        surface_host: sh,
                        anchor_edge,
                        anchor_size_fraction: new_size_fraction,
                        cross_axis_margin_start_px,
                        cross_axis_margin_end_px,
                        resizable,
                    },
                );
            }
            WorkbenchHostActionDispatchOutcome::Consumed
        }
        WorkbenchHostAction::ToggleExpandNode(node_key) => {
            crate::shell::desktop::workbench::graph_tree_commands::toggle_expand(
                graph_tree, node_key,
            );
            WorkbenchHostActionDispatchOutcome::Consumed
        }
    };
    emit_workbench_host_action_outcome(diagnostic_code, outcome);
}

fn frame_pin_name_for_node(node_key: NodeKey, graph_app: &GraphBrowserApp) -> Option<String> {
    let node = graph_app.domain_graph().get_node(node_key)?;
    let label = node_primary_label(node);
    let sanitized = label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        Some(format!("pane-node-{}", node_key.index()))
    } else {
        Some(format!("pane-{sanitized}"))
    }
}

fn toolbar_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text)
        .frame(false)
        .min_size(egui::Vec2 { x: 20.0, y: 20.0 })
}

fn render_frame_pin_controls(
    ui: &mut egui::Ui,
    has_hosted_panes: bool,
    focused_pane_pin_name: Option<&str>,
    persisted_frame_names: &HashSet<String>,
    post_host_actions: &mut Vec<WorkbenchHostAction>,
) {
    if !has_hosted_panes {
        return;
    }

    if let Some(pane_pin_name) = focused_pane_pin_name {
        let pane_is_pinned = persisted_frame_names.contains(pane_pin_name);
        let pane_pin_label = if pane_is_pinned { "P-" } else { "P+" };
        let pane_pin_button =
            ui.add(toolbar_button(pane_pin_label))
                .on_hover_text(if pane_is_pinned {
                    "Unpin focused pane frame snapshot"
                } else {
                    "Pin focused pane frame snapshot"
                });
        if pane_pin_button.clicked() {
            if pane_is_pinned {
                post_host_actions.push(WorkbenchHostAction::DeleteFrame(pane_pin_name.to_string()));
            } else {
                post_host_actions.push(WorkbenchHostAction::SaveFrameSnapshotNamed(
                    pane_pin_name.to_string(),
                ));
            }
        }
    }

    let workspace_pin_name = "workspace:pin:space";
    let space_is_pinned = persisted_frame_names.contains(workspace_pin_name);
    let space_pin_label = if space_is_pinned { "W-" } else { "W+" };
    let space_pin_button =
        ui.add(toolbar_button(space_pin_label))
            .on_hover_text(if space_is_pinned {
                "Unpin current frame snapshot"
            } else {
                "Pin current frame snapshot"
            });
    if space_pin_button.clicked() {
        if space_is_pinned {
            post_host_actions.push(WorkbenchHostAction::DeleteFrame(
                workspace_pin_name.to_string(),
            ));
        } else {
            post_host_actions.push(WorkbenchHostAction::SaveFrameSnapshotNamed(
                workspace_pin_name.to_string(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::app::workbench_layout_policy::NavigatorHostId;
    #[cfg(feature = "diagnostics")]
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, NodePaneState, ToolPaneRef};
    use egui_tiles::Tiles;
    use tempfile::TempDir;

    /// Test-only dispatch wrapper that creates a throwaway GraphTree.
    /// Production code passes the real GraphTree; tests that don't exercise
    /// ToggleExpandNode can use this convenience helper.
    fn apply_workbench_host_action_test(
        action: WorkbenchHostAction,
        graph_app: &mut GraphBrowserApp,
        tiles_tree: &mut Tree<TileKind>,
    ) {
        let mut gt = graph_tree::GraphTree::new(
            graph_tree::LayoutMode::TreeStyleTabs,
            graph_tree::ProjectionLens::Traversal,
        );
        apply_workbench_host_action(action, graph_app, tiles_tree, &mut gt);
    }

    #[cfg(feature = "diagnostics")]
    fn channel_count(snapshot: &serde_json::Value, channel: &str) -> u64 {
        snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(channel))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    }

    fn dispatch_pending_workbench_intents(app: &mut GraphBrowserApp, tree: &mut Tree<TileKind>) {
        for intent in app.take_pending_workbench_intents() {
            assert!(
                crate::shell::desktop::runtime::registries::dispatch_workbench_surface_intent(
                    app, tree, None, intent,
                )
                .is_none()
            );
        }
    }

    #[test]
    fn projection_is_graph_only_when_tree_has_only_graph_panes() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_host_graph_only", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.layer_state, WorkbenchLayerState::GraphOnly);
        assert_eq!(projection.chrome_policy, ChromeExposurePolicy::GraphOnly);
        assert_eq!(projection.host_layout.anchor_edge, AnchorEdge::Left);
        assert_eq!(
            projection.host_layout.form_factor,
            WorkbenchHostFormFactor::Sidebar
        );
        assert_eq!(
            projection
                .active_graph_view
                .as_ref()
                .map(|(view_id, _)| *view_id),
            Some(graph_view)
        );
        assert!(projection.extra_graph_views.is_empty());
        assert!(!projection.visible());
    }

    #[test]
    fn projection_includes_graph_view_switcher_projection() {
        let primary_view = GraphViewId::new();
        let secondary_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(primary_view);
        app.ensure_graph_view_registered(secondary_view);
        app.workspace.graph_runtime.focused_view = Some(primary_view);

        if let Some(view) = app.workspace.graph_runtime.views.get_mut(&primary_view) {
            view.name = "Primary".to_string();
        }
        if let Some(view) = app.workspace.graph_runtime.views.get_mut(&secondary_view) {
            view.name = "Secondary".to_string();
        }

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(primary_view)));
        let tree = Tree::new("workbench_host_graph_views_projection", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(
            projection.active_graph_view,
            Some((primary_view, "Primary".to_string()))
        );
        assert_eq!(
            projection.extra_graph_views,
            vec![(secondary_view, "Secondary".to_string())]
        );
        assert!(graph_view_switcher_visible(&projection));
    }

    #[test]
    fn graph_view_switcher_fallback_is_only_needed_without_rendered_graph_host() {
        let graph_scope_host = WorkbenchHostLayout {
            host: SurfaceHostId::Navigator(NavigatorHostId::Right),
            anchor_edge: AnchorEdge::Right,
            form_factor: WorkbenchHostFormFactor::Sidebar,
            configured_scope: NavigatorHostScope::GraphOnly,
            resolved_scope: NavigatorHostScope::GraphOnly,
            size_fraction: 0.15,
            cross_axis_margin_start_px: 0.0,
            cross_axis_margin_end_px: 0.0,
            resizable: true,
        };
        let workbench_scope_host = WorkbenchHostLayout {
            host: SurfaceHostId::Navigator(NavigatorHostId::Right),
            anchor_edge: AnchorEdge::Right,
            form_factor: WorkbenchHostFormFactor::Sidebar,
            configured_scope: NavigatorHostScope::WorkbenchOnly,
            resolved_scope: NavigatorHostScope::WorkbenchOnly,
            size_fraction: 0.15,
            cross_axis_margin_start_px: 0.0,
            cross_axis_margin_end_px: 0.0,
            resizable: true,
        };

        let graph_only_projection = WorkbenchChromeProjection {
            layer_state: WorkbenchLayerState::GraphOnly,
            chrome_policy: ChromeExposurePolicy::GraphOnly,
            host_layout: graph_scope_host.clone(),
            host_layouts: vec![graph_scope_host.clone()],
            active_graph_view: Some((GraphViewId::new(), "Primary".to_string())),
            extra_graph_views: vec![(GraphViewId::new(), "Secondary".to_string())],
            active_pane_title: None,
            active_frame_name: None,
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![],
            tree_root: None,
            active_graphlet_roster: vec![],
        };
        assert!(graph_scope_requires_fallback_toolbar_host(
            &graph_only_projection
        ));

        let rendered_graph_host_projection = WorkbenchChromeProjection {
            layer_state: WorkbenchLayerState::WorkbenchPinned,
            chrome_policy: ChromeExposurePolicy::GraphPlusWorkbenchHostPinned,
            host_layout: graph_scope_host.clone(),
            host_layouts: vec![graph_scope_host],
            active_graph_view: Some((GraphViewId::new(), "Primary".to_string())),
            extra_graph_views: vec![(GraphViewId::new(), "Secondary".to_string())],
            active_pane_title: None,
            active_frame_name: None,
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![],
            tree_root: None,
            active_graphlet_roster: vec![],
        };
        assert!(!graph_scope_requires_fallback_toolbar_host(
            &rendered_graph_host_projection
        ));

        let workbench_only_projection = WorkbenchChromeProjection {
            layer_state: WorkbenchLayerState::WorkbenchPinned,
            chrome_policy: ChromeExposurePolicy::GraphPlusWorkbenchHostPinned,
            host_layout: workbench_scope_host.clone(),
            host_layouts: vec![workbench_scope_host],
            active_graph_view: Some((GraphViewId::new(), "Primary".to_string())),
            extra_graph_views: vec![(GraphViewId::new(), "Secondary".to_string())],
            active_pane_title: None,
            active_frame_name: None,
            saved_frame_names: vec![],
            navigator_groups: vec![],
            pane_entries: vec![],
            tree_root: None,
            active_graphlet_roster: vec![],
        };
        assert!(graph_scope_requires_fallback_toolbar_host(
            &workbench_only_projection
        ));
    }

    #[test]
    fn set_pane_presentation_mode_action_updates_node_pane_mode() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/docked-mode".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let mut tree = Tree::new("workbench_host_set_presentation_mode", root, tiles);

        let pane_id = match tree.tiles.get(node_tile) {
            Some(Tile::Pane(TileKind::Node(state))) => state.pane_id,
            other => panic!("expected node pane tile, got {other:?}"),
        };

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetPanePresentationMode {
                pane: pane_id,
                mode: PanePresentationMode::Docked,
            },
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let docked_mode = match tree.tiles.get(node_tile) {
            Some(Tile::Pane(TileKind::Node(state))) => state.presentation_mode,
            other => panic!("expected node pane tile after docking, got {other:?}"),
        };
        assert_eq!(docked_mode, PanePresentationMode::Docked);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetPanePresentationMode {
                pane: pane_id,
                mode: PanePresentationMode::Tiled,
            },
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let tiled_mode = match tree.tiles.get(node_tile) {
            Some(Tile::Pane(TileKind::Node(state))) => state.presentation_mode,
            other => panic!("expected node pane tile after restore, got {other:?}"),
        };
        assert_eq!(tiled_mode, PanePresentationMode::Tiled);
    }

    #[test]
    fn swap_viewer_backend_action_updates_node_override() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/viewer-swap".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let mut tree = Tree::new("workbench_host_swap_viewer_backend", root, tiles);

        let pane_id = match tree.tiles.get(node_tile) {
            Some(Tile::Pane(TileKind::Node(state))) => state.pane_id,
            other => panic!("expected node pane tile, got {other:?}"),
        };

        apply_workbench_host_action_test(
            WorkbenchHostAction::SwapViewerBackend {
                pane: pane_id,
                node,
                viewer_id_override: Some(ViewerId::new("viewer:wry")),
            },
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let override_after_swap = match tree.tiles.get(node_tile) {
            Some(Tile::Pane(TileKind::Node(state))) => state.viewer_id_override.clone(),
            other => panic!("expected node pane tile after swap, got {other:?}"),
        };
        assert_eq!(override_after_swap, Some(ViewerId::new("viewer:wry")));

        apply_workbench_host_action_test(
            WorkbenchHostAction::SwapViewerBackend {
                pane: pane_id,
                node,
                viewer_id_override: None,
            },
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let override_after_reset = match tree.tiles.get(node_tile) {
            Some(Tile::Pane(TileKind::Node(state))) => state.viewer_id_override.clone(),
            other => panic!("expected node pane tile after reset, got {other:?}"),
        };
        assert_eq!(override_after_reset, None);
    }

    #[test]
    fn set_view_lens_action_updates_graph_view_policy() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let root = tiles.insert_tab_tile(vec![graph]);
        let mut tree = Tree::new("workbench_host_set_view_lens", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetViewLensId {
                view_id: graph_view,
                lens_id: crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY.to_string(),
            },
            &mut app,
            &mut tree,
        );

        let view = app
            .workspace
            .graph_runtime
            .views
            .get(&graph_view)
            .expect("graph view state");
        assert_eq!(
            view.resolved_lens_id(),
            Some(crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY)
        );
    }

    #[test]
    fn set_physics_profile_action_updates_workspace_default() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let root = tiles.insert_tab_tile(vec![graph]);
        let mut tree = Tree::new("workbench_host_set_physics_profile", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetPhysicsProfile {
                profile_id: crate::registries::atomic::lens::PHYSICS_ID_SETTLE.to_string(),
            },
            &mut app,
            &mut tree,
        );

        assert_eq!(
            app.default_registry_physics_id(),
            Some(crate::registries::atomic::lens::PHYSICS_ID_SETTLE)
        );
    }

    #[test]
    fn toggle_semantic_depth_action_switches_dimension_mode() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let root = tiles.insert_tab_tile(vec![graph]);
        let mut tree = Tree::new("workbench_host_toggle_semantic_depth", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::ToggleSemanticDepthView {
                view_id: graph_view,
            },
            &mut app,
            &mut tree,
        );

        let view = app
            .workspace
            .graph_runtime
            .views
            .get(&graph_view)
            .expect("graph view state");
        assert!(crate::app::is_semantic_depth_dimension(&view.dimension));
    }

    #[test]
    fn set_view_dimension_action_applies_explicit_dimension() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let root = tiles.insert_tab_tile(vec![graph]);
        let mut tree = Tree::new("workbench_host_set_view_dimension", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetViewDimension {
                view_id: graph_view,
                dimension: crate::app::default_view_dimension_for_mode(
                    crate::app::ThreeDMode::Isometric,
                ),
            },
            &mut app,
            &mut tree,
        );

        let view = app
            .workspace
            .graph_runtime
            .views
            .get(&graph_view)
            .expect("graph view state");
        assert!(matches!(
            view.dimension,
            crate::app::ViewDimension::ThreeD {
                mode: crate::app::ThreeDMode::Isometric,
                z_source: crate::app::ZSource::BfsDepth { scale: 12.0 }
            }
        ));
    }

    #[test]
    fn move_graph_view_slot_action_updates_slot_position() {
        let left = GraphViewId::new();
        let right = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(left);
        app.ensure_graph_view_registered(right);
        app.move_graph_view_slot(right, 0, 1);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(left)));
        let root = tiles.insert_tab_tile(vec![graph]);
        let mut tree = Tree::new("workbench_host_move_graph_view_slot", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::MoveGraphViewSlot {
                view_id: left,
                row: 0,
                col: 1,
            },
            &mut app,
            &mut tree,
        );

        let slots = &app.workspace.graph_runtime.graph_view_layout_manager.slots;
        let moved = slots.get(&left).expect("moved slot should exist");
        let swapped = slots.get(&right).expect("swapped slot should exist");
        assert_eq!((moved.row, moved.col), (0, 1));
        assert_eq!((swapped.row, swapped.col), (0, 0));
    }

    #[test]
    fn clear_graph_view_filter_action_clears_active_filter() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.apply_reducer_intents([GraphIntent::SetViewFilter {
            view_id: graph_view,
            expr: Some(crate::model::graph::filter::FacetExpr::Predicate(
                crate::model::graph::filter::FacetPredicate {
                    facet_key: crate::model::graph::filter::facet_keys::UDC_CLASSES.to_string(),
                    operator: crate::model::graph::filter::FacetOperator::Eq,
                    operand: crate::model::graph::filter::FacetOperand::Scalar(
                        crate::model::graph::filter::FacetScalar::Text("udc:51".to_string()),
                    ),
                },
            )),
        }]);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let root = tiles.insert_tab_tile(vec![graph]);
        let mut tree = Tree::new("workbench_host_clear_graph_view_filter", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::ClearGraphViewFilter {
                view_id: graph_view,
            },
            &mut app,
            &mut tree,
        );

        let view = app
            .workspace
            .graph_runtime
            .views
            .get(&graph_view)
            .expect("graph view state");
        assert!(view.effective_filter_expr().is_none());
    }

    #[test]
    fn graph_scope_control_summary_exposes_active_filter_chip_labels() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.workspace.graph_runtime.focused_view = Some(graph_view);
        app.apply_reducer_intents([GraphIntent::SetViewFilter {
            view_id: graph_view,
            expr: Some(crate::model::graph::filter::FacetExpr::And(vec![
                crate::model::graph::filter::FacetExpr::Predicate(
                    crate::model::graph::filter::FacetPredicate {
                        facet_key: crate::model::graph::filter::facet_keys::UDC_CLASSES.to_string(),
                        operator: crate::model::graph::filter::FacetOperator::Eq,
                        operand: crate::model::graph::filter::FacetOperand::Scalar(
                            crate::model::graph::filter::FacetScalar::Text("udc:51".to_string()),
                        ),
                    },
                ),
                crate::model::graph::filter::FacetExpr::Predicate(
                    crate::model::graph::filter::FacetPredicate {
                        facet_key: crate::model::graph::filter::facet_keys::TITLE.to_string(),
                        operator: crate::model::graph::filter::FacetOperator::Eq,
                        operand: crate::model::graph::filter::FacetOperand::Scalar(
                            crate::model::graph::filter::FacetScalar::Text("Atlas".to_string()),
                        ),
                    },
                ),
            ])),
        }]);

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_host_filter_chip_projection", root, tiles);
        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let summary = active_graph_scope_control_summary(&projection, &app)
            .expect("graph scope summary should exist");

        assert_eq!(
            summary.active_filter_chip_labels,
            vec!["udc_classes=udc:51".to_string(), "title=Atlas".to_string()]
        );
    }

    #[test]
    fn projection_reports_graph_overlay_without_host() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.workspace.chrome_ui.show_command_palette = true;
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_host_graph_overlay", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(
            projection.layer_state,
            WorkbenchLayerState::GraphOverlayActive
        );
        assert_eq!(
            projection.chrome_policy,
            ChromeExposurePolicy::GraphWithOverlay
        );
        assert!(!projection.visible());
    }

    #[test]
    fn projection_stays_visible_when_host_is_pinned() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.set_workbench_host_pinned(true);
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_host_pinned", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.layer_state, WorkbenchLayerState::WorkbenchPinned);
        assert_eq!(
            projection.chrome_policy,
            ChromeExposurePolicy::GraphPlusWorkbenchHostPinned
        );
        assert_eq!(projection.host_layout.anchor_edge, AnchorEdge::Left);
        assert_eq!(
            projection.host_layout.form_factor,
            WorkbenchHostFormFactor::Sidebar
        );
        assert!(projection.visible());
    }

    #[test]
    fn projection_prefers_overlay_state_over_pinned_split_when_visible() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.set_workbench_host_pinned(true);
        app.set_workbench_overlay_visible(true);
        let node_key = app.add_node_and_sync(
            "https://example.com/overlay".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        let tree = Tree::new("workbench_host_overlay", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(
            projection.layer_state,
            WorkbenchLayerState::WorkbenchOverlayActive
        );
        assert_eq!(
            projection.chrome_policy,
            ChromeExposurePolicy::GraphPlusWorkbenchOverlay
        );
        assert!(projection.visible());
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn projection_switches_to_dedicated_workbench_mode_when_configured() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.set_workbench_display_mode(WorkbenchDisplayMode::Dedicated);
        let node_key = app.add_node_and_sync(
            "https://example.com/dedicated".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        let tree = Tree::new("workbench_host_dedicated", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.layer_state, WorkbenchLayerState::WorkbenchOnly);
        assert_eq!(
            projection.chrome_policy,
            ChromeExposurePolicy::WorkbenchOnly
        );
        assert!(projection.visible());
    }

    #[test]
    fn projection_uses_right_anchor_when_sidebar_preference_is_right() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.set_workbench_host_pinned(true);
        app.set_navigator_sidebar_side_preference(
            crate::app::NavigatorSidebarSidePreference::Right,
        );
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_host_prefers_right_sidebar", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(
            projection.host_layout.host,
            SurfaceHostId::Navigator(NavigatorHostId::Right)
        );
        assert_eq!(projection.host_layout.anchor_edge, AnchorEdge::Right);
        assert!(projection.visible());
    }

    #[test]
    fn projection_uses_runtime_layout_constraint_for_toolbar_host_geometry() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.set_workbench_host_pinned(true);
        app.set_workbench_layout_constraint(
            SurfaceHostId::Navigator(NavigatorHostId::Right),
            WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: SurfaceHostId::Navigator(NavigatorHostId::Right),
                anchor_edge: AnchorEdge::Top,
                anchor_size_fraction: 0.18,
                cross_axis_margin_start_px: 24.0,
                cross_axis_margin_end_px: 16.0,
                resizable: false,
            },
        );
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_host_toolbar_constraint", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(
            projection.host_layout.host,
            SurfaceHostId::Navigator(NavigatorHostId::Right)
        );
        assert_eq!(projection.host_layout.anchor_edge, AnchorEdge::Top);
        assert_eq!(
            projection.host_layout.form_factor,
            WorkbenchHostFormFactor::Toolbar
        );
        assert_eq!(projection.host_layout.size_fraction, 0.18);
        assert_eq!(projection.host_layout.cross_axis_margin_start_px, 24.0);
        assert_eq!(projection.host_layout.cross_axis_margin_end_px, 16.0);
        assert!(!projection.host_layout.resizable);
    }

    #[test]
    fn node_primary_label_uses_clip_visible_metadata() {
        let mut node = crate::graph::Node::test_stub("verso://clip/clip-host-label");
        node.title.clear();
        node.replace_history_state(vec!["https://example.com/source".to_string()], 0);
        node.session_form_draft = Some(
            r#"{"source_url":"https://example.com/source","page_title":"Example Source","clip_title":"Host Label Clip","text_excerpt":"excerpt","tag_name":"article","href":null,"image_url":null,"dom_path":"body > article:nth-of-type(1)","document_html":"<html><body>clip</body></html>"}"#.to_string(),
        );

        assert_eq!(node_primary_label(&node), "Host Label Clip");
        assert_eq!(
            node_pane_entry_subtitle(&node).as_deref(),
            Some("https://example.com/source")
        );
    }

    #[test]
    fn host_overlay_layout_is_enabled_only_when_cross_axis_margins_are_present() {
        let host = WorkbenchHostLayout::default_for_host(
            SurfaceHostId::Navigator(NavigatorHostId::Right),
            false,
        );
        assert!(!host_uses_overlay_layout(&host));

        let with_margins = WorkbenchHostLayout {
            cross_axis_margin_start_px: 18.0,
            ..host
        };
        assert!(host_uses_overlay_layout(&with_margins));
    }

    #[test]
    fn host_overlay_rect_for_sidebar_respects_vertical_margins() {
        let host = WorkbenchHostLayout {
            host: SurfaceHostId::Navigator(NavigatorHostId::Right),
            anchor_edge: AnchorEdge::Right,
            form_factor: WorkbenchHostFormFactor::Sidebar,
            configured_scope: NavigatorHostScope::Both,
            resolved_scope: NavigatorHostScope::Both,
            size_fraction: 0.2,
            cross_axis_margin_start_px: 20.0,
            cross_axis_margin_end_px: 30.0,
            resizable: true,
        };
        let rect = host_overlay_rect(
            &host,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1000.0, 800.0)),
        )
        .expect("sidebar host with margins should use overlay rect");

        assert_eq!(rect.min.y, 20.0);
        assert_eq!(rect.max.y, 770.0);
        assert_eq!(rect.min.x, 800.0);
        assert_eq!(rect.max.x, 1000.0);
    }

    #[test]
    fn host_overlay_rect_for_toolbar_respects_horizontal_margins() {
        let host = WorkbenchHostLayout {
            host: SurfaceHostId::Navigator(NavigatorHostId::Top),
            anchor_edge: AnchorEdge::Top,
            form_factor: WorkbenchHostFormFactor::Toolbar,
            configured_scope: NavigatorHostScope::Both,
            resolved_scope: NavigatorHostScope::Both,
            size_fraction: 0.2,
            cross_axis_margin_start_px: 40.0,
            cross_axis_margin_end_px: 60.0,
            resizable: true,
        };
        let rect = host_overlay_rect(
            &host,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1200.0, 500.0)),
        )
        .expect("toolbar host with margins should use overlay rect");

        assert_eq!(rect.min.x, 40.0);
        assert_eq!(rect.max.x, 1140.0);
        assert_eq!(rect.min.y, 0.0);
        assert_eq!(rect.max.y, 180.0);
    }

    #[test]
    fn first_use_prompt_visibility_respects_terminal_outcomes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let host = SurfaceHostId::Navigator(NavigatorHostId::Right);

        assert!(first_use_prompt_visible(&app, &host));

        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::ConfigureNow),
        });
        assert!(first_use_prompt_visible(&app, &host));

        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::RememberedConstraint(
                WorkbenchLayoutConstraint::anchored_split(host.clone(), AnchorEdge::Top, 0.2),
            )),
        });
        assert!(!first_use_prompt_visible(&app, &host));

        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::Discarded),
        });
        assert!(!first_use_prompt_visible(&app, &host));

        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::Dismissed),
        });
        assert!(!first_use_prompt_visible(&app, &host));

        app.suppress_first_use_prompt_for_session(host.clone());
        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::ConfigureNow),
        });
        assert!(!first_use_prompt_visible(&app, &host));
    }

    #[test]
    fn first_use_prompt_visibility_hides_when_constraint_is_already_persisted() {
        let mut app = GraphBrowserApp::new_for_testing();
        let host = SurfaceHostId::Navigator(NavigatorHostId::Right);
        app.set_workbench_layout_constraint(
            host.clone(),
            WorkbenchLayoutConstraint::anchored_split(host.clone(), AnchorEdge::Right, 0.2),
        );

        assert!(!first_use_prompt_visible(&app, &host));
    }

    #[test]
    fn configuring_overlay_spec_matches_spec_and_is_hidden_when_locked() {
        let locked = configuring_overlay_spec(false);
        assert!(locked.is_none());

        let configuring =
            configuring_overlay_spec(true).expect("configuring overlays should exist");
        assert_eq!(
            configuring.edge_targets,
            vec![
                AnchorEdge::Top,
                AnchorEdge::Bottom,
                AnchorEdge::Left,
                AnchorEdge::Right,
            ]
        );
        assert!(configuring.has_unconstrain_target);
        assert!(configuring.has_size_slider);
        assert_eq!(configuring.margin_handle_labels, vec!["Start", "End"]);
    }

    #[test]
    fn projection_collects_multiple_navigator_host_layouts_in_anchor_priority_order() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.set_workbench_host_pinned(true);
        app.set_workbench_layout_constraint(
            SurfaceHostId::Navigator(NavigatorHostId::Bottom),
            WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: SurfaceHostId::Navigator(NavigatorHostId::Bottom),
                anchor_edge: AnchorEdge::Bottom,
                anchor_size_fraction: 0.12,
                cross_axis_margin_start_px: 0.0,
                cross_axis_margin_end_px: 0.0,
                resizable: true,
            },
        );
        app.set_workbench_layout_constraint(
            SurfaceHostId::Navigator(NavigatorHostId::Left),
            WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: SurfaceHostId::Navigator(NavigatorHostId::Left),
                anchor_edge: AnchorEdge::Left,
                anchor_size_fraction: 0.16,
                cross_axis_margin_start_px: 8.0,
                cross_axis_margin_end_px: 10.0,
                resizable: false,
            },
        );

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_host_multi_host_layouts", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.host_layouts.len(), 2);
        assert_eq!(
            projection.host_layouts[0].host,
            SurfaceHostId::Navigator(NavigatorHostId::Bottom)
        );
        assert_eq!(projection.host_layouts[0].anchor_edge, AnchorEdge::Bottom);
        assert_eq!(
            projection.host_layouts[1].host,
            SurfaceHostId::Navigator(NavigatorHostId::Left)
        );
        assert_eq!(projection.host_layouts[1].anchor_edge, AnchorEdge::Left);
    }

    #[test]
    fn projection_preserves_independent_scope_settings_for_multiple_hosts() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.set_workbench_host_pinned(true);

        let top_host = SurfaceHostId::Navigator(NavigatorHostId::Top);
        let bottom_host = SurfaceHostId::Navigator(NavigatorHostId::Bottom);
        app.set_navigator_host_scope(top_host.clone(), NavigatorHostScope::GraphOnly);
        app.set_navigator_host_scope(bottom_host.clone(), NavigatorHostScope::WorkbenchOnly);
        app.set_workbench_layout_constraint(
            top_host.clone(),
            WorkbenchLayoutConstraint::anchored_split(top_host, AnchorEdge::Top, 0.14),
        );
        app.set_workbench_layout_constraint(
            bottom_host.clone(),
            WorkbenchLayoutConstraint::anchored_split(bottom_host, AnchorEdge::Bottom, 0.16),
        );

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let tree = Tree::new("workbench_host_multi_scope_layouts", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);
        assert_eq!(projection.host_layouts.len(), 2);
        assert_eq!(
            projection.host_layouts[0].resolved_scope,
            NavigatorHostScope::GraphOnly
        );
        assert_eq!(
            projection.host_layouts[1].resolved_scope,
            NavigatorHostScope::WorkbenchOnly
        );
    }

    #[test]
    fn session_only_first_use_follow_up_clears_terminal_outcome_but_keeps_active_draft() {
        let mut app = GraphBrowserApp::new_for_testing();
        let host = SurfaceHostId::Navigator(NavigatorHostId::Top);
        let draft_constraint = WorkbenchLayoutConstraint::AnchoredSplit {
            surface_host: host.clone(),
            anchor_edge: AnchorEdge::Left,
            anchor_size_fraction: 0.21,
            cross_axis_margin_start_px: 18.0,
            cross_axis_margin_end_px: 12.0,
            resizable: true,
        };

        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::ConfigureNow),
        });
        app.set_workbench_layout_constraint_draft(host.clone(), draft_constraint.clone());

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_session_only_follow_up", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetFirstUsePolicy(SurfaceFirstUsePolicy {
                surface_host: host.clone(),
                prompt_shown: true,
                outcome: None,
            }),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SuppressFirstUsePromptForSession(host.clone()),
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let policy = app
            .workbench_profile()
            .first_use_policies
            .get(&host)
            .expect("first-use policy should exist");
        assert_eq!(policy.outcome, None);
        assert_eq!(
            app.workbench_layout_constraint_draft_for_host(&host),
            Some(&draft_constraint)
        );
        assert!(app.is_first_use_prompt_suppressed_for_session(&host));
    }

    #[test]
    fn navigator_reconfiguration_drag_across_axis_preserves_scope_and_commits_margins() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let host = SurfaceHostId::Navigator(NavigatorHostId::Top);

        app.set_navigator_host_scope(host.clone(), NavigatorHostScope::GraphOnly);
        app.set_workbench_surface_config_mode(UxConfigMode::Configuring {
            surface_host: host.clone(),
        });

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let mut tree = Tree::new("workbench_host_reconfigure_across_axis", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetLayoutConstraintDraft {
                surface_host: host.clone(),
                constraint: WorkbenchLayoutConstraint::AnchoredSplit {
                    surface_host: host.clone(),
                    anchor_edge: AnchorEdge::Left,
                    anchor_size_fraction: 0.22,
                    cross_axis_margin_start_px: 20.0,
                    cross_axis_margin_end_px: 14.0,
                    resizable: true,
                },
            },
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let draft_constraint = app
            .workbench_layout_constraint_draft_for_host(&host)
            .cloned()
            .expect("configuring drag should create a draft constraint");
        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);
        assert_eq!(projection.host_layout.anchor_edge, AnchorEdge::Left);
        assert_eq!(
            projection.host_layout.form_factor,
            WorkbenchHostFormFactor::Sidebar
        );
        assert_eq!(projection.host_layout.cross_axis_margin_start_px, 20.0);
        assert_eq!(projection.host_layout.cross_axis_margin_end_px, 14.0);
        assert_eq!(
            projection.host_layout.resolved_scope,
            NavigatorHostScope::GraphOnly
        );

        apply_workbench_host_action_test(
            WorkbenchHostAction::CommitLayoutConstraintDraft(host.clone()),
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);
        app.set_workbench_surface_config_mode(UxConfigMode::Locked);

        assert!(
            app.workbench_layout_constraint_draft_for_host(&host)
                .is_none()
        );
        assert_eq!(
            app.workbench_profile().layout_constraints.get(&host),
            Some(&draft_constraint)
        );
        assert_eq!(
            app.navigator_host_scope(&host),
            NavigatorHostScope::GraphOnly
        );
    }

    #[test]
    fn navigator_first_use_flow_persists_reconfigured_host_across_restart() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().to_path_buf();
        let host = SurfaceHostId::Navigator(NavigatorHostId::Top);

        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        assert!(first_use_prompt_visible(&app, &host));

        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::ConfigureNow),
        });
        app.set_workbench_surface_config_mode(UxConfigMode::Configuring {
            surface_host: host.clone(),
        });
        app.set_navigator_host_scope(host.clone(), NavigatorHostScope::GraphOnly);
        app.set_workbench_layout_constraint_draft(
            host.clone(),
            WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: host.clone(),
                anchor_edge: AnchorEdge::Left,
                anchor_size_fraction: 0.22,
                cross_axis_margin_start_px: 20.0,
                cross_axis_margin_end_px: 14.0,
                resizable: true,
            },
        );
        app.set_workbench_surface_config_mode(UxConfigMode::Locked);
        assert!(
            app.workbench_layout_constraint_draft_for_host(&host)
                .is_some()
        );

        let remembered_constraint = app
            .workbench_layout_constraint_for_host(&host)
            .cloned()
            .expect("draft should still be active");
        app.commit_workbench_layout_constraint_draft(&host);
        app.set_surface_first_use_policy(SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: Some(FirstUseOutcome::RememberedConstraint(
                remembered_constraint.clone(),
            )),
        });
        drop(app);

        let reopened = GraphBrowserApp::new_from_dir(path);
        let reopened_constraint = reopened
            .workbench_profile()
            .layout_constraints
            .get(&host)
            .expect("remembered navigator host should restore after restart");
        assert_eq!(reopened_constraint, &remembered_constraint);
        assert_eq!(
            reopened.navigator_host_scope(&host),
            NavigatorHostScope::GraphOnly
        );
        assert!(!first_use_prompt_visible(&reopened, &host));

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let tree = Tree::new("workbench_host_restart_projection", root, tiles);
        let projection = WorkbenchChromeProjection::from_tree(&reopened, &tree, None);
        assert_eq!(projection.host_layout.anchor_edge, AnchorEdge::Left);
        assert_eq!(
            projection.host_layout.form_factor,
            WorkbenchHostFormFactor::Sidebar
        );
        assert_eq!(projection.host_layout.cross_axis_margin_start_px, 20.0);
        assert_eq!(projection.host_layout.cross_axis_margin_end_px, 14.0);
        assert_eq!(
            projection.host_layout.resolved_scope,
            NavigatorHostScope::GraphOnly
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn set_first_use_policy_emits_prompt_shown_only_for_visible_prompt_state() {
        let mut diagnostics = DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("first_use_prompt_shown", root, tiles);
        let host = SurfaceHostId::Navigator(NavigatorHostId::Right);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetFirstUsePolicy(SurfaceFirstUsePolicy {
                surface_host: host.clone(),
                prompt_shown: true,
                outcome: None,
            }),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SetFirstUsePolicy(SurfaceFirstUsePolicy {
                surface_host: host,
                prompt_shown: true,
                outcome: Some(FirstUseOutcome::AcceptDefault),
            }),
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert_eq!(
            channel_count(
                &snapshot,
                crate::shell::desktop::runtime::registries::CHANNEL_UX_FIRST_USE_PROMPT_SHOWN,
            ),
            1
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn workbench_host_action_emits_dispatch_started_and_consumed() {
        let mut diagnostics = DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_dispatch_consumed", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetWorkbenchPinned(true),
            &mut app,
            &mut tree,
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert_eq!(channel_count(&snapshot, CHANNEL_UX_DISPATCH_STARTED), 1);
        assert_eq!(channel_count(&snapshot, CHANNEL_UX_DISPATCH_CONSUMED), 1);
        assert_eq!(channel_count(&snapshot, CHANNEL_UX_CONTRACT_WARNING), 0);
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn workbench_host_action_emits_contract_warning_for_missing_frame_target() {
        let mut diagnostics = DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_dispatch_warning", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetFrameSplitOfferSuppressed {
                frame_name: "missing-frame".to_string(),
                suppressed: true,
            },
            &mut app,
            &mut tree,
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert_eq!(channel_count(&snapshot, CHANNEL_UX_DISPATCH_STARTED), 1);
        assert_eq!(channel_count(&snapshot, CHANNEL_UX_DISPATCH_CONSUMED), 0);
        assert_eq!(channel_count(&snapshot, CHANNEL_UX_CONTRACT_WARNING), 1);
    }

    #[test]
    fn surface_navigation_host_actions_enqueue_workbench_intents() {
        let source = GraphViewId::new();
        let destination = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(source);
        app.ensure_graph_view_registered(destination);
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(source)));
        let mut tree = Tree::new("workbench_host_surface_navigation_intents", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::FocusGraphView(source),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::OpenGraphView(destination),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::TransferSelectedNodesToGraphView {
                source_view: source,
                destination_view: destination,
            },
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::ToggleOverviewPlane,
            &mut app,
            &mut tree,
        );

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [
                WorkbenchIntent::FocusGraphView { view_id: focused },
                WorkbenchIntent::OpenGraphViewPane {
                    view_id: opened,
                    mode: PendingTileOpenMode::Tab
                },
                WorkbenchIntent::TransferSelectedNodesToGraphView {
                    source_view: transferred_from,
                    destination_view: transferred_to,
                },
                WorkbenchIntent::ToggleOverviewPlane,
            ] if *focused == source
                && *opened == destination
                && *transferred_from == source
                && *transferred_to == destination
        ));
    }

    #[test]
    fn layout_and_policy_host_actions_enqueue_workbench_intents() {
        let host = SurfaceHostId::Navigator(NavigatorHostId::Top);
        let policy = SurfaceFirstUsePolicy {
            surface_host: host.clone(),
            prompt_shown: true,
            outcome: None,
        };
        let constraint = WorkbenchLayoutConstraint::AnchoredSplit {
            surface_host: host.clone(),
            anchor_edge: AnchorEdge::Left,
            anchor_size_fraction: 0.22,
            cross_axis_margin_start_px: 20.0,
            cross_axis_margin_end_px: 14.0,
            resizable: true,
        };

        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_layout_policy_intents", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetWorkbenchPinned(true),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SetLayoutConstraintDraft {
                surface_host: host.clone(),
                constraint: constraint.clone(),
            },
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::CommitLayoutConstraintDraft(host.clone()),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::DiscardLayoutConstraintDraft(host.clone()),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SetNavigatorHostScope {
                surface_host: host.clone(),
                scope: NavigatorHostScope::GraphOnly,
            },
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SetFirstUsePolicy(policy.clone()),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SuppressFirstUsePromptForSession(host.clone()),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::DismissFrameSplitOfferForSession(
                "workspace-session-dismiss".to_string(),
            ),
            &mut app,
            &mut tree,
        );

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [
                WorkbenchIntent::SetWorkbenchPinned { pinned: true },
                WorkbenchIntent::SetLayoutConstraintDraft {
                    surface_host: drafted_host,
                    constraint: drafted_constraint,
                },
                WorkbenchIntent::CommitLayoutConstraintDraft {
                    surface_host: committed_host,
                },
                WorkbenchIntent::DiscardLayoutConstraintDraft {
                    surface_host: discarded_host,
                },
                WorkbenchIntent::SetNavigatorHostScope {
                    surface_host: scoped_host,
                    scope: NavigatorHostScope::GraphOnly,
                },
                WorkbenchIntent::SetFirstUsePolicy { policy: queued_policy },
                WorkbenchIntent::SuppressFirstUsePromptForSession {
                    surface_host: suppressed_host,
                },
                WorkbenchIntent::DismissFrameSplitOfferForSession { frame_name },
            ] if drafted_host == &host
                && drafted_constraint == &constraint
                && committed_host == &host
                && discarded_host == &host
                && scoped_host == &host
                && queued_policy == &policy
                && suppressed_host == &host
                && frame_name == "workspace-session-dismiss"
        ));
    }

    #[test]
    fn frame_request_host_actions_enqueue_workbench_intents() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_frame_request_intents", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::RenameFrame {
                from: "workspace-old".to_string(),
                to: "workspace-new".to_string(),
            },
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::DeleteFrame("workspace-delete".to_string()),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SaveFrameSnapshotNamed("workspace-save".to_string()),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SaveCurrentFrame,
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::PruneEmptyFrames,
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::RestoreFrame("workspace-restore".to_string()),
            &mut app,
            &mut tree,
        );

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [
                WorkbenchIntent::RenameFrame { from, to },
                WorkbenchIntent::DeleteFrame { frame_name },
                WorkbenchIntent::SaveFrameSnapshotNamed { name: saved_name },
                WorkbenchIntent::SaveCurrentFrame,
                WorkbenchIntent::PruneEmptyFrames,
                WorkbenchIntent::RestoreFrame { name },
            ] if from == "workspace-old"
                && to == "workspace-new"
                && frame_name == "workspace-delete"
                && saved_name == "workspace-save"
                && name == "workspace-restore"
        ));
    }

    #[test]
    fn frame_and_navigator_host_actions_enqueue_workbench_intents() {
        let host = SurfaceHostId::Navigator(NavigatorHostId::Right);
        let kind = Some(GraphletKind::Component);
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/frame-intent".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let mut tree = Tree::new("workbench_host_frame_intents", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-frame-intents", &tree);
        let frame_key =
            frame_key_for_name(&app, "workspace-frame-intents").expect("frame anchor should exist");

        apply_workbench_host_action_test(
            WorkbenchHostAction::OpenFrameAsSplit {
                node_key: node,
                frame_name: "workspace-frame-intents".to_string(),
            },
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SetFrameSplitOfferSuppressed {
                frame_name: "workspace-frame-intents".to_string(),
                suppressed: true,
            },
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::MoveFrameLayoutHint {
                frame_name: "workspace-frame-intents".to_string(),
                from_index: 2,
                to_index: 1,
            },
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::RemoveFrameLayoutHint {
                frame_name: "workspace-frame-intents".to_string(),
                hint_index: 3,
            },
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SetNavigatorSpecialtyView {
                host: host.clone(),
                kind,
            },
            &mut app,
            &mut tree,
        );

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [
                WorkbenchIntent::OpenFrameAsSplit {
                    node_key: opened_node,
                    frame_name,
                },
                WorkbenchIntent::SetFrameSplitOfferSuppressed {
                    frame: suppressed_frame,
                    suppressed: true,
                },
                WorkbenchIntent::MoveFrameLayoutHint {
                    frame: moved_frame,
                    from_index: 2,
                    to_index: 1,
                },
                WorkbenchIntent::RemoveFrameLayoutHint {
                    frame: removed_frame,
                    hint_index: 3,
                },
                WorkbenchIntent::SetNavigatorSpecialtyView {
                    host: queued_host,
                    kind: queued_kind,
                },
            ] if *opened_node == node
                && frame_name == "workspace-frame-intents"
                && *suppressed_frame == frame_key
                && *moved_frame == frame_key
                && *removed_frame == frame_key
                && queued_host == &host
                && *queued_kind == kind
        ));
    }

    #[test]
    fn frame_request_host_actions_apply_via_workbench_intents() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_frame_request_apply", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SaveCurrentFrame,
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::SaveFrameSnapshotNamed("workspace-explicit-save".to_string()),
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::PruneEmptyFrames,
            &mut app,
            &mut tree,
        );
        apply_workbench_host_action_test(
            WorkbenchHostAction::RestoreFrame("workspace-restore".to_string()),
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        assert!(
            app.take_pending_save_frame_snapshot_named()
                .is_some_and(|name| name.starts_with("workspace:workbench-host-"))
        );
        assert_eq!(
            app.take_pending_save_frame_snapshot_named(),
            Some("workspace-explicit-save".to_string())
        );
        assert!(app.take_pending_prune_empty_frames());
        assert_eq!(
            app.take_pending_restore_frame_snapshot_named(),
            Some("workspace-restore".to_string())
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn projection_becomes_visible_for_tool_or_node_panes() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let node_key = app.add_node_and_sync(
            "https://example.com".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let tool = tiles.insert_pane(TileKind::Tool(ToolPaneRef::new(ToolPaneState::Settings)));
        let root = tiles.insert_tab_tile(vec![graph, node, tool]);
        let tree = Tree::new("workbench_host_visible", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.layer_state, WorkbenchLayerState::WorkbenchActive);
        assert!(projection.visible());
        assert_eq!(projection.pane_entries.len(), 3);
    }

    #[test]
    fn projection_preserves_split_and_tab_hierarchy() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let left_node = app.add_node_and_sync(
            "https://example.com/left".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let right_node = app.add_node_and_sync(
            "https://example.com/right".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let left = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left_node)));
        let right = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right_node)));
        let left_tabs = tiles.insert_tab_tile(vec![graph, left]);
        let right_tabs = tiles.insert_tab_tile(vec![right]);
        let root = tiles.insert_horizontal_tile(vec![left_tabs, right_tabs]);
        let tree = Tree::new("workbench_host_hierarchy", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let root = projection.tree_root.as_ref().expect("hierarchy root");
        match root {
            WorkbenchChromeNode::Split { children, .. } => {
                assert_eq!(children.len(), 2);
                match &children[0] {
                    WorkbenchChromeNode::Tabs { children, .. } => {
                        assert_eq!(children.len(), 2);
                        assert!(matches!(children[0], WorkbenchChromeNode::Pane(_)));
                        assert!(matches!(children[1], WorkbenchChromeNode::Pane(_)));
                    }
                    other => panic!("expected left child tabs, got {other:?}"),
                }
                match &children[1] {
                    WorkbenchChromeNode::Tabs { children, .. } => {
                        assert_eq!(children.len(), 1);
                        assert!(matches!(children[0], WorkbenchChromeNode::Pane(_)));
                    }
                    other => panic!("expected right child tabs, got {other:?}"),
                }
            }
            other => panic!("expected split root, got {other:?}"),
        }
    }

    #[test]
    fn projection_marks_semantic_tab_affordance_for_pane_rest_members() {
        let mut app = GraphBrowserApp::new_for_testing();
        let left_node = app.add_node_and_sync(
            "https://example.com/semantic-left".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let right_node = app.add_node_and_sync(
            "https://example.com/semantic-right".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let left_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left_node)));
        let right_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right_node)));
        let root = tiles.insert_tab_tile(vec![left_tile, right_tile]);
        let mut tree = Tree::new("workbench_host_semantic_affordance", root, tiles);

        let semantics =
            crate::shell::desktop::ui::persistence_ops::derive_runtime_frame_tab_semantics_from_tree(
                &tree,
            )
            .expect("semantic tab metadata");
        let group = semantics.tab_groups[0].clone();
        app.set_current_frame_tab_semantics(Some(semantics));

        let pane_id = match tree.tiles.get(left_tile) {
            Some(Tile::Pane(tile)) => tile.pane_id(),
            other => panic!("expected pane tile, got {other:?}"),
        };
        assert!(tile_view_ops::collapse_semantic_tab_group_to_pane_rest(
            &mut tree,
            &mut app,
            group.group_id
        ));

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, Some(pane_id));
        let entry = projection
            .pane_entries
            .iter()
            .find(|entry| entry.pane_id == pane_id)
            .expect("collapsed pane should remain visible");

        assert_eq!(
            entry.semantic_tab_affordance,
            Some(semantic_tabs::SemanticTabAffordance::Restore {
                group_id: group.group_id,
                member_count: 2,
            })
        );
    }

    #[test]
    fn projection_exposes_active_frame_name_for_current_frame_handle() {
        let node_key = NodeKey::new(1);
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.workbench_session.current_workspace_name = Some("alpha".to_string());

        let mut tiles = Tiles::default();
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let root = tiles.insert_tab_tile(vec![node]);
        let tree = Tree::new("workbench_host_frame_projection", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        assert_eq!(projection.active_frame_name.as_deref(), Some("alpha"));
    }

    #[test]
    fn projection_labels_root_tab_group_as_frame_when_current_frame_is_open() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://frame-label.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        app.note_frame_activated("alpha", [node_key]);

        let mut tiles = Tiles::default();
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
        let root = tiles.insert_tab_tile(vec![node]);
        let tree = Tree::new("workbench_host_frame_label", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        match projection.tree_root.as_ref() {
            Some(WorkbenchChromeNode::Tabs { label, .. }) => {
                assert!(label.contains("Frame: alpha"));
            }
            other => panic!("expected tabs root, got {other:?}"),
        }
    }

    #[test]
    fn projection_includes_arrangement_navigator_groups() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let left_node = app.add_node_and_sync(
            "https://example.com/left".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let right_node = app.add_node_and_sync(
            "https://example.com/right".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let left = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left_node)));
        let right = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right_node)));
        let left_tabs = tiles.insert_tab_tile(vec![graph, left]);
        let right_tabs = tiles.insert_tab_tile(vec![right]);
        let root = tiles.insert_horizontal_tile(vec![left_tabs, right_tabs]);
        let tree = Tree::new("workbench_host_navigator_groups", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-alpha", &tree);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let arrangement_group = projection
            .navigator_groups
            .iter()
            .find(|group| {
                group.section == WorkbenchNavigatorSection::Workbench
                    && group.title.starts_with("Frame: ")
            })
            .expect("arrangement group");
        assert_eq!(arrangement_group.members.len(), 3);
        assert!(arrangement_group.title.contains("workspace-alpha"));
    }

    #[test]
    fn projection_highlights_current_frame_navigator_group() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let left_node = app.add_node_and_sync(
            "https://example.com/current-left".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let right_node = app.add_node_and_sync(
            "https://example.com/current-right".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let left = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left_node)));
        let right = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right_node)));
        let root = tiles.insert_tab_tile(vec![graph, left, right]);
        let tree = Tree::new("workbench_host_current_frame_group", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-alpha", &tree);
        app.note_frame_activated("workspace-alpha", [left_node, right_node]);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let arrangement_group = projection
            .navigator_groups
            .iter()
            .find(|group| group.title == "Frame: workspace-alpha")
            .expect("frame navigator group");
        assert!(arrangement_group.is_highlighted);
    }

    #[test]
    fn projection_highlights_selected_frame_navigator_group() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let left_node = app.add_node_and_sync(
            "https://example.com/selected-left".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let right_node = app.add_node_and_sync(
            "https://example.com/selected-right".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let left = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left_node)));
        let right = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right_node)));
        let root = tiles.insert_tab_tile(vec![graph, left, right]);
        let tree = Tree::new("workbench_host_selected_frame_group", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-alpha", &tree);
        app.apply_reducer_intents([GraphIntent::SetSelectedFrame {
            frame_name: Some("workspace-alpha".to_string()),
        }]);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let arrangement_group = projection
            .navigator_groups
            .iter()
            .find(|group| group.title == "Frame: workspace-alpha")
            .expect("frame navigator group");
        assert!(arrangement_group.is_highlighted);
    }

    #[test]
    fn frame_split_offer_candidate_detects_hinted_frame_for_selected_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/split-offer".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let pane = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![graph, pane]);
        let tree = Tree::new("workbench_host_split_offer", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-split", &tree);
        let frame_url = VersoAddress::frame("workspace-split").to_string();
        let (frame_key, _) = app
            .domain_graph()
            .get_node_by_url(&frame_url)
            .expect("frame anchor should exist");
        let node_id = app
            .domain_graph()
            .get_node(node)
            .expect("node should exist")
            .id
            .to_string();
        app.apply_reducer_intents([GraphIntent::RecordFrameLayoutHint {
            frame: frame_key,
            hint: crate::graph::FrameLayoutHint::SplitHalf {
                first: node_id.clone(),
                second: node_id,
                orientation: crate::graph::SplitOrientation::Vertical,
            },
        }]);
        app.select_node(node, false);

        let candidate = frame_split_offer_candidate(&app).expect("split offer candidate");
        assert_eq!(candidate.node_key, node);
        assert_eq!(candidate.frame_name, "workspace-split");
        assert_eq!(candidate.hint_count, 1);
    }

    #[test]
    fn frame_split_offer_candidate_skips_session_dismissed_frame() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/split-dismissed".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let tree = Tree::new("workbench_host_split_offer_dismissed", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-split-dismissed", &tree);
        let frame_url = VersoAddress::frame("workspace-split-dismissed").to_string();
        let (frame_key, _) = app
            .domain_graph()
            .get_node_by_url(&frame_url)
            .expect("frame anchor should exist");
        let node_id = app
            .domain_graph()
            .get_node(node)
            .expect("node should exist")
            .id
            .to_string();
        app.apply_reducer_intents([GraphIntent::RecordFrameLayoutHint {
            frame: frame_key,
            hint: crate::graph::FrameLayoutHint::SplitHalf {
                first: node_id.clone(),
                second: node_id,
                orientation: crate::graph::SplitOrientation::Vertical,
            },
        }]);
        app.select_node(node, false);
        app.dismiss_frame_split_offer_for_session("workspace-split-dismissed");

        assert!(frame_split_offer_candidate(&app).is_none());
    }

    #[test]
    fn frame_split_offer_candidate_skips_durably_suppressed_frame() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/split-suppressed".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let tree = Tree::new("workbench_host_split_offer_suppressed", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-split-suppressed", &tree);
        let frame_key = frame_key_for_name(&app, "workspace-split-suppressed")
            .expect("frame anchor should exist");
        let node_id = app
            .domain_graph()
            .get_node(node)
            .expect("node should exist")
            .id
            .to_string();
        app.apply_reducer_intents([
            GraphIntent::RecordFrameLayoutHint {
                frame: frame_key,
                hint: crate::graph::FrameLayoutHint::SplitHalf {
                    first: node_id.clone(),
                    second: node_id,
                    orientation: crate::graph::SplitOrientation::Vertical,
                },
            },
            GraphIntent::SetFrameSplitOfferSuppressed {
                frame: frame_key,
                suppressed: true,
            },
        ]);
        app.select_node(node, false);

        assert!(frame_split_offer_candidate(&app).is_none());
    }

    #[test]
    fn open_frame_as_split_action_routes_to_preferred_frame_restore() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/open-frame-as-split".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let node_id = app
            .domain_graph()
            .get_node(node)
            .expect("node should exist")
            .id;
        app.init_membership_index(HashMap::from([(
            node_id,
            BTreeSet::from(["workspace-alpha".to_string(), "workspace-beta".to_string()]),
        )]));

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let mut tree = Tree::new("workbench_host_open_frame_as_split", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::OpenFrameAsSplit {
                node_key: node,
                frame_name: "workspace-beta".to_string(),
            },
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        assert_eq!(
            app.take_pending_restore_workspace_snapshot_named(),
            Some("workspace-beta".to_string())
        );
        assert_eq!(
            app.take_pending_workspace_restore_open_request(),
            Some(crate::app::PendingNodeOpenRequest {
                key: node,
                mode: crate::app::PendingTileOpenMode::Tab,
            })
        );
    }

    #[test]
    fn set_frame_split_offer_suppressed_action_updates_frame_metadata() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/frame-action".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let mut tree = Tree::new("workbench_host_frame_action", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-action-toggle", &tree);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetFrameSplitOfferSuppressed {
                frame_name: "workspace-action-toggle".to_string(),
                suppressed: true,
            },
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);
        let frame_key =
            frame_key_for_name(&app, "workspace-action-toggle").expect("frame anchor should exist");
        assert_eq!(
            app.domain_graph().frame_split_offer_suppressed(frame_key),
            Some(true)
        );

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetFrameSplitOfferSuppressed {
                frame_name: "workspace-action-toggle".to_string(),
                suppressed: false,
            },
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);
        assert_eq!(
            app.domain_graph().frame_split_offer_suppressed(frame_key),
            Some(false)
        );
    }

    #[test]
    fn frame_split_offer_suppression_persists_across_restart() {
        let temp_dir = TempDir::new().expect("temp dir");
        let path = temp_dir.path().to_path_buf();

        let frame_key = {
            let mut app = GraphBrowserApp::new_from_dir(path.clone());
            let node = app.add_node_and_sync(
                "https://example.com/frame-suppression-restart".to_string(),
                euclid::default::Point2D::new(0.0, 0.0),
            );

            let mut tiles = Tiles::default();
            let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
            let root = tiles.insert_tab_tile(vec![node_tile]);
            let mut tree = Tree::new("workbench_host_frame_suppression_restart", root, tiles);

            app.sync_named_workbench_frame_graph_representation("workspace-restart-toggle", &tree);
            let frame_key = frame_key_for_name(&app, "workspace-restart-toggle")
                .expect("frame anchor should exist");

            apply_workbench_host_action_test(
                WorkbenchHostAction::SetFrameSplitOfferSuppressed {
                    frame_name: "workspace-restart-toggle".to_string(),
                    suppressed: true,
                },
                &mut app,
                &mut tree,
            );
            dispatch_pending_workbench_intents(&mut app, &mut tree);
            assert_eq!(
                app.domain_graph().frame_split_offer_suppressed(frame_key),
                Some(true)
            );
            frame_key
        };

        let reopened = GraphBrowserApp::new_from_dir(path);
        assert_eq!(
            reopened
                .domain_graph()
                .frame_split_offer_suppressed(frame_key),
            Some(true)
        );
    }

    #[test]
    fn dismiss_frame_split_offer_for_session_action_does_not_set_durable_suppression() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://example.com/frame-session-dismiss".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let mut tree = Tree::new("workbench_host_frame_session_dismiss", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-session-dismiss", &tree);
        let frame_key = frame_key_for_name(&app, "workspace-session-dismiss")
            .expect("frame anchor should exist");

        apply_workbench_host_action_test(
            WorkbenchHostAction::DismissFrameSplitOfferForSession(
                "workspace-session-dismiss".to_string(),
            ),
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        assert!(app.is_frame_split_offer_dismissed_for_session("workspace-session-dismiss"));
        assert_eq!(
            app.domain_graph().frame_split_offer_suppressed(frame_key),
            Some(false)
        );
    }

    #[test]
    fn frame_split_offer_session_dismiss_expires_across_restart() {
        let temp_dir = TempDir::new().expect("temp dir");
        let path = temp_dir.path().to_path_buf();

        {
            let mut app = GraphBrowserApp::new_from_dir(path.clone());
            let node = app.add_node_and_sync(
                "https://example.com/frame-session-expiry".to_string(),
                euclid::default::Point2D::new(0.0, 0.0),
            );

            let mut tiles = Tiles::default();
            let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
            let root = tiles.insert_tab_tile(vec![node_tile]);
            let tree = Tree::new("workbench_host_frame_session_expiry", root, tiles);

            app.sync_named_workbench_frame_graph_representation("workspace-session-expiry", &tree);
            let frame_key = frame_key_for_name(&app, "workspace-session-expiry")
                .expect("frame anchor should exist");
            let node_id = app
                .domain_graph()
                .get_node(node)
                .expect("node should exist")
                .id
                .to_string();
            app.apply_reducer_intents([GraphIntent::RecordFrameLayoutHint {
                frame: frame_key,
                hint: crate::graph::FrameLayoutHint::SplitHalf {
                    first: node_id.clone(),
                    second: node_id,
                    orientation: crate::graph::SplitOrientation::Vertical,
                },
            }]);
            app.select_node(node, false);
            app.dismiss_frame_split_offer_for_session("workspace-session-expiry");

            assert!(frame_split_offer_candidate(&app).is_none());
        }

        let mut reopened = GraphBrowserApp::new_from_dir(path);
        let reopened_node = reopened
            .domain_graph()
            .nodes()
            .find_map(|(key, node)| {
                (node.url() == "https://example.com/frame-session-expiry").then_some(key)
            })
            .expect("node should reopen");
        reopened.select_node(reopened_node, false);

        let candidate = frame_split_offer_candidate(&reopened)
            .expect("split offer should reappear in new session");
        assert_eq!(candidate.frame_name, "workspace-session-expiry");
    }

    #[test]
    fn move_frame_layout_hint_action_reorders_frame_metadata() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync(
            "https://example.com/frame-a".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://example.com/frame-b".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );
        let c = app.add_node_and_sync(
            "https://example.com/frame-c".to_string(),
            euclid::default::Point2D::new(200.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let left = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(a)));
        let middle = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(b)));
        let right = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(c)));
        let root = tiles.insert_tab_tile(vec![left, middle, right]);
        let mut tree = Tree::new("workbench_host_frame_move_hint", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-move-hints", &tree);
        let frame_key =
            frame_key_for_name(&app, "workspace-move-hints").expect("frame anchor should exist");
        let a_id = app.domain_graph().get_node(a).unwrap().id.to_string();
        let b_id = app.domain_graph().get_node(b).unwrap().id.to_string();
        let c_id = app.domain_graph().get_node(c).unwrap().id.to_string();
        app.apply_reducer_intents([
            GraphIntent::RecordFrameLayoutHint {
                frame: frame_key,
                hint: crate::graph::FrameLayoutHint::SplitHalf {
                    first: a_id.clone(),
                    second: b_id.clone(),
                    orientation: crate::graph::SplitOrientation::Vertical,
                },
            },
            GraphIntent::RecordFrameLayoutHint {
                frame: frame_key,
                hint: crate::graph::FrameLayoutHint::SplitHalf {
                    first: b_id.clone(),
                    second: c_id.clone(),
                    orientation: crate::graph::SplitOrientation::Horizontal,
                },
            },
        ]);

        apply_workbench_host_action_test(
            WorkbenchHostAction::MoveFrameLayoutHint {
                frame_name: "workspace-move-hints".to_string(),
                from_index: 1,
                to_index: 0,
            },
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let hints = app.domain_graph().frame_layout_hints(frame_key).unwrap();
        assert_eq!(
            hints[0],
            crate::graph::FrameLayoutHint::SplitHalf {
                first: b_id.clone(),
                second: c_id,
                orientation: crate::graph::SplitOrientation::Horizontal,
            }
        );
        assert_eq!(
            hints[1],
            crate::graph::FrameLayoutHint::SplitHalf {
                first: a_id,
                second: b_id.clone(),
                orientation: crate::graph::SplitOrientation::Vertical,
            }
        );
    }

    #[test]
    fn remove_frame_layout_hint_action_updates_frame_metadata() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync(
            "https://example.com/frame-remove-a".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://example.com/frame-remove-b".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let left = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(a)));
        let right = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(b)));
        let root = tiles.insert_tab_tile(vec![left, right]);
        let mut tree = Tree::new("workbench_host_frame_remove_hint", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-remove-hint", &tree);
        let frame_key =
            frame_key_for_name(&app, "workspace-remove-hint").expect("frame anchor should exist");
        let a_id = app.domain_graph().get_node(a).unwrap().id.to_string();
        let b_id = app.domain_graph().get_node(b).unwrap().id.to_string();
        app.apply_reducer_intents([GraphIntent::RecordFrameLayoutHint {
            frame: frame_key,
            hint: crate::graph::FrameLayoutHint::SplitHalf {
                first: a_id,
                second: b_id,
                orientation: crate::graph::SplitOrientation::Vertical,
            },
        }]);

        apply_workbench_host_action_test(
            WorkbenchHostAction::RemoveFrameLayoutHint {
                frame_name: "workspace-remove-hint".to_string(),
                hint_index: 0,
            },
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        assert!(
            app.domain_graph()
                .frame_layout_hints(frame_key)
                .is_some_and(|hints| hints.is_empty())
        );
    }

    #[test]
    fn set_navigator_specialty_view_action_updates_specialty_state_via_workbench_intent() {
        let host = SurfaceHostId::Navigator(NavigatorHostId::Right);
        let mut app = GraphBrowserApp::new_for_testing();
        let selected = app.add_node_and_sync(
            "https://example.com/navigator-specialty".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        app.select_node(selected, false);

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_navigator_specialty", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetNavigatorSpecialtyView {
                host: host.clone(),
                kind: Some(GraphletKind::Component),
            },
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        assert_eq!(
            app.workspace
                .workbench_session
                .navigator_specialty_views
                .get(&host)
                .map(|view| view.kind),
            Some(GraphletKind::Component)
        );
    }

    #[test]
    fn select_node_in_graph_view_action_focuses_target_view_before_selection() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source_view = GraphViewId::new();
        let target_view = GraphViewId::new();
        app.ensure_graph_view_registered(source_view);
        app.ensure_graph_view_registered(target_view);
        app.workspace.graph_runtime.focused_view = Some(source_view);

        let node = app.add_node_and_sync(
            "https://example.com/graphlet-anchor-select".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(target_view)));
        let mut tree = Tree::new("workbench_host_select_node_in_graph_view", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SelectNodeInGraphView {
                view_id: target_view,
                node_key: node,
            },
            &mut app,
            &mut tree,
        );

        assert_eq!(app.workspace.graph_runtime.focused_view, Some(target_view));
        assert_eq!(app.focused_selection().primary(), Some(node));
    }

    #[test]
    fn open_navigator_specialty_from_node_prepares_anchor_before_dispatch() {
        let host = SurfaceHostId::Navigator(NavigatorHostId::Right);
        let mut app = GraphBrowserApp::new_for_testing();
        let source_view = GraphViewId::new();
        let target_view = GraphViewId::new();
        app.ensure_graph_view_registered(source_view);
        app.ensure_graph_view_registered(target_view);
        app.workspace.graph_runtime.focused_view = Some(source_view);

        let node = app.add_node_and_sync(
            "https://example.com/graphlet-specialty-anchor".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(target_view)));
        let mut tree = Tree::new("workbench_host_open_graphlet_specialty", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::OpenNavigatorSpecialtyFromNode {
                host: host.clone(),
                view_id: target_view,
                node_key: node,
                kind: GraphletKind::Component,
            },
            &mut app,
            &mut tree,
        );

        assert_eq!(app.workspace.graph_runtime.focused_view, Some(target_view));
        assert_eq!(app.focused_selection().primary(), Some(node));
        let pending_intents = app.take_pending_workbench_intents();
        assert!(matches!(
            pending_intents.as_slice(),
            [WorkbenchIntent::SetNavigatorSpecialtyView {
                host: queued_host,
                kind: Some(GraphletKind::Component),
            }] if queued_host == &host
        ));
    }

    #[test]
    fn navigator_specialty_corridor_uses_selected_pair_and_tree_layout() {
        // Node URLs deliberately use three distinct hosts. Nodes sharing
        // a host get automatic `ContainmentRelation::Domain` derived
        // edges toward a per-host anchor chosen by (depth, UUID), and
        // the UUID tiebreak is non-deterministic. That derivation adds
        // a direct edge between the corridor's two endpoints, yielding
        // a 1-hop shortest path that bypasses `middle` and makes this
        // test flake ~60 % of the time. Distinct hosts sidestep the
        // derivation entirely.
        let host = SurfaceHostId::Navigator(NavigatorHostId::Right);
        let mut app = GraphBrowserApp::new_for_testing();
        let left = app.add_node_and_sync(
            "https://left.test/graphlet-corridor".to_string(),
            euclid::default::Point2D::new(-20.0, 0.0),
        );
        let middle = app.add_node_and_sync(
            "https://middle.test/graphlet-corridor".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let right = app.add_node_and_sync(
            "https://right.test/graphlet-corridor".to_string(),
            euclid::default::Point2D::new(20.0, 0.0),
        );
        app.add_edge_and_sync(left, middle, crate::graph::EdgeType::Hyperlink, None);
        app.add_edge_and_sync(middle, right, crate::graph::EdgeType::Hyperlink, None);
        app.select_node(left, false);
        app.select_node(right, true);

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_navigator_specialty_corridor", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetNavigatorSpecialtyView {
                host: host.clone(),
                kind: Some(GraphletKind::Corridor),
            },
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let specialty = app
            .workspace
            .workbench_session
            .navigator_specialty_views
            .get(&host)
            .cloned()
            .expect("specialty view should be active");
        assert_eq!(specialty.kind, GraphletKind::Corridor);

        let view = app
            .workspace
            .graph_runtime
            .views
            .get(&specialty.view_id)
            .expect("specialty view state should exist");
        let mask = view
            .graphlet_node_mask
            .as_ref()
            .expect("corridor specialty should publish a mask");
        assert!(mask.contains(&left));
        assert!(mask.contains(&middle));
        assert!(mask.contains(&right));
        assert!(matches!(
            view.layout_policy.mode,
            crate::registries::atomic::lens::LayoutMode::Tree { .. }
        ));
        assert_eq!(
            view.resolved_layout_algorithm_id(),
            crate::app::graph_layout::GRAPH_LAYOUT_TREE
        );
    }

    #[test]
    fn clearing_navigator_specialty_view_removes_mask_and_registration() {
        let host = SurfaceHostId::Navigator(NavigatorHostId::Right);
        let mut app = GraphBrowserApp::new_for_testing();
        let selected = app.add_node_and_sync(
            "https://example.com/navigator-specialty-clear".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        app.select_node(selected, false);

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let mut tree = Tree::new("workbench_host_navigator_specialty_clear", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetNavigatorSpecialtyView {
                host: host.clone(),
                kind: Some(GraphletKind::Component),
            },
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);

        let specialty_view_id = app
            .workspace
            .workbench_session
            .navigator_specialty_views
            .get(&host)
            .map(|view| view.view_id)
            .expect("specialty view should be active before clearing");

        apply_workbench_host_action_test(
            WorkbenchHostAction::SetNavigatorSpecialtyView {
                host: host.clone(),
                kind: None,
            },
            &mut app,
            &mut tree,
        );
        dispatch_pending_workbench_intents(&mut app, &mut tree);

        assert!(
            app.workspace
                .workbench_session
                .navigator_specialty_views
                .get(&host)
                .is_none(),
            "specialty registration should clear"
        );
        assert!(
            app.workspace
                .graph_runtime
                .views
                .get(&specialty_view_id)
                .is_some_and(|view| view.graphlet_node_mask.is_none()),
            "specialty mask should clear from the derived view"
        );
    }

    #[test]
    fn rename_frame_action_updates_persisted_layout_and_selected_frame() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(temp_dir.path().to_path_buf());
        let node = app.add_node_and_sync(
            "https://example.com/frame-rename".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let mut tree = Tree::new("workbench_host_frame_rename", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-rename-old", &tree);
        crate::shell::desktop::ui::persistence_ops::save_named_frame_bundle(
            &mut app,
            "workspace-rename-old",
            &tree,
        )
        .expect("save frame bundle");
        app.workspace.graph_runtime.selected_frame_name = Some("workspace-rename-old".to_string());

        apply_workbench_host_action_test(
            WorkbenchHostAction::RenameFrame {
                from: "workspace-rename-old".to_string(),
                to: "workspace-rename-new".to_string(),
            },
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        assert!(
            app.load_workspace_layout_json("workspace-rename-old")
                .is_none()
        );
        assert!(
            app.load_workspace_layout_json("workspace-rename-new")
                .is_some()
        );
        assert_eq!(app.selected_frame_name(), Some("workspace-rename-new"));
        assert_eq!(
            app.pending_frame_context_target(),
            Some("workspace-rename-new")
        );
    }

    #[test]
    fn delete_frame_action_clears_selected_and_current_frame_state() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(temp_dir.path().to_path_buf());
        let node = app.add_node_and_sync(
            "https://example.com/frame-delete".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let mut tree = Tree::new("workbench_host_frame_delete", root, tiles);

        app.sync_named_workbench_frame_graph_representation("workspace-delete-me", &tree);
        crate::shell::desktop::ui::persistence_ops::save_named_frame_bundle(
            &mut app,
            "workspace-delete-me",
            &tree,
        )
        .expect("save frame bundle");
        app.workspace.workbench_session.current_workspace_name =
            Some("workspace-delete-me".to_string());
        app.workspace.graph_runtime.selected_frame_name = Some("workspace-delete-me".to_string());
        app.set_pending_frame_context_target(Some("workspace-delete-me".to_string()));

        apply_workbench_host_action_test(
            WorkbenchHostAction::DeleteFrame("workspace-delete-me".to_string()),
            &mut app,
            &mut tree,
        );

        dispatch_pending_workbench_intents(&mut app, &mut tree);

        assert!(
            app.load_workspace_layout_json("workspace-delete-me")
                .is_none()
        );
        assert_eq!(app.current_workspace_name(), None);
        assert_eq!(app.selected_frame_name(), None);
        assert_eq!(app.pending_frame_context_target(), None);
    }

    #[test]
    fn projection_adds_unrelated_group_for_nodes_without_arrangement_membership() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        // Use a scheme that has no host and no folder group so the node
        // lands in the Unrelated bucket instead of Domain or Folders.
        let unrelated_key = app.add_node_and_sync(
            "about:blank".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(unrelated_key)));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        let tree = Tree::new("workbench_host_unrelated", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let unrelated_group = projection
            .navigator_groups
            .iter()
            .find(|group| group.section == WorkbenchNavigatorSection::Unrelated)
            .expect("unrelated group");
        assert_eq!(unrelated_group.members.len(), 1);
        assert_eq!(unrelated_group.members[0].node_key, unrelated_key);
    }

    #[test]
    fn projection_adds_imported_groups_labeled_by_source() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let imported_key = app.add_node_and_sync(
            "https://example.com/imported".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        assert!(app.workspace.domain.graph.set_node_import_provenance(
            imported_key,
            vec![crate::graph::NodeImportProvenance {
                source_id: "import:firefox-bookmarks".to_string(),
                source_label: "Firefox bookmarks".to_string(),
            }],
        ));

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let root = tiles.insert_tab_tile(vec![graph]);
        let tree = Tree::new("workbench_host_imported", root, tiles);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let imported_group = projection
            .navigator_groups
            .iter()
            .find(|group| group.section == WorkbenchNavigatorSection::Imported)
            .expect("imported group");
        assert_eq!(imported_group.title, "Firefox bookmarks");
        assert_eq!(imported_group.members.len(), 1);
        assert_eq!(imported_group.members[0].node_key, imported_key);
    }

    #[test]
    fn selecting_offscreen_node_requests_fit_selection_for_visible_graph() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let node_key = app.add_node_and_sync(
            "https://example.com/offscreen".to_string(),
            euclid::default::Point2D::new(400.0, 400.0),
        );
        app.workspace.graph_runtime.graph_view_canvas_rects.insert(
            graph_view,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
        );

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let mut tree = Tree::new("workbench_host_select_offscreen", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SelectNode {
                node_key,
                row_key: Some("node:test".to_string()),
            },
            &mut app,
            &mut tree,
        );

        assert!(app.focused_selection().contains(&node_key));
        assert_eq!(
            app.pending_camera_command(),
            Some(CameraCommand::FitSelection)
        );
        assert_eq!(app.pending_camera_command_target(), Some(graph_view));
    }

    #[test]
    fn selecting_onscreen_node_keeps_highlight_in_place() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let node_key = app.add_node_and_sync(
            "https://example.com/onscreen".to_string(),
            euclid::default::Point2D::new(40.0, 40.0),
        );
        app.workspace.graph_runtime.graph_view_canvas_rects.insert(
            graph_view,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
        );

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let mut tree = Tree::new("workbench_host_select_onscreen", root, tiles);

        apply_workbench_host_action_test(
            WorkbenchHostAction::SelectNode {
                node_key,
                row_key: Some("node:test".to_string()),
            },
            &mut app,
            &mut tree,
        );

        assert!(app.focused_selection().contains(&node_key));
        assert!(app.pending_camera_command().is_none());
        assert!(app.pending_camera_command_target().is_none());
    }

    #[test]
    fn activating_workbench_resident_node_focuses_its_pane() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let live_node = app.add_node_and_sync(
            "https://example.com/live".to_string(),
            euclid::default::Point2D::new(40.0, 40.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(live_node)));
        let root = tiles.insert_tab_tile(vec![graph, node]);
        let mut tree = Tree::new("workbench_host_activate_live", root, tiles);
        let _ = tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Graph(_))));

        apply_workbench_host_action_test(
            WorkbenchHostAction::ActivateNode {
                node_key: live_node,
                row_key: Some("node:live".to_string()),
            },
            &mut app,
            &mut tree,
        );

        assert!(app.focused_selection().contains(&live_node));
        assert_eq!(
            tree.active_tiles()
                .into_iter()
                .filter_map(|tile_id| tree.tiles.get(tile_id))
                .find_map(|tile| match tile {
                    Tile::Pane(TileKind::Node(state)) => Some(state.node),
                    _ => None,
                }),
            Some(live_node)
        );
        assert!(app.pending_camera_command().is_none());
        assert!(app.pending_open_node_request().is_none());
    }

    #[test]
    fn activating_prewarmed_cold_node_focuses_graph_instead_of_workbench() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let cold_node = app.add_node_and_sync(
            "https://example.com/cold".to_string(),
            euclid::default::Point2D::new(400.0, 400.0),
        );
        app.workspace.graph_runtime.graph_view_canvas_rects.insert(
            graph_view,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let other_node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(
            app.add_node_and_sync(
                "https://example.com/other".to_string(),
                euclid::default::Point2D::new(0.0, 0.0),
            ),
        )));
        let root = tiles.insert_tab_tile(vec![graph, other_node]);
        let mut tree = Tree::new("workbench_host_activate_cold", root, tiles);
        let _ = tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Node(_))));

        apply_workbench_host_action_test(
            WorkbenchHostAction::SelectNode {
                node_key: cold_node,
                row_key: Some("node:cold".to_string()),
            },
            &mut app,
            &mut tree,
        );
        assert!(
            app.domain_graph()
                .get_node(cold_node)
                .is_some_and(|node| node.lifecycle != crate::graph::NodeLifecycle::Cold)
        );

        app.clear_pending_camera_command();

        apply_workbench_host_action_test(
            WorkbenchHostAction::ActivateNode {
                node_key: cold_node,
                row_key: Some("node:cold".to_string()),
            },
            &mut app,
            &mut tree,
        );

        assert!(app.focused_selection().contains(&cold_node));
        assert_eq!(
            tree.active_tiles()
                .into_iter()
                .filter_map(|tile_id| tree.tiles.get(tile_id))
                .find_map(|tile| match tile {
                    Tile::Pane(TileKind::Graph(graph_ref)) => Some(graph_ref.graph_view_id),
                    _ => None,
                }),
            Some(graph_view)
        );
        assert_eq!(
            app.pending_camera_command(),
            Some(CameraCommand::FitSelection)
        );
        assert_eq!(app.pending_camera_command_target(), Some(graph_view));
        assert!(app.pending_open_node_request().is_none());
    }

    #[test]
    fn recent_navigator_members_count_visits_and_skip_arranged_nodes() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        let recent_key = app.add_node_and_sync(
            "https://example.com/recent".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let arranged_key = app.add_node_and_sync(
            "https://example.com/arranged".to_string(),
            euclid::default::Point2D::new(1.0, 0.0),
        );

        let arrangement_memberships =
            HashMap::from([(arranged_key, vec!["Frame: alpha".to_string()])]);
        assert!(app.apply_node_history_change(
            recent_key,
            vec![
                "https://example.com/source".to_string(),
                "https://example.com/recent".to_string(),
                "https://example.com/recent".to_string(),
            ],
            2,
        ));
        assert!(app.apply_node_history_change(
            arranged_key,
            vec![
                "https://example.com/source".to_string(),
                "https://example.com/arranged".to_string(),
            ],
            1,
        ));
        app.rebuild_semantic_navigation_runtime();

        let members = recent_navigator_members(&app, &arrangement_memberships, &HashSet::new());

        assert_eq!(members.len(), 2);
        assert_eq!(members[0].node_key, recent_key);
        assert!(members[0].title.contains("3 visits"));
        assert_eq!(members[1].node_key, recent_key);
        assert_eq!(
            members[1].target_url.as_deref(),
            Some("https://example.com/recent")
        );
        assert!(members[1].title.contains("Now:"));
    }

    #[test]
    fn recent_navigator_members_include_semantic_alternate_targets() {
        let mut app = GraphBrowserApp::new_for_testing();
        let recent_key = app.add_node_and_sync(
            "https://example.com/a".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        assert!(app.apply_node_history_change(
            recent_key,
            vec![
                "https://example.com/a".to_string(),
                "https://example.com/b".to_string(),
            ],
            1,
        ));
        assert!(app.apply_node_history_change(
            recent_key,
            vec![
                "https://example.com/a".to_string(),
                "https://example.com/b".to_string(),
            ],
            0,
        ));
        assert!(app.apply_node_history_change(
            recent_key,
            vec![
                "https://example.com/a".to_string(),
                "https://example.com/c".to_string(),
            ],
            1,
        ));
        app.rebuild_semantic_navigation_runtime();

        let members = recent_navigator_members(&app, &HashMap::new(), &HashSet::new());

        assert_eq!(members.len(), 3);
        assert_eq!(members[0].node_key, recent_key);
        assert!(members[0].title.contains("1 fork"));
        assert_eq!(
            members[1].target_url.as_deref(),
            Some("https://example.com/c")
        );
        assert!(members[1].title.contains("Now:"));
        assert_eq!(
            members[2].target_url.as_deref(),
            Some("https://example.com/b")
        );
        assert!(members[2].title.contains("Alt:"));
    }

    /// `build_active_graphlet_roster` includes cold durable peers with `is_cold = true`
    /// alongside the warm seed node (Phase 3, §12).
    #[test]
    fn active_graphlet_roster_marks_cold_peers_as_cold() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);

        let warm_node = app.add_node_and_sync(
            "https://example.com/warm".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let cold_node = app.add_node_and_sync(
            "https://example.com/cold".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        // Create a durable edge — cold_node stays Cold (no tile opened).
        app.apply_reducer_intents([crate::app::GraphIntent::CreateUserGroupedEdge {
            from: warm_node,
            to: cold_node,
            label: None,
        }]);

        // Promote warm_node to Active (selecting it advances its lifecycle from Cold).
        app.apply_reducer_intents([crate::app::GraphIntent::SelectNode {
            key: warm_node,
            multi_select: false,
        }]);

        // Build a tree with a tile for warm_node only.
        let mut tiles = Tiles::default();
        let warm_pane_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(warm_node)));
        let root = tiles.insert_tab_tile(vec![warm_pane_tile]);
        let tree = Tree::new("roster_cold_peer", root, tiles);

        // Determine the pane id for the warm node tile.
        let active_pane = tree.tiles.iter().find_map(|(_, tile)| match tile {
            egui_tiles::Tile::Pane(TileKind::Node(state)) if state.node == warm_node => {
                Some(state.pane_id)
            }
            _ => None,
        });

        let roster = build_active_graphlet_roster(&app, &tree, active_pane, None);

        assert_eq!(
            roster.len(),
            2,
            "roster should include warm_node and cold_node"
        );

        let cold_entry = roster
            .iter()
            .find(|e| e.node_key == cold_node)
            .expect("cold_node must appear in roster");
        assert!(
            cold_entry.is_cold,
            "cold_node entry must have is_cold = true"
        );

        let warm_entry = roster
            .iter()
            .find(|e| e.node_key == warm_node)
            .expect("warm_node must appear in roster");
        assert!(
            !warm_entry.is_cold,
            "warm_node entry must have is_cold = false"
        );
    }

    #[test]
    fn active_graphlet_roster_uses_view_projection_override() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);
        app.set_workspace_focused_view_with_transition(Some(graph_view));

        let warm_node = app.add_node_and_sync(
            "https://example.com/warm-view".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let cold_node = app.add_node_and_sync(
            "https://example.com/cold-view".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );
        let _ = app.assert_relation_and_sync(
            warm_node,
            cold_node,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );
        app.apply_reducer_intents([crate::app::GraphIntent::SetViewEdgeProjectionOverride {
            view_id: graph_view,
            selectors: Some(vec![crate::graph::RelationSelector::Semantic(
                crate::graph::SemanticSubKind::Hyperlink,
            )]),
        }]);
        app.apply_reducer_intents([crate::app::GraphIntent::SelectNode {
            key: warm_node,
            multi_select: false,
        }]);

        let mut tiles = Tiles::default();
        let warm_pane_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(warm_node)));
        let root = tiles.insert_tab_tile(vec![warm_pane_tile]);
        let tree = Tree::new("roster_view_override", root, tiles);

        let active_pane = tree.tiles.iter().find_map(|(_, tile)| match tile {
            egui_tiles::Tile::Pane(TileKind::Node(state)) if state.node == warm_node => {
                Some(state.pane_id)
            }
            _ => None,
        });

        let roster = build_active_graphlet_roster(&app, &tree, active_pane, Some(graph_view));
        assert!(roster.iter().any(|entry| entry.node_key == cold_node));
    }

    /// After a DismissTile, the cold node's `WorkbenchNavigatorMember` inside an
    /// arrangement group carries `is_cold = true` (Phase 4, §12).
    #[test]
    fn arrangement_navigator_member_marks_dismissed_cold_peer_as_cold() {
        let graph_view = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(graph_view);

        let warm_node = app.add_node_and_sync(
            "https://example.com/warm2".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let peer_node = app.add_node_and_sync(
            "https://example.com/peer".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        // Open both nodes into a tile tree so sync_named_workbench_frame sets up
        // ArrangementRelation edges for the arrangement navigator group.
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let warm_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(warm_node)));
        let peer_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(peer_node)));
        let root = tiles.insert_tab_tile(vec![graph, warm_tile, peer_tile]);
        let tree = Tree::new("nav_cold_peer", root, tiles);
        app.sync_named_workbench_frame_graph_representation("alpha-frame", &tree);

        // Promote warm_node to Active; peer_node will be explicitly demoted below.
        app.apply_reducer_intents([crate::app::GraphIntent::SelectNode {
            key: warm_node,
            multi_select: false,
        }]);

        // Demote peer_node to Cold (simulates DismissTile lifecycle change).
        app.demote_node_to_cold(peer_node);

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);

        let group = projection
            .navigator_groups
            .iter()
            .find(|g| {
                g.section == WorkbenchNavigatorSection::Workbench && g.title.contains("alpha-frame")
            })
            .expect("arrangement navigator group for alpha-frame");

        let cold_member = group
            .members
            .iter()
            .find(|m| m.node_key == peer_node)
            .expect("peer_node must appear in arrangement group");
        assert!(
            cold_member.is_cold,
            "dismissed peer_node must have is_cold = true in navigator group"
        );

        let warm_member = group
            .members
            .iter()
            .find(|m| m.node_key == warm_node)
            .expect("warm_node must appear in arrangement group");
        assert!(
            !warm_member.is_cold,
            "warm_node must have is_cold = false in navigator group"
        );
    }
}
