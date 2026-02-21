/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use euclid::Scale;
use log::debug;
use servo::{
    AuthenticationRequest, ConsoleLogLevel, Cursor, DeviceIndependentIntRect,
    DeviceIndependentPixel, DeviceIntPoint, DeviceIntSize, DevicePixel, EmbedderControl,
    EmbedderControlId, GenericSender, InputEventId, InputEventResult, MediaSessionEvent,
    PermissionRequest, RenderingContext, ScreenGeometry, Servo, UserContentManager, WebView,
    WebViewBuilder, WebViewDelegate, WebViewId,
};
use url::Url;

use crate::app::RendererId;
use crate::running_app_state::{RunningAppState, UserInterfaceCommand, WebViewCollection};

pub(crate) trait WebViewCreationContext {
    fn servo(&self) -> &Servo;
    fn user_content_manager(&self) -> Rc<UserContentManager>;
    fn webview_delegate(self: Rc<Self>) -> Rc<dyn WebViewDelegate>;
    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    fn gamepad_provider(&self) -> Option<Rc<crate::desktop::gamepad::AppGamepadProvider>>;
}

// This should vary by zoom level and maybe actual text size (focused or under cursor)
#[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
pub(crate) const LINE_HEIGHT: f32 = 76.0;
#[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
pub(crate) const LINE_WIDTH: f32 = 76.0;

/// <https://github.com/web-platform-tests/wpt/blob/9320b1f724632c52929a3fdb11bdaf65eafc7611/webdriver/tests/classic/set_window_rect/set.py#L287-L290>
/// "A window size of 10x10px shouldn't be supported by any browser."
#[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
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
pub(crate) struct GraphSemanticEvent {
    pub(crate) seq: u64,
    pub(crate) kind: GraphSemanticEventKind,
}

#[derive(Clone, Debug)]
pub(crate) enum GraphSemanticEventKind {
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
    CreateNewWebView {
        parent_webview_id: RendererId,
        child_webview_id: RendererId,
        initial_url: Option<String>,
    },
    WebViewCrashed {
        webview_id: RendererId,
        reason: String,
        has_backtrace: bool,
    },
}

pub(crate) struct EmbedderWindow {
    /// The [`WebView`]s that have been added to this window.
    pub(crate) webview_collection: RefCell<WebViewCollection>,
    /// A handle to the [`PlatformWindow`] that graphshell is rendering in.
    platform_window: Rc<dyn PlatformWindow>,
    /// Whether or not this window should be closed at the end of the spin of the next event loop.
    close_scheduled: Cell<bool>,
    /// Whether or not the application interface needs to be updated.
    needs_update: Cell<bool>,
    /// Whether or not Servo needs to repaint its display. Currently this is global
    /// because every `WebView` shares a `RenderingContext`.
    needs_repaint: Cell<bool>,
    /// List of webviews that have favicon textures which are not yet uploaded
    /// to the GPU by egui.
    pending_favicon_loads: RefCell<Vec<WebViewId>>,
    /// List of webviews that have reached `LoadStatus::Complete` and should
    /// schedule a thumbnail capture in the GUI.
    pending_thumbnail_capture_requests: RefCell<Vec<WebViewId>>,
    /// Pending graph semantic events emitted from delegate callbacks.
    pending_graph_events: RefCell<Vec<GraphSemanticEvent>>,
    /// Global sequence source so graph semantic event order is deterministic across windows.
    graph_event_sequence: Arc<AtomicU64>,
    /// Optional runtime tracing for delegate/event ordering diagnostics.
    trace_graph_events: bool,
    /// Monotonic startup timestamp used for trace log deltas.
    trace_graph_events_started_at: Instant,
    /// Sequence number for graph event queue drains.
    trace_graph_event_drains: Cell<u64>,
    /// Pending [`UserInterfaceCommand`] that have yet to be processed by the main loop.
    pending_commands: RefCell<Vec<UserInterfaceCommand>>,
}

