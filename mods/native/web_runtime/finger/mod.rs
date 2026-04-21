/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Finger protocol support for Verso.
//!
//! [`server`] — `FingerServer`: serve personal profile content as plain
//! text over plain TCP (RFC 1288).

pub(crate) mod server;

pub(crate) use server::{
    FingerProfile, FingerRegistry, FingerServer, FingerServerConfig, FingerServerHandle,
};
