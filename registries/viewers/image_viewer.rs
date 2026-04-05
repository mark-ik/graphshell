use crate::registries::atomic::viewer::{EmbeddedViewer, EmbeddedViewerContext, EmbeddedViewerOutput};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_CACHED_TEXTURES: usize = 64;

#[derive(Clone)]
struct TextureCacheEntry {
    content_hash: u64,
    last_access_tick: u64,
    handle: egui::TextureHandle,
}

static ACCESS_COUNTER: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static TEXTURE_CACHE: RefCell<HashMap<crate::graph::NodeKey, TextureCacheEntry>> =
        RefCell::new(HashMap::new());
}

fn prune_texture_cache(cache: &mut HashMap<crate::graph::NodeKey, TextureCacheEntry>) {
    while cache.len() > MAX_CACHED_TEXTURES {
        let Some(evict_key) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_access_tick)
            .map(|(key, _)| *key)
        else {
            break;
        };
        cache.remove(&evict_key);
    }
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

pub(crate) struct ImageEmbeddedViewer;

impl EmbeddedViewer for ImageEmbeddedViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:image"
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput {
        match render_image(ui, ctx) {
            Ok(()) => {}
            Err(err) => {
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), err);
            }
        }
        EmbeddedViewerOutput::empty()
    }
}

fn render_image(ui: &mut egui::Ui, ctx: &EmbeddedViewerContext<'_>) -> Result<(), String> {
    let path =
        crate::shell::desktop::workbench::tile_behavior::guarded_file_path_from_node_url(ctx.node_url, ctx.file_access_policy)?;
    let bytes = std::fs::read(&path)
        .map_err(|err| format!("Failed to read '{}': {err}", path.display()))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|err| format!("Failed to decode image '{}': {err}", path.display()))?
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    let image_hash = hash_bytes(&bytes);
    let access_tick = ACCESS_COUNTER.fetch_add(1, Ordering::Relaxed);

    let handle = TEXTURE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.get_mut(&ctx.node_key) {
            if entry.content_hash == image_hash {
                entry.last_access_tick = access_tick;
                return entry.handle.clone();
            }
        }

        let handle = ui.ctx().load_texture(
            format!("embedded-image-{:?}-{image_hash}", ctx.node_key),
            color_image,
            Default::default(),
        );
        cache.insert(
            ctx.node_key,
            TextureCacheEntry {
                content_hash: image_hash,
                last_access_tick: access_tick,
                handle: handle.clone(),
            },
        );
        prune_texture_cache(&mut cache);
        handle
    });

    let available = ui.available_size();
    let image_size = egui::Vec2::new(size[0] as f32, size[1] as f32);
    let scale = ((available.x / image_size.x).min(available.y / image_size.y)).max(0.1);
    let desired = if available.x.is_finite() && available.y.is_finite() {
        if scale < 1.0 {
            image_size * scale
        } else {
            image_size
        }
    } else {
        image_size
    };

    egui::ScrollArea::both().show(ui, |ui| {
        ui.add(egui::Image::new((handle.id(), desired)));
        ui.small(format!("{} x {}", size[0], size[1]));
    });
    Ok(())
}
