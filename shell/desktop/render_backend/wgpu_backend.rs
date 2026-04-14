/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::num::NonZeroU32;

use egui::{Context, LayerId, Rect as EguiRect};
use egui_wgpu::RendererOptions;
use egui_wgpu::winit::Painter;
use egui_winit::EventResponse;
use pollster::block_on;
use servo::RenderingContext;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;

use super::{BackendGraphicsContext, BackendTextureToken, BackendViewportInPixels, UiRenderBackendInit};

#[derive(Clone, Default)]
pub(crate) struct BackendCustomPass;

struct PendingFrame {
    textures_delta: egui::TexturesDelta,
    clipped_primitives: Vec<egui::epaint::ClippedPrimitive>,
    pixels_per_point: f32,
}

/// GL-shaped stub: creates a no-op custom pass on the wgpu backend.
///
/// This is the `ParentRenderCallback` fallback path shape. It does not wire
/// into the wgpu pre-render texture handoff model (`SharedWgpuTexture`).
/// Retained for GL compat fallback builds; retire with Phase F.
pub(crate) fn custom_pass_from_backend_viewport<F>(_render: F) -> BackendCustomPass
where
    F: Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync + 'static,
{
    BackendCustomPass
}

/// GL-shaped stub: no-op paint callback registration on the wgpu backend.
///
/// This is the `ParentRenderCallback` fallback path shape. The wgpu primary
/// path (`SharedWgpuTexture`) composes via pre-render texture import, not
/// egui paint callbacks. Retained for GL compat fallback builds; retire with Phase F.
pub(crate) fn register_custom_paint_callback(
    _ctx: &Context,
    _layer: LayerId,
    _rect: EguiRect,
    _callback: BackendCustomPass,
) {
}

pub(crate) fn texture_token_from_handle(handle: &egui::TextureHandle) -> BackendTextureToken {
    BackendTextureToken(handle.id())
}

pub(crate) fn texture_id_from_token(token: BackendTextureToken) -> egui::TextureId {
    token.0
}

pub(crate) fn activate_ui_render_backend(render_context: &servo::OffscreenRenderingContext) {
    render_context
        .make_current()
        .expect("Could not make window RenderingContext current");
}

pub(crate) fn begin_ui_render_backend_paint(_render_context: &servo::OffscreenRenderingContext) {}

pub(crate) fn end_ui_render_backend_paint(_render_context: &servo::OffscreenRenderingContext) {}

pub(crate) fn create_ui_render_backend(
    _event_loop: &ActiveEventLoop,
    init: UiRenderBackendInit<'_>,
) -> UiRenderBackendHandle {
    let egui_ctx = Context::default();
    let mut painter = block_on(Painter::new(
        egui_ctx.clone(),
        init.render_host.wgpu_bootstrap().configuration.clone(),
        false,
        RendererOptions::default(),
    ));

    block_on(unsafe { painter.set_window_unsafe(egui::ViewportId::ROOT, Some(init.window)) })
        .expect("Could not create egui_wgpu surface for the root viewport");

    let mut egui_winit = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        init.window,
        Some(init.window.scale_factor() as f32),
        init.window.theme(),
        painter.max_texture_side(),
    );
    if let Some(max_texture_side) = painter.max_texture_side() {
        egui_winit.set_max_texture_side(max_texture_side);
    }

    UiRenderBackendHandle {
        egui_ctx,
        egui_winit,
        painter,
        pending_frame: None,
        native_textures: HashMap::new(),
    }
}

pub(crate) struct UiRenderBackendHandle {
    egui_ctx: Context,
    egui_winit: egui_winit::State,
    painter: Painter,
    pending_frame: Option<PendingFrame>,
    native_textures: HashMap<BackendTextureToken, servo::wgpu::Texture>,
}

pub(crate) trait UiRenderBackendContract {
    fn init_surface_accesskit<Event>(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        event_loop_proxy: EventLoopProxy<Event>,
    ) where
        Event: From<egui_winit::accesskit_winit::Event> + Send + 'static;

    fn egui_context(&self) -> &Context;
    fn egui_context_mut(&mut self) -> &mut Context;
    fn egui_winit_state_mut(&mut self) -> &mut egui_winit::State;

    fn handle_window_event(&mut self, window: &Window, event: &WindowEvent) -> EventResponse;
    fn run_ui_frame(&mut self, window: &Window, run_ui: impl FnMut(&Context, &mut Self));

