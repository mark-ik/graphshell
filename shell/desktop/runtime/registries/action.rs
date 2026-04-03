use std::collections::HashMap;

use crate::app::{
    AppCommand, ChooseFramePickerMode, ClipboardCopyKind, ClipboardCopyRequest, GraphBrowserApp,
    GraphIntent, GraphMutation, LifecycleCause, PendingConnectedOpenScope, PendingNodeOpenRequest,
    PendingTileOpenMode, RuntimeEvent, SelectionUpdateMode, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::shell::desktop::workbench::pane_model::{PaneId, SplitDirection, ToolPaneState};
use crate::util::{GraphshellSettingsPath, VersoAddress};

use super::index::SearchResultKind;

pub(crate) const ACTION_OMNIBOX_NODE_SEARCH: &str = "omnibox:node_search";
pub(crate) const ACTION_GRAPH_VIEW_SUBMIT: &str = "graph:view_submit";
pub(crate) const ACTION_DETAIL_VIEW_SUBMIT: &str = "detail:view_submit";

pub(crate) const ACTION_GRAPH_NODE_OPEN: &str = "graph:node_open";
pub(crate) const ACTION_GRAPH_NODE_CLOSE: &str = "graph:node_close";
pub(crate) const ACTION_GRAPH_NODE_NEW: &str = "graph:node_new";
pub(crate) const ACTION_GRAPH_NODE_NEW_AS_TAB: &str = "graph:node_new_as_tab";
pub(crate) const ACTION_GRAPH_NODE_PIN_TOGGLE: &str = "graph:node_pin_toggle";
pub(crate) const ACTION_GRAPH_NODE_PIN_SELECTED: &str = "graph:node_pin_selected";
pub(crate) const ACTION_GRAPH_NODE_UNPIN_SELECTED: &str = "graph:node_unpin_selected";
pub(crate) const ACTION_GRAPH_NODE_DELETE: &str = "graph:node_delete";
pub(crate) const ACTION_GRAPH_NODE_CHOOSE_FRAME: &str = "graph:node_choose_frame";
pub(crate) const ACTION_GRAPH_NODE_ADD_TO_FRAME: &str = "graph:node_add_to_frame";
pub(crate) const ACTION_GRAPH_NODE_ADD_CONNECTED_TO_FRAME: &str =
    "graph:node_add_connected_to_frame";
pub(crate) const ACTION_GRAPH_NODE_OPEN_FRAME: &str = "graph:node_open_frame";
pub(crate) const ACTION_GRAPH_NODE_OPEN_NEIGHBORS: &str = "graph:node_open_neighbors";
pub(crate) const ACTION_GRAPH_NODE_OPEN_CONNECTED: &str = "graph:node_open_connected";
pub(crate) const ACTION_GRAPH_NODE_OPEN_SPLIT: &str = "graph:node_open_split";
/// Open a tile for every cold node in the current selection, routing each into
/// its graphlet's existing tab group (Phase 6 — canvas warm-select).
pub(crate) const ACTION_GRAPH_SELECTION_WARM_SELECT: &str = "graph:selection_warm_select";
/// Retract all durable graphlet edges (`UserGrouped` and `FrameMember`) between
/// the selected node and its graphlet peers, removing it from the graphlet.
/// Lifecycle is unchanged; semantic history edges (Hyperlink, History) are not touched.
pub(crate) const ACTION_GRAPH_NODE_REMOVE_FROM_GRAPHLET: &str = "graph:node_remove_from_graphlet";
pub(crate) const ACTION_GRAPH_NODE_COPY_URL: &str = "graph:node_copy_url";
pub(crate) const ACTION_GRAPH_NODE_COPY_TITLE: &str = "graph:node_copy_title";
pub(crate) const ACTION_GRAPH_EDGE_CREATE: &str = "graph:edge_create";
pub(crate) const ACTION_GRAPH_EDGE_CONNECT_PAIR: &str = "graph:edge_connect_pair";
pub(crate) const ACTION_GRAPH_EDGE_CONNECT_BOTH: &str = "graph:edge_connect_both";
pub(crate) const ACTION_GRAPH_EDGE_REMOVE_USER: &str = "graph:edge_remove_user";
pub(crate) const ACTION_GRAPH_SET_PHYSICS_PROFILE: &str = "graph:set_physics_profile";
pub(crate) const ACTION_GRAPH_FIT: &str = "graph:fit";
pub(crate) const ACTION_GRAPH_TOGGLE_PHYSICS: &str = "graph:toggle_physics";
pub(crate) const ACTION_GRAPH_NAVIGATE_BACK: &str = "graph:navigate_back";
pub(crate) const ACTION_GRAPH_NAVIGATE_FORWARD: &str = "graph:navigate_forward";
pub(crate) const ACTION_GRAPH_SELECT_NODE: &str = "graph:select_node";
pub(crate) const ACTION_GRAPH_DESELECT_ALL: &str = "graph:deselect_all";
pub(crate) const ACTION_WORKBENCH_DETACH_TO_SPLIT: &str = "workbench:detach_to_split";
pub(crate) const ACTION_WORKBENCH_MOVE_NODE_TO_PANE: &str = "workbench:move_node_to_pane";
pub(crate) const ACTION_WORKBENCH_SPLIT_HORIZONTAL: &str = "workbench:split_horizontal";
pub(crate) const ACTION_WORKBENCH_SPLIT_VERTICAL: &str = "workbench:split_vertical";
pub(crate) const ACTION_WORKBENCH_CLOSE_PANE: &str = "workbench:close_pane";
pub(crate) const ACTION_WORKBENCH_COMMAND_PALETTE_OPEN: &str = "workbench:command_palette_open";
pub(crate) const ACTION_WORKBENCH_HELP_OPEN: &str = "workbench:help_open";
pub(crate) const ACTION_WORKBENCH_SETTINGS_PANE_OPEN: &str = "workbench:settings_pane_open";
pub(crate) const ACTION_WORKBENCH_SETTINGS_OVERLAY_OPEN: &str = "workbench:settings_overlay_open";
/// Legacy alias retained while higher-level bundles migrate to the explicit pane action.
pub(crate) const ACTION_WORKBENCH_SETTINGS_OPEN: &str = "workbench:settings_open";
pub(crate) const ACTION_WORKBENCH_UNDO: &str = "workbench:undo";
pub(crate) const ACTION_WORKBENCH_REDO: &str = "workbench:redo";
pub(crate) const ACTION_WORKBENCH_SAVE_SNAPSHOT: &str = "workbench:save_snapshot";
pub(crate) const ACTION_WORKBENCH_RESTORE_SESSION: &str = "workbench:restore_session";
pub(crate) const ACTION_WORKBENCH_SAVE_GRAPH: &str = "workbench:save_graph";
pub(crate) const ACTION_WORKBENCH_RESTORE_GRAPH: &str = "workbench:restore_graph";
pub(crate) const ACTION_WORKBENCH_OPEN_PERSISTENCE_HUB: &str = "workbench:open_persistence_hub";
pub(crate) const ACTION_WORKBENCH_OPEN_HISTORY_MANAGER: &str = "workbench:open_history_manager";
pub(crate) const ACTION_WORKBENCH_ACTIVATE_WORKFLOW: &str = "workbench:activate_workflow";

// Verse sync actions (Step 5.3)
pub(crate) const ACTION_VERSE_PAIR_DEVICE: &str = "verse:pair_device";
pub(crate) const ACTION_VERSE_SYNC_NOW: &str = "verse:sync_now";
pub(crate) const ACTION_VERSE_SHARE_WORKSPACE: &str = "verse:share_workspace";
pub(crate) const ACTION_VERSE_FORGET_DEVICE: &str = "verse:forget_device";

#[derive(Debug, Clone)]
pub(crate) enum ActionPayload {
    GraphNodeOpen {
        node_key: NodeKey,
        pane_id: Option<PaneId>,
    },
    GraphNodeClose {
        node_key: NodeKey,
    },
    GraphNodeNew,
    GraphNodeNewAsTab,
    GraphNodePinToggle,
    GraphNodePinSelected,
    GraphNodeUnpinSelected,
    GraphNodeDelete,
    GraphNodeChooseFrame,
    GraphNodeAddToFrame,
    GraphNodeAddConnectedToFrame,
    GraphNodeOpenFrame,
    GraphNodeOpenNeighbors,
    GraphNodeOpenConnected,
    GraphNodeOpenSplit,
    GraphNodeCopyUrl,
    GraphNodeCopyTitle,
    GraphEdgeCreate {
        from: NodeKey,
        to: NodeKey,
        label: Option<String>,
    },
    GraphEdgeConnectPair,
    GraphEdgeConnectBoth,
    GraphEdgeRemoveUser,
    GraphSetPhysicsProfile {
        profile_id: String,
    },
    GraphFit,
    GraphTogglePhysics,
    GraphNavigateBack,
    GraphNavigateForward,
    GraphSelectNode {
        node_key: NodeKey,
    },
    GraphDeselectAll,
    WorkbenchSplitHorizontal {
        pane_id: PaneId,
    },
    WorkbenchSplitVertical {
        pane_id: PaneId,
    },
    WorkbenchClosePane {
        pane_id: PaneId,
    },
    WorkbenchDetachToSplit,
    WorkbenchMoveNodeToPane {
        pane_id: PaneId,
    },
    WorkbenchCommandPaletteOpen,
    WorkbenchHelpOpen,
    WorkbenchSettingsPaneOpen,
    WorkbenchSettingsOverlayOpen,
    WorkbenchSettingsOpen,
    WorkbenchUndo,
    WorkbenchRedo,
    WorkbenchSaveSnapshot,
    WorkbenchRestoreSession {
        name: String,
    },
    WorkbenchSaveGraph {
        name: String,
    },
    WorkbenchRestoreGraph {
        name: Option<String>,
    },
    WorkbenchOpenPersistenceHub,
    WorkbenchOpenHistoryManager,
    WorkbenchActivateWorkflow {
        workflow_id: String,
    },
    OmniboxNodeSearch {
        query: String,
    },
    GraphViewSubmit {
        input: String,
    },
    DetailViewSubmit {
        normalized_url: String,
        focused_node: Option<NodeKey>,
    },
    // Verse sync actions (Step 5.3)
    VersePairDevice {
        mode: PairingMode,
    },
    VerseSyncNow,
    VerseShareWorkspace {
        workspace_id: String,
    },
    VerseForgetDevice {
        node_id: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum PairingMode {
    /// Show our pairing code for others to enter
    ShowCode,
    /// Enter someone else's code to pair with them
    EnterCode { code: String },
    /// Pair with a discovered mDNS peer
    LocalPeer { node_id: String },
}

type ActionHandler = fn(&GraphBrowserApp, &ActionPayload) -> ActionOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionCapability {
    AlwaysAvailable,
    RequiresActiveNode,
    RequiresSelection,
    RequiresWritableWorkspace,
}

#[derive(Clone)]
struct ActionDescriptor {
    id: String,
    required_capability: ActionCapability,
    handler: ActionHandler,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActionFailureKind {
    UnknownAction,
    InvalidPayload,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActionFailure {
    pub(crate) kind: ActionFailureKind,
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ActionDispatch {
    pub(crate) intents: Vec<GraphIntent>,
    pub(crate) workbench_intents: Vec<WorkbenchIntent>,
    pub(crate) app_commands: Vec<AppCommand>,
    pub(crate) runtime_actions: Vec<RuntimeAction>,
}

impl ActionDispatch {
    fn intents(intents: Vec<GraphIntent>) -> Self {
        Self {
            intents,
            workbench_intents: Vec::new(),
            app_commands: Vec::new(),
            runtime_actions: Vec::new(),
        }
    }

    fn workbench_intent(intent: WorkbenchIntent) -> Self {
        Self {
            intents: Vec::new(),
            workbench_intents: vec![intent],
            app_commands: Vec::new(),
            runtime_actions: Vec::new(),
        }
    }

    fn workbench_intents(intents: Vec<WorkbenchIntent>) -> Self {
        Self {
            intents: Vec::new(),
            workbench_intents: intents,
            app_commands: Vec::new(),
            runtime_actions: Vec::new(),
        }
    }

    fn app_commands(app_commands: Vec<AppCommand>) -> Self {
        Self {
            intents: Vec::new(),
            workbench_intents: Vec::new(),
            app_commands,
            runtime_actions: Vec::new(),
        }
    }

    fn runtime_action(action: RuntimeAction) -> Self {
        Self {
            intents: Vec::new(),
            workbench_intents: Vec::new(),
            app_commands: Vec::new(),
            runtime_actions: vec![action],
        }
    }

    fn dispatch_len(&self) -> usize {
        self.intents.len()
            + self.workbench_intents.len()
            + self.app_commands.len()
            + self.runtime_actions.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeAction {
    ActivateWorkflow { workflow_id: String },
    PublishSettingsRouteRequested { url: String, prefer_overlay: bool },
}

#[derive(Debug, Clone)]
pub(crate) enum ActionOutcome {
    Dispatch(ActionDispatch),
    Failure(ActionFailure),
}

impl ActionOutcome {
    pub(crate) fn succeeded(&self) -> bool {
        matches!(self, Self::Dispatch(_))
    }

    pub(crate) fn intent_len(&self) -> usize {
        match self {
            Self::Dispatch(dispatch) => dispatch.dispatch_len(),
            Self::Failure(_) => 0,
        }
    }

    pub(crate) fn into_intents(self) -> Vec<GraphIntent> {
        match self {
            Self::Dispatch(dispatch) => dispatch.intents,
            Self::Failure(_) => Vec::new(),
        }
    }

    pub(crate) fn into_workbench_intent(self) -> Option<WorkbenchIntent> {
        match self {
            Self::Dispatch(mut dispatch) => {
                if dispatch.workbench_intents.len() == 1 {
                    dispatch.workbench_intents.pop()
                } else {
                    None
                }
            }
            Self::Failure(_) => None,
        }
    }

    pub(crate) fn into_app_commands(self) -> Vec<AppCommand> {
        match self {
            Self::Dispatch(dispatch) => dispatch.app_commands,
            Self::Failure(_) => Vec::new(),
        }
    }

    pub(crate) fn into_runtime_actions(self) -> Vec<RuntimeAction> {
        match self {
            Self::Dispatch(dispatch) => dispatch.runtime_actions,
            Self::Failure(_) => Vec::new(),
        }
    }
}

pub(crate) struct ActionRegistry {
    handlers: HashMap<String, ActionDescriptor>,
}

impl ActionRegistry {
    pub(crate) fn register(
        &mut self,
        action_id: &str,
        required_capability: ActionCapability,
        handler: ActionHandler,
    ) {
        if !is_namespaced_action_id(action_id) {
            log::warn!(
                "action_registry: key {:?} does not follow namespace:name format",
                action_id
            );
        }
        self.handlers.insert(
            action_id.to_ascii_lowercase(),
            ActionDescriptor {
                id: action_id.to_ascii_lowercase(),
                required_capability,
                handler,
            },
        );
    }

    pub(crate) fn describe_action(&self, action_id: &str) -> Option<ActionCapability> {
        self.handlers
            .get(&action_id.to_ascii_lowercase())
            .map(|descriptor| descriptor.required_capability)
    }

    pub(crate) fn unregister(&mut self, action_id: &str) -> bool {
        self.handlers
            .remove(&action_id.to_ascii_lowercase())
            .is_some()
    }

    pub(crate) fn execute(
        &self,
        action_id: &str,
        app: &GraphBrowserApp,
        payload: ActionPayload,
    ) -> ActionOutcome {
        let normalized_action_id = action_id.to_ascii_lowercase();
        if let Some(descriptor) = self.handlers.get(&normalized_action_id) {
            if !capability_available(app, descriptor.required_capability) {
                return ActionOutcome::Failure(ActionFailure {
                    kind: ActionFailureKind::Rejected,
                    reason: format!(
                        "action '{}' unavailable: {}",
                        descriptor.id,
                        capability_reason(descriptor.required_capability)
                    ),
                });
            }
            return (descriptor.handler)(app, &payload);
        }

        ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::UnknownAction,
            reason: format!("unknown action: {normalized_action_id}"),
        })
    }

    /// Resolve the handler fn pointer + capability without executing it.
    /// Returns `None` if the action id is unknown.
    /// Use this to execute an action without holding any external lock — call
    /// `resolve`, drop whatever lock gated access to this registry, then call
    /// the returned fn pointer.
    pub(crate) fn resolve(
        &self,
        action_id: &str,
    ) -> Option<(ActionHandler, ActionCapability, String)> {
        let normalized = action_id.to_ascii_lowercase();
        self.handlers
            .get(&normalized)
            .map(|d| (d.handler, d.required_capability, d.id.clone()))
    }
}

fn is_namespaced_action_id(action_id: &str) -> bool {
    let mut segments = action_id.split(':');
    let Some(namespace) = segments.next() else {
        return false;
    };
    let Some(name) = segments.next() else {
        return false;
    };

    !namespace.is_empty() && !name.is_empty() && segments.next().is_none()
}

impl Default for ActionRegistry {
    fn default() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };
        registry.register(
            ACTION_GRAPH_NODE_OPEN,
            ActionCapability::AlwaysAvailable,
            execute_graph_node_open_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_CLOSE,
            ActionCapability::AlwaysAvailable,
            execute_graph_node_close_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_NEW,
            ActionCapability::RequiresWritableWorkspace,
            execute_graph_node_new_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_NEW_AS_TAB,
            ActionCapability::RequiresWritableWorkspace,
            execute_graph_node_new_as_tab_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_PIN_TOGGLE,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_pin_toggle_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_PIN_SELECTED,
            ActionCapability::RequiresSelection,
            execute_graph_node_pin_selected_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_UNPIN_SELECTED,
            ActionCapability::RequiresSelection,
            execute_graph_node_unpin_selected_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_DELETE,
            ActionCapability::RequiresSelection,
            execute_graph_node_delete_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_CHOOSE_FRAME,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_choose_frame_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_ADD_TO_FRAME,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_add_to_frame_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_ADD_CONNECTED_TO_FRAME,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_add_connected_to_frame_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_OPEN_FRAME,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_open_frame_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_OPEN_NEIGHBORS,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_open_neighbors_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_OPEN_CONNECTED,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_open_connected_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_OPEN_SPLIT,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_open_split_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_COPY_URL,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_copy_url_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_COPY_TITLE,
            ActionCapability::RequiresActiveNode,
            execute_graph_node_copy_title_action,
        );
        registry.register(
            ACTION_GRAPH_EDGE_CREATE,
            ActionCapability::RequiresWritableWorkspace,
            execute_graph_edge_create_action,
        );
        registry.register(
            ACTION_GRAPH_EDGE_CONNECT_PAIR,
            ActionCapability::RequiresSelection,
            execute_graph_edge_connect_pair_action,
        );
        registry.register(
            ACTION_GRAPH_EDGE_CONNECT_BOTH,
            ActionCapability::RequiresSelection,
            execute_graph_edge_connect_both_action,
        );
        registry.register(
            ACTION_GRAPH_EDGE_REMOVE_USER,
            ActionCapability::RequiresSelection,
            execute_graph_edge_remove_user_action,
        );
        registry.register(
            ACTION_GRAPH_SET_PHYSICS_PROFILE,
            ActionCapability::RequiresWritableWorkspace,
            execute_graph_set_physics_profile_action,
        );
        registry.register(
            ACTION_GRAPH_FIT,
            ActionCapability::AlwaysAvailable,
            execute_graph_fit_action,
        );
        registry.register(
            ACTION_GRAPH_TOGGLE_PHYSICS,
            ActionCapability::RequiresWritableWorkspace,
            execute_graph_toggle_physics_action,
        );
        registry.register(
            ACTION_GRAPH_NAVIGATE_BACK,
            ActionCapability::AlwaysAvailable,
            execute_graph_navigate_back_action,
        );
        registry.register(
            ACTION_GRAPH_NAVIGATE_FORWARD,
            ActionCapability::AlwaysAvailable,
            execute_graph_navigate_forward_action,
        );
        registry.register(
            ACTION_GRAPH_SELECT_NODE,
            ActionCapability::AlwaysAvailable,
            execute_graph_select_node_action,
        );
        registry.register(
            ACTION_GRAPH_DESELECT_ALL,
            ActionCapability::RequiresSelection,
            execute_graph_deselect_all_action,
        );
        registry.register(
            ACTION_WORKBENCH_SPLIT_HORIZONTAL,
            ActionCapability::AlwaysAvailable,
            execute_workbench_split_horizontal_action,
        );
        registry.register(
            ACTION_WORKBENCH_DETACH_TO_SPLIT,
            ActionCapability::RequiresActiveNode,
            execute_workbench_detach_to_split_action,
        );
        registry.register(
            ACTION_WORKBENCH_MOVE_NODE_TO_PANE,
            ActionCapability::RequiresActiveNode,
            execute_workbench_move_node_to_pane_action,
        );
        registry.register(
            ACTION_WORKBENCH_SPLIT_VERTICAL,
            ActionCapability::AlwaysAvailable,
            execute_workbench_split_vertical_action,
        );
        registry.register(
            ACTION_WORKBENCH_CLOSE_PANE,
            ActionCapability::AlwaysAvailable,
            execute_workbench_close_pane_action,
        );
        registry.register(
            ACTION_WORKBENCH_COMMAND_PALETTE_OPEN,
            ActionCapability::AlwaysAvailable,
            execute_workbench_command_palette_open_action,
        );
        registry.register(
            ACTION_WORKBENCH_HELP_OPEN,
            ActionCapability::AlwaysAvailable,
            execute_workbench_help_open_action,
        );
        registry.register(
            ACTION_WORKBENCH_SETTINGS_PANE_OPEN,
            ActionCapability::AlwaysAvailable,
            execute_workbench_settings_pane_open_action,
        );
        registry.register(
            ACTION_WORKBENCH_SETTINGS_OVERLAY_OPEN,
            ActionCapability::AlwaysAvailable,
            execute_workbench_settings_overlay_open_action,
        );
        registry.register(
            ACTION_WORKBENCH_SETTINGS_OPEN,
            ActionCapability::AlwaysAvailable,
            execute_workbench_settings_open_action,
        );
        registry.register(
            ACTION_WORKBENCH_UNDO,
            ActionCapability::AlwaysAvailable,
            execute_workbench_undo_action,
        );
        registry.register(
            ACTION_WORKBENCH_REDO,
            ActionCapability::AlwaysAvailable,
            execute_workbench_redo_action,
        );
        registry.register(
            ACTION_WORKBENCH_SAVE_SNAPSHOT,
            ActionCapability::RequiresWritableWorkspace,
            execute_workbench_save_snapshot_action,
        );
        registry.register(
            ACTION_WORKBENCH_RESTORE_SESSION,
            ActionCapability::AlwaysAvailable,
            execute_workbench_restore_session_action,
        );
        registry.register(
            ACTION_WORKBENCH_SAVE_GRAPH,
            ActionCapability::RequiresWritableWorkspace,
            execute_workbench_save_graph_action,
        );
        registry.register(
            ACTION_WORKBENCH_RESTORE_GRAPH,
            ActionCapability::AlwaysAvailable,
            execute_workbench_restore_graph_action,
        );
        registry.register(
            ACTION_WORKBENCH_OPEN_PERSISTENCE_HUB,
            ActionCapability::AlwaysAvailable,
            execute_workbench_open_persistence_hub_action,
        );
        registry.register(
            ACTION_WORKBENCH_OPEN_HISTORY_MANAGER,
            ActionCapability::AlwaysAvailable,
            execute_workbench_open_history_manager_action,
        );
        registry.register(
            ACTION_WORKBENCH_ACTIVATE_WORKFLOW,
            ActionCapability::AlwaysAvailable,
            execute_workbench_activate_workflow_action,
        );
        registry.register(
            ACTION_DETAIL_VIEW_SUBMIT,
            ActionCapability::RequiresWritableWorkspace,
            execute_detail_view_submit_action,
        );
        registry.register(
            ACTION_GRAPH_VIEW_SUBMIT,
            ActionCapability::RequiresWritableWorkspace,
            execute_graph_view_submit_action,
        );
        registry.register(
            ACTION_OMNIBOX_NODE_SEARCH,
            ActionCapability::AlwaysAvailable,
            execute_omnibox_node_search_action,
        );

        // Verse sync actions (Step 5.3)
        registry.register(
            ACTION_VERSE_PAIR_DEVICE,
            ActionCapability::AlwaysAvailable,
            execute_verse_pair_device_action,
        );
        registry.register(
            ACTION_VERSE_SYNC_NOW,
            ActionCapability::AlwaysAvailable,
            execute_verse_sync_now_action,
        );
        registry.register(
            ACTION_VERSE_SHARE_WORKSPACE,
            ActionCapability::RequiresWritableWorkspace,
            execute_verse_share_workspace_action,
        );
        registry.register(
            ACTION_VERSE_FORGET_DEVICE,
            ActionCapability::AlwaysAvailable,
            execute_verse_forget_device_action,
        );
        registry.register(
            ACTION_GRAPH_SELECTION_WARM_SELECT,
            ActionCapability::RequiresSelection,
            execute_graph_selection_warm_select_action,
        );
        registry.register(
            ACTION_GRAPH_NODE_REMOVE_FROM_GRAPHLET,
            ActionCapability::RequiresSelection,
            execute_graph_node_remove_from_graphlet_action,
        );

        registry
    }
}

pub(super) fn capability_available(app: &GraphBrowserApp, capability: ActionCapability) -> bool {
    match capability {
        ActionCapability::AlwaysAvailable => true,
        ActionCapability::RequiresActiveNode => app.get_single_selected_node().is_some(),
        ActionCapability::RequiresSelection => !app.focused_selection().is_empty(),
        // No explicit read-only workspace mode exists yet, so writable capability currently
        // gates intent shape and future UI affordances rather than a persisted lock bit.
        ActionCapability::RequiresWritableWorkspace => true,
    }
}

pub(super) fn capability_reason(capability: ActionCapability) -> &'static str {
    match capability {
        ActionCapability::AlwaysAvailable => "always available",
        ActionCapability::RequiresActiveNode => "requires an active node",
        ActionCapability::RequiresSelection => "requires a non-empty selection",
        ActionCapability::RequiresWritableWorkspace => "requires a writable workspace",
    }
}

fn execute_graph_node_open_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeOpen {
        node_key,
        pane_id: _,
    } = payload
    else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_open requires GraphNodeOpen payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::SelectNode {
            key: *node_key,
            multi_select: false,
        },
        RuntimeEvent::PromoteNodeToActive {
            key: *node_key,
            cause: LifecycleCause::UserSelect,
        }
        .into(),
    ]))
}

