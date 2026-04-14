use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use egui_tiles::{Tiles, Tree};
use serde_json::Value;

use super::super::harness::TestRegistry;
use crate::app::{GraphBrowserApp, GraphViewId, WorkbenchIntent};
use crate::render::radial_menu::{
    RadialPaletteSemanticSnapshot, RadialPaletteSemanticSummary, RadialSectorSemanticMetadata,
    clear_semantic_snapshot, publish_semantic_snapshot,
};
use crate::shell::desktop::ui::gui_orchestration;
use crate::shell::desktop::ui::toolbar::toolbar_ui::{
    clear_command_surface_semantic_snapshot, lock_command_surface_snapshot_tests,
};
use crate::shell::desktop::workbench::pane_model::GraphPaneRef;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::ux_tree;

const UPDATE_SNAPSHOTS_ENV_VAR: &str = "GRAPHSHELL_UPDATE_UX_SNAPSHOTS";
const SNAPSHOT_DIR: &str = "tests/scenarios/snapshots";

#[derive(Debug, Clone)]
struct SnapshotBaselineCase {
    name: &'static str,
    snapshot: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JsonSnapshotDiffGate {
    semantic_changed: bool,
    presentation_changed: bool,
    trace_changed: bool,
    blocking_failure: bool,
}

fn update_snapshots_enabled() -> bool {
    std::env::var_os(UPDATE_SNAPSHOTS_ENV_VAR).is_some()
}

fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT_DIR)
}

fn write_snapshot_baseline(path: &Path, snapshot: &Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("snapshot baseline directory should be creatable");
    }
    let body = serde_json::to_string_pretty(snapshot)
        .expect("normalized snapshot baseline should serialize to JSON");
    fs::write(path, body).expect("snapshot baseline should be writable");
}

fn load_snapshot_baseline(path: &Path) -> Value {
    let body = fs::read_to_string(path).expect("snapshot baseline should be readable");
    serde_json::from_str(&body).expect("snapshot baseline should parse as JSON")
}

fn classify_snapshot_json_diff_gate(
    baseline: &Value,
    current: &Value,
    promote_presentation: bool,
) -> JsonSnapshotDiffGate {
    let semantic_changed = baseline.get("semantic_version") != current.get("semantic_version")
        || baseline.get("semantic_nodes") != current.get("semantic_nodes");
    let presentation_changed = baseline.get("presentation_version")
        != current.get("presentation_version")
        || baseline.get("presentation_nodes") != current.get("presentation_nodes");
    let trace_changed = baseline.get("trace_version") != current.get("trace_version")
        || baseline.get("trace_nodes") != current.get("trace_nodes")
        || baseline.get("trace_summary") != current.get("trace_summary");
    let blocking_failure = semantic_changed || (presentation_changed && promote_presentation);

    JsonSnapshotDiffGate {
        semantic_changed,
        presentation_changed,
        trace_changed,
        blocking_failure,
    }
}

fn normalize_snapshot_json_for_baseline(snapshot: &ux_tree::UxTreeSnapshot) -> Value {
    let mut json = ux_tree::snapshot_json_for_tests(snapshot);
    let mut uuid_aliases = HashMap::<String, String>::new();
    let mut next_uuid_alias = 1usize;
    normalize_json_value(&mut json, &mut uuid_aliases, &mut next_uuid_alias);
    json
}

fn normalize_json_value(
    value: &mut Value,
    uuid_aliases: &mut HashMap<String, String>,
    next_uuid_alias: &mut usize,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                normalize_json_value(item, uuid_aliases, next_uuid_alias);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                normalize_json_value(item, uuid_aliases, next_uuid_alias);
            }
        }
        Value::String(text) => {
            *text = normalize_uuid_substrings(text, uuid_aliases, next_uuid_alias);
        }
        _ => {}
    }
}

