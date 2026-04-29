/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};

use egui_tiles::{Tile, Tree};
use servo::WebViewId;

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause, RuntimeEvent};
use crate::graph::{NodeKey, NodeLifecycle};
#[cfg(feature = "wry")]
use crate::mods::native::web_runtime;
use crate::registries::atomic::viewer::ViewerRenderMode;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::webview_status_sync::servo_webview_id_from_renderer;
use crate::shell::desktop::workbench::pane_model::{NodePaneState, TileRenderMode};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use ::verso::{HostCapabilities as VersoHostCapabilities, WebEnginePreference};

pub(crate) struct TileCoordinator;

impl TileCoordinator {
    fn registry_render_mode_for_viewer_id(viewer_id: &str) -> TileRenderMode {
        let Some(capability) =
            crate::shell::desktop::runtime::registries::phase0_describe_viewer(viewer_id)
        else {
            return TileRenderMode::Placeholder;
        };

        match capability.render_mode {
            ViewerRenderMode::CompositedTexture => TileRenderMode::CompositedTexture,
            ViewerRenderMode::NativeOverlay => TileRenderMode::NativeOverlay,
            ViewerRenderMode::EmbeddedHost => TileRenderMode::EmbeddedHost,
            ViewerRenderMode::Placeholder => TileRenderMode::Placeholder,
        }
    }

    fn render_mode_for_viewer_id(viewer_id: &str) -> TileRenderMode {
        Self::registry_render_mode_for_viewer_id(viewer_id)
    }

    fn wry_render_mode_for_preferences(graph_app: &GraphBrowserApp) -> TileRenderMode {
        #[cfg(feature = "wry")]
        {
            match graph_app.wry_render_mode_preference() {
                crate::app::WryRenderModePreference::ForceOverlay => TileRenderMode::NativeOverlay,
                crate::app::WryRenderModePreference::ForceTexture
                    if crate::registries::infrastructure::mod_loader::runtime_has_capability(
                        "viewer:wry",
                    ) && crate::mods::native::web_runtime::wry_composited_texture_support()
                        .supported =>
                {
                    TileRenderMode::CompositedTexture
                }
                crate::app::WryRenderModePreference::ForceTexture => TileRenderMode::NativeOverlay,
                crate::app::WryRenderModePreference::Auto => TileRenderMode::NativeOverlay,
            }
        }
        #[cfg(not(feature = "wry"))]
        {
            let _ = graph_app;
            TileRenderMode::NativeOverlay
        }
    }

    fn render_mode_for_effective_viewer_id(
        graph_app: &GraphBrowserApp,
        viewer_id: &str,
    ) -> TileRenderMode {
        if viewer_id == "viewer:wry" {
            return Self::wry_render_mode_for_preferences(graph_app);
        }
        Self::registry_render_mode_for_viewer_id(viewer_id)
    }

    pub(crate) fn viewer_id_uses_composited_runtime(viewer_id: &str) -> bool {
        Self::render_mode_for_viewer_id(viewer_id) == TileRenderMode::CompositedTexture
    }

    pub(crate) fn preferred_viewer_id_for_content(
        graph_app: &GraphBrowserApp,
        url: &str,
        mime_hint: Option<&str>,
    ) -> String {
        let web_engine_preference = graph_app
            .default_web_viewer_backend()
            .web_engine_preference();

        if let Some(decision) = ::verso::select_viewer_for_content(
            url,
            mime_hint,
            &verso_host_capabilities_for_graphshell(graph_app),
            web_engine_preference,
        ) {
            return decision.viewer_id().to_string();
        }

        crate::shell::desktop::runtime::registries::phase0_select_viewer_for_content(url, mime_hint)
            .viewer_id
            .to_string()
    }

    fn node_pane_effective_viewer_id(
        state: &NodePaneState,
        graph_app: &GraphBrowserApp,
    ) -> Option<String> {
        if let Some(viewer_id_override) = state.viewer_id_override.as_ref() {
            return Some(viewer_id_override.as_str().to_string());
        }

        let node = graph_app.domain_graph().get_node(state.node)?;
        let web_engine_preference = if node.compat_mode {
            WebEnginePreference::Wry
        } else {
            graph_app
                .default_web_viewer_backend()
                .web_engine_preference()
        };
        if let Some(decision) = ::verso::select_viewer_for_content(
            node.url(),
            node.mime_hint.as_deref(),
            &verso_host_capabilities_for_graphshell(graph_app),
            web_engine_preference,
        ) {
            return Some(decision.viewer_id().to_string());
        }
        Some(
            crate::shell::desktop::runtime::registries::phase0_select_viewer_for_content(
                node.url(),
                node.mime_hint.as_deref(),
            )
            .viewer_id
            .to_string(),
        )
    }