impl EmbedderWindow {
    pub(crate) fn new(
        platform_window: Rc<dyn PlatformWindow>,
        graph_event_sequence: Arc<AtomicU64>,
    ) -> Self {
        Self {
            webview_collection: Default::default(),
            platform_window,
            close_scheduled: Default::default(),
            needs_update: Default::default(),
            needs_repaint: Default::default(),
            pending_favicon_loads: Default::default(),
            pending_thumbnail_capture_requests: Default::default(),
            pending_graph_events: Default::default(),
            graph_event_sequence,
            trace_graph_events: std::env::var_os("GRAPHSHELL_TRACE_DELEGATE_EVENTS").is_some(),
            trace_graph_events_started_at: Instant::now(),
            trace_graph_event_drains: Cell::new(0),
            pending_commands: Default::default(),
        }
    }

    pub(crate) fn id(&self) -> EmbedderWindowId {
        self.platform_window().id()
    }

    pub(crate) fn create_and_activate_toplevel_webview<T>(
        &self,
        state: Rc<T>,
        url: Url,
    ) -> WebView
    where
        T: WebViewCreationContext + 'static,
    {
        let webview = self.create_toplevel_webview(state, url);
        self.activate_webview(webview.id());
        webview
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
        rendering_context: Rc<dyn RenderingContext>,
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

        #[cfg(all(
            feature = "gamepad",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        if let Some(gamepad_provider) = state.gamepad_provider() {
            webview.set_gamepad_provider(gamepad_provider);
        }

        webview.notify_theme_change(self.platform_window.theme());
        self.add_webview(webview.clone());
        webview
    }

    /// Repaint the focused [`WebView`].
    pub(crate) fn repaint_webviews(&self) {
        let Some(webview_id) = self.platform_window().preferred_input_webview_id(self) else {
            return;
        };
        let Some(webview) = self.webview_by_id(webview_id) else {
            return;
        };

        self.platform_window()
            .rendering_context()
            .make_current()
            .expect("Could not make PlatformWindow RenderingContext current");
        webview.paint();
        self.platform_window().rendering_context().present();
    }

    /// Whether or not this [`EmbedderWindow`] should close.
    pub(crate) fn should_close(&self) -> bool {
        self.close_scheduled.get()
    }

    pub(crate) fn contains_webview(&self, id: WebViewId) -> bool {
        self.webview_collection.borrow().contains(id)
    }

    pub(crate) fn webview_by_id(&self, id: WebViewId) -> Option<WebView> {
        self.webview_collection.borrow().get(id).cloned()
    }

    pub(crate) fn set_needs_update(&self) {
        self.needs_update.set(true);
    }

