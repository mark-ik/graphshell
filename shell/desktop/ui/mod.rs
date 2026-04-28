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
// `finalize_actions` is now ungated: it uses cfg blocks internally to
// delegate to `gui_orchestration` (servo-engine) or call runtime
// helpers directly (iced-only path).
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
// `gui_frame_inbox` is ungated — iced-host and egui-host both use it.
pub(crate) mod gui_frame_inbox;
// `gui_state` is now ungated: Servo-coupled fields (viewer_surfaces,
// viewer_surface_host) are gated inside the file on servo-engine.
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
// 2026-04-25 S3b.1 / Lane 5a: iced launch path modules are now
// ungated from servo-engine.  gui_state is portable; Servo-coupled
// fields are gated inside it.  iced_host's IcedWgpuContext block is
// gated on servo-engine inside the file.
#[cfg(feature = "iced-host")]
pub(crate) mod iced_app;
#[cfg(feature = "iced-host")]
pub(crate) mod iced_events;
#[cfg(feature = "iced-host")]
pub(crate) mod iced_graph_canvas;
#[cfg(feature = "iced-host")]
pub(crate) mod iced_host;
#[cfg(feature = "iced-host")]
pub(crate) mod iced_host_ports;
#[cfg(feature = "iced-host")]
pub(crate) mod iced_middlenet_viewer;
#[cfg(all(feature = "iced-host", test))]
pub(crate) mod iced_parity;
#[cfg(feature = "servo-engine")]
pub(crate) mod nav_targeting;
pub(crate) mod navigator_context;
pub(crate) mod omnibar_state;
#[cfg(feature = "servo-engine")]
pub(crate) mod overview_plane;
#[cfg(feature = "servo-engine")]
pub(crate) mod persistence_ops;
pub(crate) mod portable_time;
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
