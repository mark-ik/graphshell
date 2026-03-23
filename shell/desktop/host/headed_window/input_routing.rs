/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Platform input routing helpers: pointer resolution, keyboard dispatch,
//! mouse/touch delivery, and shortcut interception.

use std::env;
use std::rc::Rc;
use std::time::Duration;

use euclid::{Point2D, Rect};
use keyboard_types::ShortcutMatcher;
use servo::{
    Cursor, DeviceIndependentPixel, DevicePixel, EditingActionEvent, InputEvent, KeyboardEvent,
    Modifiers, MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent,
    MouseLeftViewportEvent, MouseMoveEvent, NamedKey, TouchEventType, WebRenderDebugOption,
    WebView, WebViewId,
};
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseButton, TouchPhase};
use winit::keyboard::KeyCode;
use winit::window::CursorIcon;

use crate::app::{BrowserCommand, BrowserCommandTarget, OpenSurfaceSource};
use crate::shell::desktop::host::geometry::winit_position_to_euclid_point;
use crate::shell::desktop::host::keyutils::{
    CMD_OR_ALT, CMD_OR_CONTROL, keyboard_event_from_winit,
};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::{
    EmbedderWindow, PlatformWindowOps, PlatformWindowRendering,
};

use super::HeadedWindow;

// ── Input target resolution ──────────────────────────────────────────────────

pub(super) fn explicit_input_webview(
    headed: &HeadedWindow,
    window: &EmbedderWindow,
) -> Option<WebView> {
    resolved_input_webview_id(headed, window).and_then(|id| window.webview_by_id(id))
}

pub(super) fn resolved_input_webview_id(
    headed: &HeadedWindow,
    window: &EmbedderWindow,
) -> Option<WebViewId> {
    window.resolve_input_webview_id(headed.gui.borrow().focused_embedded_content_webview_id())
}

pub(super) fn explicit_chrome_webview(
    _headed: &HeadedWindow,
    window: &EmbedderWindow,
) -> Option<WebView> {
    window
        .explicit_chrome_webview_id()
        .and_then(|id| window.webview_by_id(id))
}

pub(super) fn should_retarget_webview_focus(state: ElementState) -> bool {
    state == ElementState::Pressed
}

pub(super) fn is_graph_control_shortcut(key_code: KeyCode) -> bool {
    matches!(
        key_code,
        KeyCode::KeyT
            | KeyCode::KeyP
            | KeyCode::KeyC
            | KeyCode::Home
            | KeyCode::Escape
            | KeyCode::F2
            | KeyCode::F6
            | KeyCode::F9
    )
}

pub(super) fn resolve_pointer_position(
    headed: &HeadedWindow,
    event_position: Option<PhysicalPosition<f64>>,
) -> Option<Point2D<f32, DeviceIndependentPixel>> {
    event_position
        .map(|position| {
            winit_position_to_euclid_point(position).to_f32() / headed.hidpi_scale_factor()
        })
        .or_else(|| headed.gui.borrow().pointer_hover_position())
        .or(headed.last_mouse_position.get())
}

// ── Pointer state ────────────────────────────────────────────────────────────

pub(super) fn set_webview_relative_mouse_point(
    headed: &HeadedWindow,
    point: Point2D<f32, DeviceIndependentPixel>,
) {
    let scale = headed.hidpi_scale_factor().get();
    headed
        .webview_relative_mouse_point
        .set(Point2D::new(point.x * scale, point.y * scale));
}

// ── Mouse event delivery ─────────────────────────────────────────────────────

pub(super) fn handle_mouse_button_event(
    headed: &HeadedWindow,
    webview: &WebView,
    button: MouseButton,
    action: ElementState,
) {
    let point = headed.webview_relative_mouse_point.get();
    let webview_rect: Rect<_, _> = webview.size().into();
    if !webview_rect.contains(point) {
        return;
    }

    if headed
        .touch_event_simulator
        .as_ref()
        .is_some_and(|sim| sim.maybe_consume_move_button_event(webview, button, action, point))
    {
        return;
    }

    let mouse_button = match &button {
        MouseButton::Left => ServoMouseButton::Left,
        MouseButton::Right => ServoMouseButton::Right,
        MouseButton::Middle => ServoMouseButton::Middle,
        MouseButton::Back => ServoMouseButton::Back,
        MouseButton::Forward => ServoMouseButton::Forward,
        MouseButton::Other(value) => ServoMouseButton::Other(*value),
    };

    let action = match action {
        ElementState::Pressed => MouseButtonAction::Down,
        ElementState::Released => MouseButtonAction::Up,
    };

    webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
        action,
        mouse_button,
        point.into(),
    )));
}