    fn resolve_node_pane_render_mode(
        state: &NodePaneState,
        graph_app: &GraphBrowserApp,
    ) -> TileRenderMode {
        Self::node_pane_effective_viewer_id(state, graph_app)
            .as_deref()
            .map(|viewer_id| Self::render_mode_for_effective_viewer_id(graph_app, viewer_id))
            .unwrap_or(TileRenderMode::Placeholder)
    }

    fn render_path_hint_for_mode(
        render_mode: TileRenderMode,
        mapped_webview: bool,
        has_context: bool,
    ) -> &'static str {
        match render_mode {
            TileRenderMode::CompositedTexture => {
                if mapped_webview && has_context {
                    "composited"
                } else if mapped_webview {
                    "composited-missing-context"
                } else {
                    "composited-unmapped"
                }
            }
            TileRenderMode::NativeOverlay => "native-overlay",
            TileRenderMode::EmbeddedHost => "embedded-host",
            TileRenderMode::Placeholder => "placeholder",
        }
    }

    fn node_pane_uses_composited_runtime_impl(
        state: &NodePaneState,
        graph_app: &GraphBrowserApp,
    ) -> bool {
        let effective_mode = if state.render_mode == TileRenderMode::Placeholder {
            // Transitional fallback for panes that have not yet had render mode refreshed.
            Self::resolve_node_pane_render_mode(state, graph_app)
        } else {
            state.render_mode
        };
        effective_mode == TileRenderMode::CompositedTexture
    }

    fn collect_node_pane_keys_using_composited_runtime(
        tiles_tree: &Tree<TileKind>,
        graph_app: &GraphBrowserApp,
    ) -> HashSet<NodeKey> {
        tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Node(state))
                    if Self::node_pane_uses_composited_runtime_impl(state, graph_app) =>
                {
                    Some(state.node)
                }
                _ => None,
            })
            .collect()
    }

    fn should_preserve_runtime_webview(
        node_exists: bool,
        mapped_webview: Option<crate::app::RendererId>,
    ) -> bool {
        node_exists && mapped_webview.is_some()
    }

    pub(crate) fn reset_runtime_webview_state(
        tiles_tree: &mut Tree<TileKind>,
        viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
        viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
            crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
        >,
        tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
        favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    ) {
        let surface_keys: Vec<_> = viewer_surfaces.keys().copied().collect();
        for node_key in surface_keys {
            viewer_surface_host.retire_surface(viewer_surfaces, node_key);
        }
        tile_favicon_textures.clear();
        favicon_textures.clear();
        Self::remove_all_node_panes(tiles_tree);
    }

    pub(crate) fn has_any_node_panes(tiles_tree: &Tree<TileKind>) -> bool {
        tiles_tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(_))))
    }

    pub(crate) fn all_node_pane_keys(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
        tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Node(state)) => Some(state.node),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn all_node_pane_keys_using_composited_runtime(
        tiles_tree: &Tree<TileKind>,
        graph_app: &GraphBrowserApp,
    ) -> HashSet<NodeKey> {
        Self::collect_node_pane_keys_using_composited_runtime(tiles_tree, graph_app)
    }

    pub(crate) fn node_pane_uses_composited_runtime(
        state: &NodePaneState,
        graph_app: &GraphBrowserApp,
    ) -> bool {
        Self::node_pane_uses_composited_runtime_impl(state, graph_app)
    }

    pub(crate) fn refresh_node_pane_render_modes(
        tiles_tree: &mut Tree<TileKind>,
        graph_app: &GraphBrowserApp,
    ) {
        for (_, tile) in tiles_tree.tiles.iter_mut() {
            if let Tile::Pane(TileKind::Node(state)) = tile {
                let viewer_id = Self::node_pane_effective_viewer_id(state, graph_app);
                state.render_mode = viewer_id
                    .as_deref()
                    .map(|vid| Self::render_mode_for_effective_viewer_id(graph_app, vid))
                    .unwrap_or(TileRenderMode::Placeholder);
                state.resolved_viewer_id = viewer_id;
                state.resolved_route = resolve_route_for_node_pane(state, graph_app);
            }
        }
    }

    pub(crate) fn prune_stale_node_pane_keys_only(
        tiles_tree: &mut Tree<TileKind>,
        graph_app: &GraphBrowserApp,
    ) {
        let stale_nodes: Vec<_> = Self::all_node_pane_keys(tiles_tree)
            .into_iter()
            .filter(|node_key| graph_app.domain_graph().get_node(*node_key).is_none())
            .collect();
        for node_key in stale_nodes {
            Self::remove_node_pane_for_node(tiles_tree, node_key);
        }
    }

    pub(crate) fn remove_all_node_panes(tiles_tree: &mut Tree<TileKind>) {
        let tile_ids: Vec<_> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(tile_id, tile)| match tile {
                Tile::Pane(TileKind::Node(_)) => Some(*tile_id),
                _ => None,
            })
            .collect();
        for tile_id in tile_ids {
            tiles_tree.remove_recursively(tile_id);
        }
    }

    pub(crate) fn remove_node_pane_for_node(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
        let tile_ids: Vec<_> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(tile_id, tile)| match tile {
                Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(*tile_id),
                _ => None,
            })
            .collect();
        for tile_id in tile_ids {
            tiles_tree.remove_recursively(tile_id);
        }
    }

    pub(crate) fn prune_stale_node_panes(
        tiles_tree: &mut Tree<TileKind>,
        graph_app: &mut GraphBrowserApp,
        window: &EmbedderWindow,
        viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
        viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
            crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
        >,
        lifecycle_intents: &mut Vec<GraphIntent>,
    ) {
        let stale_nodes: Vec<_> = Self::all_node_pane_keys(tiles_tree)
            .into_iter()
            .filter(|node_key| graph_app.domain_graph().get_node(*node_key).is_none())
            .collect();

        for node_key in stale_nodes {
            Self::remove_node_pane_for_node(tiles_tree, node_key);
            Self::release_node_runtime_for_pane(
                graph_app,
                window,
                viewer_surfaces,
                viewer_surface_host,
                node_key,
                lifecycle_intents,
            );
        }
    }

    pub(crate) fn release_node_runtime_for_pane(
        graph_app: &mut GraphBrowserApp,
        window: &EmbedderWindow,
        viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
        viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
            crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
        >,
        node_key: NodeKey,
        lifecycle_intents: &mut Vec<GraphIntent>,
    ) {
        let node_exists = graph_app.domain_graph().get_node(node_key).is_some();
        let mapped_webview = graph_app.get_webview_for_node(node_key);

        if mapped_webview.is_none() {
            #[cfg(feature = "wry")]
            {
                // NativeOverlay backends do not use mapped Servo webviews; if this
                // node is currently managed by Wry, hide/detach it on pane release.
                let handled_by_wry = if node_exists {
                    web_runtime::hide_wry_overlay_for_node(node_key)
                } else {
                    web_runtime::destroy_wry_overlay_for_node(node_key)
                };

                if handled_by_wry {
                    if node_exists {
                        let lifecycle = graph_app
                            .domain_graph()
                            .get_node(node_key)
                            .map(|node| node.lifecycle)
                            .unwrap_or(NodeLifecycle::Cold);
                        if lifecycle != NodeLifecycle::Warm {
                            lifecycle_intents.push(
                                lifecycle_intents::demote_node_to_warm(
                                    node_key,
                                    LifecycleCause::WorkspaceRetention,
                                )
                                .into(),
                            );
                        }
                    } else {
                        lifecycle_intents.push(
                            lifecycle_intents::demote_node_to_cold(
                                node_key,
                                LifecycleCause::NodeRemoval,
                            )
                            .into(),
                        );
                    }
                }
            }

            viewer_surface_host.retire_surface(viewer_surfaces, node_key);
            return;
        }

        if Self::should_preserve_runtime_webview(node_exists, mapped_webview) {
            let lifecycle = graph_app
                .domain_graph()
                .get_node(node_key)
                .map(|node| node.lifecycle)
                .unwrap_or(NodeLifecycle::Cold);
            // If the node was explicitly dismissed (lifecycle already Cold), fall
            // through to the teardown path — do not re-promote to Warm.
            if lifecycle != NodeLifecycle::Cold {
                if lifecycle != NodeLifecycle::Warm {
                    lifecycle_intents.push(
                        lifecycle_intents::demote_node_to_warm(
                            node_key,
                            LifecycleCause::WorkspaceRetention,
                        )
                        .into(),
                    );
                }
                return;
            }
        }

        viewer_surface_host.retire_surface(viewer_surfaces, node_key);

        if let Some(renderer_id) = mapped_webview {
            if let Some(servo_webview_id) = servo_webview_id_from_renderer(renderer_id) {
                window.close_webview(servo_webview_id);
            }
            lifecycle_intents.push(
                RuntimeEvent::UnmapWebview {
                    webview_id: renderer_id,
                }
                .into(),
            );
        }
        lifecycle_intents.push(
            lifecycle_intents::demote_node_to_cold(node_key, LifecycleCause::NodeRemoval).into(),
        );
    }
}

