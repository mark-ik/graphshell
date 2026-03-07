pub(crate) mod canvas;
pub(crate) mod profile_registry;
pub(crate) mod viewer_surface;
pub(crate) mod workbench_surface;

/// Conformance level for a surface capability declaration.
///
/// Used by `CapabilityDeclaration` to declare whether a surface or profile
/// fully, partially, or does not implement a cross-cutting guarantee. Partial
/// conformance must carry a `reason`.
///
/// Populated at registry registration time; read by subsystem diagnostics and
/// validation to drive degraded-path warnings and conformance audit trails.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ConformanceLevel {
    /// Guarantee fully satisfied by this surface/profile.
    Full,
    /// Guarantee partially satisfied; `reason` must describe the gap.
    Partial,
    /// Guarantee not provided by this surface/profile.
    None,
}

/// Conformance declaration for a surface or viewer subsystem.
///
/// Registered alongside the owning profile to allow diagnostics to audit
/// conformance without reaching into runtime rendering code.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CapabilityDeclaration {
    pub(crate) level: ConformanceLevel,
    /// Required when `level` is `Partial` or `None`; describes the gap or
    /// degraded path.
    pub(crate) reason: Option<String>,
}

impl CapabilityDeclaration {
    pub(crate) fn full() -> Self {
        Self {
            level: ConformanceLevel::Full,
            reason: None,
        }
    }

    pub(crate) fn partial(reason: impl Into<String>) -> Self {
        Self {
            level: ConformanceLevel::Partial,
            reason: Some(reason.into()),
        }
    }

    pub(crate) fn none(reason: impl Into<String>) -> Self {
        Self {
            level: ConformanceLevel::None,
            reason: Some(reason.into()),
        }
    }
}

/// Folded subsystem conformance declarations carried by surface descriptors.
///
/// This keeps subsystem declarations typed and colocated with the owning
/// surface profile while allowing runtime/diagnostics code to inspect one field
/// instead of ad hoc per-subsystem plumbing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SurfaceSubsystemCapabilities {
    pub(crate) accessibility: CapabilityDeclaration,
    pub(crate) security: CapabilityDeclaration,
    pub(crate) storage: CapabilityDeclaration,
    pub(crate) history: CapabilityDeclaration,
}

impl SurfaceSubsystemCapabilities {
    pub(crate) fn full() -> Self {
        Self {
            accessibility: CapabilityDeclaration::full(),
            security: CapabilityDeclaration::full(),
            storage: CapabilityDeclaration::full(),
            history: CapabilityDeclaration::full(),
        }
    }
}

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

    #[test]
    fn conformance_level_full_has_no_reason() {
        let caps = CapabilityDeclaration::full();
        assert_eq!(caps.level, ConformanceLevel::Full);
        assert!(caps.reason.is_none());
    }

    #[test]
    fn conformance_level_partial_has_reason() {
        let caps = CapabilityDeclaration::partial("WebView bridge unavailable");
        assert_eq!(caps.level, ConformanceLevel::Partial);
        assert_eq!(caps.reason.as_deref(), Some("WebView bridge unavailable"));
    }

    #[test]
    fn conformance_level_none_has_reason() {
        let caps = CapabilityDeclaration::none("content rendered without sandbox");
        assert_eq!(caps.level, ConformanceLevel::None);
        assert!(caps.reason.is_some());
    }

    #[test]
    fn default_surface_profiles_declare_full_conformance() {
        let domain = LayoutDomainRegistry::default();
        let resolution = domain.resolve_profile(
            crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT,
            crate::registries::domain::layout::workbench_surface::WORKBENCH_SURFACE_DEFAULT,
            crate::registries::domain::layout::viewer_surface::VIEWER_SURFACE_DEFAULT,
        );
        assert_eq!(
            resolution.canvas.profile.subsystems.accessibility.level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution
                .workbench_surface
                .profile
                .subsystems
                .accessibility
                .level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution
                .viewer_surface
                .profile
                .subsystems
                .accessibility
                .level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution.canvas.profile.subsystems.security.level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution.canvas.profile.subsystems.storage.level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution.canvas.profile.subsystems.history.level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution
                .workbench_surface
                .profile
                .subsystems
                .storage
                .level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution
                .workbench_surface
                .profile
                .subsystems
                .history
                .level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution.viewer_surface.profile.subsystems.storage.level,
            ConformanceLevel::Full
        );
        assert_eq!(
            resolution.viewer_surface.profile.subsystems.history.level,
            ConformanceLevel::Full
        );
    }

    #[test]
    fn conformance_capability_structs_round_trip_via_json() {
        let capability = CapabilityDeclaration::partial("keyboard-only degraded path");
        let json = serde_json::to_string(&capability).expect("capability should serialize");
        let restored: CapabilityDeclaration =
            serde_json::from_str(&json).expect("capability should deserialize");

        assert_eq!(restored.level, ConformanceLevel::Partial);
        assert_eq!(
            restored.reason.as_deref(),
            Some("keyboard-only degraded path")
        );
    }
}
