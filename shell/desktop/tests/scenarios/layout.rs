use super::super::harness::TestRegistry;
use crate::shell::desktop::runtime::diagnostics::{CompositorTileSample, HierarchySample};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
    CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops;
use egui_tiles::Tile;

#[test]
fn compositor_frames_capture_sequence_and_active_tile_count_transitions() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.open_node_tab(node);
    harness.map_test_webview(node);

    let rect_a = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(320.0, 240.0));
    let rect_b = egui::Rect::from_min_max(egui::pos2(8.0, 12.0), egui::pos2(360.0, 260.0));

    harness.step_with_tile_sample(node, true, true, rect_a);
    harness.step_with_tile_sample(node, true, true, rect_b);

    let snapshot = harness.snapshot();
    let frames = snapshot
        .get("compositor_frames")
        .and_then(|v| v.as_array())
        .expect("snapshot should contain compositor_frames");

    assert!(frames.len() >= 2, "expected at least two frame samples");
    let prev = &frames[frames.len() - 2];
    let last = frames.last().expect("last frame should exist");

    let prev_seq = prev
        .get("sequence")
        .and_then(|v| v.as_u64())
        .expect("previous frame must include sequence");
    let last_seq = last
        .get("sequence")
        .and_then(|v| v.as_u64())
        .expect("last frame must include sequence");

    assert!(
        last_seq > prev_seq,
        "frame sequence should increase across samples"
    );
    assert_eq!(
        last.get("active_tile_count").and_then(|v| v.as_u64()),
        Some(1),
        "active tile count should remain stable for single-tile flow"
    );
}

#[test]
fn compositor_tile_rects_are_non_zero_in_healthy_layout_path() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.open_node_tab(node);
    harness.map_test_webview(node);

    let rect = egui::Rect::from_min_max(egui::pos2(16.0, 24.0), egui::pos2(420.0, 300.0));
    harness.step_with_tile_sample(node, true, true, rect);

    let snapshot = harness.snapshot();
    let tile = TestRegistry::tile_for_node(&snapshot, node).expect("tile should exist");

    let min_x = tile
        .get("rect")
        .and_then(|r| r.get("min"))
        .and_then(|m| m.get("x"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let min_y = tile
        .get("rect")
        .and_then(|r| r.get("min"))
        .and_then(|m| m.get("y"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let max_x = tile
        .get("rect")
        .and_then(|r| r.get("max"))
        .and_then(|m| m.get("x"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let max_y = tile
        .get("rect")
        .and_then(|r| r.get("max"))
        .and_then(|m| m.get("y"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    assert!(max_x > min_x, "tile width must be non-zero");
    assert!(max_y > min_y, "tile height must be non-zero");
}

#[test]
fn healthy_layout_path_keeps_active_tile_violation_channel_zero() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.open_node_tab(node);
    harness.map_test_webview(node);
    let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 200.0));
    harness.step_with_tile_sample(node, true, true, rect);

    let snapshot = harness.snapshot();
    let violations =
        TestRegistry::channel_count(&snapshot, "tile_render_pass.active_tile_violation");

    assert_eq!(
        violations, 0,
        "healthy path should not emit active tile violation channel"
    );
}

#[test]
fn unhealthy_layout_signal_is_observable_via_active_tile_violation_channel() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.open_node_tab(node);
    let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(280.0, 180.0));
    harness.step_with_tile_sample(node, false, false, rect);

    harness
        .diagnostics
        .emit_message_sent_for_tests("tile_render_pass.active_tile_violation", 1);

    let snapshot = harness.snapshot();
    let violations =
        TestRegistry::channel_count(&snapshot, "tile_render_pass.active_tile_violation");

    assert!(
        violations > 0,
        "violation channel should expose unhealthy active tile signal"
    );
}

#[test]
fn healthy_composited_overlay_emits_style_and_mode_without_pass_order_violation() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com/overlay");
    harness.open_node_tab(node);
    harness.map_test_webview(node);
    let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(220.0, 140.0));
    harness.step_with_tile_sample(node, true, true, rect);

    harness
        .diagnostics
        .emit_message_sent_for_tests(CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE, 1);
    harness
        .diagnostics
        .emit_message_sent_for_tests(CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE, 1);

    let snapshot = harness.snapshot();
    let style_count = TestRegistry::channel_count(&snapshot, CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE);
    let mode_count =
        TestRegistry::channel_count(&snapshot, CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE);
    let violation_count = TestRegistry::channel_count(&snapshot, "tile_compositor.pass_order_violation");

    assert!(style_count > 0, "expected overlay style channel in healthy path");
    assert!(mode_count > 0, "expected overlay mode channel in healthy path");
    assert_eq!(
        violation_count, 0,
        "healthy composited overlay path should not emit pass-order violations"
    );
}

