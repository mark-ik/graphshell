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

// The portable action taxonomy — `InputMode`, `ActionCategory`,
// `ActionId`, the key/label helpers, `all_action_ids`,
// `action_id_has_namespace_format` — lives in
// [`graphshell_core::actions`]. Re-exported here so existing call
// sites (`use crate::render::action_registry::{ActionId, …}`)
// resolve unchanged. Host-coupled helpers
// (`shortcut_hints_for_action`, `ActionContext`, `ActionEntry`,
// `list_actions_for_context`, `rank_categories_for_context`) stay
// in this module because they depend on the host-side input
// registry / app-state types.
pub use graphshell_core::actions::{
    action_id_has_namespace_format, all_action_ids, ActionCategory, ActionId,
    CATEGORY_PIN_ORDER_PERSIST_KEY, CATEGORY_RECENCY_PERSIST_KEY, InputMode,
    category_from_persisted_name, category_persisted_name, default_category_order,
};

static ACTION_KEY_AUDIT_ONCE: Once = Once::new();

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
        ActionCategory::Graph if action_context.target_frame_name.is_some() => 320,
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

/// Shortcut-binding display labels for a given action, formatted for
/// palette / radial / tooltip presentation. Reads the live input
/// registry, which maps action-ids to user-configured bindings.
///
/// Kept as a free function (rather than a method on `ActionId`) so
/// the portable `ActionId` enum in `graphshell-core` doesn't depend
/// on the host-side input registry. Call sites use
/// `shortcut_hints_for_action(id)` where they previously wrote
/// `id.shortcut_hints()`.
pub fn shortcut_hints_for_action(id: ActionId) -> Vec<String> {
    let action_ids: &[&str] = match id {
        ActionId::NodeNew => &[input_action::graph::NODE_NEW],
        ActionId::NodePinToggle => &[input_action::graph::NODE_PIN_TOGGLE],
        ActionId::NodePinSelected => &[input_action::graph::NODE_PIN_SELECTED],
        ActionId::NodeUnpinSelected => &[input_action::graph::NODE_UNPIN_SELECTED],
        ActionId::NodeDelete => &[input_action::graph::NODE_DELETE],
        ActionId::NodeEditTags => &[input_action::graph::NODE_EDIT_TAGS],
        ActionId::EdgeConnectPair => &[input_action::graph::EDGE_CONNECT_PAIR],
        ActionId::EdgeConnectBoth => &[input_action::graph::EDGE_CONNECT_BOTH],
        ActionId::EdgeRemoveUser => &[input_action::graph::EDGE_REMOVE_USER],
        ActionId::GraphFit => &[],
        ActionId::GraphFitGraphlet => &[],
        ActionId::GraphToggleOverviewPlane => &[input_action::graph::TOGGLE_OVERVIEW_PLANE],
        ActionId::GraphTogglePhysics => &[input_action::graph::TOGGLE_PHYSICS],
        ActionId::GraphPhysicsConfig => &[input_action::workbench::OPEN_PHYSICS_SETTINGS],
        ActionId::GraphCommandPalette => &[input_action::graph::COMMAND_PALETTE_OPEN],
        ActionId::GraphRadialMenu => &[input_action::graph::RADIAL_MENU_OPEN],
        ActionId::WorkbenchToggleOverlay => &[input_action::workbench::TOGGLE_WORKBENCH_OVERLAY],
        ActionId::PersistUndo => &[input_action::workbench::UNDO],
        ActionId::PersistRedo => &[input_action::workbench::REDO],
        ActionId::PersistOpenHistoryManager => &[input_action::workbench::OPEN_HISTORY_MANAGER],
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

// The ActionId enum body and its `key` / `short_label` / `label` /
// `category` methods moved to `graphshell_core::actions` in M4
// slice 1 (2026-04-22). They're re-exported via the `pub use ...`
// at the top of this module so existing call sites
// (`ActionId::NodeNew`, `id.label()`, `id.category()`, etc.) resolve
// unchanged. `shortcut_hints` stays here as a free function above
// because it reads the host-side input registry.

// The `all_action_ids` and `action_id_has_namespace_format`
// helpers also moved to `graphshell_core::actions`; both are
// re-exported above.

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
    /// Originating scope of the action invocation (global palette,
    /// a graph view with/without a target, or a workbench pane).
    /// Added by the 2026-04-20 action-surfaces redesign to make
    /// scope a first-class input to future filtering logic. Legacy
    /// per-field proxies (`target_node`, `target_frame_name`, etc.)
    /// remain for backward compatibility with existing enable/disable
    /// predicates.
    pub scope: crate::app::ActionScope,
    /// Primary target node, if any (right-click target, hovered node, etc.).
    /// `None` means global scope — the full action list is returned.
    pub target_node: Option<NodeKey>,
    /// Primary target frame, if any (frame backdrop or frame tab affordance).
    pub target_frame_name: Option<String>,
    /// Representative frame member used for open-as-frame operations.
    pub target_frame_member: Option<NodeKey>,
    /// Whether the target frame currently suppresses split offers.
    pub target_frame_split_offer_suppressed: bool,
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
    let frame_ops_enabled = context.target_frame_name.is_some();
    let pair_enabled = context.pair_context.is_some();

    let all: &[(ActionId, bool)] = &[
        // Node
        (NodeNew, true),
        (NodeNewAsTab, true),
        (NodePinSelected, node_ops_enabled),
        (NodeUnpinSelected, node_ops_enabled),
        (NodeDelete, node_ops_enabled),
        (NodeEditTags, node_ops_enabled),
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
        (NodeImportWebFinger, node_ops_enabled),
        (NodeResolveNip05, node_ops_enabled),
        (NodeResolveMatrix, node_ops_enabled),
        (NodeResolveActivityPub, node_ops_enabled),
        (NodeRefreshPersonIdentity, node_ops_enabled),
        (NodeMarkTombstone, node_ops_enabled),
        // Edge
        (EdgeConnectPair, pair_enabled),
        (EdgeConnectBoth, pair_enabled),
        (EdgeRemoveUser, pair_enabled),
        // Graph
        (GraphFit, true),
        (GraphFitGraphlet, true),
        (GraphToggleOverviewPlane, true),
        (GraphTogglePhysics, true),
        (GraphToggleGhostNodes, true),
        (GraphPhysicsConfig, true),
        (GraphCommandPalette, true),
        (GraphRadialMenu, true),
        (WorkbenchToggleOverlay, true),
        (FrameSelect, frame_ops_enabled),
        (
            FrameOpen,
            frame_ops_enabled && context.target_frame_member.is_some(),
        ),
        (
            FrameOpenAsSplit,
            frame_ops_enabled && context.target_frame_member.is_some(),
        ),
        (FrameRename, frame_ops_enabled),
        (FrameSettings, frame_ops_enabled),
        (
            FrameSuppressSplitOffer,
            frame_ops_enabled && !context.target_frame_split_offer_suppressed,
        ),
        (FrameDelete, frame_ops_enabled),
        (
            FrameEnableSplitOffer,
            frame_ops_enabled && context.target_frame_split_offer_suppressed,
        ),
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
        (PersistImportBookmarks, true),
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
        WorkbenchToggleOverlay,
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
            scope: crate::app::ActionScope::default(),
            target_node: None,
            target_frame_name: None,
            target_frame_member: None,
            target_frame_split_offer_suppressed: false,
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
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeEditTags)
                .is_some_and(|entry| !entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeImportWebFinger)
                .is_some_and(|entry| !entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeResolveNip05)
                .is_some_and(|entry| !entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeRefreshPersonIdentity)
                .is_some_and(|entry| !entry.enabled)
        );
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
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeEditTags)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeImportWebFinger)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeResolveMatrix)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeResolveActivityPub)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|e| e.id == ActionId::NodeRefreshPersonIdentity)
                .is_some_and(|entry| entry.enabled)
        );
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
        for entry in entries.iter().filter(|e| {
            matches!(e.id.category(), ActionCategory::Graph)
                && !matches!(
                    e.id,
                    ActionId::FrameSelect
                        | ActionId::FrameOpen
                        | ActionId::FrameOpenAsSplit
                        | ActionId::FrameRename
                        | ActionId::FrameSettings
                        | ActionId::FrameSuppressSplitOffer
                        | ActionId::FrameDelete
                        | ActionId::FrameEnableSplitOffer
                        | ActionId::WorkbenchUnlockSurfaceLayout
                        | ActionId::WorkbenchLockSurfaceLayout
                        | ActionId::WorkbenchRememberLayoutPreference
                )
        }) {
            assert!(entry.enabled, "{:?} should always be enabled", entry.id);
        }
    }

    #[test]
    fn frame_actions_enable_against_targeted_frame_context() {
        let entries = list_actions_for_context(&ActionContext {
            target_frame_name: Some("workspace-alpha".to_string()),
            target_frame_member: Some(NodeKey::new(7)),
            target_frame_split_offer_suppressed: false,
            ..default_context()
        });

        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameSelect)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameOpen)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameOpenAsSplit)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameRename)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameSuppressSplitOffer)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameDelete)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameEnableSplitOffer)
                .is_some_and(|entry| !entry.enabled)
        );
    }

    #[test]
    fn frame_enable_action_is_only_enabled_when_frame_offer_is_suppressed() {
        let entries = list_actions_for_context(&ActionContext {
            target_frame_name: Some("workspace-alpha".to_string()),
            target_frame_member: Some(NodeKey::new(7)),
            target_frame_split_offer_suppressed: true,
            ..default_context()
        });

        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameEnableSplitOffer)
                .is_some_and(|entry| entry.enabled)
        );
        assert!(
            entries
                .iter()
                .find(|entry| entry.id == ActionId::FrameSuppressSplitOffer)
                .is_some_and(|entry| !entry.enabled)
        );
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
            ActionId::PersistImportBookmarks,
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
    fn test_workbench_overlay_action_is_listed_in_graph_bucket() {
        let ctx = default_context();
        let entries = list_actions_for_context(&ctx);

        let entry = entries
            .iter()
            .find(|entry| entry.id == ActionId::WorkbenchToggleOverlay)
            .unwrap_or_else(|| {
                panic!(
                    "missing action entry for {:?}",
                    ActionId::WorkbenchToggleOverlay
                )
            });

        assert!(entry.enabled);
        assert_eq!(
            ActionId::WorkbenchToggleOverlay.category(),
            ActionCategory::Graph
        );
    }

    #[test]
    fn test_representative_action_labels_convey_purpose_in_context() {
        let cases = [
            (ActionId::NodeCopyUrl, ["Copy", "URL"].as_slice()),
            (
                ActionId::NodeImportWebFinger,
                ["Import", "WebFinger"].as_slice(),
            ),
            (ActionId::NodeResolveNip05, ["Resolve", "NIP-05"].as_slice()),
            (
                ActionId::NodeResolveMatrix,
                ["Resolve", "Matrix"].as_slice(),
            ),
            (
                ActionId::NodeResolveActivityPub,
                ["Import", "ActivityPub"].as_slice(),
            ),
            (
                ActionId::NodeRefreshPersonIdentity,
                ["Refresh", "Identity"].as_slice(),
            ),
            (ActionId::NodeDelete, ["Delete", "Node"].as_slice()),
            (ActionId::NodeOpenFrame, ["Open", "Frame"].as_slice()),
            (ActionId::EdgeConnectPair, ["Connect", "Target"].as_slice()),
            (ActionId::PersistSaveGraph, ["Save", "Graph"].as_slice()),
            (
                ActionId::PersistRestoreLatestGraph,
                ["Restore", "Graph"].as_slice(),
            ),
            (
                ActionId::PersistImportBookmarks,
                ["Import", "Bookmarks"].as_slice(),
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