fn execute_graph_node_close_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeClose { node_key } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_close requires GraphNodeClose payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        RuntimeEvent::DemoteNodeToCold {
            key: *node_key,
            cause: LifecycleCause::ExplicitClose,
        }
        .into(),
    ]))
}

fn execute_graph_selection_warm_select_action(
    app: &GraphBrowserApp,
    _payload: &ActionPayload,
) -> ActionOutcome {
    use crate::graph::NodeLifecycle;
    use crate::shell::desktop::workbench::pane_model::PaneId;

    // Collect cold nodes from the current graph selection.
    let cold_nodes: Vec<NodeKey> = app
        .focused_selection()
        .iter()
        .copied()
        .filter(|&key| {
            app.domain_graph()
                .get_node(key)
                .is_some_and(|n| n.lifecycle == NodeLifecycle::Cold)
        })
        .collect();

    if cold_nodes.is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "graph:selection_warm_select requires at least one cold node in the selection"
                .to_string(),
        });
    }

    // For each cold node, dispatch OpenNodeInPane with a fresh (non-existent) pane ID so
    // that the graphlet-aware handler routes the node into its warm peer's tab group
    // (step 2 of handle_open_node_in_pane_intent), or creates a standalone tile as fallback.
    let workbench_intents = cold_nodes
        .into_iter()
        .map(|node| WorkbenchIntent::OpenNodeInPane {
            node,
            pane: PaneId::new(),
        })
        .collect();

    ActionOutcome::Dispatch(ActionDispatch::workbench_intents(workbench_intents))
}

