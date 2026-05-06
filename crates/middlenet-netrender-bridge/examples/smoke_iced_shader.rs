/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Level-1b smoke: iced shader widget hosts netrender on iced's wgpu
//! device, with vello rendering into an intermediate Rgba8Unorm
//! texture and `wgpu::util::TextureBlitter` copying onto iced's
//! BGRA target through iced's command encoder.
//!
//! This is the architecturally-blessed shape per vello's own docstring
//! at `vello/src/lib.rs:460-473`:
//!
//!   "If you want to render Vello content to a surface (such as in a
//!    UI toolkit), you have two options:
//!     1) Render to an intermediate texture, which is the same size as
//!        the surface. You would then use `TextureBlitter` to blit the
//!        rendered result from that texture to the surface.
//!     2) Call render_to_texture directly on the SurfaceTexture's
//!        texture. This should generally be avoided..."
//!
//! Option 1 it is.
//!
//! What this validates if it works:
//! 1. Fake-handles cross-instance approach: `WgpuHandles { instance: fresh,
//!    adapter: fresh, device: iced's, queue: iced's }` is accepted by
//!    `create_netrender_instance` and survives all the way through a
//!    full vello dispatch. **Verified** — the v1 of this file got past
//!    `BridgePipeline::new` and into vello's `TransientBindMap::create_bind_group`
//!    before tripping on the format mismatch (Rgba8Unorm storage binding vs
//!    Bgra8Unorm view).
//! 2. Vello → iced compositing: vello renders into intermediate texture
//!    (Rgba8Unorm, vello's required format), `TextureBlitter` does a
//!    full-screen-quad blit onto iced's actual swapchain target through
//!    iced's encoder (so iced's frame model owns the submit timing).
//! 3. Coexistence: vello's own command encoder + submit happens during
//!    `render_vello`; iced's encoder is used afterwards for the blit;
//!    iced's frame submission picks up the blit's output as part of the
//!    iced widget tree's normal render pass.
//!
//! Run with:
//!   cargo run --example smoke_iced_shader -p middlenet-netrender-bridge

use std::sync::{Arc, Mutex};

use iced::widget::shader::{self, Viewport};
use iced::{Element, Length, Rectangle, Task, mouse, widget::shader::Shader};
use middlenet_core::document::{
    DocumentMeta, DocumentProvenance, SemanticBlock, SemanticDocument,
};
use middlenet_core::source::{MiddleNetContentKind, MiddleNetSource};
use middlenet_netrender_bridge::{SceneAdapter, ThemeColors};
use middlenet_render::{RenderMode, RenderRequest, ThemeTokens, render_document};
use netrender::{
    ColorLoad, NetrenderOptions, Renderer as NetRenderer, Scene, WgpuHandles,
    create_netrender_instance,
};

const VW: u32 = 720;
const VH: u32 = 1100;
const TILE: u32 = 64;

fn sample_document() -> SemanticDocument {
    let source = MiddleNetSource::new(MiddleNetContentKind::Markdown)
        .with_uri("https://example.com/blog/post.md")
        .with_title_hint("Bridge Smoke (shader widget)");
    SemanticDocument::new(
        DocumentMeta::for_source(&source),
        DocumentProvenance::for_source(&source),
        vec![
            SemanticBlock::Heading {
                level: 1,
                text: "Netrender via iced shader widget".to_string(),
            },
            SemanticBlock::Paragraph(
                "Level-1b smoke. iced gives the shader Pipeline its wgpu \
                 Device + Queue; we construct a fresh wgpu::Instance + \
                 Adapter alongside them and pack the four into a \
                 netrender WgpuHandles. Vello renders into a private \
                 Rgba8Unorm intermediate texture, then \
                 wgpu::util::TextureBlitter copies that into iced's \
                 swapchain target through iced's command encoder."
                    .to_string(),
            ),
            SemanticBlock::Heading {
                level: 2,
                text: "What you should see".to_string(),
            },
            SemanticBlock::List {
                ordered: false,
                items: vec![
                    "This document, rendered by netrender + vello".to_string(),
                    "Inside an iced::widget::shader::Program".to_string(),
                    "Through iced's wgpu Device (no readback)".to_string(),
                    "Compositing via TextureBlitter for format conversion".to_string(),
                ],
            },
            SemanticBlock::Quote(
                "If this renders, the cross-instance + cross-format \
                 plumbing is solid; the rest is just exposing knobs \
                 and wiring real document data."
                    .to_string(),
            ),
            SemanticBlock::Rule,
            SemanticBlock::Badge {
                text: "LEVEL 1B".to_string(),
            },
            SemanticBlock::MetadataRow {
                label: "Path".to_string(),
                value: "shader::Program → fake-handles → vello → blitter".to_string(),
            },
        ],
    )
}

