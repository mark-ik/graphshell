/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! HTML → `netrender::Scene` painter.
//!
//! **Clean path** per the 2026-05-05 architectural call: skip Blitz's
//! `blitz-paint` and the `anyrender` abstraction layer. Walk the
//! `blitz_dom::BaseDocument` directly and emit `netrender::Scene`
//! primitives. The reference implementation we're cribbing from is
//! `blitz/packages/blitz-paint/src/render.rs` (BlitzDomPainter); we
//! re-implement the walk against netrender's display-list shape so
//! we can pass through tile-cache, picture-cache, and render-graph
//! features that AnyRender's per-primitive surface can't express.
//!
//! WASM-clean: same discipline as the rest of this crate. blitz-dom
//! is brought in with `default-features = false` to drop
//! `system_fonts` and `file_input`.
//!
//! Coverage today (2026-05-05, first slice — debug-box mode):
//! - [x] tree traversal: walk `BaseDocument::root_element` → children
//! - [x] per-element box: emit a translucent rect at `final_layout` so
//!       the layout shape is visible
//! - [ ] background-color from ComputedValues
//! - [ ] border (color, width, radius)
//! - [ ] text runs (parley layout + `netrender_text::push_layout`)
//! - [ ] images
//! - [ ] transforms
//! - [ ] clipping / overflow
//! - [ ] box-shadow / filters
//! - [ ] selection highlight
//!
//! The first slice is intentionally a debug visualizer — it answers
//! "did the dep graph compile, did blitz parse + resolve, can we
//! walk the resolved tree?" Each subsequent slice picks up one row
//! of the coverage list, with a smoke PNG receipt.

use std::sync::Arc;

use blitz_dom::FontContext;
use blitz_dom::node::{TextBrush, TextLayout};
use blitz_dom::{BaseDocument, Node};
use netrender_text::parley::fontique;
use netrender::{ColorLoad, FontBlob, FontRegistry, Glyph, Renderer, Scene, SceneClip, SceneLayer};
// `ColorLoad` is unused in the painter today (callers pass it to
// `Renderer::render_vello`); keeping it imported here so the
// `_ = ColorLoad::Clear` line in the smoke can stay alongside the
// other netrender re-exports without `crates::netrender::*` clutter.
const _: fn() -> ColorLoad = || ColorLoad::Clear(wgpu::Color::BLACK);
use netrender_text::parley::{Layout as ParleyLayout, PositionedLayoutItem};
use style::color::AbsoluteColor;
use style::properties::generated::ComputedValues;
use style::values::computed::{BorderCornerRadius, CSSPixelLength, Overflow};
use style::values::specified::BorderStyle;

/// Paint a fully-resolved [`BaseDocument`] (post-`resolve(0.0)`) into
/// a [`netrender::Scene`].
///
/// `scale` is the device-pixel ratio caller used when constructing the
/// `Viewport` for the document; pass it through so the per-node
/// `final_layout` (in CSS pixels) maps to the right scene-space units.
/// `viewport_w` / `viewport_h` are scene-space dimensions for the
/// background fill; blitz's resolved layout extends past these for
/// long pages — caller can crop or grow the netrender target.
pub fn paint_html_document(
    scene: &mut Scene,
    document: &BaseDocument,
    scale: f64,
    viewport_w: f32,
    viewport_h: f32,
    renderer: &Renderer,
) {
    // Page background — slate, so debug boxes are visible. Real
    // implementation reads `<body>` or root-element `background-color`
    // from ComputedValues.
    scene.push_rect(0.0, 0.0, viewport_w, viewport_h, [0.10, 0.11, 0.13, 1.0]);

    let root = document.root_element();
    let mut font_registry = FontRegistry::new();
    let mut shadow_key_seed: u64 = 0xDEC0DE_5400;
    paint_node(
        scene,
        document,
        root,
        [0.0, 0.0],
        scale,
        viewport_h,
        &mut font_registry,
        renderer,
        &mut shadow_key_seed,
    );
}

