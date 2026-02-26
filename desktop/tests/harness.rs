use std::collections::HashMap;

use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
use egui_tiles::{Tiles, Tree};
use euclid::Point2D;
use serde_json::Value;

use crate::app::{GraphBrowserApp, GraphViewId};
use crate::shell::desktop::runtime::diagnostics::{
    CompositorFrameSample, CompositorTileSample, DiagnosticsState, HierarchySample,
};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::{self, TileOpenMode};
use crate::graph::NodeKey;

pub(crate) struct TestHarness {
    pub(crate) app: GraphBrowserApp,
    pub(crate) diagnostics: DiagnosticsState,
    pub(crate) tiles_tree: Tree<TileKind>,
    frame_sequence: u64,
}

impl TestHarness {
    pub(crate) fn new() -> Self {
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
        let tiles_tree = Tree::new("test_harness_tree", graph_tile, tiles);

        Self {
            app: GraphBrowserApp::new_for_testing(),
            diagnostics: DiagnosticsState::new(),
            tiles_tree,
            frame_sequence: 1,
        }
    }

    pub(crate) fn add_node(&mut self, url: &str) -> NodeKey {
        self.app
            .add_node_and_sync(url.to_string(), Point2D::new(0.0, 0.0))
    }

    pub(crate) fn open_node_tab(&mut self, key: NodeKey) {
        tile_view_ops::open_or_focus_node_pane_with_mode(
            &mut self.tiles_tree,
            key,
            TileOpenMode::Tab,
        );
    }

    pub(crate) fn map_test_webview(&mut self, key: NodeKey) {
        self.app.map_webview_to_node(test_webview_id(), key);
    }

    pub(crate) fn step_with_tile_sample(
        &mut self,
        key: NodeKey,
        mapped_webview: bool,
        has_context: bool,
        rect: egui::Rect,
    ) {
        let hierarchy = vec![HierarchySample {
            line: format!("* Tile WebView {:?}", key),
            node_key: Some(key),
        }];
        let tiles = vec![CompositorTileSample {
            node_key: key,
            rect,
            mapped_webview,
            has_context,
            paint_callback_registered: mapped_webview && has_context,
        }];

        self.step_with_frame_sample(1, mapped_webview, rect, hierarchy, tiles);
    }

    pub(crate) fn step_with_frame_sample(
        &mut self,
        active_tile_count: usize,
        focused_webview_present: bool,
        viewport_rect: egui::Rect,
        hierarchy: Vec<HierarchySample>,
        tiles: Vec<CompositorTileSample>,
    ) {
        self.diagnostics.push_frame(CompositorFrameSample {
            sequence: self.frame_sequence,
            active_tile_count,
            focused_webview_present,
            viewport_rect,
            hierarchy,
            tiles,
        });
        self.frame_sequence = self.frame_sequence.saturating_add(1);

        self.diagnostics.force_drain_for_tests();
    }

    pub(crate) fn snapshot(&mut self) -> Value {
        self.diagnostics.force_drain_for_tests();
        self.diagnostics.snapshot_json_for_tests()
    }

    pub(crate) fn tile_for_node(snapshot: &Value, key: NodeKey) -> Option<Value> {
        let node_index = key.index().to_string();
        let frames = snapshot.get("compositor_frames")?.as_array()?;
        let last = frames.last()?;
        let tiles = last.get("tiles")?.as_array()?;
        for tile in tiles {
            let maybe_key = tile
                .get("node_key")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
                .or_else(|| tile.get("node_key").map(|v| v.to_string()));
            if let Some(text) = maybe_key
                && text.contains(&node_index)
            {
                return Some(tile.clone());
            }
        }
        if tiles.len() == 1 {
            return tiles.first().cloned();
        }
        None
    }

    pub(crate) fn channel_count(snapshot: &Value, channel: &str) -> u64 {
        snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(channel))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    }

    pub(crate) fn all_channels(snapshot: &Value) -> HashMap<String, u64> {
        snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n)))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default()
    }
}

fn test_webview_id() -> servo::WebViewId {
    PIPELINE_NAMESPACE.with(|tls| {
        if tls.get().is_none() {
            PipelineNamespace::install(TEST_NAMESPACE);
        }
    });
    servo::WebViewId::new(PainterId::next())
}
