/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! ActionRegistry: unified action catalogue for command surfaces.
//!
//! Both the command palette and the radial palette draw their content from
//! [`list_actions_for_context`] rather than from hardcoded enums.  Each
//! returned [`ActionEntry`] carries enough metadata for any surface to
//! render the action and decide whether it is currently enabled.

use crate::app::{GraphViewId, SurfaceHostId};
use crate::graph::NodeKey;
use crate::shell::desktop::runtime::registries::input::action_id as input_action;
use std::sync::Once;

static ACTION_KEY_AUDIT_ONCE: Once = Once::new();

/// Preferred input mode, used as a layout hint by control surfaces.
///
/// `InputMode` is not a gate — both surfaces work in both modes — but
/// surfaces may use it to choose their default presentation (e.g. radial
/// menu as the primary surface in Gamepad mode).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InputMode {
    #[default]
    MouseKeyboard,
    Gamepad,
}

/// Logical grouping of actions, used for separators and ordering in the
/// command palette and as sector grouping in the radial palette.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionCategory {
    Node,
    Edge,
    Graph,
    Persistence,
}

pub const CATEGORY_RECENCY_PERSIST_KEY: &str = "command_palette_category_recency";
pub const CATEGORY_PIN_ORDER_PERSIST_KEY: &str = "command_palette_category_pins";

impl ActionCategory {
    /// Display label for the category group heading.
    pub fn label(self) -> &'static str {
        match self {
            Self::Node => "Node",
            Self::Edge => "Edge",
            Self::Graph => "Graph",
            Self::Persistence => "Persistence",
        }
    }
}

pub fn default_category_order() -> [ActionCategory; 4] {
    [
        ActionCategory::Node,
        ActionCategory::Edge,
        ActionCategory::Graph,
        ActionCategory::Persistence,
    ]
}

pub fn category_persisted_name(category: ActionCategory) -> &'static str {
    match category {
        ActionCategory::Node => "node",
        ActionCategory::Edge => "edge",
        ActionCategory::Graph => "graph",
        ActionCategory::Persistence => "persistence",
    }
}

pub fn category_from_persisted_name(name: &str) -> Option<ActionCategory> {
    match name {
        "node" => Some(ActionCategory::Node),
        "edge" => Some(ActionCategory::Edge),
        "graph" => Some(ActionCategory::Graph),
        "persistence" => Some(ActionCategory::Persistence),
        _ => None,
    }
}

fn base_category_rank(category: ActionCategory) -> usize {
    match category {
        ActionCategory::Node => 0,
        ActionCategory::Edge => 1,
        ActionCategory::Graph => 2,
        ActionCategory::Persistence => 3,
    }
}

fn category_context_score(category: ActionCategory, action_context: &ActionContext) -> i32 {
    match category {
        ActionCategory::Node => {
            let mut score = 100;
            if action_context.target_node.is_some() {
                score += 300;
            }
            if action_context.any_selected {
                score += 60;
            }
            if action_context.focused_pane_available {
                score += 25;
            }
            score
        }
        ActionCategory::Edge => {
            let mut score = 60;
            if action_context.pair_context.is_some() {
                score += 320;
            }
            score
        }
        ActionCategory::Graph => 80,
        ActionCategory::Persistence => 70,
    }
}

fn category_recency_score(category: ActionCategory, recency: &[ActionCategory]) -> i32 {
    recency
        .iter()
        .position(|entry| *entry == category)
        .map(|idx| 120_i32.saturating_sub((idx as i32) * 20))
        .unwrap_or(0)
}

pub fn rank_categories_for_context(
    categories: &[ActionCategory],
    action_context: &ActionContext,
    recency: &[ActionCategory],
    pinned: &[ActionCategory],
) -> Vec<ActionCategory> {
    let mut ordered = Vec::new();

    for category in pinned {
        if categories.contains(category) && !ordered.contains(category) {
            ordered.push(*category);
        }
    }

    let mut dynamic: Vec<ActionCategory> = categories
        .iter()
        .copied()
        .filter(|category| !ordered.contains(category))
        .collect();
    dynamic.sort_by_key(|category| {
        let context = category_context_score(*category, action_context);
        let recent = category_recency_score(*category, recency);
        let base = base_category_rank(*category) as i32;
        (-(context + recent), base)
    });

    ordered.extend(dynamic);
    ordered
}

