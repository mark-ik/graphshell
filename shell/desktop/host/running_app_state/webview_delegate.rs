/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! WebView delegate implementation for [`RunningAppState`].
//!
//! Separated from the coordinator so the callback surface and orchestration logic
//! are legible in isolation.

use std::ops::Deref;
use std::rc::Rc;

use servo::{
    AuthenticationRequest, BluetoothDeviceSelectionRequest, ConsoleLogLevel,
    CreateNewWebViewRequest, DeviceIntPoint, DeviceIntSize, EmbedderControl, EmbedderControlId,
    InputEventId, InputEventResult, LoadStatus, MediaSessionEvent, PermissionRequest, TraversalId,
    WebDriverLoadStatus, WebView, WebViewDelegate,
};
use url::Url;

use crate::app::OpenSurfaceSource;

use super::RunningAppState;

pub(super) struct RunningAppStateWebViewDelegate {
    pub(super) state: Rc<RunningAppState>,
}

impl Deref for RunningAppStateWebViewDelegate {
    type Target = RunningAppState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl WebViewDelegate for RunningAppStateWebViewDelegate {
    fn screen_geometry(&self, webview: WebView) -> Option<servo::ScreenGeometry> {
        Some(
            self.platform_window_for_webview_id(webview.id())
                .screen_geometry(),
        )
    }

    fn notify_status_text_changed(&self, webview: WebView, _status: Option<String>) {
        self.window_for_webview_id(webview.id()).set_needs_update();
    }

    fn notify_url_changed(&self, webview: WebView, url: Url) {
        let window = self.window_for_webview_id(webview.id());
        window.notify_url_changed(webview, url);
    }

    fn notify_history_changed(&self, webview: WebView, entries: Vec<Url>, current: usize) {
        let window = self.window_for_webview_id(webview.id());
        window.notify_history_changed(webview, entries, current);
    }

    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        let window = self.window_for_webview_id(webview.id());
        window.notify_page_title_changed(webview, title);
    }

    fn notify_traversal_complete(&self, _webview: WebView, traversal_id: TraversalId) {
        self.webdriver.complete_traversal(traversal_id);
    }

    fn request_move_to(&self, webview: WebView, new_position: DeviceIntPoint) {
        self.platform_window_for_webview_id(webview.id())
            .set_position(new_position);
    }

    fn request_resize_to(&self, webview: WebView, requested_outer_size: DeviceIntSize) {
        self.platform_window_for_webview_id(webview.id())
            .request_resize(&webview, requested_outer_size);
    }

    fn request_authentication(
        &self,
        webview: WebView,
        authentication_request: AuthenticationRequest,
    ) {
        self.window_for_webview_id(webview.id())
            .show_http_authentication_dialog(webview.id(), authentication_request);
    }

    fn request_create_new(&self, parent_webview: WebView, request: CreateNewWebViewRequest) {
        let window = self.window_for_webview_id(parent_webview.id());
        let token = self.store_pending_create_request(request);
        window.notify_host_open_request(
            "about:blank".into(),
            OpenSurfaceSource::ChildWebview,
            Some(
                crate::shell::desktop::lifecycle::webview_status_sync::renderer_id_from_servo(
                    parent_webview.id(),
                ),
            ),
            Some(token),
        );
    }

    fn notify_closed(&self, webview: WebView) {
        self.window_for_webview_id(webview.id())
            .close_webview(webview.id())
    }

    fn notify_input_event_handled(
        &self,
        webview: WebView,
        id: InputEventId,
        result: InputEventResult,
    ) {
        self.platform_window_for_webview_id(webview.id())
            .notify_input_event_handled(&webview, id, result);
        self.webdriver.finish_input_event(id);
    }

    fn notify_cursor_changed(&self, webview: WebView, cursor: servo::Cursor) {
        self.platform_window_for_webview_id(webview.id())
            .set_cursor(cursor);
    }

    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        let window = self.window_for_webview_id(webview.id());
        window.set_needs_update();

        if status == LoadStatus::Complete {
            window.notify_load_status_complete(webview.clone());
            if let Some(sender) = self.webdriver.take_load_status_sender(webview.id()) {
                let _ = sender.send(WebDriverLoadStatus::Complete);
            }
            self.maybe_request_screenshot(webview);
        }
    }

    fn notify_fullscreen_state_changed(&self, webview: WebView, fullscreen_state: bool) {
        self.platform_window_for_webview_id(webview.id())
            .set_fullscreen(fullscreen_state);
    }

    fn show_bluetooth_device_dialog(
        &self,
        webview: WebView,
        request: BluetoothDeviceSelectionRequest,
    ) {
        self.window_for_webview_id(webview.id())
            .show_bluetooth_device_dialog(webview.id(), request);
    }

    fn request_permission(&self, webview: WebView, permission_request: PermissionRequest) {
        self.window_for_webview_id(webview.id())
            .show_permission_dialog(webview.id(), permission_request);
    }

    fn notify_new_frame_ready(&self, webview: WebView) {
        self.window_for_webview_id(webview.id()).set_needs_repaint();
    }

    fn show_embedder_control(&self, webview: WebView, embedder_control: EmbedderControl) {
        if self.app_preferences.webdriver_port.get().is_some() {
            if matches!(&embedder_control, EmbedderControl::SimpleDialog(..)) {
                self.webdriver.interrupt_script_evaluation();

                // Dialogs block the page load, so need need to notify WebDriver
                self.webdriver.block_load_status_if_any(webview.id());
            }

            self.webdriver
                .show_embedder_control(webview.id(), embedder_control);
            return;
        }

        self.window_for_webview_id(webview.id())
            .show_embedder_control(webview, embedder_control);
    }

    fn hide_embedder_control(&self, webview: WebView, embedder_control_id: EmbedderControlId) {
        if self.app_preferences.webdriver_port.get().is_some() {
            self.webdriver
                .hide_embedder_control(webview.id(), embedder_control_id);
            return;
        }

        self.window_for_webview_id(webview.id())
            .hide_embedder_control(webview, embedder_control_id);
    }

    fn notify_favicon_changed(&self, webview: WebView) {
        self.window_for_webview_id(webview.id())
            .notify_favicon_changed(webview);
    }

    fn notify_media_session_event(&self, webview: WebView, event: MediaSessionEvent) {
        self.platform_window_for_webview_id(webview.id())
            .notify_media_session_event(event);
    }

    fn notify_crashed(&self, webview: WebView, reason: String, backtrace: Option<String>) {
        let window = self.window_for_webview_id(webview.id());
        window.notify_webview_crashed(webview.clone(), reason.clone(), backtrace.clone());
        self.platform_window_for_webview_id(webview.id())
            .notify_crashed(webview, reason, backtrace);
    }

    fn show_console_message(&self, webview: WebView, level: ConsoleLogLevel, message: String) {
        self.platform_window_for_webview_id(webview.id())
            .show_console_message(level, &message);
    }

    fn notify_accessibility_tree_update(
        &self,
        webview: WebView,
        tree_update: servo::accesskit::TreeUpdate,
    ) {
        self.platform_window_for_webview_id(webview.id())
            .notify_accessibility_tree_update(webview, tree_update);
    }
}
