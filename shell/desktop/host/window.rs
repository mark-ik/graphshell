/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

mod graph_events;
mod projection;
mod runtime;

use self::graph_events::WindowGraphEventQueue;
use self::projection::WindowProjectionState;
use self::runtime::WindowRuntimeState;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use euclid::Scale;
#[cfg(feature = "wry")]
use raw_window_handle::RawWindowHandle;
use servo::{
    AuthenticationRequest, ConsoleLogLevel, Cursor, DeviceIndependentIntRect,
    DeviceIndependentPixel, DeviceIntPoint, DeviceIntSize, DevicePixel, EmbedderControl,
    EmbedderControlId, InputEventId, InputEventResult, MediaSessionEvent, PermissionRequest,
    RenderingContextCore, ScreenGeometry, Servo, UserContentManager, WebView, WebViewBuilder,
    WebViewDelegate, WebViewId,
};
use url::Url;

use crate::app::{
    HostOpenRequest, OpenSurfaceSource, PendingCreateToken, RendererId, WorkbenchIntent,
};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::lifecycle::webview_status_sync::{
    forget_renderer_id_for_servo, renderer_id_from_servo, servo_webview_id_from_renderer,
};
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics::{self, DiagnosticEvent};
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::workbench::pane_model::PaneId;

pub(crate) trait WebViewCreationContext {
    fn servo(&self) -> &Servo;
    fn user_content_manager(&self) -> Rc<UserContentManager>;
    fn webview_delegate(self: Rc<Self>) -> Rc<dyn WebViewDelegate>;
}

// This should vary by zoom level and maybe actual text size (focused or under cursor)
pub(crate) const LINE_HEIGHT: f32 = 76.0;
pub(crate) const LINE_WIDTH: f32 = 76.0;

/// <https://github.com/web-platform-tests/wpt/blob/9320b1f724632c52929a3fdb11bdaf65eafc7611/webdriver/tests/classic/set_window_rect/set.py#L287-L290>
/// "A window size of 10x10px shouldn't be supported by any browser."
pub(crate) const MIN_WINDOW_INNER_SIZE: DeviceIntSize = DeviceIntSize::new(100, 100);

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub(crate) struct EmbedderWindowId(u64);

impl From<u64> for EmbedderWindowId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Graph-relevant semantic events emitted from Servo delegate callbacks.
#[derive(Clone, Debug)]
pub(crate) struct WebViewLifecycleEvent {
    pub(crate) seq: u64,
    pub(crate) kind: WebViewLifecycleEventKind,
}

