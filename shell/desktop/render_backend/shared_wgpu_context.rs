/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dpi::PhysicalSize;
use image::RgbaImage;
use log::warn;
use servo::{DeviceIntRect, RenderingContextCore, WgpuCapability};

/// Non-presenting rendering context backed by a shared wgpu device/queue.
///
/// Servo gets `WgpuCapability`, but `acquire_frame_target()` always returns
/// `None`, so WebRender renders into its internal composite texture instead of
/// presenting directly to a window surface.
pub(crate) struct SharedWgpuRenderingContext {
    device: servo::wgpu::Device,
    queue: servo::wgpu::Queue,
    size: Cell<PhysicalSize<u32>>,
}

impl SharedWgpuRenderingContext {
    pub(crate) fn new(
        device: servo::wgpu::Device,
        queue: servo::wgpu::Queue,
        size: PhysicalSize<u32>,
    ) -> Self {
        Self {
            device,
            queue,
            size: Cell::new(size),
        }
    }
}

impl RenderingContextCore for SharedWgpuRenderingContext {
    fn size(&self) -> PhysicalSize<u32> {
        self.size.get()
    }

    fn resize(&self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            warn!(
                "SharedWgpuRenderingContext: ignoring resize to {size:?} (dimensions must be >= 1)"
            );
            return;
        }
        self.size.set(size);
    }

    fn present(&self) {}

    fn read_to_image(&self, _rect: DeviceIntRect) -> Option<RgbaImage> {
        None
    }

    fn wgpu(&self) -> Option<&dyn WgpuCapability> {
        Some(self)
    }
}

impl WgpuCapability for SharedWgpuRenderingContext {
    fn device(&self) -> servo::wgpu::Device {
        self.device.clone()
    }

    fn queue(&self) -> servo::wgpu::Queue {
        self.queue.clone()
    }

    fn acquire_frame_target(&self) -> Option<servo::wgpu::TextureView> {
        None
    }
}