pub(crate) fn verso_host_capabilities_for_graphshell(
    graph_app: &GraphBrowserApp,
) -> VersoHostCapabilities {
    let supports_middlenet =
        crate::shell::desktop::runtime::registries::phase0_describe_viewer("viewer:middlenet")
            .is_some();
    let supports_servo =
        crate::shell::desktop::runtime::registries::phase0_describe_viewer("viewer:webview")
            .is_some();
    let supports_wry = cfg!(feature = "wry")
        && graph_app.wry_enabled()
        && crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry");

    VersoHostCapabilities {
        supports_middlenet_direct: supports_middlenet,
        supports_middlenet_html: false,
        supports_middlenet_faithful_source: supports_middlenet,
        supports_servo,
        supports_wry,
    }
}

/// Resolve the verso route for a node pane. Returns `None` when
/// verso does not route this content (specialized non-web viewers:
/// images, PDFs, plaintext, directory listings) — in that case the
/// registry-selected viewer at `state.resolved_viewer_id` is
/// authoritative.
///
/// Called during `refresh_node_pane_render_modes` so the resulting
/// route can be cached on `NodePaneState.resolved_route` and
/// consumed by downstream surfaces (workbench chrome, debug UI)
/// without re-dispatching verso every frame.
pub(crate) fn resolve_route_for_node_pane(
    state: &NodePaneState,
    graph_app: &GraphBrowserApp,
) -> Option<::verso::VersoResolvedRoute> {
    let node = graph_app.domain_graph().get_node(state.node)?;
    let owner = if state.viewer_id_override.is_some() {
        ::verso::VersoPaneOwner::UserPin
    } else {
        ::verso::VersoPaneOwner::Policy
    };
    // Per-node compat mode biases the web-engine preference toward Wry
    // for this node only; the app-level default still drives every
    // other node. Middlenet lane selection is unaffected because compat
    // mode only shifts the web-engine preference.
    let web_engine_preference = if node.compat_mode {
        ::verso::WebEnginePreference::Wry
    } else {
        graph_app
            .default_web_viewer_backend()
            .web_engine_preference()
    };
    ::verso::resolve_route_for_content(
        node.url(),
        node.mime_hint.as_deref(),
        &verso_host_capabilities_for_graphshell(graph_app),
        web_engine_preference,
        owner,
    )
}

