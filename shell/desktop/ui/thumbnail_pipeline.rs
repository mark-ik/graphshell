/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::mpsc::{Receiver, Sender};

use graphshell_core::content::ViewerInstanceId;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use log::warn;
use serde_json::Value;
use servo::{Image, PixelFormat, WebViewId};

use crate::app::{GraphBrowserApp, GraphIntent};
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_status_sync::{
    renderer_id_from_servo, servo_webview_id_from_viewer_instance, viewer_instance_id_from_servo,
};
use crate::shell::desktop::render_backend::{texture_id_from_token, texture_token_from_handle};

// Historical defaults; new `chrome_ui.thumbnail_settings` overrides
// these for the live capture path. Kept as constants so the cached
// thumbnail bytes stored under the old defaults can still be returned
// from `cached_thumbnail_result_for_request` with their original
// dimensions until the pipeline re-captures them at the new size.
const DEFAULT_NODE_THUMBNAIL_WIDTH: u32 = 256;
const DEFAULT_NODE_THUMBNAIL_HEIGHT: u32 = 192;

pub(crate) type RendererFaviconTextureCache =
    HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>;

fn thumbnail_url_cache_key(node_key: crate::graph::NodeKey) -> String {
    format!("thumbnail:url:{}", node_key.index())
}

fn cached_thumbnail_result_for_request(
    graph_app: &GraphBrowserApp,
    webview_id: WebViewId,
    node_key: crate::graph::NodeKey,
    requested_url: &str,
) -> Option<ThumbnailCaptureResult> {
    let url_key = thumbnail_url_cache_key(node_key);
    let cached_url_value = graph_app
        .workspace
        .graph_runtime
        .runtime_caches
        .get_parsed_metadata(&url_key);
    let cached_url = cached_url_value.as_deref().and_then(Value::as_str)?;
    if cached_url != requested_url {
        return None;
    }
    let png_bytes = graph_app
        .workspace
        .graph_runtime
        .runtime_caches
        .get_thumbnail(node_key)
        .as_deref()
        .cloned()?;
    // Bugfix: decode the cached bytes once to recover the true
    // dimensions. Previously we returned the hardcoded defaults
    // (256×192), which — because downstream `set_node_thumbnail`
    // overwrites `node.thumbnail_width/height` whenever any field
    // differs — would corrupt the node's stored dimensions back to
    // defaults on every cache-hit frame when the user had configured
    // non-default thumbnail dimensions. The decode is a
    // microsecond-scale cost and only runs when a capture was
    // requested and the URL marker still matches. On decode failure
    // (corrupt cached bytes, format the decoder can't handle) we
    // fall back to defaults so the cache still serves its purpose.
    let (width, height) = image::load_from_memory(&png_bytes)
        .map(|img| (img.width(), img.height()))
        .unwrap_or((DEFAULT_NODE_THUMBNAIL_WIDTH, DEFAULT_NODE_THUMBNAIL_HEIGHT));
    Some(ThumbnailCaptureResult {
        webview_id,
        requested_url: requested_url.to_string(),
        png_bytes: Some(png_bytes),
        width,
        height,
    })
}

/// Map the user-facing [`ThumbnailFilter`](crate::app::ThumbnailFilter)
/// enum onto the concrete `image` crate resampling filter. Kept narrow
/// so settings surface can expand (CatmullRom, Lanczos3, etc.) without
/// the pipeline needing to know the new variants.
fn resolve_filter(filter: crate::app::ThumbnailFilter) -> FilterType {
    match filter {
        crate::app::ThumbnailFilter::Nearest => FilterType::Nearest,
        crate::app::ThumbnailFilter::Triangle => FilterType::Triangle,
        crate::app::ThumbnailFilter::CatmullRom => FilterType::CatmullRom,
        crate::app::ThumbnailFilter::Gaussian => FilterType::Gaussian,
        crate::app::ThumbnailFilter::Lanczos3 => FilterType::Lanczos3,
    }
}

