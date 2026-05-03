/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Misfin (`misfin://`) protocol parser — **scaffold crate**.
//!
//! [Misfin](https://misfin.org) is a gemini-style peer-to-peer
//! email protocol. Messages have a gemtext body wrapped in a
//! sender/recipient/timestamp envelope; addressing is via
//! `name@domain`-shaped misfin addresses tied to client
//! certificates.
//!
//! A real implementation already exists in
//! [`graphshell-comms::misfin`](../../graphshell-comms/src/misfin.rs)
//! — that's the transport + envelope side. This crate would host
//! the body-shape parser (gemtext via
//! [`middlenet-gemini`](../middlenet-gemini/index.html)) plus the
//! envelope-to-`SemanticDocument`-meta translation. Slice 61
//! scaffold; the bridge to graphshell-comms::misfin lands when the
//! viewer side actually consumes parsed misfin messages through the
//! middlenet adapter family.

use middlenet_core::document::SemanticDocument;
use middlenet_core::source::MiddleNetSource;

pub fn parse_misfin(_source: &MiddleNetSource, _body: &str) -> Result<SemanticDocument, String> {
    Err(
        "middlenet-misfin: parser not yet implemented (Slice 61 scaffold). \
         The transport + envelope side already exists in \
         graphshell-comms::misfin; this crate will host the body-shape \
         adapter + envelope-to-meta translation."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn scaffold_returns_not_implemented_error() {
        let source = MiddleNetSource::new(MiddleNetContentKind::MisfinMessage);
        assert!(parse_misfin(&source, "# Hello\n").is_err());
    }
}
