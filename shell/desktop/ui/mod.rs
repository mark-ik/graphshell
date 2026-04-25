/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod command_palette_state;
pub(crate) mod command_surface_telemetry;
// 2026-04-25 servo-into-verso S2b: dialog + egui-host modules
// surface Servo embedder events through egui widgets. Gated with
// servo-engine since the egui-host path is the only consumer; the
// iced-host path uses its own surfaces (iced_host*, gui_state).
#[cfg(feature = "servo-engine")]
pub(crate) mod dialog;
#[cfg(feature = "servo-engine")]
pub(crate) mod dialog_panels;
#[cfg(feature = "servo-engine")]
pub(crate) mod egui_host_ports;
// 2026-04-25 servo-into-verso S2b: these UI flow files all consume
// gated `gui` / `gui_orchestration` / `persistence_ops`. Gated with
// servo-engine until the egui-host UI flow is decoupled from the
// shared finalize/search/palette paths in S3.
#[cfg(feature = "servo-engine")]
pub(crate) mod finalize_actions;
#[cfg(feature = "servo-engine")]
pub(crate) mod graph_search_flow;
#[cfg(feature = "servo-engine")]
pub(crate) mod graph_search_ui;
#[cfg(feature = "servo-engine")]
pub(crate) mod gui;
#[cfg(feature = "servo-engine")]
pub(crate) mod gui_frame;
#[cfg(feature = "servo-engine")]
pub(crate) mod gui_orchestration;
// 2026-04-25 servo-into-verso S2b: gui_state holds GraphshellRuntime
// which threads through host_ports, webview_backpressure, and
// compositor_adapter (all gated). S3 will extract a host-neutral
// runtime surface; for now the whole module is gated.
#[cfg(feature = "servo-engine")]
pub(crate) mod gui_state;
// 2026-04-25 servo-into-verso S3a: host_ports is now a thin
// re-export shim over the trait surface in `graphshell-runtime`.
// The Servo-specific WebViewId converter helper inside is itself
// gated behind servo-engine, but the file as a whole is host-neutral
// and reachable from both egui and iced launch paths.
pub(crate) mod host_ports;
// nav_targeting / persistence_ops / toolbar* / toolbar_routing /
// workbench_host all consume Servo embedder, render_backend, or
// compositor_adapter types and only run on the Servo+egui-host
// path. Gated together until they're refactored.
// 2026-04-25 servo-into-verso S2b/S3b.1: iced launch path modules.
// `iced_host_ports` is fully decoupled from Servo-coupled modules
// after S3a (trait extraction → graphshell-runtime) + S3b.1
// (CachedTexture relocation), so it ships under just `iced-host`.
// The remaining iced launch path (iced_app, iced_host,
// iced_graph_canvas, iced_events, iced_middlenet_viewer) still
// consumes `gui_state::GraphshellRuntime` (gated) so they require
// both features for now. Extracting GraphshellRuntime is the next
// S3b slice; until then, the iced no-Servo path is via the
// standalone `crates/iced-{middlenet,graph-canvas,wry}-viewer`
// demo crates.
#[cfg(feature = "iced-host")]
pub(crate) mod iced_host_ports;
#[cfg(all(feature = "iced-host", feature = "servo-engine"))]
pub(crate) mod iced_app;
#[cfg(all(feature = "iced-host", feature = "servo-engine"))]
pub(crate) mod iced_events;
#[cfg(all(feature = "iced-host", feature = "servo-engine"))]
pub(crate) mod iced_graph_canvas;
#[cfg(all(feature = "iced-host", feature = "servo-engine"))]
pub(crate) mod iced_host;
#[cfg(all(feature = "iced-host", feature = "servo-engine"))]
pub(crate) mod iced_middlenet_viewer;
#[cfg(all(feature = "iced-host", feature = "servo-engine", test))]
pub(crate) mod iced_parity;
#[cfg(feature = "servo-engine")]
pub(crate) mod nav_targeting;
pub(crate) mod navigator_context;
pub(crate) mod omnibar_state;
#[cfg(feature = "servo-engine")]
pub(crate) mod overview_plane;
pub(crate) mod portable_time;
#[cfg(feature = "servo-engine")]
pub(crate) mod persistence_ops;
#[cfg(feature = "servo-engine")]
pub(crate) mod shell_layout_pass;
pub(crate) mod swatch;
pub(crate) mod tag_panel;
// 2026-04-25 servo-into-verso S2b: thumbnail capture pulls Servo
// screenshot frames; gated together with servo-engine.
#[cfg(feature = "servo-engine")]
pub(crate) mod thumbnail_pipeline;
#[cfg(feature = "servo-engine")]
pub(crate) mod toolbar;
#[cfg(feature = "servo-engine")]
pub(crate) mod toolbar_routing;
pub(crate) mod undo_boundary;
#[cfg(feature = "servo-engine")]
pub(crate) mod workbench_host;
