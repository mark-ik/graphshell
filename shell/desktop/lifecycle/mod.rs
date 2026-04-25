/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod lifecycle_intents;
// 2026-04-25 servo-into-verso S2b: these submodules consume
// Servo embedder / WebView / RenderingContext types directly.
// Gated with servo-engine; lifecycle_intents is host-neutral
// vocab and stays available unconditionally.
#[cfg(feature = "servo-engine")]
pub(crate) mod lifecycle_reconcile;
#[cfg(feature = "servo-engine")]
pub(crate) mod semantic_event_pipeline;
#[cfg(feature = "servo-engine")]
pub(crate) mod webview_backpressure;
#[cfg(feature = "servo-engine")]
pub(crate) mod webview_controller;
#[cfg(feature = "servo-engine")]
pub(crate) mod webview_status_sync;
