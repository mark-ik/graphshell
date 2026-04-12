/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

mod gl_backend;
mod wgpu_backend;

use std::rc::Rc;
use std::sync::Arc;

use euclid::{Point2D, Rect, Size2D};
use servo::{OffscreenRenderingContext, WindowRenderingContext};
use winit::window::Window;

pub(crate) use gl_backend::{
    BackendFramebufferHandle, BackendGraphicsContext, BackendParentRenderCallback,
};
#[cfg(feature = "gl_compat")]
pub(crate) use gl_backend::{
    backend_active_texture, backend_bind_framebuffer, backend_chaos_alternate_texture_unit,
    backend_chaos_framebuffer_handle, backend_framebuffer_binding,
    backend_framebuffer_from_binding, backend_is_blend_enabled, backend_is_scissor_enabled,
    backend_primary_texture_unit, backend_scissor_box, backend_set_active_texture, backend_set_blend_enabled,
    backend_set_scissor_box, backend_set_scissor_enabled, backend_set_viewport, backend_viewport,
};
pub(crate) use wgpu_backend::{
    BackendCustomPass, UiRenderBackendContract, UiRenderBackendHandle, activate_ui_render_backend,
    begin_ui_render_backend_paint, create_ui_render_backend, custom_pass_from_backend_viewport,
    end_ui_render_backend_paint, register_custom_paint_callback, texture_id_from_token,
    texture_token_from_handle,
};

pub(crate) struct UiHostRenderBootstrap {
    rendering_context: Rc<OffscreenRenderingContext>,
    window_rendering_context: Rc<WindowRenderingContext>,
    wgpu: UiWgpuHostBootstrap,
}

impl UiHostRenderBootstrap {
    pub(crate) fn new(
        rendering_context: Rc<OffscreenRenderingContext>,
        window_rendering_context: Rc<WindowRenderingContext>,
    ) -> Self {
        Self {
            rendering_context,
            window_rendering_context,
            wgpu: UiWgpuHostBootstrap::default(),
        }
    }

    pub(crate) fn rendering_context(&self) -> &OffscreenRenderingContext {
        self.rendering_context.as_ref()
    }

    pub(crate) fn window_rendering_context(&self) -> &WindowRenderingContext {
        self.window_rendering_context.as_ref()
    }

    pub(crate) fn into_contexts(self) -> (Rc<OffscreenRenderingContext>, Rc<WindowRenderingContext>) {
        (self.rendering_context, self.window_rendering_context)
    }

    pub(crate) fn wgpu_bootstrap(&self) -> &UiWgpuHostBootstrap {
        &self.wgpu
    }
}

pub(crate) struct UiRenderBackendInit<'a> {
    pub(crate) window: &'a Window,
    pub(crate) render_host: &'a UiHostRenderBootstrap,
}

#[derive(Clone)]
pub(crate) struct UiWgpuHostBootstrap {
    pub(crate) configuration: egui_wgpu::WgpuConfiguration,
}

impl Default for UiWgpuHostBootstrap {
    fn default() -> Self {
        Self {
            configuration: egui_wgpu::WgpuConfiguration::default(),
        }
    }
}

const BACKEND_BRIDGE_MODE_ENV_VAR: &str = "GRAPHSHELL_BACKEND_BRIDGE_MODE";
const BACKEND_BRIDGE_READINESS_GATE_ENV_VAR: &str =
    "GRAPHSHELL_ENABLE_WGPU_BRIDGE_READINESS_GATE";
