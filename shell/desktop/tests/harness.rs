use std::collections::HashMap;

use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
use egui_tiles::{Tiles, Tree};
use euclid::Point2D;
use serde_json::Value;

use crate::app::VisibleNavigationRegionSet;
use crate::app::{GraphBrowserApp, GraphViewId};
use crate::graph::NodeKey;
use crate::shell::desktop::runtime::diagnostics::{
    CompositorFrameSample, CompositorTileSample, DiagnosticsState, HierarchySample,
};
use crate::shell::desktop::workbench::pane_model::GraphPaneRef;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::{self, TileOpenMode};
use crate::shell::desktop::workbench::ux_bridge::{
    self, UxBridgeError, UxBridgeResponse, UxDriver, UxNodeSelector,
};
use crate::shell::desktop::workbench::ux_tree::{self, UxAction, UxSemanticNode, UxTreeSnapshot};

pub(crate) struct TestRegistry {
    pub(crate) app: GraphBrowserApp,
    pub(crate) diagnostics: DiagnosticsState,
    pub(crate) tiles_tree: Tree<TileKind>,
    pub(crate) graph_tree: graph_tree::GraphTree<NodeKey>,
    frame_sequence: u64,
}

impl TestRegistry {
    pub(crate) fn new() -> Self {
        let mut tiles = Tiles::default();
        let graph_tile =
            tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let tiles_tree = Tree::new("test_registry_tree", graph_tile, tiles);

        Self {
            app: GraphBrowserApp::new_for_testing(),
            diagnostics: DiagnosticsState::new(),
            tiles_tree,
            graph_tree: graph_tree::GraphTree::new(
                graph_tree::LayoutMode::TreeStyleTabs,
                graph_tree::ProjectionLens::Traversal,
            ),
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
            &self.app,
            key,
            TileOpenMode::Tab,
        );
    }

    pub(crate) fn open_graph_tab(&mut self, view_id: GraphViewId) {
        tile_view_ops::open_or_focus_graph_pane_with_mode(
            &mut self.tiles_tree,
            view_id,
            TileOpenMode::Tab,
        );
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn open_tool_tab(
        &mut self,
        kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
    ) {
        tile_view_ops::open_or_focus_tool_pane(&mut self.tiles_tree, kind);
    }

    pub(crate) fn ux_snapshot_via_driver(&mut self) -> Result<UxTreeSnapshot, UxBridgeError> {
        match self.dispatch_ux_driver_script(&UxDriver::get_ux_snapshot_script())? {
            UxBridgeResponse::Snapshot(snapshot) => Ok(snapshot),
            other => Err(UxBridgeError::invalid_transport_payload(format!(
                "expected Snapshot response, got {other:?}"
            ))),
        }
    }

    pub(crate) fn ux_find_node_via_driver(
        &mut self,
        selector: &UxNodeSelector,
    ) -> Result<Option<UxSemanticNode>, UxBridgeError> {
        match self.dispatch_ux_driver_script(&UxDriver::find_ux_node_script(selector))? {
            UxBridgeResponse::Node(node) => Ok(node),
            other => Err(UxBridgeError::invalid_transport_payload(format!(
                "expected Node response, got {other:?}"
            ))),
        }
    }

    pub(crate) fn ux_focus_path_via_driver(&mut self) -> Result<Vec<String>, UxBridgeError> {
        match self.dispatch_ux_driver_script(&UxDriver::get_focus_path_script())? {
            UxBridgeResponse::FocusPath(path) => Ok(path),
            other => Err(UxBridgeError::invalid_transport_payload(format!(
                "expected FocusPath response, got {other:?}"
            ))),
        }
    }

    pub(crate) fn ux_invoke_action_via_driver(
        &mut self,
        selector: &UxNodeSelector,
        action: UxAction,
    ) -> Result<UxBridgeResponse, UxBridgeError> {
        self.dispatch_ux_driver_script(&UxDriver::invoke_ux_action_script(selector, action))
    }

    pub(crate) fn map_test_webview(&mut self, key: NodeKey) {
        let _ = self.map_test_webview_with_id(key);
    }

    pub(crate) fn map_test_webview_with_id(&mut self, key: NodeKey) -> crate::app::RendererId {
        let webview_id = test_webview_id();
        self.app.map_webview_to_node(webview_id, key);
        webview_id
    }

    pub(crate) fn step_with_tile_sample(
        &mut self,
        key: NodeKey,
        mapped_webview: bool,
        has_context: bool,
        rect: egui::Rect,
    ) {
        let hierarchy = vec![HierarchySample {
            line: format!("* Tile Node Viewer {:?}", key),
            node_key: Some(key),
        }];
        let tiles = vec![CompositorTileSample {
            pane_id: format!("pane:{key:?}"),
            node_key: key,
            render_mode:
                crate::shell::desktop::workbench::pane_model::TileRenderMode::CompositedTexture,
            estimated_content_bytes: 0,
            rect,
            mapped_webview,
            has_context,
            paint_callback_registered: mapped_webview && has_context,
            render_path_hint: if mapped_webview && has_context {
                "composited"
            } else if mapped_webview {
                "missing-context"
            } else {
                "unmapped-node-viewer"
            },
        }];

        self.step_with_frame_sample(1, mapped_webview, rect, hierarchy, tiles);
    }

    pub(crate) fn step_with_frame_sample(
        &mut self,
        active_tile_count: usize,
        focused_node_present: bool,
        content_rect: egui::Rect,
        hierarchy: Vec<HierarchySample>,
        tiles: Vec<CompositorTileSample>,
    ) {
        self.diagnostics.push_frame(CompositorFrameSample {
            sequence: self.frame_sequence,
            active_tile_count,
            focused_node_present,
            content_rect,
            visible_regions: VisibleNavigationRegionSet::singleton(content_rect),
            occluding_host_rects: Vec::new(),
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

    fn dispatch_ux_driver_script(
        &mut self,
        script: &str,
    ) -> Result<UxBridgeResponse, UxBridgeError> {
        let payload = script
            .strip_prefix(ux_bridge::WEBDRIVER_SCRIPT_PREFIX)
            .ok_or_else(|| {
                UxBridgeError::invalid_transport_payload(
                    "UxDriver script did not use the reserved webdriver prefix.",
                )
            })?;
        let command = ux_bridge::parse_transport_command(payload)?;

        match command {
            ux_bridge::UxBridgeCommand::InvokeUxAction { .. } => ux_bridge::handle_runtime_command(
                &mut self.app,
                &mut self.tiles_tree,
                &mut self.graph_tree,
                command,
            ),
            _ => {
                let snapshot = ux_tree::build_snapshot(&self.tiles_tree, &self.app, 0);
                ux_tree::publish_snapshot(&snapshot);
                ux_bridge::handle_latest_snapshot_command(command)
            }
        }
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