/// Downscale `source` per the user's aspect policy. Returns a
/// `DynamicImage` whose dimensions depend on the aspect mode:
/// - `Fixed` ⇒ exactly `target_width × target_height` (crops to fit,
///   same as the pre-aspect-option behavior).
/// - `MatchSource` ⇒ preserves source aspect; longer side is
///   `max(target_width, target_height)`.
/// - `Square` ⇒ `target_width × target_width` (crops to 1:1).
///
/// `resize_to_fill` crops; `resize` preserves aspect by fitting within
/// the target bounding box. Split because MatchSource needs the
/// preserve-aspect semantic.
fn resize_for_aspect(
    source: DynamicImage,
    aspect: crate::app::ThumbnailAspect,
    target_width: u32,
    target_height: u32,
    filter: FilterType,
) -> DynamicImage {
    match aspect {
        crate::app::ThumbnailAspect::Fixed => {
            source.resize_to_fill(target_width, target_height, filter)
        }
        crate::app::ThumbnailAspect::Square => {
            source.resize_to_fill(target_width, target_width, filter)
        }
        crate::app::ThumbnailAspect::MatchSource => {
            // `resize` fits inside the (w, h) box while preserving
            // aspect; passing the longer side for both dims makes the
            // resulting longest side equal to `max_dim`.
            let max_dim = target_width.max(target_height);
            source.resize(max_dim, max_dim, filter)
        }
    }
}

pub(crate) struct ThumbnailCaptureResult {
    pub(crate) webview_id: WebViewId,
    pub(crate) requested_url: String,
    /// Encoded thumbnail bytes. Historically PNG-only; with
    /// `ThumbnailFormat::Jpeg` the bytes are a JPEG stream instead.
    /// Consumers decode via `image::load_from_memory` which detects the
    /// format from the magic bytes, so the field name is kept to avoid
    /// a wider rename while the migration is in flight.
    pub(crate) png_bytes: Option<Vec<u8>>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// Host-neutral bundle holding the mpsc sender + receiver halves the
/// thumbnail pipeline uses to hand async screenshot results back from
/// `webview.take_screenshot(...)` callbacks into the synchronous
/// frame-loop consumer.
///
/// Before M4.1 session 4 follow-ons, the two halves were separate
/// fields on `EguiHost`; the runtime-facing helpers took
/// `&Sender<ThumbnailCaptureResult>` / `&Receiver<…>` through several
/// arg-struct hops. Consolidating them here gives the iced host (and
/// any future host) one named surface to construct and borrow from —
/// `ThumbnailChannel::new()` on construction, `&ThumbnailChannel`
/// through the phase pipeline, `clone_sender()` for the async capture
/// callback, `try_recv()` for the per-frame drain.
///
/// The channel itself is plain `std::sync::mpsc`; this type is just
/// packaging. A future `BackendThumbnailPort` trait can wrap the same
/// shape if we grow a need (e.g., swapping in a tokio channel for the
/// iced host). For now it's a struct so no dispatch cost and trivial
/// interop with the existing servo callback signature.
pub(crate) struct ThumbnailChannel {
    tx: Sender<ThumbnailCaptureResult>,
    rx: Receiver<ThumbnailCaptureResult>,
}

impl ThumbnailChannel {
    pub(crate) fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self { tx, rx }
    }

    /// Produce a sender clone suitable for moving into an async
    /// `webview.take_screenshot(...)` callback.
    pub(crate) fn clone_sender(&self) -> Sender<ThumbnailCaptureResult> {
        self.tx.clone()
    }