/// Recursive walk. Reads `background-color` from each element's
/// `ComputedValues` and emits a fill at the (accumulated) layout
/// rect, then walks the element's `inline_layout_data` (parley
/// `Layout<TextBrush>`) and emits glyph runs.
///
/// `parent_origin` is the absolute position (in scene-space units)
/// of this node's parent's content rect. Taffy reports each node's
/// `final_layout.location` relative to its parent, so we accumulate
/// down the tree to get absolute coords. Pattern matches blitz-paint's
/// `BlitzDomPainter::node_position`.
#[allow(clippy::too_many_arguments)]
fn paint_node(
    scene: &mut Scene,
    document: &BaseDocument,
    node: &Node,
    parent_origin: [f32; 2],
    scale: f64,
    viewport_h: f32,
    font_registry: &mut FontRegistry,
    renderer: &Renderer,
    shadow_key_seed: &mut u64,
) {
    let layout = node.final_layout;
    let x0 = parent_origin[0] + (layout.location.x as f64 * scale) as f32;
    let y0 = parent_origin[1] + (layout.location.y as f64 * scale) as f32;
    let x1 = x0 + (layout.size.width as f64 * scale) as f32;
    let y1 = y0 + (layout.size.height as f64 * scale) as f32;

    // CSS `opacity` wraps the entire element (background, text, border,
    // children) — push a layer at the start, pop after recursion.
    // Skipped when alpha >= 1.0 to avoid a no-op offscreen pass.
    let pushed_opacity = node
        .primary_styles()
        .map(|s| element_opacity(&s))
        .filter(|a| *a < 1.0)
        .map(|alpha| {
            scene.push_layer(SceneLayer::alpha(alpha));
        })
        .is_some();

    if let Some(elem_data) = node.element_data()
        && let Some(styles) = node.primary_styles()
        && (x1 - x0) > 0.0
        && (y1 - y0) > 0.0
    {
        // Outset box-shadow paints UNDER the element's own background +
        // border (CSS painting order). One shadow for now; multi-shadow
        // and inset shadows are deferred refinements.
        paint_outset_box_shadow(
            scene,
            renderer,
            &styles,
            [x0, y0, x1, y1],
            scale,
            viewport_h,
            shadow_key_seed,
        );

        if let Some(rgba) = element_background_rgba(&styles) {
            scene.push_rect(x0, y0, x1, y1, rgba);
        }

        // Inline text — `inline_layout_data` is `Some` on elements
        // that root an inline-formatting context (most leaf-text
        // elements, plus blocks containing inline content).
        if let Some(inline_layout) = elem_data.inline_layout_data.as_ref() {
            push_blitz_inline_layout(
                scene,
                document,
                inline_layout,
                [x0, y0],
                scale,
                font_registry,
            );
        }

        // Border — single uniform stroke from the top-side color +
        // width, with four corner radii. Per-side variation
        // (different color/width per edge, dashed/dotted) is a later
        // refinement.
        if let Some((color, width, radii)) = element_border(&styles, x1 - x0, y1 - y0, scale) {
            scene.push_stroke_rounded(x0, y0, x1, y1, color, width, radii);
        }
    }

    // `overflow: hidden` clips children only — the element's own
    // background/border already painted outside the clip.
    let pushed_clip = node
        .primary_styles()
        .filter(|s| element_overflow_clips(s))
        .map(|s| {
            let radii = element_corner_radii(&s, x1 - x0, y1 - y0, scale);
            scene.push_layer(SceneLayer::clip(SceneClip::Rect {
                rect: [x0, y0, x1, y1],
                radii,
            }));
        })
        .is_some();

    // Children accumulate from this node's top-left, regardless of
    // whether this node is itself an element with computed styles
    // (text nodes still propagate origin to their siblings/peers).
    let child_origin = [x0, y0];
    let tree = document.tree();
    for &child_id in &node.children {
        if let Some(child) = tree.get(child_id) {
            paint_node(
                scene,
                document,
                child,
                child_origin,
                scale,
                viewport_h,
                font_registry,
                renderer,
                shadow_key_seed,
            );
        }
    }

    if pushed_clip {
        scene.pop_layer();
    }
    if pushed_opacity {
        scene.pop_layer();
    }
}

