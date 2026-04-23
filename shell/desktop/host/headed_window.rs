/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A winit window implementation.

#![deny(clippy::panic)]
#![deny(clippy::unwrap_used)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use euclid::{Length, Point2D, Rect, Scale, Size2D};
use keyboard_types::ShortcutMatcher;
use log::{debug, info, warn};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawWindowHandle};
use servo::{
    AuthenticationRequest, BluetoothDeviceSelectionRequest, Cursor, DeviceIndependentIntRect,
    DeviceIndependentPixel, DeviceIntPoint, DeviceIntRect, DeviceIntSize, DevicePixel, DevicePoint,
    EmbedderControl, EmbedderControlId, ImeEvent, InputEvent, InputEventId, InputEventResult,
    InputMethodControl, KeyboardEvent, MouseLeftViewportEvent, OffscreenRenderingContext,
    PermissionRequest, RenderingContextCore, ScreenGeometry, Theme, TouchEvent,
    TouchEventType, TouchId, WebView, WebViewId, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext, convert_rect_to_css_pixel,
};
use url::Url;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
#[cfg(target_os = "linux")]
use winit::platform::wayland::WindowAttributesExtWayland;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use winit::window::Icon;
#[cfg(target_os = "macos")]
use {
    objc2_app_kit::{NSColorSpace, NSView},
    objc2_foundation::MainThreadMarker,
};

use crate::prefs::AppPreferences;
use crate::shell::desktop::host::accelerated_gl_media::setup_gl_accelerated_media;
use crate::shell::desktop::host::event_loop::AppEvent;
use crate::shell::desktop::host::geometry::{
    winit_position_to_euclid_point, winit_size_to_euclid_size,
};
use crate::shell::desktop::host::keyutils::CMD_OR_CONTROL;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::{
    EmbedderWindow, EmbedderWindowId, LINE_HEIGHT, LINE_WIDTH, MIN_WINDOW_INNER_SIZE,
    PlatformWindow, PlatformWindowDialogs, PlatformWindowOps, PlatformWindowRendering,
    PlatformWindowSignals,
};
use crate::shell::desktop::lifecycle::webview_status_sync::{
    renderer_id_from_servo, servo_webview_id_from_renderer,
};
use crate::shell::desktop::render_backend::UiHostRenderBootstrap;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UX_NAVIGATION_TRANSITION;
use crate::shell::desktop::ui::dialog::Dialog;
use crate::shell::desktop::ui::gui::EguiHost;
use crate::shell::desktop::ui::toolbar_routing::ToolbarNavAction;

mod clip_extraction;
mod embedder_controls;
mod input_routing;
mod xr;

pub(crate) const INITIAL_WINDOW_TITLE: &str = "Graphshell";

pub struct HeadedWindow {
    /// The egui interface that is responsible for showing the user interface elements of
    /// this headed `Window`.
    gui: RefCell<EguiHost>,
    screen_size: Size2D<u32, DeviceIndependentPixel>,
    monitor: winit::monitor::MonitorHandle,
    webview_relative_mouse_point: Cell<Point2D<f32, DevicePixel>>,
    /// The inner size of the window in physical pixels which excludes OS decorations.
    /// It equals viewport size + (0, toolbar height).
    inner_size: Cell<PhysicalSize<u32>>,
    fullscreen: Cell<bool>,
    device_pixel_ratio_override: Option<f32>,
    xr_window_poses: RefCell<Vec<Rc<xr::XRWindowPose>>>,
    modifiers_state: Cell<ModifiersState>,
    /// The `RenderingContext` of Servo itself. This is used to render Servo results
    /// temporarily until they can be blitted into the egui scene.
    rendering_context: Rc<OffscreenRenderingContext>,
    /// The RenderingContext that renders directly onto the Window. This is used as
    /// the target of egui rendering and also where Servo rendering results are finally
    /// blitted.
    window_rendering_context: Rc<WindowRenderingContext>,
    /// A helper that simulates touch events when the `--simulate-touch-events` flag
    /// is enabled.
    touch_event_simulator: Option<TouchEventSimulator>,
    /// Keyboard events that have been sent to Servo that have still not been handled yet.
    /// When these are handled, they will optionally be used to trigger keybindings that
    /// are overridable by web content.
    pending_keyboard_events: RefCell<HashMap<InputEventId, KeyboardEvent>>,
    // Keep this as the last field of the struct to ensure that the rendering context is
    // dropped first.
    // (https://github.com/servo/servo/issues/36711)
    winit_window: winit::window::Window,
    /// The last title set on this window. We need to store this value here, as `winit::Window::title`
    /// is not supported very many platforms.
    last_title: RefCell<String>,
    /// The current set of open dialogs.
    dialogs: RefCell<HashMap<WebViewId, Vec<Dialog>>>,
    event_loop_proxy: EventLoopProxy<AppEvent>,
    /// A list of showing [`InputMethod`] interfaces.
    visible_input_methods: RefCell<Vec<EmbedderControlId>>,
    /// The position of the mouse cursor after the most recent `MouseMove` event.
    last_mouse_position: Cell<Option<Point2D<f32, DeviceIndependentPixel>>>,
    /// Most recent cursor requested by Servo while no dialog override is active.
    last_servo_cursor: Cell<Cursor>,
    /// True while dialogs force the cursor to default.
    dialog_cursor_override_active: Cell<bool>,
    /// Prevent overlapping pointer-stack JS requests while inspector mode is active.
    clip_inspector_pointer_request_in_flight: Cell<bool>,
}

