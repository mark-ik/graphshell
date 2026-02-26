use super::super::harness::TestRegistry;

#[test]
fn webview_tile_snapshot_reports_mapping_and_context_health() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.open_node_tab(node);
    harness.map_test_webview(node);

    let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(320.0, 240.0));
    harness.step_with_tile_sample(node, true, true, rect);

    let snapshot = harness.snapshot();
    let tile = TestRegistry::tile_for_node(&snapshot, node).expect("tile should exist in snapshot");

    assert_eq!(tile.get("mapped_webview").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(tile.get("has_context").and_then(|v| v.as_bool()), Some(true));
    let width = tile
        .get("rect")
        .and_then(|r| r.get("max"))
        .and_then(|m| m.get("x"))
        .and_then(|max_x| {
            tile.get("rect")
                .and_then(|r| r.get("min"))
                .and_then(|m| m.get("x"))
                .and_then(|min_x| Some(max_x.as_f64()? - min_x.as_f64()?))
        })
        .unwrap_or(0.0);
    assert!(width > 0.0, "tile rect must have non-zero width");
}

#[test]
fn engine_snapshot_exposes_servo_runtime_channels() {
    let mut harness = TestRegistry::new();

    harness
        .diagnostics
        .emit_message_sent_for_tests("servo.delegate.url_changed", 1);
    harness
        .diagnostics
        .emit_message_received_for_tests("servo.event_loop.spin", 42);

    let snapshot = harness.snapshot();
    let channels = TestRegistry::all_channels(&snapshot);

    assert!(channels.get("servo.delegate.url_changed").copied().unwrap_or(0) > 0);
    assert!(channels.get("servo.event_loop.spin").copied().unwrap_or(0) > 0);
}
