use std::collections::HashMap;

use crate::registries::domain::layout::CapabilityDeclaration;
use crate::util::VersoAddress;

pub(crate) const VIEWER_ID_FALLBACK: &str = "viewer:webview";

// ---------------------------------------------------------------------------
// EmbeddedViewer — trait-dispatched rendering for non-composited viewers
// ---------------------------------------------------------------------------

/// Outcome of a single `EmbeddedViewer::render` call.
///
/// Viewers that need to emit graph intents (e.g. `NavigateTo` from a markdown
/// link, or `SetNodeUrl` from a directory click) return them here so the tile
/// behavior can queue them without the viewer holding a mutable reference to
/// `GraphBrowserApp`.
pub(crate) struct EmbeddedViewerOutput {
    pub(crate) intents: Vec<crate::app::GraphIntent>,
}

impl EmbeddedViewerOutput {
    pub(crate) fn empty() -> Self {
        Self {
            intents: Vec::new(),
        }
    }
}

/// Read-only rendering context passed to each `EmbeddedViewer::render` call.
pub(crate) struct EmbeddedViewerContext<'a> {
    pub(crate) node_key: crate::graph::NodeKey,
    pub(crate) node_url: &'a str,
    pub(crate) mime_hint: Option<&'a str>,
}

/// Trait for viewers that render directly into an egui `Ui`.
///
/// Each concrete viewer owns its own per-node state (cached directory listings,
/// async image decode handles, etc.) and is dispatched through the
/// `EmbeddedViewerRegistry` rather than an inline `if/else` chain.
pub(crate) trait EmbeddedViewer {
    fn viewer_id(&self) -> &'static str;
    fn render(
        &self,
        ui: &mut egui::Ui,
        ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput;
}

/// Registry mapping viewer IDs to concrete `EmbeddedViewer` trait objects.
pub(crate) struct EmbeddedViewerRegistry {
    viewers: HashMap<&'static str, Box<dyn EmbeddedViewer + Send + Sync>>,
}

impl EmbeddedViewerRegistry {
    pub(crate) fn new() -> Self {
        Self {
            viewers: HashMap::new(),
        }
    }

    pub(crate) fn register(&mut self, viewer: Box<dyn EmbeddedViewer + Send + Sync>) {
        let id = viewer.viewer_id();
        self.viewers.insert(id, viewer);
    }

    pub(crate) fn get(&self, viewer_id: &str) -> Option<&(dyn EmbeddedViewer + Send + Sync)> {
        self.viewers.get(viewer_id).map(|v| v.as_ref())
    }

    /// Build the default registry with all built-in embedded viewers.
    pub(crate) fn default_with_viewers() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(SettingsViewer));
        registry.register(Box::new(super::super::viewers::PlaintextEmbeddedViewer));
        registry.register(Box::new(super::super::viewers::ImageEmbeddedViewer));
        registry.register(Box::new(super::super::viewers::DirectoryEmbeddedViewer));
        registry.register(Box::new(FallbackViewer));
        registry
    }
}

/// Settings viewer — delegates to the settings/history render surfaces.
struct SettingsViewer;
impl EmbeddedViewer for SettingsViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:settings"
    }
    fn render(
        &self,
        _ui: &mut egui::Ui,
        _ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput {
        // Settings rendering requires access to GraphBrowserApp and is handled
        // specially in tile_behavior dispatch; this trait impl exists so the
        // viewer ID is recognized by the registry.
        EmbeddedViewerOutput::empty()
    }
}