const SERVO_WGPU_BACKEND_ENV_VAR: &str = "SERVO_WGPU_BACKEND";
const BACKEND_BRIDGE_PATH_WGPU_SHARED: &str = "wgpu.shared_texture";
const BACKEND_BRIDGE_PATH_GL_CALLBACK: &str = "gl.render_to_parent_callback";
const BACKEND_BRIDGE_PATH_WGPU_PREFERRED_FALLBACK_GL: &str =
    "wgpu.preferred.fallback_gl.render_to_parent_callback";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BackendContentBridgeMode {
    /// Primary wgpu path: per-webview texture imported into egui/wgpu.
    WgpuShared,
    GlCallback,
    WgpuPreferredFallbackGlCallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackendContentBridgePolicy {
    GlBaseline,
    ExperimentalEnvRequestedMode,
}

#[derive(Clone)]
pub(crate) struct BackendContentBridgeSelection {
    pub(crate) mode: BackendContentBridgeMode,
    pub(crate) bridge: BackendContentBridge,
}

/// Import closure for the wgpu shared-texture content bridge path.
///
/// Captures an `OffscreenRenderingContext` by `Rc`; single-threaded use only.
/// When called with the shared wgpu device and queue, returns the composited
/// webview texture, or `None` if the import is not yet available.
pub(crate) type BackendSharedWgpuImport = std::rc::Rc<
    dyn Fn(servo::wgpu::Device, servo::wgpu::Queue) -> Option<servo::wgpu::Texture>,
>;

#[derive(Clone)]
pub(crate) enum BackendContentBridge {
    /// Primary wgpu path: import the per-webview texture via the shared wgpu device.
    SharedWgpuTexture(BackendSharedWgpuImport),
    /// Fallback GL callback path: composites via OpenGL parent render callback.
    ParentRenderCallback(BackendParentRenderCallback),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackendContentBridgeCapabilities {
    pub(crate) supports_wgpu_parent_render_bridge: bool,
    /// Whether the wgpu shared-texture import path is available.
    /// True when the servo wgpu backend is enabled.
    pub(crate) supports_wgpu_shared_texture: bool,
}

impl Default for BackendContentBridgeCapabilities {
    fn default() -> Self {
        Self {
            supports_wgpu_parent_render_bridge: false,
            supports_wgpu_shared_texture: false,
        }
    }
}

fn env_flag_enabled(key: &str) -> bool {
    std::env::var(key)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn servo_wgpu_backend_requested() -> bool {
    env_flag_enabled(SERVO_WGPU_BACKEND_ENV_VAR)
}

fn requested_backend_content_bridge_mode() -> BackendContentBridgeMode {
    std::env::var(BACKEND_BRIDGE_MODE_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| match value.as_str() {
            "wgpu_shared" => BackendContentBridgeMode::WgpuShared,
            "wgpu_preferred" => BackendContentBridgeMode::WgpuPreferredFallbackGlCallback,
            _ => BackendContentBridgeMode::GlCallback,
        })
        .unwrap_or(BackendContentBridgeMode::GlCallback)
}

fn active_backend_content_bridge_policy() -> BackendContentBridgePolicy {
    if env_flag_enabled(BACKEND_BRIDGE_READINESS_GATE_ENV_VAR) {
        BackendContentBridgePolicy::ExperimentalEnvRequestedMode
    } else {
        BackendContentBridgePolicy::GlBaseline
    }
}

fn content_bridge_capabilities_from_observed_context(
    has_parent_render_callback: bool,
    servo_wgpu_backend_enabled: bool,
) -> BackendContentBridgeCapabilities {
    BackendContentBridgeCapabilities {
        supports_wgpu_parent_render_bridge: has_parent_render_callback && servo_wgpu_backend_enabled,
        supports_wgpu_shared_texture: servo_wgpu_backend_enabled,
    }
}

fn resolve_backend_content_bridge_mode(
    policy: BackendContentBridgePolicy,
    capabilities: BackendContentBridgeCapabilities,
) -> BackendContentBridgeMode {
    match policy {
        BackendContentBridgePolicy::GlBaseline => BackendContentBridgeMode::GlCallback,
        BackendContentBridgePolicy::ExperimentalEnvRequestedMode => {
            let requested = requested_backend_content_bridge_mode();
            match requested {
                BackendContentBridgeMode::WgpuShared
                    if !capabilities.supports_wgpu_shared_texture =>
                {
                    BackendContentBridgeMode::GlCallback
                }
                BackendContentBridgeMode::WgpuPreferredFallbackGlCallback
                    if !capabilities.supports_wgpu_parent_render_bridge =>
                {
                    BackendContentBridgeMode::GlCallback
                }
                mode => mode,
            }
        }
    }
}

pub(crate) fn select_backend_content_bridge_with_capabilities(
    callback: BackendParentRenderCallback,
    capabilities: BackendContentBridgeCapabilities,
) -> BackendContentBridgeSelection {
    let mode =
        resolve_backend_content_bridge_mode(active_backend_content_bridge_policy(), capabilities);

    BackendContentBridgeSelection {
        mode,
        bridge: BackendContentBridge::ParentRenderCallback(callback),
    }
}

fn select_backend_content_bridge_with_policy_for_tests(
    callback: BackendParentRenderCallback,
    policy: BackendContentBridgePolicy,
    capabilities: BackendContentBridgeCapabilities,
) -> BackendContentBridgeSelection {
    let mode = resolve_backend_content_bridge_mode(policy, capabilities);

    BackendContentBridgeSelection {
        mode,
        bridge: BackendContentBridge::ParentRenderCallback(callback),
    }
}

pub(crate) fn select_backend_content_bridge(
    callback: BackendParentRenderCallback,
) -> BackendContentBridgeSelection {
    select_backend_content_bridge_with_capabilities(
        callback,
        BackendContentBridgeCapabilities::default(),
    )
}

fn content_bridge_capabilities_for_render_context(
    render_context: &OffscreenRenderingContext,
) -> BackendContentBridgeCapabilities {
    content_bridge_capabilities_from_observed_context(
        render_context.render_to_parent_callback().is_some(),
        servo_wgpu_backend_requested(),
    )
}

#[cfg(test)]
pub(crate) fn backend_bridge_test_env_lock() -> &'static std::sync::Mutex<()> {
    use std::sync::{Mutex, OnceLock};

    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
pub(crate) fn clear_backend_bridge_env_for_tests() {
    unsafe {
        std::env::remove_var(BACKEND_BRIDGE_MODE_ENV_VAR);
        std::env::remove_var(BACKEND_BRIDGE_READINESS_GATE_ENV_VAR);
        std::env::remove_var(SERVO_WGPU_BACKEND_ENV_VAR);
    }
}

#[cfg(test)]
pub(crate) fn set_backend_bridge_mode_env_for_tests(value: &str) {
    unsafe {
        std::env::set_var(BACKEND_BRIDGE_MODE_ENV_VAR, value);
    }
}

#[cfg(test)]
pub(crate) fn set_backend_bridge_readiness_gate_for_tests(enabled: bool) {
    unsafe {
        if enabled {
            std::env::set_var(BACKEND_BRIDGE_READINESS_GATE_ENV_VAR, "1");
        } else {
            std::env::remove_var(BACKEND_BRIDGE_READINESS_GATE_ENV_VAR);
        }
    }
}

pub(crate) fn select_content_bridge_from_render_context(
    render_context: &OffscreenRenderingContext,
) -> Option<BackendContentBridgeSelection> {
    let render_to_parent = render_context.render_to_parent_callback()?;

    let callback: BackendParentRenderCallback = Arc::new(move |gl, region| {
        let rect_in_parent = Rect::new(
            Point2D::new(region.left_px, region.from_bottom_px),
            Size2D::new(region.width_px, region.height_px),
        );
        render_to_parent(gl, rect_in_parent)
    });

    let capabilities = content_bridge_capabilities_for_render_context(render_context);
    Some(select_backend_content_bridge_with_capabilities(
        callback,
        capabilities,
    ))
}

/// Build a `SharedWgpuTexture` bridge by capturing an `OffscreenRenderingContext`.
///
/// This is the primary wgpu content bridge factory. The returned selection uses
/// `BackendContentBridgeMode::WgpuShared` and the import closure forwards to
/// `OffscreenRenderingContext::import_to_shared_wgpu_texture` at call time.
///
/// Use when `servo_wgpu_backend_requested()` is true and a render context
/// is available. The GL callback path (`select_content_bridge_from_render_context`)
/// remains the production default until the readiness gate is enabled.
pub(crate) fn select_content_bridge_wgpu_from_render_context(
    render_context: std::rc::Rc<OffscreenRenderingContext>,
) -> BackendContentBridgeSelection {
    let import: BackendSharedWgpuImport = std::rc::Rc::new(move |device, queue| {
        render_context.import_to_shared_wgpu_texture(device, queue)
    });
    BackendContentBridgeSelection {
        mode: BackendContentBridgeMode::WgpuShared,
        bridge: BackendContentBridge::SharedWgpuTexture(import),
    }
}

pub(crate) fn backend_content_bridge_path(mode: BackendContentBridgeMode) -> &'static str {
    match mode {
        BackendContentBridgeMode::WgpuShared => BACKEND_BRIDGE_PATH_WGPU_SHARED,
        BackendContentBridgeMode::GlCallback => BACKEND_BRIDGE_PATH_GL_CALLBACK,
        BackendContentBridgeMode::WgpuPreferredFallbackGlCallback => {
            BACKEND_BRIDGE_PATH_WGPU_PREFERRED_FALLBACK_GL
        }
    }
}

pub(crate) fn backend_content_bridge_mode_label(mode: BackendContentBridgeMode) -> &'static str {
    match mode {
        BackendContentBridgeMode::WgpuShared => "wgpu_shared",
        BackendContentBridgeMode::GlCallback => "gl_callback",
        BackendContentBridgeMode::WgpuPreferredFallbackGlCallback => {
            "wgpu_preferred_fallback_gl_callback"
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackendViewportInPixels {
    pub(crate) left_px: i32,
    pub(crate) from_bottom_px: i32,
    pub(crate) width_px: i32,
    pub(crate) height_px: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackendParentRenderRegionInPixels {
    pub(crate) left_px: i32,
    pub(crate) from_bottom_px: i32,
    pub(crate) width_px: i32,
    pub(crate) height_px: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct BackendTextureToken(pub(crate) egui::TextureId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_mode_defaults_to_gl_callback() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge(callback);

        assert_eq!(selected.mode, BackendContentBridgeMode::GlCallback);
    }

    #[test]
    fn bridge_mode_selects_wgpu_preferred_fallback_when_requested() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();
        set_backend_bridge_mode_env_for_tests("wgpu_preferred");

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge_with_policy_for_tests(
            callback,
            BackendContentBridgePolicy::ExperimentalEnvRequestedMode,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
                supports_wgpu_shared_texture: false,
            },
        );

        assert_eq!(
            selected.mode,
            BackendContentBridgeMode::WgpuPreferredFallbackGlCallback
        );

        clear_backend_bridge_env_for_tests();
    }

    #[test]
    fn active_policy_uses_gl_even_when_env_requests_wgpu_preferred() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();
        set_backend_bridge_mode_env_for_tests("wgpu_preferred");

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge_with_capabilities(
            callback,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
                supports_wgpu_shared_texture: false,
            },
        );

        assert_eq!(selected.mode, BackendContentBridgeMode::GlCallback);

        clear_backend_bridge_env_for_tests();
    }

    #[test]
    fn bridge_mode_falls_back_to_gl_when_wgpu_capability_is_unavailable() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();
        set_backend_bridge_mode_env_for_tests("wgpu_preferred");

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge_with_policy_for_tests(
            callback,
            BackendContentBridgePolicy::ExperimentalEnvRequestedMode,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: false,
                supports_wgpu_shared_texture: false,
            },
        );

        assert_eq!(selected.mode, BackendContentBridgeMode::GlCallback);

        clear_backend_bridge_env_for_tests();
    }

    #[test]
    fn active_policy_honors_requested_mode_when_readiness_gate_is_enabled() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();
        set_backend_bridge_mode_env_for_tests("wgpu_preferred");
        set_backend_bridge_readiness_gate_for_tests(true);

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge_with_capabilities(
            callback,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
                supports_wgpu_shared_texture: false,
            },
        );

        assert_eq!(
            selected.mode,
            BackendContentBridgeMode::WgpuPreferredFallbackGlCallback
        );

        clear_backend_bridge_env_for_tests();
    }

    #[test]
    fn capability_probe_requires_servo_wgpu_backend_and_parent_render_callback() {
        assert_eq!(
            content_bridge_capabilities_from_observed_context(true, true),
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
                supports_wgpu_shared_texture: true,
            }
        );
        assert_eq!(
            content_bridge_capabilities_from_observed_context(true, false),
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: false,
                supports_wgpu_shared_texture: false,
            }
        );
        assert_eq!(
            content_bridge_capabilities_from_observed_context(false, true),
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: false,
                supports_wgpu_shared_texture: true,
            }
        );
    }

    #[test]
    fn bridge_path_maps_per_bridge_mode() {
        assert_eq!(
            backend_content_bridge_path(BackendContentBridgeMode::WgpuShared),
            BACKEND_BRIDGE_PATH_WGPU_SHARED
        );
        assert_eq!(
            backend_content_bridge_path(BackendContentBridgeMode::GlCallback),
            BACKEND_BRIDGE_PATH_GL_CALLBACK
        );
        assert_eq!(
            backend_content_bridge_path(
                BackendContentBridgeMode::WgpuPreferredFallbackGlCallback,
            ),
            BACKEND_BRIDGE_PATH_WGPU_PREFERRED_FALLBACK_GL
        );
    }
}