/// Stable identifier for a registered action.
///
/// Each variant corresponds to one logical operation.  The action content
/// (label, category, enabled state) is resolved at runtime via
/// [`list_actions_for_context`] so that control surfaces remain free of
/// hardcoded dispatch tables.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionId {
    // Node actions
    NodeNew,
    NodeNewAsTab,
    NodePinToggle,
    NodePinSelected,
    NodeUnpinSelected,
    NodeDelete,
    NodeChooseFrame,
    NodeAddToFrame,
    NodeAddConnectedToFrame,
    NodeOpenFrame,
    NodeOpenNeighbors,
    NodeOpenConnected,
    NodeOpenSplit,
    NodeDetachToSplit,
    NodeMoveToActivePane,
    NodeWarmSelect,
    NodeRemoveFromGraphlet,
    /// Soft-delete selected nodes → Ghost Node (Tombstone lifecycle).
    NodeMarkTombstone,
    NodeCopyUrl,
    NodeCopyTitle,
    NodeRenderAuto,
    NodeRenderWebView,
    NodeRenderWry,
    // Edge actions
    EdgeConnectPair,
    EdgeConnectBoth,
    EdgeRemoveUser,
    // Graph actions
    GraphFit,
    GraphFitGraphlet,
    GraphCycleFocusRegion,
    GraphTogglePhysics,
    /// Toggle per-view ghost node (tombstone) visibility.
    GraphToggleGhostNodes,
    GraphPhysicsConfig,
    GraphCommandPalette,
    GraphRadialMenu,
    WorkbenchUnlockSurfaceLayout,
    WorkbenchLockSurfaceLayout,
    WorkbenchRememberLayoutPreference,
    WorkbenchGroupSelectedTiles,
    // Persistence actions
    PersistUndo,
    PersistRedo,
    PersistSaveSnapshot,
    PersistRestoreSession,
    PersistSaveGraph,
    PersistRestoreLatestGraph,
    PersistOpenHub,
    WorkbenchOpenSettingsPane,
    WorkbenchOpenSettingsOverlay,
    PersistOpenHistoryManager,
    WorkbenchActivateWorkflowDefault,
    WorkbenchActivateWorkflowResearch,
    WorkbenchActivateWorkflowReading,
}