fn normalize_uuid_substrings(
    input: &str,
    uuid_aliases: &mut HashMap<String, String>,
    next_uuid_alias: &mut usize,
) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;

    while cursor < input.len() {
        if let Some(candidate) = input.get(cursor..cursor.saturating_add(36))
            && looks_like_uuid(candidate)
            && uuid::Uuid::parse_str(candidate).is_ok()
        {
            let alias = uuid_aliases
                .entry(candidate.to_string())
                .or_insert_with(|| {
                    let alias = format!("<uuid:{:02}>", *next_uuid_alias);
                    *next_uuid_alias += 1;
                    alias
                })
                .clone();
            output.push_str(&alias);
            cursor += 36;
            continue;
        }

        let ch = input[cursor..]
            .chars()
            .next()
            .expect("cursor should remain on a valid UTF-8 boundary");
        output.push(ch);
        cursor += ch.len_utf8();
    }

    output
}

fn looks_like_uuid(candidate: &str) -> bool {
    if candidate.len() != 36 {
        return false;
    }

    candidate.char_indices().all(|(idx, ch)| match idx {
        8 | 13 | 18 | 23 => ch == '-',
        _ => ch.is_ascii_hexdigit(),
    })
}

fn capture_snapshot_case(name: &'static str, snapshot: ux_tree::UxTreeSnapshot) -> SnapshotBaselineCase {
    SnapshotBaselineCase {
        name,
        snapshot: normalize_snapshot_json_for_baseline(&snapshot),
    }
}

fn graph_navigation_selected_node_case() -> SnapshotBaselineCase {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-camera.example/page");
    harness.open_node_tab(node);
    harness.app.select_node(node, false);

    capture_snapshot_case(
        "pre_wgpu_graph_navigation_selected_node",
        ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 11),
    )
}

fn pane_lifecycle_open_node_tab_case() -> SnapshotBaselineCase {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-pane.example/article");
    harness.open_node_tab(node);

    capture_snapshot_case(
        "pre_wgpu_pane_lifecycle_open_node_tab",
        ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 12),
    )
}

fn command_surface_radial_palette_case() -> SnapshotBaselineCase {
    publish_semantic_snapshot(RadialPaletteSemanticSnapshot {
        sectors: vec![
            RadialSectorSemanticMetadata {
                tier: 1,
                domain_label: "Workbench".to_string(),
                action_id: "workbench.toggle_command_palette".to_string(),
                enabled: true,
                page: 0,
                rail_position: 0.0,
                angle_rad: 0.0,
                hover_scale: 1.15,
            },
            RadialSectorSemanticMetadata {
                tier: 2,
                domain_label: "Graph".to_string(),
                action_id: "graph.open_selected_node".to_string(),
                enabled: true,
                page: 0,
                rail_position: 0.25,
                angle_rad: 1.2,
                hover_scale: 1.0,
            },
        ],
        summary: RadialPaletteSemanticSummary {
            tier1_visible_count: 1,
            tier2_visible_count: 1,
            tier2_page: 0,
            tier2_page_count: 1,
            overflow_hidden_entries: 0,
            label_pre_collisions: 0,
            label_post_collisions: 0,
            fallback_to_palette: false,
            fallback_reason: None,
        },
    });

    let harness = TestRegistry::new();
    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 14);
    clear_semantic_snapshot();

    capture_snapshot_case("pre_wgpu_command_surface_radial_palette", snapshot)
}

fn degraded_viewer_placeholder_case() -> SnapshotBaselineCase {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("unknown-scheme://scenario-degraded.example/page");
    harness.open_node_tab(node);

    for tile in harness.tiles_tree.tiles.iter_mut() {
        if let egui_tiles::Tile::Pane(TileKind::Node(state)) = tile.1
            && state.node == node
        {
            state.render_mode = crate::shell::desktop::workbench::pane_model::TileRenderMode::Placeholder;
        }
    }

    capture_snapshot_case(
        "pre_wgpu_degraded_viewer_placeholder",
        ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 16),
    )
}

fn degraded_viewer_composited_case() -> SnapshotBaselineCase {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-healthy.example/page");
    harness.open_node_tab(node);

    capture_snapshot_case(
        "pre_wgpu_degraded_viewer_composited",
        ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 17),
    )
}

