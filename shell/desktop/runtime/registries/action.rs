use std::collections::HashMap;

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphMutation, LifecycleCause, RuntimeEvent, SelectionUpdateMode,
    WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::shell::desktop::workbench::pane_model::{PaneId, SplitDirection, ToolPaneState};
use euclid::default::Point2D;

use super::index::SearchResultKind;

pub(crate) const ACTION_OMNIBOX_NODE_SEARCH: &str = "omnibox:node_search";
pub(crate) const ACTION_GRAPH_VIEW_SUBMIT: &str = "graph:view_submit";
pub(crate) const ACTION_DETAIL_VIEW_SUBMIT: &str = "detail:view_submit";

pub(crate) const ACTION_GRAPH_NODE_OPEN: &str = "graph:node_open";
pub(crate) const ACTION_GRAPH_NODE_CLOSE: &str = "graph:node_close";
pub(crate) const ACTION_GRAPH_EDGE_CREATE: &str = "graph:edge_create";
pub(crate) const ACTION_GRAPH_SET_PHYSICS_PROFILE: &str = "graph:set_physics_profile";
pub(crate) const ACTION_GRAPH_NAVIGATE_BACK: &str = "graph:navigate_back";
pub(crate) const ACTION_GRAPH_NAVIGATE_FORWARD: &str = "graph:navigate_forward";
pub(crate) const ACTION_GRAPH_SELECT_NODE: &str = "graph:select_node";
pub(crate) const ACTION_GRAPH_DESELECT_ALL: &str = "graph:deselect_all";
pub(crate) const ACTION_WORKBENCH_SPLIT_HORIZONTAL: &str = "workbench:split_horizontal";
pub(crate) const ACTION_WORKBENCH_SPLIT_VERTICAL: &str = "workbench:split_vertical";
pub(crate) const ACTION_WORKBENCH_CLOSE_PANE: &str = "workbench:close_pane";
pub(crate) const ACTION_WORKBENCH_COMMAND_PALETTE_OPEN: &str = "workbench:command_palette_open";
pub(crate) const ACTION_WORKBENCH_SETTINGS_OPEN: &str = "workbench:settings_open";

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
    GraphEdgeCreate {
        from: NodeKey,
        to: NodeKey,
        label: Option<String>,
    },
    GraphSetPhysicsProfile {
        profile_id: String,
    },
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
    WorkbenchCommandPaletteOpen,
    WorkbenchSettingsOpen,
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
pub(crate) enum ActionOutcome {
    Intents(Vec<GraphIntent>),
    WorkbenchIntent(WorkbenchIntent),
    Failure(ActionFailure),
}

impl ActionOutcome {
    pub(crate) fn succeeded(&self) -> bool {
        matches!(self, Self::Intents(_) | Self::WorkbenchIntent(_))
    }

    pub(crate) fn intent_len(&self) -> usize {
        match self {
            Self::Intents(intents) => intents.len(),
            Self::WorkbenchIntent(_) => 1,
            Self::Failure(_) => 0,
        }
    }

    pub(crate) fn into_intents(self) -> Vec<GraphIntent> {
        match self {
            Self::Intents(intents) => intents,
            Self::WorkbenchIntent(_) => Vec::new(),
            Self::Failure(_) => Vec::new(),
        }
    }

    pub(crate) fn into_workbench_intent(self) -> Option<WorkbenchIntent> {
        match self {
            Self::WorkbenchIntent(intent) => Some(intent),
            Self::Intents(_) | Self::Failure(_) => None,
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
        self.handlers.remove(&action_id.to_ascii_lowercase()).is_some()
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
            ACTION_GRAPH_EDGE_CREATE,
            ActionCapability::RequiresWritableWorkspace,
            execute_graph_edge_create_action,
        );
        registry.register(
            ACTION_GRAPH_SET_PHYSICS_PROFILE,
            ActionCapability::RequiresWritableWorkspace,
            execute_graph_set_physics_profile_action,
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
            ACTION_WORKBENCH_SETTINGS_OPEN,
            ActionCapability::AlwaysAvailable,
            execute_workbench_settings_open_action,
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

        registry
    }
}

fn capability_available(app: &GraphBrowserApp, capability: ActionCapability) -> bool {
    match capability {
        ActionCapability::AlwaysAvailable => true,
        ActionCapability::RequiresActiveNode => app.get_single_selected_node().is_some(),
        ActionCapability::RequiresSelection => !app.focused_selection().is_empty(),
        // No explicit read-only workspace mode exists yet, so writable capability currently
        // gates intent shape and future UI affordances rather than a persisted lock bit.
        ActionCapability::RequiresWritableWorkspace => true,
    }
}

fn capability_reason(capability: ActionCapability) -> &'static str {
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

