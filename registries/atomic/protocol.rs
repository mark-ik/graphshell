use std::collections::HashMap;

use http::Uri;
use tower::Service;

pub(crate) type ContentStream = Box<dyn std::io::Read + Send>;
pub(crate) type ProtocolError = String;

pub(crate) trait ProtocolHandler:
    Service<Uri, Response = ContentStream, Error = ProtocolError> + Send
{
}

impl<T> ProtocolHandler for T where
    T: Service<Uri, Response = ContentStream, Error = ProtocolError> + Send
{
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtocolContractResolution {
    pub(crate) requested_scheme: String,
    pub(crate) resolved_scheme: String,
    pub(crate) fallback_used: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ProtocolContractRegistry {
    handlers: HashMap<String, &'static str>,
    fallback_scheme: String,
}

impl ProtocolContractRegistry {
    pub(crate) fn new(fallback_scheme: impl Into<String>) -> Self {
        Self {
            handlers: HashMap::new(),
            fallback_scheme: fallback_scheme.into(),
        }
    }

    pub(crate) fn register_scheme(&mut self, scheme: &str, handler_id: &'static str) {
        self.handlers.insert(scheme.to_ascii_lowercase(), handler_id);
    }

    pub(crate) fn has_scheme(&self, scheme: &str) -> bool {
        self.handlers.contains_key(&scheme.to_ascii_lowercase())
    }

    pub(crate) fn scheme_ids(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    pub(crate) fn resolve_scheme(&self, uri: &str) -> ProtocolContractResolution {
        let requested_scheme = uri
            .split_once(':')
            .map(|(left, _)| left)
            .unwrap_or("")
            .to_ascii_lowercase();

        if self.has_scheme(&requested_scheme) {
            return ProtocolContractResolution {
                requested_scheme: requested_scheme.clone(),
                resolved_scheme: requested_scheme,
                fallback_used: false,
            };
        }

        ProtocolContractResolution {
            requested_scheme,
            resolved_scheme: self.fallback_scheme.clone(),
            fallback_used: true,
        }
    }

    pub(crate) fn core_seed() -> Self {
        let mut registry = Self::new("about");
        registry.register_scheme("file", "protocol:file");
        registry.register_scheme("about", "protocol:about");
        registry
    }
}

impl Default for ProtocolContractRegistry {
    fn default() -> Self {
        Self::core_seed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_seed_contains_file_and_about() {
        let registry = ProtocolContractRegistry::core_seed();
        assert!(registry.has_scheme("file"));
        assert!(registry.has_scheme("about"));
        assert!(!registry.has_scheme("https"));
    }

    #[test]
    fn resolves_unknown_scheme_to_fallback() {
        let registry = ProtocolContractRegistry::core_seed();
        let resolution = registry.resolve_scheme("https://example.com");
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_scheme, "about");
    }
}
