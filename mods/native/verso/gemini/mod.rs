/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Gemini protocol support for Verso.
//!
//! - [`server`] — `GeminiCapsuleServer`: serve Graphshell content as a
//!   Gemini capsule over TCP + TLS.

pub(crate) mod server;

pub(crate) use server::{
    CapsuleRegistry, GeminiCapsuleServer, GeminiServerConfig, GeminiServerHandle, ServedNode,
};
