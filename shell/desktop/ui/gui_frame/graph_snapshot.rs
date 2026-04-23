/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::lifecycle::webview_controller;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_pending_graph_snapshot_actions(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    handle_pending_graph_snapshot_save_delete(graph_app);
    handle_pending_graph_snapshot_restore(
        graph_app,
        window,
        tiles_tree,
        viewer_surfaces,
        viewer_surface_host,
        tile_favicon_textures,
        webview_creation_backpressure,
        focused_node_hint,
    );
}

fn handle_pending_graph_snapshot_save_delete(graph_app: &mut GraphBrowserApp) {
    handle_pending_save_graph_snapshot_named(graph_app);
    handle_pending_delete_graph_snapshot_named(graph_app);
}

fn handle_pending_save_graph_snapshot_named(graph_app: &mut GraphBrowserApp) {
    if let Some(name) = graph_app.take_pending_save_graph_snapshot_named()
        && let Err(e) = graph_app.save_named_graph_snapshot(&name)
    {
        warn!("Failed to save named graph snapshot '{name}': {e}");
    }
}

fn handle_pending_delete_graph_snapshot_named(graph_app: &mut GraphBrowserApp) {
    if let Some(name) = graph_app.take_pending_delete_graph_snapshot_named()
        && let Err(e) = graph_app.delete_named_graph_snapshot(&name)
    {
        warn!("Failed to delete named graph snapshot '{name}': {e}");
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_pending_graph_snapshot_restore(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    call_pending_named_graph_snapshot_restore(
        graph_app,
        window,
        tiles_tree,
        viewer_surfaces,
        viewer_surface_host,
        tile_favicon_textures,
        webview_creation_backpressure,
        focused_node_hint,
    );

    call_pending_latest_graph_snapshot_restore(
        graph_app,
        window,
        tiles_tree,
        viewer_surfaces,
        viewer_surface_host,
        tile_favicon_textures,
        webview_creation_backpressure,
        focused_node_hint,
    );
}

#[allow(clippy::too_many_arguments)]
fn call_pending_named_graph_snapshot_restore(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    if let Some(name) = graph_app.take_pending_restore_graph_snapshot_named() {
        restore_pending_named_graph_snapshot(
            graph_app,
            window,
            tiles_tree,
            viewer_surfaces,
            viewer_surface_host,
            tile_favicon_textures,
            webview_creation_backpressure,
            focused_node_hint,
            &name,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn call_pending_latest_graph_snapshot_restore(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    if graph_app.take_pending_restore_graph_snapshot_latest() {
        restore_pending_latest_graph_snapshot(
            graph_app,
            window,
            tiles_tree,
            viewer_surfaces,
            viewer_surface_host,
            tile_favicon_textures,
            webview_creation_backpressure,
            focused_node_hint,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn restore_pending_named_graph_snapshot(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
    name: &str,
) {
    restore_graph_snapshot_and_reset_workspace(
        graph_app,
        window,
        tiles_tree,
        viewer_surfaces,
        viewer_surface_host,
        tile_favicon_textures,
        webview_creation_backpressure,
        focused_node_hint,
        |graph_app| load_named_graph_snapshot_result(graph_app, name),
        |e| warn!("Failed to load named graph snapshot '{name}': {e}"),
    );
}

fn load_named_graph_snapshot_result(
    graph_app: &mut GraphBrowserApp,
    name: &str,
) -> Result<(), String> {
    graph_app
        .load_named_graph_snapshot(name)
        .map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
fn restore_pending_latest_graph_snapshot(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    restore_graph_snapshot_and_reset_workspace(
        graph_app,
        window,
        tiles_tree,
        viewer_surfaces,
        viewer_surface_host,
        tile_favicon_textures,
        webview_creation_backpressure,
        focused_node_hint,
        load_latest_graph_snapshot_result,
        |e| warn!("Failed to load autosaved latest graph snapshot: {e}"),
    );
}

fn load_latest_graph_snapshot_result(graph_app: &mut GraphBrowserApp) -> Result<(), String> {
    graph_app
        .load_latest_graph_snapshot()
        .map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
fn restore_graph_snapshot_and_reset_workspace(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
    restore: impl FnOnce(&mut GraphBrowserApp) -> Result<(), String>,
    on_error: impl FnOnce(&str),
) {
    record_workspace_undo_boundary_from_tiles_tree(
        graph_app,
        tiles_tree,
        UndoBoundaryReason::RestoreGraphSnapshot,
    );
    close_all_webviews_and_apply_intents(graph_app, window, tiles_tree);
    apply_graph_snapshot_restore_result(
        restore(graph_app),
        tiles_tree,
        viewer_surfaces,
        viewer_surface_host,
        tile_favicon_textures,
        webview_creation_backpressure,
        focused_node_hint,
        on_error,
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_graph_snapshot_restore_result(
    restore_result: Result<(), String>,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
    on_error: impl FnOnce(&str),
) {
    match restore_result {
        Ok(()) => {
            reset_graph_workspace_after_snapshot_restore(
                tiles_tree,
                viewer_surfaces,
                viewer_surface_host,
                tile_favicon_textures,
                webview_creation_backpressure,
                focused_node_hint,
            );
        }
        Err(e) => on_error(e.as_str()),
    }
}

fn close_all_webviews_and_apply_intents(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
) {
    let mut close_intents = webview_controller::close_all_webviews(graph_app, window);
    apply_intents_if_any(graph_app, tiles_tree, &mut close_intents);
}

pub(super) fn reset_graph_workspace_after_snapshot_restore(
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    let previous_focus_hint = *focused_node_hint;
    let surface_keys: Vec<_> = viewer_surfaces.keys().copied().collect();
    for node_key in surface_keys {
        viewer_surface_host.retire_surface(viewer_surfaces, node_key);
    }
    tile_favicon_textures.clear();
    webview_creation_backpressure.clear();
    *focused_node_hint = None;
    if previous_focus_hint != *focused_node_hint {
        diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
    let mut tiles = Tiles::default();
    let graph_tile_id = tiles.insert_pane(TileKind::Graph(
        crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(GraphViewId::default()),
    ));
    *tiles_tree = Tree::new("graphshell_tiles", graph_tile_id, tiles);
}
