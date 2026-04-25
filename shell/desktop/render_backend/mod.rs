/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

mod gl_backend;
mod shared_wgpu_context;
mod wgpu_backend;

use std::rc::Rc;
use std::sync::Arc;

use dpi::PhysicalSize;
use euclid::{Point2D, Rect, Size2D};
use servo::{OffscreenRenderingContext, RenderingContextCore, WindowRenderingContext};
use winit::window::Window;

pub(crate) use gl_backend::{
    BackendFramebufferHandle, BackendGraphicsContext, BackendParentRenderCallback,
};
#[cfg(feature = "gl_compat")]
pub(crate) use gl_backend::{
    backend_active_texture, backend_bind_framebuffer, backend_chaos_alternate_texture_unit,
    backend_chaos_framebuffer_handle, backend_framebuffer_binding,
    backend_framebuffer_from_binding, backend_is_blend_enabled, backend_is_scissor_enabled,
    backend_primary_texture_unit, backend_scissor_box, backend_set_active_texture,
    backend_set_blend_enabled, backend_set_scissor_box, backend_set_scissor_enabled,
    backend_set_viewport, backend_viewport,
};
pub(crate) use wgpu_backend::{
    BackendCustomPass, HostNeutralRenderBackend, UiRenderBackendContract, UiRenderBackendHandle,
    activate_ui_render_backend, begin_ui_render_backend_paint, create_ui_render_backend,
    custom_pass_from_backend_viewport, end_ui_render_backend_paint,
    register_custom_paint_callback, texture_id_from_token, texture_token_from_handle,
};

pub(crate) fn create_shared_wgpu_rendering_context(
    device: servo::wgpu::Device,
    queue: servo::wgpu::Queue,
    size: PhysicalSize<u32>,
) -> Rc<dyn RenderingContextCore> {
    Rc::new(shared_wgpu_context::SharedWgpuRenderingContext::new(
        device, queue, size,
    ))
}

pub(crate) struct UiHostRenderBootstrap {
    rendering_context: Rc<OffscreenRenderingContext>,
    window_rendering_context: Rc<WindowRenderingContext>,
    wgpu: UiWgpuHostBootstrap,
}

