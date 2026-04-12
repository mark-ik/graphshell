/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared state and methods for desktop and EGL implementations.

use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
#[cfg(all(
    feature = "diagnostics",
    not(any(target_os = "android", target_env = "ohos"))
))]
use std::time::Instant;

use image::{DynamicImage, ImageFormat};
#[cfg(all(
    any(coverage, llvm_pgo),
    any(target_os = "android", target_env = "ohos")
))]
use libc::c_char;
use log::{error, info};
use servo::user_contents::UserStyleSheet;
use servo::{
    AllowOrDenyRequest, ConsoleLogLevel, CreateNewWebViewRequest, EventLoopWaker, PrefValue,
    Preferences, Servo, ServoDelegate, ServoError, UserContentManager, WebView, WebViewDelegate,
    WebViewId, pref,
};
use url::Url;

use crate::app::{PendingCreateToken, RuntimeUserStylesheetSpec, WorkspaceUserStylesheetSetting};
use crate::prefs::{AppPreferences, EXPERIMENTAL_PREFS};
use crate::shell::desktop::host::embedder::EmbedderCore;
#[cfg(all(
    feature = "gamepad",
    not(any(target_os = "android", target_env = "ohos"))
))]
pub(crate) use crate::shell::desktop::host::gamepad::AppGamepadProvider;
#[cfg(all(
    feature = "gamepad",
    not(any(target_os = "android", target_env = "ohos"))
))]
use crate::shell::desktop::host::gamepad::GamepadUiCommand;
#[cfg(all(
    feature = "gamepad",
    not(any(target_os = "android", target_env = "ohos"))
))]
use crate::shell::desktop::host::gamepad_runtime::GamepadRuntime;
use crate::shell::desktop::host::webdriver_runtime::WebDriverRuntime;
use crate::shell::desktop::host::window::{
    EmbedderWindow, EmbedderWindowId, WebViewLifecycleEvent, PlatformWindow, WebViewCreationContext,
};
#[cfg(all(
    feature = "diagnostics",
    not(any(target_os = "android", target_env = "ohos"))
))]
use crate::shell::desktop::runtime::diagnostics::{self, DiagnosticEvent, SpanPhase};

mod webview_delegate;
use self::webview_delegate::RunningAppStateWebViewDelegate;
#[cfg(all(
    any(coverage, llvm_pgo),
    any(target_os = "android", target_env = "ohos")
))]
unsafe extern "C" {
    fn __llvm_profile_set_filename(file: *const c_char);
    fn __llvm_profile_write_file();
}

#[derive(Default)]
pub struct WebViewCollection {
    /// List of top-level browsing contexts.
    /// Modified by EmbedderMsg::WebViewOpened and EmbedderMsg::WebViewClosed,
    /// and we exit if it ever becomes empty.
    webviews: HashMap<WebViewId, WebView>,

    /// The order in which the webviews were created.
    pub(crate) creation_order: Vec<WebViewId>,
}

impl WebViewCollection {
    pub fn add(&mut self, webview: WebView) {
        let id = webview.id();
        webview.show();
        self.creation_order.push(id);
        self.webviews.insert(id, webview);
    }

    /// Removes a webview from the collection by [`WebViewId`].
    pub fn remove(&mut self, id: WebViewId) -> Option<WebView> {
        self.creation_order.retain(|&webview_id| webview_id != id);
        self.webviews.remove(&id)
    }

    pub fn get(&self, id: WebViewId) -> Option<&WebView> {
        self.webviews.get(&id)
    }

    pub fn contains(&self, id: WebViewId) -> bool {
        self.webviews.contains_key(&id)
    }

    /// Gets a reference to the most recently created webview, if any.
    pub fn newest(&self) -> Option<&WebView> {
        self.creation_order
            .last()
            .and_then(|id| self.webviews.get(id))
    }