pub(super) fn handle_mouse_move_event_with_webview_relative_point(
    headed: &HeadedWindow,
    webview: &WebView,
    point: Point2D<f32, DevicePixel>,
) {
    let previous_point = headed.webview_relative_mouse_point.get();
    headed.webview_relative_mouse_point.set(point);

    let webview_rect: Rect<_, _> = webview.size().into();
    if !webview_rect.contains(point) {
        if webview_rect.contains(previous_point) {
            webview.notify_input_event(InputEvent::MouseLeftViewport(
                MouseLeftViewportEvent::default(),
            ));
        }
        return;
    }

    if headed
        .touch_event_simulator
        .as_ref()
        .is_some_and(|sim| sim.maybe_consume_mouse_move_event(webview, point))
    {
        return;
    }

    webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(point.into())));
}

// ── Keyboard delivery ────────────────────────────────────────────────────────

pub(super) fn handle_keyboard_input(
    headed: &HeadedWindow,
    state: Rc<RunningAppState>,
    window: &EmbedderWindow,
    winit_event: KeyEvent,
) {
    let keyboard_event = keyboard_event_from_winit(&winit_event, headed.modifiers_state.get());
    if handle_intercepted_key_bindings(headed, state, window, &keyboard_event) {
        return;
    }

    let Some(webview) = explicit_input_webview(headed, window) else {
        return;
    };

    for xr_window_pose in headed.xr_window_poses.borrow().iter() {
        xr_window_pose.handle_xr_rotation(&winit_event, headed.modifiers_state.get());
        xr_window_pose.handle_xr_translation(&keyboard_event);
    }

    let id = webview.notify_input_event(InputEvent::Keyboard(keyboard_event.clone()));
    headed
        .pending_keyboard_events
        .borrow_mut()
        .insert(id, keyboard_event);
}

pub(super) fn handle_intercepted_key_bindings(
    headed: &HeadedWindow,
    state: Rc<RunningAppState>,
    window: &EmbedderWindow,
    key_event: &KeyboardEvent,
) -> bool {
    let Some(active_webview) = explicit_input_webview(headed, window) else {
        return false;
    };

    let mut handled = true;
    ShortcutMatcher::from_event(key_event.event.clone())
        .shortcut(CMD_OR_CONTROL, 'R', || {
            headed.gui.borrow_mut().request_browser_command(
                BrowserCommandTarget::FocusedInput,
                BrowserCommand::Reload,
            );
        })
        .shortcut(CMD_OR_CONTROL, 'W', || {
            headed
                .gui
                .borrow_mut()
                .request_browser_command(BrowserCommandTarget::FocusedInput, BrowserCommand::Close);
        })
        .shortcut(CMD_OR_CONTROL, 'P', || {
            let rate = env::var("SAMPLING_RATE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10);
            let duration = env::var("SAMPLING_DURATION")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10);
            active_webview.toggle_sampling_profiler(
                Duration::from_millis(rate),
                Duration::from_secs(duration),
            );
        })
        .shortcut(CMD_OR_CONTROL, 'X', || {
            active_webview.notify_input_event(InputEvent::EditingAction(EditingActionEvent::Cut));
        })
        .shortcut(CMD_OR_CONTROL, 'C', || {
            active_webview.notify_input_event(InputEvent::EditingAction(EditingActionEvent::Copy));
        })
        .shortcut(CMD_OR_CONTROL, 'V', || {
            active_webview.notify_input_event(InputEvent::EditingAction(EditingActionEvent::Paste));
        })
        .shortcut(Modifiers::CONTROL, servo::Key::Named(NamedKey::F9), || {
            active_webview.capture_webrender();
        })
        .shortcut(Modifiers::CONTROL, servo::Key::Named(NamedKey::F10), || {
            active_webview.toggle_webrender_debugging(WebRenderDebugOption::RenderTargetDebug);
        })
        .shortcut(Modifiers::CONTROL, servo::Key::Named(NamedKey::F11), || {
            active_webview.toggle_webrender_debugging(WebRenderDebugOption::TextureCacheDebug);
        })
        .shortcut(Modifiers::CONTROL, servo::Key::Named(NamedKey::F12), || {
            active_webview.toggle_webrender_debugging(WebRenderDebugOption::Profiler);
        })
        .shortcut(CMD_OR_ALT, servo::Key::Named(NamedKey::ArrowRight), || {
            headed.gui.borrow_mut().request_browser_command(
                BrowserCommandTarget::FocusedInput,
                BrowserCommand::Forward,
            );
        })
        .optional_shortcut(
            cfg!(not(target_os = "windows")),
            CMD_OR_CONTROL,
            ']',
            || {
                headed.gui.borrow_mut().request_browser_command(
                    BrowserCommandTarget::FocusedInput,
                    BrowserCommand::Forward,
                );
            },
        )
        .shortcut(CMD_OR_ALT, servo::Key::Named(NamedKey::ArrowLeft), || {
            headed
                .gui
                .borrow_mut()
                .request_browser_command(BrowserCommandTarget::FocusedInput, BrowserCommand::Back);
        })
        .optional_shortcut(
            cfg!(not(target_os = "windows")),
            CMD_OR_CONTROL,
            '[',
            || {
                headed.gui.borrow_mut().request_browser_command(
                    BrowserCommandTarget::FocusedInput,
                    BrowserCommand::Back,
                );
            },
        )
        .optional_shortcut(
            headed.get_fullscreen(),
            Modifiers::empty(),
            servo::Key::Named(NamedKey::Escape),
            || active_webview.exit_fullscreen(),
        )
        .shortcut(CMD_OR_CONTROL, 'T', || {
            window.notify_host_open_request(
                "servo:newtab".to_string(),
                OpenSurfaceSource::KeyboardShortcut,
                Some(active_webview.id()),
                None,
            );
        })
        .shortcut(CMD_OR_CONTROL, 'Q', || state.schedule_exit())
        .otherwise(|| handled = false);
    handled
}