impl ActionId {
    pub fn shortcut_hints(self) -> Vec<String> {
        let action_ids: &[&str] = match self {
            Self::NodeNew => &[input_action::graph::NODE_NEW],
            Self::NodePinToggle => &[input_action::graph::NODE_PIN_TOGGLE],
            Self::NodePinSelected => &[input_action::graph::NODE_PIN_SELECTED],
            Self::NodeUnpinSelected => &[input_action::graph::NODE_UNPIN_SELECTED],
            Self::NodeDelete => &[input_action::graph::NODE_DELETE],
            Self::EdgeConnectPair => &[input_action::graph::EDGE_CONNECT_PAIR],
            Self::EdgeConnectBoth => &[input_action::graph::EDGE_CONNECT_BOTH],
            Self::EdgeRemoveUser => &[input_action::graph::EDGE_REMOVE_USER],
            Self::GraphFit => &[],
            Self::GraphFitGraphlet => &[],
            Self::GraphTogglePhysics => &[input_action::graph::TOGGLE_PHYSICS],
            Self::GraphPhysicsConfig => &[input_action::workbench::OPEN_PHYSICS_SETTINGS],
            Self::GraphCommandPalette => &[input_action::graph::COMMAND_PALETTE_OPEN],
            Self::GraphRadialMenu => &[input_action::graph::RADIAL_MENU_OPEN],
            Self::PersistUndo => &[input_action::workbench::UNDO],
            Self::PersistRedo => &[input_action::workbench::REDO],
            Self::PersistOpenHistoryManager => &[input_action::workbench::OPEN_HISTORY_MANAGER],
            _ => &[],
        };

        action_ids
            .iter()
            .flat_map(|action_id| {
                crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
                    action_id,
                )
            })
            .collect()
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::NodeNew => "node:new",
            Self::NodeNewAsTab => "node:new_as_tab",
            Self::NodePinToggle => "node:pin_toggle",
            Self::NodePinSelected => "node:pin_selected",
            Self::NodeUnpinSelected => "node:unpin_selected",
            Self::NodeDelete => "node:delete",
            Self::NodeChooseFrame => "node:choose_frame",
            Self::NodeAddToFrame => "node:add_to_frame",
            Self::NodeAddConnectedToFrame => "node:add_connected_to_frame",
            Self::NodeOpenFrame => "node:open_frame",
            Self::NodeOpenNeighbors => "node:open_neighbors",
            Self::NodeOpenConnected => "node:open_connected",
            Self::NodeOpenSplit => "node:open_split",
            Self::NodeDetachToSplit => "node:detach_to_split",
            Self::NodeMoveToActivePane => "node:move_to_active_pane",
            Self::NodeWarmSelect => "node:warm_select",
            Self::NodeRemoveFromGraphlet => "node:remove_from_graphlet",
            Self::NodeMarkTombstone => "node:mark_tombstone",
            Self::NodeCopyUrl => "node:copy_url",
            Self::NodeCopyTitle => "node:copy_title",
            Self::NodeRenderAuto => "node:render_auto",
            Self::NodeRenderWebView => "node:render_webview",
            Self::NodeRenderWry => "node:render_wry",
            Self::EdgeConnectPair => "edge:connect_pair",
            Self::EdgeConnectBoth => "edge:connect_both",
            Self::EdgeRemoveUser => "edge:remove_user",
            Self::GraphFit => "graph:fit",
            Self::GraphFitGraphlet => "graph:fit_graphlet",
            Self::GraphCycleFocusRegion => "graph:cycle_focus_region",
            Self::GraphTogglePhysics => "graph:toggle_physics",
            Self::GraphToggleGhostNodes => "graph:toggle_ghost_nodes",
            Self::GraphPhysicsConfig => "graph:physics_config",
            Self::GraphCommandPalette => "workbench:command_palette_open",
            Self::GraphRadialMenu => "workbench:radial_menu_open",
            Self::WorkbenchUnlockSurfaceLayout => "workbench:unlock_surface_layout",
            Self::WorkbenchLockSurfaceLayout => "workbench:lock_surface_layout",
            Self::WorkbenchRememberLayoutPreference => "workbench:remember_layout_preference",
            Self::WorkbenchGroupSelectedTiles => "workbench:group_selected_tiles",
            Self::PersistUndo => "persistence:undo",
            Self::PersistRedo => "persistence:redo",
            Self::PersistSaveSnapshot => "persistence:save_snapshot",
            Self::PersistRestoreSession => "persistence:restore_session",
            Self::PersistSaveGraph => "persistence:save_graph",
            Self::PersistRestoreLatestGraph => "persistence:restore_latest_graph",
            Self::PersistOpenHub => "persistence:open_hub",
            Self::WorkbenchOpenSettingsPane => "workbench:settings_pane",
            Self::WorkbenchOpenSettingsOverlay => "workbench:settings_overlay",
            Self::PersistOpenHistoryManager => "workbench:open_history_manager",
            Self::WorkbenchActivateWorkflowDefault => "workbench:activate_workflow_default",
            Self::WorkbenchActivateWorkflowResearch => "workbench:activate_workflow_research",
            Self::WorkbenchActivateWorkflowReading => "workbench:activate_workflow_reading",
        }
    }

    /// Short label suitable for a radial menu sector (≤ 12 chars).
    pub fn short_label(self) -> &'static str {
        match self {
            Self::NodeNew => "New",
            Self::NodeNewAsTab => "New Tab",
            Self::NodePinToggle => "Pin",
            Self::NodePinSelected => "Pin",
            Self::NodeUnpinSelected => "Unpin",
            Self::NodeDelete => "Delete",
            Self::NodeChooseFrame => "Choose F",
            Self::NodeAddToFrame => "Add F",
            Self::NodeAddConnectedToFrame => "Add Conn F",
            Self::NodeOpenFrame => "Frame",
            Self::NodeOpenNeighbors => "Neighbors",
            Self::NodeOpenConnected => "Connected",
            Self::NodeOpenSplit => "Split",
            Self::NodeDetachToSplit => "Detach",
            Self::NodeMoveToActivePane => "Move",
            Self::NodeWarmSelect => "Open Cold",
            Self::NodeRemoveFromGraphlet => "Leave Group",
            Self::NodeMarkTombstone => "Ghost",
            Self::NodeCopyUrl => "Copy URL",
            Self::NodeCopyTitle => "Copy Title",
            Self::NodeRenderAuto => "Auto",
            Self::NodeRenderWebView => "WebView",
            Self::NodeRenderWry => "Wry",
            Self::EdgeConnectPair => "Pair",
            Self::EdgeConnectBoth => "Both",
            Self::EdgeRemoveUser => "Remove",
            Self::GraphFit => "Fit",
            Self::GraphFitGraphlet => "Fit Graphlet",
            Self::GraphCycleFocusRegion => "Focus",
            Self::GraphTogglePhysics => "Physics",
            Self::GraphToggleGhostNodes => "Ghosts",
            Self::GraphPhysicsConfig => "Config",
            Self::GraphCommandPalette => "Cmd",
            Self::GraphRadialMenu => "Radial",
            Self::WorkbenchUnlockSurfaceLayout => "Unlock",
            Self::WorkbenchLockSurfaceLayout => "Lock",
            Self::WorkbenchRememberLayoutPreference => "Remember",
            Self::WorkbenchGroupSelectedTiles => "Group Tiles",
            Self::PersistUndo => "Undo",
            Self::PersistRedo => "Redo",
            Self::PersistSaveSnapshot => "Save W",
            Self::PersistRestoreSession => "Restore W",
            Self::PersistSaveGraph => "Save G",
            Self::PersistRestoreLatestGraph => "Latest G",
            Self::PersistOpenHub => "Persist Ovl",
            Self::WorkbenchOpenSettingsPane => "Set Pane",
            Self::WorkbenchOpenSettingsOverlay => "Set Ovl",
            Self::PersistOpenHistoryManager => "History",
            Self::WorkbenchActivateWorkflowDefault => "Workflow D",
            Self::WorkbenchActivateWorkflowResearch => "Workflow R",
            Self::WorkbenchActivateWorkflowReading => "Workflow Read",
        }
    }

    /// Full label suitable for the command palette list.
    pub fn label(self) -> &'static str {
        match self {
            Self::NodeNew => "Create Node",
            Self::NodeNewAsTab => "Create Node as Tab",
            Self::NodePinToggle => "Toggle Pin",
            Self::NodePinSelected => "Pin Selected",
            Self::NodeUnpinSelected => "Unpin Selected",
            Self::NodeDelete => "Delete Selected Node(s)",
            Self::NodeChooseFrame => "Choose Frame...",
            Self::NodeAddToFrame => "Add To Frame...",
            Self::NodeAddConnectedToFrame => "Add Connected To Frame...",
            Self::NodeOpenFrame => "Open via Frame Route",
            Self::NodeOpenNeighbors => "Open with Neighbors",
            Self::NodeOpenConnected => "Open with Connected",
            Self::NodeOpenSplit => "Open Node in Split",
            Self::NodeDetachToSplit => "Detach Focused to Split",
            Self::NodeMoveToActivePane => "Move Node to Active Pane",
            Self::NodeWarmSelect => "Open Cold Selection as Tiles",
            Self::NodeRemoveFromGraphlet => "Remove from Graphlet",
            Self::NodeMarkTombstone => "Ghost Selected Node(s)",
            Self::NodeCopyUrl => "Copy Node URL",
            Self::NodeCopyTitle => "Copy Node Title",
            Self::NodeRenderAuto => "Render With Auto",
            Self::NodeRenderWebView => "Render With WebView",
            Self::NodeRenderWry => "Render With Wry",
            Self::EdgeConnectPair => "Connect Source -> Target",
            Self::EdgeConnectBoth => "Connect Both Directions",
            Self::EdgeRemoveUser => "Remove User Edge",
            Self::GraphFit => "Fit Graph to Screen",
            Self::GraphFitGraphlet => "Fit Graphlet to Screen",
            Self::GraphCycleFocusRegion => "Cycle Focus Region",
            Self::GraphTogglePhysics => "Toggle Physics Simulation",
            Self::GraphToggleGhostNodes => "Toggle Ghost Node Visibility",
            Self::GraphPhysicsConfig => "Open Physics Settings",
            Self::GraphCommandPalette => "Open Command Palette",
            Self::GraphRadialMenu => "Open Radial Palette",
            Self::WorkbenchUnlockSurfaceLayout => "Unlock Surface Layout",
            Self::WorkbenchLockSurfaceLayout => "Lock Surface Layout",
            Self::WorkbenchRememberLayoutPreference => "Remember Layout Preference",
            Self::WorkbenchGroupSelectedTiles => "Group Selected Tiles",
            Self::PersistUndo => "Undo",
            Self::PersistRedo => "Redo",
            Self::PersistSaveSnapshot => "Save Frame Snapshot",
            Self::PersistRestoreSession => "Restore Session Frame",
            Self::PersistSaveGraph => "Save Graph Snapshot",
            Self::PersistRestoreLatestGraph => "Restore Latest Graph",
            Self::PersistOpenHub => "Open Persistence Overlay",
            Self::WorkbenchOpenSettingsPane => "Open Settings Pane",
            Self::WorkbenchOpenSettingsOverlay => "Open Settings Overlay",
            Self::PersistOpenHistoryManager => "Open History Manager",
            Self::WorkbenchActivateWorkflowDefault => "Activate Default Workflow",
            Self::WorkbenchActivateWorkflowResearch => "Activate Research Workflow",
            Self::WorkbenchActivateWorkflowReading => "Activate Reading Workflow",
        }
    }

    /// Logical category for grouping in command surfaces.
    pub fn category(self) -> ActionCategory {
        match self {
            Self::NodeNew
            | Self::NodeNewAsTab
            | Self::NodePinToggle
            | Self::NodePinSelected
            | Self::NodeUnpinSelected
            | Self::NodeDelete
            | Self::NodeChooseFrame
            | Self::NodeAddToFrame
            | Self::NodeAddConnectedToFrame
            | Self::NodeOpenFrame
            | Self::NodeOpenNeighbors
            | Self::NodeOpenConnected
            | Self::NodeOpenSplit
            | Self::NodeDetachToSplit
            | Self::NodeMoveToActivePane
            | Self::NodeCopyUrl
            | Self::NodeCopyTitle
            | Self::NodeRenderAuto
            | Self::NodeRenderWebView
            | Self::NodeRenderWry
            | Self::NodeWarmSelect
            | Self::NodeRemoveFromGraphlet
            | Self::NodeMarkTombstone => ActionCategory::Node,
            Self::EdgeConnectPair | Self::EdgeConnectBoth | Self::EdgeRemoveUser => {
                ActionCategory::Edge
            }
            Self::GraphFit
            | Self::GraphFitGraphlet
            | Self::GraphCycleFocusRegion
            | Self::GraphTogglePhysics
            | Self::GraphToggleGhostNodes
            | Self::GraphPhysicsConfig
            | Self::GraphCommandPalette
            | Self::GraphRadialMenu
            | Self::WorkbenchUnlockSurfaceLayout
            | Self::WorkbenchLockSurfaceLayout
            | Self::WorkbenchRememberLayoutPreference
            | Self::WorkbenchGroupSelectedTiles => ActionCategory::Graph,
            Self::PersistUndo
            | Self::PersistRedo
            | Self::PersistSaveSnapshot
            | Self::PersistRestoreSession
            | Self::PersistSaveGraph
            | Self::PersistRestoreLatestGraph
            | Self::PersistOpenHub
            | Self::WorkbenchOpenSettingsPane
            | Self::WorkbenchOpenSettingsOverlay
            | Self::PersistOpenHistoryManager
            | Self::WorkbenchActivateWorkflowDefault
            | Self::WorkbenchActivateWorkflowResearch
            | Self::WorkbenchActivateWorkflowReading => ActionCategory::Persistence,
        }
    }
}

