use super::super::harness::TestRegistry;
use crate::app::{GraphViewId, ToolSurfaceReturnTarget};
use crate::shell::desktop::workbench::ux_bridge::UxNodeSelector;
use crate::shell::desktop::workbench::ux_tree;
use crate::shell::desktop::ui::toolbar::toolbar_ui::{
    CommandBarSemanticMetadata, CommandSurfaceSemanticSnapshot, OmnibarSemanticMetadata,
    PaletteSurfaceSemanticMetadata, clear_command_surface_semantic_snapshot,
    lock_command_surface_snapshot_tests, publish_command_surface_semantic_snapshot,
};

#[test]
fn uxtree_snapshot_and_probe_are_healthy_for_selected_node_flow() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-uxtree.example");
    harness.open_node_tab(node);
    harness.app.select_node(node, false);

    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 7);
    let snapshot_json = ux_tree::snapshot_json_for_tests(&snapshot);

    assert_eq!(
        snapshot_json
            .get("semantic_version")
            .and_then(|v| v.as_u64()),
        Some(5),
        "semantic schema version should be present"
    );
    assert_eq!(
        snapshot_json
            .get("presentation_version")
            .and_then(|v| v.as_u64()),
        Some(2),
        "presentation schema version should be present"
    );

    let semantic_nodes = snapshot_json
        .get("semantic_nodes")
        .and_then(|v| v.as_array())
        .expect("uxtree snapshot should expose semantic nodes");
    assert!(
        semantic_nodes.iter().any(|node| node
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .contains("Node")),
        "expected semantic layer to include node-domain identity"
    );

    let violation = ux_tree::presentation_id_consistency_violation(&snapshot);
    assert!(
        violation.is_none(),
        "healthy selected-node flow should satisfy semantic/presentation consistency invariant"
    );

    let bridge_focus_path = harness
        .ux_focus_path_via_driver()
        .expect("driver-backed focus path query should succeed");
    assert_eq!(bridge_focus_path.first().map(String::as_str), Some(ux_tree::UX_TREE_WORKBENCH_ROOT_ID));
    assert!(
        !bridge_focus_path.is_empty(),
        "bridge focus path should include at least the canonical workbench root"
    );
}

#[test]
fn command_surface_uxtree_snapshot_is_healthy_with_return_target() {
    let _guard = lock_command_surface_snapshot_tests();
    clear_command_surface_semantic_snapshot();
    publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
        command_bar: CommandBarSemanticMetadata {
            active_pane: None,
            focused_node: None,
            location_focused: false,
            route_events: crate::shell::desktop::ui::toolbar::toolbar_ui::CommandRouteEventSequenceMetadata::default(),
        },
        omnibar: OmnibarSemanticMetadata {
            active: false,
            focused: false,
            query: Some("graphshell".to_string()),
            match_count: 3,
            provider_status: None,
            active_pane: None,
            focused_node: None,
            mailbox_events: crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarMailboxEventSequenceMetadata::default(),
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

    let mut harness = TestRegistry::new();
    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 7);
    let snapshot_json = ux_tree::snapshot_json_for_tests(&snapshot);
    let semantic_nodes = snapshot_json
        .get("semantic_nodes")
        .and_then(|v| v.as_array())
        .expect("uxtree snapshot should expose semantic nodes");

    assert!(
        semantic_nodes.iter().any(|node| {
            node.get("role").and_then(|v| v.as_str()) == Some("CommandBar")
                && node.get("label").and_then(|v| v.as_str()) == Some("Command Bar")
        }),
        "command bar semantic node should be projected with stable label"
    );
    assert!(
        ux_tree::interactive_label_presence_violation(&snapshot).is_none(),
        "command-surface snapshot should satisfy interactive label presence invariant"
    );
    assert!(
        ux_tree::command_surface_capture_owner_violation(&snapshot).is_none(),
        "command-surface snapshot should not advertise conflicting capture owners"
    );
    assert!(
        ux_tree::command_surface_return_target_violation(&snapshot).is_none(),
        "command-surface snapshot should preserve a valid return path"
    );

    let command_bar = harness
        .ux_find_node_via_driver(&UxNodeSelector::ByRoleAndLabel(
            ux_tree::UxNodeRole::CommandBar,
            "Command Bar".to_string(),
        ))
        .expect("driver-backed find-node query should succeed");
    assert!(command_bar.is_some(), "bridge should resolve the command bar semantic node");

    clear_command_surface_semantic_snapshot();
}
