/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shell-side bridge from Servo's `RenderingContextCore` to the host-neutral
//! [`RenderingContextProducer`] trait in `graphshell-runtime`.
//!
//! The adapter is used wherever code has a `Rc<dyn servo::RenderingContextCore>`
//! and needs to hand it to portable consumers as `&dyn RenderingContextProducer`
//! (or `Rc<dyn RenderingContextProducer>` when ownership transfer is required).
//!
//! The trait surface is wgpu-first — `size_in_pixels`, `resize`, `present`. GL
//! compat (`make_current`, `prepare_for_rendering`) is path-specific to
//! `OffscreenRenderingContext` consumers and not part of the producer contract;
//! see `compositor_adapter::paint_offscreen_content_pass` for that path.

use std::rc::Rc;

use dpi::PhysicalSize;
use graphshell_runtime::RenderingContextProducer;
use servo::RenderingContextCore;

/// Adapter that wraps a `Rc<dyn RenderingContextCore>` and presents it as a
/// `dyn RenderingContextProducer`.
pub(crate) struct ServoRenderingContextProducer {
    inner: Rc<dyn RenderingContextCore>,
}

impl ServoRenderingContextProducer {
    pub(crate) fn new(inner: Rc<dyn RenderingContextCore>) -> Self {
        Self { inner }
    }
}

impl RenderingContextProducer for ServoRenderingContextProducer {
    fn size_in_pixels(&self) -> (u32, u32) {
        let PhysicalSize { width, height } = self.inner.size();
        (width, height)
    }

    fn resize(&self, width: u32, height: u32) {
        self.inner.resize(PhysicalSize::new(width, height));
    }

    fn present(&self) {
        self.inner.present();
    }
}