/// Push a blitz `TextLayout` into the scene as glyph runs.
///
/// Mirrors `netrender_text::push_layout_with_registry`'s shape but
/// resolves the per-run color via blitz's `TextBrush` (which carries
/// a node id) — we look up the node, read its inherited text color
/// (the `color` property), and use that as the run's brush. This is
/// the same pattern blitz-paint's `text::stroke_text` uses, just
/// emitting through netrender's `Scene::push_glyph_run` instead of
/// anyrender's `draw_glyphs`.
fn push_blitz_inline_layout(
    scene: &mut Scene,
    document: &BaseDocument,
    text_layout: &TextLayout,
    origin: [f32; 2],
    scale: f64,
    font_registry: &mut FontRegistry,
) {
    let layout: &ParleyLayout<TextBrush> = &text_layout.layout;
    let scale_f32 = scale as f32;

    for line in layout.lines() {
        for item in line.items() {
            let glyph_run = match item {
                PositionedLayoutItem::GlyphRun(gr) => gr,
                PositionedLayoutItem::InlineBox(_) => continue,
            };

            // Resolve text color via the brush's node-id lookup. Defaults
            // to opaque white if the lookup fails (defensive — shouldn't
            // happen for well-formed documents).
            let style_node_id = glyph_run.style().brush.id;
            let text_color = document
                .get_node(style_node_id)
                .and_then(|n| n.primary_styles())
                .map(|s| absolute_to_premul_rgba(&s.clone_color()))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);

            // Register the font (deduped via the FontRegistry).
            let run = glyph_run.run();
            let font_data = run.font();
            let font_id = font_registry.intern(
                scene,
                FontBlob {
                    data: font_data.data.clone(),
                    index: font_data.index,
                },
            );

            let glyphs: Vec<Glyph> = glyph_run
                .positioned_glyphs()
                .map(|g| Glyph {
                    id: g.id,
                    x: origin[0] + g.x * scale_f32,
                    y: origin[1] + g.y * scale_f32,
                })
                .collect();

            if !glyphs.is_empty() {
                scene.push_glyph_run(font_id, run.font_size() * scale_f32, glyphs, text_color);
            }
        }
    }
}

/// Resolve an element's background-color to a premultiplied sRGB
/// `[f32; 4]` (netrender's Scene contract). Returns `None` when the
/// background is transparent — caller should skip the rect emit so
/// the page background or a parent's fill shows through.
///
/// Pattern lifted from `blitz/packages/blitz-paint/src/render/background.rs`
/// (`draw_solid_bg`); we keep the resolve-to-absolute step (which
/// handles `currentColor`-style indirections) and bypass the
/// `peniko::Brush` allocation by going straight to a 4-float array.
fn element_background_rgba(styles: &ComputedValues) -> Option<[f32; 4]> {
    let current_color = styles.clone_color();
    let background_color = &styles.get_background().background_color;
    let absolute = background_color.resolve_to_absolute(&current_color);
    let [r, g, b, a] = absolute_to_premul_rgba(&absolute);
    if a <= f32::EPSILON {
        None
    } else {
        Some([r, g, b, a])
    }
}

/// Build a `parley::FontContext` with a single system TTF loaded —
/// enough for blitz's layout pipeline to measure text without
/// requiring `blitz-dom/system_fonts` (which would pull in
/// platform-specific font discovery that doesn't compile to
/// `wasm32-unknown-unknown`).
///
/// Returns `None` if no candidate path resolves on this host. Pass
/// the result into `DocumentConfig::font_ctx` so blitz uses it
/// instead of constructing a new (empty) one.
///
/// Slice 2.4 of the html-paint roadmap: this is the wasm-clean
/// alternative to enabling `system_fonts`. The fontique blob path
/// works the same on every target; what changes per-target is which
/// bytes the caller supplies.
pub fn load_default_font_context() -> Option<FontContext> {
    let candidates = [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/Library/Fonts/Arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut font_cx = FontContext::default();
            let blob = fontique::Blob::new(Arc::new(bytes));
            font_cx.collection.register_fonts(blob, None);
            return Some(font_cx);
        }
    }
    None
}