pub(crate) fn effective_viewer_id_for_pane_in_tree(
    tiles_tree: &Tree<TileKind>,
    pane_id: crate::shell::desktop::workbench::pane_model::PaneId,
    graph_app: &GraphBrowserApp,
) -> Option<String> {
    tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        Tile::Pane(TileKind::Node(state)) if state.pane_id == pane_id => {
            TileCoordinator::node_pane_effective_viewer_id(state, graph_app)
        }
        _ => None,
    })
}

pub(crate) fn viewer_id_uses_composited_runtime(viewer_id: &str) -> bool {
    TileCoordinator::viewer_id_uses_composited_runtime(viewer_id)
}

pub(crate) fn reset_runtime_webview_state(
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
) {
    TileCoordinator::reset_runtime_webview_state(
        tiles_tree,
        viewer_surfaces,
        viewer_surface_host,
        tile_favicon_textures,
        favicon_textures,
    );
}

pub(crate) fn has_any_node_panes(tiles_tree: &Tree<TileKind>) -> bool {
    TileCoordinator::has_any_node_panes(tiles_tree)
}

pub(crate) fn all_node_pane_keys(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
    TileCoordinator::all_node_pane_keys(tiles_tree)
}

pub(crate) fn all_node_pane_keys_using_composited_runtime(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
) -> HashSet<NodeKey> {
    TileCoordinator::all_node_pane_keys_using_composited_runtime(tiles_tree, graph_app)
}

pub(crate) fn node_pane_uses_composited_runtime(
    state: &NodePaneState,
    graph_app: &GraphBrowserApp,
) -> bool {
    TileCoordinator::node_pane_uses_composited_runtime(state, graph_app)
}

