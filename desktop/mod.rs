/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Contains files specific to the servoshell app for Desktop systems.

mod accelerated_gl_media;
pub(crate) mod app;
pub(crate) mod cli;
pub(crate) mod dialog;
mod dialog_panels;
#[cfg(feature = "diagnostics")]
pub(crate) mod diagnostics;
pub(crate) mod embedder;
pub(crate) mod event_loop;
#[cfg(feature = "gamepad")]
pub(crate) mod gamepad;
pub mod geometry;
mod graph_search_flow;
mod graph_search_ui;
pub(crate) mod gui;
mod gui_frame;
pub(crate) mod headed_window;
pub(crate) mod headless_window;
mod keyutils;
mod lifecycle_intents;
mod lifecycle_reconcile;
mod nav_targeting;
pub(crate) mod persistence_ops;
mod protocols;
#[cfg(feature = "diagnostics")]
pub(crate) mod registries;
mod semantic_event_pipeline;
mod selection_range;
mod thumbnail_pipeline;
mod tile_behavior;
mod tile_compositor;
mod tile_grouping;
mod tile_invariants;
pub(crate) mod tile_kind;
mod tile_post_render;
mod tile_render_pass;
mod tile_runtime;
mod tile_view_ops;
mod toolbar_routing;
mod toolbar_ui;
mod tracing;
mod webview_backpressure;
mod webview_controller;
mod webview_status_sync;
#[cfg(feature = "webxr")]
mod webxr;

#[cfg(test)]
mod tests;
