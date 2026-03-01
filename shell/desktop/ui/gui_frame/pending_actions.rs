/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn run_post_render_pending_actions(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    let mut pipeline = PendingPostRenderActionPipeline {
        graph_app,
        window,
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        webview_creation_backpressure,
        focused_node_hint,
    };
    run_pending_post_render_action_pipeline(&mut pipeline);
}

struct PendingPostRenderActionPipeline<'a> {
    graph_app: &'a mut GraphBrowserApp,
    window: &'a EmbedderWindow,
    tiles_tree: &'a mut Tree<TileKind>,
    tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &'a mut Option<NodeKey>,
}

#[derive(Copy, Clone)]
enum PendingPostRenderStage {
    FrameSnapshot,
    GraphSnapshot,
    WorkspaceLayout,
}

const PENDING_POST_RENDER_STAGE_SEQUENCE: [PendingPostRenderStage; 3] = [
    PendingPostRenderStage::FrameSnapshot,
    PendingPostRenderStage::GraphSnapshot,
    PendingPostRenderStage::WorkspaceLayout,
];

fn run_pending_post_render_action_pipeline(pipeline: &mut PendingPostRenderActionPipeline<'_>) {
    for stage in PENDING_POST_RENDER_STAGE_SEQUENCE {
        run_pending_post_render_stage(pipeline, stage);
    }
}

fn run_pending_post_render_stage(
    pipeline: &mut PendingPostRenderActionPipeline<'_>,
    stage: PendingPostRenderStage,
) {
    match stage {
        PendingPostRenderStage::FrameSnapshot => run_pending_frame_snapshot_stage(pipeline),
        PendingPostRenderStage::GraphSnapshot => run_pending_graph_snapshot_stage(pipeline),
        PendingPostRenderStage::WorkspaceLayout => run_pending_workspace_layout_stage(pipeline),
    }
}

fn run_pending_frame_snapshot_stage(pipeline: &mut PendingPostRenderActionPipeline<'_>) {
    handle_pending_frame_snapshot_actions(pipeline.graph_app, pipeline.tiles_tree);
}

fn run_pending_graph_snapshot_stage(pipeline: &mut PendingPostRenderActionPipeline<'_>) {
    handle_pending_graph_snapshot_actions(
        pipeline.graph_app,
        pipeline.window,
        pipeline.tiles_tree,
        pipeline.tile_rendering_contexts,
        pipeline.tile_favicon_textures,
        pipeline.webview_creation_backpressure,
        pipeline.focused_node_hint,
    );
}

fn run_pending_workspace_layout_stage(pipeline: &mut PendingPostRenderActionPipeline<'_>) {
    handle_pending_detach_node_to_split(pipeline.graph_app, pipeline.tiles_tree);
    handle_pending_open_connected_from(pipeline.graph_app, pipeline.tiles_tree);
    handle_pending_history_frame_restore(pipeline.graph_app, pipeline.tiles_tree);
    autosave_session_workspace_layout_if_allowed(pipeline.graph_app, pipeline.tiles_tree);
}
