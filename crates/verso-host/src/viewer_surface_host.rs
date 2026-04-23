/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use graphshell_core::graph::NodeKey;
use graphshell_core::viewer_host::{ViewerSurfaceError, ViewerSurfaceHost};

pub trait ViewerSurfaceRegistryHost {
    type Surface;

    fn get_or_insert_surface_with<F>(&mut self, node_key: NodeKey, create_surface: F)
    where
        F: FnOnce() -> Self::Surface;

    fn retire_surface(&mut self, node_key: NodeKey);

    fn has_surface(&self, node_key: NodeKey) -> bool;
}

pub struct ServoViewerSurfaceHost<Surface, Allocate>
where
    Allocate: FnMut() -> Surface,
{
    allocate_surface: Allocate,
}

impl<Surface, Allocate> ServoViewerSurfaceHost<Surface, Allocate>
where
    Allocate: FnMut() -> Surface,
{
    pub fn new(allocate_surface: Allocate) -> Self {
        Self { allocate_surface }
    }
}

impl<Registry, Surface, Allocate> ViewerSurfaceHost<Registry>
    for ServoViewerSurfaceHost<Surface, Allocate>
where
    Registry: ViewerSurfaceRegistryHost<Surface = Surface>,
    Allocate: FnMut() -> Surface,
{
    fn allocate_surface(
        &mut self,
        registry: &mut Registry,
        node_key: NodeKey,
    ) -> Result<(), ViewerSurfaceError> {
        registry.get_or_insert_surface_with(node_key, || (self.allocate_surface)());
        Ok(())
    }

    fn retire_surface(&mut self, registry: &mut Registry, node_key: NodeKey) {
        registry.retire_surface(node_key);
    }

    fn has_surface(&self, registry: &Registry, node_key: NodeKey) -> bool {
        registry.has_surface(node_key)
    }
}

#[derive(Default)]
pub struct NoopViewerSurfaceHost;

impl<Registry> ViewerSurfaceHost<Registry> for NoopViewerSurfaceHost
where
    Registry: ViewerSurfaceRegistryHost,
{
    fn allocate_surface(
        &mut self,
        _registry: &mut Registry,
        _node_key: NodeKey,
    ) -> Result<(), ViewerSurfaceError> {
        Ok(())
    }

    fn retire_surface(&mut self, registry: &mut Registry, node_key: NodeKey) {
        registry.retire_surface(node_key);
    }

    fn has_surface(&self, registry: &Registry, node_key: NodeKey) -> bool {
        registry.has_surface(node_key)
    }
}