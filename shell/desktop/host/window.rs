/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use euclid::Scale;
use log::debug;
#[cfg(feature = "wry")]
use raw_window_handle::RawWindowHandle;
use servo::{
    AuthenticationRequest, ConsoleLogLevel, Cursor, DeviceIndependentIntRect,
    DeviceIndependentPixel, DeviceIntPoint, DeviceIntSize, DevicePixel, EmbedderControl,
    EmbedderControlId, GenericSender, InputEventId, InputEventResult, MediaSessionEvent,
    PermissionRequest, RenderingContext, ScreenGeometry, Servo, UserContentManager, WebView,
    WebViewBuilder, WebViewDelegate, WebViewId,
};
use url::Url;

use crate::app::{HostOpenRequest, OpenSurfaceSource, PendingCreateToken, RendererId};
use crate::shell::desktop::host::running_app_state::{RunningAppState, WebViewCollection};
#[cfg(all(
    feature = "diagnostics",
    not(any(target_os = "android", target_env = "ohos"))
))]
use crate::shell::desktop::runtime::diagnostics::{self, DiagnosticEvent};
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::workbench::pane_model::PaneId;

pub(crate) trait WebViewCreationContext {
    fn servo(&self) -> &Servo;
    fn user_content_manager(&self) -> Rc<UserContentManager>;
    fn webview_delegate(self: Rc<Self>) -> Rc<dyn WebViewDelegate>;
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
    HostOpenRequest {
        request: HostOpenRequest,
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

struct WindowProjectionState {
    focused_pane: Cell<Option<PaneId>>,
    input_target: Cell<Option<InputTarget>>,
    chrome_projection_source: Cell<Option<ChromeProjectionSource>>,
    dialog_owner: Cell<Option<DialogOwner>>,
    visible_node_panes: RefCell<Vec<PaneId>>,
}

impl Default for WindowProjectionState {
    fn default() -> Self {
        Self {
            focused_pane: Cell::new(None),
            input_target: Cell::new(None),
            chrome_projection_source: Cell::new(None),
            dialog_owner: Cell::new(None),
            visible_node_panes: RefCell::new(Vec::new()),
        }
    }
}

impl WindowProjectionState {
    fn focused_pane(&self) -> Option<PaneId> {
        self.focused_pane.get()
    }

    fn set_focused_pane(&self, pane_id: Option<PaneId>) {
        self.focused_pane.set(pane_id);
    }

    fn input_target(&self) -> Option<InputTarget> {
        self.input_target.get()
    }

    fn set_input_target(&self, target: Option<InputTarget>) {
        self.input_target.set(target);
    }

    fn chrome_projection_source(&self) -> Option<ChromeProjectionSource> {
        self.chrome_projection_source.get()
    }

    fn set_chrome_projection_source(&self, source: Option<ChromeProjectionSource>) {
        self.chrome_projection_source.set(source);
    }

    fn dialog_owner(&self) -> Option<DialogOwner> {
        self.dialog_owner.get()
    }

    fn set_dialog_owner(&self, owner: Option<DialogOwner>) {
        self.dialog_owner.set(owner);
    }

    fn set_visible_node_panes(&self, pane_ids: Vec<PaneId>) {
        *self.visible_node_panes.borrow_mut() = pane_ids;
    }

    fn visible_renderer_ids(&self) -> Vec<WebViewId> {
        let mut seen = HashSet::new();
        self.visible_node_panes
            .borrow()
            .iter()
            .filter_map(|pane_id| registries::phase1_renderer_attachment_for_pane(*pane_id))
            .filter_map(|attachment| {
                seen.insert(attachment.renderer_id)
                    .then_some(attachment.renderer_id)
            })
            .collect()
    }

    fn explicit_input_webview_id(&self) -> Option<WebViewId> {
        match self.input_target() {
            Some(InputTarget::Host) => None,
            Some(InputTarget::Renderer(renderer_id)) => Some(renderer_id),
            Some(InputTarget::Pane(pane_id)) => {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }
            None => self.focused_pane().and_then(|pane_id| {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }),
        }
    }

    fn targeted_input_webview_id(&self) -> Option<WebViewId> {
        match self.input_target() {
            Some(InputTarget::Host) => None,
            Some(InputTarget::Renderer(renderer_id)) => Some(renderer_id),
            Some(InputTarget::Pane(pane_id)) => {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }
            None => None,
        }
    }

    fn explicit_dialog_webview_id(&self) -> Option<WebViewId> {
        match self.dialog_owner() {
            Some(DialogOwner::Renderer(renderer_id)) => Some(renderer_id),
            Some(DialogOwner::Pane(pane_id)) => {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }
            None => None,
        }
    }

    fn explicit_chrome_webview_id(&self) -> Option<WebViewId> {
        match self.chrome_projection_source() {
            Some(ChromeProjectionSource::Renderer(renderer_id)) => Some(renderer_id),
            Some(ChromeProjectionSource::Pane(pane_id)) => {
                registries::phase1_renderer_attachment_for_pane(pane_id)
                    .map(|attachment| attachment.renderer_id)
            }
            None => None,
        }
    }

    fn dialog_owner_for_webview(&self, webview_id: WebViewId) -> DialogOwner {
        registries::phase1_pane_for_renderer(webview_id)
            .map(DialogOwner::Pane)
            .unwrap_or(DialogOwner::Renderer(webview_id))
    }

    fn sync_explicit_targets_for_webview(&self, webview_id: WebViewId) {
        let pane_id = registries::phase1_pane_for_renderer(webview_id);
        self.set_focused_pane(pane_id);
        self.set_input_target(Some(InputTarget::Renderer(webview_id)));
        self.set_chrome_projection_source(Some(ChromeProjectionSource::Renderer(webview_id)));
        self.set_dialog_owner(Some(self.dialog_owner_for_webview(webview_id)));
    }

    fn clear_explicit_targets_for_closed_webview(
        &self,
        webview_id: WebViewId,
        detached_pane_id: Option<PaneId>,
    ) {
        if self.focused_pane() == detached_pane_id {
            self.set_focused_pane(None);
        }

        if matches!(
            self.input_target(),
            Some(InputTarget::Renderer(renderer_id)) if renderer_id == webview_id
        ) || matches!(
            (self.input_target(), detached_pane_id),
            (Some(InputTarget::Pane(pane_id)), Some(detached_pane_id)) if pane_id == detached_pane_id
        ) {
            self.set_input_target(None);
        }

        if matches!(
            self.chrome_projection_source(),
            Some(ChromeProjectionSource::Renderer(renderer_id)) if renderer_id == webview_id
        ) || matches!(
            (self.chrome_projection_source(), detached_pane_id),
            (Some(ChromeProjectionSource::Pane(pane_id)), Some(detached_pane_id)) if pane_id == detached_pane_id
        ) {
            self.set_chrome_projection_source(None);
        }

        if matches!(
            self.dialog_owner(),
            Some(DialogOwner::Renderer(renderer_id)) if renderer_id == webview_id
        ) || matches!(
            (self.dialog_owner(), detached_pane_id),
            (Some(DialogOwner::Pane(pane_id)), Some(detached_pane_id)) if pane_id == detached_pane_id
        ) {
            self.set_dialog_owner(None);
        }
    }
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

struct WindowGraphEventQueue {
    pending_events: RefCell<Vec<GraphSemanticEvent>>,
    sequence: Arc<AtomicU64>,
    trace_enabled: bool,
    trace_started_at: Instant,
    trace_drains: Cell<u64>,
}

impl WindowGraphEventQueue {
    fn new(sequence: Arc<AtomicU64>) -> Self {
        Self {
            pending_events: Default::default(),
            sequence,
            trace_enabled: std::env::var_os("GRAPHSHELL_TRACE_DELEGATE_EVENTS").is_some(),
            trace_started_at: Instant::now(),
            trace_drains: Cell::new(0),
        }
    }

    fn enqueue(&self, kind: GraphSemanticEventKind) {
        let event = self.new_event(kind);
        self.trace_event(&event);
        self.pending_events.borrow_mut().push(event);
    }

    fn take_pending(&self) -> Vec<GraphSemanticEvent> {
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        let drain_started = Instant::now();

        let events = std::mem::take(&mut *self.pending_events.borrow_mut());

        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        {
            diagnostics::emit_event(DiagnosticEvent::MessageReceived {
                channel_id: "window.graph_event.drain",
                latency_us: drain_started.elapsed().as_micros() as u64,
            });
            diagnostics::emit_event(DiagnosticEvent::MessageReceived {
                channel_id: "servo.graph_event.drain",
                latency_us: drain_started.elapsed().as_micros() as u64,
            });
            diagnostics::emit_event(DiagnosticEvent::MessageSent {
                channel_id: "window.graph_event.drain_count",
                byte_len: events.len(),
            });
            diagnostics::emit_event(DiagnosticEvent::MessageSent {
                channel_id: "servo.graph_event.drain_count",
                byte_len: events.len(),
            });
        }

        if self.trace_enabled {
            let drain_id = self.trace_drains.get() + 1;
            self.trace_drains.set(drain_id);
            let elapsed_ms = self.trace_started_at.elapsed().as_millis();
            debug!(
                "graph_event_trace drain={} t_ms={} count={}",
                drain_id,
                elapsed_ms,
                events.len()
            );
        }

        events
    }

    #[cfg(test)]
    fn enqueue_for_test(&self, kind: GraphSemanticEventKind) {
        self.enqueue(kind);
    }

    fn new_event(&self, kind: GraphSemanticEventKind) -> GraphSemanticEvent {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        GraphSemanticEvent { seq, kind }
    }

    fn trace_event(&self, event: &GraphSemanticEvent) {
        if !self.trace_enabled {
            return;
        }

        let elapsed_ms = self.trace_started_at.elapsed().as_millis();
        match &event.kind {
            GraphSemanticEventKind::UrlChanged {
                webview_id,
                new_url,
            } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=url_changed webview={:?} url={}",
                    event.seq, elapsed_ms, webview_id, new_url
                );
            }
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
            }
            GraphSemanticEventKind::PageTitleChanged { webview_id, title } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=title_changed webview={:?} title_present={}",
                    event.seq,
                    elapsed_ms,
                    webview_id,
                    title.as_deref().is_some_and(|value| !value.is_empty())
                );
            }
            GraphSemanticEventKind::HostOpenRequest { request } => {
                debug!(
                    "graph_event_trace seq={} t_ms={} kind=host_open_request url={} source={:?} parent={:?}",
                    event.seq, elapsed_ms, request.url, request.source, request.parent_webview_id
                );
            }
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
            }
        }
    }
}