    ActionOutcome::Intents(vec![
        GraphIntent::SelectNode {
            key: *node_key,
            multi_select: false,
        },
        RuntimeEvent::PromoteNodeToActive {
            key: *node_key,
            cause: LifecycleCause::UserSelect,
        }
        .into(),
    ])
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

    ActionOutcome::Intents(vec![
        RuntimeEvent::DemoteNodeToCold {
            key: *node_key,
            cause: LifecycleCause::ExplicitClose,
        }
        .into(),
    ])
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

    ActionOutcome::Intents(vec![
        GraphMutation::CreateUserGroupedEdge {
            from: *from,
            to: *to,
            label: label.clone(),
        }
        .into(),
    ])
}

fn execute_graph_set_physics_profile_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::GraphSetPhysicsProfile { profile_id } = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "graph:set_physics_profile requires GraphSetPhysicsProfile payload"
                .to_string(),
        });
    };

    ActionOutcome::Intents(vec![GraphIntent::SetPhysicsProfile {
        profile_id: profile_id.clone(),
    }])
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

    ActionOutcome::Intents(vec![GraphIntent::TraverseBack])
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

    ActionOutcome::Intents(vec![GraphIntent::TraverseForward])
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

    ActionOutcome::Intents(vec![GraphIntent::SelectNode {
        key: *node_key,
        multi_select: false,
    }])
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

    ActionOutcome::Intents(vec![GraphIntent::UpdateSelection {
        keys: Vec::new(),
        mode: SelectionUpdateMode::Replace,
    }])
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

    ActionOutcome::WorkbenchIntent(WorkbenchIntent::SplitPane {
        source_pane: *pane_id,
        direction: SplitDirection::Horizontal,
    })
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

    ActionOutcome::WorkbenchIntent(WorkbenchIntent::SplitPane {
        source_pane: *pane_id,
        direction: SplitDirection::Vertical,
    })
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

    ActionOutcome::WorkbenchIntent(WorkbenchIntent::ClosePane {
        pane: *pane_id,
        restore_previous_focus: true,
    })
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

    ActionOutcome::WorkbenchIntent(WorkbenchIntent::OpenCommandPalette)
}

fn execute_workbench_settings_open_action(
    _app: &GraphBrowserApp,
    payload: &ActionPayload,
) -> ActionOutcome {
    let ActionPayload::WorkbenchSettingsOpen = payload else {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::InvalidPayload,
            reason: "workbench:settings_open requires WorkbenchSettingsOpen payload".to_string(),
        });
    };

    ActionOutcome::WorkbenchIntent(WorkbenchIntent::OpenToolPane {
        kind: ToolPaneState::Settings,
    })
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
        ActionOutcome::Intents(vec![
            GraphMutation::SetNodeUrl {
                key: selected_node,
                new_url: input.to_string(),
            }
            .into(),
        ])
    } else {
        let position = new_node_position_for_context(app, app.focused_selection().primary());
        ActionOutcome::Intents(vec![
            GraphMutation::CreateNodeAtUrl {
                url: input.to_string(),
                position,
            }
            .into(),
        ])
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
        return ActionOutcome::Intents(vec![
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
        ]);
    }

    ActionOutcome::Intents(vec![
        GraphMutation::CreateNodeAtUrl {
            url: normalized_url.clone(),
            position: new_node_position_for_context(app, app.focused_selection().primary()),
        }
        .into(),
    ])
}

fn graph_centroid_or_default(app: &GraphBrowserApp) -> Point2D<f32> {
    app.workspace
        .domain
        .graph
        .projected_centroid()
        .unwrap_or_else(|| Point2D::new(400.0, 300.0))
}

