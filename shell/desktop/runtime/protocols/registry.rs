/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Protocol Registry for managing URL scheme handlers.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

/// Result of a protocol resolution attempt.
pub enum ProtocolResult {
    /// Content is available as a byte stream.
    Stream(Box<dyn std::io::Read + Send>),
    /// Content is available at a local file path (e.g. for Servo to load).
    LocalPath(std::path::PathBuf),
    /// Resolution failed.
    Error(String),
}

/// Capabilities declared by a protocol handler.
#[derive(Debug, Clone, Default)]
pub struct ProtocolCapabilities {
    pub supports_search: bool,
    pub supports_caching: bool,
    pub is_secure: bool,
}

/// Trait for implementing a custom protocol handler (IPFS, Gemini, etc.).
pub trait ProtocolHandler: Send + Sync {
    /// The URL scheme this handler supports (e.g., "ipfs", "gemini").
    fn scheme(&self) -> &str;

    /// Resolve the content (fetch, stream, or proxy).
    fn resolve(&self, uri: &str) -> ProtocolResult;

    /// Return capabilities.
    fn capabilities(&self) -> ProtocolCapabilities;
}

/// Registry managing active protocol handlers.
pub struct ProtocolRegistry {
    handlers: HashMap<String, Arc<dyn ProtocolHandler>>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, handler: impl ProtocolHandler + 'static) {
        self.handlers
            .insert(handler.scheme().to_string(), Arc::new(handler));
    }

    pub fn get(&self, scheme: &str) -> Option<Arc<dyn ProtocolHandler>> {
        self.handlers.get(scheme).cloned()
    }

    pub fn resolve(&self, uri: &str) -> ProtocolResult {
        let Some((scheme, _)) = uri.split_once(':') else {
            return ProtocolResult::Error("Invalid URI format".to_string());
        };

        if let Some(handler) = self.get(scheme) {
            handler.resolve(uri)
        } else {
            ProtocolResult::Error(format!("No handler for scheme: {}", scheme))
        }
    }
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ProtocolResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stream(_) => f.write_str("Stream(<readable>)"),
            Self::LocalPath(path) => f.debug_tuple("LocalPath").field(path).finish(),
            Self::Error(error) => f.debug_tuple("Error").field(error).finish(),
        }
    }
}