fn wry_placeholder_reason(graph_app: &GraphBrowserApp) -> Option<&'static str> {
    if !cfg!(feature = "wry") {
        return Some("Wry backend is not compiled in this build.");
    }
    if !crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry") {
        return Some("Runtime capability 'viewer:wry' is unavailable.");
    }
    if !graph_app.wry_enabled() {
        return Some("Wry backend is disabled. Enable it in Settings -> Viewer Backends.");
    }
    None
}

pub(crate) fn effective_viewer_id_for_node_pane(
    state: &NodePaneState,
    graph_app: &GraphBrowserApp,
) -> Option<String> {
    TileCoordinator::node_pane_effective_viewer_id(state, graph_app)
}

pub(crate) fn candidate_viewer_ids_for_node_pane(
    state: &NodePaneState,
    graph_app: &GraphBrowserApp,
) -> Vec<String> {
    let Some(node) = graph_app.domain_graph().get_node(state.node) else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    let mut push_candidate = |viewer_id: &str| {
        if crate::shell::desktop::runtime::registries::phase0_describe_viewer(viewer_id).is_some()
            && !candidates.iter().any(|existing| existing == viewer_id)
        {
            candidates.push(viewer_id.to_string());
        }
    };

    if let Some(effective_viewer_id) =
        TileCoordinator::node_pane_effective_viewer_id(state, graph_app)
    {
        push_candidate(&effective_viewer_id);
    }

    // Ask verso to enumerate web/Middlenet candidates. Verso's
    // `select_viewer_for_content` is decision-oriented (returns one
    // best choice per preference), so we query both preferences to
    // surface both web-engine candidates when the host supports
    // them. Middlenet content returns the same decision regardless
    // of preference; `push_candidate` dedups the duplicate.
    //
    // TODO (Phase 4/PR 4): replace the two-call pattern with a
    // proper `list_candidate_routes` API on `crates/verso` once
    // the resolved-route types land.
    let host_caps = verso_host_capabilities_for_graphshell(graph_app);
    for preference in [WebEnginePreference::Servo, WebEnginePreference::Wry] {
        if let Some(decision) = ::verso::select_viewer_for_content(
            node.url(),
            node.mime_hint.as_deref(),
            &host_caps,
            preference,
        ) {
            push_candidate(decision.viewer_id());
        }
    }

    // For specialized non-web content (images, PDFs, local files),
    // the registry still owns viewer selection. Verso returned
    // nothing for this URL, so consult the registry.
    let selected = crate::shell::desktop::runtime::registries::phase0_select_viewer_for_content(
        node.url(),
        node.mime_hint.as_deref(),
    );
    push_candidate(selected.viewer_id);

    candidates.sort_by(|left, right| {
        viewer_candidate_sort_key(left).cmp(&viewer_candidate_sort_key(right))
    });
    candidates
}

fn viewer_candidate_sort_key(viewer_id: &str) -> (u8, &str) {
    match viewer_id {
        "viewer:webview" => (0, viewer_id),
        "viewer:wry" => (1, viewer_id),
        _ => (2, viewer_id),
    }
}

pub(crate) fn fallback_reason_for_node_pane(
    state: &NodePaneState,
    graph_app: &GraphBrowserApp,
) -> Option<String> {
    if state.render_mode != TileRenderMode::Placeholder {
        return None;
    }

    let Some(effective_viewer_id) =
        TileCoordinator::node_pane_effective_viewer_id(state, graph_app)
    else {
        return Some("No viewer could be resolved for this node.".to_string());
    };

    if effective_viewer_id == "viewer:wry"
        && let Some(reason) = wry_placeholder_reason(graph_app)
    {
        return Some(reason.to_string());
    }

    if crate::shell::desktop::runtime::registries::phase0_describe_viewer(&effective_viewer_id)
        .is_none()
    {
        return Some(format!(
            "Viewer '{effective_viewer_id}' is unresolved for this build path."
        ));
    }

    Some(format!(
        "'{effective_viewer_id}' currently falls back to placeholder rendering."
    ))
}

pub(crate) fn refresh_node_pane_render_modes(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
) {
    TileCoordinator::refresh_node_pane_render_modes(tiles_tree, graph_app);
}

pub(crate) fn render_path_hint_for_mode(
    render_mode: TileRenderMode,
    mapped_webview: bool,
    has_context: bool,
) -> &'static str {
    TileCoordinator::render_path_hint_for_mode(render_mode, mapped_webview, has_context)
}

