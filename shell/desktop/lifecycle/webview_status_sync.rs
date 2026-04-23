/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use graphshell_core::content::{ContentLoadState, ViewerInstanceId};
use servo::{LoadStatus, WebViewId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::app::{GraphBrowserApp, RendererId};
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::ui::gui_state::{
    FocusedContentDownloadState, FocusedContentFeatureSupport, FocusedContentMediaState,
    FocusedContentStatus,
};

const FOCUSED_CONTENT_STOP_LOAD_SUPPORTED: bool = false;

static NEXT_RENDERER_ID: AtomicU64 = AtomicU64::new(1);
static SERVO_RENDERER_IDS: OnceLock<Mutex<HashMap<WebViewId, RendererId>>> = OnceLock::new();
static RENDERER_SERVO_IDS: OnceLock<Mutex<HashMap<RendererId, WebViewId>>> = OnceLock::new();

fn servo_renderer_ids() -> &'static Mutex<HashMap<WebViewId, RendererId>> {
    SERVO_RENDERER_IDS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn renderer_servo_ids() -> &'static Mutex<HashMap<RendererId, WebViewId>> {
    RENDERER_SERVO_IDS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Convert servo's `LoadStatus` to the portable `ContentLoadState`.
///
/// This is the sole boundary where `servo::LoadStatus` enters
/// shell-side state; everywhere else the shell uses `ContentLoadState`
/// so the toolbar / view-model / future iced host all see the same
/// vocabulary regardless of which content engine is backing a pane.
///
/// The mapping is 1:1 because both enums enumerate the same three
/// checkpoints on the `document.readyState` lifecycle.
pub(crate) fn content_load_state_from_servo(status: LoadStatus) -> ContentLoadState {
    match status {
        LoadStatus::Started => ContentLoadState::Started,
        LoadStatus::HeadParsed => ContentLoadState::HeadParsed,
        LoadStatus::Complete => ContentLoadState::Complete,
    }
}

/// Convert a `servo::WebViewId` to the portable `ViewerInstanceId`.
///
/// Uses `serde_json` for a stable, deterministic encoding. `WebViewId`'s
/// internal fields are private to the servo crate, so direct byte
/// extraction isn't available from the outside; JSON round-trips
/// cleanly because `WebViewId` derives `Serialize`/`Deserialize`.
///
/// Per-op cost is a small `String` allocation. Fine for the low-
/// frequency sites that track viewer identity in the shell state
/// (thumbnail in-flight, focus target, pending context-surface
/// requests, embedded-content focus). If a hotspot emerges, swap this
/// encoding for a host-side `WebViewId ↔ u64` registry without
/// changing the `ViewerInstanceId` public API.
pub(crate) fn viewer_instance_id_from_servo(id: WebViewId) -> ViewerInstanceId {
    ViewerInstanceId::Servo(
        serde_json::to_string(&id).expect("servo::WebViewId is serde-serializable"),
    )
}

pub(crate) fn renderer_id_from_servo(id: WebViewId) -> RendererId {
    if let Some(existing) = servo_renderer_ids()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&id)
        .copied()
    {
        return existing;
    }

    let renderer_id = RendererId::from_raw(NEXT_RENDERER_ID.fetch_add(1, Ordering::Relaxed));
    servo_renderer_ids()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(id, renderer_id);
    renderer_servo_ids()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(renderer_id, id);
    renderer_id
}

pub(crate) fn viewer_instance_id_from_renderer(id: RendererId) -> Option<ViewerInstanceId> {
    servo_webview_id_from_renderer(id).map(viewer_instance_id_from_servo)
}

pub(crate) fn servo_webview_id_from_renderer(id: RendererId) -> Option<WebViewId> {
    renderer_servo_ids()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&id)
        .copied()
}

pub(crate) fn forget_renderer_id_for_servo(id: WebViewId) -> Option<RendererId> {
    let renderer_id = servo_renderer_ids()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&id)?;
    renderer_servo_ids()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&renderer_id);
    Some(renderer_id)
}

/// Inverse of [`viewer_instance_id_from_servo`]. Returns `None` when
/// the id was produced by a different provider (Wry / iced_webview /
/// MiddleNet Direct) or when the encoded string has been corrupted.
pub(crate) fn servo_webview_id_from_viewer_instance(
    id: &ViewerInstanceId,
) -> Option<WebViewId> {
    match id {
        ViewerInstanceId::Servo(encoded) => serde_json::from_str(encoded).ok(),
        _ => None,
    }
}

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
    let Some(webview) = window.webview_by_renderer_id(renderer_id) else {
        return FocusedContentStatus::unavailable(
            focused_node_key,
            viewer_instance_id_from_renderer(renderer_id),
        );
    };

    let load_status = content_load_state_from_servo(webview.load_status());
    FocusedContentStatus {
        node_key: focused_node_key,
        renderer_id: viewer_instance_id_from_renderer(renderer_id),
        current_url: webview.url().map(|url| url.to_string()),
        load_status,
        status_text: webview.status_text(),
        can_go_back: webview.can_go_back(),
        can_go_forward: webview.can_go_forward(),
        can_stop_load: FOCUSED_CONTENT_STOP_LOAD_SUPPORTED && !load_status.is_complete(),
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
    load_status: &mut ContentLoadState,
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
        app.map_webview_to_node(renderer_id_from_servo(webview_id), node_key);

        let status = focused_content_status(Some(node_key), &app, &window);

        assert_eq!(status.node_key, Some(node_key));
        assert_eq!(
            status.renderer_id,
            Some(viewer_instance_id_from_servo(webview_id))
        );
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
            load_status: ContentLoadState::Started,
            can_stop_load: true,
            ..FocusedContentStatus::unavailable(None, None)
        };
        let mut load_status = ContentLoadState::Complete;
        let mut location_dirty = true;

        let changed = update_load_status(&mut load_status, &mut location_dirty, &status);

        assert!(changed);
        assert_eq!(load_status, ContentLoadState::Started);
        assert!(!location_dirty);
    }

    #[test]
    fn content_load_state_from_servo_maps_all_three_variants() {
        assert_eq!(
            content_load_state_from_servo(LoadStatus::Started),
            ContentLoadState::Started
        );
        assert_eq!(
            content_load_state_from_servo(LoadStatus::HeadParsed),
            ContentLoadState::HeadParsed
        );
        assert_eq!(
            content_load_state_from_servo(LoadStatus::Complete),
            ContentLoadState::Complete
        );
    }

    #[test]
    fn viewer_instance_id_round_trips_through_servo_encoding() {
        use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        let original = WebViewId::new(PainterId::next());

        let portable = viewer_instance_id_from_servo(original);
        let decoded = servo_webview_id_from_viewer_instance(&portable)
            .expect("portable ViewerInstanceId round-trips through servo");
        assert_eq!(original, decoded);
    }

    #[test]
    fn servo_webview_id_from_non_servo_variant_returns_none() {
        assert!(servo_webview_id_from_viewer_instance(
            &graphshell_core::content::ViewerInstanceId::Wry(42)
        )
        .is_none());
        assert!(servo_webview_id_from_viewer_instance(
            &graphshell_core::content::ViewerInstanceId::IcedWebview(99)
        )
        .is_none());
    }
}
