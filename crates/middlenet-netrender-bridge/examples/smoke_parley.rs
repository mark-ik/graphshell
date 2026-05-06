/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Level-0.5 smoke: parley layout → netrender_text::push_layout → render.
//!
//! What this proves on top of `smoke_one_rect`:
//! 1. `netrender_text` (parley adapter) compiles in graphshell's tree.
//! 2. parley's `FontContext` discovers a system font and registers it.
//! 3. A short string lays out into glyph runs without panic.
//! 4. `netrender_text::push_layout(&mut scene, &layout, origin)` pushes
//!    those runs into a `netrender::Scene` cleanly.
//! 5. `render_vello` rasterizes the glyphs alongside solid rects in
//!    one pass — no separate text path, no atlas plumbing needed by
//!    the consumer.
//!
//! Per vello plan §4.4: parley sits at the embedder layer. middlenet
//! (or this bridge) owns the layout step; netrender consumes glyph
//! runs. This smoke is the embedder layer in miniature.
//!
//! What this still doesn't do:
//! - Font fallback when no system font matches (the current set is
//!   Windows-only first; Linux/macOS paths are tried as fallback).
//! - BiDi / complex script shaping (only Latin tested here).
//! - middlenet `RenderTextRun` integration (next step — write the
//!   `RenderScene → Scene` adapter that uses parley for any text run).
//!
//! Run with:
//!   cargo run --example smoke_parley -p middlenet-netrender-bridge

use std::path::PathBuf;
use std::sync::Arc;

use netrender::{ColorLoad, NetrenderOptions, Scene, boot, create_netrender_instance};
use netrender_text::parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, Layout, LayoutContext, StyleProperty,
    fontique,
};

const VW: u32 = 480;
const VH: u32 = 200;
const TILE: u32 = 64;

const LABEL: &str = "Hello, parley + netrender + vello";
const LABEL_COLOR: [f32; 4] = [0.95, 0.95, 0.95, 1.0]; // premultiplied: opaque white-ish
const LABEL_SIZE: f32 = 22.0;
const LABEL_X: f32 = 30.0;
const LABEL_Y: f32 = 80.0;

/// Try to find a system TTF and register it as parley's primary
/// font family. Returns the registered family name on success;
/// returns `None` (with a stderr note) if no candidate path exists.
fn try_load_system_font(font_cx: &mut FontContext) -> Option<String> {
    // Windows-first because that's the host this is being built on.
    // Linux/macOS paths kept for portability if the bridge runs in
    // CI or on developer laptops with different OSes.
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
            eprintln!("smoke_parley: loaded font {} ({} bytes)", path, bytes.len());
            let blob = fontique::Blob::new(Arc::new(bytes));
            let (family_id, _) = font_cx
                .collection
                .register_fonts(blob, None)
                .into_iter()
                .next()
                .expect("register_fonts on a real TTF returns at least one family");
            let family_name = font_cx
                .collection
                .family_name(family_id)
                .expect("registered family has a name")
                .to_owned();
            return Some(family_name);
        }
    }
    eprintln!("smoke_parley: no system font found at the candidate paths");
    None
}

fn main() {
    let handles = boot().expect("netrender::boot failed");
    let renderer = create_netrender_instance(
        handles.clone(),
        NetrenderOptions { tile_cache_size: Some(TILE), enable_vello: true },
    )
    .expect("create_netrender_instance failed");

    // Parley setup.
    let mut font_cx = FontContext::new();
    let mut layout_cx: LayoutContext<[f32; 4]> = LayoutContext::new();
    let family_name = try_load_system_font(&mut font_cx)
        .expect("smoke_parley needs a system font; none found at candidate paths");

    // Build the scene: teal background + laid-out label.
    let mut scene = Scene::new(VW, VH);
    scene.push_rect(0.0, 0.0, VW as f32, VH as f32, [0.18, 0.55, 0.55, 1.0]);

    // Lay out the label via parley.
    let mut builder = layout_cx.ranged_builder(&mut font_cx, LABEL, 1.0, true);
    builder.push_default(StyleProperty::FontSize(LABEL_SIZE));
    builder.push_default(StyleProperty::Brush(LABEL_COLOR));
    builder.push_default(StyleProperty::FontFamily(FontFamily::named(&family_name)));
    let mut layout: Layout<[f32; 4]> = builder.build(LABEL);
    let max_advance = (VW as f32) - LABEL_X * 2.0;
    layout.break_all_lines(Some(max_advance));
    layout.align(Some(max_advance), Alignment::Start, AlignmentOptions::default());

    // Push glyph runs into the scene.
    netrender_text::push_layout(&mut scene, &layout, [LABEL_X, LABEL_Y]);

    eprintln!(
        "smoke_parley: laid out {} chars across {} lines (max_advance {:.1})",
        LABEL.chars().count(),
        layout.len(),
        max_advance
    );

    // Render target — same Rgba8Unorm + Rgba8UnormSrgb view-format chain
    // as smoke_one_rect (vello plan §6.1).
    let target = handles.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke_parley target"),
        size: wgpu::Extent3d { width: VW, height: VH, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor {
        label: Some("smoke_parley view"),
        format: Some(wgpu::TextureFormat::Rgba8Unorm),
        ..Default::default()
    });

    renderer.render_vello(&scene, &view, ColorLoad::Clear(wgpu::Color::BLACK));
    let bytes = renderer.wgpu_device.read_rgba8_texture(&target, VW, VH);

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("output")
        .join("smoke_parley.png");
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    let file = std::fs::File::create(&out)
        .unwrap_or_else(|e| panic!("creating {}: {}", out.display(), e));
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), VW, VH);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(&bytes).expect("png pixels");

    println!("smoke_parley: wrote {}", out.display());
    println!(
        "  expected: {}x{} image, teal background, '{}' rendered at ({:.0}, {:.0}) in {}",
        VW, VH, LABEL, LABEL_X, LABEL_Y, family_name
    );
}
