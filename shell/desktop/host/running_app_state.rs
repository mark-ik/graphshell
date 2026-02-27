/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared state and methods for desktop and EGL implementations.

use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::Deref;
use std::rc::Rc;
#[cfg(all(
    feature = "diagnostics",
    not(any(target_os = "android", target_env = "ohos"))
))]
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender, unbounded};
use euclid::Rect;
use image::{DynamicImage, ImageFormat, RgbaImage};
#[cfg(all(
    any(coverage, llvm_pgo),
    any(target_os = "android", target_env = "ohos")
))]
use libc::c_char;
use log::{error, info, warn};
use servo::{
    AllowOrDenyRequest, AuthenticationRequest, CSSPixel, ConsoleLogLevel, CreateNewWebViewRequest,
    DeviceIntPoint, DeviceIntSize, EmbedderControl, EmbedderControlId, EventLoopWaker,
    GenericSender, InputEvent, InputEventId, InputEventResult, JSValue, LoadStatus,
    MediaSessionEvent, PermissionRequest, PrefValue, Preferences, ScreenshotCaptureError, Servo,
    ServoDelegate, ServoError, TraversalId, UserContentManager, WebDriverCommandMsg,
    WebDriverJSResult, WebDriverLoadStatus, WebDriverScriptCommand, WebDriverSenders, WebView,
    WebViewDelegate, WebViewId, pref,
};
use url::Url;

use crate::prefs::{AppPreferences, EXPERIMENTAL_PREFS};
use crate::shell::desktop::host::embedder::EmbedderCore;
#[cfg(all(
    feature = "gamepad",
    not(any(target_os = "android", target_env = "ohos"))
))]
pub(crate) use crate::shell::desktop::host::gamepad::AppGamepadProvider;
use crate::shell::desktop::host::window::{
    EmbedderWindow, EmbedderWindowId, GraphSemanticEvent, PlatformWindow, WebViewCreationContext,
};
#[cfg(all(
    feature = "diagnostics",
    not(any(target_os = "android", target_env = "ohos"))
))]
use crate::shell::desktop::runtime::diagnostics::{self, DiagnosticEvent, SpanPhase};
use crate::webdriver::WebDriverEmbedderControls;

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

    /// The [`WebView`] that is currently active. This is the [`WebView`] that is shown and has
    /// input focus.
    active_webview_id: Option<WebViewId>,
}

impl WebViewCollection {
    pub fn add(&mut self, webview: WebView) {
        let id = webview.id();
        webview.show();
        self.creation_order.push(id);
        self.webviews.insert(id, webview);
    }

    /// Removes a webview from the collection by [`WebViewId`]. If the removed [`WebView`] was the active
    /// [`WebView`] then the next newest [`WebView`] will be activated.
    pub fn remove(&mut self, id: WebViewId) -> Option<WebView> {
        self.creation_order.retain(|&webview_id| webview_id != id);
        let removed_webview = self.webviews.remove(&id);

        if self.active_webview_id == Some(id) {
            self.active_webview_id = None;
            if let Some(newest) = self.creation_order.last() {
                self.activate_webview(*newest);
            }
        }

        removed_webview
    }

    pub fn get(&self, id: WebViewId) -> Option<&WebView> {
        self.webviews.get(&id)
    }

    pub fn contains(&self, id: WebViewId) -> bool {
        self.webviews.contains_key(&id)
    }

    pub fn active_id(&self) -> Option<WebViewId> {
        self.active_webview_id
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

    pub(crate) fn activate_webview(&mut self, id_to_activate: WebViewId) {
        assert!(self.creation_order.contains(&id_to_activate));

        self.active_webview_id = Some(id_to_activate);
        if let Some(webview) = self.webviews.get(&id_to_activate) {
            webview.show();
            webview.focus();
        }
    }

    pub(crate) fn activate_webview_by_index(&mut self, index: usize) {
        self.activate_webview(
            *self
                .creation_order
                .get(index)
                .expect("Tried to activate an unknown WebView"),
        );
    }
}

/// A command received via the user interacting with the user interface.
#[cfg_attr(any(target_os = "android", target_env = "ohos"), expect(dead_code))]
pub(crate) enum UserInterfaceCommand {
    ReloadAll,
}

pub(crate) struct RunningAppState {
    /// The gamepad provider, used for handling gamepad events and set on each WebView.
    /// May be `None` if gamepad support is disabled or failed to initialize.
    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    gamepad_provider: Option<Rc<AppGamepadProvider>>,