impl UiHostRenderBootstrap {
    pub(crate) fn new(
        rendering_context: Rc<OffscreenRenderingContext>,
        window_rendering_context: Rc<WindowRenderingContext>,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> Self {
        Self {
            rendering_context,
            window_rendering_context,
            wgpu: UiWgpuHostBootstrap::from_event_loop(event_loop),
        }
    }

    pub(crate) fn rendering_context(&self) -> &OffscreenRenderingContext {
        self.rendering_context.as_ref()
    }

    pub(crate) fn window_rendering_context(&self) -> &WindowRenderingContext {
        self.window_rendering_context.as_ref()
    }

    pub(crate) fn into_contexts(
        self,
    ) -> (Rc<OffscreenRenderingContext>, Rc<WindowRenderingContext>) {
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

impl UiWgpuHostBootstrap {
    fn from_event_loop(event_loop: &winit::event_loop::ActiveEventLoop) -> Self {
        let owned_handle = event_loop.owned_display_handle();
        let mut configuration = egui_wgpu::WgpuConfiguration::default();
        configuration.wgpu_setup = egui_wgpu::WgpuSetup::from_display_handle(owned_handle);
        Self { configuration }
    }
}

impl Default for UiWgpuHostBootstrap {
    fn default() -> Self {
        Self {
            configuration: egui_wgpu::WgpuConfiguration::default(),
        }
    }
}

/// Override the default content bridge mode for debugging.
/// Values: "wgpu_shared", "wgpu_preferred", "gl_callback".
const BACKEND_BRIDGE_MODE_ENV_VAR: &str = "GRAPHSHELL_BACKEND_BRIDGE_MODE";
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
pub(crate) type BackendSharedWgpuImport =
    std::rc::Rc<dyn Fn(servo::wgpu::Device, servo::wgpu::Queue) -> Option<servo::wgpu::Texture>>;

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

fn content_bridge_capabilities_from_observed_context(
    has_parent_render_callback: bool,
) -> BackendContentBridgeCapabilities {
    BackendContentBridgeCapabilities {
        supports_wgpu_parent_render_bridge: has_parent_render_callback,
        supports_wgpu_shared_texture: true,
    }
}

/// Resolve the content bridge mode.
///
/// Default: `WgpuPreferredFallbackGlCallback` — uses the wgpu shared texture
/// path when the capability is present, falls back to GL callback otherwise.
/// Set `GRAPHSHELL_BACKEND_BRIDGE_MODE` to override for debugging:
///   - `wgpu_shared` — wgpu only, no fallback (degrades to GL if unavailable)
///   - `gl_callback` — force GL callback path
///   - `wgpu_preferred` — explicit default (wgpu with GL fallback)
fn resolve_backend_content_bridge_mode(
    capabilities: BackendContentBridgeCapabilities,
) -> BackendContentBridgeMode {
    let requested = std::env::var(BACKEND_BRIDGE_MODE_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .and_then(|value| match value.as_str() {
            "wgpu_shared" => Some(BackendContentBridgeMode::WgpuShared),
            "wgpu_preferred" => Some(BackendContentBridgeMode::WgpuPreferredFallbackGlCallback),
            "gl_callback" => Some(BackendContentBridgeMode::GlCallback),
            _ => None,
        })
        .unwrap_or(BackendContentBridgeMode::WgpuPreferredFallbackGlCallback);

    match requested {
        BackendContentBridgeMode::WgpuShared if !capabilities.supports_wgpu_shared_texture => {
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

pub(crate) fn select_backend_content_bridge_with_capabilities(
    callback: BackendParentRenderCallback,
    capabilities: BackendContentBridgeCapabilities,
) -> BackendContentBridgeSelection {
    let mode = resolve_backend_content_bridge_mode(capabilities);

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
    }
}

#[cfg(test)]
pub(crate) fn set_backend_bridge_mode_env_for_tests(value: &str) {
    unsafe {
        std::env::set_var(BACKEND_BRIDGE_MODE_ENV_VAR, value);
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

// 2026-04-25 servo-into-verso S3a: BackendViewportInPixels was
// extracted into `graphshell_runtime::ports::BackendViewportInPixels`
// so the host-port trait surface (HostSurfacePort) can ship a
// host-neutral viewport type. Re-export the canonical type here so
// the egui-host compositor + render_backend internals keep their
// existing import path while the trait surface stays portable.
pub(crate) use graphshell_runtime::ports::BackendViewportInPixels;

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
    fn bridge_mode_defaults_to_wgpu_preferred_with_gl_fallback() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge_with_capabilities(
            callback,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
                supports_wgpu_shared_texture: true,
            },
        );

        assert_eq!(
            selected.mode,
            BackendContentBridgeMode::WgpuPreferredFallbackGlCallback
        );
    }

    #[test]
    fn bridge_mode_falls_back_to_gl_without_parent_render_callback() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge_with_capabilities(
            callback,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: false,
                supports_wgpu_shared_texture: true,
            },
        );

        assert_eq!(selected.mode, BackendContentBridgeMode::GlCallback);
    }

    #[test]
    fn env_override_forces_gl_callback() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();
        set_backend_bridge_mode_env_for_tests("gl_callback");

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge_with_capabilities(
            callback,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
                supports_wgpu_shared_texture: true,
            },
        );

        assert_eq!(selected.mode, BackendContentBridgeMode::GlCallback);

        clear_backend_bridge_env_for_tests();
    }

    #[test]
    fn env_override_forces_wgpu_shared() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();
        set_backend_bridge_mode_env_for_tests("wgpu_shared");

        let callback: BackendParentRenderCallback = std::sync::Arc::new(|_, _| {});
        let selected = select_backend_content_bridge_with_capabilities(
            callback,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
                supports_wgpu_shared_texture: true,
            },
        );

        assert_eq!(selected.mode, BackendContentBridgeMode::WgpuShared);

        clear_backend_bridge_env_for_tests();
    }

    #[test]
    fn wgpu_shared_override_degrades_without_capability() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();
        set_backend_bridge_mode_env_for_tests("wgpu_shared");

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
    fn capability_probe_with_parent_callback() {
        assert_eq!(
            content_bridge_capabilities_from_observed_context(true),
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
                supports_wgpu_shared_texture: true,
            }
        );
        assert_eq!(
            content_bridge_capabilities_from_observed_context(false),
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
            backend_content_bridge_path(BackendContentBridgeMode::WgpuPreferredFallbackGlCallback,),
            BACKEND_BRIDGE_PATH_WGPU_PREFERRED_FALLBACK_GL
        );
    }
}
