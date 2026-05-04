/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 67: portable viewer registry types moved to register-viewer.
// What stays in this file: the `EmbeddedViewer*` trait family and
// `SettingsViewer` / `FallbackViewer` impls — all egui-host-gated
// because they reference `crate::app::GraphIntent` /
// `crate::app::AppCommand` / `crate::prefs::FileAccessPolicy` /
// `crate::shell::desktop::workbench::*`. They retire with the
// egui-host build.

use std::collections::HashMap;

// Re-export the portable viewer types from register-viewer at the
// original paths so existing call sites resolve unchanged.
pub(crate) use register_viewer::{
    ViewerCapability, ViewerDescriptor, ViewerHandler, ViewerRegistry, ViewerRenderMode,
    ViewerSelection, ViewerSubsystemCapabilities, VIEWER_ID_FALLBACK,
};

// ---------------------------------------------------------------------------
// EmbeddedViewer — trait-dispatched rendering for non-composited viewers
// ---------------------------------------------------------------------------

/// Outcome of a single `EmbeddedViewer::render` call.
///
/// Viewers that need to emit graph intents (e.g. `NavigateTo` from a markdown
/// link, or `SetNodeUrl` from a directory click) return them here so the tile
/// behavior can queue them without the viewer holding a mutable reference to
/// `GraphBrowserApp`.
#[cfg(feature = "egui-host")]
pub(crate) struct EmbeddedViewerOutput {
    pub(crate) intents: Vec<crate::app::GraphIntent>,
    pub(crate) app_commands: Vec<crate::app::AppCommand>,
}

#[cfg(feature = "egui-host")]
impl EmbeddedViewerOutput {
    pub(crate) fn empty() -> Self {
        Self {
            intents: Vec::new(),
            app_commands: Vec::new(),
        }
    }
}

/// Read-only rendering context passed to each `EmbeddedViewer::render` call.
#[cfg(feature = "egui-host")]
pub(crate) struct EmbeddedViewerContext<'a> {
    pub(crate) node_key: crate::graph::NodeKey,
    pub(crate) node_url: &'a str,
    pub(crate) mime_hint: Option<&'a str>,
    pub(crate) file_access_policy: &'a crate::prefs::FileAccessPolicy,
}

/// Trait for viewers that render directly into an egui `Ui`.
///
/// Each concrete viewer owns its own per-node state (cached directory listings,
/// async image decode handles, etc.) and is dispatched through the
/// `EmbeddedViewerRegistry` rather than an inline `if/else` chain.
#[cfg(feature = "egui-host")]
pub(crate) trait EmbeddedViewer {
    fn viewer_id(&self) -> &'static str;
    fn render(&self, ui: &mut egui::Ui, ctx: &EmbeddedViewerContext<'_>) -> EmbeddedViewerOutput;
}

/// Registry mapping viewer IDs to concrete `EmbeddedViewer` trait objects.
#[cfg(feature = "egui-host")]
pub(crate) struct EmbeddedViewerRegistry {
    viewers: HashMap<&'static str, Box<dyn EmbeddedViewer + Send + Sync>>,
}

#[cfg(feature = "egui-host")]
impl EmbeddedViewerRegistry {
    pub(crate) fn new() -> Self {
        Self {
            viewers: HashMap::new(),
        }
    }

    pub(crate) fn register(&mut self, viewer: Box<dyn EmbeddedViewer + Send + Sync>) {
        let id = viewer.viewer_id();
        self.viewers.insert(id, viewer);
    }

    pub(crate) fn get(&self, viewer_id: &str) -> Option<&(dyn EmbeddedViewer + Send + Sync)> {
        self.viewers.get(viewer_id).map(|v| v.as_ref())
    }

    /// Build the default registry with all built-in embedded viewers.
    pub(crate) fn default_with_viewers() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(SettingsViewer));
        registry.register(Box::new(super::super::viewers::MiddleNetEmbeddedViewer));
        registry.register(Box::new(super::super::viewers::PlaintextEmbeddedViewer));
        registry.register(Box::new(super::super::viewers::ImageEmbeddedViewer));
        registry.register(Box::new(super::super::viewers::DirectoryEmbeddedViewer));
        #[cfg(feature = "pdf")]
        registry.register(Box::new(super::super::viewers::PdfEmbeddedViewer));
        #[cfg(feature = "audio")]
        registry.register(Box::new(super::super::viewers::AudioEmbeddedViewer));
        registry.register(Box::new(FallbackViewer));
        registry
    }
}

/// Settings viewer — delegates to the settings/history render surfaces.
#[cfg(feature = "egui-host")]
struct SettingsViewer;
#[cfg(feature = "egui-host")]
impl EmbeddedViewer for SettingsViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:settings"
    }
    fn render(&self, _ui: &mut egui::Ui, _ctx: &EmbeddedViewerContext<'_>) -> EmbeddedViewerOutput {
        // Settings rendering requires access to GraphBrowserApp and is handled
        // specially in tile_behavior dispatch; this trait impl exists so the
        // viewer ID is recognized by the registry.
        EmbeddedViewerOutput::empty()
    }
}

/// Fallback / metadata viewer — shown when no dedicated viewer is registered.
#[cfg(feature = "egui-host")]
struct FallbackViewer;
#[cfg(feature = "egui-host")]
impl EmbeddedViewer for FallbackViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:fallback"
    }
    fn render(&self, ui: &mut egui::Ui, ctx: &EmbeddedViewerContext<'_>) -> EmbeddedViewerOutput {
        ui.colored_label(
            egui::Color32::from_rgb(220, 180, 60),
            "No dedicated viewer is available for this content yet.",
        );
        ui.label(format!("URL: {}", ctx.node_url));
        if let Some(mime_hint) = ctx.mime_hint {
            ui.small(format!("Detected content type: {mime_hint}"));
        } else {
            ui.small("Detected content type: unknown");
        }
        ui.small(
            "Recovery: switch to WebView for compatibility content, or keep this node as a graph-backed placeholder until a native viewer lands.",
        );
        EmbeddedViewerOutput::empty()
    }
}
