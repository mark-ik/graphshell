use crate::shell::desktop::runtime::protocols::registry as scaffold;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtocolResolution {
    pub(crate) scheme: String,
    pub(crate) matched_scheme: String,
    pub(crate) supported: bool,
    pub(crate) fallback_used: bool,
    pub(crate) inferred_mime_hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ProtocolResolveControl {
    pub(crate) cancelled: bool,
}

impl ProtocolResolveControl {
    #[cfg(test)]
    pub(crate) fn cancelled() -> Self {
        Self { cancelled: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProtocolResolveOutcome {
    Resolved(ProtocolResolution),
    Cancelled,
}

struct RegistrySchemeHandler {
    scheme: String,
}

impl scaffold::ProtocolHandler for RegistrySchemeHandler {
    fn scheme(&self) -> &str {
        &self.scheme
    }

    fn resolve(&self, _uri: &str) -> scaffold::ProtocolResult {
        scaffold::ProtocolResult::Error("Phase0 registry handler does not fetch content".to_string())
    }

    fn capabilities(&self) -> scaffold::ProtocolCapabilities {
        scaffold::ProtocolCapabilities {
            supports_search: true,
            supports_caching: true,
            is_secure: self.scheme == "https",
        }
    }
}

pub(crate) struct ProtocolRegistry {
    scaffold: scaffold::ProtocolRegistry,
    fallback_scheme: String,
}

impl ProtocolRegistry {
    pub(crate) fn new(fallback_scheme: impl Into<String>) -> Self {
        Self {
            scaffold: scaffold::ProtocolRegistry::new(),
            fallback_scheme: fallback_scheme.into(),
        }
    }

    pub(crate) fn register_scheme(&mut self, scheme: &str) {
        let normalized = scheme.to_ascii_lowercase();
        self.scaffold
            .register(RegistrySchemeHandler { scheme: normalized });
    }

    pub(crate) fn resolve(&self, uri: &str) -> ProtocolResolution {
        let scheme = uri
            .split_once(':')
            .map(|(left, _)| left)
            .unwrap_or("")
            .to_ascii_lowercase();

        let inferred_mime_hint = infer_mime_hint(uri, &scheme);

        if self.scaffold.get(&scheme).is_some() {
            return ProtocolResolution {
                scheme: scheme.clone(),
                matched_scheme: scheme,
                supported: true,
                fallback_used: false,
                inferred_mime_hint,
            };
        }

        ProtocolResolution {
            scheme,
            matched_scheme: self.fallback_scheme.clone(),
            supported: false,
            fallback_used: true,
            inferred_mime_hint,
        }
    }

    pub(crate) fn resolve_with_control(
        &self,
        uri: &str,
        control: ProtocolResolveControl,
    ) -> ProtocolResolveOutcome {
        if control.cancelled {
            return ProtocolResolveOutcome::Cancelled;
        }

        ProtocolResolveOutcome::Resolved(self.resolve(uri))
    }
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        let mut registry = Self::new("https");
        for scheme in ["http", "https", "file", "about", "resource", "data", "graphshell"] {
            registry.register_scheme(scheme);
        }
        registry
    }
}

fn infer_mime_hint(uri: &str, scheme: &str) -> Option<String> {
    if scheme == "graphshell" {
        return infer_graphshell_mime_hint(uri);
    }

    if scheme == "data" {
        return infer_data_uri_mime_hint(uri);
    }

    let no_fragment = uri.split('#').next().unwrap_or(uri);
    let no_query = no_fragment.split('?').next().unwrap_or(no_fragment);
    let path = no_query.split_once(':').map(|(_, tail)| tail).unwrap_or(no_query);
    let trimmed = path.trim_start_matches('/');

    if trimmed.is_empty() {
        return None;
    }

    let guessed = mime_guess::from_path(trimmed).first_raw()?;
    Some(guessed.to_ascii_lowercase())
}

fn infer_data_uri_mime_hint(uri: &str) -> Option<String> {
    let metadata = uri.strip_prefix("data:")?.split_once(',')?.0;
    if metadata.is_empty() {
        return Some("text/plain".to_string());
    }

    let media_type = metadata
        .split(';')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("text/plain");

    Some(media_type.to_ascii_lowercase())
}

fn infer_graphshell_mime_hint(uri: &str) -> Option<String> {
    let tail = uri
        .strip_prefix("graphshell://")
        .unwrap_or(uri)
        .to_ascii_lowercase();

    if tail == "settings" || tail.starts_with("settings/") {
        return Some("application/x-graphshell-settings".to_string());
    }

    Some("application/x-graphshell-internal".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_resolution_returns_cancelled_outcome_when_control_cancelled() {
        let registry = ProtocolRegistry::default();
        let outcome =
            registry.resolve_with_control("https://example.com/path", ProtocolResolveControl::cancelled());

        assert_eq!(outcome, ProtocolResolveOutcome::Cancelled);
    }

    #[test]
    fn protocol_resolution_with_active_control_matches_standard_resolution() {
        let registry = ProtocolRegistry::default();
        let baseline = registry.resolve("https://example.com/path/readme.md");
        let outcome =
            registry.resolve_with_control("https://example.com/path/readme.md", ProtocolResolveControl::default());

        assert_eq!(outcome, ProtocolResolveOutcome::Resolved(baseline));
    }

    #[test]
    fn protocol_resolution_infers_data_uri_mime_hint() {
        let registry = ProtocolRegistry::default();
        let resolution = registry.resolve("data:text/csv,foo,bar");
        assert_eq!(resolution.inferred_mime_hint.as_deref(), Some("text/csv"));
        assert!(resolution.supported);
    }

    #[test]
    fn protocol_resolution_infers_file_extension_mime_hint() {
        let registry = ProtocolRegistry::default();
        let resolution = registry.resolve("https://example.com/path/report.pdf");
        assert_eq!(resolution.inferred_mime_hint.as_deref(), Some("application/pdf"));
        assert!(resolution.supported);
    }

    #[test]
    fn protocol_resolution_supports_graphshell_scheme_with_settings_hint() {
        let registry = ProtocolRegistry::default();
        let resolution = registry.resolve("graphshell://settings/history");

        assert!(resolution.supported);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.matched_scheme, "graphshell");
        assert_eq!(
            resolution.inferred_mime_hint.as_deref(),
            Some("application/x-graphshell-settings")
        );
    }
}
