/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Portable MiddleNet engine scaffolding.
//!
//! This module is the first extraction seam for the future portable web core.
//! It intentionally starts with the format-agnostic document model already used
//! by the Gemini/Gopher/Finger paths, plus source metadata that future protocol
//! adapters can share without depending on Servo or host-native viewers.

pub mod document;
pub mod adapters;
pub mod capabilities;
pub mod engine;
pub mod identity;
pub mod misfin;
pub mod source;
pub mod transport;
pub mod webfinger;

// Phase 2: Engine Stack Scaffolding
pub mod dom;
pub mod style;
pub mod layout;
pub mod compositor;
pub mod script;


pub mod viewer;