fn action_id_has_namespace_format(id: &str) -> bool {
    let mut parts = id.split(':');
    let Some(namespace) = parts.next() else {
        return false;
    };
    let Some(name) = parts.next() else {
        return false;
    };
    if parts.next().is_some() || namespace.is_empty() || name.is_empty() {
        return false;
    }
    namespace
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch == '_')
        && name.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_')
}

fn all_action_ids() -> &'static [ActionId] {
    use ActionId::*;

    &[
        NodeNew,
        NodeNewAsTab,
        NodePinToggle,
        NodePinSelected,
        NodeUnpinSelected,
        NodeDelete,
        NodeChooseFrame,
        NodeAddToFrame,
        NodeAddConnectedToFrame,
        NodeOpenFrame,
        NodeOpenNeighbors,
        NodeOpenConnected,
        NodeOpenSplit,
        NodeDetachToSplit,
        NodeMoveToActivePane,
        NodeCopyUrl,
        NodeCopyTitle,
        NodeRenderAuto,
        NodeRenderWebView,
        NodeRenderWry,
        NodeWarmSelect,
        NodeRemoveFromGraphlet,
        NodeMarkTombstone,
        EdgeConnectPair,
        EdgeConnectBoth,
        EdgeRemoveUser,
        GraphFit,
        GraphFitGraphlet,
        GraphCycleFocusRegion,
        GraphTogglePhysics,
        GraphToggleGhostNodes,
        GraphPhysicsConfig,
        GraphCommandPalette,
        GraphRadialMenu,
        WorkbenchUnlockSurfaceLayout,
        WorkbenchLockSurfaceLayout,
        WorkbenchRememberLayoutPreference,
        WorkbenchGroupSelectedTiles,
        PersistUndo,
        PersistRedo,
        PersistSaveSnapshot,
        PersistRestoreSession,
        PersistSaveGraph,
        PersistRestoreLatestGraph,
        PersistOpenHub,
        WorkbenchOpenSettingsPane,
        WorkbenchOpenSettingsOverlay,
        PersistOpenHistoryManager,
        WorkbenchActivateWorkflowDefault,
        WorkbenchActivateWorkflowResearch,
        WorkbenchActivateWorkflowReading,
    ]
}

