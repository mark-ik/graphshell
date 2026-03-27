/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::lifecycle::webview_status_sync;
use crate::shell::desktop::ui::nav_targeting;

pub(super) fn update_location_in_toolbar(
    graph_app: &GraphBrowserApp,
    toolbar_state: &mut ToolbarState,
    has_node_panes: bool,
    focused_node_key: Option<NodeKey>,
    window: &EmbedderWindow,
) -> bool {
    let chrome_projection_node =
        nav_targeting::chrome_projection_node(graph_app, window).or(focused_node_key);
    webview_status_sync::update_location_in_toolbar(
        toolbar_state.location_dirty,
        &mut toolbar_state.location,
        has_node_panes,
        selected_node_url_for_toolbar(graph_app),
        chrome_projection_node,
        graph_app,
        window,
    )
}

pub(super) fn sync_toolbar_webview_status_fields(
    toolbar_state: &mut ToolbarState,
    focused_node_key: Option<NodeKey>,
    graph_app: &GraphBrowserApp,
    window: &EmbedderWindow,
) -> bool {
    let chrome_projection_node =
        nav_targeting::chrome_projection_node(graph_app, window).or(focused_node_key);
    let focused_content_status =
        webview_status_sync::focused_content_status(chrome_projection_node, graph_app, window);
    let load_status_changed = sync_toolbar_load_status(toolbar_state, &focused_content_status);
    let status_text_changed = sync_toolbar_status_text(toolbar_state, &focused_content_status);
    let nav_state_changed = sync_toolbar_navigation_state(toolbar_state, &focused_content_status);

    load_status_changed | status_text_changed | nav_state_changed
}

fn sync_toolbar_load_status(
    toolbar_state: &mut ToolbarState,
    focused_content_status: &crate::shell::desktop::ui::gui_state::FocusedContentStatus,
) -> bool {
    webview_status_sync::update_load_status(
        &mut toolbar_state.load_status,
        &mut toolbar_state.location_dirty,
        focused_content_status,
    )
}

fn sync_toolbar_status_text(
    toolbar_state: &mut ToolbarState,
    focused_content_status: &crate::shell::desktop::ui::gui_state::FocusedContentStatus,
) -> bool {
    webview_status_sync::update_status_text(&mut toolbar_state.status_text, focused_content_status)
}

fn sync_toolbar_navigation_state(
    toolbar_state: &mut ToolbarState,
    focused_content_status: &crate::shell::desktop::ui::gui_state::FocusedContentStatus,
) -> bool {
    webview_status_sync::update_can_go_back_and_forward(
        &mut toolbar_state.can_go_back,
        &mut toolbar_state.can_go_forward,
        focused_content_status,
    )
}

fn selected_node_url_for_toolbar(graph_app: &GraphBrowserApp) -> Option<String> {
    graph_app
        .get_single_selected_node()
        .and_then(|key| selected_node_url(graph_app, key))
}

fn selected_node_url(graph_app: &GraphBrowserApp, key: NodeKey) -> Option<String> {
    node_url_in_workspace_graph(graph_app.domain_graph(), key)
}

fn node_url_in_workspace_graph(graph: &crate::graph::Graph, key: NodeKey) -> Option<String> {
    graph.get_node(key).map(|node| clone_node_url(node.url()))
}

fn clone_node_url(url: &str) -> String {
    url.to_owned()
}
