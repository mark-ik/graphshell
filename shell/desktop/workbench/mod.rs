/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// 2026-04-25 servo-into-verso S2b: `compositor_adapter` consumes
// Servo's compositor + RenderingContextCore types directly.
#[cfg(feature = "servo-engine")]
pub(crate) mod compositor_adapter;
#[cfg(feature = "egui-host")]
pub(crate) mod graph_tree_adapter;
pub(crate) mod graph_tree_binding;
pub(crate) mod graph_tree_commands;
// 2026-04-25 servo-into-verso S2b: graph_tree_dual_write consumes
// tile_view_ops (gated above) directly. Gated together.
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod graph_tree_dual_write;
#[cfg(feature = "egui-host")]
pub(crate) mod graph_tree_facade;
// 2026-04-25 servo-into-verso S2b: graph_tree_projection consumes
// gated `workbench_host` (egui-host workbench surface).
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod graph_tree_projection;
#[cfg(feature = "egui-host")]
pub(crate) mod graph_tree_sync;
pub(crate) mod interaction_policy;
pub(crate) mod local_file_access;
pub(crate) mod pane_model;
pub(crate) mod selection_range;
// 2026-04-25 servo-into-verso S2b: semantic_tabs consumes gated
// `persistence_ops` and `gui_state`.
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod semantic_tabs;
// 2026-04-25 servo-into-verso S2b: tile_behavior + tile_compositor +
// tile_invariants + tile_post_render all consume the gated tile_*
// or render_backend or compositor_adapter modules (Servo render
// pipeline). Gated together.
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod tile_behavior;
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod tile_compositor;
#[cfg(feature = "egui-host")]
pub(crate) mod tile_grouping;
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod tile_invariants;
pub(crate) mod tile_kind;
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod tile_post_render;
// 2026-04-25 servo-into-verso S2b: these tile pipeline modules
// thread Servo's WebView / paint result types through; gated
// together with `servo-engine`.
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod tile_render_pass;
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod tile_runtime;
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod tile_view_ops;
#[cfg(feature = "ux-bridge")]
pub(crate) mod ux_bridge;
// 2026-04-25 servo-into-verso S2b: ux_probes consumes gated toolbar.
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod ux_probes;
#[cfg(feature = "egui-host")]
pub(crate) mod ux_replay;
pub(crate) mod ux_tree;
pub(crate) mod ux_tree_source;
