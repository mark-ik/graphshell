use crate::registries::atomic::viewer::{
    EmbeddedViewer, EmbeddedViewerContext, EmbeddedViewerOutput,
};
use std::cell::RefCell;
use std::collections::HashMap;

struct PdfCacheEntry {
    texture: egui::TextureHandle,
    page_count: u16,
    source_path: String,
}

thread_local! {
    static PDF_CACHE: RefCell<HashMap<crate::graph::NodeKey, PdfCacheEntry>> =
        RefCell::new(HashMap::new());
}

pub(crate) struct PdfEmbeddedViewer;

impl EmbeddedViewer for PdfEmbeddedViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:pdf"
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput {
        match render_pdf(ui, ctx) {
            Ok(()) => {}
            Err(err) => {
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), err);
            }
        }
        EmbeddedViewerOutput::empty()
    }
}

fn render_pdf(ui: &mut egui::Ui, ctx: &EmbeddedViewerContext<'_>) -> Result<(), String> {
    let path =
        crate::shell::desktop::workbench::tile_behavior::guarded_file_path_from_node_url(
            ctx.node_url,
            ctx.file_access_policy,
        )?;
    let path_str = path.to_string_lossy().to_string();
    let node_key = ctx.node_key;

    // Check cache — return early if the cached texture is still valid.
    let cached = PDF_CACHE.with(|cache| {
        let cache = cache.borrow();
        cache
            .get(&node_key)
            .filter(|e| e.source_path == path_str)
            .map(|e| (e.texture.clone(), e.page_count))
    });

    if let Some((texture, page_count)) = cached {
        draw_pdf_page(ui, &texture, page_count);
        return Ok(());
    }

    // Bind to PDFium runtime library.
    use pdfium_render::prelude::*;

    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .or_else(|_| {
                Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            })
            .map_err(|e| {
                format!(
                    "PDFium library not found: {e}. \
                     Place the PDFium shared library next to the graphshell binary \
                     or install it system-wide to enable native PDF rendering."
                )
            })?,
    );

    let document = pdfium
        .load_pdf_from_file(&path, None)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;

    let pages = document.pages();
    let page_count = pages.len() as u16;

    if page_count == 0 {
        ui.label("PDF has no pages.");
        return Ok(());
    }

    let page = pages
        .get(0)
        .map_err(|e| format!("Failed to load first page: {e}"))?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(800)
        .set_maximum_height(1200);

    let bitmap = page
        .render_with_config(&render_config)
        .map_err(|e| format!("Failed to render page: {e}"))?;

    let image_buf = bitmap.as_image();
    let rgba = image_buf.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());

    let texture = ui.ctx().load_texture(
        format!("pdf-{node_key}-p0"),
        color_image,
        egui::TextureOptions::LINEAR,
    );

    PDF_CACHE.with(|cache| {
        cache.borrow_mut().insert(
            node_key,
            PdfCacheEntry {
                texture: texture.clone(),
                page_count,
                source_path: path_str,
            },
        );
    });

    draw_pdf_page(ui, &texture, page_count);
    Ok(())
}

fn draw_pdf_page(ui: &mut egui::Ui, texture: &egui::TextureHandle, page_count: u16) {
    ui.vertical(|ui| {
        ui.small(format!("PDF — page 1 of {page_count}"));
        let available = ui.available_size();
        let tex_size = texture.size_vec2();
        let scale = (available.x / tex_size.x).min(1.0);
        ui.image(egui::load::SizedTexture::new(
            texture.id(),
            tex_size * scale,
        ));
    });
}