impl HeadedWindow {
    pub(crate) fn new(
        app_preferences: &AppPreferences,
        event_loop: &ActiveEventLoop,
        event_loop_proxy: EventLoopProxy<AppEvent>,
        initial_url: Url,
    ) -> Rc<Self> {
        let no_native_titlebar = app_preferences.no_native_titlebar;
        let inner_size = app_preferences.initial_window_size;
        let window_attr = winit::window::Window::default_attributes()
            .with_title(INITIAL_WINDOW_TITLE.to_string())
            .with_decorations(!no_native_titlebar)
            .with_transparent(no_native_titlebar)
            .with_inner_size(LogicalSize::new(inner_size.width, inner_size.height))
            .with_min_inner_size(LogicalSize::new(
                MIN_WINDOW_INNER_SIZE.width,
                MIN_WINDOW_INNER_SIZE.height,
            ))
            // Must be invisible at startup; accesskit_winit setup needs to
            // happen before the window is shown for the first time.
            .with_visible(false);

        // Set a name so it can be pinned to taskbars in Linux.
        #[cfg(target_os = "linux")]
        let window_attr = window_attr.with_name("org.graphshell.Graphshell", "Graphshell");

        #[allow(deprecated)]
        let winit_window = event_loop
            .create_window(window_attr)
            .expect("Failed to create window.");

        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            let icon_bytes = include_bytes!("../../../resources/servo_64.png");
            winit_window.set_window_icon(Some(load_icon(icon_bytes)));
        }

        let window_handle = winit_window
            .window_handle()
            .expect("winit window did not have a window handle");
        HeadedWindow::force_srgb_color_space(window_handle.as_raw());

        let monitor = winit_window
            .current_monitor()
            .or_else(|| winit_window.available_monitors().nth(0))
            .expect("No monitor detected");

        let (screen_size, screen_scale) = app_preferences.screen_size_override.map_or_else(
            || (monitor.size(), winit_window.scale_factor()),
            |size| (PhysicalSize::new(size.width, size.height), 1.0),
        );
        let screen_scale: Scale<f64, DeviceIndependentPixel, DevicePixel> =
            Scale::new(screen_scale);
        let screen_size = (winit_size_to_euclid_size(screen_size).to_f64() / screen_scale).to_u32();
        let inner_size = winit_window.inner_size();

