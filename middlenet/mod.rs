/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Portable MiddleNet engine scaffolding.
//!
//! This module is the first extraction seam for the future portable web core.
//! It intentionally starts with the format-agnostic document model already used
//! by the Gemini/Gopher/Finger paths, plus source metadata that future protocol
//! adapters can share without depending on Servo or host-native viewers.

pub(crate) mod document;
pub(crate) mod adapters;
pub(crate) mod engine;
pub(crate) mod identity;
pub(crate) mod misfin;
pub(crate) mod source;
pub(crate) mod transport;
pub(crate) mod webfinger;