fn new_node_position_for_context(app: &GraphBrowserApp, anchor: Option<NodeKey>) -> Point2D<f32> {
    let base = anchor
        .and_then(|key| app.domain_graph().node_projected_position(key))
        .unwrap_or_else(|| graph_centroid_or_default(app));
    let n = app.domain_graph().node_count() as f32;
    let angle = n * std::f32::consts::FRAC_PI_4;
    let radius = 90.0;
    Point2D::new(base.x + radius * angle.cos(), base.y + radius * angle.sin())
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
            SearchResultKind::Node(key) => ActionOutcome::Intents(vec![GraphIntent::SelectNode {
                key: *key,
                multi_select: false,
            }]),
            SearchResultKind::HistoryUrl(url) => ActionOutcome::Intents(vec![
                GraphMutation::CreateNodeAtUrl {
                    url: url.clone(),
                    position: new_node_position_for_context(
                        app,
                        app.focused_selection().primary(),
                    ),
                }
                .into(),
            ]),
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
                Ok(node_id) => ActionOutcome::Intents(vec![GraphIntent::TrustPeer {
                    peer_id: node_id.to_string(),
                    display_name: format!("Paired {}", &node_id.to_string()[..8]),
                }]),
                Err(error) => ActionOutcome::Failure(ActionFailure {
                    kind: ActionFailureKind::Rejected,
                    reason: format!("pairing code decode failed: {error}"),
                }),
            }
        }
        PairingMode::LocalPeer { node_id } => match node_id.parse::<iroh::NodeId>() {
            Ok(parsed_node_id) => ActionOutcome::Intents(vec![GraphIntent::TrustPeer {
                peer_id: parsed_node_id.to_string(),
                display_name: format!("Local {}", &parsed_node_id.to_string()[..8]),
            }]),
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
    ActionOutcome::Intents(vec![crate::app::RuntimeEvent::SyncNow.into()])
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

    let peers = crate::mods::native::verse::get_trusted_peers();
    if peers.is_empty() {
        return ActionOutcome::Failure(ActionFailure {
            kind: ActionFailureKind::Rejected,
            reason: "verse_share_workspace has no trusted peers to share with".to_string(),
        });
    }

    ActionOutcome::Intents(
        peers
            .into_iter()
            .map(|peer| GraphIntent::GrantWorkspaceAccess {
                peer_id: peer.node_id.to_string(),
                workspace_id: workspace_id.clone(),
            })
            .collect(),
    )
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

    ActionOutcome::Intents(vec![
        GraphMutation::ForgetDevice {
            peer_id: node_id.clone(),
        }
        .into(),
    ])
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
        let peer_id = iroh::SecretKey::generate(&mut rand::thread_rng())
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
            ActionOutcome::Intents(intents) => {
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
            ActionOutcome::WorkbenchIntent(intent) => {
                panic!("unexpected workbench intent for verse share action: {intent:?}");
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
    fn action_registry_graph_set_physics_profile_emits_reducer_intent() {
        let app = GraphBrowserApp::new_for_testing();
        let registry = ActionRegistry::default();

        let execution = registry.execute(
            ACTION_GRAPH_SET_PHYSICS_PROFILE,
            &app,
            ActionPayload::GraphSetPhysicsProfile {
                profile_id: crate::registries::atomic::lens::PHYSICS_ID_GAS.to_string(),
            },
        );

        assert!(matches!(
            execution.into_intents().first(),
            Some(GraphIntent::SetPhysicsProfile { profile_id })
                if profile_id == crate::registries::atomic::lens::PHYSICS_ID_GAS
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
            ACTION_WORKBENCH_SETTINGS_OPEN,
            &app,
            ActionPayload::WorkbenchSettingsOpen,
        );
        assert!(matches!(
            settings.into_workbench_intent(),
            Some(WorkbenchIntent::OpenToolPane {
                kind: ToolPaneState::Settings
            })
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
            registry.describe_action(ACTION_WORKBENCH_SETTINGS_OPEN),
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
        assert!(is_namespaced_action_id(ACTION_GRAPH_EDGE_CREATE));
        assert!(is_namespaced_action_id(ACTION_GRAPH_SET_PHYSICS_PROFILE));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_SPLIT_HORIZONTAL));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_SPLIT_VERTICAL));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_CLOSE_PANE));
        assert!(is_namespaced_action_id(
            ACTION_WORKBENCH_COMMAND_PALETTE_OPEN
        ));
        assert!(is_namespaced_action_id(ACTION_WORKBENCH_SETTINGS_OPEN));
        assert!(is_namespaced_action_id(ACTION_VERSE_PAIR_DEVICE));
        assert!(!is_namespaced_action_id("action.invalid.dot"));
        assert!(!is_namespaced_action_id("missing_colon"));
        assert!(!is_namespaced_action_id("namespace:"));
        assert!(!is_namespaced_action_id(":name"));
        assert!(!is_namespaced_action_id("too:many:segments"));
    }
}