#[derive(Clone, Debug)]
pub(crate) enum WebViewLifecycleEventKind {
    UrlChanged {
        webview_id: RendererId,
        new_url: String,
    },
    HistoryChanged {
        webview_id: RendererId,
        entries: Vec<String>,
        current: usize,
    },
    PageTitleChanged {
        webview_id: RendererId,
        title: Option<String>,
    },
    HostOpenRequest {
        request: HostOpenRequest,
    },
    WorkbenchIntentRequested {
        intent: WorkbenchIntent,
    },
    WebViewCrashed {
        webview_id: RendererId,
        reason: String,
        has_backtrace: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputTarget {
    Host,
    Pane(PaneId),
    Renderer(RendererId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChromeProjectionSource {
    Pane(PaneId),
    Renderer(RendererId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogOwner {
    Pane(PaneId),
    Renderer(RendererId),
}

#[derive(Default)]
struct WindowUiSignals {
    pending_favicon_loads: RefCell<Vec<WebViewId>>,
    pending_thumbnail_capture_requests: RefCell<Vec<WebViewId>>,
}

impl WindowUiSignals {
    fn enqueue_favicon_load(&self, webview_id: WebViewId) {
        self.pending_favicon_loads.borrow_mut().push(webview_id);
    }

    fn enqueue_thumbnail_capture_request(&self, webview_id: WebViewId) {
        self.pending_thumbnail_capture_requests
            .borrow_mut()
            .push(webview_id);
    }

    fn take_pending_favicon_loads(&self) -> Vec<WebViewId> {
        std::mem::take(&mut *self.pending_favicon_loads.borrow_mut())
    }

    fn take_pending_thumbnail_capture_requests(&self) -> Vec<WebViewId> {
        std::mem::take(&mut *self.pending_thumbnail_capture_requests.borrow_mut())
    }
}

pub(crate) struct EmbedderWindow {
    /// A handle to the [`PlatformWindow`] that graphshell is rendering in.
    platform_window: Rc<dyn PlatformWindow>,
    /// Embedder runtime state: owned WebViews and repaint/update/close flags.
    runtime_state: WindowRuntimeState,
    /// Graphshell-side UI signal queues derived from WebView state changes.
    ui_signals: WindowUiSignals,
    /// Graphshell projection state: pane focus, input retargeting, chrome projection,
    /// dialog ownership, and visible pane snapshots.
    projection_state: WindowProjectionState,
    /// Graphshell semantic event queue emitted from Servo delegate callbacks.
    graph_events: WindowGraphEventQueue,
}

impl EmbedderWindow {
    pub(crate) fn new(
        platform_window: Rc<dyn PlatformWindow>,
        graph_event_sequence: Arc<AtomicU64>,
    ) -> Self {
        Self {
            platform_window,
            runtime_state: Default::default(),
            ui_signals: Default::default(),
            projection_state: Default::default(),
            graph_events: WindowGraphEventQueue::new(graph_event_sequence),
        }
    }

    pub(crate) fn id(&self) -> EmbedderWindowId {
        self.platform_window().id()
    }

    /// Returns the raw OS window handle for use with native child-window creation (e.g. wry overlays).
    #[cfg(feature = "wry")]
    pub(crate) fn raw_window_handle_for_child(&self) -> Option<RawWindowHandle> {
        self.platform_window.raw_window_handle_for_child()
    }

    pub(crate) fn create_toplevel_webview<T>(&self, state: Rc<T>, url: Url) -> WebView
    where
        T: WebViewCreationContext + 'static,
    {
        self.create_toplevel_webview_with_context(
            state,
            url,
            self.platform_window.rendering_context(),
        )
    }

    pub(crate) fn create_toplevel_webview_with_context<T>(
        &self,
        state: Rc<T>,
        url: Url,
        rendering_context: Rc<dyn RenderingContextCore>,
    ) -> WebView
    where
        T: WebViewCreationContext + 'static,
    {
        let webview = WebViewBuilder::new(state.servo(), rendering_context)
            .url(url)
            .hidpi_scale_factor(self.platform_window.hidpi_scale_factor())
            .user_content_manager(state.user_content_manager())
            .delegate(state.clone().webview_delegate())
            .build();

        webview.notify_theme_change(self.platform_window.theme());
        self.add_webview(webview.clone());
        webview
    }

    /// Repaint the currently visible node-pane renderers.
    pub(crate) fn repaint_webviews(&self) {
        let visible_renderers = self.visible_renderer_ids();
        if visible_renderers.is_empty() {
            return;
        }

        for webview_id in visible_renderers {
            let Some(webview) = self.webview_by_id(webview_id) else {
                continue;
            };
            webview.render();
        }
    }

    /// Whether or not this [`EmbedderWindow`] should close.
    pub(crate) fn should_close(&self) -> bool {
        self.runtime_state.should_close()
    }

    pub(crate) fn contains_webview(&self, id: WebViewId) -> bool {
        self.runtime_state.contains_webview(id)
    }

    pub(crate) fn webview_by_id(&self, id: WebViewId) -> Option<WebView> {
        self.runtime_state.webview_by_id(id)
    }

    pub(crate) fn webview_by_renderer_id(&self, renderer_id: RendererId) -> Option<WebView> {
        servo_webview_id_from_renderer(renderer_id).and_then(|id| self.webview_by_id(id))
    }

    pub(crate) fn newest_webview_id(&self) -> Option<WebViewId> {
        self.runtime_state.newest_webview_id()
    }

    pub(crate) fn set_needs_update(&self) {
        self.runtime_state.set_needs_update();
    }

    pub(crate) fn set_needs_repaint(&self) {
        self.runtime_state.set_needs_repaint()
    }

    pub(crate) fn schedule_close(&self) {
        self.runtime_state.schedule_close()
    }

    pub(crate) fn platform_window(&self) -> Rc<dyn PlatformWindow> {
        self.platform_window.clone()
    }

    pub(crate) fn focus(&self) {
        self.platform_window.focus()
    }

    pub(crate) fn focused_pane(&self) -> Option<PaneId> {
        self.projection_state.focused_pane()
    }

    pub(crate) fn set_focused_pane(&self, pane_id: Option<PaneId>) {
        self.projection_state.set_focused_pane(pane_id);
    }

    pub(crate) fn input_target(&self) -> Option<InputTarget> {
        self.projection_state.input_target()
    }

    pub(crate) fn set_input_target(&self, target: Option<InputTarget>) {
        self.projection_state.set_input_target(target);
    }

    pub(crate) fn chrome_projection_source(&self) -> Option<ChromeProjectionSource> {
        self.projection_state.chrome_projection_source()
    }

    pub(crate) fn set_chrome_projection_source(&self, source: Option<ChromeProjectionSource>) {
        self.projection_state.set_chrome_projection_source(source);
    }

    pub(crate) fn dialog_owner(&self) -> Option<DialogOwner> {
        self.projection_state.dialog_owner()
    }

    pub(crate) fn set_dialog_owner(&self, owner: Option<DialogOwner>) {
        self.projection_state.set_dialog_owner(owner);
    }

    pub(crate) fn set_visible_node_panes(&self, pane_ids: Vec<PaneId>) {
        self.projection_state.set_visible_node_panes(pane_ids);
    }

    fn visible_renderer_ids(&self) -> Vec<WebViewId> {
        self.projection_state.visible_renderer_ids()
    }

    pub(crate) fn explicit_input_webview_id(&self) -> Option<WebViewId> {
        self.projection_state.explicit_input_webview_id()
    }

    pub(crate) fn targeted_input_webview_id(&self) -> Option<WebViewId> {
        self.projection_state.targeted_input_webview_id()
    }

    /// Resolves the active input target webview, giving priority to an embedded-GUI focus
    /// override (e.g. an egui-hosted webview that has explicit keyboard focus) over the
    /// window's projection-state input target.
    pub(crate) fn resolve_input_webview_id(
        &self,
        embedded_focus: Option<WebViewId>,
    ) -> Option<WebViewId> {
        embedded_focus.or_else(|| self.targeted_input_webview_id())
    }

    pub(crate) fn explicit_dialog_webview_id(&self) -> Option<WebViewId> {
        self.projection_state.explicit_dialog_webview_id()
    }

    pub(crate) fn explicit_chrome_webview_id(&self) -> Option<WebViewId> {
        self.projection_state.explicit_chrome_webview_id()
    }

    pub(crate) fn retarget_input_to_webview(&self, webview_id: WebViewId) {
        self.projection_state
            .sync_explicit_targets_for_webview(webview_id);
        self.set_needs_update();
    }

    pub(crate) fn retarget_input_to_host(&self) {
        self.set_input_target(Some(InputTarget::Host));
        self.set_needs_update();
    }

    fn clear_explicit_targets_for_closed_webview(
        &self,
        webview_id: WebViewId,
        detached_pane_id: Option<PaneId>,
    ) {
        self.projection_state
            .clear_explicit_targets_for_closed_webview(webview_id, detached_pane_id);
    }

    pub(crate) fn add_webview(&self, webview: WebView) {
        self.runtime_state.add_webview(webview);
        self.set_needs_update();
        self.set_needs_repaint();
    }

    pub(crate) fn webview_ids(&self) -> Vec<WebViewId> {
        self.runtime_state.webview_ids()
    }

    /// Returns all [`WebView`]s in creation order.
    pub(crate) fn webviews(&self) -> Vec<(WebViewId, WebView)> {
        self.runtime_state.webviews()
    }

    pub(crate) fn update_and_request_repaint_if_necessary(&self, state: &RunningAppState) {
        let updated_user_interface = self.runtime_state.take_needs_update()
            && self
                .platform_window
                .update_user_interface_state(state, self);

        // Delegate handlers may have asked us to present or update painted WebView contents.
        // Currently, egui-file-dialog dialogs need to be constantly redrawn or animations aren't fluid.
        let needs_repaint = self.runtime_state.take_needs_repaint();
        if updated_user_interface || needs_repaint {
            self.platform_window.request_repaint(self);
        }
    }

    /// Close the given [`WebView`] via its [`WebViewId`].
    ///
    /// Note: This can happen because we can trigger a close with a UI action and then get
    /// the close notification via the [`WebViewDelegate`] later.
    pub(crate) fn close_webview(&self, webview_id: WebViewId) {
        if !self.runtime_state.remove_webview(webview_id) {
            return;
        }
        let detached_attachment =
            registries::phase1_detach_renderer(renderer_id_from_servo(webview_id));
        let _ = forget_renderer_id_for_servo(webview_id);
        self.clear_explicit_targets_for_closed_webview(
            webview_id,
            detached_attachment.map(|attachment| attachment.pane_id),
        );
        self.platform_window
            .dismiss_embedder_controls_for_webview(webview_id);

        self.set_needs_update();
        self.set_needs_repaint();
    }

    pub(crate) fn notify_favicon_changed(&self, webview: WebView) {
        self.ui_signals.enqueue_favicon_load(webview.id());
        self.set_needs_repaint();
    }

    pub(crate) fn notify_load_status_complete(&self, webview: WebView) {
        self.ui_signals
            .enqueue_thumbnail_capture_request(webview.id());
        self.set_needs_repaint();
    }

    pub(crate) fn notify_url_changed(&self, webview: WebView, new_url: Url) {
        let kind = WebViewLifecycleEventKind::UrlChanged {
            webview_id: renderer_id_from_servo(webview.id()),
            new_url: new_url.to_string(),
        };
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "window.graph_event.url_changed",
            byte_len: 1,
        });
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "servo.delegate.url_changed",
            byte_len: 1,
        });
        self.graph_events.enqueue(kind);
        self.set_needs_update();
    }

    pub(crate) fn notify_history_changed(
        &self,
        webview: WebView,
        entries: Vec<Url>,
        current: usize,
    ) {
        let kind = WebViewLifecycleEventKind::HistoryChanged {
            webview_id: renderer_id_from_servo(webview.id()),
            entries: entries.into_iter().map(|u| u.to_string()).collect(),
            current,
        };
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "window.graph_event.history_changed",
            byte_len: 1,
        });
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "servo.delegate.history_changed",
            byte_len: 1,
        });
        self.graph_events.enqueue(kind);
        self.set_needs_update();
    }

    pub(crate) fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        let kind = WebViewLifecycleEventKind::PageTitleChanged {
            webview_id: renderer_id_from_servo(webview.id()),
            title,
        };
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "window.graph_event.title_changed",
            byte_len: 1,
        });
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "servo.delegate.title_changed",
            byte_len: 1,
        });
        self.graph_events.enqueue(kind);
        self.set_needs_update();
    }

    pub(crate) fn notify_host_open_request(
        &self,
        url: String,
        source: OpenSurfaceSource,
        parent_webview_id: Option<RendererId>,
        pending_create_token: Option<PendingCreateToken>,
    ) {
        let kind = WebViewLifecycleEventKind::HostOpenRequest {
            request: HostOpenRequest {
                url,
                source,
                parent_webview_id,
                pending_create_token,
            },
        };
        self.graph_events.enqueue(kind);
        self.set_needs_update();
    }

    pub(crate) fn notify_workbench_intent_request(&self, intent: WorkbenchIntent) {
        self.graph_events
            .enqueue(WebViewLifecycleEventKind::WorkbenchIntentRequested { intent });
        self.set_needs_update();
    }

    pub(crate) fn notify_webview_crashed(
        &self,
        webview: WebView,
        reason: String,
        backtrace: Option<String>,
    ) {
        let kind = WebViewLifecycleEventKind::WebViewCrashed {
            webview_id: renderer_id_from_servo(webview.id()),
            reason,
            has_backtrace: backtrace.as_deref().is_some_and(|b| !b.is_empty()),
        };
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "window.graph_event.webview_crashed",
            byte_len: 1,
        });
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "servo.delegate.webview_crashed",
            byte_len: 1,
        });
        self.graph_events.enqueue(kind);
        self.set_needs_update();
    }

    pub(crate) fn hidpi_scale_factor_changed(&self) {
        let new_scale_factor = self.platform_window.hidpi_scale_factor();
        self.runtime_state
            .for_each_webview(|webview| webview.set_hidpi_scale_factor(new_scale_factor));
    }

    /// Return a list of all webviews that have favicons that have not yet been loaded by egui.
    pub(crate) fn take_pending_favicon_loads(&self) -> Vec<WebViewId> {
        self.ui_signals.take_pending_favicon_loads()
    }

    /// Return webviews that should schedule thumbnail capture.
    pub(crate) fn take_pending_thumbnail_capture_requests(&self) -> Vec<WebViewId> {
        self.ui_signals.take_pending_thumbnail_capture_requests()
    }

    /// Return all pending graph semantic events.
    pub(crate) fn take_pending_graph_events(&self) -> Vec<WebViewLifecycleEvent> {
        self.graph_events.take_pending()
    }

    #[cfg(test)]
    pub(crate) fn enqueue_test_graph_event_kind(&self, kind: WebViewLifecycleEventKind) {
        self.graph_events.enqueue_for_test(kind);
    }

    pub(crate) fn show_embedder_control(
        &self,
        webview: WebView,
        embedder_control: EmbedderControl,
    ) {
        self.set_dialog_owner(Some(
            self.projection_state.dialog_owner_for_webview(webview.id()),
        ));
        self.platform_window
            .show_embedder_control(webview.id(), embedder_control);
        self.set_needs_update();
        self.set_needs_repaint();
    }

    pub(crate) fn show_permission_dialog(
        &self,
        webview_id: WebViewId,
        permission_request: PermissionRequest,
    ) {
        self.set_dialog_owner(Some(
            self.projection_state.dialog_owner_for_webview(webview_id),
        ));
        self.platform_window
            .show_permission_dialog(webview_id, permission_request);
        self.set_needs_update();
        self.set_needs_repaint();
    }

    pub(crate) fn show_http_authentication_dialog(
        &self,
        webview_id: WebViewId,
        authentication_request: AuthenticationRequest,
    ) {
        self.set_dialog_owner(Some(
            self.projection_state.dialog_owner_for_webview(webview_id),
        ));
        self.platform_window
            .show_http_authentication_dialog(webview_id, authentication_request);
        self.set_needs_update();
        self.set_needs_repaint();
    }

    pub(crate) fn hide_embedder_control(
        &self,
        webview: WebView,
        embedder_control: EmbedderControlId,
    ) {
        self.platform_window
            .hide_embedder_control(webview.id(), embedder_control);
        self.set_needs_update();
        self.set_needs_repaint();
    }
}

