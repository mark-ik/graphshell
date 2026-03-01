/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

use egui::{Context, LayerId, PaintCallback, Rect as EguiRect};
use egui_winit::EventResponse;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;

pub(crate) use egui_glow::CallbackFn as BackendCallbackFn;
pub(crate) use egui_glow::EguiGlow as UiRenderBackend;
pub(crate) use egui_glow::glow;

pub(crate) type BackendGraphicsContext = glow::Context;
pub(crate) type BackendFramebufferHandle = glow::NativeFramebuffer;
pub(crate) type BackendGraphicsApi = std::sync::Arc<BackendGraphicsContext>;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BackendTextureToken(pub(crate) egui::TextureId);

#[derive(Clone)]
pub(crate) struct BackendCustomPass {
	callback: Arc<BackendCallbackFn>,
}

impl BackendCustomPass {
	pub(crate) fn from_callback_fn(callback: BackendCallbackFn) -> Self {
		Self {
			callback: Arc::new(callback),
		}
	}
}

pub(crate) fn custom_pass_from_backend_viewport<F>(render: F) -> BackendCustomPass
where
	F: Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync + 'static,
{
	BackendCustomPass::from_callback_fn(BackendCallbackFn::new(move |info, painter| {
		let clip = info.viewport_in_pixels();
		render(
			painter.gl(),
			BackendViewportInPixels {
				left_px: clip.left_px,
				from_bottom_px: clip.from_bottom_px,
				width_px: clip.width_px,
				height_px: clip.height_px,
			},
		);
	}))
}

pub(crate) fn custom_pass_from_glow_viewport<F>(render: F) -> BackendCustomPass
where
	F: Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync + 'static,
{
	custom_pass_from_backend_viewport(render)
}

pub(crate) fn backend_scissor_box(gl: &BackendGraphicsContext) -> [i32; 4] {
	let mut scissor_box = [0_i32; 4];

	unsafe {
		glow::HasContext::get_parameter_i32_slice(gl, glow::SCISSOR_BOX, &mut scissor_box);
	}

	scissor_box
}

pub(crate) fn backend_set_scissor_box(gl: &BackendGraphicsContext, scissor_box: [i32; 4]) {
	unsafe {
		glow::HasContext::scissor(
			gl,
			scissor_box[0],
			scissor_box[1],
			scissor_box[2],
			scissor_box[3],
		);
	}
}

pub(crate) fn backend_is_scissor_enabled(gl: &BackendGraphicsContext) -> bool {
	unsafe { glow::HasContext::is_enabled(gl, glow::SCISSOR_TEST) }
}

pub(crate) fn backend_set_scissor_enabled(gl: &BackendGraphicsContext, enabled: bool) {
	unsafe {
		if enabled {
			glow::HasContext::enable(gl, glow::SCISSOR_TEST);
		} else {
			glow::HasContext::disable(gl, glow::SCISSOR_TEST);
		}
	}
}

pub(crate) fn backend_viewport(gl: &BackendGraphicsContext) -> [i32; 4] {
	let mut viewport = [0_i32; 4];

	unsafe {
		glow::HasContext::get_parameter_i32_slice(gl, glow::VIEWPORT, &mut viewport);
	}

	viewport
}

pub(crate) fn backend_set_viewport(gl: &BackendGraphicsContext, viewport: [i32; 4]) {
	unsafe {
		glow::HasContext::viewport(
			gl,
			viewport[0],
			viewport[1],
			viewport[2],
			viewport[3],
		);
	}
}

pub(crate) fn backend_is_blend_enabled(gl: &BackendGraphicsContext) -> bool {
	unsafe { glow::HasContext::is_enabled(gl, glow::BLEND) }
}

pub(crate) fn backend_set_blend_enabled(gl: &BackendGraphicsContext, enabled: bool) {
	unsafe {
		if enabled {
			glow::HasContext::enable(gl, glow::BLEND);
		} else {
			glow::HasContext::disable(gl, glow::BLEND);
		}
	}
}

pub(crate) fn backend_active_texture(gl: &BackendGraphicsContext) -> i32 {
	unsafe { glow::HasContext::get_parameter_i32(gl, glow::ACTIVE_TEXTURE) }
}

pub(crate) fn backend_set_active_texture(gl: &BackendGraphicsContext, texture: u32) {
	unsafe {
		glow::HasContext::active_texture(gl, texture);
	}
}

