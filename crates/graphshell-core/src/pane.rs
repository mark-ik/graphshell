/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Portable workbench-pane identity + render-mode classification.
//!
//! [`PaneId`] is the canonical pane identity used by the pane-hosted
//! view architecture — distinct from `egui_tiles::TileId` (host layout
//! identity) and the legacy `u64` persistence key that predated it.
//!
//! [`TileRenderMode`] classifies how a pane's viewer produces pixels
//! — a composited offscreen texture (Servo), an OS-native overlay
//! window (Wry), direct egui rendering, or a placeholder when the
//! viewer is unresolved. The runtime uses this to drive compositor
//! routing decisions; hosts read it when selecting a surface backend.
//!
//! Both types are fully portable: `PaneId` wraps a UUIDv4, and
//! `TileRenderMode` is a 4-variant enum with `serde` derives. Pre-M4
//! slice 8 (2026-04-22) they lived in
//! `shell/desktop/workbench/pane_model.rs`, alongside larger
//! shell-coupled types that remain shell-side; the shell module
//! re-exports these two at their original paths.

use serde::{Deserialize, Serialize};

/// Opaque stable identifier for a workbench pane.
///
/// Distinct from `egui_tiles::TileId` (layout tree identity) and the
/// legacy `u64` persistence key. This is the canonical pane identity
/// for the pane-hosted view architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(uuid::Uuid);

impl PaneId {
    /// Construct a fresh random pane identity (UUIDv4).
    ///
    /// Gated to non-WASM targets because `Uuid::new_v4()` pulls in an
    /// OS randomness source unavailable on `wasm32-unknown-unknown`.
    /// WASM hosts should obtain the UUID from the host runtime (e.g.
    /// `crypto.randomUUID()`) and construct via [`PaneId::from_uuid`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Construct a `PaneId` from an existing UUID. Prefer this on
    /// WASM targets where OS randomness is unavailable.
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    /// Borrow the underlying UUID. Useful for serialising or
    /// rendering; does not expose any mutation point.
    pub fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for PaneId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pane:{}", self.0)
    }
}

/// Node viewer pane render classification.
///
/// The runtime's `pane_render_modes` map uses this to decide which
/// compositor/backend path drives each pane. Hosts render accordingly:
///
/// - `CompositedTexture` → read the pane's composited texture and
///   blit into the layout slot (Servo's standard path).
/// - `NativeOverlay` → position an OS-native subwindow at the pane's
///   rect (Wry's path on platforms that prefer native viewports).
/// - `EmbeddedEgui` → draw directly in the egui layer (used for
///   fallback/placeholder viewers that don't need a separate surface).
/// - `Placeholder` → nothing to render yet; the pane is reserved.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize,
)]
pub enum TileRenderMode {
    /// Viewer renders to a Graphshell-owned composited texture (e.g. Servo).
    CompositedTexture,
    /// Viewer uses an OS-native overlay window (e.g. Wry).
    NativeOverlay,
    /// Viewer renders directly into egui UI.
    EmbeddedEgui,
    /// Viewer is unavailable or unresolved for this pane.
    #[default]
    Placeholder,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_id_display_uses_pane_prefix() {
        let pid = PaneId::from_uuid(uuid::uuid!("00000000-0000-0000-0000-000000000001"));
        assert_eq!(
            pid.to_string(),
            "pane:00000000-0000-0000-0000-000000000001"
        );
    }

    #[test]
    fn pane_id_from_uuid_preserves_value() {
        let uuid = uuid::uuid!("11111111-2222-3333-4444-555555555555");
        let pid = PaneId::from_uuid(uuid);
        assert_eq!(pid.as_uuid(), &uuid);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn pane_id_new_produces_distinct_values() {
        let a = PaneId::new();
        let b = PaneId::new();
        // UUIDv4 collision probability is ~0 for any practical
        // consecutive calls. Pin the sanity check.
        assert_ne!(a, b);
    }

    #[test]
    fn pane_id_serde_round_trip() {
        let original = PaneId::from_uuid(uuid::uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
        let json = serde_json::to_string(&original).unwrap();
        let decoded: PaneId = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn tile_render_mode_default_is_placeholder() {
        // The default matters: a freshly-discovered pane with no
        // viewer resolution yet should not be treated as composited.
        // Pin that default stays Placeholder across any enum-reorder.
        assert_eq!(TileRenderMode::default(), TileRenderMode::Placeholder);
    }

    #[test]
    fn tile_render_mode_serde_uses_variant_names() {
        // The render-mode is persisted in session state via serde; its
        // discriminants must remain stable or existing persisted
        // sessions fail to deserialise after an update. Pin the wire
        // shape for each variant.
        for (variant, expected) in [
            (TileRenderMode::CompositedTexture, "\"CompositedTexture\""),
            (TileRenderMode::NativeOverlay, "\"NativeOverlay\""),
            (TileRenderMode::EmbeddedEgui, "\"EmbeddedEgui\""),
            (TileRenderMode::Placeholder, "\"Placeholder\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected);
            let back: TileRenderMode = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }
}