pub(crate) fn render_mode_for_node_pane_in_tree(
    tiles_tree: &Tree<TileKind>,
    node_key: NodeKey,
) -> TileRenderMode {
    tiles_tree
        .tiles
        .iter()
        .find_map(|(_, tile)| match tile {
            Tile::Pane(kind @ TileKind::Node(state)) if state.node == node_key => {
                kind.node_render_mode()
            }
            _ => None,
        })
        .unwrap_or(TileRenderMode::Placeholder)
}

pub(crate) fn render_mode_for_pane_in_tree(
    tiles_tree: &Tree<TileKind>,
    pane_id: crate::shell::desktop::workbench::pane_model::PaneId,
) -> TileRenderMode {
    tiles_tree
        .tiles
        .iter()
        .find_map(|(_, tile)| match tile {
            Tile::Pane(kind @ TileKind::Node(state)) if state.pane_id == pane_id => {
                kind.node_render_mode()
            }
            _ => None,
        })
        .unwrap_or(TileRenderMode::Placeholder)
}

pub(crate) fn prune_stale_node_pane_keys_only(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
) {
    TileCoordinator::prune_stale_node_pane_keys_only(tiles_tree, graph_app);
}

#[allow(dead_code)]
pub(crate) fn remove_all_node_panes(tiles_tree: &mut Tree<TileKind>) {
    TileCoordinator::remove_all_node_panes(tiles_tree);
}

#[allow(dead_code)]
pub(crate) fn remove_node_pane_for_node(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
    TileCoordinator::remove_node_pane_for_node(tiles_tree, node_key);
}

pub(crate) fn prune_stale_node_panes(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    TileCoordinator::prune_stale_node_panes(
        tiles_tree,
        graph_app,
        window,
        viewer_surfaces,
        viewer_surface_host,
        lifecycle_intents,
    );
}