fn warn_on_nonconforming_action_keys() {
    ACTION_KEY_AUDIT_ONCE.call_once(|| {
        for action_id in all_action_ids() {
            let key = action_id.key();
            if !action_id_has_namespace_format(key) {
                log::warn!(
                    "action_registry: key {:?} does not follow namespace:name format",
                    key
                );
            }
        }
    });
}

/// Context passed to [`list_actions_for_context`] to drive enabled/disabled
/// state and scope filtering.
#[derive(Clone, Debug, Default)]
pub struct ActionContext {
    /// Primary target node, if any (right-click target, hovered node, etc.).
    /// `None` means global scope — the full action list is returned.
    pub target_node: Option<NodeKey>,
    /// Whether a valid source–target pair exists (for edge actions).
    pub pair_context: Option<(NodeKey, NodeKey)>,
    /// Whether at least one node is selected.
    pub any_selected: bool,
    /// Whether a focused pane node is available (for detach-to-split).
    pub focused_pane_available: bool,
    /// Whether undo stack has an available entry.
    pub undo_available: bool,
    /// Whether redo stack has an available entry.
    pub redo_available: bool,
    /// Preferred input mode (layout hint).
    pub input_mode: InputMode,
    /// Active view (for future per-view action customisation).
    pub view_id: GraphViewId,
    /// Whether explicit Wry override selection is currently allowed.
    pub wry_override_allowed: bool,
    /// Whether a layout-configurable Navigator host is currently available.
    pub layout_surface_host_available: bool,
    /// The explicit Navigator host targeted by layout actions when unambiguous.
    pub layout_surface_target_host: Option<SurfaceHostId>,
    /// Whether multiple visible Navigator hosts exist and no explicit host target is selected.
    pub layout_surface_target_ambiguous: bool,
    /// Whether the active layout-configurable host is currently in config mode.
    pub layout_surface_configuring: bool,
    /// Whether the active layout-configurable host currently has a draft layout.
    pub layout_surface_has_draft: bool,
}

/// A single resolved action entry returned by [`list_actions_for_context`].
#[derive(Clone, Debug)]
pub struct ActionEntry {
    /// Stable action identifier used for dispatch.
    pub id: ActionId,
    /// Whether the action is executable in the current context.
    pub enabled: bool,
}