/// Fallback / metadata viewer — shown when no dedicated viewer is registered.
struct FallbackViewer;
impl EmbeddedViewer for FallbackViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:fallback"
    }
    fn render(
        &self,
        ui: &mut egui::Ui,
        ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput {
        ui.colored_label(
            egui::Color32::from_rgb(220, 180, 60),
            "No dedicated viewer is available for this content yet.",
        );
        ui.label(format!("URL: {}", ctx.node_url));
        if let Some(mime_hint) = ctx.mime_hint {
            ui.small(format!("Detected content type: {mime_hint}"));
        } else {
            ui.small("Detected content type: unknown");
        }
        ui.small(
            "Recovery: switch to WebView for compatibility content, or keep this node as a graph-backed placeholder until a native viewer lands.",
        );
        EmbeddedViewerOutput::empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ViewerRenderMode {
    CompositedTexture,
    NativeOverlay,
    EmbeddedEgui,
    Placeholder,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ViewerCapability {
    pub(crate) viewer_id: String,
    pub(crate) supported_mime_types: Vec<String>,
    pub(crate) supported_extensions: Vec<String>,
    pub(crate) render_mode: ViewerRenderMode,
    pub(crate) overlay_affordance: bool,
    #[serde(flatten)]
    pub(crate) subsystems: ViewerSubsystemCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ViewerSubsystemCapabilities {
    pub(crate) accessibility: CapabilityDeclaration,
    pub(crate) security: CapabilityDeclaration,
    pub(crate) storage: CapabilityDeclaration,
    pub(crate) history: CapabilityDeclaration,
}

impl ViewerSubsystemCapabilities {
    pub(crate) fn full() -> Self {
        Self {
            accessibility: CapabilityDeclaration::full(),
            security: CapabilityDeclaration::full(),
            storage: CapabilityDeclaration::full(),
            history: CapabilityDeclaration::full(),
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
    ) -> Option<ViewerSubsystemCapabilities> {
        self.capabilities.insert(viewer_id, capabilities)
    }

    pub(crate) fn capabilities_for(&self, viewer_id: &'static str) -> ViewerSubsystemCapabilities {
        self.capabilities
            .get(viewer_id)
            .cloned()
            .unwrap_or_else(ViewerSubsystemCapabilities::full)
    }

    pub(crate) fn describe_viewer(&self, viewer_id: &str) -> Option<ViewerCapability> {
        let normalized = viewer_id.trim();
        if normalized.is_empty() {
            return None;
        }

        let known = self.capabilities.contains_key(normalized)
            || self
                .mime_handlers
                .values()
                .any(|registered| *registered == normalized)
            || self
                .extension_handlers
                .values()
                .any(|registered| *registered == normalized);
        if !known {
            return None;
        }

        let mut supported_mime_types = self
            .mime_handlers
            .iter()
            .filter_map(|(mime, registered)| (*registered == normalized).then_some(mime.clone()))
            .collect::<Vec<_>>();
        supported_mime_types.sort();
        supported_mime_types.dedup();

        let mut supported_extensions = self
            .extension_handlers
            .iter()
            .filter_map(|(extension, registered)| {
                (*registered == normalized).then_some(extension.clone())
            })
            .collect::<Vec<_>>();
        supported_extensions.sort();
        supported_extensions.dedup();

        Some(ViewerCapability {
            viewer_id: normalized.to_string(),
            supported_mime_types,
            supported_extensions,
            render_mode: render_mode_for_viewer_id(normalized),
            overlay_affordance: overlay_affordance_for_viewer_id(normalized),
            subsystems: self
                .capabilities
                .get(normalized)
                .cloned()
                .unwrap_or_else(ViewerSubsystemCapabilities::full),
        })
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

    pub(crate) fn register_mime(
        &mut self,
        mime: &str,
        viewer_id: &'static str,
    ) -> Option<&'static str> {
        self.mime_handlers
            .insert(mime.to_ascii_lowercase(), viewer_id)
    }

    pub(crate) fn unregister_mime(&mut self, mime: &str) -> Option<&'static str> {
        self.mime_handlers.remove(&mime.to_ascii_lowercase())
    }

    pub(crate) fn register_extension(
        &mut self,
        extension: &str,
        viewer_id: &'static str,
    ) -> Option<&'static str> {
        self.extension_handlers
            .insert(extension.to_ascii_lowercase(), viewer_id)
    }

    pub(crate) fn unregister_extension(&mut self, extension: &str) -> Option<&'static str> {
        self.extension_handlers
            .remove(&extension.to_ascii_lowercase())
    }

    pub(crate) fn unregister_capabilities(
        &mut self,
        viewer_id: &'static str,
    ) -> Option<ViewerSubsystemCapabilities> {
        self.capabilities.remove(viewer_id)
    }

    pub(crate) fn select_for_uri(&self, uri: &str, mime_hint: Option<&str>) -> ViewerSelection {
        if let Some(address) = VersoAddress::parse(uri) {
            if address.is_settings() {
                return self.selection("viewer:settings", false, "internal");
            }

            if let Some(viewer_id) = self.mime_handlers.get(address.inferred_mime_hint()) {
                return self.selection(viewer_id, false, "internal");
            }
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

        // Magic-byte fallback for local files when no MIME hint and no extension match.
        if mime_hint.is_none() {
            if let crate::graph::AddressKind::File = crate::graph::address_kind_from_url(uri) {
                if let Ok(path) = crate::shell::desktop::workbench::tile_behavior::file_path_from_node_url(uri) {
                    if let Ok(mut file) = std::fs::File::open(&path) {
                        let mut buf = [0u8; 512];
                        let n = std::io::Read::read(&mut file, &mut buf).unwrap_or(0);
                        if let Some(kind) = infer::get(&buf[..n]) {
                            let detected_mime = kind.mime_type().to_ascii_lowercase();
                            if let Some(viewer_id) = self.mime_handlers.get(&detected_mime) {
                                return self.selection(viewer_id, false, "magic");
                            }
                        }
                    }
                }
            }
        }

        // For non-HTTP address kinds (local files, custom schemes), avoid falling
        // back to the web renderer. Use plaintext only if the configured fallback
        // is the composited viewer; otherwise respect the registry's own fallback.
        let fallback = match crate::graph::address_kind_from_url(uri) {
            crate::graph::AddressKind::File | crate::graph::AddressKind::Unknown
                if self.fallback_viewer_id == "viewer:webview" =>
            {
                "viewer:plaintext"
            }
            _ => self.fallback_viewer_id,
        };
        self.selection(fallback, true, "fallback")
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
            // HTTP/HTTPS: use the registry's configured default (normally viewer:webview).
            crate::graph::AddressKind::Http => self.fallback_viewer_id,
            // Local files and unknown/non-web schemes: plaintext is the safe fallback.
            crate::graph::AddressKind::File
            | crate::graph::AddressKind::Unknown
            | crate::graph::AddressKind::Data
            | crate::graph::AddressKind::GraphshellClip
            | crate::graph::AddressKind::Directory => "viewer:plaintext",
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
        let mut registry = Self::new(VIEWER_ID_FALLBACK);
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
                accessibility: CapabilityDeclaration::full(),
                security: CapabilityDeclaration::full(),
                storage: CapabilityDeclaration::full(),
                history: CapabilityDeclaration::full(),
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

fn render_mode_for_viewer_id(viewer_id: &str) -> ViewerRenderMode {
    match viewer_id {
        "viewer:webview" => ViewerRenderMode::CompositedTexture,
        "viewer:wry" => ViewerRenderMode::NativeOverlay,
        "viewer:plaintext" | "viewer:markdown" | "viewer:pdf" | "viewer:csv"
        | "viewer:settings" | "viewer:metadata" => ViewerRenderMode::EmbeddedEgui,
        _ => ViewerRenderMode::Placeholder,
    }
}

fn overlay_affordance_for_viewer_id(viewer_id: &str) -> bool {
    !matches!(
        render_mode_for_viewer_id(viewer_id),
        ViewerRenderMode::Placeholder
    )
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
            extract_extension(&descriptor.uri)
                .map(|e| e.to_ascii_lowercase())
                .as_deref(),
            Some(
                "txt"
                    | "md"
                    | "rs"
                    | "py"
                    | "js"
                    | "ts"
                    | "json"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "html"
                    | "css"
                    | "sh"
                    | "bash"
                    | "zsh"
                    | "fish"
                    | "csv"
                    | "xml"
                    | "log"
                    | "ini"
                    | "cfg"
                    | "conf"
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::AddressKind;
    use crate::registries::domain::layout::ConformanceLevel;
    use crate::util::{GraphshellSettingsPath, VersoAddress};

    #[test]
    fn viewer_registry_selects_internal_settings_viewer_for_graphshell_settings_url() {
        let registry = ViewerRegistry::default();
        let selection = registry.select_for_uri(
            &VersoAddress::settings(GraphshellSettingsPath::History).to_string(),
            None,
        );

        assert_eq!(selection.viewer_id, "viewer:settings");
        assert!(!selection.fallback_used);
        assert_eq!(selection.matched_by, "internal");
        assert_eq!(
            selection.capabilities.accessibility.level,
            ConformanceLevel::Full
        );
    }

    #[test]
    fn viewer_registry_uses_internal_mime_for_graphshell_frame_route() {
        let registry = ViewerRegistry::default();
        let selection = registry.select_for_uri(
            &VersoAddress::frame("frame-123").to_string(),
            Some("application/x-graphshell-internal"),
        );

        assert_eq!(selection.viewer_id, "viewer:webview");
        assert!(!selection.fallback_used);
        assert_eq!(selection.matched_by, "internal");
    }

    #[test]
    fn viewer_registry_selects_internal_viewer_for_graphshell_frame_route_without_mime_hint() {
        let registry = ViewerRegistry::default();
        let selection =
            registry.select_for_uri(&VersoAddress::frame("frame-123").to_string(), None);

        assert_eq!(selection.viewer_id, "viewer:webview");
        assert!(!selection.fallback_used);
        assert_eq!(selection.matched_by, "internal");
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

        assert_eq!(fallback.capabilities.history.level, ConformanceLevel::Full);
    }

    #[test]
    fn viewer_registry_reports_registered_capabilities_in_selection() {
        let mut registry = ViewerRegistry::new("viewer:fallback");
        registry.register_mime("text/plain", "viewer:plaintext");
        registry.register_capabilities(
            "viewer:plaintext",
            ViewerSubsystemCapabilities {
                accessibility: CapabilityDeclaration::partial("access bridge disabled in test"),
                security: CapabilityDeclaration::full(),
                storage: CapabilityDeclaration::full(),
                history: CapabilityDeclaration::full(),
            },
        );

        let selection = registry.select_for_uri("file:///notes/readme.txt", Some("text/plain"));
        assert_eq!(selection.viewer_id, "viewer:plaintext");
        assert_eq!(
            selection.capabilities.accessibility.level,
            ConformanceLevel::Partial
        );
        assert_eq!(
            selection.capabilities.accessibility.reason.as_deref(),
            Some("access bridge disabled in test")
        );
    }

    #[test]
    fn viewer_capabilities_round_trip_via_json() {
        let capabilities = ViewerSubsystemCapabilities {
            accessibility: CapabilityDeclaration::partial("access bridge degraded"),
            security: CapabilityDeclaration::full(),
            storage: CapabilityDeclaration::full(),
            history: CapabilityDeclaration::none("history replay unavailable"),
        };

        let json = serde_json::to_string(&capabilities).expect("capabilities should serialize");
        let restored: ViewerSubsystemCapabilities =
            serde_json::from_str(&json).expect("capabilities should deserialize");

        assert_eq!(restored.accessibility.level, ConformanceLevel::Partial);
        assert_eq!(
            restored.accessibility.reason.as_deref(),
            Some("access bridge degraded")
        );
        assert_eq!(restored.history.level, ConformanceLevel::None);
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
    fn select_for_unknown_scheme_no_mime_routes_to_plaintext_fallback() {
        let registry = ViewerRegistry::default();
        assert_eq!(
            registry.select_for(None, AddressKind::Unknown),
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

    #[test]
    fn describe_viewer_returns_capability_payload_for_registered_viewer() {
        let registry = ViewerRegistry::default();
        let capability = registry
            .describe_viewer("viewer:webview")
            .expect("viewer:webview should be described");

        assert_eq!(capability.viewer_id, "viewer:webview");
        assert_eq!(capability.render_mode, ViewerRenderMode::CompositedTexture);
        assert_eq!(
            capability.subsystems.accessibility.level,
            ConformanceLevel::Full
        );
        assert_eq!(capability.subsystems.accessibility.reason, None);
        assert!(capability.overlay_affordance);
        assert!(
            capability
                .supported_mime_types
                .iter()
                .any(|mime| mime == "text/html")
        );
    }

    #[test]
    fn describe_viewer_returns_none_for_unknown_viewer() {
        let registry = ViewerRegistry::default();
        assert!(registry.describe_viewer("viewer:unknown").is_none());
    }

    #[test]
    fn select_for_unknown_mime_uses_canonical_runtime_fallback() {
        let registry = ViewerRegistry::default();
        let selection = registry.select_for_uri("https://example.com/file.bin", Some(""));

        assert_eq!(selection.viewer_id, VIEWER_ID_FALLBACK);
        assert!(selection.fallback_used);
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