// ── PlatformWindow capability sub-traits ─────────────────────────────────────

/// Rendering and presentation: geometry, scale, context, and UI lifecycle.
pub(crate) trait PlatformWindowRendering {
    fn id(&self) -> EmbedderWindowId;
    fn screen_geometry(&self) -> ScreenGeometry;
    fn device_hidpi_scale_factor(&self) -> Scale<f32, DeviceIndependentPixel, DevicePixel>;
    fn hidpi_scale_factor(&self) -> Scale<f32, DeviceIndependentPixel, DevicePixel>;
    /// This returns [`RenderingContext`] matching the viewport.
    fn rendering_context(&self) -> Rc<dyn RenderingContextCore>;
    fn theme(&self) -> servo::Theme {
        servo::Theme::Light
    }
    fn window_rect(&self) -> DeviceIndependentIntRect;
    /// Request that the window redraw itself. It is up to the window to do this
    /// once the windowing system is ready. If this is a headless window, the redraw
    /// will happen immediately.
    fn request_repaint(&self, _: &EmbedderWindow);
    /// Request a new outer size for the window, including external decorations.
    /// This should be the same as `window.outerWidth` and `window.outerHeight`.
    fn request_resize(&self, webview: &WebView, outer_size: DeviceIntSize)
    -> Option<DeviceIntSize>;
    /// Request that the `Window` rebuild its user interface, if it has one. This should
    /// not repaint, but should prepare the user interface for painting when it is
    /// actually requested.
    fn rebuild_user_interface(&self, _: &RunningAppState, _: &EmbedderWindow) {}
    /// Inform the `Window` that the state of a `WebView` has changed and that it should
    /// do an incremental update of user interface state. Returns `true` if the user
    /// interface actually changed and a rebuild and repaint is needed, `false` otherwise.
    fn update_user_interface_state(&self, _: &RunningAppState, _: &EmbedderWindow) -> bool {
        false
    }
    /// Returns the raw OS window handle for use with native child-window creation (e.g. wry).
    ///
    /// Only available for headed windows; headless windows return `None`.
    #[cfg(feature = "wry")]
    fn raw_window_handle_for_child(&self) -> Option<RawWindowHandle> {
        None
    }
}