fn critical_path_snapshot_cases() -> Vec<SnapshotBaselineCase> {
    vec![
        graph_navigation_selected_node_case(),
        pane_lifecycle_open_node_tab_case(),
        command_surface_radial_palette_case(),
        degraded_viewer_placeholder_case(),
        degraded_viewer_composited_case(),
    ]
}

fn assert_snapshot_case_matches_baseline(case: &SnapshotBaselineCase) {
    let baseline_path = snapshot_dir().join(format!("{}.json", case.name));
    if update_snapshots_enabled() || !baseline_path.exists() {
        write_snapshot_baseline(&baseline_path, &case.snapshot);
    }

    let baseline = load_snapshot_baseline(&baseline_path);
    let gate = classify_snapshot_json_diff_gate(&baseline, &case.snapshot, false);

    if gate.blocking_failure {
        panic!(
            "UxTree snapshot baseline mismatch for {name} at {path}. semantic_changed={semantic_changed} presentation_changed={presentation_changed} trace_changed={trace_changed}. Set {env_var}=1 and rerun the test to refresh committed baselines if this is an intentional structural change.",
            name = case.name,
            path = baseline_path.display(),
            semantic_changed = gate.semantic_changed,
            presentation_changed = gate.presentation_changed,
            trace_changed = gate.trace_changed,
            env_var = UPDATE_SNAPSHOTS_ENV_VAR,
        );
    }

    if gate.presentation_changed || gate.trace_changed {
        eprintln!(
            "informational UxTree snapshot drift for {} (presentation_changed={}, trace_changed={}) against {}",
            case.name,
            gate.presentation_changed,
            gate.trace_changed,
            baseline_path.display()
        );
    }
}

#[test]
fn ux_tree_diff_gate_policy_matches_contract_defaults() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-uxtree-diff.example");
    harness.open_node_tab(node);

    let baseline = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 9);

    let mut semantic_changed = baseline.clone();
    semantic_changed.semantic_nodes[0].label = "Workbench Contract Shift".to_string();
    let semantic_gate = ux_tree::classify_snapshot_diff_gate(&baseline, &semantic_changed, false);
    assert!(
        semantic_gate.blocking_failure,
        "semantic diffs must be blocking"
    );

    let mut presentation_changed = baseline.clone();
    presentation_changed.presentation_nodes[0]
        .transient_flags
        .push("hover:fade");
    let presentation_gate =
        ux_tree::classify_snapshot_diff_gate(&baseline, &presentation_changed, false);
    assert!(
        !presentation_gate.blocking_failure,
        "presentation diffs should remain informational by default"
    );

    let promoted_gate =
        ux_tree::classify_snapshot_diff_gate(&baseline, &presentation_changed, true);
    assert!(
        promoted_gate.blocking_failure,
        "promoted presentation diffs should become blocking"
    );
}

#[test]
fn pre_wgpu_critical_path_snapshots_match_baselines() {
    for case in critical_path_snapshot_cases() {
        assert_snapshot_case_matches_baseline(&case);
    }
}

#[test]
fn command_surface_toggle_command_palette_snapshot_stays_structurally_stable() {
    let _guard = lock_command_surface_snapshot_tests();
    clear_command_surface_semantic_snapshot();

    let mut app = GraphBrowserApp::new_for_testing();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
    let mut tree = Tree::new("pre_wgpu_command_palette_structure", root, tiles);

    let baseline = normalize_snapshot_json_for_baseline(&ux_tree::build_snapshot(&tree, &app, 21));

    let mut intents = vec![WorkbenchIntent::ToggleCommandPalette];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);
    let current = normalize_snapshot_json_for_baseline(&ux_tree::build_snapshot(&tree, &app, 22));
    let gate = classify_snapshot_json_diff_gate(&baseline, &current, false);

    assert!(
        !gate.blocking_failure,
        "command palette open state should not mutate the structural UxTree snapshot gate"
    );

    clear_command_surface_semantic_snapshot();
}

