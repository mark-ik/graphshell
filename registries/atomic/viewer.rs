use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ViewerConformanceLevel {
    Full,
    Partial,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ViewerCapabilityDeclaration {
    pub(crate) level: ViewerConformanceLevel,
    pub(crate) reason: Option<String>,
}

impl ViewerCapabilityDeclaration {
    pub(crate) fn full() -> Self {
        Self {
            level: ViewerConformanceLevel::Full,
            reason: None,
        }
    }

    pub(crate) fn partial(reason: impl Into<String>) -> Self {
        Self {
            level: ViewerConformanceLevel::Partial,
            reason: Some(reason.into()),
        }
    }

    pub(crate) fn none(reason: impl Into<String>) -> Self {
        Self {
            level: ViewerConformanceLevel::None,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ViewerSubsystemCapabilities {
    pub(crate) accessibility: ViewerCapabilityDeclaration,
    pub(crate) security: ViewerCapabilityDeclaration,
    pub(crate) storage: ViewerCapabilityDeclaration,
    pub(crate) history: ViewerCapabilityDeclaration,
}

impl ViewerSubsystemCapabilities {
    pub(crate) fn full() -> Self {
        Self {
            accessibility: ViewerCapabilityDeclaration::full(),
            security: ViewerCapabilityDeclaration::full(),
            storage: ViewerCapabilityDeclaration::full(),
            history: ViewerCapabilityDeclaration::full(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ViewerDescriptor {
    pub(crate) uri: String,
    pub(crate) mime_hint: Option<String>,
}

pub(crate) trait ViewerHandler: Send + Sync {
    fn viewer_id(&self) -> &'static str;
    fn can_render(&self, descriptor: &ViewerDescriptor) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ViewerSelection {
    pub(crate) viewer_id: &'static str,
    pub(crate) fallback_used: bool,
    pub(crate) matched_by: &'static str,
    pub(crate) capabilities: ViewerSubsystemCapabilities,
}

#[derive(Debug, Clone)]
pub(crate) struct ViewerRegistry {
    mime_handlers: HashMap<String, &'static str>,
    extension_handlers: HashMap<String, &'static str>,
    capabilities: HashMap<&'static str, ViewerSubsystemCapabilities>,
    fallback_viewer_id: &'static str,
}

impl ViewerRegistry {
    pub(crate) fn new(fallback_viewer_id: &'static str) -> Self {
        Self {
            mime_handlers: HashMap::new(),
            extension_handlers: HashMap::new(),
            capabilities: HashMap::new(),
            fallback_viewer_id,
        }
    }

    pub(crate) fn register_capabilities(
        &mut self,
        viewer_id: &'static str,
        capabilities: ViewerSubsystemCapabilities,
    ) {
        self.capabilities.insert(viewer_id, capabilities);
    }

    pub(crate) fn capabilities_for(&self, viewer_id: &'static str) -> ViewerSubsystemCapabilities {
        self.capabilities
            .get(viewer_id)
            .cloned()
            .unwrap_or_else(ViewerSubsystemCapabilities::full)
    }

    fn selection(
        &self,
        viewer_id: &'static str,
        fallback_used: bool,
        matched_by: &'static str,
    ) -> ViewerSelection {
        ViewerSelection {
            viewer_id,
            fallback_used,
            matched_by,
            capabilities: self.capabilities_for(viewer_id),
        }
    }

    pub(crate) fn register_mime(&mut self, mime: &str, viewer_id: &'static str) {
        self.mime_handlers
            .insert(mime.to_ascii_lowercase(), viewer_id);
    }

    pub(crate) fn register_extension(&mut self, extension: &str, viewer_id: &'static str) {
        self.extension_handlers
            .insert(extension.to_ascii_lowercase(), viewer_id);
    }

    pub(crate) fn select_for_uri(&self, uri: &str, mime_hint: Option<&str>) -> ViewerSelection {
        if uri.eq_ignore_ascii_case("graphshell://settings")
            || uri
                .to_ascii_lowercase()
                .starts_with("graphshell://settings/")
        {
            return self.selection("viewer:settings", false, "internal");
        }

        if let Some(mime) = mime_hint.map(|m| m.to_ascii_lowercase())
            && let Some(viewer_id) = self.mime_handlers.get(&mime)
        {
            return self.selection(viewer_id, false, "mime");
        }

        if let Some(ext) = extract_extension(uri)
            && let Some(viewer_id) = self.extension_handlers.get(ext)
        {
            return self.selection(viewer_id, false, "extension");
        }

        self.selection(self.fallback_viewer_id, true, "fallback")
    }

    /// Select a viewer based on MIME hint and address kind.
    ///
    /// Selection order:
    /// 1. MIME-based lookup (highest priority when a hint is available).
    /// 2. Address-kind heuristic — `Http` falls back to the registry default (Servo webview);
    ///    `File` and `Custom` fall back to `viewer:plaintext` as a safe last resort.
    ///
    /// This method does **not** consult `viewer_id_override` or workspace defaults;
    /// those are the caller's responsibility and should be applied before calling this.
    pub(crate) fn select_for(
        &self,
        mime: Option<&str>,
        kind: crate::graph::AddressKind,
    ) -> &'static str {
        // 1. MIME-based lookup.
        if let Some(mime_val) = mime.map(|m| m.to_ascii_lowercase())
            && let Some(viewer_id) = self.mime_handlers.get(&mime_val)
        {
            return viewer_id;
        }

        // 2. Address-kind heuristic fallback.
        match kind {
            // HTTP/HTTPS: use the registry's configured default (normally viewer:servo/webview).
            crate::graph::AddressKind::Http => self.fallback_viewer_id,
            // Local files and custom schemes: plaintext is the safe fallback.
            crate::graph::AddressKind::File | crate::graph::AddressKind::Custom => {
                "viewer:plaintext"
            },
        }
    }

    pub(crate) fn core_seed() -> Self {
        let mut registry = Self::new("viewer:metadata");
        registry.register_mime("text/plain", "viewer:plaintext");
        registry.register_mime("application/octet-stream", "viewer:metadata");
        registry.register_extension("txt", "viewer:plaintext");
        registry.register_capabilities("viewer:plaintext", ViewerSubsystemCapabilities::full());
        registry.register_capabilities("viewer:metadata", ViewerSubsystemCapabilities::full());
        registry
    }
}

impl Default for ViewerRegistry {
    fn default() -> Self {
        let mut registry = Self::new("viewer:webview");
        registry.register_mime("application/x-graphshell-settings", "viewer:settings");
        registry.register_mime("application/x-graphshell-internal", "viewer:webview");
        registry.register_mime("text/html", "viewer:webview");
        registry.register_mime("text/plain", "viewer:plaintext");
        registry.register_mime("text/markdown", "viewer:markdown");
        registry.register_mime("text/x-markdown", "viewer:markdown");
        registry.register_mime("application/json", "viewer:plaintext");
        registry.register_mime("application/toml", "viewer:plaintext");
        registry.register_mime("application/yaml", "viewer:plaintext");
        registry.register_mime("application/x-yaml", "viewer:plaintext");
        registry.register_mime("application/pdf", "viewer:pdf");
        registry.register_mime("text/csv", "viewer:csv");
        registry.register_extension("md", "viewer:markdown");
        registry.register_extension("pdf", "viewer:pdf");
        registry.register_extension("csv", "viewer:csv");
        registry.register_extension("txt", "viewer:plaintext");
        registry.register_extension("json", "viewer:plaintext");
        registry.register_extension("toml", "viewer:plaintext");
        registry.register_extension("yaml", "viewer:plaintext");
        registry.register_extension("yml", "viewer:plaintext");
        registry.register_extension("rs", "viewer:plaintext");
        registry.register_extension("py", "viewer:plaintext");
        registry.register_extension("js", "viewer:plaintext");
        registry.register_extension("ts", "viewer:plaintext");
        registry.register_capabilities(
            "viewer:webview",
            ViewerSubsystemCapabilities {
                accessibility: ViewerCapabilityDeclaration {
                    level: ViewerConformanceLevel::Partial,
                    reason: Some(
                        "WebView accessibility tree injection deferred due accesskit version mismatch"
                            .to_string(),
                    ),
                },
                security: ViewerCapabilityDeclaration::full(),
                storage: ViewerCapabilityDeclaration::full(),
                history: ViewerCapabilityDeclaration::full(),
            },
        );
        registry.register_capabilities("viewer:settings", ViewerSubsystemCapabilities::full());
        registry.register_capabilities("viewer:metadata", ViewerSubsystemCapabilities::full());
        registry.register_capabilities("viewer:plaintext", ViewerSubsystemCapabilities::full());
        registry.register_capabilities("viewer:markdown", ViewerSubsystemCapabilities::full());
        registry.register_capabilities("viewer:pdf", ViewerSubsystemCapabilities::full());
        registry.register_capabilities("viewer:csv", ViewerSubsystemCapabilities::full());
        registry
    }
}

fn extract_extension(uri: &str) -> Option<&str> {
    let no_fragment = uri.split('#').next().unwrap_or(uri);
    let no_query = no_fragment.split('?').next().unwrap_or(no_fragment);
    no_query.rsplit_once('.').map(|(_, ext)| ext)
}

/// Baseline plaintext viewer handler.
///
/// Handles all `text/*` MIME types plus common structured-text formats
/// (`application/json`, `application/toml`, `application/yaml`).
/// This is the last-resort embedded renderer for local files and custom-scheme
/// content — it always accepts rather than falling through to the web renderer.
pub(crate) struct PlaintextViewerHandler;

impl ViewerHandler for PlaintextViewerHandler {
    fn viewer_id(&self) -> &'static str {
        "viewer:plaintext"
    }

    fn can_render(&self, descriptor: &ViewerDescriptor) -> bool {
        if let Some(ref mime) = descriptor.mime_hint {
            let lower = mime.to_ascii_lowercase();
            return lower.starts_with("text/")
                || lower == "application/json"
                || lower == "application/toml"
                || lower == "application/yaml"
                || lower == "application/x-yaml";
        }
        // No MIME hint — check the URI extension.
        matches!(
            extract_extension(&descriptor.uri).map(|e| e.to_ascii_lowercase()).as_deref(),
            Some(
                "txt" | "md" | "rs" | "py" | "js" | "ts" | "json" | "toml" | "yaml" | "yml"
                    | "html" | "css" | "sh" | "bash" | "zsh" | "fish" | "csv" | "xml" | "log"
                    | "ini" | "cfg" | "conf"
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::AddressKind;

    #[test]
    fn viewer_registry_selects_internal_settings_viewer_for_graphshell_settings_url() {
        let registry = ViewerRegistry::default();
        let selection = registry.select_for_uri("graphshell://settings/history", None);

        assert_eq!(selection.viewer_id, "viewer:settings");
        assert!(!selection.fallback_used);
        assert_eq!(selection.matched_by, "internal");
        assert_eq!(
            selection.capabilities.accessibility.level,
            ViewerConformanceLevel::Full
        );
    }

    #[test]
    fn viewer_registry_core_seed_uses_plaintext_and_metadata() {
        let registry = ViewerRegistry::core_seed();

        let plaintext = registry.select_for_uri("file:///notes/readme.txt", Some("text/plain"));
        assert_eq!(plaintext.viewer_id, "viewer:plaintext");
        assert!(!plaintext.fallback_used);

        let fallback = registry.select_for_uri("file:///archive/blob.bin", None);
        assert_eq!(fallback.viewer_id, "viewer:metadata");
        assert!(fallback.fallback_used);

        assert_eq!(
            fallback.capabilities.history.level,
            ViewerConformanceLevel::Full
        );
    }

    #[test]
    fn viewer_registry_reports_registered_capabilities_in_selection() {
        let mut registry = ViewerRegistry::new("viewer:fallback");
        registry.register_mime("text/plain", "viewer:plaintext");
        registry.register_capabilities(
            "viewer:plaintext",
            ViewerSubsystemCapabilities {
                accessibility: ViewerCapabilityDeclaration {
                    level: ViewerConformanceLevel::Partial,
                    reason: Some("access bridge disabled in test".to_string()),
                },
                security: ViewerCapabilityDeclaration::full(),
                storage: ViewerCapabilityDeclaration::full(),
                history: ViewerCapabilityDeclaration::full(),
            },
        );

        let selection = registry.select_for_uri("file:///notes/readme.txt", Some("text/plain"));
        assert_eq!(selection.viewer_id, "viewer:plaintext");
        assert_eq!(
            selection.capabilities.accessibility.level,
            ViewerConformanceLevel::Partial
        );
        assert_eq!(
            selection.capabilities.accessibility.reason.as_deref(),
            Some("access bridge disabled in test")
        );
    }

    #[test]
    fn viewer_capabilities_round_trip_via_json() {
        let capabilities = ViewerSubsystemCapabilities {
            accessibility: ViewerCapabilityDeclaration {
                level: ViewerConformanceLevel::Partial,
                reason: Some("access bridge degraded".to_string()),
            },
            security: ViewerCapabilityDeclaration::full(),
            storage: ViewerCapabilityDeclaration::full(),
            history: ViewerCapabilityDeclaration::none("history replay unavailable"),
        };

        let json = serde_json::to_string(&capabilities).expect("capabilities should serialize");
        let restored: ViewerSubsystemCapabilities =
            serde_json::from_str(&json).expect("capabilities should deserialize");

        assert_eq!(restored.accessibility.level, ViewerConformanceLevel::Partial);
        assert_eq!(
            restored.accessibility.reason.as_deref(),
            Some("access bridge degraded")
        );
        assert_eq!(restored.history.level, ViewerConformanceLevel::None);
    }

    // --- select_for tests ---

    #[test]
    fn select_for_pdf_mime_routes_to_pdf_viewer() {
        let registry = ViewerRegistry::default();
        assert_eq!(
            registry.select_for(Some("application/pdf"), AddressKind::File),
            "viewer:pdf"
        );
    }

    #[test]
    fn select_for_text_plain_routes_to_plaintext_viewer() {
        let registry = ViewerRegistry::default();
        assert_eq!(
            registry.select_for(Some("text/plain"), AddressKind::File),
            "viewer:plaintext"
        );
    }

    #[test]
    fn select_for_http_no_mime_routes_to_webview_fallback() {
        let registry = ViewerRegistry::default();
        assert_eq!(
            registry.select_for(None, AddressKind::Http),
            "viewer:webview"
        );
    }

    #[test]
    fn select_for_file_no_mime_routes_to_plaintext_fallback() {
        let registry = ViewerRegistry::default();
        assert_eq!(
            registry.select_for(None, AddressKind::File),
            "viewer:plaintext"
        );
    }

    #[test]
    fn select_for_custom_scheme_no_mime_routes_to_plaintext_fallback() {
        let registry = ViewerRegistry::default();
        assert_eq!(
            registry.select_for(None, AddressKind::Custom),
            "viewer:plaintext"
        );
    }

    #[test]
    fn select_for_html_mime_routes_to_webview() {
        let registry = ViewerRegistry::default();
        assert_eq!(
            registry.select_for(Some("text/html"), AddressKind::Http),
            "viewer:webview"
        );
    }

    #[test]
    fn select_for_json_routes_to_plaintext() {
        let registry = ViewerRegistry::default();
        assert_eq!(
            registry.select_for(Some("application/json"), AddressKind::File),
            "viewer:plaintext"
        );
    }

    // --- PlaintextViewerHandler tests ---

    #[test]
    fn plaintext_handler_id_is_viewer_plaintext() {
        let handler = PlaintextViewerHandler;
        assert_eq!(handler.viewer_id(), "viewer:plaintext");
    }

    #[test]
    fn plaintext_handler_can_render_text_plain() {
        let handler = PlaintextViewerHandler;
        assert!(handler.can_render(&ViewerDescriptor {
            uri: "file:///foo.txt".to_string(),
            mime_hint: Some("text/plain".to_string()),
        }));
    }

    #[test]
    fn plaintext_handler_can_render_text_markdown() {
        let handler = PlaintextViewerHandler;
        assert!(handler.can_render(&ViewerDescriptor {
            uri: "file:///doc.md".to_string(),
            mime_hint: Some("text/markdown".to_string()),
        }));
    }

    #[test]
    fn plaintext_handler_can_render_application_json() {
        let handler = PlaintextViewerHandler;
        assert!(handler.can_render(&ViewerDescriptor {
            uri: "file:///data.json".to_string(),
            mime_hint: Some("application/json".to_string()),
        }));
    }

    #[test]
    fn plaintext_handler_can_render_rs_by_extension_without_mime() {
        let handler = PlaintextViewerHandler;
        assert!(handler.can_render(&ViewerDescriptor {
            uri: "file:///src/main.rs".to_string(),
            mime_hint: None,
        }));
    }

    #[test]
    fn plaintext_handler_cannot_render_binary_without_mime() {
        let handler = PlaintextViewerHandler;
        assert!(!handler.can_render(&ViewerDescriptor {
            uri: "file:///archive.zip".to_string(),
            mime_hint: None,
        }));
    }

    #[test]
    fn plaintext_handler_cannot_render_image_mime() {
        let handler = PlaintextViewerHandler;
        assert!(!handler.can_render(&ViewerDescriptor {
            uri: "file:///photo.png".to_string(),
            mime_hint: Some("image/png".to_string()),
        }));
    }
}