/// Retract all durable edges (`UserGrouped` + `FrameMember`) that connect the
/// selected node to its graphlet.  Emits one `GraphIntent::RemoveEdge` per edge.
/// Lifecycle is not changed; circumstantial edges (Hyperlink, History) are left intact.
fn execute_graph_node_remove_from_graphlet_action(
    app: &GraphBrowserApp,
    _payload: &ActionPayload,
) -> ActionOutcome {
    use crate::graph::{ArrangementSubKind, RelationSelector, SemanticSubKind};
    use petgraph::visit::{EdgeRef, IntoEdgeReferences};

    let Some(&node_key) = app.focused_selection().iter().next() else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "graph:node_remove_from_graphlet requires a selected node".to_string(),
        });
    };

    let edges_to_retract: Vec<(NodeKey, NodeKey, crate::graph::RelationSelector)> = app
        .domain_graph()
        .inner
        .edge_references()
        .flat_map(|edge| {
            if edge.source() != node_key && edge.target() != node_key {
                return Vec::new();
            }

            let mut selectors = Vec::new();
            if edge
                .weight()
                .has_relation(RelationSelector::Semantic(SemanticSubKind::UserGrouped))
            {
                selectors.push((
                    edge.source(),
                    edge.target(),
                    RelationSelector::Semantic(SemanticSubKind::UserGrouped),
                ));
            }
            if edge.weight().has_relation(RelationSelector::Arrangement(
                ArrangementSubKind::FrameMember,
            )) {
                selectors.push((
                    edge.source(),
                    edge.target(),
                    RelationSelector::Arrangement(ArrangementSubKind::FrameMember),
                ));
            }
            selectors
        })
        .collect();

    if edges_to_retract.is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "node has no durable graphlet edges to retract".to_string(),
        });
    }

    let intents = edges_to_retract
        .into_iter()
        .map(|(from, to, selector)| GraphIntent::RemoveEdge { from, to, selector })
        .collect();

    ActionOutcome::Dispatch(ActionDispatch {
        intents,
        workbench_intents: Vec::new(),
        app_commands: Vec::new(),
        runtime_actions: Vec::new(),
    })
}

