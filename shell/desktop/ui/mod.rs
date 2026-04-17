/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod dialog;
pub(crate) mod dialog_panels;
pub(crate) mod egui_host_ports;
pub(crate) mod frame_model;
pub(crate) mod graph_search_flow;
pub(crate) mod graph_search_ui;
pub(crate) mod gui;
pub(crate) mod gui_frame;
pub(crate) mod gui_orchestration;
pub(crate) mod gui_state;
pub(crate) mod host_ports;
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
pub(crate) mod nav_targeting;
pub(crate) mod navigator_context;
pub(crate) mod overview_plane;
pub(crate) mod persistence_ops;
pub(crate) mod shell_layout_pass;
pub(crate) mod swatch;
pub(crate) mod tag_panel;
pub(crate) mod thumbnail_pipeline;
pub(crate) mod toolbar;
pub(crate) mod toolbar_routing;
pub(crate) mod undo_boundary;
pub(crate) mod workbench_host;
