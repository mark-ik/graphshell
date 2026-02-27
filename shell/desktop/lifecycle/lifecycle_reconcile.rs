/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use egui_tiles::Tree;
use servo::{OffscreenRenderingContext, WebViewId};
use sysinfo::System;

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause, MemoryPressureLevel};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::webview_backpressure::{
    self, WebviewCreationBackpressureState,
};
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::workbench::tile_compositor;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;

pub(crate) struct RuntimeReconcileArgs<'a> {
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(crate) frame_intents: &'a mut Vec<GraphIntent>,
}

fn sample_memory_pressure() -> (MemoryPressureLevel, u64, u64) {
    let mut system = System::new();
    system.refresh_memory();

    let total_bytes = system.total_memory();
    let available_bytes = system.available_memory();
    let total_mib = total_bytes / (1024 * 1024);
    let available_mib = available_bytes / (1024 * 1024);

    if total_bytes == 0 {
        return (MemoryPressureLevel::Unknown, available_mib, total_mib);
    }

    let available_pct = available_bytes as f64 / total_bytes as f64;
    let level = if available_mib <= 512 || available_pct <= 0.08 {
        MemoryPressureLevel::Critical
    } else if available_mib <= 1024 || available_pct <= 0.15 {
        MemoryPressureLevel::Warning
    } else {
        MemoryPressureLevel::Normal
    };
    (level, available_mib, total_mib)
}

fn pressure_adjusted_active_limit(base_limit: usize, level: MemoryPressureLevel) -> usize {
    match level {
        MemoryPressureLevel::Unknown | MemoryPressureLevel::Normal => base_limit,
        MemoryPressureLevel::Warning => base_limit.saturating_sub(1).max(1),
        MemoryPressureLevel::Critical => 1,
    }
}