fn require_active_node(app: &GraphBrowserApp, action_id: &str) -> Result<NodeKey, ActionOutcome> {
    app.get_single_selected_node().ok_or_else(|| {
        ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: format!("action '{action_id}' requires exactly one active node"),
        })
    })
}

fn require_named_payload(
    name: &str,
    value: &str,
    action_id: &str,
) -> Result<String, ActionOutcome> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: format!("action '{action_id}' requires non-empty {name}"),
        }));
    }
    Ok(trimmed.to_string())
}

fn execute_graph_node_new_action(_app: &GraphBrowserApp, payload: &ActionPayload) -> ActionOutcome {
    let ActionPayload::GraphNodeNew = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_new requires GraphNodeNew payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::CreateNodeNearCenter,
    ]))
}

fn execute_graph_node_new_as_tab_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeNewAsTab = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_new_as_tab requires GraphNodeNewAsTab payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::CreateNodeNearCenterAndOpen {
            mode: PendingTileOpenMode::Tab,
        },
    ]))
}

fn execute_graph_node_pin_toggle_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodePinToggle = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_pin_toggle requires GraphNodePinToggle payload".to_string(),
        });
    };
    if let Err(outcome) = require_active_node(app, ACTION_GRAPH_NODE_PIN_TOGGLE) {
        return outcome;
    }

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::TogglePrimaryNodePin,
    ]))
}

fn execute_graph_node_pin_selected_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodePinSelected = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_pin_selected requires GraphNodePinSelected payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::ExecuteEdgeCommand {
            command: crate::app::EdgeCommand::PinSelected,
        },
    ]))
}

fn execute_graph_node_unpin_selected_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeUnpinSelected = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_unpin_selected requires GraphNodeUnpinSelected payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::ExecuteEdgeCommand {
            command: crate::app::EdgeCommand::UnpinSelected,
        },
    ]))
}

fn execute_graph_node_delete_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeDelete = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_delete requires GraphNodeDelete payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::RemoveSelectedNodes,
    ]))
}

fn execute_graph_node_choose_frame_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeChooseFrame = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_choose_frame requires GraphNodeChooseFrame payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_CHOOSE_FRAME) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::ChooseWorkspacePicker {
            request: crate::app::ChooseFramePickerRequest {
                node: node_key,
                mode: ChooseFramePickerMode::OpenNodeInFrame,
            },
            exact_nodes: None,
        },
    ]))
}

fn execute_graph_node_add_to_frame_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeAddToFrame = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_add_to_frame requires GraphNodeAddToFrame payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_ADD_TO_FRAME) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::ChooseWorkspacePicker {
            request: crate::app::ChooseFramePickerRequest {
                node: node_key,
                mode: ChooseFramePickerMode::AddNodeToFrame,
            },
            exact_nodes: None,
        },
    ]))
}

fn execute_graph_node_add_connected_to_frame_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeAddConnectedToFrame = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason:
                "graph:node_add_connected_to_frame requires GraphNodeAddConnectedToFrame payload"
                    .to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_ADD_CONNECTED_TO_FRAME) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::ChooseWorkspacePicker {
            request: crate::app::ChooseFramePickerRequest {
                node: node_key,
                mode: ChooseFramePickerMode::AddConnectedSelectionToFrame,
            },
            exact_nodes: None,
        },
    ]))
}

fn execute_graph_node_open_frame_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeOpenFrame = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_open_frame requires GraphNodeOpenFrame payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_OPEN_FRAME) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::OpenNodeFrameRouted {
            key: node_key,
            prefer_frame: None,
        },
    ]))
}

fn execute_graph_node_open_neighbors_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeOpenNeighbors = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_open_neighbors requires GraphNodeOpenNeighbors payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_OPEN_NEIGHBORS) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::OpenConnected {
            source: node_key,
            mode: PendingTileOpenMode::Tab,
            scope: PendingConnectedOpenScope::Neighbors,
        },
    ]))
}

fn execute_graph_node_open_connected_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeOpenConnected = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_open_connected requires GraphNodeOpenConnected payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_OPEN_CONNECTED) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::OpenConnected {
            source: node_key,
            mode: PendingTileOpenMode::Tab,
            scope: PendingConnectedOpenScope::Connected,
        },
    ]))
}

fn execute_graph_node_open_split_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeOpenSplit = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_open_split requires GraphNodeOpenSplit payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_OPEN_SPLIT) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![AppCommand::OpenNode {
        request: PendingNodeOpenRequest {
            key: node_key,
            mode: PendingTileOpenMode::SplitHorizontal,
        },
    }]))
}

fn execute_graph_node_copy_url_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeCopyUrl = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_copy_url requires GraphNodeCopyUrl payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_COPY_URL) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::ClipboardCopy {
            request: ClipboardCopyRequest {
                key: node_key,
                kind: ClipboardCopyKind::Url,
            },
        },
    ]))
}

fn execute_graph_node_copy_title_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNodeCopyTitle = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:node_copy_title requires GraphNodeCopyTitle payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_GRAPH_NODE_COPY_TITLE) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::ClipboardCopy {
            request: ClipboardCopyRequest {
                key: node_key,
                kind: ClipboardCopyKind::Title,
            },
        },
    ]))
}