pub(crate) fn backend_framebuffer_binding(gl: &BackendGraphicsContext) -> i32 {
	unsafe { glow::HasContext::get_parameter_i32(gl, glow::FRAMEBUFFER_BINDING) }
}

pub(crate) fn backend_bind_framebuffer(
	gl: &BackendGraphicsContext,
	framebuffer: Option<BackendFramebufferHandle>,
) {
	unsafe {
		glow::HasContext::bind_framebuffer(gl, glow::FRAMEBUFFER, framebuffer);
	}
}

pub(crate) fn backend_framebuffer_from_binding(binding: i32) -> Option<BackendFramebufferHandle> {
	if binding <= 0 {
		None
	} else {
		std::num::NonZeroU32::new(binding as u32).map(glow::NativeFramebuffer)
	}
}

pub(crate) fn backend_chaos_framebuffer_handle() -> BackendFramebufferHandle {
	glow::NativeFramebuffer(std::num::NonZeroU32::new(9).expect("non-zero"))
}

pub(crate) fn backend_primary_texture_unit() -> u32 {
	glow::TEXTURE0
}

pub(crate) fn backend_chaos_alternate_texture_unit() -> u32 {
	glow::TEXTURE3
}

pub(crate) fn texture_token_from_handle(handle: &egui::TextureHandle) -> BackendTextureToken {
	BackendTextureToken(handle.id())
}

pub(crate) fn texture_id_from_token(token: BackendTextureToken) -> egui::TextureId {
	token.0
}

pub(crate) fn create_ui_render_backend(
	event_loop: &ActiveEventLoop,
	gl_api: BackendGraphicsApi,
) -> UiRenderBackendHandle {
	UiRenderBackendHandle {
		inner: UiRenderBackend::new(event_loop, gl_api, None, None, false),
	}
}

pub(crate) struct UiRenderBackendHandle {
	inner: UiRenderBackend,
}

pub(crate) trait UiRenderBackendContract {
	fn init_surface_accesskit<Event>(
		&mut self,
		event_loop: &ActiveEventLoop,
		window: &Window,
		event_loop_proxy: EventLoopProxy<Event>,
	)
	where
		Event: From<egui_winit::accesskit_winit::Event> + Send + 'static;

	fn egui_context(&self) -> &Context;
	fn egui_context_mut(&mut self) -> &mut Context;
	fn egui_winit_state_mut(&mut self) -> &mut egui_winit::State;

	fn handle_window_event(&mut self, window: &Window, event: &WindowEvent) -> EventResponse;
	fn run_ui_frame(&mut self, window: &Window, run_ui: impl FnMut(&Context));

	fn register_texture_token(&mut self, texture_id: egui::TextureId) -> BackendTextureToken;

	fn submit_frame(&mut self, window: &Window);
	fn destroy_surface(&mut self);
}

impl UiRenderBackendContract for UiRenderBackendHandle {
	fn init_surface_accesskit<Event>(
		&mut self,
		event_loop: &ActiveEventLoop,
		window: &Window,
		event_loop_proxy: EventLoopProxy<Event>,
	)
	where
		Event: From<egui_winit::accesskit_winit::Event> + Send + 'static,
	{
		self.inner
			.egui_winit
			.init_accesskit(event_loop, window, event_loop_proxy);
	}

	fn egui_context(&self) -> &Context {
		&self.inner.egui_ctx
	}

	fn egui_context_mut(&mut self) -> &mut Context {
		&mut self.inner.egui_ctx
	}

	fn egui_winit_state_mut(&mut self) -> &mut egui_winit::State {
		&mut self.inner.egui_winit
	}

	fn handle_window_event(&mut self, window: &Window, event: &WindowEvent) -> EventResponse {
		self.inner.on_window_event(window, event)
	}

	fn run_ui_frame(&mut self, window: &Window, run_ui: impl FnMut(&Context)) {
		self.inner.run(window, run_ui)
	}

	fn register_texture_token(&mut self, texture_id: egui::TextureId) -> BackendTextureToken {
		BackendTextureToken(texture_id)
	}

	fn submit_frame(&mut self, window: &Window) {
		self.inner.paint(window);
	}

	fn destroy_surface(&mut self) {
		self.inner.destroy();
	}
}

pub(crate) fn register_custom_paint_callback(
	ctx: &Context,
	layer: LayerId,
	rect: EguiRect,
	callback: BackendCustomPass,
) {
	ctx.layer_painter(layer).add(PaintCallback {
		rect,
		callback: callback.callback,
	});
}