    /// Non-blocking receive — `Ok(result)` if a capture completed since
    /// the last call, `Err(_)` when the queue is empty or the channel
    /// closed.
    pub(crate) fn try_recv(&self) -> Result<ThumbnailCaptureResult, std::sync::mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

impl Default for ThumbnailChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Host-neutral port for async thumbnail result delivery. The
/// pipeline helpers borrow `&dyn BackendThumbnailPort` instead of
/// `&ThumbnailChannel` directly so a future iced host (or any
/// non-mpsc-backed host) can supply its own implementation — e.g., a
/// tokio-channel adapter or a direct synchronous dispatcher — without
/// changing the pipeline.
///
/// Shape rationale:
/// - `clone_sender()` returns a concrete `Sender<ThumbnailCaptureResult>`
///   because `webview.take_screenshot(...)`'s async callback signature
///   requires `FnOnce + Send + 'static` and `mpsc::Sender` already
///   satisfies that cleanly. Hosts that want to use a different queue
///   internally can supply a small mpsc→other-queue adapter task; the
///   pipeline doesn't care.
/// - `try_recv()` returns `Option` (not `Result<_, TryRecvError>`) to
///   keep the trait backend-agnostic. A "queue empty" vs "channel
///   disconnected" distinction isn't meaningful here — the pipeline
///   drains what's available and moves on.
pub(crate) trait BackendThumbnailPort {
    /// Produce a sender clone suitable for moving into the async
    /// screenshot callback.
    fn clone_sender(&self) -> Sender<ThumbnailCaptureResult>;

    /// Non-blocking receive — `Some(result)` if a capture completed
    /// since the last call, `None` otherwise.
    fn try_recv(&self) -> Option<ThumbnailCaptureResult>;
}

impl BackendThumbnailPort for ThumbnailChannel {
    fn clone_sender(&self) -> Sender<ThumbnailCaptureResult> {
        self.clone_sender()
    }

