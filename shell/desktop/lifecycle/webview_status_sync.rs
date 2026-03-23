/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use servo::LoadStatus;

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::ui::gui_state::{
    FocusedContentDownloadState, FocusedContentFeatureSupport, FocusedContentMediaState,
    FocusedContentStatus,
};

const FOCUSED_CONTENT_STOP_LOAD_SUPPORTED: bool = false;

pub(crate) fn focused_content_status(
    focused_node_key: Option<NodeKey>,
    graph_app: &GraphBrowserApp,
    window: &EmbedderWindow,
) -> FocusedContentStatus {
    let renderer_id =
        focused_node_key.and_then(|node_key| graph_app.get_webview_for_node(node_key));
    let Some(renderer_id) = renderer_id else {
        return FocusedContentStatus::unavailable(focused_node_key, None);
    };
    let Some(webview) = window.webview_by_id(renderer_id) else {
        return FocusedContentStatus::unavailable(focused_node_key, Some(renderer_id));
    };

    let load_status = webview.load_status();
    FocusedContentStatus {
        node_key: focused_node_key,
        renderer_id: Some(renderer_id),
        current_url: webview.url().map(|url| url.to_string()),
        load_status,
        status_text: webview.status_text(),
        can_go_back: webview.can_go_back(),
        can_go_forward: webview.can_go_forward(),
        can_stop_load: FOCUSED_CONTENT_STOP_LOAD_SUPPORTED && load_status != LoadStatus::Complete,
        find_in_page: FocusedContentFeatureSupport::Unsupported,
        content_zoom_level: Some(webview.page_zoom()),
        media_state: FocusedContentMediaState::Unsupported,
        download_state: FocusedContentDownloadState::Unsupported,
    }
}

pub(crate) fn update_location_in_toolbar(
    location_dirty: bool,
    location: &mut String,
    has_node_panes: bool,
    selected_node_url: Option<String>,
    focused_node_key: Option<NodeKey>,
    graph_app: &GraphBrowserApp,
    window: &EmbedderWindow,
) -> bool {
    if location_dirty {
        return false;
    }

    if !has_node_panes {
        if let Some(url) = selected_node_url.as_ref()
            && *url != *location
        {
            *location = url.clone();
            return true;
        }
        if selected_node_url.is_none() && !location.is_empty() {
            location.clear();
            return true;
        }
        return false;
    }

    if focused_node_key.is_none() {
        if !location.is_empty() {
            location.clear();
            return true;
        }
        return false;
    }

    let status = focused_content_status(focused_node_key, graph_app, window);
    match status.current_url {
        Some(new_location) if new_location != *location => {
            *location = new_location;
            true
        }
        _ => false,
    }
}

pub(crate) fn update_load_status(
    load_status: &mut LoadStatus,
    location_dirty: &mut bool,
    focused_content_status: &FocusedContentStatus,
) -> bool {
    let old_status = std::mem::replace(load_status, focused_content_status.load_status);
    let status_changed = old_status != *load_status;

    if status_changed {
        *location_dirty = false;
    }

    status_changed
}

pub(crate) fn update_status_text(
    status_text: &mut Option<String>,
    focused_content_status: &FocusedContentStatus,
) -> bool {
    let old_status = std::mem::replace(status_text, focused_content_status.status_text.clone());
    old_status != *status_text
}

pub(crate) fn update_can_go_back_and_forward(
    can_go_back: &mut bool,
    can_go_forward: &mut bool,
    focused_content_status: &FocusedContentStatus,
) -> bool {
    let state_can_go_back = focused_content_status.can_go_back;
    let state_can_go_forward = focused_content_status.can_go_forward;

    let can_go_back_changed = *can_go_back != state_can_go_back;
    let can_go_forward_changed = *can_go_forward != state_can_go_forward;
    *can_go_back = state_can_go_back;
    *can_go_forward = state_can_go_forward;
    can_go_back_changed || can_go_forward_changed
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use super::*;
    use crate::prefs::AppPreferences;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use euclid::default::Point2D;
    use servo::WebViewId;

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }

    #[test]
    fn focused_content_status_defaults_to_unavailable_without_focus() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        let app = GraphBrowserApp::new_for_testing();

        let status = focused_content_status(None, &app, &window);

        assert_eq!(status, FocusedContentStatus::unavailable(None, None));
        assert!(!status.live_content_active());
    }

    #[test]
    fn focused_content_status_preserves_mapping_when_webview_is_not_live() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, node_key);

        let status = focused_content_status(Some(node_key), &app, &window);

        assert_eq!(status.node_key, Some(node_key));
        assert_eq!(status.renderer_id, Some(webview_id));
        assert_eq!(status.load_status, LoadStatus::Complete);
        assert_eq!(status.content_zoom_level, None);
        assert_eq!(
            status.find_in_page,
            FocusedContentFeatureSupport::Unsupported
        );
        assert_eq!(status.media_state, FocusedContentMediaState::Unsupported);
        assert_eq!(
            status.download_state,
            FocusedContentDownloadState::Unsupported
        );
        assert!(status.live_content_active());
    }

    #[test]
    fn update_load_status_resets_dirty_when_snapshot_changes() {
        let status = FocusedContentStatus {
            load_status: LoadStatus::Started,
            can_stop_load: true,
            ..FocusedContentStatus::unavailable(None, None)
        };
        let mut load_status = LoadStatus::Complete;
        let mut location_dirty = true;

        let changed = update_load_status(&mut load_status, &mut location_dirty, &status);

        assert!(changed);
        assert_eq!(load_status, LoadStatus::Started);
        assert!(!location_dirty);
    }
}
