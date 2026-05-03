/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Scroll protocol parser — **scaffold crate**.
//!
//! [Scroll](https://scroll.mozz.us) is a small-web protocol whose
//! responses combine a binary header (sender / signature / timestamp
//! / content-type) with a body in a content-type-determined format
//! (typically gemtext or markdown). The parser concerns are
//! envelope-decode (header parse + signature verify) plus body
//! delegation to the appropriate body-format parser
//! ([`middlenet-gemini`](../middlenet-gemini/index.html) /
//! [`middlenet-markdown`](../middlenet-markdown/index.html)).
//!
//! Slice 61 scaffold: returns `Err` for now. A future
//! implementation owns the binary-envelope decode and dispatches
//! the body to the format-specific parser.

use middlenet_core::document::SemanticDocument;
use middlenet_core::source::MiddleNetSource;

pub fn parse_scroll(_source: &MiddleNetSource, _body: &str) -> Result<SemanticDocument, String> {
    Err(
        "middlenet-scroll: parser not yet implemented (Slice 61 scaffold). \
         Scroll responses combine a signed binary envelope with a \
         content-type-determined body; full impl decodes the envelope \
         and delegates the body to the appropriate parser."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn scaffold_returns_not_implemented_error() {
        let source = MiddleNetSource::new(MiddleNetContentKind::ScrollDocument);
        assert!(parse_scroll(&source, "# Body\n").is_err());
    }
}