/// Focus and window operations: fullscreen, position, cursor, maximize.
pub(crate) trait PlatformWindowOps {
    fn focus(&self) {}
    fn has_platform_focus(&self) -> bool {
        true
    }
    fn get_fullscreen(&self) -> bool;
    fn set_fullscreen(&self, _state: bool) {}
    fn set_position(&self, _point: DeviceIntPoint) {}
    fn set_cursor(&self, _cursor: Cursor) {}
    fn maximize(&self, _: &WebView) {}
}

/// Dialog and embedder-control management.
pub(crate) trait PlatformWindowDialogs {
    fn show_embedder_control(&self, _: WebViewId, _: EmbedderControl) {}
    fn hide_embedder_control(&self, _: WebViewId, _: EmbedderControlId) {}
    fn dismiss_embedder_controls_for_webview(&self, _: WebViewId) {}
    fn show_permission_dialog(&self, _: WebViewId, _: PermissionRequest) {}
    fn show_http_authentication_dialog(&self, _: WebViewId, _: AuthenticationRequest) {}
}

/// Signals and notifications from Servo to the platform shell.
pub(crate) trait PlatformWindowSignals {
    fn notify_input_event_handled(
        &self,
        _webview: &WebView,
        _id: InputEventId,
        _result: InputEventResult,
    ) {
    }
    fn notify_media_session_event(&self, _: MediaSessionEvent) {}
    fn notify_crashed(&self, _: WebView, _reason: String, _backtrace: Option<String>) {}
    fn show_console_message(&self, _level: ConsoleLogLevel, _message: &str) {}
    fn notify_accessibility_tree_update(&self, _: WebView, _: servo::accesskit::TreeUpdate) {}
}

