use super::SurfaceSubsystemCapabilities;
use super::profile_registry::{ProfileRegistry, ProfileResolution};
use crate::registries::atomic::viewer::{ViewerCapability, ViewerRenderMode};

pub(crate) const VIEWER_SURFACE_DEFAULT: &str = "viewer_surface:default";
pub(crate) const VIEWER_SURFACE_WEB: &str = "viewer_surface:web";
pub(crate) const VIEWER_SURFACE_DOCUMENT: &str = "viewer_surface:document";
pub(crate) const VIEWER_SURFACE_EMBEDDED: &str = "viewer_surface:embedded";
pub(crate) const VIEWER_SURFACE_NATIVE_OVERLAY: &str = "viewer_surface:native_overlay";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ViewerSurfaceProfile {
    pub(crate) profile_id: String,
    pub(crate) reader_mode_default: bool,
    pub(crate) smooth_scroll_enabled: bool,
    pub(crate) zoom_step: f32,
    /// Folded subsystem conformance declarations for this viewer surface.
    #[serde(flatten)]
    pub(crate) subsystems: SurfaceSubsystemCapabilities,
}

pub(crate) type ViewerSurfaceResolution = ProfileResolution<ViewerSurfaceProfile>;

pub(crate) struct ViewerSurfaceRegistry {
    profiles: ProfileRegistry<ViewerSurfaceProfile>,
}

impl ViewerSurfaceRegistry {
    pub(crate) fn register(&mut self, profile_id: &str, profile: ViewerSurfaceProfile) {
        self.profiles.register(profile_id, profile);
    }

    pub(crate) fn resolve(&self, profile_id: &str) -> ViewerSurfaceResolution {
        self.profiles.resolve(profile_id, "viewer surface")
    }

    pub(crate) fn resolve_for_viewer(
        &self,
        viewer_id: &str,
        capability: Option<&ViewerCapability>,
    ) -> ViewerSurfaceResolution {
        let profile_id = profile_id_for_viewer(viewer_id, capability);
        self.resolve(profile_id)
    }
}