/// Return all actions that should appear in a command surface for the given
/// context, with enabled/disabled state pre-resolved.
///
/// The returned list is ordered: Node actions first, then Edge, Graph, and
/// Persistence.  Disabled actions are included so surfaces can show them
/// greyed out rather than hiding them (consistent palette behaviour).
pub fn list_actions_for_context(context: &ActionContext) -> Vec<ActionEntry> {
    warn_on_nonconforming_action_keys();
    use ActionId::*;

    let node_ops_enabled = context.any_selected || context.target_node.is_some();
    let pair_enabled = context.pair_context.is_some();

    let all: &[(ActionId, bool)] = &[
        // Node
        (NodeNew, true),
        (NodeNewAsTab, true),
        (NodePinSelected, node_ops_enabled),
        (NodeUnpinSelected, node_ops_enabled),
        (NodeDelete, node_ops_enabled),
        (NodeChooseFrame, node_ops_enabled),
        (NodeAddToFrame, node_ops_enabled),
        (NodeAddConnectedToFrame, node_ops_enabled),
        (NodeOpenFrame, node_ops_enabled),
        (NodeOpenNeighbors, node_ops_enabled),
        (NodeOpenConnected, node_ops_enabled),
        (NodeOpenSplit, node_ops_enabled),
        (NodeDetachToSplit, context.focused_pane_available),
        (NodeMoveToActivePane, node_ops_enabled),
        (NodeCopyUrl, node_ops_enabled),
        (NodeCopyTitle, node_ops_enabled),
        (NodeRenderAuto, node_ops_enabled),
        (NodeRenderWebView, node_ops_enabled),
        (
            NodeRenderWry,
            node_ops_enabled && context.wry_override_allowed,
        ),
        (NodeWarmSelect, node_ops_enabled),
        (NodeRemoveFromGraphlet, node_ops_enabled),
        (NodeMarkTombstone, node_ops_enabled),
        // Edge
        (EdgeConnectPair, pair_enabled),
        (EdgeConnectBoth, pair_enabled),
        (EdgeRemoveUser, pair_enabled),
        // Graph
        (GraphFit, true),
        (GraphFitGraphlet, true),
        (GraphTogglePhysics, true),
        (GraphToggleGhostNodes, true),
        (GraphPhysicsConfig, true),
        (GraphCommandPalette, true),
        (
            WorkbenchUnlockSurfaceLayout,
            context.layout_surface_host_available && !context.layout_surface_configuring,
        ),
        (
            WorkbenchLockSurfaceLayout,
            context.layout_surface_host_available && context.layout_surface_configuring,
        ),
        (
            WorkbenchRememberLayoutPreference,
            context.layout_surface_host_available
                && (context.layout_surface_configuring || context.layout_surface_has_draft),
        ),
        (WorkbenchGroupSelectedTiles, true),
        // Persistence
        (PersistUndo, context.undo_available),
        (PersistRedo, context.redo_available),
        (PersistSaveSnapshot, true),
        (PersistRestoreSession, true),
        (PersistSaveGraph, true),
        (PersistRestoreLatestGraph, true),
        (PersistOpenHub, true),
        (WorkbenchOpenSettingsPane, true),
        (WorkbenchOpenSettingsOverlay, true),
        (PersistOpenHistoryManager, true),
        (WorkbenchActivateWorkflowDefault, true),
        (WorkbenchActivateWorkflowResearch, true),
        (WorkbenchActivateWorkflowReading, true),
    ];

    all.iter()
        .map(|&(id, enabled)| ActionEntry { id, enabled })
        .collect()
}

/// Return only the actions belonging to a specific category, with
/// enabled/disabled state resolved for the given context.
///
/// Convenience wrapper around [`list_actions_for_context`] used by the
/// radial menu to populate per-domain sectors.
pub fn list_actions_for_category(
    context: &ActionContext,
    category: ActionCategory,
) -> Vec<ActionEntry> {
    list_actions_for_context(context)
        .into_iter()
        .filter(|e| e.id.category() == category)
        .collect()
}