// ── Cursor ───────────────────────────────────────────────────────────────────

pub(super) fn apply_platform_cursor(headed: &HeadedWindow, cursor: Cursor) {
    let winit_cursor = match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::ContextMenu => CursorIcon::ContextMenu,
        Cursor::Help => CursorIcon::Help,
        Cursor::Progress => CursorIcon::Progress,
        Cursor::Wait => CursorIcon::Wait,
        Cursor::Cell => CursorIcon::Cell,
        Cursor::Crosshair => CursorIcon::Crosshair,
        Cursor::Text => CursorIcon::Text,
        Cursor::VerticalText => CursorIcon::VerticalText,
        Cursor::Alias => CursorIcon::Alias,
        Cursor::Copy => CursorIcon::Copy,
        Cursor::Move => CursorIcon::Move,
        Cursor::NoDrop => CursorIcon::NoDrop,
        Cursor::NotAllowed => CursorIcon::NotAllowed,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::EResize => CursorIcon::EResize,
        Cursor::NResize => CursorIcon::NResize,
        Cursor::NeResize => CursorIcon::NeResize,
        Cursor::NwResize => CursorIcon::NwResize,
        Cursor::SResize => CursorIcon::SResize,
        Cursor::SeResize => CursorIcon::SeResize,
        Cursor::SwResize => CursorIcon::SwResize,
        Cursor::WResize => CursorIcon::WResize,
        Cursor::EwResize => CursorIcon::EwResize,
        Cursor::NsResize => CursorIcon::NsResize,
        Cursor::NeswResize => CursorIcon::NeswResize,
        Cursor::NwseResize => CursorIcon::NwseResize,
        Cursor::ColResize => CursorIcon::ColResize,
        Cursor::RowResize => CursorIcon::RowResize,
        Cursor::AllScroll => CursorIcon::AllScroll,
        Cursor::ZoomIn => CursorIcon::ZoomIn,
        Cursor::ZoomOut => CursorIcon::ZoomOut,
        Cursor::None => {
            headed.winit_window.set_cursor_visible(false);
            return;
        }
    };
    headed.winit_window.set_cursor(winit_cursor);
    headed.winit_window.set_cursor_visible(true);
}

// ── Touch phase conversion ───────────────────────────────────────────────────

pub(super) fn winit_phase_to_touch_event_type(phase: TouchPhase) -> TouchEventType {
    match phase {
        TouchPhase::Started => TouchEventType::Down,
        TouchPhase::Moved => TouchEventType::Move,
        TouchPhase::Ended => TouchEventType::Up,
        TouchPhase::Cancelled => TouchEventType::Cancel,
    }
}
