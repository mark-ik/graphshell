/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

mod servo_rendering_context_producer;
mod shared_wgpu_context;
mod wgpu_backend;

use std::rc::Rc;

use dpi::PhysicalSize;
use servo::{OffscreenRenderingContext, RenderingContextCore, WindowRenderingContext};
use winit::window::Window;

pub(crate) use wgpu_backend::{
    HostNeutralRenderBackend, UiRenderBackendContract, UiRenderBackendHandle,
    activate_ui_render_backend, begin_ui_render_backend_paint, create_ui_render_backend,
    end_ui_render_backend_paint, texture_id_from_token, texture_token_from_handle,
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

// 2026-04-25 servo-into-verso S3a: BackendViewportInPixels was
// extracted into `graphshell_runtime::ports::BackendViewportInPixels`
// so the host-port trait surface (HostSurfacePort) can ship a
// host-neutral viewport type. Re-export the canonical type here so
// the egui-host compositor + render_backend internals keep their
// existing import path while the trait surface stays portable.
pub(crate) use graphshell_runtime::ports::BackendViewportInPixels;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct BackendTextureToken(pub(crate) egui::TextureId);