    fn try_recv(&self) -> Option<ThumbnailCaptureResult> {
        self.try_recv().ok()
    }
}

/// Encode an RGBA8 resized screenshot into the bytes we cache and hand
/// back to the host. Returns `None` when the encoder errors, which
/// collapses into a `ThumbnailCaptureResult { png_bytes: None, … }` at
/// the call site — `in_flight` still gets cleared, and
/// `graph_intent_for_thumbnail_result` will skip emitting an intent for
/// that node so the pipeline self-heals.
///
/// Field name `png_bytes` on `ThumbnailCaptureResult` predates the
/// format knob; downstream decoders use `image::load_from_memory`
/// (magic-byte sniffing) so JPEG and PNG bytes are both decoded
/// correctly regardless of the legacy field name.
fn encode_thumbnail(
    resized: image::RgbaImage,
    format: crate::app::ThumbnailFormat,
    jpeg_quality: u8,
    id: WebViewId,
) -> Option<Vec<u8>> {
    let (width, height) = resized.dimensions();
    match format {
        crate::app::ThumbnailFormat::Png => {
            let mut cursor = Cursor::new(Vec::new());
            match DynamicImage::ImageRgba8(resized).write_to(&mut cursor, ImageFormat::Png) {
                Ok(()) => Some(cursor.into_inner()),
                Err(error) => {
                    warn!("Could not encode thumbnail PNG for {id:?}: {error}");
                    None
                }
            }
        }
        crate::app::ThumbnailFormat::Jpeg => {
            // JPEG has no alpha channel. Composite RGBA over a white
            // backdrop before encoding; transparent regions render
            // white. Screenshots are usually opaque anyway so this
            // only affects edge cases (transparent popups, etc.).
            let rgb = DynamicImage::ImageRgba8(resized).to_rgb8();
            let quality = jpeg_quality.clamp(
                crate::app::ThumbnailSettings::MIN_JPEG_QUALITY,
                crate::app::ThumbnailSettings::MAX_JPEG_QUALITY,
            );
            let mut cursor = Cursor::new(Vec::new());
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
            match encoder.encode(rgb.as_raw(), width, height, image::ExtendedColorType::Rgb8) {
                Ok(()) => Some(cursor.into_inner()),
                Err(error) => {
                    warn!("Could not encode thumbnail JPEG for {id:?}: {error}");
                    None
                }
            }
        }
        crate::app::ThumbnailFormat::WebP => {
            // Lossless WebP preserves alpha — unlike JPEG — so we
            // pass the RGBA buffer through directly. `image` 0.25's
            // built-in encoder is lossless-only; see the doc on
            // `ThumbnailFormat::WebP` for why we don't expose a
            // quality knob here.
            let mut cursor = Cursor::new(Vec::new());
            match DynamicImage::ImageRgba8(resized).write_to(&mut cursor, ImageFormat::WebP) {
                Ok(()) => Some(cursor.into_inner()),
                Err(error) => {
                    warn!("Could not encode thumbnail WebP for {id:?}: {error}");
                    None
                }
            }
        }
    }
}

pub(crate) fn request_pending_thumbnail_captures(
    graph_app: &GraphBrowserApp,
    window: &EmbedderWindow,
    channel: &dyn BackendThumbnailPort,
    in_flight: &mut HashSet<ViewerInstanceId>,
) {
    // Retain only in-flight ids whose webview still exists. Paired
    // with the matching retain at the top of
    // `load_pending_thumbnail_results` so either entry point catches a
    // webview disappearance regardless of call ordering; the second
    // call is a no-op within a single frame.
    //
    // The set stores portable `ViewerInstanceId`s now; decode each
    // back to its servo `WebViewId` for the window-side membership
    // check. Non-servo ids (future Wry/iced_webview entries) fall
    // through to `false` and are dropped — the current pipeline only
    // submits servo captures, so that branch is defensive.
    in_flight.retain(|id| {
        servo_webview_id_from_viewer_instance(id)
            .is_some_and(|servo_id| window.contains_webview(servo_id))
    });

    let settings = graph_app.workspace.chrome_ui.thumbnail_settings;
    // Kill switch: when the user has disabled thumbnails (privacy
    // preference, low-memory device, perf-sensitive mode), we still
    // honor pending requests so the window-side queue doesn't back up,
    // but we skip any capture or cache lookup. In-flight captures
    // complete normally through `load_pending_thumbnail_results`.
    if !settings.enabled {
        // Drain pending requests so the window doesn't accumulate
        // stale requests while the user has captures disabled.
        for _ in window.take_pending_thumbnail_capture_requests() {}
        return;
    }
    let target_width = settings.width;
    let target_height = settings.height;
    let filter = resolve_filter(settings.filter);
    let format = settings.format;
    let jpeg_quality = settings.jpeg_quality;
    let aspect = settings.aspect;

    for id in window.take_pending_thumbnail_capture_requests() {
        let portable_id = viewer_instance_id_from_servo(id);
        if in_flight.contains(&portable_id) {
            continue;
        }

        let Some(node_key) = graph_app.get_node_for_webview(renderer_id_from_servo(id)) else {
            continue;
        };
        let Some(node) = graph_app.domain_graph().get_node(node_key) else {
            continue;
        };

        let requested_url = graph_app
            .runtime_display_url_for_node(node_key)
            .unwrap_or_else(|| node.url().to_string());
        if requested_url.starts_with("about:blank") {
            continue;
        }

        if let Some(cached_result) =
            cached_thumbnail_result_for_request(graph_app, id, node_key, &requested_url)
        {
            let _ = channel.clone_sender().send(cached_result);
            continue;
        }

        let Some(webview) = window.webview_by_id(id) else {
            continue;
        };

        let sender = channel.clone_sender();
        in_flight.insert(portable_id);
        webview.take_screenshot(None, move |result| {
            let (png_bytes, width, height) = match result {
                Ok(image) => {
                    let resized = resize_for_aspect(
                        DynamicImage::ImageRgba8(image),
                        aspect,
                        target_width,
                        target_height,
                        filter,
                    )
                    .to_rgba8();
                    let (width, height) = resized.dimensions();
                    let bytes = encode_thumbnail(resized, format, jpeg_quality, id);
                    (bytes, width, height)
                }
                Err(error) => {
                    warn!("Could not capture thumbnail for {id:?}: {error:?}");
                    (None, 0, 0)
                }
            };
            let _ = sender.send(ThumbnailCaptureResult {
                webview_id: id,
                requested_url,
                png_bytes,
                width,
                height,
            });
        });
    }
}

pub(crate) fn load_pending_thumbnail_results(
    graph_app: &GraphBrowserApp,
    window: &EmbedderWindow,
    channel: &dyn BackendThumbnailPort,
    in_flight: &mut HashSet<ViewerInstanceId>,
) -> Vec<GraphIntent> {
    // Retain only in-flight ids whose webview still exists. Mirrors
    // the retain in `request_pending_thumbnail_captures` so either
    // entry point reclaims stale ids on webview teardown. Within a
    // single frame one of the two calls is a no-op, but keeping both
    // makes the helpers safe to call independently (e.g., if a test
    // or iced bring-up path only invokes one side).
    in_flight.retain(|id| {
        servo_webview_id_from_viewer_instance(id)
            .is_some_and(|servo_id| window.contains_webview(servo_id))
    });
    let mut intents = Vec::new();

    while let Some(result) = channel.try_recv() {
        in_flight.remove(&viewer_instance_id_from_servo(result.webview_id));
        if let Some(intent) = graph_intent_for_thumbnail_result(graph_app, &result) {
            intents.push(intent);
        }
    }
    intents
}

pub(crate) fn graph_intent_for_thumbnail_result(
    graph_app: &GraphBrowserApp,
    result: &ThumbnailCaptureResult,
) -> Option<GraphIntent> {
    let node_key = graph_app.get_node_for_webview(renderer_id_from_servo(result.webview_id))?;
    let node = graph_app.domain_graph().get_node(node_key)?;
    let current_runtime_url = graph_app
        .runtime_display_url_for_node(node_key)
        .unwrap_or_else(|| node.url().to_string());
    if current_runtime_url != result.requested_url {
        return None;
    }
    let png_bytes = result.png_bytes.clone()?;
    graph_app
        .workspace
        .graph_runtime
        .runtime_caches
        .insert_thumbnail(node_key, png_bytes.clone());
    graph_app
        .workspace
        .graph_runtime
        .runtime_caches
        .insert_parsed_metadata(
            thumbnail_url_cache_key(node_key),
            Value::String(result.requested_url.clone()),
        );
    Some(GraphIntent::SetNodeThumbnail {
        key: node_key,
        png_bytes,
        width: result.width,
        height: result.height,
    })
}

fn embedder_image_to_rgba(image: &Image) -> (usize, usize, Vec<u8>) {
    let width = image.width as usize;
    let height = image.height as usize;

    let data = match image.format {
        PixelFormat::K8 => image.data().iter().flat_map(|&v| [v, v, v, 255]).collect(),
        PixelFormat::KA8 => image
            .data()
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        PixelFormat::RGB8 => image
            .data()
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        PixelFormat::RGBA8 => image.data().to_vec(),
        PixelFormat::BGRA8 => image
            .data()
            .chunks_exact(4)
            .flat_map(|chunk| [chunk[2], chunk[1], chunk[0], chunk[3]])
            .collect(),
    };

    (width, height, data)
}

pub(crate) fn load_pending_favicons(
    ctx: &egui::Context,
    window: &EmbedderWindow,
    graph_app: &GraphBrowserApp,
    renderer_favicon_textures: &mut RendererFaviconTextureCache,
) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    for id in window.take_pending_favicon_loads() {
        let Some(webview) = window.webview_by_id(id) else {
            continue;
        };
        let Some(favicon) = webview.favicon() else {
            continue;
        };

        let (width, height, rgba) = embedder_image_to_rgba(&favicon);
        let egui_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);
        let handle = ctx.load_texture(format!("favicon-{id:?}"), egui_image, Default::default());
        let texture_token = texture_token_from_handle(&handle);
        let texture = egui::load::SizedTexture::new(
            texture_id_from_token(texture_token),
            egui::vec2(favicon.width as f32, favicon.height as f32),
        );
        renderer_favicon_textures.insert(id, (handle, texture));