/// Return the radial menu action set: a curated subset per category for
/// directional/radial layout, omitting palette-only actions like `NodePinSelected`
/// / `NodeUnpinSelected` (represented by the combined `NodePinToggle`) and
/// `NodeDetachToSplit` / `NodeNewAsTab`.
pub fn list_radial_actions_for_category(
    context: &ActionContext,
    category: ActionCategory,
) -> Vec<ActionEntry> {
    use ActionId::*;
    // Palette-only actions excluded from the radial menu.
    const RADIAL_EXCLUDED: &[ActionId] = &[
        NodePinSelected,
        NodeUnpinSelected,
        NodeDetachToSplit,
        NodeNewAsTab,
    ];

    let mut entries = list_actions_for_category(context, category);
    // Replace NodePinSelected/NodeUnpinSelected with NodePinToggle for the radial menu.
    let has_node_pin = category == ActionCategory::Node;
    if has_node_pin {
        entries.retain(|e| !RADIAL_EXCLUDED.contains(&e.id));
        // Insert NodePinToggle in place of the pin actions.
        let pin_idx = entries
            .iter()
            .position(|e| e.id == NodeDelete)
            .unwrap_or(entries.len());
        entries.insert(
            pin_idx,
            ActionEntry {
                id: NodePinToggle,
                enabled: context.any_selected || context.target_node.is_some(),
            },
        );
    } else {
        entries.retain(|e| !RADIAL_EXCLUDED.contains(&e.id));
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphViewId;

    fn default_context() -> ActionContext {
        ActionContext {
            target_node: None,
            pair_context: None,
            any_selected: false,
            focused_pane_available: false,
            undo_available: false,
            redo_available: false,
            input_mode: InputMode::MouseKeyboard,
            view_id: GraphViewId::new(),
            wry_override_allowed: false,
            layout_surface_host_available: false,
            layout_surface_target_host: None,
            layout_surface_target_ambiguous: false,
            layout_surface_configuring: false,
            layout_surface_has_draft: false,
        }
    }

    #[test]
    fn test_list_actions_returns_all_action_ids() {
        let ctx = default_context();
        let entries = list_actions_for_context(&ctx);
        let ids: Vec<ActionId> = entries.iter().map(|e| e.id).collect();
        assert!(ids.contains(&ActionId::NodeNew));
        assert!(ids.contains(&ActionId::EdgeConnectPair));
        assert!(ids.contains(&ActionId::GraphFit));
        assert!(ids.contains(&ActionId::PersistUndo));
    }

    #[test]
    fn test_node_ops_disabled_without_selection_or_target() {
        let ctx = default_context();
        let entries = list_actions_for_context(&ctx);
        let pin = entries
            .iter()
            .find(|e| e.id == ActionId::NodePinSelected)
            .unwrap();
        assert!(!pin.enabled);
    }

    #[test]
    fn test_node_ops_enabled_with_selection() {
        let ctx = ActionContext {
            any_selected: true,
            ..default_context()
        };
        let entries = list_actions_for_context(&ctx);
        let pin = entries
            .iter()
            .find(|e| e.id == ActionId::NodePinSelected)
            .unwrap();
        assert!(pin.enabled);
    }

    #[test]
    fn test_edge_ops_disabled_without_pair() {
        let ctx = default_context();
        let entries = list_actions_for_context(&ctx);
        let edge = entries
            .iter()
            .find(|e| e.id == ActionId::EdgeConnectPair)
            .unwrap();
        assert!(!edge.enabled);
    }

    #[test]
    fn test_edge_ops_enabled_with_pair() {
        use crate::app::GraphBrowserApp;
        use euclid::default::Point2D;
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));
        let ctx = ActionContext {
            pair_context: Some((a, b)),
            ..default_context()
        };
        let entries = list_actions_for_context(&ctx);
        let edge = entries
            .iter()
            .find(|e| e.id == ActionId::EdgeConnectPair)
            .unwrap();
        assert!(edge.enabled);
    }

    #[test]
    fn test_detach_to_split_enabled_only_with_focused_pane() {
        let ctx_no_pane = default_context();
        let entries = list_actions_for_context(&ctx_no_pane);
        let detach = entries
            .iter()
            .find(|e| e.id == ActionId::NodeDetachToSplit)
            .unwrap();
        assert!(!detach.enabled);
        let ctx_with_pane = ActionContext {
            focused_pane_available: true,
            ..default_context()
        };
        let entries = list_actions_for_context(&ctx_with_pane);
        let detach = entries
            .iter()
            .find(|e| e.id == ActionId::NodeDetachToSplit)
            .unwrap();
        assert!(detach.enabled);
    }

    #[test]
    fn test_layout_actions_follow_host_config_state() {
        let ctx = ActionContext {
            layout_surface_host_available: true,
            layout_surface_target_host: Some(SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            )),
            ..default_context()
        };
        let entries = list_actions_for_context(&ctx);
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::WorkbenchUnlockSurfaceLayout)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::WorkbenchLockSurfaceLayout)
                .is_some_and(|entry| !entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::WorkbenchRememberLayoutPreference)
                .is_some_and(|entry| !entry.enabled)
        );

        let ctx = ActionContext {
            layout_surface_host_available: true,
            layout_surface_target_host: Some(SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            )),
            layout_surface_configuring: true,
            layout_surface_has_draft: true,
            ..default_context()
        };
        let entries = list_actions_for_context(&ctx);
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::WorkbenchUnlockSurfaceLayout)
                .is_some_and(|entry| !entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::WorkbenchLockSurfaceLayout)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::WorkbenchRememberLayoutPreference)
                .is_some_and(|entry| entry.enabled)
        );

        let ctx = ActionContext {
            layout_surface_host_available: false,
            layout_surface_target_ambiguous: true,
            ..default_context()
        };
        let entries = list_actions_for_context(&ctx);
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::WorkbenchUnlockSurfaceLayout)
                .is_some_and(|entry| !entry.enabled)
        );
    }

    #[test]
    fn test_list_actions_for_category_filters_correctly() {
        let ctx = default_context();
        let node_entries = list_actions_for_category(&ctx, ActionCategory::Node);
        assert!(
            node_entries
                .iter()
                .all(|e| e.id.category() == ActionCategory::Node)
        );
        assert!(!node_entries.is_empty());
    }

    #[test]
    fn test_radial_actions_exclude_palette_only_entries() {
        let ctx = default_context();
        let radial_node = list_radial_actions_for_category(&ctx, ActionCategory::Node);
        let ids: Vec<ActionId> = radial_node.iter().map(|e| e.id).collect();
        assert!(!ids.contains(&ActionId::NodePinSelected));
        assert!(!ids.contains(&ActionId::NodeUnpinSelected));
        assert!(!ids.contains(&ActionId::NodeDetachToSplit));
        assert!(!ids.contains(&ActionId::NodeNewAsTab));
        assert!(ids.contains(&ActionId::NodePinToggle));
    }

    #[test]
    fn test_action_id_labels_are_nonempty() {
        for &id in all_action_ids() {
            assert!(!id.label().is_empty(), "{id:?} has empty label");
            assert!(!id.short_label().is_empty(), "{id:?} has empty short_label");
        }
    }

    #[test]
    fn test_action_ids_follow_namespace_name_format() {
        for &id in all_action_ids() {
            assert!(
                action_id_has_namespace_format(id.key()),
                "{:?} key should follow namespace:name, got {}",
                id,
                id.key()
            );
        }
    }

    #[test]
    fn test_graph_actions_always_enabled() {
        let ctx = default_context();
        let entries = list_actions_for_context(&ctx);
        for entry in entries
            .iter()
            .filter(|e| matches!(e.id.category(), ActionCategory::Graph))
        {
            assert!(entry.enabled, "{:?} should always be enabled", entry.id);
        }
    }

    #[test]
    fn test_rank_categories_pins_precede_dynamic_order() {
        let ctx = default_context();
        let categories = default_category_order();
        let ordered = rank_categories_for_context(
            &categories,
            &ctx,
            &[ActionCategory::Node],
            &[ActionCategory::Persistence, ActionCategory::Graph],
        );
        assert_eq!(ordered[0], ActionCategory::Persistence);
        assert_eq!(ordered[1], ActionCategory::Graph);
    }

    #[test]
    fn test_rank_categories_node_context_promotes_node_when_unpinned() {
        let ctx = ActionContext {
            target_node: Some(NodeKey::new(1)),
            ..default_context()
        };
        let categories = default_category_order();
        let ordered =
            rank_categories_for_context(&categories, &ctx, &[ActionCategory::Persistence], &[]);
        assert_eq!(ordered[0], ActionCategory::Node);
    }

    #[test]
    fn test_persistence_undo_redo_disabled_without_stack_entries() {
        let ctx = default_context();
        let entries = list_actions_for_context(&ctx);
        let undo = entries
            .iter()
            .find(|e| e.id == ActionId::PersistUndo)
            .unwrap();
        let redo = entries
            .iter()
            .find(|e| e.id == ActionId::PersistRedo)
            .unwrap();
        assert!(!undo.enabled);
        assert!(!redo.enabled);
    }

    #[test]
    fn test_persistence_undo_redo_enabled_with_stack_entries() {
        let ctx = ActionContext {
            undo_available: true,
            redo_available: true,
            ..default_context()
        };
        let entries = list_actions_for_context(&ctx);
        let undo = entries
            .iter()
            .find(|e| e.id == ActionId::PersistUndo)
            .unwrap();
        let redo = entries
            .iter()
            .find(|e| e.id == ActionId::PersistRedo)
            .unwrap();
        assert!(undo.enabled);
        assert!(redo.enabled);
    }

    #[test]
    fn test_workflow_and_history_actions_are_listed_in_persistence_bucket() {
        let ctx = default_context();
        let entries = list_actions_for_context(&ctx);

        for action_id in [
            ActionId::WorkbenchOpenSettingsPane,
            ActionId::WorkbenchOpenSettingsOverlay,
            ActionId::PersistOpenHistoryManager,
            ActionId::WorkbenchActivateWorkflowDefault,
            ActionId::WorkbenchActivateWorkflowResearch,
            ActionId::WorkbenchActivateWorkflowReading,
        ] {
            let entry = entries
                .iter()
                .find(|entry| entry.id == action_id)
                .unwrap_or_else(|| panic!("missing action entry for {action_id:?}"));
            assert!(entry.enabled);
            assert_eq!(action_id.category(), ActionCategory::Persistence);
        }
    }

    #[test]
    fn test_representative_action_labels_convey_purpose_in_context() {
        let cases = [
            (ActionId::NodeCopyUrl, ["Copy", "URL"].as_slice()),
            (ActionId::NodeDelete, ["Delete", "Node"].as_slice()),
            (ActionId::NodeOpenFrame, ["Open", "Frame"].as_slice()),
            (ActionId::EdgeConnectPair, ["Connect", "Target"].as_slice()),
            (ActionId::PersistSaveGraph, ["Save", "Graph"].as_slice()),
            (
                ActionId::PersistRestoreLatestGraph,
                ["Restore", "Graph"].as_slice(),
            ),
            (
                ActionId::PersistOpenHistoryManager,
                ["Open", "History"].as_slice(),
            ),
            (
                ActionId::WorkbenchOpenSettingsPane,
                ["Open", "Settings", "Pane"].as_slice(),
            ),
            (
                ActionId::WorkbenchOpenSettingsOverlay,
                ["Open", "Settings", "Overlay"].as_slice(),
            ),
            (
                ActionId::WorkbenchActivateWorkflowResearch,
                ["Activate", "Research"].as_slice(),
            ),
        ];

        for (action_id, required_terms) in cases {
            let label = action_id.label();
            for term in required_terms {
                assert!(
                    label.contains(term),
                    "{action_id:?} label should include '{term}' to communicate purpose, got: {label}"
                );
            }
        }
    }
}