        let display_handle = event_loop
            .display_handle()
            .expect("could not get display handle from window");
        let window_handle = winit_window
            .window_handle()
            .expect("could not get window handle from window");
        let window_rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, inner_size)
                .expect("Could not create RenderingContext for Window"),
        );

        // Setup for GL accelerated media handling. This is only active on certain Linux platforms
        // and Windows.
        {
            let details = window_rendering_context.surfman_details();
            setup_gl_accelerated_media(details.0, details.1);
        }

        // Make sure the gl context is made current.
        if let Some(gl) = window_rendering_context.gl() {
            gl.make_current()
                .expect("Could not make window RenderingContext current");
        }

        let rendering_context = Rc::new(window_rendering_context.offscreen_context(inner_size));
        let render_host = UiHostRenderBootstrap::new(
            rendering_context.clone(),
            window_rendering_context.clone(),
            event_loop,
        );
        let gui = RefCell::new(EguiHost::new(
            &winit_window,
            event_loop,
            event_loop_proxy.clone(),
            render_host,
            initial_url,
            app_preferences.graph_data_dir.clone(),
            app_preferences.graph_snapshot_interval_secs,
            app_preferences.worker_idle_threshold_secs,
        ));

        debug!("Created window {:?}", winit_window.id());
        Rc::new(HeadedWindow {
            gui,
            winit_window,
            webview_relative_mouse_point: Cell::new(Point2D::zero()),
            fullscreen: Cell::new(false),
            inner_size: Cell::new(inner_size),
            monitor,
            screen_size,
            device_pixel_ratio_override: app_preferences.device_pixel_ratio_override,
            xr_window_poses: RefCell::new(vec![]),
            modifiers_state: Cell::new(ModifiersState::empty()),
            window_rendering_context,
            touch_event_simulator: app_preferences.simulate_touch_events.then(Default::default),
            pending_keyboard_events: Default::default(),
            rendering_context,
            last_title: RefCell::new(String::from(INITIAL_WINDOW_TITLE)),
            dialogs: Default::default(),
            event_loop_proxy,
            visible_input_methods: Default::default(),
            last_mouse_position: Default::default(),
            last_servo_cursor: Cell::new(Cursor::Default),
            dialog_cursor_override_active: Cell::new(false),
            clip_inspector_pointer_request_in_flight: Cell::new(false),
        })
    }

    pub(crate) fn winit_window(&self) -> &winit::window::Window {
        &self.winit_window
    }

    #[cfg_attr(not(target_os = "macos"), expect(unused_variables))]
    fn force_srgb_color_space(window_handle: RawWindowHandle) {
        #[cfg(target_os = "macos")]
        {
            if let RawWindowHandle::AppKit(handle) = window_handle {
                assert!(MainThreadMarker::new().is_some());
                unsafe {
                    let view = handle.ns_view.cast::<NSView>().as_ref();
                    view.window()
                        .expect("Should have a window")
                        .setColorSpace(Some(&NSColorSpace::sRGBColorSpace()));
                }
            }
        }
    }

    fn show_ime(&self, input_method: InputMethodControl) {
        let position = input_method.position();
        self.winit_window.set_ime_allowed(true);
        self.winit_window.set_ime_cursor_area(
            LogicalPosition::new(
                position.min.x,
                position.min.y + (self.toolbar_height().0 as i32),
            ),
            LogicalSize::new(
                position.max.x - position.min.x,
                position.max.y - position.min.y,
            ),
        );
    }

    pub(crate) fn for_each_active_dialog(
        &self,
        window: &EmbedderWindow,
        dialog_target_webview_id: Option<WebViewId>,
        toolbar_offset: Length<f32, DeviceIndependentPixel>,
        callback: impl Fn(&mut Dialog) -> bool,
    ) {
        // Important: this path must not borrow `self.gui`. It can be called while
        // `EguiHost::update` holds a mutable borrow of the same RefCell during redraw.
        let Some(dialog_webview_id) = dialog_target_webview_id else {
            return;
        };
        let mut all_dialogs = self.dialogs.borrow_mut();
        let had_any_active_dialog = all_dialogs.values().any(|entries| !entries.is_empty());
        let Some(active_dialogs) = all_dialogs.get_mut(&dialog_webview_id) else {
            return;
        };
        if active_dialogs.is_empty() {
            if self.dialog_cursor_override_active.replace(false) {
                input_routing::apply_platform_cursor(self, self.last_servo_cursor.get());
            }
            return;
        }

        // Force default cursor while dialog is open and restore the last Servo cursor once all
        // dialogs close.
        if !self.dialog_cursor_override_active.replace(true) {
            input_routing::apply_platform_cursor(self, Cursor::Default);
        }

        let length = active_dialogs.len();
        active_dialogs.retain_mut(|dialog| {
            dialog.set_toolbar_offset(toolbar_offset);
            callback(dialog)
        });
        if active_dialogs.is_empty() && self.dialog_cursor_override_active.replace(false) {
            input_routing::apply_platform_cursor(self, self.last_servo_cursor.get());
        }
        if length != active_dialogs.len() {
            window.set_needs_repaint();
        }
        let has_any_active_dialog = all_dialogs.values().any(|entries| !entries.is_empty());
        if had_any_active_dialog != has_any_active_dialog {
            emit_navigation_transition_host_dialog_capture();
        }
    }

    fn add_dialog(&self, webview_id: WebViewId, dialog: Dialog) {
        let mut dialogs = self.dialogs.borrow_mut();
        let had_any_active_dialog = dialogs.values().any(|entries| !entries.is_empty());
        dialogs.entry(webview_id).or_default().push(dialog);
        let has_any_active_dialog = dialogs.values().any(|entries| !entries.is_empty());
        if had_any_active_dialog != has_any_active_dialog {
            emit_navigation_transition_host_dialog_capture();
        }
    }

    fn remove_dialog(&self, webview_id: WebViewId, embedder_control_id: EmbedderControlId) {
        let mut dialogs = self.dialogs.borrow_mut();
        let had_any_active_dialog = dialogs.values().any(|entries| !entries.is_empty());
        if let Some(dialogs) = dialogs.get_mut(&webview_id) {
            dialogs.retain(|dialog| dialog.embedder_control_id() != Some(embedder_control_id));
        }
        dialogs.retain(|_, dialogs| !dialogs.is_empty());
        let has_any_active_dialog = dialogs.values().any(|entries| !entries.is_empty());
        if had_any_active_dialog != has_any_active_dialog {
            emit_navigation_transition_host_dialog_capture();
        }
    }

    fn has_any_active_dialog(&self) -> bool {
        let mut dialogs = self.dialogs.borrow_mut();
        dialogs.retain(|_, dialogs| !dialogs.is_empty());
        !dialogs.is_empty()
    }

    fn ui_or_dialog_capture_active(&self) -> bool {
        self.gui.borrow().ui_overlay_active() || self.has_any_active_dialog()
    }

    fn toolbar_height(&self) -> Length<f32, DeviceIndependentPixel> {
        self.gui.borrow().toolbar_height()
    }

    pub(crate) fn handle_winit_window_event(
        &self,
        state: Rc<RunningAppState>,
        window: Rc<EmbedderWindow>,
        event: WindowEvent,
    ) {
        if event == WindowEvent::RedrawRequested {
            // WARNING: do not defer painting or presenting to some later tick of the event
            // loop or Graphshell may become unresponsive! (servo#30312)
            let mut gui = self.gui.borrow_mut();
            // Store state Rc before calling update
            gui.set_state(state.clone());
            gui.update(&state, &window, self);
            gui.paint(&self.winit_window);
        }

        let forward_mouse_event_to_egui = |point: Option<PhysicalPosition<f64>>| {
            if self.gui.borrow().is_graph_view() {
                return true;
            }
            if self.ui_or_dialog_capture_active() || self.gui.borrow().egui_wants_pointer_input() {
                return true;
            }

            let Some(point) = input_routing::resolve_pointer_position(self, point) else {
                return true;
            };

            self.last_mouse_position.set(Some(point));
            self.gui.borrow().webview_at_point(point).is_none()
        };

        // Handle the event
        let mut consumed = false;
        match event {
            WindowEvent::Focused(true) => state.handle_focused(window.clone()),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // Intercept ScaleFactorChanged before backend window-event handling so
                // we can use our own logic for calculating the scale factor and set egui’s
                // scale factor to that value manually.
                let desired_scale_factor = self.hidpi_scale_factor().get();
                let effective_egui_zoom_factor = desired_scale_factor / scale_factor as f32;

                info!(
                    "window scale factor changed to {}, setting egui zoom factor to {}",
                    scale_factor, effective_egui_zoom_factor
                );

                self.gui
                    .borrow()
                    .set_zoom_factor(effective_egui_zoom_factor);

                window.hidpi_scale_factor_changed();

                // Request a winit redraw event, so we can recomposite, update and paint
                // the GUI, and present the new frame.
                self.winit_window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                window.schedule_close();
                consumed = true;
            }
            WindowEvent::CursorMoved { position, .. }
                if !forward_mouse_event_to_egui(Some(position)) => {}
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Forward,
                ..
            } => {
                if let Some(webview_id) = input_routing::resolved_input_webview_id(self, &window) {
                    self.gui
                        .borrow_mut()
                        .set_embedded_content_focus_webview(Some(webview_id));
                    window.retarget_input_to_webview(webview_id);
                    self.gui
                        .borrow_mut()
                        .request_toolbar_nav_action_for_webview(
                            webview_id,
                            ToolbarNavAction::Forward,
                        );
                    window.set_needs_update();
                }
                consumed = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Back,
                ..
            } => {
                if let Some(webview_id) = input_routing::resolved_input_webview_id(self, &window) {
                    self.gui
                        .borrow_mut()
                        .set_embedded_content_focus_webview(Some(webview_id));
                    window.retarget_input_to_webview(webview_id);
                    self.gui
                        .borrow_mut()
                        .request_toolbar_nav_action_for_webview(webview_id, ToolbarNavAction::Back);
                    window.set_needs_update();
                }
                consumed = true;
            }
            WindowEvent::MouseWheel { .. } | WindowEvent::MouseInput { .. }
                if !forward_mouse_event_to_egui(None) =>
            {
                window.retarget_input_to_host();
                self.gui.borrow_mut().reclaim_host_focus();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if matches!(
                key_code,
                KeyCode::KeyT
                    | KeyCode::KeyP
                    | KeyCode::KeyC
                    | KeyCode::Home
                    | KeyCode::Escape
                    | KeyCode::F2
                    | KeyCode::F6
                    | KeyCode::F9
            ) =>
            {
                // Graph control shortcuts always go to GUI, even when webview has focus
                if key_code == KeyCode::F2 {
                    self.gui.borrow_mut().request_command_palette_toggle();
                    self.winit_window.request_redraw();
                    consumed = true;
                } else {
                    let response = self
                        .gui
                        .borrow_mut()
                        .on_window_event(&self.winit_window, &event);
                    if response.repaint {
                        self.winit_window.request_redraw();
                    }
                    consumed = response.consumed;
                }
            }
            ref event => {
                let response = self
                    .gui
                    .borrow_mut()
                    .on_window_event(&self.winit_window, event);

                if let WindowEvent::Resized(_) = event {
                    self.gui.borrow_mut().set_state(state.clone());
                    self.rebuild_user_interface(&state, &window);
                }

                if response.repaint && *event != WindowEvent::RedrawRequested {
                    self.winit_window.request_redraw();
                }

                consumed = response.consumed;
                if let WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    ..
                } = event
                    && !self.ui_or_dialog_capture_active()
                {
                    let cursor_point = input_routing::resolve_pointer_position(self, None);
                    if let Some(point) = cursor_point {
                        self.last_mouse_position.set(Some(point));
                        let clicked_webview = self
                            .gui
                            .borrow()
                            .webview_at_point(point)
                            .map(|(webview_id, _)| webview_id);
                        if let Some(webview_id) = clicked_webview {
                            // Update pane focus on click even when egui consumes the event
                            // (e.g. tab strip/workbench interactions).
                            let mut gui = self.gui.borrow_mut();
                            let focused_node_key = gui.node_key_for_webview_id(webview_id);
                            gui.set_focused_node_key(focused_node_key);
                            gui.set_embedded_content_focus_webview(Some(webview_id));
                            window.retarget_input_to_webview(webview_id);
                        } else if self.gui.borrow().graph_at_point(point) {
                            self.gui.borrow_mut().focus_graph_surface();
                        }
                    }
                }
                if !consumed
                    && let WindowEvent::KeyboardInput {
                        event: key_event, ..
                    } = event
                    && key_event.state == ElementState::Pressed
                    && matches!(key_event.physical_key, PhysicalKey::Code(KeyCode::Enter))
                    && self.gui.borrow().location_has_focus()
                {
                    self.gui.borrow_mut().request_location_submit();
                    self.winit_window.request_redraw();
                    consumed = true;
                }
                if let WindowEvent::KeyboardInput {
                    event: key_event, ..
                } = event
                    && key_event.state == ElementState::Pressed
                    && matches!(key_event.physical_key, PhysicalKey::Code(KeyCode::Tab))
                {
                    let gui = self.gui.borrow();
                    let egui_wants_keyboard_input = gui.egui_wants_keyboard_input();
                    let graph_surface_focused = gui.graph_surface_focused();
                    let tab_target_is_webview = gui.has_focused_node();
                    let selected_node_key = gui.primary_selected_node_key();
                    drop(gui);

                    if graph_surface_focused && !egui_wants_keyboard_input {
                        if selected_node_key.is_some() {
                            self.gui.borrow_mut().set_focused_node_key(selected_node_key);
                            self.winit_window.request_redraw();
                            consumed = true;
                        }
                    } else if tab_target_is_webview && !egui_wants_keyboard_input {
                        self.gui.borrow_mut().focus_graph_surface();
                        self.winit_window.request_redraw();
                        consumed = true;
                    }
                }
            }
        }

        if !consumed {
            // Make sure to handle early resize events even when there are no webviews yet
            if let WindowEvent::Resized(new_inner_size) = event {
                if self.inner_size.get() != new_inner_size {
                    self.inner_size.set(new_inner_size);
                    // This should always be set to inner size
                    // because we are resizing `SurfmanRenderingContext`.
                    // See https://github.com/servo/servo/issues/38369#issuecomment-3138378527
                    self.window_rendering_context.resize(new_inner_size);
                }
            }

            match event {
                WindowEvent::KeyboardInput { event, .. } => {
                    if !self.ui_or_dialog_capture_active() {
                        if let Some(webview_id) =
                            input_routing::resolved_input_webview_id(self, &window)
                        {
                            self.gui
                                .borrow_mut()
                                .set_embedded_content_focus_webview(Some(webview_id));
                            window.retarget_input_to_webview(webview_id);
                        }
                        input_routing::handle_keyboard_input(self, state.clone(), &window, event)
                    }
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    self.modifiers_state.set(modifiers.state())
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if !self.ui_or_dialog_capture_active()
                        && !self.gui.borrow().egui_wants_pointer_input()
                    {
                        let pointer_position = input_routing::resolve_pointer_position(self, None);
                        if let Some(point) = pointer_position {
                            self.last_mouse_position.set(Some(point));
                        }
                        let pointer_target = pointer_position
                            .and_then(|point| self.gui.borrow().webview_at_point(point));
                        if input_routing::should_retarget_webview_focus(state) {
                            if let Some(webview_id) = pointer_target.map(|(id, _)| id) {
                                let mut gui = self.gui.borrow_mut();
                                let focused_node_key = gui.node_key_for_webview_id(webview_id);
                                gui.set_focused_node_key(focused_node_key);
                                gui.set_embedded_content_focus_webview(Some(webview_id));
                                window.retarget_input_to_webview(webview_id);
                            } else if let Some(point) = pointer_position
                                && self.gui.borrow().graph_at_point(point)
                            {
                                self.gui.borrow_mut().focus_graph_surface();
                            }
                        }
                        if let Some((webview_id, local_point)) = pointer_target
                            && let Some(webview) = window.webview_by_id(webview_id)
                        {
                            input_routing::set_webview_relative_mouse_point(self, local_point);
                            input_routing::handle_mouse_button_event(self, &webview, button, state);
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let point = winit_position_to_euclid_point(position).to_f32()
                        / self.hidpi_scale_factor();
                    // Keep hit-test position fresh even when egui owns pointer this frame.
                    self.last_mouse_position.set(Some(point));
                    if !self.ui_or_dialog_capture_active()
                        && !self.gui.borrow().egui_wants_pointer_input()
                    {
                        let pointer_target = self.gui.borrow().webview_at_point(point);
                        if let Some((webview_id, local_point)) = pointer_target
                            && let Some(webview) = window.webview_by_id(webview_id)
                        {
                            if self.gui.borrow().clip_inspector_target_webview_id()
                                == Some(renderer_id_from_servo(webview_id))
                            {
                                self.request_clip_inspector_stack_at_pointer(
                                    &window,
                                    webview_id,
                                    local_point,
                                );
                            }
                            input_routing::set_webview_relative_mouse_point(self, local_point);
                            input_routing::handle_mouse_move_event_with_webview_relative_point(
                                self,
                                &webview,
                                self.webview_relative_mouse_point.get(),
                            );
                        }
                    }
                }
                WindowEvent::CursorLeft { .. } => {
                    if !self.ui_or_dialog_capture_active()
                        && !self.gui.borrow().egui_wants_pointer_input()
                    {
                        let pointer_target = input_routing::resolve_pointer_position(self, None)
                            .and_then(|point| self.gui.borrow().webview_at_point(point));
                        if let Some((webview_id, local_point)) = pointer_target
                            && let Some(webview) = window.webview_by_id(webview_id)
                        {
                            input_routing::set_webview_relative_mouse_point(self, local_point);
                            let webview_rect: Rect<_, _> = webview.size().into();
                            if webview_rect.contains(self.webview_relative_mouse_point.get()) {
                                webview.notify_input_event(InputEvent::MouseLeftViewport(
                                    MouseLeftViewportEvent::default(),
                                ));
                            }
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    if !self.ui_or_dialog_capture_active()
                        && !self.gui.borrow().egui_wants_pointer_input()
                    {
                        let pointer_target = input_routing::resolve_pointer_position(self, None)
                            .and_then(|point| self.gui.borrow().webview_at_point(point));
                        if let Some((webview_id, local_point)) = pointer_target
                            && let Some(webview) = window.webview_by_id(webview_id)
                        {
                            input_routing::set_webview_relative_mouse_point(self, local_point);
                            let (delta_x, delta_y, mode) = match delta {
                                MouseScrollDelta::LineDelta(delta_x, delta_y) => (
                                    (delta_x * LINE_WIDTH) as f64,
                                    (delta_y * LINE_HEIGHT) as f64,
                                    WheelMode::DeltaPixel,
                                ),
                                MouseScrollDelta::PixelDelta(delta) => {
                                    (delta.x, delta.y, WheelMode::DeltaPixel)
                                }
                            };

                            let delta = WheelDelta {
                                x: delta_x,
                                y: delta_y,
                                z: 0.0,
                                mode,
                            };
                            let point = self.webview_relative_mouse_point.get();
                            webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                                delta,
                                point.into(),
                            )));
                        }
                    }
                }
                WindowEvent::Touch(touch) => {
                    if !self.ui_or_dialog_capture_active() {
                        if let Some(webview_id) =
                            input_routing::resolved_input_webview_id(self, &window)
                            && let Some(webview) = window.webview_by_id(webview_id)
                        {
                            self.gui
                                .borrow_mut()
                                .set_embedded_content_focus_webview(Some(webview_id));
                            window.retarget_input_to_webview(webview_id);
                            webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
                                input_routing::winit_phase_to_touch_event_type(touch.phase),
                                TouchId(touch.id as i32),
                                DevicePoint::new(touch.location.x as f32, touch.location.y as f32)
                                    .into(),
                            )));
                        }
                    }
                }
                WindowEvent::PinchGesture { delta, .. } => {
                    if !self.ui_or_dialog_capture_active() {
                        let pointer_target = input_routing::resolve_pointer_position(self, None)
                            .and_then(|point| self.gui.borrow().webview_at_point(point));
                        if let Some((webview_id, local_point)) = pointer_target
                            && let Some(webview) = window.webview_by_id(webview_id)
                        {
                            input_routing::set_webview_relative_mouse_point(self, local_point);
                            webview.adjust_pinch_zoom(
                                delta as f32 + 1.0,
                                self.webview_relative_mouse_point.get(),
                            );
                        }
                    }
                }
                WindowEvent::CloseRequested => {
                    window.schedule_close();
                }
                WindowEvent::ThemeChanged(theme) => {
                    crate::shell::desktop::runtime::registries::phase3_apply_system_theme_preference(
                        matches!(theme, winit::window::Theme::Dark),
                    );
                    if let Some(webview) = input_routing::explicit_input_webview(self, &window) {
                        webview.notify_theme_change(match theme {
                            winit::window::Theme::Light => Theme::Light,
                            winit::window::Theme::Dark => Theme::Dark,
                        });
                    }
                }
                WindowEvent::Ime(ime) => {
                    if let Some(webview) = input_routing::explicit_input_webview(self, &window) {
                        match ime {
                            Ime::Enabled => {
                                webview.notify_input_event(InputEvent::Ime(ImeEvent::Composition(
                                    servo::CompositionEvent {
                                        state: servo::CompositionState::Start,
                                        data: String::new(),
                                    },
                                )));
                            }
                            Ime::Preedit(text, _) => {
                                webview.notify_input_event(InputEvent::Ime(ImeEvent::Composition(
                                    servo::CompositionEvent {
                                        state: servo::CompositionState::Update,
                                        data: text,
                                    },
                                )));
                            }
                            Ime::Commit(text) => {
                                webview.notify_input_event(InputEvent::Ime(ImeEvent::Composition(
                                    servo::CompositionEvent {
                                        state: servo::CompositionState::End,
                                        data: text,
                                    },
                                )));
                            }
                            Ime::Disabled => {
                                webview.notify_input_event(InputEvent::Ime(ImeEvent::Dismissed));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn handle_winit_app_event(&self, window: &EmbedderWindow, app_event: AppEvent) {
        match app_event {
            AppEvent::Accessibility(ref event) => {
                // Deferred (Stage 4f audit, 2026-03-07): accesskit_winit::WindowEvent forwarding
                // into Servo remains gated on upstream integration shape; keep handled at GUI layer
                // until Servo-side routing contract is finalized (tracking: #41930).

                if self
                    .gui
                    .borrow_mut()
                    .handle_accesskit_event(&event.window_event)
                {
                    self.winit_window.request_redraw();
                }
            }
            AppEvent::ClipExtractionCompleted { result, .. } => {
                self.gui.borrow_mut().handle_clip_extraction_result(result);
                window.set_needs_update();
                self.winit_window.request_redraw();
            }
            AppEvent::ClipBatchExtractionCompleted { result, .. } => {
                self.gui
                    .borrow_mut()
                    .handle_clip_batch_extraction_result(result);
                window.set_needs_update();
                self.winit_window.request_redraw();
            }
            AppEvent::ClipInspectorPointerUpdated {
                webview_id, result, ..
            } => {
                self.clip_inspector_pointer_request_in_flight.set(false);
                self.gui
                    .borrow_mut()
                    .handle_clip_inspector_pointer_result(webview_id, result);
                window.set_needs_update();
                self.winit_window.request_redraw();
            }
            AppEvent::Waker => {}
        }
    }

    pub(crate) fn request_clip_element(
        &self,
        window: &EmbedderWindow,
        webview_id: WebViewId,
        element_rect: DeviceIntRect,
    ) {
        let Some(webview) = window.webview_by_id(webview_id) else {
            warn!(
                "Clip request ignored because webview {:?} no longer exists",
                webview_id
            );
            return;
        };

        let proxy = self.event_loop_proxy.clone();
        let window_id = self.winit_window.id();
        webview.evaluate_javascript(
            clip_extraction::build_clip_extraction_script(element_rect),
            move |result| {
                let result = clip_extraction::parse_clip_capture_result(webview_id, result);
                if let Err(error) =
                    proxy.send_event(AppEvent::ClipExtractionCompleted { window_id, result })
                {
                    warn!("Failed to deliver clip extraction result to event loop: {error}");
                }
            },
        );
    }

    pub(crate) fn request_page_inspector_candidates(
        &self,
        window: &EmbedderWindow,
        webview_id: WebViewId,
    ) {
        let Some(webview) = window.webview_by_id(webview_id) else {
            warn!(
                "Page inspector request ignored because webview {:?} no longer exists",
                webview_id
            );
            return;
        };

        let proxy = self.event_loop_proxy.clone();
        let window_id = self.winit_window.id();
        webview.evaluate_javascript(
            clip_extraction::build_page_inspector_extraction_script(),
            move |result| {
                let result = clip_extraction::parse_clip_capture_batch_result(webview_id, result);
                if let Err(error) =
                    proxy.send_event(AppEvent::ClipBatchExtractionCompleted { window_id, result })
                {
                    warn!(
                        "Failed to deliver page inspector extraction result to event loop: {error}"
                    );
                }
            },
        );
    }

    pub(crate) fn request_clip_inspector_stack_at_pointer(
        &self,
        window: &EmbedderWindow,
        webview_id: WebViewId,
        local_point: Point2D<f32, DeviceIndependentPixel>,
    ) {
        if self.clip_inspector_pointer_request_in_flight.get() {
            return;
        }
        let Some(webview) = window.webview_by_id(webview_id) else {
            return;
        };
        self.clip_inspector_pointer_request_in_flight.set(true);
        let proxy = self.event_loop_proxy.clone();
        let window_id = self.winit_window.id();
        webview.evaluate_javascript(
            clip_extraction::build_clip_inspector_stack_script(local_point),
            move |result| {
                let result = clip_extraction::parse_clip_capture_batch_result(webview_id, result);
                let send_result = proxy.send_event(AppEvent::ClipInspectorPointerUpdated {
                    window_id,
                    webview_id,
                    result,
                });
                if let Err(error) = send_result {
                    warn!("Failed to deliver clip inspector pointer result to event loop: {error}");
                }
            },
        );
    }

    pub(crate) fn sync_clip_inspector_highlight(
        &self,
        window: &EmbedderWindow,
        webview_id: crate::app::RendererId,
        dom_path: Option<&str>,
    ) {
        let Some(webview_id) = servo_webview_id_from_renderer(webview_id) else {
            return;
        };
        let Some(webview) = window.webview_by_id(webview_id) else {
            return;
        };
        webview.evaluate_javascript(
            clip_extraction::build_clip_inspector_highlight_script(dom_path),
            |_| {},
        );
    }
}

fn emit_navigation_transition_host_dialog_capture() {
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
}

impl PlatformWindowRendering for HeadedWindow {
    fn id(&self) -> EmbedderWindowId {
        let id: u64 = self.winit_window.id().into();
        id.into()
    }

    #[cfg(feature = "wry")]
    fn raw_window_handle_for_child(&self) -> Option<RawWindowHandle> {
        self.winit_window.window_handle().ok().map(|h| h.as_raw())
    }

    fn screen_geometry(&self) -> ScreenGeometry {
        let hidpi_factor = self.hidpi_scale_factor();
        let toolbar_size = Size2D::new(0.0, (self.toolbar_height() * self.hidpi_scale_factor()).0);
        let screen_size = self.screen_size.to_f32() * hidpi_factor;

        // Deferred (Stage 4f audit, 2026-03-07): this currently subtracts toolbar-only space.
        // Accurate subtraction of OS-reserved UI areas depends on winit capabilities tracked in
        // https://github.com/rust-windowing/winit/issues/2494.
        let available_screen_size = screen_size - toolbar_size;

        let window_rect = DeviceIntRect::from_origin_and_size(
            winit_position_to_euclid_point(self.winit_window.outer_position().unwrap_or_default()),
            winit_size_to_euclid_size(self.winit_window.outer_size()).to_i32(),
        );

        ScreenGeometry {
            size: screen_size.to_i32(),
            available_size: available_screen_size.to_i32(),
            window_rect,
        }
    }

    fn device_hidpi_scale_factor(&self) -> Scale<f32, DeviceIndependentPixel, DevicePixel> {
        Scale::new(self.winit_window.scale_factor() as f32)
    }

    fn hidpi_scale_factor(&self) -> Scale<f32, DeviceIndependentPixel, DevicePixel> {
        self.device_pixel_ratio_override
            .map(Scale::new)
            .unwrap_or_else(|| self.device_hidpi_scale_factor())
    }

    fn rendering_context(&self) -> Rc<dyn RenderingContextCore> {
        self.rendering_context.clone()
    }

    fn theme(&self) -> servo::Theme {
        match self.winit_window.theme() {
            Some(winit::window::Theme::Dark) => servo::Theme::Dark,
            Some(winit::window::Theme::Light) | None => servo::Theme::Light,
        }
    }

    fn window_rect(&self) -> DeviceIndependentIntRect {
        let outer_size = self.winit_window.outer_size();
        let scale = self.hidpi_scale_factor();

        let outer_size = winit_size_to_euclid_size(outer_size).to_i32();

        let origin = self
            .winit_window
            .outer_position()
            .map(winit_position_to_euclid_point)
            .unwrap_or_default();
        convert_rect_to_css_pixel(
            DeviceIntRect::from_origin_and_size(origin, outer_size),
            scale,
        )
    }

    fn rebuild_user_interface(&self, state: &RunningAppState, window: &EmbedderWindow) {
        self.gui.borrow_mut().update(state, window, self);
    }

    fn update_user_interface_state(&self, _: &RunningAppState, window: &EmbedderWindow) -> bool {
        let title = input_routing::explicit_chrome_webview(self, window)
            .and_then(|webview| {
                webview
                    .page_title()
                    .filter(|title| !title.is_empty())
                    .map(|title| title.to_string())
                    .or_else(|| webview.url().map(|url| url.to_string()))
            })
            .unwrap_or_else(|| INITIAL_WINDOW_TITLE.to_string());
        if title != *self.last_title.borrow() {
            self.winit_window.set_title(&title);
            *self.last_title.borrow_mut() = title;
        }

        self.gui.borrow_mut().update_webview_data(window)
    }

    fn request_repaint(&self, _window: &EmbedderWindow) {
        self.winit_window.request_redraw();
    }

    fn request_resize(&self, _: &WebView, new_outer_size: DeviceIntSize) -> Option<DeviceIntSize> {
        // Allocate space for the window deocrations, but do not let the inner size get
        // smaller than `MIN_WINDOW_INNER_SIZE` or larger than twice the screen size.
        let inner_size = self.winit_window.inner_size();
        let outer_size = self.winit_window.outer_size();
        let decoration_size: DeviceIntSize = Size2D::new(
            outer_size.height - inner_size.height,
            outer_size.width - inner_size.width,
        )
        .cast();

        let screen_size = (self.screen_size.to_f32() * self.hidpi_scale_factor()).to_i32();
        let new_outer_size =
            new_outer_size.clamp(MIN_WINDOW_INNER_SIZE + decoration_size, screen_size * 2);

        if outer_size.width == new_outer_size.width as u32
            && outer_size.height == new_outer_size.height as u32
        {
            return Some(new_outer_size);
        }

        let new_inner_size = new_outer_size - decoration_size;
        self.winit_window
            .request_inner_size(PhysicalSize::new(
                new_inner_size.width,
                new_inner_size.height,
            ))
            .map(|resulting_size| {
                DeviceIntSize::new(
                    resulting_size.width as i32 + decoration_size.width,
                    resulting_size.height as i32 + decoration_size.height,
                )
            })
    }

    #[cfg(feature = "webxr")]
    fn new_glwindow(&self, event_loop: &ActiveEventLoop) -> Rc<dyn servo::webxr::GlWindow> {
        xr::new_glwindow(self, event_loop)
    }
}

impl PlatformWindowOps for HeadedWindow {
    fn focus(&self) {
        self.winit_window.focus_window();
    }

    fn has_platform_focus(&self) -> bool {
        self.winit_window.has_focus()
    }

    fn get_fullscreen(&self) -> bool {
        self.fullscreen.get()
    }

    fn set_fullscreen(&self, state: bool) {
        if self.fullscreen.get() != state {
            self.winit_window.set_fullscreen(if state {
                Some(winit::window::Fullscreen::Borderless(Some(
                    self.monitor.clone(),
                )))
            } else {
                None
            });
        }
        self.fullscreen.set(state);
    }

    fn set_position(&self, point: DeviceIntPoint) {
        self.winit_window
            .set_outer_position::<PhysicalPosition<i32>>(PhysicalPosition::new(point.x, point.y))
    }

    fn set_cursor(&self, cursor: Cursor) {
        self.last_servo_cursor.set(cursor);
        if self.dialog_cursor_override_active.get() {
            input_routing::apply_platform_cursor(self, Cursor::Default);
            return;
        }
        input_routing::apply_platform_cursor(self, cursor);
    }

    fn maximize(&self, _webview: &WebView) {
        self.winit_window.set_maximized(true);
    }
}

impl PlatformWindowDialogs for HeadedWindow {
    fn show_embedder_control(&self, webview_id: WebViewId, embedder_control: EmbedderControl) {
        embedder_controls::show_embedder_control(self, webview_id, embedder_control);
    }

    fn hide_embedder_control(&self, webview_id: WebViewId, embedder_control_id: EmbedderControlId) {
        embedder_controls::hide_embedder_control(self, webview_id, embedder_control_id);
    }

    fn show_bluetooth_device_dialog(
        &self,
        webview_id: WebViewId,
        request: BluetoothDeviceSelectionRequest,
    ) {
        embedder_controls::show_bluetooth_device_dialog(self, webview_id, request);
    }

    fn show_permission_dialog(&self, webview_id: WebViewId, permission_request: PermissionRequest) {
        embedder_controls::show_permission_dialog(self, webview_id, permission_request);
    }

    fn show_http_authentication_dialog(
        &self,
        webview_id: WebViewId,
        authentication_request: AuthenticationRequest,
    ) {
        embedder_controls::show_http_authentication_dialog(
            self,
            webview_id,
            authentication_request,
        );
    }

    fn dismiss_embedder_controls_for_webview(&self, webview_id: WebViewId) {
        embedder_controls::dismiss_embedder_controls_for_webview(self, webview_id);
    }
}

impl PlatformWindowSignals for HeadedWindow {
    /// Handle Graphshell key bindings that may have been prevented by the page in the active webview.
    fn notify_input_event_handled(
        &self,
        webview: &WebView,
        id: InputEventId,
        result: InputEventResult,
    ) {
        let Some(keyboard_event) = self.pending_keyboard_events.borrow_mut().remove(&id) else {
            return;
        };
        if result.intersects(InputEventResult::DefaultPrevented | InputEventResult::Consumed) {
            return;
        }

        ShortcutMatcher::from_event(keyboard_event.event)
            .shortcut(CMD_OR_CONTROL, '=', || {
                self.gui
                    .borrow_mut()
                    .set_embedded_content_focus_webview(Some(webview.id()));
                self.gui
                    .borrow_mut()
                    .request_toolbar_nav_action_for_webview(webview.id(), ToolbarNavAction::ZoomIn);
            })
            .shortcut(CMD_OR_CONTROL, '+', || {
                self.gui
                    .borrow_mut()
                    .set_embedded_content_focus_webview(Some(webview.id()));
                self.gui
                    .borrow_mut()
                    .request_toolbar_nav_action_for_webview(webview.id(), ToolbarNavAction::ZoomIn);
            })
            .shortcut(CMD_OR_CONTROL, '-', || {
                self.gui
                    .borrow_mut()
                    .set_embedded_content_focus_webview(Some(webview.id()));
                self.gui
                    .borrow_mut()
                    .request_toolbar_nav_action_for_webview(
                        webview.id(),
                        ToolbarNavAction::ZoomOut,
                    );
            })
            .shortcut(CMD_OR_CONTROL, '0', || {
                self.gui
                    .borrow_mut()
                    .set_embedded_content_focus_webview(Some(webview.id()));
                self.gui
                    .borrow_mut()
                    .request_toolbar_nav_action_for_webview(
                        webview.id(),
                        ToolbarNavAction::ZoomReset,
                    );
            });
    }

    fn show_console_message(&self, level: servo::ConsoleLogLevel, message: &str) {
        println!("{message}");
        log::log!(level.into(), "{message}");
    }

    fn notify_accessibility_tree_update(
        &self,
        webview: WebView,
        tree_update: servo::accesskit::TreeUpdate,
    ) {
        self.gui
            .borrow_mut()
            .notify_accessibility_tree_update(webview.id(), tree_update);
    }
}

impl PlatformWindow for HeadedWindow {
    fn as_headed_window(&self) -> Option<&Self> {
        Some(self)
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn load_icon(icon_bytes: &[u8]) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        use image::{GenericImageView, Pixel};
        let image = image::load_from_memory(icon_bytes).expect("Failed to load icon");
        let (width, height) = image.dimensions();
        let mut rgba = Vec::with_capacity((width * height) as usize * 4);
        for (_, _, pixel) in image.pixels() {
            rgba.extend_from_slice(&pixel.to_rgba().0);
        }
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to load icon")
}

#[derive(Default)]
pub struct TouchEventSimulator {
    pub left_mouse_button_down: Cell<bool>,
}

impl TouchEventSimulator {
    fn maybe_consume_move_button_event(
        &self,
        webview: &WebView,
        button: MouseButton,
        action: ElementState,
        point: DevicePoint,
    ) -> bool {
        if button != MouseButton::Left {
            return false;
        }

        if action == ElementState::Pressed && !self.left_mouse_button_down.get() {
            webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
                TouchEventType::Down,
                TouchId(0),
                point.into(),
            )));
            self.left_mouse_button_down.set(true);
        } else if action == ElementState::Released {
            webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
                TouchEventType::Up,
                TouchId(0),
                point.into(),
            )));
            self.left_mouse_button_down.set(false);
        }

        true
    }

    fn maybe_consume_mouse_move_event(
        &self,
        webview: &WebView,
        point: Point2D<f32, DevicePixel>,
    ) -> bool {
        if !self.left_mouse_button_down.get() {
            return false;
        }

        webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
            TouchEventType::Move,
            TouchId(0),
            point.into(),
        )));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_retarget_webview_focus_only_on_press() {
        assert!(input_routing::should_retarget_webview_focus(
            ElementState::Pressed
        ));
        assert!(!input_routing::should_retarget_webview_focus(
            ElementState::Released
        ));
    }

    #[test]
    fn test_graph_control_shortcut_includes_focus_and_camera_lock_keys() {
        assert!(input_routing::is_graph_control_shortcut(KeyCode::F6));
        assert!(input_routing::is_graph_control_shortcut(KeyCode::F9));
    }

    #[test]
    fn test_graph_control_shortcut_excludes_regular_text_entry_keys() {
        assert!(!input_routing::is_graph_control_shortcut(KeyCode::Enter));
        assert!(!input_routing::is_graph_control_shortcut(KeyCode::KeyQ));
    }
}