fn execute_graph_edge_create_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphEdgeCreate { from, to, label } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:edge_create requires GraphEdgeCreate payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphMutation::CreateUserGroupedEdge {
            from: *from,
            to: *to,
            label: label.clone(),
        }
        .into(),
    ]))
}

fn execute_graph_edge_connect_pair_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphEdgeConnectPair = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:edge_connect_pair requires GraphEdgeConnectPair payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::ExecuteEdgeCommand {
            command: crate::app::EdgeCommand::ConnectSelectedPair,
        },
    ]))
}

fn execute_graph_edge_connect_both_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphEdgeConnectBoth = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:edge_connect_both requires GraphEdgeConnectBoth payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::ExecuteEdgeCommand {
            command: crate::app::EdgeCommand::ConnectBothDirections,
        },
    ]))
}

fn execute_graph_edge_remove_user_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphEdgeRemoveUser = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:edge_remove_user requires GraphEdgeRemoveUser payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::ExecuteEdgeCommand {
            command: crate::app::EdgeCommand::RemoveUserEdge,
        },
    ]))
}

fn execute_graph_set_physics_profile_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphSetPhysicsProfile { profile_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:set_physics_profile requires GraphSetPhysicsProfile payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::SetPhysicsProfile {
            profile_id: profile_id.clone(),
        },
    ]))
}

fn execute_graph_fit_action(_app: &GraphBrowserApp, payload: &ActionPayload) -> ActionOutcome {
    let ActionPayload::GraphFit = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:fit requires GraphFit payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::RequestFitToScreen,
    ]))
}

fn execute_graph_toggle_physics_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphTogglePhysics = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:toggle_physics requires GraphTogglePhysics payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::TogglePhysics]))
}

fn execute_graph_navigate_back_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNavigateBack = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:navigate_back requires GraphNavigateBack payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::TraverseBack]))
}

fn execute_graph_navigate_forward_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphNavigateForward = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:navigate_forward requires GraphNavigateForward payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::TraverseForward]))
}

fn execute_graph_select_node_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphSelectNode { node_key } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:select_node requires GraphSelectNode payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::SelectNode {
        key: *node_key,
        multi_select: false,
    }]))
}

fn execute_graph_deselect_all_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphDeselectAll = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:deselect_all requires GraphDeselectAll payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphIntent::UpdateSelection {
            keys: Vec::new(),
            mode: SelectionUpdateMode::Replace,
        },
    ]))
}

fn execute_workbench_detach_to_split_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchDetachToSplit = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:detach_to_split requires WorkbenchDetachToSplit payload".to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_WORKBENCH_DETACH_TO_SPLIT) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::workbench_intent(
        WorkbenchIntent::DetachNodeToSplit { key: node_key },
    ))
}

fn execute_workbench_move_node_to_pane_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchMoveNodeToPane { pane_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:move_node_to_pane requires WorkbenchMoveNodeToPane payload"
                .to_string(),
        });
    };
    let node_key = match require_active_node(app, ACTION_WORKBENCH_MOVE_NODE_TO_PANE) {
        Ok(node_key) => node_key,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::workbench_intent(
        WorkbenchIntent::OpenNodeInPane {
            node: node_key,
            pane: *pane_id,
        },
    ))
}

fn execute_workbench_split_horizontal_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchSplitHorizontal { pane_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:split_horizontal requires WorkbenchSplitHorizontal payload"
                .to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::workbench_intent(
        WorkbenchIntent::SplitPane {
            source_pane: *pane_id,
            direction: SplitDirection::Horizontal,
        },
    ))
}

fn execute_workbench_split_vertical_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchSplitVertical { pane_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:split_vertical requires WorkbenchSplitVertical payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::workbench_intent(
        WorkbenchIntent::SplitPane {
            source_pane: *pane_id,
            direction: SplitDirection::Vertical,
        },
    ))
}

fn execute_workbench_close_pane_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchClosePane { pane_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:close_pane requires WorkbenchClosePane payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::workbench_intent(
        WorkbenchIntent::ClosePane {
            pane: *pane_id,
            restore_previous_focus: true,
        },
    ))
}

fn execute_workbench_command_palette_open_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchCommandPaletteOpen = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:command_palette_open requires WorkbenchCommandPaletteOpen payload"
                .to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::workbench_intent(
        WorkbenchIntent::OpenCommandPalette,
    ))
}

fn execute_workbench_help_open_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchHelpOpen = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:help_open requires WorkbenchHelpOpen payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::ToggleHelpPanel]))
}

fn execute_workbench_settings_pane_open_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchSettingsPaneOpen = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:settings_pane_open requires WorkbenchSettingsPaneOpen payload"
                .to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::workbench_intent(
        WorkbenchIntent::OpenToolPane {
            kind: ToolPaneState::Settings,
        },
    ))
}

fn execute_workbench_settings_overlay_open_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchSettingsOverlayOpen = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:settings_overlay_open requires WorkbenchSettingsOverlayOpen payload"
                .to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::runtime_action(
        RuntimeAction::PublishSettingsRouteRequested {
            url: VersoAddress::settings(GraphshellSettingsPath::General).to_string(),
            prefer_overlay: true,
        },
    ))
}

fn execute_workbench_settings_open_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchSettingsOpen = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:settings_open requires WorkbenchSettingsOpen payload".to_string(),
        });
    };

    // Keep the legacy action id mapped to the hosted-pane behavior until all
    // external bundles and docs are updated to the explicit pane/overlay split.
    execute_workbench_settings_pane_open_action(app, &ActionPayload::WorkbenchSettingsPaneOpen)
}

fn execute_workbench_undo_action(_app: &GraphBrowserApp, payload: &ActionPayload) -> ActionOutcome {
    let ActionPayload::WorkbenchUndo = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:undo requires WorkbenchUndo payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::Undo]))
}

fn execute_workbench_redo_action(_app: &GraphBrowserApp, payload: &ActionPayload) -> ActionOutcome {
    let ActionPayload::WorkbenchRedo = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:redo requires WorkbenchRedo payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::Redo]))
}

fn execute_workbench_save_snapshot_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchSaveSnapshot = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:save_snapshot requires WorkbenchSaveSnapshot payload".to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::SaveWorkspaceSnapshot,
    ]))
}

fn execute_workbench_restore_session_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchRestoreSession { name } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:restore_session requires WorkbenchRestoreSession payload"
                .to_string(),
        });
    };
    let name = match require_named_payload("session name", name, ACTION_WORKBENCH_RESTORE_SESSION) {
        Ok(name) => name,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::RestoreWorkspaceSnapshotNamed { name },
    ]))
}

fn execute_workbench_save_graph_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchSaveGraph { name } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:save_graph requires WorkbenchSaveGraph payload".to_string(),
        });
    };
    let name = match require_named_payload("graph name", name, ACTION_WORKBENCH_SAVE_GRAPH) {
        Ok(name) => name,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![
        AppCommand::SaveGraphSnapshotNamed { name },
    ]))
}

fn execute_workbench_restore_graph_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchRestoreGraph { name } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:restore_graph requires WorkbenchRestoreGraph payload".to_string(),
        });
    };

    let command = match name {
        Some(name) => {
            let name =
                match require_named_payload("graph name", name, ACTION_WORKBENCH_RESTORE_GRAPH) {
                    Ok(name) => name,
                    Err(outcome) => return outcome,
                };
            AppCommand::RestoreGraphSnapshotNamed { name }
        }
        None => AppCommand::RestoreGraphSnapshotLatest,
    };

    ActionOutcome::Dispatch(ActionDispatch::app_commands(vec![command]))
}

fn execute_workbench_open_persistence_hub_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchOpenPersistenceHub = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:open_persistence_hub requires WorkbenchOpenPersistenceHub payload"
                .to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::runtime_action(
        RuntimeAction::PublishSettingsRouteRequested {
            url: VersoAddress::settings(GraphshellSettingsPath::Persistence).to_string(),
            prefer_overlay: true,
        },
    ))
}

fn execute_workbench_open_history_manager_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchOpenHistoryManager = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:open_history_manager requires WorkbenchOpenHistoryManager payload"
                .to_string(),
        });
    };

    ActionOutcome::Dispatch(ActionDispatch::workbench_intent(
        WorkbenchIntent::OpenToolPane {
            kind: ToolPaneState::HistoryManager,
        },
    ))
}

fn execute_workbench_activate_workflow_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchActivateWorkflow { workflow_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:activate_workflow requires WorkbenchActivateWorkflow payload"
                .to_string(),
        });
    };
    let workflow_id = match require_named_payload(
        "workflow id",
        workflow_id,
        ACTION_WORKBENCH_ACTIVATE_WORKFLOW,
    ) {
        Ok(workflow_id) => workflow_id,
        Err(outcome) => return outcome,
    };

    ActionOutcome::Dispatch(ActionDispatch::runtime_action(
        RuntimeAction::ActivateWorkflow { workflow_id },
    ))
}

