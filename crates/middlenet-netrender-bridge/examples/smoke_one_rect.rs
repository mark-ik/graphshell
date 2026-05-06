/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Level-0 smoke test: drive `netrender` end-to-end from graphshell's
//! tree, no iced involvement yet.
//!
//! What this proves (if it works):
//! 1. The cross-repo path dep (`graphshell/crates/middlenet-netrender-bridge`
//!    → `webrender-wgpu/netrender`) compiles cleanly.
//! 2. `netrender::boot()` succeeds in this environment (wgpu adapter
//!    discovery, device creation, queue creation).
//! 3. `create_netrender_instance` succeeds with `enable_vello: true`.
//! 4. A `Scene` with one primitive renders through `Renderer::render_vello`
//!    without panic or vello error.
//! 5. The output texture reads back to RGBA bytes that PNG-encode.
//!
//! What this does NOT prove:
//! - iced shader widget integration (level 1)
//! - device sharing between iced and netrender
//! - `RenderScene → Scene` adapter shape (no middlenet involvement)
//! - text / parley layout (no glyph runs in this scene)
//! - color contract end-to-end (no eyeball validation against an oracle PNG)
//!
//! Run with:
//!   cargo run --example smoke_one_rect -p middlenet-netrender-bridge

use std::path::PathBuf;

use netrender::{ColorLoad, NetrenderOptions, Scene, boot, create_netrender_instance};

const VW: u32 = 320;
const VH: u32 = 200;
const TILE: u32 = 64;

fn main() {
    let handles = boot().expect("netrender::boot — wgpu adapter discovery failed");
    let renderer = create_netrender_instance(
        handles.clone(),
        NetrenderOptions { tile_cache_size: Some(TILE), enable_vello: true },
    )
    .expect("create_netrender_instance — vello renderer setup failed");

    // Scene: solid background + one inset rect. Premultiplied RGBA per
    // netrender's Scene API (vello plan §6.3).
    let mut scene = Scene::new(VW, VH);
    // Background — slate.
    scene.push_rect(0.0, 0.0, VW as f32, VH as f32, [0.10, 0.11, 0.13, 1.0]);
    // Inset rect — teal. Inset 40px on each side.
    scene.push_rect(40.0, 40.0, (VW - 40) as f32, (VH - 40) as f32, [0.18, 0.55, 0.55, 1.0]);

    // Render target — Rgba8Unorm storage, sRGB view-format chain per
    // vello plan §6.1 ("Rgba8Unorm storage with view_formats: &[Rgba8UnormSrgb]").
    let target = handles.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke_one_rect target"),
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
        label: Some("smoke_one_rect view"),
        format: Some(wgpu::TextureFormat::Rgba8Unorm),
        ..Default::default()
    });

    renderer.render_vello(&scene, &view, ColorLoad::Clear(wgpu::Color::BLACK));
    let bytes = renderer.wgpu_device.read_rgba8_texture(&target, VW, VH);

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("output")
        .join("smoke_one_rect.png");
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

    println!("smoke_one_rect: wrote {}", out.display());
    println!("  expected: {}x{} image, slate background, teal inset rect", VW, VH);
    println!(
        "  scene pushed {} primitives; render_vello returned without error",
        2
    );
}
