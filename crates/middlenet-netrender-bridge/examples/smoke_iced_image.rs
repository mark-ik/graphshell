/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Level-1a smoke: render a `SemanticDocument` through netrender and
//! display the result inside an iced window.
//!
//! This is **NOT** the production iced shader widget integration — it
//! uses `iced::widget::image::Handle::from_rgba` to upload netrender's
//! output into iced's image cache (CPU readback, then iced uploads it
//! back to the GPU). It proves "the rendered pixels look correct
//! inside a live iced app" without needing to share the wgpu device
//! between iced and netrender.
//!
//! The shader widget path requires access to `wgpu::Adapter` and
//! `wgpu::Instance` from inside `iced::widget::shader::Pipeline::new`,
//! which iced doesn't expose today (only `device`, `queue`, `format`
//! are passed in). Three options:
//!
//! 1. Patch iced's vendored copy to expose the adapter/instance to
//!    `Pipeline::new`.
//! 2. Build `WgpuHandles` with a fresh `wgpu::Instance` + re-discovered
//!    adapter inside `Pipeline::new`. The adapter would be a separate
//!    handle from iced's; netrender's `with_external` only reads
//!    `adapter.features()` (no-op for empty features), so this might
//!    work in practice — but it's cross-instance correctness risk.
//! 3. Two-device model: netrender owns its own wgpu device, render to
//!    its own texture, copy into iced's textures. The level-1a path
//!    here is the moral equivalent (the copy goes via CPU readback +
//!    `Handle::from_rgba`, which iced re-uploads).
//!
//! For now level-1a validates the user-facing path: open an iced
//! window, see a netrender-rendered document.
//!
//! Run with:
//!   cargo run --example smoke_iced_image -p middlenet-netrender-bridge

use iced::{
    Element, Length, Task,
    widget::{column, container, image, scrollable, text},
};
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
        .with_title_hint("Bridge Smoke (in iced)");
    SemanticDocument::new(
        DocumentMeta::for_source(&source),
        DocumentProvenance::for_source(&source),
        vec![
            SemanticBlock::Heading {
                level: 1,
                text: "Netrender + middlenet, in iced".to_string(),
            },
            SemanticBlock::Paragraph(
                "This is the level-1a smoke: a SemanticDocument rendered \
                 through middlenet-render → netrender → vello → RGBA bytes \
                 → iced::widget::image. The image you're looking at was \
                 painted by vello running on its own wgpu device; iced \
                 hosts it via its image widget."
                    .to_string(),
            ),
            SemanticBlock::Heading {
                level: 2,
                text: "What this proves".to_string(),
            },
            SemanticBlock::List {
                ordered: false,
                items: vec![
                    "iced + netrender both compile + link in one binary".to_string(),
                    "wgpu 29 versioning is consistent across the dep graph".to_string(),
                    "netrender's output composites cleanly inside an iced layout".to_string(),
                    "color contract holds across the readback boundary".to_string(),
                ],
            },
            SemanticBlock::Quote(
                "The shader-widget integration is the next step — \
                 see the doc-comment in this example for the three options."
                    .to_string(),
            ),
            SemanticBlock::CodeFence {
                lang: Some("rust".to_string()),
                text: "// Production shape (deferred):\nimpl shader::Program for Bridge { … }"
                    .to_string(),
            },
            SemanticBlock::Rule,
            SemanticBlock::Link {
                text: "Plan: 2026-04-30 renderer-and-host-refactor".to_string(),
                target: LinkTarget::new(
                    "https://example.com/design_docs/aspect_render/2026-04-30_renderer_and_host_refactor_plan.md",
                )
                .with_title("Open the plan"),
            },
            SemanticBlock::MetadataRow {
                label: "Smoke level".to_string(),
                value: "1a (image widget)".to_string(),
            },
            SemanticBlock::Badge {
                text: "LEVEL 1A".to_string(),
            },
        ],
    )
}

/// Render the document into RGBA8 bytes via the netrender pipeline.
/// Called once at app startup; the bytes are uploaded to iced's image
/// cache via `Handle::from_rgba`.
fn render_document_to_rgba() -> Vec<u8> {
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

    let mut adapter = SceneAdapter::new(theme, ThemeColors::dark())
        .expect("SceneAdapter — no system font found");

    let handles = boot().expect("netrender::boot failed");
    let renderer = create_netrender_instance(
        handles.clone(),
        NetrenderOptions { tile_cache_size: Some(TILE), enable_vello: true },
    )
    .expect("create_netrender_instance failed");

    let mut scene = Scene::new(VW, VH);
    adapter.build_scene(&mut scene, &render_scene, VW as f32, VH as f32);

    let target = handles.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke_iced_image target"),
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
        label: Some("smoke_iced_image view"),
        format: Some(wgpu::TextureFormat::Rgba8Unorm),
        ..Default::default()
    });

    renderer.render_vello(&scene, &view, ColorLoad::Clear(wgpu::Color::BLACK));
    renderer.wgpu_device.read_rgba8_texture(&target, VW, VH)
}

#[derive(Debug, Clone)]
enum Message {}

struct App {
    handle: image::Handle,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let bytes = render_document_to_rgba();
        let handle = image::Handle::from_rgba(VW, VH, bytes);
        (Self { handle }, Task::none())
    }

    fn update(&mut self, _msg: Message) -> Task<Message> {
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let header = text("middlenet-netrender-bridge / smoke_iced_image")
            .size(14.0);
        let img = image(self.handle.clone())
            .width(Length::Fixed(VW as f32))
            .height(Length::Fixed(VH as f32));
        let body = column![header, scrollable(img)].spacing(8);
        container(body).padding(8).into()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| "smoke_iced_image".to_string())
        .run()
}
