use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewerSelection {
    pub(crate) viewer_id: &'static str,
    pub(crate) fallback_used: bool,
    pub(crate) matched_by: &'static str,
}

#[derive(Debug, Clone)]
pub(crate) struct ViewerRegistry {
    mime_handlers: HashMap<String, &'static str>,
    extension_handlers: HashMap<String, &'static str>,
    fallback_viewer_id: &'static str,
}

impl ViewerRegistry {
    pub(crate) fn new(fallback_viewer_id: &'static str) -> Self {
        Self {
            mime_handlers: HashMap::new(),
            extension_handlers: HashMap::new(),
            fallback_viewer_id,
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
        if let Some(mime) = mime_hint.map(|m| m.to_ascii_lowercase())
            && let Some(viewer_id) = self.mime_handlers.get(&mime)
        {
            return ViewerSelection {
                viewer_id,
                fallback_used: false,
                matched_by: "mime",
            };
        }

        if let Some(ext) = extract_extension(uri)
            && let Some(viewer_id) = self.extension_handlers.get(ext)
        {
            return ViewerSelection {
                viewer_id,
                fallback_used: false,
                matched_by: "extension",
            };
        }

        ViewerSelection {
            viewer_id: self.fallback_viewer_id,
            fallback_used: true,
            matched_by: "fallback",
        }
    }
}

impl Default for ViewerRegistry {
    fn default() -> Self {
        let mut registry = Self::new("viewer:webview");
        registry.register_mime("text/html", "viewer:webview");
        registry.register_mime("text/markdown", "viewer:markdown");
        registry.register_mime("application/pdf", "viewer:pdf");
        registry.register_mime("text/csv", "viewer:csv");
        registry.register_extension("md", "viewer:markdown");
        registry.register_extension("pdf", "viewer:pdf");
        registry.register_extension("csv", "viewer:csv");
        registry
    }
}

fn extract_extension(uri: &str) -> Option<&str> {
    let no_fragment = uri.split('#').next().unwrap_or(uri);
    let no_query = no_fragment.split('?').next().unwrap_or(no_fragment);
    no_query.rsplit_once('.').map(|(_, ext)| ext)
}
