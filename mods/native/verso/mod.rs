/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Verso Native Mod: HTTP/HTTPS/Webview Support
//!
//! Provides web rendering capabilities via Servo/Wry integration.
//! Registers HTTP/HTTPS protocol handlers and webview viewer support.
//!
//! This mod is optional; the app functions as an offline graph organizer
//! without it (core seed mode).

use crate::registries::infrastructure::mod_loader::{
    ModCapability, ModManifest, ModType, NativeModRegistration,
};
use crate::registries::atomic::{
    ProtocolHandlerProviders, ViewerHandlerProviders,
};

/// Verso mod manifest - registered at compile time via inventory
pub(crate) fn verso_manifest() -> ModManifest {
    ModManifest::new(
        "verso",
        "Verso — Web Rendering",
        ModType::Native,
        vec![
            "protocol:http".to_string(),
            "protocol:https".to_string(),
            "protocol:data".to_string(),
            "viewer:webview".to_string(),
        ],
        vec![
            "ViewerRegistry".to_string(),
            "ProtocolRegistry".to_string(),
        ],
        vec![ModCapability::Network],
    )
}

// Register this mod via inventory at compile time
inventory::submit! {
    NativeModRegistration {
        manifest: verso_manifest,
    }
}

/// Verso mod activation handler — called when this mod is loaded.
/// Registers HTTP/HTTPS/data protocol handlers and webview viewer support
/// into the protocol and viewer registries.
pub(crate) fn activate() -> Result<(), String> {
    // Phase 2.2: Register Verso protocol and viewer handlers.
    // In a full implementation, this would be called with mutable access to the
    // ProtocolContractRegistry and ViewerRegistry to actually register handlers.
    //
    // For Phase 2, we establish the hook and structure; Phase 2.3 extends to
    // actual handler registration when registries are mutable during app init.
    log::debug!("verso: activation hook called");
    Ok(())
}

/// Register Verso's protocol handlers into the ProtocolHandlerProviders.
/// Called during application initialization to wire Verso's handlers.
pub(crate) fn register_protocol_handlers(providers: &mut ProtocolHandlerProviders) {
    providers.register_fn(|registry| {
        // Register HTTP and HTTPS protocol handlers
        registry.register_scheme("http", "protocol:http");
        registry.register_scheme("https", "protocol:https");
        registry.register_scheme("data", "protocol:data");
        log::debug!("verso: registered protocol handlers for http, https, data");
    });
}

/// Register Verso's viewer handlers into the ViewerHandlerProviders.
/// Called during application initialization to wire Verso's handlers.
pub(crate) fn register_viewer_handlers(providers: &mut ViewerHandlerProviders) {
    providers.register_fn(|registry| {
        // Register webview viewer for common MIME types and extensions
        registry.register_mime("text/html", "viewer:webview");
        registry.register_mime("application/pdf", "viewer:webview");
        registry.register_mime("image/svg+xml", "viewer:webview");
        registry.register_mime("text/css", "viewer:webview");
        registry.register_mime("application/javascript", "viewer:webview");
        registry.register_extension("html", "viewer:webview");
        registry.register_extension("htm", "viewer:webview");
        registry.register_extension("pdf", "viewer:webview");
        registry.register_extension("svg", "viewer:webview");
        log::debug!("verso: registered viewer handlers for web content");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verso_manifest_provides_required_capabilities() {
        let manifest = verso_manifest();
        assert_eq!(manifest.mod_id, "verso");
        assert_eq!(manifest.mod_type, ModType::Native);
        assert!(manifest.provides.contains(&"protocol:http".to_string()));
        assert!(manifest.provides.contains(&"protocol:https".to_string()));
        assert!(manifest.provides.contains(&"viewer:webview".to_string()));
        assert!(manifest
            .capabilities
            .contains(&ModCapability::Network));
    }

    #[test]
    fn verso_protocol_handlers_register_http_https() {
        use crate::registries::atomic::protocol::ProtocolContractRegistry;
        
        let mut providers = ProtocolHandlerProviders::new();
        register_protocol_handlers(&mut providers);
        
        let mut registry = ProtocolContractRegistry::core_seed();
        providers.apply_all(&mut registry);
        
        assert!(registry.has_scheme("http"));
        assert!(registry.has_scheme("https"));
        assert!(registry.has_scheme("data"));
    }

    #[test]
    fn verso_viewer_handlers_register_html_webview() {
        use crate::registries::atomic::viewer::ViewerRegistry;
        
        let mut providers = ViewerHandlerProviders::new();
        register_viewer_handlers(&mut providers);
        
        let mut registry = ViewerRegistry::core_seed();
        providers.apply_all(&mut registry);
        
        // Core seed includes metadata, but Verso adds webview support
        let html_selection = registry.select_for_uri("example.com/page.html", Some("text/html"));
        assert_eq!(html_selection.viewer_id, "viewer:webview");
    }
}