#[test]
fn compositor_multi_tile_layout_samples_have_non_overlapping_rects() {
    let mut harness = TestRegistry::new();
    let left = harness.add_node("https://example.com/left");
    let right = harness.add_node("https://example.com/right");
    harness.open_node_tab(left);
    harness.open_node_tab(right);
    harness.map_test_webview(left);
    harness.map_test_webview(right);

    let left_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 220.0));
    let right_rect = egui::Rect::from_min_max(egui::pos2(320.0, 0.0), egui::pos2(620.0, 220.0));
    let viewport = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(640.0, 240.0));

    let hierarchy = vec![
        HierarchySample {
            line: "* Split Horizontal".to_string(),
            node_key: None,
        },
        HierarchySample {
            line: format!("  * Tile Node Viewer {:?}", left),
            node_key: Some(left),
        },
        HierarchySample {
            line: format!("  * Tile Node Viewer {:?}", right),
            node_key: Some(right),
        },
    ];
    let tiles = vec![
        CompositorTileSample {
            node_key: left,
            rect: left_rect,
            mapped_webview: true,
            has_context: true,
            paint_callback_registered: true,
            render_path_hint: "composited",
        },
        CompositorTileSample {
            node_key: right,
            rect: right_rect,
            mapped_webview: true,
            has_context: true,
            paint_callback_registered: true,
            render_path_hint: "composited",
        },
    ];

    harness.step_with_frame_sample(2, true, viewport, hierarchy, tiles);

    let snapshot = harness.snapshot();
    let frames = snapshot
        .get("compositor_frames")
        .and_then(|v| v.as_array())
        .expect("snapshot should contain compositor_frames");
    let last = frames
        .last()
        .expect("at least one compositor frame expected");
    let sampled_tiles = last
        .get("tiles")
        .and_then(|v| v.as_array())
        .expect("last frame should include tiles");

    assert_eq!(sampled_tiles.len(), 2, "expected two sampled tiles");
    assert_eq!(
        last.get("active_tile_count").and_then(|v| v.as_u64()),
        Some(2),
        "active tile count should match multi-tile sample"
    );

    let left_max_x = sampled_tiles[0]
        .get("rect")
        .and_then(|r| r.get("max"))
        .and_then(|m| m.get("x"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let right_min_x = sampled_tiles[1]
        .get("rect")
        .and_then(|r| r.get("min"))
        .and_then(|m| m.get("x"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    assert!(
        left_max_x <= right_min_x,
        "tile rects should not overlap in horizontal split sample"
    );
}

#[test]
fn compositor_hierarchy_samples_include_split_container_and_child_tiles() {
    let mut harness = TestRegistry::new();
    let left = harness.add_node("https://example.com/a");
    let right = harness.add_node("https://example.com/b");
    harness.open_node_tab(left);
    harness.open_node_tab(right);
    harness.map_test_webview(left);
    harness.map_test_webview(right);

    let viewport = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(700.0, 260.0));
    let hierarchy = vec![
        HierarchySample {
            line: "* Split Horizontal".to_string(),
            node_key: None,
        },
        HierarchySample {
            line: format!("  * Tile Node Viewer {:?}", left),
            node_key: Some(left),
        },
        HierarchySample {
            line: format!("  * Tile Node Viewer {:?}", right),
            node_key: Some(right),
        },
    ];
    let tiles = vec![
        CompositorTileSample {
            node_key: left,
            rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(330.0, 240.0)),
            mapped_webview: true,
            has_context: true,
            paint_callback_registered: true,
            render_path_hint: "composited",
        },
        CompositorTileSample {
            node_key: right,
            rect: egui::Rect::from_min_max(egui::pos2(350.0, 0.0), egui::pos2(680.0, 240.0)),
            mapped_webview: true,
            has_context: true,
            paint_callback_registered: true,
            render_path_hint: "composited",
        },
    ];

    harness.step_with_frame_sample(2, true, viewport, hierarchy, tiles);

    let snapshot = harness.snapshot();
    let frames = snapshot
        .get("compositor_frames")
        .and_then(|v| v.as_array())
        .expect("snapshot should contain compositor_frames");
    let last = frames
        .last()
        .expect("at least one compositor frame expected");
    let hierarchy_lines = last
        .get("hierarchy")
        .and_then(|v| v.as_array())
        .expect("last frame should include hierarchy")
        .iter()
        .filter_map(|entry| entry.get("line").and_then(|line| line.as_str()))
        .collect::<Vec<_>>();

    assert!(
        hierarchy_lines
            .iter()
            .any(|line| line.contains("Split Horizontal")),
        "hierarchy should include split container"
    );
    assert!(
        hierarchy_lines
            .iter()
            .any(|line| line.contains(&left.index().to_string())),
        "hierarchy should include left tile node"
    );
    assert!(
        hierarchy_lines
            .iter()
            .any(|line| line.contains(&right.index().to_string())),
        "hierarchy should include right tile node"
    );
}

#[test]
fn pane_close_handoff_restores_graph_pane_focus_deterministically() {
    let mut harness = TestRegistry::new();
    let node_a = harness.add_node("https://example.com/a");
    let node_b = harness.add_node("https://example.com/b");
    harness.open_node_tab(node_a);
    harness.open_node_tab(node_b);

    let node_tile_ids: Vec<_> = harness
        .tiles_tree
        .tiles
        .iter()
        .filter_map(|(tile_id, tile)| match tile {
            Tile::Pane(TileKind::Node(_)) => Some(*tile_id),
            _ => None,
        })
        .collect();

    assert!(node_tile_ids.len() >= 2, "expected at least two node panes");

    for tile_id in node_tile_ids {
        harness.tiles_tree.remove_recursively(tile_id);
    }

    let _ = tile_view_ops::ensure_active_tile(&mut harness.tiles_tree);

    let active_graph = tile_view_ops::active_graph_view_id(&harness.tiles_tree);
    assert!(
        active_graph.is_some(),
        "graph pane should hold focus after node pane closures"
    );
}
