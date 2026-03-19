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

#[cfg(all(test, feature = "wry"))]
use crate::mods::native::verso::wry_manager::OverlaySyncState;
use crate::registries::atomic::{ProtocolHandlerProviders, ViewerHandlerProviders};
use crate::registries::infrastructure::mod_loader::{
    ModCapability, ModManifest, ModType, NativeModRegistration,
};
#[cfg(feature = "wry")]
use crate::{
    graph::NodeKey,
    mods::native::verso::wry_manager::{OverlayRect, WryManager},
};
#[cfg(feature = "wry")]
use raw_window_handle::RawWindowHandle;
#[cfg(feature = "wry")]
use std::cell::RefCell;

pub(crate) mod client_storage;
#[cfg(feature = "wry")]
pub(crate) mod wry_manager;
#[cfg(feature = "wry")]
pub(crate) mod wry_types;
#[cfg(feature = "wry")]
pub(crate) mod wry_viewer;

// WryManager is not Send (wry::WebView is !Send on Windows/macOS due to COM/Obj-C constraints).
// Access is always from the main thread; a thread-local RefCell provides safe single-threaded access.
#[cfg(feature = "wry")]
thread_local! {
    static WRY_MANAGER: RefCell<WryManager> = RefCell::new(WryManager::new());
}

#[cfg(feature = "wry")]
fn with_wry_manager<F, R>(f: F) -> R
where
    F: FnOnce(&mut WryManager) -> R,
{
    WRY_MANAGER.with(|cell| f(&mut cell.borrow_mut()))
}

/// Ensure a wry child WebView exists for `node_key`, creating it if needed.
///
/// Must be called before `sync_wry_overlay_for_node`.  `url` is the initial URL
/// to load; `parent_handle` is the OS window handle from
/// `EmbedderWindow::raw_window_handle_for_child`.
#[cfg(feature = "wry")]
pub(crate) fn ensure_wry_overlay_for_node(
    node_key: NodeKey,
    url: &str,
    parent_handle: RawWindowHandle,
) {
    let node_id = node_key.index() as u64;
    with_wry_manager(|manager| {
        if !manager.has_webview(node_id) {
            manager.create_webview(node_id, url, parent_handle);
        }
    });
}

#[cfg(feature = "wry")]
pub(crate) fn sync_wry_overlay_for_node(node_key: NodeKey, rect: OverlayRect, visible: bool) {
    let node_id = node_key.index() as u64;
    with_wry_manager(|manager| manager.sync_overlay(node_id, rect, visible));
}

#[cfg(feature = "wry")]
pub(crate) fn hide_wry_overlay_for_node(node_key: NodeKey) -> bool {
    let node_id = node_key.index() as u64;
    with_wry_manager(|manager| {
        if !manager.has_webview(node_id) {
            return false;
        }
        if let Some(last_state) = manager.last_sync_state(node_id) {
            manager.sync_overlay(node_id, last_state.rect, false);
        }
        true
    })
}

#[cfg(feature = "wry")]
pub(crate) fn destroy_wry_overlay_for_node(node_key: NodeKey) -> bool {
    let node_id = node_key.index() as u64;
    with_wry_manager(|manager| {
        if !manager.has_webview(node_id) {
            return false;
        }
        manager.destroy_webview(node_id);
        true
    })
}

#[cfg(all(test, feature = "wry"))]
pub(crate) fn last_wry_overlay_sync_for_node_for_tests(
    node_key: NodeKey,
) -> Option<OverlaySyncState> {
    with_wry_manager(|manager| manager.last_sync_state(node_key.index() as u64))
}

#[cfg(all(test, feature = "wry"))]
pub(crate) fn reset_wry_manager_for_tests() {
    with_wry_manager(|manager| *manager = WryManager::new());
}

