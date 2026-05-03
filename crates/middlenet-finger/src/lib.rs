/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Finger (`finger://`) protocol response parser.
//!
//! Finger ([RFC 1288](https://datatracker.ietf.org/doc/html/rfc1288))
//! returns free-form line-shaped text — typically a multi-line
//! biography, a `.plan` file, a `.project` file, or office-hours.
//! There's no structural protocol payload beyond that, so the
//! Finger parser delegates to the plain-text parser.
//!
//! This crate exists for protocol-identity in the middlenet family
//! (so `finger://` traffic resolves to `middlenet-finger` rather
//! than `middlenet-plain-text`) — it's a one-line wrapper today,
//! but a future slice can add finger-specific enrichment (auto-link
//! `mailto:` addresses, recognise `.plan`-shaped headers, etc.)
//! without disturbing the plain-text parser.

use middlenet_core::document::SemanticDocument;
use middlenet_core::source::MiddleNetSource;

/// Parse a finger response body. Currently delegates to
/// [`middlenet_plain_text::parse_plain_text`]; the wrapper exists
/// so finger-specific enrichment (link-recognition, etc.) can land
/// here in a follow-up slice without disturbing plain-text callers.
pub fn parse_finger(source: &MiddleNetSource, body: &str) -> SemanticDocument {
    middlenet_plain_text::parse_plain_text(source, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::document::SemanticBlock;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn parses_finger_response_lines_as_paragraphs() {
        let source = MiddleNetSource::new(MiddleNetContentKind::FingerText);
        let document = parse_finger(
            &source,
            "Login: alice\nPlan: capsule@example.com\n",
        );
        assert!(matches!(
            document.blocks.first(),
            Some(SemanticBlock::Paragraph(text)) if text.starts_with("Login")
        ));
    }
}
