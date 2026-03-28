/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Gopher protocol support for Verso.
//!
//! [`server`] — `GopherCapsuleServer`: serve Graphshell content as a
//! Gopher capsule over plain TCP (RFC 1436).

pub(crate) mod server;

pub(crate) use server::{
    GopherCapsuleServer, GopherRegistry, GopherServedNode, GopherServerConfig, GopherServerHandle,
};