// ── PlatformWindow (composed facade) ─────────────────────────────────────────

/// A `PlatformWindow` abstracts away the different kinds of platform windows that might
/// be used in a Graphshell execution. This currently includes headed (winit) and headless
/// windows.
///
/// Capabilities are grouped into sub-traits:
/// - [`PlatformWindowRendering`]: geometry, scale, rendering context, UI lifecycle
/// - [`PlatformWindowOps`]: focus, fullscreen, position, cursor, maximize
/// - [`PlatformWindowDialogs`]: embedder controls and permission/auth dialogs
/// - [`PlatformWindowSignals`]: Servo-to-shell notifications and accessibility
pub(crate) trait PlatformWindow:
    PlatformWindowRendering + PlatformWindowOps + PlatformWindowDialogs + PlatformWindowSignals
{
    /// If this window is a headed window, access the concrete type.
    fn as_headed_window(
        &self,
    ) -> Option<&crate::shell::desktop::host::headed_window::HeadedWindow> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use super::{EmbedderWindow, InputTarget, WebViewLifecycleEventKind};
    use crate::app::renderer_id::test_renderer_id as test_webview_id;
    use crate::prefs::AppPreferences;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;

    #[test]
    fn host_input_target_blocks_explicit_webview_fallback() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));

        window.set_input_target(Some(InputTarget::Host));

        assert_eq!(window.explicit_input_webview_id(), None);
    }

    #[test]
    fn test_graph_event_sequence_stamped_at_emission_across_windows() {
        let prefs = AppPreferences::default();
        let shared_seq = Arc::new(AtomicU64::new(0));
        let window_a = EmbedderWindow::new(HeadlessWindow::new(&prefs), shared_seq.clone());
        let window_b = EmbedderWindow::new(HeadlessWindow::new(&prefs), shared_seq);

        window_a.enqueue_test_graph_event_kind(WebViewLifecycleEventKind::UrlChanged {
            webview_id: test_webview_id(),
            new_url: "https://a.example".into(),
        });
        window_b.enqueue_test_graph_event_kind(WebViewLifecycleEventKind::PageTitleChanged {
            webview_id: test_webview_id(),
            title: Some("B".into()),
        });
        window_a.enqueue_test_graph_event_kind(WebViewLifecycleEventKind::WebViewCrashed {
            webview_id: test_webview_id(),
            reason: "boom".into(),
            has_backtrace: false,
        });

        let mut merged = Vec::new();
        merged.extend(window_b.take_pending_graph_events());
        merged.extend(window_a.take_pending_graph_events());
        merged.sort_by_key(|event| event.seq);

        let seqs = merged.into_iter().map(|e| e.seq).collect::<Vec<_>>();
        assert_eq!(seqs, vec![1, 2, 3]);
    }
}