fn execute_graph_view_submit_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphViewSubmit { input } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph_view_submit requires GraphViewSubmit payload".to_string(),
        });
    };

    let input = input.trim();
    if input.is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "graph_view_submit rejected empty input".to_string(),
        });
    }

    if let Some(selected_node) = app.get_single_selected_node() {
        ActionOutcome::Dispatch(ActionDispatch::intents(vec![
            GraphMutation::SetNodeUrl {
                key: selected_node,
                new_url: input.to_string(),
            }
            .into(),
        ]))
    } else {
        let position = app.suggested_new_node_position(app.focused_selection().primary());
        ActionOutcome::Dispatch(ActionDispatch::intents(vec![
            GraphMutation::CreateNodeAtUrl {
                url: input.to_string(),
                position,
            }
            .into(),
        ]))
    }
}

fn execute_detail_view_submit_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::DetailViewSubmit {
        normalized_url,
        focused_node,
    } = payload
    else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "detail_view_submit requires DetailViewSubmit payload".to_string(),
        });
    };

    if normalized_url.trim().is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "detail_view_submit rejected empty url".to_string(),
        });
    }

    if let Some(node_key) = focused_node {
        return ActionOutcome::Dispatch(ActionDispatch::intents(vec![
            GraphMutation::SetNodeUrl {
                key: *node_key,
                new_url: normalized_url.clone(),
            }
            .into(),
            RuntimeEvent::PromoteNodeToActive {
                key: *node_key,
                cause: LifecycleCause::Restore,
            }
            .into(),
        ]));
    }

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphMutation::CreateNodeAtUrl {
            url: normalized_url.clone(),
            position: app.suggested_new_node_position(app.focused_selection().primary()),
        }
        .into(),
    ]))
}

fn execute_omnibox_node_search_action(
    app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::OmniboxNodeSearch { query } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "omnibox_node_search requires OmniboxNodeSearch payload".to_string(),
        });
    };

    let results = super::phase3_index_search(app, query, 10);
    if let Some(result) = results.first() {
        return match &result.kind {
            SearchResultKind::Node(key) => {
                ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::SelectNode {
                    key: *key,
                    multi_select: false,
                }]))
            }
            SearchResultKind::HistoryUrl(url) => {
                ActionOutcome::Dispatch(ActionDispatch::intents(vec![
                    GraphMutation::CreateNodeAtUrl {
                        url: url.clone(),
                        position: app
                            .suggested_new_node_position(app.focused_selection().primary()),
                    }
                    .into(),
                ]))
            }
            SearchResultKind::KnowledgeTag { code } => ActionOutcome::Failure(ActionFailure {
                kind: ActionFailureKind::Rejected,
                reason: format!(
                    "omnibox_node_search matched knowledge tag 'udc:{code}' but no graph node yet"
                ),
            }),
        };
    }
    ActionOutcome::Failure(ActionFailure {
        kind: ActionFailureKind::Rejected,
        reason: format!("omnibox_node_search found no match for '{query}'"),
    })
}

// ===== Verse Sync Action Handlers (Step 5.3) =====

fn execute_verse_pair_device_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::VersePairDevice { mode } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "verse_pair_device requires VersePairDevice payload".to_string(),
        });
    };

    match mode {
        PairingMode::ShowCode => ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_pair_device show-code is handled by the UI surface".to_string(),
        }),
        PairingMode::EnterCode { code } => {
            match crate::mods::native::verse::decode_pairing_code(code) {
                Ok(node_id) => {
                    ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::TrustPeer {
                        peer_id: node_id.to_string(),
                        display_name: format!("Paired {}", &node_id.to_string()[..8]),
                    }]))
                }
                Err(error) => ActionOutcome::Failure(ActionFailure {
                    kind: ActionFailureKind::Rejected,
                    reason: format!("pairing code decode failed: {error}"),
                }),
            }
        }
        PairingMode::LocalPeer { node_id } => match node_id.parse::<iroh::EndpointId>() {
            Ok(parsed_node_id) => {
                ActionOutcome::Dispatch(ActionDispatch::intents(vec![GraphIntent::TrustPeer {
                    peer_id: parsed_node_id.to_string(),
                    display_name: format!("Local {}", &parsed_node_id.to_string()[..8]),
                }]))
            }
            Err(error) => ActionOutcome::Failure(ActionFailure {
                kind: ActionFailureKind::Rejected,
                reason: format!("invalid local peer id '{node_id}': {error}"),
            }),
        },
    }
}

fn execute_verse_sync_now_action(
    _app: &GraphBrowserApp,
    _payload: &ActionPayload,
) -> ActionOutcome {
    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        crate::app::RuntimeEvent::SyncNow.into(),
    ]))
}

fn execute_verse_share_workspace_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::VerseShareWorkspace { workspace_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "verse_share_workspace requires VerseShareWorkspace payload".to_string(),
        });
    };

    if workspace_id.trim().is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_share_workspace rejected empty workspace id".to_string(),
        });
    }

    let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
    if peers.is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_share_workspace has no trusted peers to share with".to_string(),
        });
    }

    ActionOutcome::Dispatch(ActionDispatch::intents(
        peers
            .into_iter()
            .map(|peer| GraphIntent::GrantWorkspaceAccess {
                peer_id: peer.node_id.to_string(),
                workspace_id: workspace_id.clone(),
            })
            .collect(),
    ))
}

fn execute_verse_forget_device_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::VerseForgetDevice { node_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "verse_forget_device requires VerseForgetDevice payload".to_string(),
        });
    };

    if node_id.trim().is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_forget_device rejected empty peer id".to_string(),
        });
    }

    ActionOutcome::Dispatch(ActionDispatch::intents(vec![
        GraphMutation::ForgetDevice {
            peer_id: node_id.clone(),
        }
        .into(),
    ]))
}

// ===== Core Action Implementations (original) =====

#[cfg(test)]
mod tests {
    use super::*;
    use euclid::default::Point2D;

