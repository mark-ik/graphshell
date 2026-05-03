/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Guppy protocol parser — **scaffold crate**.
//!
//! [Guppy](https://guppy.mozz.us) is a UDP-based small-web
//! protocol. Requests fit in a single UDP packet; responses arrive
//! in a sequence of chunked packets that the client reassembles.
//! The reassembled body is gemtext-shaped, so once the transport
//! layer is in place the parser can delegate to
//! [`middlenet-gemini`](../middlenet-gemini/index.html).
//!
//! Slice 61 scaffold: returns `Err` for now. The transport-layer
//! reassembly lives outside this crate (in a future
//! `graphshell-comms::guppy` or the network mod that owns the UDP
//! socket); this crate's `parse_guppy` is the body-shape parser
//! that takes the already-reassembled body string.

use middlenet_core::document::SemanticDocument;
use middlenet_core::source::MiddleNetSource;

pub fn parse_guppy(_source: &MiddleNetSource, _body: &str) -> Result<SemanticDocument, String> {
    Err(
        "middlenet-guppy: parser not yet implemented (Slice 61 scaffold). \
         Guppy is a UDP-based small-web protocol; this crate parses \
         the reassembled body (gemtext-shaped). Reassembly happens \
         outside this crate, in the transport layer."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn scaffold_returns_not_implemented_error() {
        let source = MiddleNetSource::new(MiddleNetContentKind::GuppyText);
        assert!(parse_guppy(&source, "# Body\n").is_err());
    }
}
