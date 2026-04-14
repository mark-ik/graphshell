/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::rc::Rc;

use euclid::{Angle, Rotation3D, Size2D, UnknownUnit, Vector3D};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{Key, KeyState, KeyboardEvent};
use winit::event::{ElementState, KeyEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as LogicalKey, ModifiersState, NamedKey as WinitNamedKey};

use super::HeadedWindow;

#[cfg(feature = "webxr")]
pub(super) fn new_glwindow(
    window: &HeadedWindow,
    event_loop: &ActiveEventLoop,
) -> Rc<dyn servo::webxr::GlWindow> {
    let size = window.winit_window.outer_size();

    let window_attr = winit::window::Window::default_attributes()
        .with_title("Graphshell XR".to_string())
        .with_inner_size(size)
        .with_visible(false);

    let winit_window = event_loop
        .create_window(window_attr)
        .expect("Failed to create window.");

    let pose = Rc::new(XRWindowPose {
        xr_rotation: Cell::new(Rotation3D::identity()),
        xr_translation: Cell::new(Vector3D::zero()),
    });
    window.xr_window_poses.borrow_mut().push(pose.clone());
    Rc::new(XRWindow { winit_window, pose })
}

pub(super) struct XRWindowPose {
    xr_rotation: Cell<Rotation3D<f32, UnknownUnit, UnknownUnit>>,
    xr_translation: Cell<Vector3D<f32, UnknownUnit>>,
}

impl XRWindowPose {
    pub(super) fn handle_xr_translation(&self, input: &KeyboardEvent) {
        if input.event.state != KeyState::Down {
            return;
        }
        const NORMAL_TRANSLATE: f32 = 0.1;
        const QUICK_TRANSLATE: f32 = 1.0;
        let mut x = 0.0;
        let mut z = 0.0;
        match input.event.key {
            Key::Character(ref k) => match &**k {
                "w" => z = -NORMAL_TRANSLATE,
                "W" => z = -QUICK_TRANSLATE,
                "s" => z = NORMAL_TRANSLATE,
                "S" => z = QUICK_TRANSLATE,
                "a" => x = -NORMAL_TRANSLATE,
                "A" => x = -QUICK_TRANSLATE,
                "d" => x = NORMAL_TRANSLATE,
                "D" => x = QUICK_TRANSLATE,
                _ => return,
            },
            _ => return,
        };
        let (old_x, old_y, old_z) = self.xr_translation.get().to_tuple();
        let vec = Vector3D::new(x + old_x, old_y, z + old_z);
        self.xr_translation.set(vec);
    }

    pub(super) fn handle_xr_rotation(&self, input: &KeyEvent, modifiers: ModifiersState) {
        if input.state != ElementState::Pressed {
            return;
        }
        let mut x = 0.0;
        let mut y = 0.0;
        match input.logical_key {
            LogicalKey::Named(WinitNamedKey::ArrowUp) => x = 1.0,
            LogicalKey::Named(WinitNamedKey::ArrowDown) => x = -1.0,
            LogicalKey::Named(WinitNamedKey::ArrowLeft) => y = 1.0,
            LogicalKey::Named(WinitNamedKey::ArrowRight) => y = -1.0,
            _ => return,
        };
        if modifiers.shift_key() {
            x *= 10.0;
            y *= 10.0;
        }
        let x: Rotation3D<_, UnknownUnit, UnknownUnit> = Rotation3D::around_x(Angle::degrees(x));
        let y: Rotation3D<_, UnknownUnit, UnknownUnit> = Rotation3D::around_y(Angle::degrees(y));
        let rotation = self.xr_rotation.get().then(&x).then(&y);
        self.xr_rotation.set(rotation);
    }
}

#[cfg(feature = "webxr")]
struct XRWindow {
    winit_window: winit::window::Window,
    pose: Rc<XRWindowPose>,
}

#[cfg(feature = "webxr")]
impl servo::webxr::GlWindow for XRWindow {
    fn get_render_target(
        &self,
        device: &mut surfman::Device,
        _context: &mut surfman::Context,
    ) -> servo::webxr::GlWindowRenderTarget {
        self.winit_window.set_visible(true);
        let window_handle = self
            .winit_window
            .window_handle()
            .expect("could not get window handle from window");
        let size = self.winit_window.inner_size();
        let size = Size2D::new(size.width as i32, size.height as i32);
        let native_widget = device
            .connection()
            .create_native_widget_from_window_handle(window_handle, size)
            .expect("Failed to create native widget");
        servo::webxr::GlWindowRenderTarget::NativeWidget(native_widget)
    }

    fn get_rotation(&self) -> Rotation3D<f32, UnknownUnit, UnknownUnit> {
        self.pose.xr_rotation.get()
    }

    fn get_translation(&self) -> Vector3D<f32, UnknownUnit> {
        self.pose.xr_translation.get()
    }

    fn get_mode(&self) -> servo::webxr::GlWindowMode {
        use servo::pref;
        if pref!(dom_webxr_glwindow_red_cyan) {
            servo::webxr::GlWindowMode::StereoRedCyan
        } else if pref!(dom_webxr_glwindow_left_right) {
            servo::webxr::GlWindowMode::StereoLeftRight
        } else if pref!(dom_webxr_glwindow_spherical) {
            servo::webxr::GlWindowMode::Spherical
        } else if pref!(dom_webxr_glwindow_cubemap) {
            servo::webxr::GlWindowMode::Cubemap
        } else {
            servo::webxr::GlWindowMode::Blit
        }
    }

    fn display_handle(&self) -> raw_window_handle::DisplayHandle<'_> {
        self.winit_window
            .display_handle()
            .expect("Every window should have a display handle")
    }
}