    pub(crate) fn set_needs_repaint(&self) {
        self.needs_repaint.set(true)
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn schedule_close(&self) {
        self.close_scheduled.set(true)
    }

    pub(crate) fn platform_window(&self) -> Rc<dyn PlatformWindow> {
        self.platform_window.clone()
    }

    pub(crate) fn focus(&self) {
        self.platform_window.focus()
    }

    pub(crate) fn add_webview(&self, webview: WebView) {
        self.webview_collection.borrow_mut().add(webview);
        self.set_needs_update();
        self.set_needs_repaint();
    }

    pub(crate) fn webview_ids(&self) -> Vec<WebViewId> {
        self.webview_collection.borrow().creation_order.clone()
    }

    /// Returns all [`WebView`]s in creation order.
    pub(crate) fn webviews(&self) -> Vec<(WebViewId, WebView)> {
        self.webview_collection
            .borrow()
            .all_in_creation_order()
            .map(|(id, webview)| (id, webview.clone()))
            .collect()
    }

    pub(crate) fn activate_webview(&self, webview_id: WebViewId) {
        self.webview_collection
            .borrow_mut()
            .activate_webview(webview_id);
        self.set_needs_update();
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn activate_webview_by_index(&self, index_to_activate: usize) {
        self.webview_collection
            .borrow_mut()
            .activate_webview_by_index(index_to_activate);
        self.set_needs_update();
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn get_active_webview_index(&self) -> Option<usize> {
        let active_id = self.webview_collection.borrow().active_id()?;
        self.webviews()
            .iter()
            .position(|webview| webview.0 == active_id)
    }

    pub(crate) fn update_and_request_repaint_if_necessary(&self, state: &RunningAppState) {
        let updated_user_interface = self.needs_update.take()
            && self
                .platform_window
                .update_user_interface_state(state, self);

        // Delegate handlers may have asked us to present or update painted WebView contents.
        // Currently, egui-file-dialog dialogs need to be constantly redrawn or animations aren't fluid.
        let needs_repaint = self.needs_repaint.take();
        if updated_user_interface || needs_repaint {
            self.platform_window.request_repaint(self);
        }
    }

    /// Close the given [`WebView`] via its [`WebViewId`].
    ///
    /// Note: This can happen because we can trigger a close with a UI action and then get
    /// the close notification via the [`WebViewDelegate`] later.
    pub(crate) fn close_webview(&self, webview_id: WebViewId) {
        let mut webview_collection = self.webview_collection.borrow_mut();
        if webview_collection.remove(webview_id).is_none() {
            return;
        }
        self.platform_window
            .dismiss_embedder_controls_for_webview(webview_id);

        self.set_needs_update();
        self.set_needs_repaint();
    }

    pub(crate) fn notify_favicon_changed(&self, webview: WebView) {
        self.pending_favicon_loads.borrow_mut().push(webview.id());
        self.set_needs_repaint();
    }

    pub(crate) fn notify_load_status_complete(&self, webview: WebView) {
        self.pending_thumbnail_capture_requests
            .borrow_mut()
            .push(webview.id());
        self.set_needs_repaint();
    }

    pub(crate) fn notify_url_changed(&self, webview: WebView, new_url: Url) {
        let event = self.new_graph_semantic_event(GraphSemanticEventKind::UrlChanged {
            webview_id: webview.id(),
            new_url: new_url.to_string(),
        });
        self.trace_graph_semantic_event(&event);
        self.pending_graph_events.borrow_mut().push(event);
        self.set_needs_update();
    }

    pub(crate) fn notify_history_changed(
        &self,
        webview: WebView,
        entries: Vec<Url>,
        current: usize,
    ) {
        let event = self.new_graph_semantic_event(GraphSemanticEventKind::HistoryChanged {
            webview_id: webview.id(),
            entries: entries.into_iter().map(|u| u.to_string()).collect(),
            current,
        });
        self.trace_graph_semantic_event(&event);
        self.pending_graph_events.borrow_mut().push(event);
        self.set_needs_update();
    }

    pub(crate) fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        let event = self.new_graph_semantic_event(GraphSemanticEventKind::PageTitleChanged {
            webview_id: webview.id(),
            title,
        });
        self.trace_graph_semantic_event(&event);
        self.pending_graph_events.borrow_mut().push(event);
        self.set_needs_update();
    }

    pub(crate) fn notify_create_new_webview(
        &self,
        parent_webview: WebView,
        child_webview: WebView,
    ) {
        let event = self.new_graph_semantic_event(GraphSemanticEventKind::CreateNewWebView {
            parent_webview_id: parent_webview.id(),
            child_webview_id: child_webview.id(),
            initial_url: child_webview.url().map(|u| u.to_string()),
        });
        self.trace_graph_semantic_event(&event);
        self.pending_graph_events.borrow_mut().push(event);
        self.set_needs_update();
    }

    pub(crate) fn notify_webview_crashed(
        &self,
        webview: WebView,
        reason: String,
        backtrace: Option<String>,
    ) {
        let event = self.new_graph_semantic_event(GraphSemanticEventKind::WebViewCrashed {
            webview_id: webview.id(),
            reason,
            has_backtrace: backtrace.as_deref().is_some_and(|b| !b.is_empty()),
        });
        self.trace_graph_semantic_event(&event);
        self.pending_graph_events.borrow_mut().push(event);
        self.set_needs_update();
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn hidpi_scale_factor_changed(&self) {
        let new_scale_factor = self.platform_window.hidpi_scale_factor();
        for webview in self.webview_collection.borrow().values() {
            webview.set_hidpi_scale_factor(new_scale_factor);
        }
    }

    /// Return a list of all webviews that have favicons that have not yet been loaded by egui.
    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn take_pending_favicon_loads(&self) -> Vec<WebViewId> {
        std::mem::take(&mut *self.pending_favicon_loads.borrow_mut())
    }

    /// Return webviews that should schedule thumbnail capture.
    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn take_pending_thumbnail_capture_requests(&self) -> Vec<WebViewId> {
        std::mem::take(&mut *self.pending_thumbnail_capture_requests.borrow_mut())
    }

    /// Return all pending graph semantic events.
    pub(crate) fn take_pending_graph_events(&self) -> Vec<GraphSemanticEvent> {
        let events = std::mem::take(&mut *self.pending_graph_events.borrow_mut());
        if self.trace_graph_events {
            let drain_id = self.trace_graph_event_drains.get() + 1;
            self.trace_graph_event_drains.set(drain_id);
            let elapsed_ms = self.trace_graph_events_started_at.elapsed().as_millis();
            debug!(
                "graph_event_trace drain={} t_ms={} count={}",
                drain_id,
                elapsed_ms,
                events.len()
            );
        }
        events
    }

    fn trace_graph_semantic_event(&self, event: &GraphSemanticEvent) {
        if !self.trace_graph_events {
            return;
        }
        let elapsed_ms = self.trace_graph_events_started_at.elapsed().as_millis();
        match &event.kind {
            GraphSemanticEventKind::UrlChanged {
                webview_id,
                new_url,
            } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=url_changed webview={:?} url={}",
                    event.seq, elapsed_ms, webview_id, new_url
                );
            },
            GraphSemanticEventKind::HistoryChanged {
                webview_id,
                entries,
                current,
            } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=history_changed webview={:?} entries_len={} current={}",
                    event.seq,
                    elapsed_ms,
                    webview_id,
                    entries.len(),
                    current
                );
            },
            GraphSemanticEventKind::PageTitleChanged { webview_id, title } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=title_changed webview={:?} title_present={}",
                    event.seq,
                    elapsed_ms,
                    webview_id,
                    title.as_deref().is_some_and(|t| !t.is_empty())
                );
            },
            GraphSemanticEventKind::CreateNewWebView {
                parent_webview_id,
                child_webview_id,
                initial_url,
            } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=create_new parent={:?} child={:?} initial_url_present={}",
                    event.seq,
                    elapsed_ms,
                    parent_webview_id,
                    child_webview_id,
                    initial_url.as_deref().is_some_and(|u| !u.is_empty())
                );
            },
            GraphSemanticEventKind::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=webview_crashed webview={:?} reason_len={} has_backtrace={}",
                    event.seq,
                    elapsed_ms,
                    webview_id,
                    reason.len(),
                    has_backtrace
                );
            },
        }
    }

    fn new_graph_semantic_event(&self, kind: GraphSemanticEventKind) -> GraphSemanticEvent {
        let seq = self.graph_event_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        GraphSemanticEvent { seq, kind }
    }

    #[cfg(test)]
    pub(crate) fn enqueue_test_graph_event_kind(&self, kind: GraphSemanticEventKind) {
        let event = self.new_graph_semantic_event(kind);
        self.pending_graph_events.borrow_mut().push(event);
    }

    pub(crate) fn show_embedder_control(
        &self,
        webview: WebView,
        embedder_control: EmbedderControl,
    ) {
        self.platform_window
            .show_embedder_control(webview.id(), embedder_control);
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

    pub(crate) fn queue_user_interface_command(&self, command: UserInterfaceCommand) {
        self.pending_commands.borrow_mut().push(command)
    }

    /// Takes any events generated during UI updates and performs their actions.
    pub(crate) fn handle_interface_commands(
        &self,
        state: &Rc<RunningAppState>,
        _create_platform_window: Option<&dyn Fn(Url) -> Rc<dyn PlatformWindow>>,
    ) {
        let commands = std::mem::take(&mut *self.pending_commands.borrow_mut());
        for event in commands {
            match event {
                UserInterfaceCommand::ReloadAll => {
                    for window in state.windows().values() {
                        window.set_needs_update();
                        for (_, webview) in window.webviews() {
                            webview.reload();
                        }
                    }
                },
            }
        }
    }
}

