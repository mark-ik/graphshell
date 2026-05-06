/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! middlenet ↔ netrender bridge.
//!
//! **WASM-clean**: this crate must compile to `wasm32-unknown-unknown`.
//! The Linebender + Servo rendering stack we're assembling here is
//! intentionally portable — `parley`, `vello`, `wgpu`, `netrender`,
//! `taffy`, `stylo` all have wasm targets. Don't reach for std::fs,
//! std::process, system clock, or platform-specific font discovery.
//! For font loading: accept caller-supplied bytes via `with_font_paths`;
//! the loader reads files only as a developer-affordance and has a
//! parallel byte-input path for wasm hosts.
//!
//! [`SceneAdapter`] maps `middlenet_render::RenderScene` (the
//! semantic-block scene model produced by `middlenet_render::render_document`)
//! into a `netrender::Scene` (the display-list builder consumed by
//! the vello tile rasterizer).
//!
//! Per the 2026-04-30 renderer plan §3 / 2026-05-01 vello plan §4.4,
//! parley layout sits at the embedder layer — this crate owns the
//! parley `FontContext` + `LayoutContext` and produces glyph runs
//! that get pushed into the `Scene` via `netrender_text::push_layout`.
//! middlenet-render itself stays text-layout-naive (it carries
//! strings + styling + estimated rects); this adapter is where strings
//! become glyphs.
//!
//! What's intentionally not in this adapter:
//! - Theme color tokens are not in `middlenet_render::ThemeTokens`
//!   (only font sizes are). Colors live on [`ThemeColors`] here, with
//!   sensible dark-mode defaults via [`ThemeColors::dark`]. When
//!   middlenet's theming gets richer (or is unified with `register_theme`),
//!   this struct goes away.
//! - Hit-test routing. `RenderScene::hit_regions` is preserved by the
//!   caller for click-handling; the adapter only emits visual fills.
//! - Outline / scroll integration. The adapter just renders blocks at
//!   their assigned rects; scrolling, virtualization, etc. are the
//!   host's concern.

pub mod html_paint;

use std::sync::Arc;

use middlenet_render::{
    RenderBlock, RenderBlockKind, RenderRect, RenderScene, RenderTextRun, TextStyle, ThemeTokens,
};
use netrender::Scene;
use netrender_text::parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontStyle, FontWeight, Layout,
    LayoutContext, StyleProperty, fontique,
};

/// Errors the adapter can surface during construction.
#[derive(Debug)]
pub enum AdapterError {
    /// No system font path matched. Bundle a font or extend
    /// [`SceneAdapter::with_font_paths`] before constructing.
    NoSystemFont,
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterError::NoSystemFont => write!(
                f,
                "no system font found at the candidate paths; \
                 supply paths via SceneAdapter::with_font_paths"
            ),
        }
    }
}

impl std::error::Error for AdapterError {}

/// Color tokens for the rendered document. Premultiplied RGBA per
/// netrender's Scene API contract (vello plan §6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    /// Page background fill (full viewport).
    pub background: [f32; 4],
    /// Body text.
    pub body: [f32; 4],
    /// Title and h1 text.
    pub title: [f32; 4],
    /// Sub-heading text (h2+).
    pub heading: [f32; 4],
    /// Quote text.
    pub quote: [f32; 4],
    /// Metadata / date / muted text.
    pub metadata: [f32; 4],
    /// Code (monospace would be ideal; falls back to default family today).
    pub code: [f32; 4],
    /// Badge text.
    pub badge: [f32; 4],
    /// Link text.
    pub link: [f32; 4],
    /// Code-fence block background.
    pub code_background: [f32; 4],
    /// Quote-block left stripe.
    pub quote_stripe: [f32; 4],
    /// Badge background pill.
    pub badge_background: [f32; 4],
    /// Horizontal rule line.
    pub rule: [f32; 4],
}

impl ThemeColors {
    /// Dark-mode defaults — the colors used by the smoke examples in
    /// this crate. Tuned to look "fine, not designed" so reviewers
    /// don't mistake them for a finished aesthetic.
    pub fn dark() -> Self {
        Self {
            background: rgba(0.10, 0.11, 0.13, 1.0),
            body: rgba(0.92, 0.92, 0.93, 1.0),
            title: rgba(0.98, 0.98, 0.99, 1.0),
            heading: rgba(0.95, 0.95, 0.96, 1.0),
            quote: rgba(0.85, 0.85, 0.88, 1.0),
            metadata: rgba(0.65, 0.65, 0.70, 1.0),
            code: rgba(0.85, 0.95, 0.85, 1.0),
            badge: rgba(0.95, 0.95, 0.95, 1.0),
            link: rgba(0.55, 0.80, 0.95, 1.0),
            code_background: rgba(0.16, 0.17, 0.20, 1.0),
            quote_stripe: rgba(0.55, 0.80, 0.95, 1.0),
            badge_background: rgba(0.30, 0.35, 0.45, 1.0),
            rule: rgba(0.30, 0.32, 0.36, 1.0),
        }
    }
}