    /// The [`WebDriverSenders`] used to reply to pending WebDriver requests.
    pub(crate) webdriver_senders: RefCell<WebDriverSenders>,

    /// When running in WebDriver mode, [`WebDriverEmbedderControls`] is a virtual container
    /// for all embedder controls. This overrides the normal behavior where these controls
    /// are shown in the GUI or not processed at all in headless mode.
    pub(crate) webdriver_embedder_controls: WebDriverEmbedderControls,

    /// A [`HashMap`] of pending WebDriver events. It is the WebDriver embedder's responsibility
    /// to inform the WebDriver server when the event has been fully handled. This map is used
    /// to report back to WebDriver when that happens.
    pub(crate) pending_webdriver_events: RefCell<HashMap<InputEventId, Sender<()>>>,

    /// A [`Receiver`] for receiving commands from a running WebDriver server, if WebDriver
    /// was enabled.
    pub(crate) webdriver_receiver: Option<Receiver<WebDriverCommandMsg>>,

    /// servoshell specific preferences created during startup of the application.
    pub(crate) app_preferences: AppPreferences,

    /// Whether or not the application has achieved stable image output. This is used
    /// for the `exit_after_stable_image` option.
    pub(crate) achieved_stable_image: Rc<Cell<bool>>,

    /// The [`UserContentManager`] for all `WebView`s created.
    pub(crate) user_content_manager: Rc<UserContentManager>,

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
    #[test]
    fn servo_callbacks_only_enqueue_events() {
        let running_state_source = include_str!("running_app_state.rs");
        let window_source = include_str!("window.rs");

        for source in [running_state_source, window_source] {
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
        servo.set_delegate(Rc::new(ServoShellServoDelegate));
        let embedder_core = EmbedderCore::new(servo);

        let webdriver_receiver = app_preferences.webdriver_port.get().map(|port| {
            let (embedder_sender, embedder_receiver) = unbounded();
            webdriver_server::start_server(
                port,
                embedder_sender,
                event_loop_waker,
                default_preferences,
            );
            embedder_receiver
        });

        let experimental_preferences_enabled =
            Cell::new(app_preferences.experimental_preferences_enabled);

        Self {
            #[cfg(all(
                feature = "gamepad",
                not(any(target_os = "android", target_env = "ohos"))
            ))]
            gamepad_provider,
            webdriver_senders: RefCell::default(),
            webdriver_embedder_controls: Default::default(),
            pending_webdriver_events: Default::default(),
            webdriver_receiver,
            app_preferences,
            achieved_stable_image: Default::default(),
            exit_scheduled: Default::default(),
            user_content_manager,
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
        window.create_and_activate_toplevel_webview(self.clone(), initial_url);
        self.embedder_core.insert_window(window.clone());

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

    pub(crate) fn webdriver_receiver(&self) -> Option<&Receiver<WebDriverCommandMsg>> {
        self.webdriver_receiver.as_ref()
    }