/// Verso mod manifest - registered at compile time via inventory
pub(crate) fn verso_manifest() -> ModManifest {
    #[cfg(feature = "wry")]
    let mut provides = vec![
        "protocol:http".to_string(),
        "protocol:https".to_string(),
        "protocol:data".to_string(),
        "viewer:webview".to_string(),
    ];

    #[cfg(feature = "wry")]
    {
        provides.push("viewer:wry".to_string());
    }

    #[cfg(not(feature = "wry"))]
    let provides = vec![
        "protocol:http".to_string(),
        "protocol:https".to_string(),
        "protocol:data".to_string(),
        "viewer:webview".to_string(),
    ];

    ModManifest::new(
        "mod:verso",
        "Verso — Web Rendering",
        ModType::Native,
        provides,
        vec!["ViewerRegistry".to_string(), "ProtocolRegistry".to_string()],
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
    #[cfg(feature = "wry")]
    {
        let platform = wry_types::WryPlatform::detect();
        log::debug!("verso: wry feature enabled ({platform:?})");
    }

    log::debug!("verso: activation hook called");
    Ok(())
}

/// Register Verso's protocol handlers into the ProtocolHandlerProviders.
/// Called during application initialization to wire Verso's handlers.
#[allow(dead_code)]
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
#[allow(dead_code)]
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

        #[cfg(feature = "wry")]
        {
            // Wry is a compatibility backend for HTTP/HTML paths and is selected
            // through explicit viewer overrides (not mime default replacement).
            registry.register_mime("application/x-graphshell-wry", "viewer:wry");
        }

        log::debug!("verso: registered viewer handlers for web content");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "wry")]
    use crate::graph::NodeKey;

    #[cfg(feature = "wry")]
    use crate::mods::native::verso::wry_manager::OverlayRect;

    #[test]
    fn verso_manifest_provides_required_capabilities() {
        let manifest = verso_manifest();
        assert_eq!(manifest.mod_id, "mod:verso");
        assert_eq!(manifest.mod_type, ModType::Native);
        assert!(manifest.provides.contains(&"protocol:http".to_string()));
        assert!(manifest.provides.contains(&"protocol:https".to_string()));
        assert!(manifest.provides.contains(&"viewer:webview".to_string()));

        #[cfg(feature = "wry")]
        assert!(manifest.provides.contains(&"viewer:wry".to_string()));

        assert!(manifest.capabilities.contains(&ModCapability::Network));
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

    #[cfg(feature = "wry")]
    #[test]
    fn wry_overlay_hide_no_ops_for_missing_slot() {
        reset_wry_manager_for_tests();
        let node_key = NodeKey::new(600);
        // hide returns false when no slot exists (no WebView was created).
        assert!(!hide_wry_overlay_for_node(node_key));
        // No sync state should exist.
        assert!(last_wry_overlay_sync_for_node_for_tests(node_key).is_none());
    }

    #[cfg(feature = "wry")]
    #[test]
    fn wry_overlay_destroy_returns_false_for_missing_slot() {
        reset_wry_manager_for_tests();
        let node_key = NodeKey::new(601);
        // destroy returns false when no slot exists.
        assert!(!destroy_wry_overlay_for_node(node_key));
        // Double-destroy is safe and also returns false.
        assert!(!destroy_wry_overlay_for_node(node_key));
        assert!(last_wry_overlay_sync_for_node_for_tests(node_key).is_none());
    }

    #[cfg(feature = "wry")]
    #[test]
    fn wry_overlay_sync_no_ops_for_missing_slot() {
        reset_wry_manager_for_tests();
        let node_key = NodeKey::new(602);
        // sync_wry_overlay_for_node is safe to call when no real WebView slot exists —
        // it skips the actual set_bounds/set_visible calls but records the intent in
        // test_sync_states for observability.
        sync_wry_overlay_for_node(
            node_key,
            OverlayRect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 100.0,
            },
            true,
        );
        // hide/destroy still return false because no real slot was ever created.
        assert!(!hide_wry_overlay_for_node(node_key));
        assert!(!destroy_wry_overlay_for_node(node_key));
    }
}
