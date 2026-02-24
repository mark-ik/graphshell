pub(crate) mod canvas;
pub(crate) mod viewer_surface;
pub(crate) mod workbench_surface;

use canvas::CanvasRegistry;
use viewer_surface::ViewerSurfaceRegistry;
use workbench_surface::WorkbenchSurfaceRegistry;

#[derive(Debug, Clone)]
pub(crate) struct LayoutDomainProfileResolution {
    pub(crate) canvas: canvas::CanvasSurfaceResolution,
    pub(crate) workbench_surface: workbench_surface::WorkbenchSurfaceResolution,
    pub(crate) viewer_surface: viewer_surface::ViewerSurfaceResolution,
}

#[derive(Default)]
pub(crate) struct LayoutDomainRegistry {
    canvas: CanvasRegistry,
    workbench_surface: WorkbenchSurfaceRegistry,
    viewer_surface: ViewerSurfaceRegistry,
}

impl LayoutDomainRegistry {
    pub(crate) fn canvas(&self) -> &CanvasRegistry {
        &self.canvas
    }

    pub(crate) fn workbench_surface(&self) -> &WorkbenchSurfaceRegistry {
        &self.workbench_surface
    }

    pub(crate) fn viewer_surface(&self) -> &ViewerSurfaceRegistry {
        &self.viewer_surface
    }

    pub(crate) fn resolve_profile(
        &self,
        canvas_profile_id: &str,
        workbench_profile_id: &str,
        viewer_profile_id: &str,
    ) -> LayoutDomainProfileResolution {
        LayoutDomainProfileResolution {
            canvas: self.canvas.resolve(canvas_profile_id),
            workbench_surface: self.workbench_surface.resolve(workbench_profile_id),
            viewer_surface: self.viewer_surface.resolve(viewer_profile_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT;
    use crate::registries::domain::layout::viewer_surface::VIEWER_SURFACE_DEFAULT;
    use crate::registries::domain::layout::workbench_surface::WORKBENCH_SURFACE_DEFAULT;

    #[test]
    fn layout_domain_resolves_composed_default_profile() {
        let domain = LayoutDomainRegistry::default();
        let resolution = domain.resolve_profile(
            CANVAS_PROFILE_DEFAULT,
            WORKBENCH_SURFACE_DEFAULT,
            VIEWER_SURFACE_DEFAULT,
        );

        assert!(resolution.canvas.matched);
        assert!(resolution.workbench_surface.matched);
        assert!(resolution.viewer_surface.matched);
        assert!(!resolution.canvas.fallback_used);
        assert!(!resolution.workbench_surface.fallback_used);
        assert!(!resolution.viewer_surface.fallback_used);
    }

    #[test]
    fn layout_domain_falls_back_each_surface_independently() {
        let domain = LayoutDomainRegistry::default();
        let resolution = domain.resolve_profile(
            "canvas:unknown",
            "workbench_surface:unknown",
            "viewer_surface:unknown",
        );

        assert!(resolution.canvas.fallback_used);
        assert!(resolution.workbench_surface.fallback_used);
        assert!(resolution.viewer_surface.fallback_used);
    }
}
