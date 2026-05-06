/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Level-1 smoke: drive the full middlenet pipeline through netrender.
//!
//! `SemanticDocument` (built here from middlenet-core types)
//!   → `middlenet_render::render_document` → `RenderScene`
//!   → `middlenet_netrender_bridge::SceneAdapter::build_scene` → `netrender::Scene`
//!   → `Renderer::render_vello` → wgpu texture → PNG.
//!
//! A representative sample document covering most `RenderBlockKind`
//! variants (Heading, Paragraph, Quote, CodeFence, List, Rule, Link,
//! MetadataRow, Badge) so the smoke surfaces missing or broken
//! mappings visually.
//!
//! Run with:
//!   cargo run --example smoke_render_scene -p middlenet-netrender-bridge

use std::path::PathBuf;

use middlenet_core::document::{
    DocumentMeta, DocumentProvenance, LinkTarget, SemanticBlock, SemanticDocument,
};
use middlenet_core::source::{MiddleNetContentKind, MiddleNetSource};
use middlenet_netrender_bridge::{SceneAdapter, ThemeColors};
use middlenet_render::{RenderMode, RenderRequest, ThemeTokens, render_document};
use netrender::{ColorLoad, NetrenderOptions, Scene, boot, create_netrender_instance};

const VW: u32 = 720;
const VH: u32 = 1100;
const TILE: u32 = 64;

fn sample_document() -> SemanticDocument {
    let source = MiddleNetSource::new(MiddleNetContentKind::Markdown)
        .with_uri("https://example.com/blog/post.md")
        .with_title_hint("Bridge Smoke");
    SemanticDocument::new(
        DocumentMeta::for_source(&source),
        DocumentProvenance::for_source(&source),
        vec![
            SemanticBlock::Heading {
                level: 1,
                text: "Bridge smoke document".to_string(),
            },
            SemanticBlock::Paragraph(
                "This document exists to exercise every RenderBlockKind \
                 variant the bridge knows how to map. The content is \
                 deliberately mundane — what we want to see is rendering, \
                 not prose."
                    .to_string(),
            ),
            SemanticBlock::Heading {
                level: 2,
                text: "Block kinds covered".to_string(),
            },
            SemanticBlock::List {
                ordered: false,
                items: vec![
                    "Heading at h1 + h2".to_string(),
                    "Paragraph (this and the others)".to_string(),
                    "Quote — for emphasis".to_string(),
                    "CodeFence — monospace-flavored".to_string(),
                    "List — bulleted (this) and ordered (below)".to_string(),
                    "Rule — horizontal divider".to_string(),
                    "Link — clickable in a real host".to_string(),
                    "MetadataRow + Badge — auxiliary info".to_string(),
                ],
            },
            SemanticBlock::Quote(
                "A quote block is rendered with an indent stripe and \
                 italic body text — visual emphasis without changing \
                 the surrounding flow."
                    .to_string(),
            ),
            SemanticBlock::CodeFence {
                lang: Some("rust".to_string()),
                text: "fn main() {\n    println!(\"hello, bridge\");\n}".to_string(),
            },
            SemanticBlock::List {
                ordered: true,
                items: vec![
                    "Numbered list — first.".to_string(),
                    "Numbered list — second.".to_string(),
                    "Numbered list — third.".to_string(),
                ],
            },
            SemanticBlock::Rule,
            SemanticBlock::Link {
                text: "An external link to example.com".to_string(),
                target: LinkTarget::new("https://example.com").with_title("Open example.com"),
            },
            SemanticBlock::MetadataRow {
                label: "Author".to_string(),
                value: "Bridge smoke harness".to_string(),
            },
            SemanticBlock::MetadataRow {
                label: "Updated".to_string(),
                value: "2026-05-04".to_string(),
            },
            SemanticBlock::Badge {
                text: "DRAFT".to_string(),
            },
        ],
    )
}

fn main() {
    // 1. SemanticDocument → RenderScene (middlenet-render does the layout
    //    estimation; produces RenderBlocks at concrete x/y/w/h rects).
    let document = sample_document();
    let theme = ThemeTokens::default();
    let request = RenderRequest {
        viewport_width: VW as f32,
        viewport_height: VH as f32,
        scale_factor: 1.0,
        theme: theme.clone(),
        font_context: None,
        image_resolver: None,
        mode: RenderMode::FullPage,
    };
    let render_scene = render_document(&document, &request);
    eprintln!(
        "smoke_render_scene: render_document produced {} blocks, scroll_extent {:.1}",
        render_scene.blocks.len(),
        render_scene.scroll_extent
    );
    if !render_scene.diagnostics.messages.is_empty() {
        for msg in &render_scene.diagnostics.messages {
            eprintln!("  diagnostic: {}", msg);
        }
    }

    // 2. RenderScene → netrender::Scene via the adapter (parley layout
    //    happens inside; one font load per adapter; reused per block).
    let mut adapter = SceneAdapter::new(theme, ThemeColors::dark())
        .expect("SceneAdapter — no system font found");
    eprintln!(
        "smoke_render_scene: adapter loaded font family '{}'",
        adapter.family_name()
    );

    // 3. Boot netrender + render.
    let handles = boot().expect("netrender::boot failed");
    let renderer = create_netrender_instance(
        handles.clone(),
        NetrenderOptions { tile_cache_size: Some(TILE), enable_vello: true },
    )
    .expect("create_netrender_instance failed");

    let mut scene = Scene::new(VW, VH);
    adapter.build_scene(&mut scene, &render_scene, VW as f32, VH as f32);

    let target = handles.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke_render_scene target"),
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
        label: Some("smoke_render_scene view"),
        format: Some(wgpu::TextureFormat::Rgba8Unorm),
        ..Default::default()
    });

    renderer.render_vello(&scene, &view, ColorLoad::Clear(wgpu::Color::BLACK));
    let bytes = renderer.wgpu_device.read_rgba8_texture(&target, VW, VH);

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("output")
        .join("smoke_render_scene.png");
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

    println!("smoke_render_scene: wrote {}", out.display());
    println!("  expected: {}x{} image, dark background", VW, VH);
    println!(
        "  blocks rendered: {} (Heading×2, Paragraph, List×2, Quote, CodeFence, Rule, Link, MetadataRow×2, Badge)",
        render_scene.blocks.len()
    );
}
