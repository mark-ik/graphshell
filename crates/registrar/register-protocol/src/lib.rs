/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Protocol contract registry — the canonical late-binding map of
//! URI schemes to handler IDs.
//!
//! This crate is the proof-of-concept for the per-registry split
//! described in
//! `design_docs/graphshell_docs/implementation_strategy/2026-05-01_workspace_architecture_proposal.md`.
//! It was extracted from `registries/atomic/protocol.rs` because that
//! file passed every §3.3 criterion — keyed namespace, value-type
//! entries (`&'static str` handler IDs), `resolve_scheme` lookup, and
//! late binding via `register_scheme()` — with the smallest external
//! dependency surface (only `http` + `tower` + `std`).
//!
//! ## Visibility
//!
//! The original visibility (`pub(crate)` everywhere) was lifted to
//! `pub` on extraction so external callers (the rest of Graphshell, and
//! eventually third parties) can use the registry through the crate
//! boundary. The Slice-49b template applies: this is the deliberate
//! API-surface widening called out in the proposal §6.

use std::collections::HashMap;

use http::Uri;
use tower::Service;

pub type ContentStream = Box<dyn std::io::Read + Send>;
pub type ProtocolError = String;

/// Marker trait for protocol handlers — any `tower::Service<Uri,
/// Response = ContentStream, Error = ProtocolError>` qualifies.
pub trait ProtocolHandler:
    Service<Uri, Response = ContentStream, Error = ProtocolError> + Send
{
}

impl<T> ProtocolHandler for T where
    T: Service<Uri, Response = ContentStream, Error = ProtocolError> + Send
{
}

/// Result of resolving a URI's scheme through the registry. Records
/// the requested scheme verbatim plus the scheme the registry
/// resolved to (the requested one if registered, the fallback
/// otherwise).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolContractResolution {
    pub requested_scheme: String,
    pub resolved_scheme: String,
    pub fallback_used: bool,
}

/// Late-binding map of URI schemes (lowercase) to handler IDs.
/// Multiple call sites (mods, feature gates, host bring-up code) can
/// `register_scheme` entries without modifying this crate; lookups
/// either hit a registered scheme or fall back to the configured
/// `fallback_scheme`.
#[derive(Debug, Clone)]
pub struct ProtocolContractRegistry {
    handlers: HashMap<String, &'static str>,
    fallback_scheme: String,
}

impl ProtocolContractRegistry {
    pub fn new(fallback_scheme: impl Into<String>) -> Self {
        Self {
            handlers: HashMap::new(),
            fallback_scheme: fallback_scheme.into(),
        }
    }

    pub fn register_scheme(&mut self, scheme: &str, handler_id: &'static str) {
        self.handlers
            .insert(scheme.to_ascii_lowercase(), handler_id);
    }

    pub fn has_scheme(&self, scheme: &str) -> bool {
        self.handlers.contains_key(&scheme.to_ascii_lowercase())
    }

    pub fn scheme_ids(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    pub fn resolve_scheme(&self, uri: &str) -> ProtocolContractResolution {
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

    /// Default seed used by Graphshell at startup: `file` and
    /// `about` schemes registered, fallback is `about`.
    pub fn core_seed() -> Self {
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

    #[test]
    fn registered_scheme_resolves_to_itself() {
        let mut registry = ProtocolContractRegistry::core_seed();
        registry.register_scheme("gemini", "protocol:gemini");
        let resolution = registry.resolve_scheme("gemini://example.com");
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_scheme, "gemini");
    }

    #[test]
    fn scheme_lookup_is_case_insensitive() {
        let registry = ProtocolContractRegistry::core_seed();
        assert!(registry.has_scheme("FILE"));
        assert!(registry.has_scheme("File"));
    }
}