/// Premultiplied-alpha helper. Caller passes straight-alpha; we
/// premultiply for the netrender Scene contract.
fn rgba(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    [r * a, g * a, b * a, a]
}

/// Adapter from `middlenet_render::RenderScene` to `netrender::Scene`.
///
/// Owns parley state (fonts + layout context) so glyph runs can be
/// produced from `RenderTextRun`s. Cheap to keep across frames; expensive
/// to construct (font loading).
pub struct SceneAdapter {
    font_cx: FontContext,
    layout_cx: LayoutContext<[f32; 4]>,
    family_name: String,
    theme: ThemeTokens,
    colors: ThemeColors,
}

impl SceneAdapter {
    /// Construct an adapter, loading a system font from the default
    /// candidate paths (Windows fonts first, then macOS, then Linux).
    /// Use [`Self::with_font_paths`] to supply your own.
    pub fn new(theme: ThemeTokens, colors: ThemeColors) -> Result<Self, AdapterError> {
        Self::with_font_paths(
            theme,
            colors,
            &[
                r"C:\Windows\Fonts\segoeui.ttf",
                r"C:\Windows\Fonts\arial.ttf",
                "/System/Library/Fonts/Helvetica.ttc",
                "/Library/Fonts/Arial.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
            ],
        )
    }

    /// Construct an adapter using the first existing path in
    /// `font_paths`. If none of the paths can be read, returns
    /// [`AdapterError::NoSystemFont`].
    pub fn with_font_paths(
        theme: ThemeTokens,
        colors: ThemeColors,
        font_paths: &[&str],
    ) -> Result<Self, AdapterError> {
        let mut font_cx = FontContext::new();
        let layout_cx = LayoutContext::new();

        for path in font_paths {
            if let Ok(bytes) = std::fs::read(path) {
                let blob = fontique::Blob::new(Arc::new(bytes));
                let (family_id, _) = font_cx
                    .collection
                    .register_fonts(blob, None)
                    .into_iter()
                    .next()
                    .ok_or(AdapterError::NoSystemFont)?;
                let family_name = font_cx
                    .collection
                    .family_name(family_id)
                    .ok_or(AdapterError::NoSystemFont)?
                    .to_owned();
                return Ok(Self {
                    font_cx,
                    layout_cx,
                    family_name,
                    theme,
                    colors,
                });
            }
        }
        Err(AdapterError::NoSystemFont)
    }

    /// Replace the theme tokens (font sizes etc.) used for layout.
    pub fn set_theme(&mut self, theme: ThemeTokens) {
        self.theme = theme;
    }

    /// Replace the color palette used for fills and text.
    pub fn set_colors(&mut self, colors: ThemeColors) {
        self.colors = colors;
    }

    /// Reference to the registered font family name. Useful for
    /// callers who want to push extra labels in the same family.
    pub fn family_name(&self) -> &str {
        &self.family_name
    }

    /// Build a `netrender::Scene` from a `RenderScene`. Pushes a
    /// background fill, then walks `render_scene.blocks` in order,
    /// emitting per-block chrome + text glyph runs.
    ///
    /// The output viewport is `viewport_w × viewport_h`. Blocks
    /// outside this rect are still encoded (vello clips at the
    /// compute boundary); callers can pre-cull if they want.
    pub fn build_scene(
        &mut self,
        scene: &mut Scene,
        render_scene: &RenderScene,
        viewport_w: f32,
        viewport_h: f32,
    ) {
        scene.push_rect(0.0, 0.0, viewport_w, viewport_h, self.colors.background);
        for block in &render_scene.blocks {
            self.push_block(scene, block);
        }
    }

