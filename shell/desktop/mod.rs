/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// 2026-04-25 servo-into-verso S2b: `host` and `render_backend` are
// entirely Servo-coupled (Servo embedder / RenderingContextCore /
// wgpu device acquisition). Gated together with `servo-engine` so a
// no-Servo build (e.g. `iced-host` only) compiles without dragging
// in these modules. Per the plan doc §3, when servo-engine is off
// the iced-host launch path returns before reaching any consumers
// of these modules.
#[cfg(feature = "servo-engine")]
pub(crate) mod host;
pub(crate) mod lifecycle;
#[cfg(feature = "servo-engine")]
pub(crate) mod render_backend;
pub(crate) mod runtime;
pub(crate) mod ui;
pub(crate) mod workbench;

#[cfg(test)]
mod tests;