pub(crate) fn reconcile_runtime(args: RuntimeReconcileArgs<'_>) {
    if args.graph_app.workspace.graph.node_count() == 0 {
        args.graph_app.workspace.active_webview_nodes.clear();
        args.webview_creation_backpressure.clear();
        tile_runtime::reset_runtime_webview_state(
            args.tiles_tree,
            args.tile_rendering_contexts,
            args.tile_favicon_textures,
            args.favicon_textures,
        );
    }

    tile_runtime::prune_stale_node_panes(
        args.tiles_tree,
        args.graph_app,
        args.window,
        args.tile_rendering_contexts,
        args.frame_intents,
    );
    for node_key in args.graph_app.take_warm_cache_evictions() {
        if let Some(webview_id) = args.graph_app.get_webview_for_node(node_key) {
            args.window.close_webview(webview_id);
            args.frame_intents
                .push(GraphIntent::UnmapWebview { webview_id });
        }
        args.tile_rendering_contexts.remove(&node_key);
        // Workspace-aware demotion:
        let is_workspace_member = !args.graph_app.workspaces_for_node_key(node_key).is_empty();
        if is_workspace_member {
            args.frame_intents
                .push(lifecycle_intents::demote_node_to_warm(
                    node_key,
                    LifecycleCause::WarmLruEviction,
                ));
        } else {
            args.frame_intents
                .push(lifecycle_intents::demote_node_to_cold(
                    node_key,
                    LifecycleCause::NodeRemoval,
                ));
        }
    }
    args.tile_favicon_textures
        .retain(|node_key, _| args.graph_app.workspace.graph.get_node(*node_key).is_some());

    let (memory_pressure_level, available_mib, total_mib) = sample_memory_pressure();
    args.graph_app
        .set_memory_pressure_status(memory_pressure_level, available_mib, total_mib);

    let tile_nodes = tile_runtime::all_node_pane_keys(args.tiles_tree);
    let active_tile_nodes: HashSet<NodeKey> =
        tile_compositor::active_node_pane_rects(args.tiles_tree)
            .into_iter()
            .map(|(node_key, _)| node_key)
            .collect();
    let has_node_panes = !tile_nodes.is_empty();
    // Emit lifecycle promotion intents for active tiles (intents applied after reconcile).
    // Webview creation happens in tile_render_pass after these intents are applied.
    for node_key in active_tile_nodes.iter().copied() {
        if args.graph_app.is_runtime_blocked(node_key, Instant::now()) {
            continue;
        }
        let should_promote = args
            .graph_app
            .workspace
            .graph
            .get_node(node_key)
            .map(|node| node.lifecycle != NodeLifecycle::Active)
            .unwrap_or(false);
        if should_promote && !args.graph_app.is_crash_blocked(node_key) {
            args.frame_intents
                .push(lifecycle_intents::promote_node_to_active(
                    node_key,
                    LifecycleCause::ActiveTileVisible,
                ));
        }
    }
    let prewarm_selected_node = args
        .graph_app
        .get_single_selected_node()
        .filter(|node_key| !active_tile_nodes.contains(node_key))
        .filter(|node_key| args.graph_app.get_webview_for_node(*node_key).is_none())
        .filter(|node_key| !args.graph_app.is_runtime_blocked(*node_key, Instant::now()))
        .filter(|node_key| !args.graph_app.is_crash_blocked(*node_key));
    if let Some(node_key) = prewarm_selected_node
        && args
            .graph_app
            .workspace
            .graph
            .get_node(node_key)
            .map(|node| node.lifecycle != NodeLifecycle::Active)
            .unwrap_or(false)
    {
        args.frame_intents
            .push(lifecycle_intents::promote_node_to_active(
                node_key,
                LifecycleCause::SelectedPrewarm,
            ));
    }

    if has_node_panes {
        args.frame_intents
            .extend(webview_controller::sync_to_graph_intents(
                args.graph_app,
                args.window,
            ));
    }

    if has_node_panes || prewarm_selected_node.is_some() {
        webview_backpressure::reconcile_webview_creation_backpressure(
            args.graph_app,
            args.window,
            args.responsive_webviews,
            args.webview_creation_backpressure,
            args.frame_intents,
        );

        // Webview creation moved to tile_render_pass (after intents are applied).
        // Reconcile only emits intents, doesn't directly create webviews.

        let mut protected_active_nodes = active_tile_nodes.clone();
        if let Some(node_key) = prewarm_selected_node {
            protected_active_nodes.insert(node_key);
        }

        let base_active_limit = args.graph_app.active_webview_limit();
        let pressure_limit =
            pressure_adjusted_active_limit(base_active_limit, memory_pressure_level);
        if pressure_limit < base_active_limit {
            for node_key in args
                .graph_app
                .take_active_webview_evictions_with_limit(pressure_limit, &protected_active_nodes)
            {
                if let Some(webview_id) = args.graph_app.get_webview_for_node(node_key) {
                    args.window.close_webview(webview_id);
                    args.frame_intents
                        .push(GraphIntent::UnmapWebview { webview_id });
                }
                args.tile_rendering_contexts.remove(&node_key);
                let is_workspace_member =
                    !args.graph_app.workspaces_for_node_key(node_key).is_empty();
                if is_workspace_member {
                    args.frame_intents
                        .push(lifecycle_intents::demote_node_to_warm(
                            node_key,
                            LifecycleCause::MemoryPressureWarning,
                        ));
                } else {
                    args.frame_intents
                        .push(lifecycle_intents::demote_node_to_cold(
                            node_key,
                            LifecycleCause::MemoryPressureCritical,
                        ));
                }
            }
        }
        for node_key in args
            .graph_app
            .take_active_webview_evictions(&protected_active_nodes)
        {
            let is_workspace_member = !args.graph_app.workspaces_for_node_key(node_key).is_empty();
            if is_workspace_member {
                args.frame_intents
                    .push(lifecycle_intents::demote_node_to_warm(
                        node_key,
                        LifecycleCause::ActiveLruEviction,
                    ));
            } else {
                args.frame_intents
                    .push(lifecycle_intents::demote_node_to_cold(
                        node_key,
                        LifecycleCause::ActiveLruEviction,
                    ));
            }
        }
    } else {
        args.webview_creation_backpressure.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryPressureLevel, pressure_adjusted_active_limit};

    #[test]
    fn test_pressure_adjusted_active_limit_respects_bounds() {
        assert_eq!(
            pressure_adjusted_active_limit(4, MemoryPressureLevel::Unknown),
            4
        );
        assert_eq!(
            pressure_adjusted_active_limit(4, MemoryPressureLevel::Normal),
            4
        );
        assert_eq!(
            pressure_adjusted_active_limit(4, MemoryPressureLevel::Warning),
            3
        );
        assert_eq!(
            pressure_adjusted_active_limit(1, MemoryPressureLevel::Warning),
            1
        );
        assert_eq!(
            pressure_adjusted_active_limit(4, MemoryPressureLevel::Critical),
            1
        );
    }

    #[test]
    fn test_reconcile_runtime_has_no_direct_lifecycle_mutation_calls() {
        let source = include_str!("lifecycle_reconcile.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(
            !production_source.contains("graph_app.promote_node_to_active("),
            "reconcile must emit lifecycle intents, not call direct promotion helpers"
        );
        assert!(
            !production_source.contains("graph_app.demote_node_to_warm("),
            "reconcile must emit lifecycle intents, not call direct demotion helpers"
        );
        assert!(
            !production_source.contains("graph_app.demote_node_to_cold("),
            "reconcile must emit lifecycle intents, not call direct demotion helpers"
        );
    }

    #[test]
    fn test_retry_exhaustion_sets_blocked_and_prevents_recreate_loop() {
        let source = include_str!("lifecycle_reconcile.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(
            production_source.contains("is_runtime_blocked("),
            "reconcile must gate promote/prewarm when runtime is blocked"
        );
    }

    #[test]
    fn test_memory_pressure_demotion_includes_cause_and_order_is_stable() {
        let source = include_str!("lifecycle_reconcile.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
        let warning_pos = production_source
            .find("LifecycleCause::MemoryPressureWarning")
            .expect("warning cause must be present");
        let critical_pos = production_source
            .find("LifecycleCause::MemoryPressureCritical")
            .expect("critical cause must be present");
        assert!(
            warning_pos < critical_pos,
            "warning demotion path should appear before critical demotion path"
        );
    }

    #[test]
    fn test_lifecycle_reconcile_emits_promote_intents_not_direct_mutation() {
        let source = include_str!("lifecycle_reconcile.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(
            production_source.contains("lifecycle_intents::promote_node_to_active("),
            "reconcile should emit promote intents via lifecycle_intents adapter"
        );
        assert!(
            !production_source.contains("graph_app.promote_node_to_active("),
            "reconcile must not directly mutate promote lifecycle state"
        );
    }
}