struct WindowRuntimeState {
    webviews: RefCell<WebViewCollection>,
    close_scheduled: Cell<bool>,
    needs_update: Cell<bool>,
    needs_repaint: Cell<bool>,
}

impl Default for WindowRuntimeState {
    fn default() -> Self {
        Self {
            webviews: Default::default(),
            close_scheduled: Default::default(),
            needs_update: Default::default(),
            needs_repaint: Default::default(),
        }
    }
}

impl WindowRuntimeState {
    fn should_close(&self) -> bool {
        self.close_scheduled.get()
    }

    fn schedule_close(&self) {
        self.close_scheduled.set(true);
    }

    fn contains_webview(&self, id: WebViewId) -> bool {
        self.webviews.borrow().contains(id)
    }

    fn webview_by_id(&self, id: WebViewId) -> Option<WebView> {
        self.webviews.borrow().get(id).cloned()
    }

    fn add_webview(&self, webview: WebView) {
        self.webviews.borrow_mut().add(webview);
    }

    fn remove_webview(&self, webview_id: WebViewId) -> bool {
        self.webviews.borrow_mut().remove(webview_id).is_some()
    }

    fn webview_ids(&self) -> Vec<WebViewId> {
        self.webviews.borrow().creation_order.clone()
    }

    fn webviews(&self) -> Vec<(WebViewId, WebView)> {
        self.webviews
            .borrow()
            .all_in_creation_order()
            .map(|(id, webview)| (id, webview.clone()))
            .collect()
    }