    fn register_texture_token(&mut self, texture_id: egui::TextureId) -> BackendTextureToken;
    fn shared_wgpu_device_queue(&self) -> Option<(servo::wgpu::Device, servo::wgpu::Queue)>;
    fn upsert_native_texture(
        &mut self,
        existing: Option<BackendTextureToken>,
        texture: &servo::wgpu::Texture,
    ) -> Option<BackendTextureToken>;
    fn free_native_texture(&mut self, token: BackendTextureToken);

    fn submit_frame(&mut self, window: &Window);
    fn destroy_surface(&mut self);
}

impl UiRenderBackendHandle {
    fn render_state(&self) -> Option<egui_wgpu::RenderState> {
        self.painter.render_state()
    }

    fn sync_surface_size(&mut self, window: &Window) {
        let size = window.inner_size();
        let (Some(width), Some(height)) = (
            NonZeroU32::new(size.width),
            NonZeroU32::new(size.height),
        ) else {
            return;
        };

        self.painter
            .on_window_resized(egui::ViewportId::ROOT, width, height);
    }
}

impl UiRenderBackendContract for UiRenderBackendHandle {
    fn init_surface_accesskit<Event>(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        event_loop_proxy: EventLoopProxy<Event>,
    ) where
        Event: From<egui_winit::accesskit_winit::Event> + Send + 'static,
    {
        self.egui_winit
            .init_accesskit(event_loop, window, event_loop_proxy);
    }

    fn egui_context(&self) -> &Context {
        &self.egui_ctx
    }

    fn egui_context_mut(&mut self) -> &mut Context {
        &mut self.egui_ctx
    }

    fn egui_winit_state_mut(&mut self) -> &mut egui_winit::State {
        &mut self.egui_winit
    }

    fn handle_window_event(&mut self, window: &Window, event: &WindowEvent) -> EventResponse {
        let response = self.egui_winit.on_window_event(window, event);
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            self.sync_surface_size(window);
        }
        response
    }

    fn run_ui_frame(&mut self, window: &Window, mut run_ui: impl FnMut(&Context, &mut Self)) {
        let raw_input = self.egui_winit.take_egui_input(window);
        let egui_ctx = self.egui_ctx.clone();
        let full_output = egui_ctx.run(raw_input, |ctx| run_ui(ctx, self));
        self.egui_winit
            .handle_platform_output(window, full_output.platform_output);

        self.pending_frame = Some(PendingFrame {
            clipped_primitives: self
                .egui_ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point),
            textures_delta: full_output.textures_delta,
            pixels_per_point: full_output.pixels_per_point,
        });
    }

    fn register_texture_token(&mut self, texture_id: egui::TextureId) -> BackendTextureToken {
        BackendTextureToken(texture_id)
    }

    fn shared_wgpu_device_queue(&self) -> Option<(servo::wgpu::Device, servo::wgpu::Queue)> {
        let render_state = self.render_state()?;
        Some((render_state.device, render_state.queue))
    }

    fn upsert_native_texture(
        &mut self,
        existing: Option<BackendTextureToken>,
        texture: &servo::wgpu::Texture,
    ) -> Option<BackendTextureToken> {
        let render_state = self.render_state()?;
        let texture_view = texture.create_view(&servo::wgpu::TextureViewDescriptor::default());
        let mut renderer = render_state.renderer.write();

        let token = if let Some(existing) = existing {
            renderer.update_egui_texture_from_wgpu_texture(
                &render_state.device,
                &texture_view,
                servo::wgpu::FilterMode::Linear,
                existing.0,
            );
            existing
        } else {
            BackendTextureToken(renderer.register_native_texture(
                &render_state.device,
                &texture_view,
                servo::wgpu::FilterMode::Linear,
            ))
        };

        self.native_textures.insert(token, texture.clone());
        Some(token)
    }

    fn free_native_texture(&mut self, token: BackendTextureToken) {
        if let Some(render_state) = self.render_state() {
            render_state.renderer.write().free_texture(&token.0);
        }
        self.native_textures.remove(&token);
    }

    fn submit_frame(&mut self, _window: &Window) {
        let Some(frame) = self.pending_frame.take() else {
            return;
        };

        let _ = self.painter.paint_and_update_textures(
            egui::ViewportId::ROOT,
            frame.pixels_per_point,
            [0.0, 0.0, 0.0, 0.0],
            &frame.clipped_primitives,
            &frame.textures_delta,
            Vec::new(),
        );
    }

    fn destroy_surface(&mut self) {
        self.native_textures.clear();
        self.painter.destroy();
    }
}
