/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

use super::BackendParentRenderRegionInPixels;

pub(crate) type BackendGraphicsContext = glow::Context;
pub(crate) type BackendFramebufferHandle = glow::NativeFramebuffer;
pub(crate) type BackendParentRenderCallback = Arc<
    dyn Fn(&BackendGraphicsContext, BackendParentRenderRegionInPixels) + Send + Sync,
>;

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
        glow::HasContext::viewport(gl, viewport[0], viewport[1], viewport[2], viewport[3]);
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