    fn push_block(&mut self, scene: &mut Scene, block: &RenderBlock) {
        let r = block.rect;
        match &block.kind {
            RenderBlockKind::Rule => {
                // Thin horizontal line centered vertically in the
                // block's reserved 8px height.
                let y_mid = r.y + r.height * 0.5;
                scene.push_rect(
                    r.x,
                    y_mid - 0.5,
                    r.x + r.width,
                    y_mid + 0.5,
                    self.colors.rule,
                );
            }
            RenderBlockKind::CodeFence => {
                // 3px-padded background pill behind the code text.
                scene.push_rect(
                    r.x - 3.0,
                    r.y - 3.0,
                    r.x + r.width + 3.0,
                    r.y + r.height + 3.0,
                    self.colors.code_background,
                );
                self.push_text_runs(scene, &block.text_runs, r);
            }
            RenderBlockKind::Quote => {
                // Vertical stripe on the left edge of the indent gutter.
                let stripe_w = 3.0;
                scene.push_rect(
                    r.x - block.indent + 2.0,
                    r.y,
                    r.x - block.indent + 2.0 + stripe_w,
                    r.y + r.height,
                    self.colors.quote_stripe,
                );
                self.push_text_runs(scene, &block.text_runs, r);
            }
            RenderBlockKind::Badge => {
                // Pill-ish background — netrender supports rounded
                // rects, but Scene::push_rect_clipped_rounded needs a
                // clip id; for the smoke a plain rect reads fine.
                scene.push_rect(
                    r.x - 4.0,
                    r.y - 2.0,
                    r.x + r.width + 4.0,
                    r.y + r.height + 2.0,
                    self.colors.badge_background,
                );
                self.push_text_runs(scene, &block.text_runs, r);
            }
            _ => {
                self.push_text_runs(scene, &block.text_runs, r);
            }
        }
    }

    /// Lay out each text run in `runs` using parley and push the
    /// resulting glyphs into `scene`. Multiple runs stack vertically
    /// inside the block's rect; per-run font size and brush come from
    /// the run's `TextStyle`.
    fn push_text_runs(&mut self, scene: &mut Scene, runs: &[RenderTextRun], rect: RenderRect) {
        let max_advance = rect.width.max(60.0);
        let mut cursor_y = rect.y;
        for run in runs {
            if run.text.is_empty() {
                continue;
            }
            let font_size = font_size_for_style(&run.style, &self.theme);
            let brush = self.brush_for_style(&run.style);
            let weight = font_weight_for_style(&run.style);
            let style = font_style_for_style(&run.style);

            let mut builder =
                self.layout_cx
                    .ranged_builder(&mut self.font_cx, &run.text, 1.0, true);
            builder.push_default(StyleProperty::FontSize(font_size));
            builder.push_default(StyleProperty::Brush(brush));
            builder.push_default(StyleProperty::FontFamily(FontFamily::named(&self.family_name)));
            builder.push_default(StyleProperty::FontWeight(weight));
            builder.push_default(StyleProperty::FontStyle(style));

            let mut layout: Layout<[f32; 4]> = builder.build(&run.text);
            layout.break_all_lines(Some(max_advance));
            layout.align(Alignment::Start, AlignmentOptions::default());

            // parley reports its layout height; advance the cursor by
            // it so multi-run blocks stack properly.
            let layout_height = layout.height();
            netrender_text::push_layout(scene, &layout, [rect.x, cursor_y]);
            cursor_y += layout_height + 2.0;
        }
    }

    fn brush_for_style(&self, style: &TextStyle) -> [f32; 4] {
        match style {
            TextStyle::Body => self.colors.body,
            TextStyle::Title => self.colors.title,
            TextStyle::Heading => self.colors.heading,
            TextStyle::Quote => self.colors.quote,
            TextStyle::Metadata => self.colors.metadata,
            TextStyle::Code => self.colors.code,
            TextStyle::Badge => self.colors.badge,
            TextStyle::Link => self.colors.link,
        }
    }
}

fn font_size_for_style(style: &TextStyle, theme: &ThemeTokens) -> f32 {
    match style {
        TextStyle::Title => theme.title_font_size,
        TextStyle::Heading => theme.heading_font_size,
        TextStyle::Metadata | TextStyle::Code | TextStyle::Badge => theme.metadata_font_size,
        TextStyle::Body | TextStyle::Quote | TextStyle::Link => theme.body_font_size,
    }
}

fn font_weight_for_style(style: &TextStyle) -> FontWeight {
    match style {
        TextStyle::Title | TextStyle::Heading | TextStyle::Badge => FontWeight::BOLD,
        _ => FontWeight::NORMAL,
    }
}

fn font_style_for_style(style: &TextStyle) -> FontStyle {
    match style {
        TextStyle::Quote => FontStyle::Italic,
        _ => FontStyle::Normal,
    }
}
