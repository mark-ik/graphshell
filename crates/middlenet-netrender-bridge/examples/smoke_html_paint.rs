/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Level-2 smoke: HTML → blitz-dom → ⟨our painter⟩ → netrender → PNG.
//!
//! First-slice coverage (debug-box mode): no styles, no text, just
//! depth-tinted layout boxes. Validates the cross-repo wiring:
//!
//! 1. blitz-dom + blitz-html compile alongside netrender + iced's
//!    transitive deps without version conflicts (Stylo, Taffy,
//!    Parley are pulled in transitively through blitz-dom).
//! 2. `HtmlDocument::from_html` parses inline HTML.
//! 3. `BaseDocument::resolve(0.0)` runs the Stylo + Taffy pipeline
//!    and populates `final_layout` on every node.
//! 4. Our painter walks the tree and emits netrender Scene primitives.
//! 5. `netrender::Renderer::render_vello` rasterizes the result to a
//!    wgpu texture; PNG receipt.
//!
//! Iteration plan (each subsequent smoke adds one capability):
//! - Level 2.1: background-color from ComputedValues
//! - Level 2.2: text runs via parley + netrender_text
//! - Level 2.3: borders
//! - Level 2.4: images
//! - Level 2.5+: transforms, clipping, box-shadow, filters, selection
//!
//! Run with:
//!   cargo run --example smoke_html_paint -p middlenet-netrender-bridge

use std::path::PathBuf;

use blitz_dom::DocumentConfig;
use middlenet_netrender_bridge::html_paint::load_default_font_context;
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
use middlenet_netrender_bridge::html_paint::paint_html_document;
use netrender::{ColorLoad, NetrenderOptions, Scene, boot, create_netrender_instance};

const VW: u32 = 720;
const VH: u32 = 900;
const TILE: u32 = 64;
const SCALE: f64 = 1.0;

const HTML: &str = r#"
<!DOCTYPE html>
<html>
<head><title>Bridge HTML smoke — backgrounds</title></head>
<body style="background: #1a1d22; color: #eee;">
  <h1 style="background: #4a6fa5; border: 4px solid #c8d4e8; border-radius: 8px; box-shadow: 0 6px 12px rgba(0,0,0,0.6);">Hello, Blitz — backgrounds + text + borders + shadow</h1>
  <p style="background: #2a3a48;">This paragraph has a slate-blue background. The slice 2.1 painter reads <code>background-color</code> from each element's <code>ComputedValues</code> via <code>style.get_background().background_color.resolve_to_absolute()</code> — same pattern blitz-paint uses internally — and emits a <code>netrender::Scene::push_rect</code> at the layout rect.</p>
  <ul style="background: #3a4a3a;">
    <li>Item one (inherits ul background)</li>
    <li style="background: #5a4a3a;">Item two (own background)</li>
    <li>Item three</li>
  </ul>
  <h2 style="background: #4a3a5a; border: 2px solid #b89bc8;">Nested boxes</h2>
  <div style="background: #2d3a4d; padding: 8px; opacity: 0.5;">This div has opacity 0.5 — its background and text should both render at half-strength against the page background.</div>
  <div style="background: #4a3a2a; padding: 8px; overflow: hidden; border-radius: 12px; height: 40px;">overflow: hidden with border-radius — this paragraph overflows the parent's 40px height; the bottom should be clipped at the rounded box edge instead of bleeding past. Lorem ipsum dolor sit amet consectetur adipiscing elit, sed do eiusmod tempor.</div>
  <div style="background: #2f2630;">
    <div style="background: #3f3640;">
      <div style="background: #4f4650;">
        <p style="background: #5f5660; border: 1px solid #ffaa00; border-radius: 16px;">Four levels of nested div backgrounds, each slightly lighter; this innermost paragraph has a 1px amber border with rounded corners.</p>
      </div>
    </div>
  </div>
  <hr/>
  <footer style="background: #2a2a2a;">Bottom of the page (transparent rule above).</footer>
</body>
</html>
"#;

fn main() {
    // 1. Parse + resolve. Slice 2.4: provide our own FontContext
    // (caller-supplied) instead of relying on blitz-dom's
    // `system_fonts` feature; restores wasm-clean discipline.
    let font_ctx = load_default_font_context()
        .expect("smoke_html_paint: no system font found at candidate paths");
    let mut doc = HtmlDocument::from_html(
        HTML,
        DocumentConfig {
            viewport: Some(Viewport::new(
                (VW as f32 * SCALE as f32) as u32,
                (VH as f32 * SCALE as f32) as u32,
                SCALE as f32,
                ColorScheme::Light,
            )),
            font_ctx: Some(font_ctx),
            ..Default::default()
        },
    );
    doc.as_mut().resolve(0.0);
    eprintln!(
        "smoke_html_paint: resolved layout — root size {:?}",
        doc.as_ref().root_element().final_layout.size
    );

    // 2. Boot netrender + build scene via our painter.
    let handles = boot().expect("netrender::boot failed");
    let renderer = create_netrender_instance(
        handles.clone(),
        NetrenderOptions { tile_cache_size: Some(TILE), enable_vello: true },
    )
    .expect("create_netrender_instance failed");

    let mut scene = Scene::new(VW, VH);
    paint_html_document(&mut scene, doc.as_ref(), SCALE, VW as f32, VH as f32, &renderer);

    // 3. Render to PNG.
    let target = handles.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke_html_paint target"),
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
        label: Some("smoke_html_paint view"),
        format: Some(wgpu::TextureFormat::Rgba8Unorm),
        ..Default::default()
    });

    renderer.render_vello(&scene, &view, ColorLoad::Clear(wgpu::Color::BLACK));
    let bytes = renderer.wgpu_device.read_rgba8_texture(&target, VW, VH);

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("output")
        .join("smoke_html_paint.png");
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

    println!("smoke_html_paint: wrote {}", out.display());
    println!(
        "  expected: {}x{} image, slate background, depth-tinted boxes outlining the HTML layout tree",
        VW, VH
    );
}