    pub fn all_in_creation_order(&self) -> impl Iterator<Item = (WebViewId, &WebView)> {
        self.creation_order
            .iter()
            .filter_map(move |id| self.webviews.get(id).map(|webview| (*id, webview)))
    }

    /// Returns an iterator over all webview references (in arbitrary order).
    pub fn values(&self) -> impl Iterator<Item = &WebView> {
        self.webviews.values()
    }
}

struct PendingCreateRequest {
    request: CreateNewWebViewRequest,
}

struct PendingCreateStore {
    requests: RefCell<HashMap<PendingCreateToken, PendingCreateRequest>>,
    next_token: Cell<u64>,
}

struct ManagedUserStylesheet {
    path: String,
    stylesheet: Rc<UserStyleSheet>,
}

impl Default for PendingCreateStore {
    fn default() -> Self {
        Self {
            requests: Default::default(),
            next_token: Cell::new(1),
        }
    }
}

impl PendingCreateStore {
    fn store(&self, request: CreateNewWebViewRequest) -> PendingCreateToken {
        let token = PendingCreateToken::new(self.next_token.get());
        self.next_token.set(self.next_token.get().saturating_add(1));
        self.requests
            .borrow_mut()
            .insert(token, PendingCreateRequest { request });
        token
    }

    fn take(&self, token: PendingCreateToken) -> Option<CreateNewWebViewRequest> {
        self.requests
            .borrow_mut()
            .remove(&token)
            .map(|entry| entry.request)
    }
}

#[derive(Default)]
struct StableImageOutput {
    achieved: Rc<Cell<bool>>,
}

impl StableImageOutput {
    fn has_achieved_stable_image(&self) -> bool {
        self.achieved.get()
    }

    fn maybe_request_screenshot(&self, prefs: &AppPreferences, webview: WebView) {
        let output_path = prefs.output_image_path.clone();
        if !prefs.exit_after_stable_image && output_path.is_none() {
            return;
        }

        if self.achieved.get() {
            return;
        }

        let achieved = self.achieved.clone();
        webview.take_screenshot(None, move |image| {
            achieved.set(true);

            let Some(output_path) = output_path else {
                return;
            };

            let image = match image {
                Ok(image) => image,
                Err(error) => {
                    error!("Could not take screenshot: {error:?}");
                    return;
                }
            };

            let image_format = ImageFormat::from_path(&output_path).unwrap_or(ImageFormat::Png);
            if let Err(error) =
                DynamicImage::ImageRgba8(image).save_with_format(output_path, image_format)
            {
                error!("Failed to save screenshot: {error}.");
            }
        });
    }
}

pub(crate) struct RunningAppState {
    /// Host-side gamepad coordination and haptics state.
    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    gamepad: GamepadRuntime,

    /// Host-side WebDriver coordination and transport state.
    webdriver: WebDriverRuntime,

    /// Graphshell-specific preferences created during startup of the application.
    pub(crate) app_preferences: AppPreferences,

    /// The [`UserContentManager`] for all `WebView`s created.
    pub(crate) user_content_manager: Rc<UserContentManager>,

    /// File-backed stylesheets currently managed through the shared user content manager.
    managed_user_stylesheets: RefCell<Vec<ManagedUserStylesheet>>,

    /// Host-side screenshot/output capture state for stable-image workflows.
    stable_image_output: StableImageOutput,

    /// Owned Servo child-create requests waiting for reconcile-time acceptance.
    pending_create_store: PendingCreateStore,

    /// Whether or not program exit has been triggered. This means that all windows
    /// will be destroyed and shutdown will start at the end of the current event loop.
    exit_scheduled: Cell<bool>,

    /// Whether the user has enabled experimental preferences.
    experimental_preferences_enabled: Cell<bool>,

    /// Owns embedder runtime state (Servo + windows + focused window + event drains).
    /// Keep this as the last field so windows drop after other runtime references.
    /// See https://github.com/servo/servo/issues/36711.
    embedder_core: EmbedderCore,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use super::*;
    use crate::prefs::AppPreferences;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;
    use crate::shell::desktop::host::window::InputTarget;