    #[test]
    fn action_registry_executes_omnibox_node_search() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".into(), Point2D::new(0.0, 0.0));
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Example Handle".into();
        }

        let registry = ActionRegistry::default();
        let execution = registry.execute(
            ACTION_OMNIBOX_NODE_SEARCH,
            &app,
            ActionPayload::OmniboxNodeSearch {
                query: "example handle".to_string(),
            },
        );

        assert!(execution.succeeded());
        let intents = execution.into_intents();
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SelectNode { key: selected, .. }) if *selected == key
        ));
    }

    #[test]
    fn action_registry_omnibox_search_falls_back_to_history_provider() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".into(), Point2D::new(0.0, 0.0));
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Example".into();
            node.history_entries = vec!["https://history.example/rust".to_string()];
            node.history_index = 0;
        }

        let registry = ActionRegistry::default();
        let execution = registry.execute(
            ACTION_OMNIBOX_NODE_SEARCH,
            &app,
            ActionPayload::OmniboxNodeSearch {
                query: "history.example/rust".to_string(),
            },
        );

        assert!(execution.succeeded());
        let intents = execution.into_intents();
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::CreateNodeAtUrl { url, .. }) if url == "https://history.example/rust"
        ));
    }

    #[test]
    fn action_registry_returns_failed_outcome_for_unknown_action_id() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();
        let execution = registry.execute(
            "unknown:action",
            &app,
            ActionPayload::OmniboxNodeSearch {
                query: "payload".to_string(),
            },
        );

        assert!(matches!(
            execution,
            ActionOutcome::Failure(ActionFailure {
                kind: ActionFailureKind::UnknownAction,
                ..
            })
        ));
    }

    #[test]
    fn action_registry_executes_graph_view_submit_action() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        let registry = ActionRegistry::default();
        let execution = registry.execute(
            ACTION_GRAPH_VIEW_SUBMIT,
            &app,
            ActionPayload::GraphViewSubmit {
                input: "https://next.com".to_string(),
            },
        );

        assert!(execution.succeeded());
        let intents = execution.into_intents();
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://next.com"
        ));
    }

    #[test]
    fn action_registry_executes_detail_view_submit_action_for_focused_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));

        let registry = ActionRegistry::default();
        let execution = registry.execute(
            ACTION_DETAIL_VIEW_SUBMIT,
            &app,
            ActionPayload::DetailViewSubmit {
                normalized_url: "https://detail-next.com".to_string(),
                focused_node: Some(key),
            },
        );

        assert!(execution.succeeded());
        let intents = execution.into_intents();
        assert_eq!(intents.len(), 2);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SetNodeUrl { key: selected, new_url })
                if *selected == key && new_url == "https://detail-next.com"
        ));
        assert!(matches!(
            intents.get(1),
            Some(GraphIntent::PromoteNodeToActive { key: selected, cause })
                if *selected == key && *cause == LifecycleCause::Restore
        ));
    }

    #[test]
    fn action_registry_pair_local_peer_emits_trust_peer_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let peer_id = crate::mods::native::verse::generate_p2p_secret_key()
            .public()
            .to_string();
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_VERSE_PAIR_DEVICE,
            &app,
            ActionPayload::VersePairDevice {
                mode: PairingMode::LocalPeer {
                    node_id: peer_id.clone(),
                },
            },
        );

        let intents = execution.into_intents();
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::TrustPeer { peer_id: emitted, .. }) if emitted == &peer_id
        ));
    }

    #[test]
    fn action_registry_share_workspace_emits_grant_access_intents_for_trusted_peers() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_VERSE_SHARE_WORKSPACE,
            &app,
            ActionPayload::VerseShareWorkspace {
                workspace_id: "workspace:test".to_string(),
            },
        );

        match execution {
            ActionOutcome::Dispatch(dispatch) => {
                let intents = dispatch.intents;
                assert!(!intents.is_empty());
                assert!(intents.iter().all(|intent| {
                    matches!(
                        intent,
                        GraphIntent::GrantWorkspaceAccess { workspace_id, .. }
                            if workspace_id == "workspace:test"
                    )
                }));
            }
            ActionOutcome::Failure(ActionFailure { kind, reason }) => {
                assert_eq!(kind, ActionFailureKind::Rejected);
                assert!(reason.contains("no trusted peers"));
            }
        }
    }

    #[test]
    fn action_registry_graph_node_open_emits_select_and_activate() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://open-target.test".into(), Point2D::new(7.0, 7.0));
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_GRAPH_NODE_OPEN,
            &app,
            ActionPayload::GraphNodeOpen {
                node_key: key,
                pane_id: None,
            },
        );

        let intents = execution.into_intents();
        assert_eq!(intents.len(), 2);
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::SelectNode { key: selected, multi_select }) if *selected == key && !multi_select
        ));
        assert!(matches!(
            intents.get(1),
            Some(GraphIntent::PromoteNodeToActive { key: selected, cause })
                if *selected == key && *cause == LifecycleCause::UserSelect
        ));
    }

    #[test]
    fn action_registry_graph_node_close_emits_demote_intent() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://close-target.test".into(), Point2D::new(9.0, 9.0));
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_GRAPH_NODE_CLOSE,
            &app,
            ActionPayload::GraphNodeClose { node_key: key },
        );

        let intents = execution.into_intents();
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::DemoteNodeToCold { key: selected, cause })
                if *selected == key && *cause == LifecycleCause::ExplicitClose
        ));
    }

    #[test]
    fn action_registry_matrix_node_actions_route_existing_intents_and_app_commands() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://matrix-node.test".into(), Point2D::new(11.0, 13.0));
        app.select_node(key, false);
        let registry = ActionRegistry::default();

        let pin_toggle = registry.execute(
            ACTION_GRAPH_NODE_PIN_TOGGLE,
            &app,
            ActionPayload::GraphNodePinToggle,
        );
        assert!(matches!(
            pin_toggle.into_intents().first(),
            Some(GraphIntent::TogglePrimaryNodePin)
        ));

        let open_split = registry.execute(
            ACTION_GRAPH_NODE_OPEN_SPLIT,
            &app,
            ActionPayload::GraphNodeOpenSplit,
        );
        assert!(matches!(
            open_split.into_app_commands().first(),
            Some(AppCommand::OpenNode {
                request: PendingNodeOpenRequest { key: opened, mode: PendingTileOpenMode::SplitHorizontal }
            }) if *opened == key
        ));

        let copy_url = registry.execute(
            ACTION_GRAPH_NODE_COPY_URL,
            &app,
            ActionPayload::GraphNodeCopyUrl,
        );
        assert!(matches!(
            copy_url.into_app_commands().first(),
            Some(AppCommand::ClipboardCopy {
                request: ClipboardCopyRequest { key: copied, kind: ClipboardCopyKind::Url }
            }) if *copied == key
        ));
    }

    #[test]
    fn action_registry_graph_edge_create_emits_grouped_edge_intent() {
        let mut app = GraphBrowserApp::new_for_testing();
        let edge_from = app
            .workspace
            .domain
            .graph
            .add_node("https://edge-from.test".into(), Point2D::new(1.0, 1.0));
        let edge_to = app
            .workspace
            .domain
            .graph
            .add_node("https://edge-to.test".into(), Point2D::new(2.0, 2.0));
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_GRAPH_EDGE_CREATE,
            &app,
            ActionPayload::GraphEdgeCreate {
                from: edge_from,
                to: edge_to,
                label: None,
            },
        );

        let intents = execution.into_intents();
        assert!(matches!(
            intents.first(),
            Some(GraphIntent::CreateUserGroupedEdge {
                from,
                to,
                ..
            }) if *from == edge_from && *to == edge_to
        ));
    }

    #[test]
    fn action_registry_matrix_edge_actions_emit_existing_edge_commands() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app
            .workspace
            .domain
            .graph
            .add_node("https://edge-a.test".into(), Point2D::new(1.0, 1.0));
        let b = app
            .workspace
            .domain
            .graph
            .add_node("https://edge-b.test".into(), Point2D::new(2.0, 2.0));
        app.select_node(a, false);
        app.select_node(b, true);
        let registry = ActionRegistry::default();

        let connect_pair = registry.execute(
            ACTION_GRAPH_EDGE_CONNECT_PAIR,
            &app,
            ActionPayload::GraphEdgeConnectPair,
        );
        assert!(matches!(
            connect_pair.into_intents().first(),
            Some(GraphIntent::ExecuteEdgeCommand {
                command: crate::app::EdgeCommand::ConnectSelectedPair
            })
        ));

        let remove_user = registry.execute(
            ACTION_GRAPH_EDGE_REMOVE_USER,
            &app,
            ActionPayload::GraphEdgeRemoveUser,
        );
        assert!(matches!(
            remove_user.into_intents().first(),
            Some(GraphIntent::ExecuteEdgeCommand {
                command: crate::app::EdgeCommand::RemoveUserEdge
            })
        ));
    }

    #[test]
    fn action_registry_graph_set_physics_profile_emits_reducer_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_GRAPH_SET_PHYSICS_PROFILE,
            &app,
            ActionPayload::GraphSetPhysicsProfile {
                profile_id: crate::registries::atomic::lens::PHYSICS_ID_SCATTER.to_string(),
            },
        );

        assert!(matches!(
            execution.into_intents().first(),
            Some(GraphIntent::SetPhysicsProfile { profile_id })
                if profile_id == crate::registries::atomic::lens::PHYSICS_ID_SCATTER
        ));
    }

    #[test]
    fn action_registry_graph_navigate_handlers_emit_traversal_intents() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();

        let back = registry.execute(
            ACTION_GRAPH_NAVIGATE_BACK,
            &app,
            ActionPayload::GraphNavigateBack,
        );
        assert!(matches!(
            back.into_intents().first(),
            Some(GraphIntent::TraverseBack)
        ));

        let forward = registry.execute(
            ACTION_GRAPH_NAVIGATE_FORWARD,
            &app,
            ActionPayload::GraphNavigateForward,
        );
        assert!(matches!(
            forward.into_intents().first(),
            Some(GraphIntent::TraverseForward)
        ));
    }

    #[test]
    fn action_registry_graph_select_and_deselect_emit_selection_intents() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app
            .workspace
            .domain
            .graph
            .add_node("https://select-target.test".into(), Point2D::new(3.0, 3.0));
        let registry = ActionRegistry::default();

        let select = registry.execute(
            ACTION_GRAPH_SELECT_NODE,
            &app,
            ActionPayload::GraphSelectNode { node_key },
        );
        assert!(matches!(
            select.into_intents().first(),
            Some(GraphIntent::SelectNode { key, multi_select }) if *key == node_key && !multi_select
        ));

        // Apply the selection so the deselect capability guard passes.
        app.select_node(node_key, false);

        let deselect = registry.execute(
            ACTION_GRAPH_DESELECT_ALL,
            &app,
            ActionPayload::GraphDeselectAll,
        );
        assert!(matches!(
            deselect.into_intents().first(),
            Some(GraphIntent::UpdateSelection { keys, mode })
                if keys.is_empty() && *mode == SelectionUpdateMode::Replace
        ));
    }

    #[test]
    fn action_registry_workbench_split_horizontal_emits_workbench_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();
        let pane_id = PaneId::new();

        let execution = registry.execute(
            ACTION_WORKBENCH_SPLIT_HORIZONTAL,
            &app,
            ActionPayload::WorkbenchSplitHorizontal { pane_id },
        );

        assert!(matches!(
            execution.into_workbench_intent(),
            Some(WorkbenchIntent::SplitPane {
                source_pane,
                direction: SplitDirection::Horizontal,
            }) if source_pane == pane_id
        ));
    }

    #[test]
    fn action_registry_workbench_close_and_open_actions_emit_workbench_authority_intents() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();
        let pane_id = PaneId::new();

        let close = registry.execute(
            ACTION_WORKBENCH_CLOSE_PANE,
            &app,
            ActionPayload::WorkbenchClosePane { pane_id },
        );
        assert!(matches!(
            close.into_workbench_intent(),
            Some(WorkbenchIntent::ClosePane {
                pane,
                restore_previous_focus: true,
            }) if pane == pane_id
        ));

        let command_palette = registry.execute(
            ACTION_WORKBENCH_COMMAND_PALETTE_OPEN,
            &app,
            ActionPayload::WorkbenchCommandPaletteOpen,
        );
        assert!(matches!(
            command_palette.into_workbench_intent(),
            Some(WorkbenchIntent::OpenCommandPalette)
        ));

        let settings = registry.execute(
            ACTION_WORKBENCH_SETTINGS_PANE_OPEN,
            &app,
            ActionPayload::WorkbenchSettingsPaneOpen,
        );
        assert!(matches!(
            settings.into_workbench_intent(),
            Some(WorkbenchIntent::OpenToolPane {
                kind: ToolPaneState::Settings
            })
        ));

        let settings_overlay = registry.execute(
            ACTION_WORKBENCH_SETTINGS_OVERLAY_OPEN,
            &app,
            ActionPayload::WorkbenchSettingsOverlayOpen,
        );
        assert!(matches!(
            settings_overlay.into_runtime_actions().as_slice(),
            [RuntimeAction::PublishSettingsRouteRequested { url, prefer_overlay }]
                if url == &VersoAddress::settings(GraphshellSettingsPath::General).to_string()
                    && *prefer_overlay
        ));

        let settings_legacy = registry.execute(
            ACTION_WORKBENCH_SETTINGS_OPEN,
            &app,
            ActionPayload::WorkbenchSettingsOpen,
        );
        assert!(matches!(
            settings_legacy.into_workbench_intent(),
            Some(WorkbenchIntent::OpenToolPane {
                kind: ToolPaneState::Settings
            })
        ));
    }

    #[test]
    fn action_registry_matrix_workbench_actions_emit_existing_dispatch_paths() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://workbench-move.test".into(), Point2D::new(4.0, 6.0));
        app.select_node(key, false);
        let registry = ActionRegistry::default();
        let pane_id = PaneId::new();

        let detach = registry.execute(
            ACTION_WORKBENCH_DETACH_TO_SPLIT,
            &app,
            ActionPayload::WorkbenchDetachToSplit,
        );
        assert!(matches!(
            detach.into_workbench_intent(),
            Some(WorkbenchIntent::DetachNodeToSplit { key: detached }) if detached == key
        ));

        let move_to_pane = registry.execute(
            ACTION_WORKBENCH_MOVE_NODE_TO_PANE,
            &app,
            ActionPayload::WorkbenchMoveNodeToPane { pane_id },
        );
        assert!(matches!(
            move_to_pane.into_workbench_intent(),
            Some(WorkbenchIntent::OpenNodeInPane { node, pane }) if node == key && pane == pane_id
        ));

        let open_persistence_hub = registry.execute(
            ACTION_WORKBENCH_OPEN_PERSISTENCE_HUB,
            &app,
            ActionPayload::WorkbenchOpenPersistenceHub,
        );
        assert!(matches!(
            open_persistence_hub.into_runtime_actions().as_slice(),
            [RuntimeAction::PublishSettingsRouteRequested { url, prefer_overlay }]
                if url == &VersoAddress::settings(GraphshellSettingsPath::Persistence).to_string()
                    && *prefer_overlay
        ));

        let open_history_manager = registry.execute(
            ACTION_WORKBENCH_OPEN_HISTORY_MANAGER,
            &app,
            ActionPayload::WorkbenchOpenHistoryManager,
        );
        assert!(matches!(
            open_history_manager.into_workbench_intent(),
            Some(WorkbenchIntent::OpenToolPane {
                kind: ToolPaneState::HistoryManager
            })
        ));

        let activate_workflow = registry.execute(
            ACTION_WORKBENCH_ACTIVATE_WORKFLOW,
            &app,
            ActionPayload::WorkbenchActivateWorkflow {
                workflow_id: "workflow:research".to_string(),
            },
        );
        assert!(matches!(
            activate_workflow.into_runtime_actions().as_slice(),
            [RuntimeAction::ActivateWorkflow { workflow_id }] if workflow_id == "workflow:research"
        ));
    }

    #[test]
    fn action_registry_matrix_persistence_actions_emit_app_commands() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();

        let save_snapshot = registry.execute(
            ACTION_WORKBENCH_SAVE_SNAPSHOT,
            &app,
            ActionPayload::WorkbenchSaveSnapshot,
        );
        assert!(matches!(
            save_snapshot.into_app_commands().first(),
            Some(AppCommand::SaveWorkspaceSnapshot)
        ));

        let restore_session = registry.execute(
            ACTION_WORKBENCH_RESTORE_SESSION,
            &app,
            ActionPayload::WorkbenchRestoreSession {
                name: "session:test".to_string(),
            },
        );
        assert!(matches!(
            restore_session.into_app_commands().first(),
            Some(AppCommand::RestoreWorkspaceSnapshotNamed { name })
                if name == "session:test"
        ));

        let restore_graph = registry.execute(
            ACTION_WORKBENCH_RESTORE_GRAPH,
            &app,
            ActionPayload::WorkbenchRestoreGraph { name: None },
        );
        assert!(matches!(
            restore_graph.into_app_commands().first(),
            Some(AppCommand::RestoreGraphSnapshotLatest)
        ));
    }

    #[test]
    fn action_registry_describe_action_reports_capability() {
        let registry = ActionRegistry::default();

        assert_eq!(
            registry.describe_action(ACTION_GRAPH_DESELECT_ALL),
            Some(ActionCapability::RequiresSelection)
        );
        assert_eq!(
            registry.describe_action(ACTION_GRAPH_NODE_COPY_URL),
            Some(ActionCapability::RequiresActiveNode)
        );
        assert_eq!(
            registry.describe_action(ACTION_WORKBENCH_SETTINGS_OPEN),
            Some(ActionCapability::AlwaysAvailable)
        );
        assert_eq!(
            registry.describe_action(ACTION_WORKBENCH_SETTINGS_PANE_OPEN),
            Some(ActionCapability::AlwaysAvailable)
        );
        assert_eq!(
            registry.describe_action(ACTION_WORKBENCH_SETTINGS_OVERLAY_OPEN),
            Some(ActionCapability::AlwaysAvailable)
        );
    }

    #[test]
    fn action_registry_capability_guard_rejects_deselect_without_selection() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_GRAPH_DESELECT_ALL,
            &app,
            ActionPayload::GraphDeselectAll,
        );

        assert!(matches!(
            execution,
            ActionOutcome::Failure(ActionFailure {
                kind: ActionFailureKind::Rejected,
                reason,
            }) if reason.contains("requires a non-empty selection")
        ));
    }

    #[test]
    fn action_registry_requires_namespace_name_keys() {
        assert!(is_namespaced_action_id(ACTION_OMNIBOX_NODE_SEARCH));
        assert!(is_namespaced_action_id(ACTION_GRAPH_VIEW_SUBMIT));
        assert!(is_namespaced_action_id(ACTION_DETAIL_VIEW_SUBMIT));
        assert!(is_namespaced_action_id(ACTION_GRAPH_NODE_OPEN));
        assert!(is_namespaced_action_id(ACTION_GRAPH_NODE_NEW));
        assert!(is_namespaced_action_id(ACTION_GRAPH_NODE_COPY_URL));
        assert!(is_namespaced_action_id(ACTION_GRAPH_EDGE_CREATE));
        assert!(is_namespaced_action_id(ACTION_GRAPH_SET_PHYSICS_PROFILE));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_UNDO));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_SPLIT_HORIZONTAL));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_SPLIT_VERTICAL));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_CLOSE_PANE));
        assert!(is_namespaced_action_id(
            ACTION_WORKBENCH_COMMAND_PALETTE_OPEN
        ));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_SETTINGS_OPEN));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_SETTINGS_PANE_OPEN));
        assert!(is_namespaced_action_id(
            ACTION_WORKBENCH_SETTINGS_OVERLAY_OPEN
        ));
        assert!(is_namespaced_action_id(
            ACTION_WORKBENCH_OPEN_PERSISTENCE_HUB
        ));
        assert!(is_namespaced_action_id(
            ACTION_WORKBENCH_OPEN_HISTORY_MANAGER
        ));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_ACTIVATE_WORKFLOW));
        assert!(is_namespaced_action_id(ACTION_VERSE_PAIR_DEVICE));
        assert!(!is_namespaced_action_id("action.invalid.dot"));
        assert!(!is_namespaced_action_id("missing_colon"));
        assert!(!is_namespaced_action_id("namespace:"));
        assert!(!is_namespaced_action_id(":name"));
        assert!(!is_namespaced_action_id("too:many:segments"));
    }
}
