/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// `host` is the legacy Servo+egui embedder. `render_backend` is lower
// Servo/wgpu plumbing and can compile independently of egui.
#[cfg(all(feature = "servo-engine", feature = "egui-host"))]
pub(crate) mod host;
pub(crate) mod lifecycle;
#[cfg(feature = "servo-engine")]
pub(crate) mod render_backend;
pub(crate) mod runtime;
pub(crate) mod ui;
pub(crate) mod workbench;

#[cfg(all(test, feature = "servo-engine", feature = "egui-host"))]
mod tests;