/// Resolve the element's border to a single (color, width, radii)
/// triple suitable for `Scene::push_stroke_rounded`. Returns `None`
/// when the border is hidden / zero-width / fully transparent.
///
/// **Simplification vs. blitz-paint**: blitz handles per-side
/// distinct colors/widths and dashed/dotted styles. This first-pass
/// adapter uses the top side's color + width uniformly and only
/// emits when border-top-style is `Solid` (or unspecified, which
/// resolves to `None`/`Hidden`). Per-side variation is a later
/// refinement; the lift is per-edge stroke calls (one per side that
/// differs), which netrender's primitive set already supports.
/// CSS `opacity` ∈ [0.0, 1.0] from `ComputedValues`. Default is 1.0
/// (no transparency); we treat anything ≥ 1.0 as 1.0 so the caller
/// can skip the offscreen layer.
fn element_opacity(styles: &ComputedValues) -> f32 {
    styles.get_effects().opacity.clamp(0.0, 1.0)
}

/// True when the element has `overflow: hidden` (or `clip`) on either
/// axis. CSS `overflow: scroll`/`auto` is also clipping in practice
/// once a scrollbox is in play, but we don't model scroll containers
/// in this slice — those need scroll offset and visible/hidden track
/// state, which is host-side.
fn element_overflow_clips(styles: &ComputedValues) -> bool {
    let box_styles = styles.get_box();
    matches!(
        box_styles.overflow_x,
        Overflow::Hidden | Overflow::Clip
    ) || matches!(
        box_styles.overflow_y,
        Overflow::Hidden | Overflow::Clip
    )
}

/// Resolve the four corner radii (top-left, top-right, bottom-right,
/// bottom-left) for clip-path / border-radius use. Same axis-min
/// reduction as `element_border` so the clip stays inside the box.
fn element_corner_radii(styles: &ComputedValues, box_w: f32, box_h: f32, scale: f64) -> [f32; 4] {
    let border = styles.get_border();
    let resolve_w = CSSPixelLength::new(box_w / scale as f32);
    let resolve_h = CSSPixelLength::new(box_h / scale as f32);
    let resolve = |c: &BorderCornerRadius| -> f32 {
        let w = c.0.width.0.resolve(resolve_w).px() as f64 * scale;
        let h = c.0.height.0.resolve(resolve_h).px() as f64 * scale;
        w.min(h) as f32
    };
    [
        resolve(&border.border_top_left_radius),
        resolve(&border.border_top_right_radius),
        resolve(&border.border_bottom_right_radius),
        resolve(&border.border_bottom_left_radius),
    ]
}

/// Paint the first outset shadow on `styles` (if any) under the
/// element's box. Builds a soft alpha mask via
/// `Renderer::build_box_shadow_mask` (which runs vello compute via
/// the render-graph, registers the result as a vello-side ImageKey)
/// and composites it as a tinted image at the offset position.
///
/// Limitations of this slice: only the first non-inset shadow is
/// rendered; spread is ignored (spread = 0); inset shadows are
/// deferred. Mask dim is the viewport height (matches the
/// `demo_card_grid.rs` reference); a tighter bound would save work.
fn paint_outset_box_shadow(
    scene: &mut Scene,
    renderer: &Renderer,
    styles: &ComputedValues,
    box_rect: [f32; 4],
    scale: f64,
    viewport_h: f32,
    shadow_key_seed: &mut u64,
) {
    let shadows = &styles.get_effects().box_shadow.0;
    let Some(shadow) = shadows.iter().find(|s| !s.inset) else {
        return;
    };

    let offset_x = shadow.base.horizontal.px() as f64 * scale;
    let offset_y = shadow.base.vertical.px() as f64 * scale;
    let blur_px = shadow.base.blur.0.px() as f64 * scale;

    // Resolve the shadow color (currentColor-aware) and bail on fully
    // transparent.
    let current_color = styles.clone_color();
    let abs = shadow.base.color.resolve_to_absolute(&current_color);
    let tint = absolute_to_premul_rgba(&abs);
    if tint[3] <= f32::EPSILON {
        return;
    }

    // Use the corner radius from `border-top-left-radius` for the mask
    // (build_box_shadow_mask takes a single uniform radius). A more
    // accurate path uses a per-corner mask but isn't exposed by netrender's
    // current API surface.
    let radii = element_corner_radii(
        styles,
        box_rect[2] - box_rect[0],
        box_rect[3] - box_rect[1],
        scale,
    );
    let corner_radius = radii.iter().copied().fold(0.0_f32, f32::max);

    // Build the mask. Caller-provided unique key — we increment per
    // shadow. The mask covers `viewport_h × viewport_h` (overkill but
    // matches netrender's existing demo pattern; tightening is a perf
    // refinement).
    let key = *shadow_key_seed;
    *shadow_key_seed += 1;
    let mask_dim = viewport_h.ceil() as u32;
    renderer.build_box_shadow_mask(
        key,
        mask_dim,
        box_rect,
        corner_radius,
        blur_px as f32,
    );

    // Composite the mask at the shadow's display rect (offset by
    // `(offset_x, offset_y)`). UVs map the box rect within the
    // mask-space dim → [0,1]² fraction.
    let dim_f = mask_dim as f32;
    let display_rect = [
        box_rect[0] + offset_x as f32,
        box_rect[1] + offset_y as f32,
        box_rect[2] + offset_x as f32,
        box_rect[3] + offset_y as f32,
    ];
    let uvs = [
        box_rect[0] / dim_f,
        box_rect[1] / dim_f,
        box_rect[2] / dim_f,
        box_rect[3] / dim_f,
    ];
    scene.push_image_full(
        display_rect[0],
        display_rect[1],
        display_rect[2],
        display_rect[3],
        uvs,
        tint,
        key,
        0,
        netrender::NO_CLIP,
    );
}