        if let Some(node_key) = graph_app.get_node_for_webview(renderer_id_from_servo(id)) {
            intents.push(GraphIntent::SetNodeFavicon {
                key: node_key,
                rgba,
                width: width as u32,
                height: height as u32,
            });
        }
    }
    intents
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }

    #[test]
    fn graph_intent_for_thumbnail_result_writes_cache_with_url_marker() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://cache.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(webview_id), node_key);

        let result = ThumbnailCaptureResult {
            webview_id,
            requested_url: "https://cache.example".to_string(),
            png_bytes: Some(vec![1, 2, 3]),
            width: DEFAULT_NODE_THUMBNAIL_WIDTH,
            height: DEFAULT_NODE_THUMBNAIL_HEIGHT,
        };

        let intent = graph_intent_for_thumbnail_result(&app, &result);
        assert!(matches!(
            intent,
            Some(GraphIntent::SetNodeThumbnail {
                key,
                width: DEFAULT_NODE_THUMBNAIL_WIDTH,
                height: DEFAULT_NODE_THUMBNAIL_HEIGHT,
                ..
            }) if key == node_key
        ));

        assert_eq!(
            app.workspace
                .graph_runtime
                .runtime_caches
                .get_thumbnail(node_key)
                .as_deref()
                .map(|bytes: &Vec<u8>| bytes.as_slice()),
            Some(&[1, 2, 3][..])
        );
        let marker_key = thumbnail_url_cache_key(node_key);
        assert_eq!(
            app.workspace
                .graph_runtime
                .runtime_caches
                .get_parsed_metadata(&marker_key)
                .as_deref()
                .and_then(Value::as_str),
            Some("https://cache.example")
        );
    }

    #[test]
    fn cached_thumbnail_result_requires_matching_url_marker() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://current.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(webview_id), node_key);

        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_thumbnail(node_key, vec![9, 9, 9]);
        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_parsed_metadata(
                thumbnail_url_cache_key(node_key),
                Value::String("https://stale.example".to_string()),
            );

        assert!(
            cached_thumbnail_result_for_request(
                &app,
                webview_id,
                node_key,
                "https://current.example"
            )
            .is_none()
        );

        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_parsed_metadata(
                thumbnail_url_cache_key(node_key),
                Value::String("https://current.example".to_string()),
            );
        let cached = cached_thumbnail_result_for_request(
            &app,
            webview_id,
            node_key,
            "https://current.example",
        )
        .expect("cached thumbnail should be returned when URL marker matches");
        assert_eq!(cached.width, DEFAULT_NODE_THUMBNAIL_WIDTH);
        assert_eq!(cached.height, DEFAULT_NODE_THUMBNAIL_HEIGHT);
        assert_eq!(cached.png_bytes.as_deref(), Some(&[9, 9, 9][..]));
    }

    // -----------------------------------------------------------------
    // Aspect + port backlog coverage
    // -----------------------------------------------------------------

    #[test]
    fn resize_for_aspect_fixed_crops_to_target_dimensions() {
        // Source 400×200 (2:1). Fixed 100×100 crops to exactly target.
        let source = DynamicImage::ImageRgba8(image::RgbaImage::new(400, 200));
        let result = resize_for_aspect(
            source,
            crate::app::ThumbnailAspect::Fixed,
            100,
            100,
            FilterType::Triangle,
        );
        assert_eq!(result.width(), 100);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn resize_for_aspect_match_source_preserves_source_ratio() {
        // Source 400×200 (2:1). MatchSource with max_dim=100 →
        // 100×50 (longer side = 100, aspect preserved).
        let source = DynamicImage::ImageRgba8(image::RgbaImage::new(400, 200));
        let result = resize_for_aspect(
            source,
            crate::app::ThumbnailAspect::MatchSource,
            100,
            100,
            FilterType::Triangle,
        );
        assert_eq!(result.width(), 100);
        assert_eq!(result.height(), 50);
    }

    #[test]
    fn resize_for_aspect_square_uses_width_for_both_dimensions() {
        let source = DynamicImage::ImageRgba8(image::RgbaImage::new(400, 200));
        let result = resize_for_aspect(
            source,
            crate::app::ThumbnailAspect::Square,
            120,
            999, // height is ignored in Square mode
            FilterType::Triangle,
        );
        assert_eq!(result.width(), 120);
        assert_eq!(result.height(), 120);
    }

    #[test]
    fn encode_thumbnail_round_trips_for_all_formats() {
        // Smoke-test each format. We don't care about byte-equality
        // after the round-trip; we care that encoding succeeds and the
        // returned bytes decode back to the original dimensions so an
        // `image::load_from_memory` consumer gets something sensible.
        let image = image::RgbaImage::from_pixel(16, 12, image::Rgba([200, 100, 50, 255]));
        let webview_id = test_webview_id();

        for (format, label) in [
            (crate::app::ThumbnailFormat::Png, "png"),
            (crate::app::ThumbnailFormat::Jpeg, "jpeg"),
            (crate::app::ThumbnailFormat::WebP, "webp"),
        ] {
            let bytes = encode_thumbnail(image.clone(), format, 85, webview_id)
                .unwrap_or_else(|| panic!("{label} encode should succeed"));
            let decoded = image::load_from_memory(&bytes)
                .unwrap_or_else(|error| panic!("{label} decode failed: {error}"));
            assert_eq!(decoded.width(), 16, "{label} decoded width mismatch");
            assert_eq!(decoded.height(), 12, "{label} decoded height mismatch");
        }
    }

    #[test]
    fn cached_thumbnail_result_recovers_real_dimensions_from_bytes() {
        // Regression test: previously the cache returned the hardcoded
        // default 256×192 regardless of the actual cached bytes, which
        // corrupted node.thumbnail_width/height on every cache hit
        // when the user had configured non-default dimensions.
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://real.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(webview_id), node_key);

        // Encode a real 400×300 PNG and store it in the cache. This
        // simulates a capture that ran under user settings width=400,
        // height=300 (not the hardcoded defaults).
        let sample = image::RgbaImage::from_pixel(400, 300, image::Rgba([10, 20, 30, 255]));
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(sample)
            .write_to(&mut png, ImageFormat::Png)
            .expect("encode should succeed");
        let png_bytes = png.into_inner();

        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_thumbnail(node_key, png_bytes);
        app.workspace
            .graph_runtime
            .runtime_caches
            .insert_parsed_metadata(
                thumbnail_url_cache_key(node_key),
                Value::String("https://real.example".to_string()),
            );

        let cached =
            cached_thumbnail_result_for_request(&app, webview_id, node_key, "https://real.example")
                .expect("cached result returned");
        assert_eq!(
            cached.width, 400,
            "cache must report the actual encoded width, not the hardcoded default"
        );
        assert_eq!(
            cached.height, 300,
            "cache must report the actual encoded height, not the hardcoded default"
        );
    }

    #[test]
    fn thumbnail_channel_implements_backend_port_via_dyn_dispatch() {
        // Pins that ThumbnailChannel flows through a &dyn port obj —
        // the shape the iced host will consume when it ships its own
        // channel type.
        let channel = ThumbnailChannel::new();
        let port: &dyn BackendThumbnailPort = &channel;
        assert!(port.try_recv().is_none());

        let sender = port.clone_sender();
        let webview_id = test_webview_id();
        sender
            .send(ThumbnailCaptureResult {
                webview_id,
                requested_url: "https://test.example".to_string(),
                png_bytes: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
                width: 8,
                height: 8,
            })
            .expect("send should succeed while receiver is live");
        let received = port.try_recv().expect("drain should yield one result");
        assert_eq!(received.width, 8);
        assert_eq!(
            received.png_bytes.as_deref(),
            Some(&[0xDE, 0xAD, 0xBE, 0xEF][..])
        );
        assert!(port.try_recv().is_none());
    }
}