#[derive(Debug)]
struct BridgePrimitive {
    scene: Arc<Mutex<Scene>>,
}

impl shader::Primitive for BridgePrimitive {
    type Pipeline = BridgePipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        let w = bounds.width.max(1.0) as u32;
        let h = bounds.height.max(1.0) as u32;
        pipeline.ensure_intermediate(device, w, h);
    }

    fn render(
        &self,
        pipeline: &Self::Pipeline,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        _clip_bounds: &Rectangle<u32>,
    ) {
        let intermediate = pipeline
            .intermediate
            .lock()
            .expect("intermediate mutex poisoned");
        let Some(ref intermediate) = *intermediate else {
            return;
        };

        // 1. vello renders into the Rgba8Unorm intermediate. render_vello
        //    creates + submits its own command encoder (vello plan §2.3);
        //    by the time it returns the intermediate texture has the
        //    rendered pixels.
        let scene = self.scene.lock().expect("scene mutex poisoned");
        pipeline.renderer.render_vello(
            &scene,
            &intermediate.view,
            ColorLoad::Clear(wgpu::Color::BLACK),
        );

        // 2. TextureBlitter does a full-screen-quad blit from the
        //    intermediate (any format) onto iced's target (the format
        //    the blitter was configured for at Pipeline::new). iced's
        //    encoder is used here, so the blit lands in iced's frame
        //    submission alongside whatever else iced is drawing.
        pipeline
            .blitter
            .copy(&pipeline.device, encoder, &intermediate.view, target);
    }
}

struct IntermediateTexture {
    #[allow(dead_code)] // kept alive so the view stays valid
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

/// Holds netrender::Renderer constructed from iced's device + queue
/// plus a fresh wgpu::Instance/Adapter (the fake-handles workaround),
/// along with the intermediate Rgba8Unorm texture vello renders into
/// and the wgpu blitter that composites onto iced's actual target.
struct BridgePipeline {
    renderer: NetRenderer,
    device: wgpu::Device,
    blitter: wgpu::util::TextureBlitter,
    intermediate: Mutex<Option<IntermediateTexture>>,
}

impl BridgePipeline {
    fn ensure_intermediate(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        let mut guard = self
            .intermediate
            .lock()
            .expect("intermediate mutex poisoned");
        let needs_create = guard.as_ref().is_none_or(|t| t.width != w || t.height != h);
        if !needs_create {
            return;
        }
        eprintln!(
            "BridgePipeline: (re)creating intermediate Rgba8Unorm texture {}x{}",
            w, h
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("middlenet-bridge intermediate"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Rgba8Unorm is required by vello (per `vello/src/lib.rs:462`).
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("middlenet-bridge intermediate view"),
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            ..Default::default()
        });
        *guard = Some(IntermediateTexture {
            texture,
            view,
            width: w,
            height: h,
        });
    }
}

impl shader::Pipeline for BridgePipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        eprintln!(
            "BridgePipeline::new — iced target format = {:?}, constructing fake handles",
            format
        );
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            },
        ))
        .expect("fake-handles: no adapter from fresh wgpu::Instance");

        let handles = WgpuHandles {
            instance,
            adapter,
            device: device.clone(),
            queue: queue.clone(),
        };

        let renderer = create_netrender_instance(
            handles,
            NetrenderOptions {
                tile_cache_size: Some(TILE),
                enable_vello: true,
            },
        )
        .expect("create_netrender_instance — fake-handles WgpuHandles rejected by netrender");

        let blitter = wgpu::util::TextureBlitter::new(device, format);

        eprintln!("BridgePipeline::new — netrender + blitter ready");
        Self {
            renderer,
            device: device.clone(),
            blitter,
            intermediate: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {}

struct App {
    scene: Arc<Mutex<Scene>>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
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
            "App::new — render_document produced {} blocks",
            render_scene.blocks.len()
        );

        let mut adapter = SceneAdapter::new(theme, ThemeColors::dark())
            .expect("SceneAdapter — no system font found");

        let mut scene = Scene::new(VW, VH);
        adapter.build_scene(&mut scene, &render_scene, VW as f32, VH as f32);
        eprintln!("App::new — netrender Scene encoded; handing to shader widget");

        (
            Self {
                scene: Arc::new(Mutex::new(scene)),
            },
            Task::none(),
        )
    }

    fn update(&mut self, _msg: Message) -> Task<Message> {
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        Shader::new(BridgeProgram {
            scene: self.scene.clone(),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

struct BridgeProgram {
    scene: Arc<Mutex<Scene>>,
}

impl<Message> shader::Program<Message> for BridgeProgram {
    type State = ();
    type Primitive = BridgePrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        BridgePrimitive {
            scene: self.scene.clone(),
        }
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| "smoke_iced_shader (level-1b, blitter)".to_string())
        .run()
}
