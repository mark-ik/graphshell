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

pub(crate) fn texture_token_from_handle(handle: &egui::TextureHandle) -> BackendTextureToken {
	BackendTextureToken(handle.id())
}

pub(crate) fn texture_id_from_token(token: BackendTextureToken) -> egui::TextureId {
	token.0
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

impl UiRenderBackendContract for UiRenderBackend {
	fn init_surface_accesskit<Event>(
		&mut self,
		event_loop: &ActiveEventLoop,
		window: &Window,
		event_loop_proxy: EventLoopProxy<Event>,
	)
	where
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
		self.on_window_event(window, event)
	}

	fn run_ui_frame(&mut self, window: &Window, run_ui: impl FnMut(&Context)) {
		self.run(window, run_ui)
	}

	fn register_texture_token(&mut self, texture_id: egui::TextureId) -> BackendTextureToken {
		BackendTextureToken(texture_id)
	}

	fn submit_frame(&mut self, window: &Window) {
		self.paint(window);
	}

	fn destroy_surface(&mut self) {
		self.destroy();
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