/// A `PlatformWindow` abstracts away the differents kinds of platform windows that might
/// be used in a servoshell execution. This currently includes headed (winit) and headless
/// windows.
pub(crate) trait PlatformWindow {
    fn id(&self) -> EmbedderWindowId;
    fn screen_geometry(&self) -> ScreenGeometry;
    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    fn device_hidpi_scale_factor(&self) -> Scale<f32, DeviceIndependentPixel, DevicePixel>;
    fn hidpi_scale_factor(&self) -> Scale<f32, DeviceIndependentPixel, DevicePixel>;
    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    fn get_fullscreen(&self) -> bool;
    /// Request that the `Window` rebuild its user interface, if it has one. This should
    /// not repaint, but should prepare the user interface for painting when it is
    /// actually requested.
    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    fn rebuild_user_interface(&self, _: &RunningAppState, _: &EmbedderWindow) {}
    /// Inform the `Window` that the state of a `WebView` has changed and that it should
    /// do an incremental update of user interface state. Returns `true` if the user
    /// interface actually changed and a rebuild  and repaint is needed, `false` otherwise.
    fn update_user_interface_state(&self, _: &RunningAppState, _: &EmbedderWindow) -> bool {
        false
    }
    /// Request that the window redraw itself. It is up to the window to do this
    /// once the windowing system is ready. If this is a headless window, the redraw
    /// will happen immediately.
    fn request_repaint(&self, _: &EmbedderWindow);
    /// Request a new outer size for the window, including external decorations.
    /// This should be the same as `window.outerWidth` and `window.outerHeight``
    fn request_resize(&self, webview: &WebView, outer_size: DeviceIntSize)
    -> Option<DeviceIntSize>;
    fn set_position(&self, _point: DeviceIntPoint) {}
    fn set_fullscreen(&self, _state: bool) {}
    fn set_cursor(&self, _cursor: Cursor) {}
    #[cfg(all(
        feature = "webxr",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    fn new_glwindow(
        &self,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> Rc<dyn servo::webxr::GlWindow>;
    /// This returns [`RenderingContext`] matching the viewport.
    fn rendering_context(&self) -> Rc<dyn RenderingContext>;
    fn theme(&self) -> servo::Theme {
        servo::Theme::Light
    }
    fn window_rect(&self) -> DeviceIndependentIntRect;
    fn maximize(&self, _: &WebView) {}
    fn focus(&self) {}
    fn has_platform_focus(&self) -> bool {
        true
    }

    fn show_embedder_control(&self, _: WebViewId, _: EmbedderControl) {}
    fn hide_embedder_control(&self, _: WebViewId, _: EmbedderControlId) {}
    fn dismiss_embedder_controls_for_webview(&self, _: WebViewId) {}
    fn show_bluetooth_device_dialog(
        &self,
        _: WebViewId,
        _devices: Vec<String>,
        _: GenericSender<Option<String>>,
    ) {
    }
    fn show_permission_dialog(&self, _: WebViewId, _: PermissionRequest) {}
    fn show_http_authentication_dialog(&self, _: WebViewId, _: AuthenticationRequest) {}

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
    /// Preferred webview target for user-input-like routing on this platform window.
    /// Defaults to the window-global active id; headed graphshell overrides this to use
    /// tile-focused targeting.
    fn preferred_input_webview_id(&self, window: &EmbedderWindow) -> Option<WebViewId> {
        window.webview_collection.borrow().active_id()
    }

    #[cfg(not(any(target_os = "android", target_env = "ohos")))]
    /// If this window is a headed window, access the concrete type.
    fn as_headed_window(&self) -> Option<&crate::desktop::headed_window::HeadedWindow> {
        None
    }

    #[cfg(any(target_os = "android", target_env = "ohos"))]
    /// If this window is a headed window, access the concrete type.
    fn as_headed_window(&self) -> Option<&crate::egl::app::EmbeddedPlatformWindow> {
        None
    }

    fn notify_accessibility_tree_update(&self, _: WebView, _: accesskit::TreeUpdate) {}
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use servo::WebViewId;

    use super::{EmbedderWindow, GraphSemanticEventKind};
    use crate::desktop::headless_window::HeadlessWindow;
    use crate::prefs::AppPreferences;

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }

    #[test]
    fn test_graph_event_sequence_stamped_at_emission_across_windows() {
        let prefs = AppPreferences::default();
        let shared_seq = Arc::new(AtomicU64::new(0));
        let window_a = EmbedderWindow::new(HeadlessWindow::new(&prefs), shared_seq.clone());
        let window_b = EmbedderWindow::new(HeadlessWindow::new(&prefs), shared_seq);

        window_a.enqueue_test_graph_event_kind(GraphSemanticEventKind::UrlChanged {
            webview_id: test_webview_id(),
            new_url: "https://a.example".into(),
        });
        window_b.enqueue_test_graph_event_kind(GraphSemanticEventKind::PageTitleChanged {
            webview_id: test_webview_id(),
            title: Some("B".into()),
        });
        window_a.enqueue_test_graph_event_kind(GraphSemanticEventKind::WebViewCrashed {
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
