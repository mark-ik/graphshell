/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Theme tokens — centralized color, radius, opacity, and shadow
//! constants the `gs::*` widgets and the iced host share.
//!
//! iced's `Theme::palette()` already provides the canonical palette
//! (background / primary / secondary / success / warning / danger);
//! this module supplements it with tokens that aren't theme-derived
//! today: hover overlay tints, scrim opacity, destructive text
//! color, panel-corner radii, and chrome opacity bands. When a real
//! `GraphshellTheme` extension lands (Stage F per the iced jump-ship
//! plan), these tokens become methods on the theme — call sites
//! don't change shape.
//!
//! Putting tokens in `graphshell-iced-widgets` (rather than the
//! shell crate) keeps them portable: future iced-consuming crates
//! (canvas viewers, content viewers, Stage-G/H hosts) get the same
//! tokens via the same dep.

use iced::Color;

// ---------------------------------------------------------------------------
// Overlay tints — additive whites for hover / pressed / focus state.
// ---------------------------------------------------------------------------

/// Subtle hover-state overlay (e.g., palette rows, tree-spine rows,
/// swatch cards). Low alpha so the underlying color reads through.
pub const HOVER_OVERLAY_SUBTLE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.05);

/// Stronger hover overlay for buttons and tab handles.
pub const HOVER_OVERLAY_STRONG: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.08);

/// Pressed-state overlay (when the user is mid-click).
pub const PRESSED_OVERLAY: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.15);

// ---------------------------------------------------------------------------
// Modal scrim — the translucent veil behind centered modals.
// ---------------------------------------------------------------------------

/// Standard modal scrim (palette / finder / confirm dialog / node
/// create / frame rename). 50% black absorbs the underlying chrome
/// without obscuring it.
pub const MODAL_SCRIM: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);

// ---------------------------------------------------------------------------
// Destructive text — used for Tombstone-shaped context-menu entries.
// ---------------------------------------------------------------------------

/// Red-tinted text for destructive actions. Independent of theme
/// palette so destructive operations always stand out, even on
/// themes that bend the danger palette in unusual directions.
pub const DESTRUCTIVE_TEXT: Color = Color::from_rgb(0.8, 0.2, 0.2);

// ---------------------------------------------------------------------------
// Border radii — corner rounding bands.
// ---------------------------------------------------------------------------

/// Tight rounding for inline rows (tree-spine row buttons, palette
/// rows, swatch labels).
pub const RADIUS_TIGHT: f32 = 2.0;

/// Standard button rounding (status-bar buttons, dialog buttons,
/// frame switcher tabs).
pub const RADIUS_BUTTON: f32 = 3.0;

/// Tab-handle rounding (TileTabs).
pub const RADIUS_TAB: f32 = 4.0;

/// Modal card rounding (gs::Modal).
pub const RADIUS_MODAL: f32 = 8.0;

// ---------------------------------------------------------------------------
// Chrome bands — alpha values for background overlays of small
// chrome elements that sit on top of the theme's base background.
// ---------------------------------------------------------------------------

/// Faintest chrome band — swatch cards, list-row hovers.
pub const CHROME_BAND_FAINT: f32 = 0.03;

/// Medium chrome band — frame switcher bar, secondary chrome rows.
pub const CHROME_BAND_MEDIUM: f32 = 0.04;

/// Standard chrome band — the StatusBar.
pub const CHROME_BAND_BASE: f32 = 0.05;

/// Build a chrome-band color: the theme's base text color at the
/// supplied alpha. Used by chrome containers that want to tint
/// faintly on top of the theme background.
pub fn chrome_band(theme_text: Color, alpha: f32) -> Color {
    Color {
        a: alpha,
        ..theme_text
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_alphas_are_monotonic() {
        // Hover overlays should be ordered subtle < strong < pressed
        // so the visual ladder reads consistently.
        assert!(HOVER_OVERLAY_SUBTLE.a < HOVER_OVERLAY_STRONG.a);
        assert!(HOVER_OVERLAY_STRONG.a < PRESSED_OVERLAY.a);
    }

    #[test]
    fn radii_are_ordered() {
        assert!(RADIUS_TIGHT < RADIUS_BUTTON);
        assert!(RADIUS_BUTTON < RADIUS_TAB);
        assert!(RADIUS_TAB < RADIUS_MODAL);
    }

    #[test]
    fn chrome_band_preserves_color_channels() {
        let text = Color::from_rgb(0.9, 0.85, 0.8);
        let banded = chrome_band(text, 0.04);
        assert_eq!(banded.r, text.r);
        assert_eq!(banded.g, text.g);
        assert_eq!(banded.b, text.b);
        assert_eq!(banded.a, 0.04);
    }
}