impl Default for ViewerSurfaceRegistry {
    fn default() -> Self {
        let mut registry = Self {
            profiles: ProfileRegistry::new(VIEWER_SURFACE_DEFAULT),
        };
        registry.register(
            VIEWER_SURFACE_DEFAULT,
            ViewerSurfaceProfile {
                profile_id: VIEWER_SURFACE_DEFAULT.to_string(),
                reader_mode_default: false,
                smooth_scroll_enabled: true,
                zoom_step: 1.1,
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        registry.register(
            VIEWER_SURFACE_WEB,
            ViewerSurfaceProfile {
                profile_id: VIEWER_SURFACE_WEB.to_string(),
                reader_mode_default: false,
                smooth_scroll_enabled: true,
                zoom_step: 1.1,
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        registry.register(
            VIEWER_SURFACE_DOCUMENT,
            ViewerSurfaceProfile {
                profile_id: VIEWER_SURFACE_DOCUMENT.to_string(),
                reader_mode_default: true,
                smooth_scroll_enabled: true,
                zoom_step: 1.05,
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        registry.register(
            VIEWER_SURFACE_EMBEDDED,
            ViewerSurfaceProfile {
                profile_id: VIEWER_SURFACE_EMBEDDED.to_string(),
                reader_mode_default: false,
                smooth_scroll_enabled: false,
                zoom_step: 1.05,
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        registry.register(
            VIEWER_SURFACE_NATIVE_OVERLAY,
            ViewerSurfaceProfile {
                profile_id: VIEWER_SURFACE_NATIVE_OVERLAY.to_string(),
                reader_mode_default: false,
                smooth_scroll_enabled: false,
                zoom_step: 1.0,
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        registry
    }
}

fn profile_id_for_viewer(viewer_id: &str, capability: Option<&ViewerCapability>) -> &'static str {
    if matches!(
        viewer_id,
        "viewer:pdf" | "viewer:markdown" | "viewer:middlenet"
    ) {
        return VIEWER_SURFACE_DOCUMENT;
    }

    match capability.map(|cap| cap.render_mode) {
        Some(ViewerRenderMode::CompositedTexture) => VIEWER_SURFACE_WEB,
        Some(ViewerRenderMode::NativeOverlay) => VIEWER_SURFACE_NATIVE_OVERLAY,
        Some(ViewerRenderMode::EmbeddedHost) => VIEWER_SURFACE_EMBEDDED,
        Some(ViewerRenderMode::Placeholder) | None => VIEWER_SURFACE_DEFAULT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::domain::layout::ConformanceLevel;

    #[test]
    fn viewer_surface_registry_falls_back_for_unknown_profile() {
        let registry = ViewerSurfaceRegistry::default();
        let resolution = registry.resolve("viewer_surface:unknown");
        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, VIEWER_SURFACE_DEFAULT);
    }

    #[test]
    fn viewer_surface_resolution_round_trips_via_json() {
        let registry = ViewerSurfaceRegistry::default();
        let resolution = registry.resolve(VIEWER_SURFACE_DEFAULT);

        let json = serde_json::to_string(&resolution).expect("resolution should serialize");
        let restored: ViewerSurfaceResolution =
            serde_json::from_str(&json).expect("resolution should deserialize");

        assert_eq!(restored.resolved_id, VIEWER_SURFACE_DEFAULT);
        assert_eq!(
            restored.profile.subsystems.history.level,
            ConformanceLevel::Full
        );
    }

    #[test]
    fn viewer_surface_registry_maps_webview_to_web_profile() {
        let registry = ViewerSurfaceRegistry::default();
        let capability = ViewerCapability {
            viewer_id: "viewer:webview".to_string(),
            supported_mime_types: vec!["text/html".to_string()],
            supported_extensions: vec!["html".to_string()],
            render_mode: ViewerRenderMode::CompositedTexture,
            overlay_affordance: true,
            subsystems: crate::registries::atomic::viewer::ViewerSubsystemCapabilities::full(),
        };

        let resolution = registry.resolve_for_viewer("viewer:webview", Some(&capability));
        assert_eq!(resolution.resolved_id, VIEWER_SURFACE_WEB);
        assert!(resolution.profile.smooth_scroll_enabled);
    }

    #[test]
    fn viewer_surface_registry_maps_markdown_to_document_profile() {
        let registry = ViewerSurfaceRegistry::default();
        let capability = ViewerCapability {
            viewer_id: "viewer:markdown".to_string(),
            supported_mime_types: vec!["text/markdown".to_string()],
            supported_extensions: vec!["md".to_string()],
            render_mode: ViewerRenderMode::EmbeddedHost,
            overlay_affordance: true,
            subsystems: crate::registries::atomic::viewer::ViewerSubsystemCapabilities::full(),
        };

        let resolution = registry.resolve_for_viewer("viewer:markdown", Some(&capability));
        assert_eq!(resolution.resolved_id, VIEWER_SURFACE_DOCUMENT);
        assert!(resolution.profile.reader_mode_default);
    }

    #[test]
    fn viewer_surface_registry_maps_middlenet_to_document_profile() {
        let registry = ViewerSurfaceRegistry::default();
        let capability = ViewerCapability {
            viewer_id: "viewer:middlenet".to_string(),
            supported_mime_types: vec!["text/gemini".to_string()],
            supported_extensions: vec!["gmi".to_string()],
            render_mode: ViewerRenderMode::EmbeddedHost,
            overlay_affordance: true,
            subsystems: crate::registries::atomic::viewer::ViewerSubsystemCapabilities::full(),
        };

        let resolution = registry.resolve_for_viewer("viewer:middlenet", Some(&capability));
        assert_eq!(resolution.resolved_id, VIEWER_SURFACE_DOCUMENT);
        assert!(resolution.profile.reader_mode_default);
    }
}