    #[test]
    fn servo_callbacks_only_enqueue_events() {
        let running_state_source = include_str!("running_app_state.rs");
        let webview_delegate_source = include_str!("running_app_state/webview_delegate.rs");
        let window_source = include_str!("window.rs");

        for source in [running_state_source, webview_delegate_source, window_source] {
            assert!(
                !source.contains(concat!("Graph", "BrowserApp")),
                concat!(
                    "Servo callbacks must not reference ",
                    "Graph",
                    "BrowserApp",
                    " directly"
                )
            );
            assert!(
                !source.contains(concat!("Graph", "Workspace")),
                concat!(
                    "Servo callbacks must not reference ",
                    "Graph",
                    "Workspace",
                    " directly"
                )
            );
            assert!(
                !source.contains(concat!("Graph", "Intent")),
                concat!(
                    "Servo callbacks must not reference ",
                    "Graph",
                    "Intent",
                    " directly"
                )
            );
            assert!(
                !source.contains(concat!("apply", "_intents")),
                concat!("Servo callbacks must not call the reducer ", "directly")
            );
        }
    }

    #[test]
    fn graph_event_sequencing_is_owned_by_window() {
        // Platform adapters must go through EmbedderWindow facade methods to emit graph
        // events. Direct access to the queue or its internal types breaks sequencing
        // guarantees (monotonic seq numbers, diagnostics instrumentation).
        let adapter_sources = [
            include_str!("headed_window.rs"),
            include_str!("headed_window/input_routing.rs"),
            include_str!("headed_window/clip_extraction.rs"),
            include_str!("headless_window.rs"),
        ];
        for source in adapter_sources {
            assert!(
                !source.contains(concat!("Window", "GraphEventQueue")),
                "Platform adapters must not reference WindowGraphEventQueue directly"
            );
            assert!(
                !source.contains(concat!("GraphSemantic", "Event")),
                "Platform adapters must not construct or name WebViewLifecycleEvent directly"
            );
            assert!(
                !source.contains(concat!("graph_events", ".enqueue")),
                "Platform adapters must not call graph_events.enqueue directly"
            );
        }
    }

    #[test]
    fn platform_adapters_do_not_reach_into_window_internals() {
        // EmbedderWindow exposes a public facade. Adapters must not bypass it by naming
        // the private sub-structs or their field paths. Keeping this boundary intact lets
        // window internals be refactored without auditing all platform-specific code.
        let adapter_sources = [
            include_str!("headed_window.rs"),
            include_str!("headed_window/input_routing.rs"),
            include_str!("headed_window/clip_extraction.rs"),
            include_str!("headless_window.rs"),
        ];
        let forbidden = [
            concat!("Window", "ProjectionState"),
            concat!("Window", "RuntimeState"),
            concat!("Window", "UiSignals"),
            concat!("Window", "GraphEventQueue"),
            ".projection_state",
            ".runtime_state",
            ".ui_signals",
        ];
        for source in adapter_sources {
            for term in forbidden {
                assert!(
                    !source.contains(term),
                    "Platform adapter must not reference window-internal type or field `{term}`"
                );
            }
        }
    }

    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    #[test]
    fn gamepad_content_target_is_none_when_host_has_reclaimed_input() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        window.set_input_target(Some(InputTarget::Host));

        assert_eq!(
            crate::shell::desktop::host::gamepad_runtime::resolve_content_webview_id(&window),
            None
        );
    }
}