    pub(crate) fn servo(&self) -> &Servo {
        self.embedder_core.servo()
    }

    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    pub(crate) fn gamepad_provider(&self) -> Option<Rc<AppGamepadProvider>> {
        self.gamepad_provider.clone()
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

    pub(crate) fn take_pending_graph_events(&self) -> Vec<GraphSemanticEvent> {
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
        // We clone here to avoid a double borrow. User interface commands can update the list of windows.
        let windows: Vec<_> = self.embedder_core.windows().values().cloned().collect();
        for window in windows {
            window.handle_interface_commands(self, create_platform_window);
        }

        self.handle_webdriver_messages(create_platform_window);

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

        if self.app_preferences.exit_after_stable_image && self.achieved_stable_image.get() {
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

    /// If we are exiting after achieving a stable image or we want to save the display of the
    /// [`WebView`] to an image file, request a screenshot of the [`WebView`].
    fn maybe_request_screenshot(&self, webview: WebView) {
        let output_path = self.app_preferences.output_image_path.clone();
        if !self.app_preferences.exit_after_stable_image && output_path.is_none() {
            return;
        }

        // Never request more than a single screenshot for now.
        let achieved_stable_image = self.achieved_stable_image.clone();
        if achieved_stable_image.get() {
            return;
        }

        webview.take_screenshot(None, move |image| {
            achieved_stable_image.set(true);

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

    pub(crate) fn set_pending_traversal(
        &self,
        traversal_id: TraversalId,
        sender: GenericSender<WebDriverLoadStatus>,
    ) {
        self.webdriver_senders
            .borrow_mut()
            .pending_traversals
            .insert(traversal_id, sender);
    }

    pub(crate) fn set_load_status_sender(
        &self,
        webview_id: WebViewId,
        sender: GenericSender<WebDriverLoadStatus>,
    ) {
        self.webdriver_senders
            .borrow_mut()
            .load_status_senders
            .insert(webview_id, sender);
    }

    fn remove_load_status_sender(&self, webview_id: WebViewId) {
        self.webdriver_senders
            .borrow_mut()
            .load_status_senders
            .remove(&webview_id);
    }

    fn set_script_command_interrupt_sender(
        &self,
        sender: Option<GenericSender<WebDriverJSResult>>,
    ) {
        self.webdriver_senders
            .borrow_mut()
            .script_evaluation_interrupt_sender = sender;
    }

    pub(crate) fn handle_webdriver_input_event(
        &self,
        webview_id: WebViewId,
        input_event: InputEvent,
        response_sender: Option<Sender<()>>,
    ) {
        if let Some(webview) = self.webview_by_id(webview_id) {
            let event_id = webview.notify_input_event(input_event);
            if let Some(response_sender) = response_sender {
                self.pending_webdriver_events
                    .borrow_mut()
                    .insert(event_id, response_sender);
            }
        } else {
            error!("Could not find WebView ({webview_id:?}) for WebDriver event: {input_event:?}");
        };
    }

    pub(crate) fn handle_webdriver_screenshot(
        &self,
        webview_id: WebViewId,
        rect: Option<Rect<f32, CSSPixel>>,
        result_sender: Sender<Result<RgbaImage, ScreenshotCaptureError>>,
    ) {
        if let Some(webview) = self.webview_by_id(webview_id) {
            let rect = rect.map(|rect| rect.to_box2d().into());
            webview.take_screenshot(rect, move |result| {
                if let Err(error) = result_sender.send(result) {
                    warn!("Failed to send response to TakeScreenshot: {error}");
                }
            });
        } else if let Err(error) =
            result_sender.send(Err(ScreenshotCaptureError::WebViewDoesNotExist))
        {
            error!("Failed to send response to TakeScreenshot: {error}");
        }
    }

    pub(crate) fn handle_webdriver_script_command(&self, script_command: &WebDriverScriptCommand) {
        match script_command {
            WebDriverScriptCommand::ExecuteScriptWithCallback(_webview_id, response_sender) => {
                // Give embedder a chance to interrupt the script command.
                // Webdriver only handles 1 script command at a time, so we can
                // safely set a new interrupt sender and remove the previous one here.
                self.set_script_command_interrupt_sender(Some(response_sender.clone()));
            }
            WebDriverScriptCommand::AddLoadStatusSender(webview_id, load_status_sender) => {
                self.set_load_status_sender(*webview_id, load_status_sender.clone());
            }
            WebDriverScriptCommand::RemoveLoadStatusSender(webview_id) => {
                self.remove_load_status_sender(*webview_id);
            }
            _ => {
                self.set_script_command_interrupt_sender(None);
            }
        }
    }

    pub(crate) fn handle_webdriver_load_url(
        &self,
        webview_id: WebViewId,
        url: Url,
        load_status_sender: GenericSender<WebDriverLoadStatus>,
    ) {
        let Some(webview) = self.webview_by_id(webview_id) else {
            return;
        };

        self.platform_window_for_webview_id(webview_id)
            .dismiss_embedder_controls_for_webview(webview_id);

        info!("Loading URL in webview {}: {}", webview_id, url);
        self.set_load_status_sender(webview_id, load_status_sender);
        webview.load(url);
    }

    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    pub(crate) fn handle_gamepad_events(&self) {
        let Some(gamepad_provider) = self.gamepad_provider.as_ref() else {
            return;
        };
        let Some(active_webview) = self.focused_window().and_then(|window| {
            let webview_id = window
                .platform_window()
                .preferred_input_webview_id(&window)?;
            window.webview_by_id(webview_id)
        }) else {
            return;
        };
        gamepad_provider.handle_gamepad_events(active_webview);
    }

    pub(crate) fn handle_focused(&self, window: Rc<EmbedderWindow>) {
        self.embedder_core.focus_window(window);
    }

    /// Interrupt any ongoing WebDriver-based script evaluation.
    ///
    /// From <https://w3c.github.io/webdriver/#dfn-execute-a-function-body>:
    /// > The rules to execute a function body are as follows. The algorithm returns
    /// > an ECMAScript completion record.
    /// >
    /// > If at any point during the algorithm a user prompt appears, immediately return
    /// > Completion { Type: normal, Value: null, Target: empty }, but continue to run the
    /// >  other steps of this algorithm in parallel.
    fn interrupt_webdriver_script_evaluation(&self) {
        if let Some(sender) = &self
            .webdriver_senders
            .borrow()
            .script_evaluation_interrupt_sender
        {
            sender.send(Ok(JSValue::Null)).unwrap_or_else(|err| {
                info!(
                    "Notify dialog appear failed. Maybe the channel to webdriver is closed: {err}"
                );
            });
        }
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

    #[cfg(all(
        feature = "gamepad",
        not(any(target_os = "android", target_env = "ohos"))
    ))]
    fn gamepad_provider(&self) -> Option<Rc<AppGamepadProvider>> {
        self.gamepad_provider()
    }
}

struct RunningAppStateWebViewDelegate {
    state: Rc<RunningAppState>,
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
        let mut webdriver_state = self.webdriver_senders.borrow_mut();
        if let Entry::Occupied(entry) = webdriver_state.pending_traversals.entry(traversal_id) {
            let sender = entry.remove();
            let _ = sender.send(WebDriverLoadStatus::Complete);
        }
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
        self.platform_window_for_webview_id(webview.id())
            .show_http_authentication_dialog(webview.id(), authentication_request);
    }

    fn request_create_new(&self, parent_webview: WebView, request: CreateNewWebViewRequest) {
        let window = self.window_for_webview_id(parent_webview.id());
        let platform_window = window.platform_window();
        let webview = request
            .builder(platform_window.rendering_context())
            .hidpi_scale_factor(platform_window.hidpi_scale_factor())
            .delegate(parent_webview.delegate())
            .build();

        webview.notify_theme_change(platform_window.theme());
        window.add_webview(webview.clone());
        window.notify_create_new_webview(parent_webview, webview.clone());

        // When WebDriver is enabled, do not focus and raise the WebView to the top,
        // as that is what the specification expects. Otherwise, we would like `window.open()`
        // to create a new foreground tab
        if self.app_preferences.webdriver_port.get().is_none() {
            window.activate_webview(webview.id());
        } else {
            webview.hide();
        }
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
        if let Some(response_sender) = self.pending_webdriver_events.borrow_mut().remove(&id) {
            let _ = response_sender.send(());
        }
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
            if let Some(sender) = self
                .webdriver_senders
                .borrow_mut()
                .load_status_senders
                .remove(&webview.id())
            {
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
        devices: Vec<String>,
        response_sender: GenericSender<Option<String>>,
    ) {
        self.platform_window_for_webview_id(webview.id())
            .show_bluetooth_device_dialog(webview.id(), devices, response_sender);
    }

    fn request_permission(&self, webview: WebView, permission_request: PermissionRequest) {
        self.platform_window_for_webview_id(webview.id())
            .show_permission_dialog(webview.id(), permission_request);
    }

    fn notify_new_frame_ready(&self, webview: WebView) {
        self.window_for_webview_id(webview.id()).set_needs_repaint();
    }

    fn show_embedder_control(&self, webview: WebView, embedder_control: EmbedderControl) {
        if self.app_preferences.webdriver_port.get().is_some() {
            if matches!(&embedder_control, EmbedderControl::SimpleDialog(..)) {
                self.interrupt_webdriver_script_evaluation();

                // Dialogs block the page load, so need need to notify WebDriver
                if let Some(sender) = self
                    .webdriver_senders
                    .borrow_mut()
                    .load_status_senders
                    .get(&webview.id())
                {
                    let _ = sender.send(WebDriverLoadStatus::Blocked);
                };
            }

            self.webdriver_embedder_controls
                .show_embedder_control(webview.id(), embedder_control);
            return;
        }

        self.window_for_webview_id(webview.id())
            .show_embedder_control(webview, embedder_control);
    }

    fn hide_embedder_control(&self, webview: WebView, embedder_control_id: EmbedderControlId) {
        if self.app_preferences.webdriver_port.get().is_some() {
            self.webdriver_embedder_controls
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
        tree_update: accesskit::TreeUpdate,
    ) {
        self.platform_window_for_webview_id(webview.id())
            .notify_accessibility_tree_update(webview, tree_update);
    }
}

struct ServoShellServoDelegate;
impl ServoDelegate for ServoShellServoDelegate {
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