pub(crate) fn release_node_runtime_for_pane(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    node_key: NodeKey,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    TileCoordinator::release_node_runtime_for_pane(
        graph_app,
        window,
        viewer_surfaces,
        viewer_surface_host,
        node_key,
        lifecycle_intents,
    );
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::TileCoordinator;
    use crate::app::GraphBrowserApp;
    use crate::graph::NodeKey;
    use crate::shell::desktop::workbench::pane_model::{
        GraphPaneRef, NodePaneState, TileRenderMode, ViewerId,
    };
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use egui_tiles::{Tile, Tiles, Tree};
    use euclid::Point2D;

    fn test_renderer_id() -> crate::app::RendererId {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        crate::app::RendererId::from_raw(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    #[test]
    fn preserve_runtime_webview_when_node_exists_and_mapped() {
        let webview_id = test_renderer_id();
        assert!(TileCoordinator::should_preserve_runtime_webview(
            true,
            Some(webview_id)
        ));
    }

    #[test]
    fn do_not_preserve_runtime_webview_when_node_missing_or_unmapped() {
        let webview_id = test_renderer_id();
        assert!(!TileCoordinator::should_preserve_runtime_webview(
            false,
            Some(webview_id)
        ));
        assert!(!TileCoordinator::should_preserve_runtime_webview(
            true, None
        ));
    }

    fn tree_with_node_pane(state: NodePaneState) -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let pane = tiles.insert_pane(TileKind::Node(state));
        let root = tiles.insert_tab_tile(vec![pane]);
        Tree::new("tile_runtime_viewer_selection_test", root, tiles)
    }

    #[test]
    fn node_pane_using_composited_runtime_uses_registry_selection_for_http_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let tree = tree_with_node_pane(NodePaneState::for_node(node_key));

        let hosts = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);
        assert!(hosts.contains(&node_key));
    }

    #[test]
    fn node_pane_using_composited_runtime_uses_registry_selection_for_file_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        // Use a .txt file: extension "txt" maps to viewer:plaintext (EmbeddedHost),
        // which is not composited. (PDF maps to viewer:webview via the Verso mod.)
        let node_key =
            app.add_node_and_sync("file:///tmp/readme.txt".into(), Point2D::new(0.0, 0.0));
        let tree = tree_with_node_pane(NodePaneState::for_node(node_key));

        let hosts = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);
        assert!(!hosts.contains(&node_key));
    }

    #[test]
    fn node_pane_using_composited_runtime_uses_fallback_for_custom_schemes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key =
            app.add_node_and_sync("gemini://example.test".into(), Point2D::new(0.0, 0.0));
        let tree = tree_with_node_pane(NodePaneState::for_node(node_key));

        let hosts = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);
        assert!(!hosts.contains(&node_key));
    }

    #[test]
    fn node_pane_using_composited_runtime_preserves_explicit_viewer_override_precedence() {
        let mut app = GraphBrowserApp::new_for_testing();
        let http_node =
            app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let file_node =
            app.add_node_and_sync("file:///tmp/report.pdf".into(), Point2D::new(10.0, 0.0));

        let http_plaintext_tree = tree_with_node_pane(NodePaneState::with_viewer(
            http_node,
            ViewerId::new("viewer:plaintext"),
        ));
        let file_webview_tree = tree_with_node_pane(NodePaneState::with_viewer(
            file_node,
            ViewerId::new("viewer:webview"),
        ));

        let http_hosts = TileCoordinator::all_node_pane_keys_using_composited_runtime(
            &http_plaintext_tree,
            &app,
        );
        let file_hosts =
            TileCoordinator::all_node_pane_keys_using_composited_runtime(&file_webview_tree, &app);

        assert!(!http_hosts.contains(&http_node));
        assert!(file_hosts.contains(&file_node));
    }

    #[test]
    fn node_pane_composited_host_selection_prefers_explicit_render_mode_field() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));

        let mut state = NodePaneState::for_node(node_key);
        state.render_mode = TileRenderMode::EmbeddedHost;
        let tree = tree_with_node_pane(state);

        let hosts = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);
        assert!(
            !hosts.contains(&node_key),
            "explicit non-composited render mode should override inferred viewer selection"
        );
    }

    #[test]
    fn node_pane_unknown_viewer_override_falls_back_to_placeholder() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key =
            app.add_node_and_sync("file:///tmp/report.pdf".into(), Point2D::new(0.0, 0.0));
        let mut tree = tree_with_node_pane(NodePaneState::with_viewer(
            node_key,
            ViewerId::new("viewer:unknown"),
        ));

        TileCoordinator::refresh_node_pane_render_modes(&mut tree, &app);

        let mut mode_for = HashMap::new();
        for (_, tile) in tree.tiles.iter() {
            if let egui_tiles::Tile::Pane(TileKind::Node(state)) = tile {
                mode_for.insert(state.node, state.render_mode);
            }
        }

        assert_eq!(
            mode_for.get(&node_key).copied(),
            Some(TileRenderMode::Placeholder)
        );
        assert!(!TileCoordinator::viewer_id_uses_composited_runtime(
            "viewer:unknown"
        ));
    }

    #[test]
    fn composited_runtime_nodes_are_subset_of_all_node_panes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let webview_node =
            app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let plaintext_node =
            app.add_node_and_sync("file:///tmp/readme.txt".into(), Point2D::new(10.0, 0.0));

        let mut tiles = Tiles::default();
        let a = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(webview_node)));
        let b = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(plaintext_node)));
        let root = tiles.insert_tab_tile(vec![a, b]);
        let tree = Tree::new("tile_runtime_node_vs_host_subset", root, tiles);

        let all_nodes = TileCoordinator::all_node_pane_keys(&tree);
        let host_nodes = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);

        assert!(all_nodes.contains(&webview_node));
        assert!(all_nodes.contains(&plaintext_node));
        assert!(host_nodes.contains(&webview_node));
        assert!(!host_nodes.contains(&plaintext_node));
        assert!(host_nodes.is_subset(&all_nodes));
    }

    #[test]
    fn refresh_node_pane_render_modes_sets_mode_for_each_node_pane() {
        let mut app = GraphBrowserApp::new_for_testing();
        let webview_node =
            app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let plaintext_node =
            app.add_node_and_sync("file:///tmp/readme.txt".into(), Point2D::new(10.0, 0.0));

        let mut tiles = Tiles::default();
        let a = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(webview_node)));
        let b = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(plaintext_node)));
        let root = tiles.insert_tab_tile(vec![a, b]);
        let mut tree = Tree::new("tile_runtime_render_mode_refresh", root, tiles);

        TileCoordinator::refresh_node_pane_render_modes(&mut tree, &app);

        let mut mode_for = HashMap::new();
        for (_, tile) in tree.tiles.iter() {
            if let egui_tiles::Tile::Pane(TileKind::Node(state)) = tile {
                mode_for.insert(state.node, state.render_mode);
            }
        }

        assert_eq!(
            mode_for.get(&webview_node).copied(),
            Some(TileRenderMode::CompositedTexture)
        );
        assert_eq!(
            mode_for.get(&plaintext_node).copied(),
            Some(TileRenderMode::EmbeddedHost)
        );
    }

    #[test]
    fn preferred_viewer_id_for_content_uses_workspace_default_wry_when_enabled() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_wry_enabled(true);
        app.set_default_web_viewer_backend(crate::app::DefaultWebViewerBackend::Wry);

        let preferred = TileCoordinator::preferred_viewer_id_for_content(
            &app,
            "https://example.test",
            Some("text/html"),
        );

        #[cfg(feature = "wry")]
        if crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry") {
            assert_eq!(preferred, "viewer:wry");
        } else {
            assert_eq!(preferred, "viewer:webview");
        }
        #[cfg(not(feature = "wry"))]
        assert_eq!(preferred, "viewer:webview");
    }

    #[test]
    fn refresh_node_pane_render_modes_uses_force_texture_for_wry_panes() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_wry_enabled(true);
        app.set_wry_render_mode_preference(crate::app::WryRenderModePreference::ForceTexture);
        let node_key = app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let mut tree = tree_with_node_pane(NodePaneState::with_viewer(
            node_key,
            ViewerId::new("viewer:wry"),
        ));

        TileCoordinator::refresh_node_pane_render_modes(&mut tree, &app);

        let mode = tree
            .tiles
            .iter()
            .find_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Node(state)) if state.node == node_key => {
                    Some(state.render_mode)
                }
                _ => None,
            })
            .expect("expected node pane render mode");

        #[cfg(feature = "wry")]
        if crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry")
            && crate::mods::native::web_runtime::wry_composited_texture_support().supported
        {
            assert_eq!(mode, TileRenderMode::CompositedTexture);
        } else {
            assert_eq!(mode, TileRenderMode::NativeOverlay);
        }
        #[cfg(not(feature = "wry"))]
        assert_eq!(mode, TileRenderMode::NativeOverlay);
    }

    #[test]
    fn render_path_hint_projects_from_tile_render_mode() {
        assert_eq!(
            TileCoordinator::render_path_hint_for_mode(
                TileRenderMode::CompositedTexture,
                true,
                true
            ),
            "composited"
        );
        assert_eq!(
            TileCoordinator::render_path_hint_for_mode(
                TileRenderMode::CompositedTexture,
                true,
                false
            ),
            "composited-missing-context"
        );
        assert_eq!(
            TileCoordinator::render_path_hint_for_mode(
                TileRenderMode::CompositedTexture,
                false,
                false
            ),
            "composited-unmapped"
        );
        assert_eq!(
            TileCoordinator::render_path_hint_for_mode(TileRenderMode::NativeOverlay, false, false),
            "native-overlay"
        );
        assert_eq!(
            TileCoordinator::render_path_hint_for_mode(TileRenderMode::EmbeddedHost, false, false),
            "embedded-host"
        );
        assert_eq!(
            TileCoordinator::render_path_hint_for_mode(TileRenderMode::Placeholder, false, false),
            "placeholder"
        );
    }

    #[test]
    fn render_mode_for_node_pane_in_tree_returns_placeholder_for_missing_node() {
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(
            crate::app::GraphViewId::default(),
        )));
        let tree = Tree::new("tile_runtime_render_mode_lookup", graph, tiles);

        assert_eq!(
            super::render_mode_for_node_pane_in_tree(&tree, NodeKey::new(9999)),
            TileRenderMode::Placeholder
        );
    }

    #[test]
    fn render_mode_for_pane_in_tree_prefers_exact_pane_identity() {
        let node_key = NodeKey::new(11);
        let mut first = NodePaneState::for_node(node_key);
        first.render_mode = TileRenderMode::CompositedTexture;
        let first_pane = first.pane_id;

        let mut second = NodePaneState::for_node(node_key);
        second.render_mode = TileRenderMode::NativeOverlay;
        let second_pane = second.pane_id;

        let mut tiles = Tiles::default();
        let a = tiles.insert_pane(TileKind::Node(first));
        let b = tiles.insert_pane(TileKind::Node(second));
        let root = tiles.insert_tab_tile(vec![a, b]);
        let tree = Tree::new("tile_runtime_render_mode_lookup_by_pane", root, tiles);

        assert_eq!(
            super::render_mode_for_pane_in_tree(&tree, first_pane),
            TileRenderMode::CompositedTexture
        );
        assert_eq!(
            super::render_mode_for_pane_in_tree(&tree, second_pane),
            TileRenderMode::NativeOverlay
        );
    }
}
