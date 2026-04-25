/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(crate) mod registry;
// 2026-04-25 servo-into-verso S2b: these handlers register with
// Servo's URL scheme infrastructure; gated together with
// servo-engine. The `registry` module above is host-neutral vocab
// (ProtocolRegistration, etc.) and stays available unconditionally.
#[cfg(feature = "servo-engine")]
pub(crate) mod resource;
#[cfg(feature = "servo-engine")]
pub(crate) mod router;
#[cfg(feature = "servo-engine")]
pub(crate) mod servo;
#[cfg(feature = "servo-engine")]
pub(crate) mod urlinfo;
