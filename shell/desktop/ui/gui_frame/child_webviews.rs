/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use log::warn;
use servo::WebViewId;

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::runtime::registries::CHANNEL_SEMANTIC_CREATE_NEW_WEBVIEW_UNMAPPED;

pub(crate) fn open_pending_child_webviews_for_tiles<F>(
    graph_app: &GraphBrowserApp,
    pending_open_child_webviews: Vec<WebViewId>,
    mut open_for_node: F,
) -> Vec<WebViewId>
where
    F: FnMut(NodeKey),
{
    let mut deferred_webviews = Vec::new();
    for child_webview_id in pending_open_child_webviews {
        if let Some(node_key) = graph_app.get_node_for_webview(child_webview_id) {
            open_for_node(node_key);
        } else {
            deferred_webviews.push(child_webview_id);
            warn!(
                "semantic child-webview {:?} had no node mapping; skipping pane-open",
                child_webview_id
            );
            #[cfg(feature = "diagnostics")]
            diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_SEMANTIC_CREATE_NEW_WEBVIEW_UNMAPPED,
                byte_len: 1,
            });
        }
    }
    deferred_webviews
}