    fn newest_webview_id(&self) -> Option<WebViewId> {
        self.webviews.borrow().newest().map(|webview| webview.id())
    }

    fn for_each_webview(&self, mut f: impl FnMut(&WebView)) {
        for webview in self.webviews.borrow().values() {
            f(webview);
        }
    }

    fn set_needs_update(&self) {
        self.needs_update.set(true);
    }

    fn take_needs_update(&self) -> bool {
        self.needs_update.take()
    }

    fn set_needs_repaint(&self) {
        self.needs_repaint.set(true);
    }

    fn take_needs_repaint(&self) -> bool {
        self.needs_repaint.take()
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

        self.platform_window()
            .rendering_context()
            .make_current()
            .expect("Could not make PlatformWindow RenderingContext current");
        for webview_id in visible_renderers {
            let Some(webview) = self.webview_by_id(webview_id) else {
                continue;
            };
            webview.paint();
        }
        self.platform_window().rendering_context().present();
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

    pub(crate) fn newest_webview_id(&self) -> Option<WebViewId> {
        self.runtime_state.newest_webview_id()
    }

    pub(crate) fn set_needs_update(&self) {
        self.runtime_state.set_needs_update();
    }

    pub(crate) fn set_needs_repaint(&self) {
        self.runtime_state.set_needs_repaint()
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
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
        let detached_attachment = registries::phase1_detach_renderer(webview_id);
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
        let kind = GraphSemanticEventKind::UrlChanged {
            webview_id: webview.id(),
            new_url: new_url.to_string(),
        };
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "window.graph_event.url_changed",
            byte_len: 1,
        });
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
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
        let kind = GraphSemanticEventKind::HistoryChanged {
            webview_id: webview.id(),
            entries: entries.into_iter().map(|u| u.to_string()).collect(),
            current,
        };
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "window.graph_event.history_changed",
            byte_len: 1,
        });
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "servo.delegate.history_changed",
            byte_len: 1,
        });
        self.graph_events.enqueue(kind);
        self.set_needs_update();
    }

    pub(crate) fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        let kind = GraphSemanticEventKind::PageTitleChanged {
            webview_id: webview.id(),
            title,
        };
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "window.graph_event.title_changed",
            byte_len: 1,
        });
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
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
        let kind = GraphSemanticEventKind::HostOpenRequest {
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

    pub(crate) fn notify_webview_crashed(
        &self,
        webview: WebView,
        reason: String,
        backtrace: Option<String>,
    ) {
        let kind = GraphSemanticEventKind::WebViewCrashed {
            webview_id: webview.id(),
            reason,
            has_backtrace: backtrace.as_deref().is_some_and(|b| !b.is_empty()),
        };
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "window.graph_event.webview_crashed",
            byte_len: 1,
        });
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        diagnostics::emit_event(DiagnosticEvent::MessageSent {
            channel_id: "servo.delegate.webview_crashed",
            byte_len: 1,
        });
        self.graph_events.enqueue(kind);
        self.set_needs_update();
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn hidpi_scale_factor_changed(&self) {
        let new_scale_factor = self.platform_window.hidpi_scale_factor();
        self.runtime_state
            .for_each_webview(|webview| webview.set_hidpi_scale_factor(new_scale_factor));
    }

    /// Return a list of all webviews that have favicons that have not yet been loaded by egui.
    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn take_pending_favicon_loads(&self) -> Vec<WebViewId> {
        self.ui_signals.take_pending_favicon_loads()
    }

    /// Return webviews that should schedule thumbnail capture.
    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn take_pending_thumbnail_capture_requests(&self) -> Vec<WebViewId> {
        self.ui_signals.take_pending_thumbnail_capture_requests()
    }

    /// Return all pending graph semantic events.
    pub(crate) fn take_pending_graph_events(&self) -> Vec<GraphSemanticEvent> {
        self.graph_events.take_pending()
    }

    #[cfg(test)]
    pub(crate) fn enqueue_test_graph_event_kind(&self, kind: GraphSemanticEventKind) {
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

    pub(crate) fn show_bluetooth_device_dialog(
        &self,
        webview_id: WebViewId,
        devices: Vec<String>,
        response_sender: GenericSender<Option<String>>,
    ) {
        self.set_dialog_owner(Some(
            self.projection_state.dialog_owner_for_webview(webview_id),
        ));
        self.platform_window
            .show_bluetooth_device_dialog(webview_id, devices, response_sender);
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

/// A `PlatformWindow` abstracts away the differents kinds of platform windows that might
/// be used in a Graphshell execution. This currently includes headed (winit) and headless
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
    #[cfg(not(any(target_os = "android", target_env = "ohos")))]
    /// If this window is a headed window, access the concrete type.
    fn as_headed_window(
        &self,
    ) -> Option<&crate::shell::desktop::host::headed_window::HeadedWindow> {
        None
    }

    #[cfg(any(target_os = "android", target_env = "ohos"))]
    /// If this window is a headed window, access the concrete type.
    fn as_headed_window(&self) -> Option<&crate::egl::app::EmbeddedPlatformWindow> {
        None
    }

    fn notify_accessibility_tree_update(&self, _: WebView, _: accesskit::TreeUpdate) {}

    /// Returns the raw OS window handle for use with native child-window creation (e.g. wry).
    ///
    /// Only available for headed windows; headless windows return `None`.
    #[cfg(feature = "wry")]
    fn raw_window_handle_for_child(&self) -> Option<RawWindowHandle> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use servo::WebViewId;

    use super::{EmbedderWindow, GraphSemanticEventKind, InputTarget};
    use crate::prefs::AppPreferences;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }

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