fn element_border(
    styles: &ComputedValues,
    box_w: f32,
    box_h: f32,
    scale: f64,
) -> Option<(
    [f32; 4], // premultiplied sRGB
    f32,      // stroke width
    [f32; 4], // [tl, tr, br, bl] corner radii
)> {
    let border = styles.get_border();

    // Width — converted via the same scale we use for layout.
    let width_px = (border.border_top_width.0.to_f64_px() * scale) as f32;
    if width_px <= 0.0 {
        return None;
    }

    // Style — skip Hidden / None / dashed (the dashed/dotted
    // path needs a different netrender primitive we'll add later).
    if !matches!(border.border_top_style, BorderStyle::Solid) {
        return None;
    }

    // Color — `currentColor` indirection same as background.
    let current_color = styles.clone_color();
    let absolute = border.border_top_color.resolve_to_absolute(&current_color);
    let rgba = absolute_to_premul_rgba(&absolute);
    if rgba[3] <= f32::EPSILON {
        return None;
    }

    // Corner radii — resolve `length-percentage` against the box
    // size, take the width axis only (we don't have separate per-axis
    // corner radii in netrender's primitive). Pattern from
    // blitz-paint::render::create_css_rect.
    let resolve_w = CSSPixelLength::new(box_w / scale as f32);
    let resolve_h = CSSPixelLength::new(box_h / scale as f32);
    let resolve = |c: &BorderCornerRadius| -> f32 {
        let w = c.0.width.0.resolve(resolve_w).px() as f64 * scale;
        let h = c.0.height.0.resolve(resolve_h).px() as f64 * scale;
        // Use the smaller axis to avoid radius > half-box
        // bleeding into the wrong corner; netrender's
        // `push_stroke_rounded` expects per-corner scalars.
        w.min(h) as f32
    };
    let radii = [
        resolve(&border.border_top_left_radius),
        resolve(&border.border_top_right_radius),
        resolve(&border.border_bottom_right_radius),
        resolve(&border.border_bottom_left_radius),
    ];

    Some((rgba, width_px, radii))
}

/// `style::color::AbsoluteColor` → premultiplied sRGB `[f32; 4]`.
///
/// Stylo carries straight-alpha colors; netrender's Scene API expects
/// premultiplied RGBA per the vello plan §6.3 contract, so we multiply
/// the components by alpha here. The `to_color_space(Srgb).raw_components()`
/// dance is the same conversion blitz-paint's `as_srgb_color()` does
/// (just without the `color::AlphaColor<Srgb>` wrapping that we don't
/// need).
fn absolute_to_premul_rgba(c: &AbsoluteColor) -> [f32; 4] {
    let comps = c
        .to_color_space(style::color::ColorSpace::Srgb)
        .raw_components()
        .to_owned();
    let [r, g, b, a] = comps;
    [r * a, g * a, b * a, a]
}