impl RunningAppState {
    pub(crate) fn new(
        servo: Servo,
        app_preferences: AppPreferences,
        event_loop_waker: Box<dyn EventLoopWaker>,
        user_content_manager: Rc<UserContentManager>,
        default_preferences: Preferences,
        #[cfg(all(
            feature = "gamepad",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        gamepad_provider: Option<Rc<AppGamepadProvider>>,
    ) -> Self {
        servo.set_delegate(Rc::new(GraphshellServoDelegate));
        let embedder_core = EmbedderCore::new(servo);

        let webdriver = WebDriverRuntime::new(
            app_preferences.webdriver_port.get(),
            event_loop_waker,
            default_preferences,
        );
        #[cfg(all(
            feature = "gamepad",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        let gamepad = GamepadRuntime::new(gamepad_provider);

        let experimental_preferences_enabled =
            Cell::new(app_preferences.experimental_preferences_enabled);
        let managed_user_stylesheets = app_preferences
            .user_stylesheets
            .iter()
            .filter_map(|stylesheet| {
                stylesheet
                    .url()
                    .to_file_path()
                    .ok()
                    .map(|path| ManagedUserStylesheet {
                        path: path.to_string_lossy().into_owned(),
                        stylesheet: stylesheet.clone(),
                    })
            })
            .collect();

        Self {
            #[cfg(all(
                feature = "gamepad",
                not(any(target_os = "android", target_env = "ohos"))
            ))]
            gamepad,
            webdriver,
            app_preferences,
            stable_image_output: Default::default(),
            pending_create_store: Default::default(),
            exit_scheduled: Default::default(),
            user_content_manager,
            managed_user_stylesheets: RefCell::new(managed_user_stylesheets),
            experimental_preferences_enabled,
            embedder_core,
        }
    }

    pub(crate) fn open_window(
        self: &Rc<Self>,
        platform_window: Rc<dyn PlatformWindow>,
        initial_url: Url,
    ) -> Rc<EmbedderWindow> {
        let window = Rc::new(EmbedderWindow::new(
            platform_window.clone(),
            self.embedder_core.graph_event_sequence_source(),
        ));
        self.embedder_core.insert_window(window.clone());
        window.notify_host_open_request(
            initial_url.to_string(),
            crate::app::OpenSurfaceSource::WindowBootstrap,
            None,
            None,
        );

        // If the window already has platform focus, mark it as focused in our application state.
        if platform_window.has_platform_focus() {
            self.focus_window(window.clone());
        }

        window
    }

    pub(crate) fn windows<'a>(&'a self) -> Ref<'a, HashMap<EmbedderWindowId, Rc<EmbedderWindow>>> {
        self.embedder_core.windows()
    }

    pub(crate) fn focused_window(&self) -> Option<Rc<EmbedderWindow>> {
        self.embedder_core.focused_window()
    }

    pub(crate) fn focus_window(&self, window: Rc<EmbedderWindow>) {
        window.focus();
        self.embedder_core.focus_window(window);
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn window(&self, id: EmbedderWindowId) -> Option<Rc<EmbedderWindow>> {
        self.embedder_core.window(id)
    }

    pub(crate) fn webview_by_id(&self, webview_id: WebViewId) -> Option<WebView> {
        self.maybe_window_for_webview_id(webview_id)?
            .webview_by_id(webview_id)
    }

    pub(crate) fn servo(&self) -> &Servo {
        self.embedder_core.servo()
    }

    pub(crate) fn schedule_exit(&self) {
        // When explicitly required to shutdown, unset webdriver port
        // which allows normal shutdown.
        // Note that when not explicitly required to shutdown, we still keep Servo alive
        // when all tabs are closed when `webdriver_port` enabled, which is necessary
        // to run wpt test using servodriver.
        self.app_preferences.webdriver_port.set(None);
        self.exit_scheduled.set(true);

        #[cfg(all(
            any(coverage, llvm_pgo),
            any(target_os = "android", target_env = "ohos")
        ))]
        {
            use std::ffi::CString;

            use crate::prefs::default_config_dir;

            let mut profile_path = default_config_dir().expect("Need a config dir");
            profile_path.push("profiles/");
            let filename = format!(
                "{}/profile-%h-%p.profraw",
                profile_path.to_str().expect("Should be unicode")
            );
            let c_filename = CString::new(filename).expect("Need a valid cstring");
            unsafe {
                __llvm_profile_set_filename(c_filename.as_ptr() as *const c_char);
                __llvm_profile_write_file()
            }
        }
    }

    pub(crate) fn store_pending_create_request(
        &self,
        request: CreateNewWebViewRequest,
    ) -> PendingCreateToken {
        self.pending_create_store.store(request)
    }

    pub(crate) fn take_pending_create_request(
        &self,
        token: PendingCreateToken,
    ) -> Option<CreateNewWebViewRequest> {
        self.pending_create_store.take(token)
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn experimental_preferences_enabled(&self) -> bool {
        self.experimental_preferences_enabled.get()
    }

    #[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
    pub(crate) fn set_experimental_preferences_enabled(&self, new_value: bool) {
        let old_value = self.experimental_preferences_enabled.replace(new_value);
        if old_value == new_value {
            return;
        }
        for pref in EXPERIMENTAL_PREFS {
            self.embedder_core
                .servo()
                .set_preference(pref, PrefValue::Bool(new_value));
        }
    }

    /// Close any [`EmbedderWindow`] that doesn't have an open [`WebView`].
    fn close_empty_windows(&self) {
        self.embedder_core
            .close_empty_windows(self.exit_scheduled.get());
    }

    pub(crate) fn take_pending_graph_events(&self) -> Vec<WebViewLifecycleEvent> {
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        let started = Instant::now();
        let events = self.embedder_core.drain_window_graph_events();
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "running_app_state::take_pending_graph_events",
            started.elapsed().as_micros() as u64,
        );
        events
    }

    /// Spins the internal application event loop.
    ///
    /// - Notifies Servo about incoming gamepad events
    /// - Spin the Servo event loop, which will update Servo's embedding layer and trigger
    ///   delegate methods.
    ///
    /// Returns true if the event loop should continue spinning and false if it should exit.
    pub(crate) fn spin_event_loop(
        self: &Rc<Self>,
        create_platform_window: Option<&dyn Fn(Url) -> Rc<dyn PlatformWindow>>,
    ) -> bool {
        self.webdriver.handle_messages(self, create_platform_window);

        #[cfg(all(
            feature = "gamepad",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        if pref!(dom_gamepad_enabled) {
            self.handle_gamepad_events();
        }

        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        let servo_spin_started = Instant::now();
        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        diagnostics::emit_event(DiagnosticEvent::Span {
            name: "servo.spin_event_loop",
            phase: SpanPhase::Enter,
            duration_us: None,
        });

        self.embedder_core.servo().spin_event_loop();

        #[cfg(all(
            feature = "diagnostics",
            not(any(target_os = "android", target_env = "ohos"))
        ))]
        {
            let elapsed = servo_spin_started.elapsed().as_micros() as u64;
            diagnostics::emit_event(DiagnosticEvent::MessageReceived {
                channel_id: "servo.event_loop.spin",
                latency_us: elapsed,
            });
            diagnostics::emit_event(DiagnosticEvent::Span {
                name: "servo.spin_event_loop",
                phase: SpanPhase::Exit,
                duration_us: Some(elapsed),
            });
        }

        for window in self.embedder_core.windows().values() {
            window.update_and_request_repaint_if_necessary(self);
        }

        if self.app_preferences.exit_after_stable_image
            && self.stable_image_output.has_achieved_stable_image()
        {
            self.schedule_exit();
        }

        self.close_empty_windows();

        // When no more windows are open, exit the application. Do not do this when
        // running WebDriver, which expects to keep running with no WebView open.
        if self.app_preferences.webdriver_port.get().is_none()
            && self.embedder_core.window_count() == 0
        {
            self.schedule_exit()
        }

        !self.exit_scheduled.get()
    }

    pub(crate) fn maybe_window_for_webview_id(
        &self,
        webview_id: WebViewId,
    ) -> Option<Rc<EmbedderWindow>> {
        self.embedder_core.maybe_window_for_webview_id(webview_id)
    }

    pub(crate) fn window_for_webview_id(&self, webview_id: WebViewId) -> Rc<EmbedderWindow> {
        self.maybe_window_for_webview_id(webview_id)
            .expect("Looking for unexpected WebView: {webview_id:?}")
    }

    pub(crate) fn platform_window_for_webview_id(
        &self,
        webview_id: WebViewId,
    ) -> Rc<dyn PlatformWindow> {
        self.window_for_webview_id(webview_id).platform_window()
    }

    /// If configured, request a screenshot of the [`WebView`] for stable-image workflows.
    fn maybe_request_screenshot(&self, webview: WebView) {
        self.stable_image_output
            .maybe_request_screenshot(&self.app_preferences, webview);
    }

    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    pub(crate) fn handle_gamepad_events(&self) {
        self.gamepad.handle_events(self.focused_window());
    }

    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    pub(crate) fn take_pending_gamepad_ui_commands(&self) -> Vec<GamepadUiCommand> {
        self.gamepad.take_pending_ui_commands()
    }

    pub(crate) fn handle_focused(&self, window: Rc<EmbedderWindow>) {
        self.embedder_core.focus_window(window);
    }

    pub(crate) fn user_stylesheet_settings_snapshot(&self) -> Vec<WorkspaceUserStylesheetSetting> {
        self.managed_user_stylesheets
            .borrow()
            .iter()
            .map(|entry| WorkspaceUserStylesheetSetting {
                path: entry.path.clone(),
                enabled: true,
            })
            .collect()
    }

    pub(crate) fn replace_user_stylesheets(&self, stylesheets: &[RuntimeUserStylesheetSpec]) {
        let previous = {
            let mut managed = self.managed_user_stylesheets.borrow_mut();
            std::mem::take(&mut *managed)
        };

        for entry in previous {
            self.user_content_manager
                .remove_stylesheet(entry.stylesheet);
        }

        let mut next = Vec::with_capacity(stylesheets.len());
        for stylesheet in stylesheets {
            let user_stylesheet = Rc::new(UserStyleSheet::new(
                stylesheet.source.clone(),
                Url::from_file_path(&stylesheet.path).unwrap(),
            ));
            self.user_content_manager
                .add_stylesheet(user_stylesheet.clone());
            next.push(ManagedUserStylesheet {
                path: stylesheet.path.to_string_lossy().into_owned(),
                stylesheet: user_stylesheet,
            });
        }

        *self.managed_user_stylesheets.borrow_mut() = next;
    }
}

impl WebViewCreationContext for RunningAppState {
    fn servo(&self) -> &Servo {
        self.servo()
    }

    fn user_content_manager(&self) -> Rc<UserContentManager> {
        self.user_content_manager.clone()
    }

    fn webview_delegate(self: Rc<Self>) -> Rc<dyn WebViewDelegate> {
        Rc::new(RunningAppStateWebViewDelegate { state: self })
    }
}

struct GraphshellServoDelegate;
impl ServoDelegate for GraphshellServoDelegate {
    fn notify_devtools_server_started(&self, port: u16, _token: String) {
        info!("Devtools Server running on port {port}");
    }

    fn request_devtools_connection(&self, request: AllowOrDenyRequest) {
        request.allow();
    }

    fn notify_error(&self, error: ServoError) {
        error!("Saw Servo error: {error:?}!");
    }

    fn show_console_message(&self, level: ConsoleLogLevel, message: String) {
        // For messages without a WebView context, apply platform-specific behavior
        #[cfg(not(any(target_os = "android", target_env = "ohos")))]
        println!("{message}");
        log::log!(level.into(), "{message}");
    }
}
