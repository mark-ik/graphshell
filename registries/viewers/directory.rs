use crate::registries::atomic::viewer::{EmbeddedViewer, EmbeddedViewerContext, EmbeddedViewerOutput};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

struct CachedDirectoryListing {
    url: String,
    entries: Vec<DirectoryEntry>,
}

struct DirectoryEntry {
    display_name: String,
    path: PathBuf,
    is_dir: bool,
}

thread_local! {
    static DIR_CACHE: RefCell<HashMap<crate::graph::NodeKey, CachedDirectoryListing>> =
        RefCell::new(HashMap::new());
}

pub(crate) struct DirectoryEmbeddedViewer;

impl EmbeddedViewer for DirectoryEmbeddedViewer {
    fn viewer_id(&self) -> &'static str {
        "viewer:directory"
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        ctx: &EmbeddedViewerContext<'_>,
    ) -> EmbeddedViewerOutput {
        let mut intents = Vec::new();
        match render_directory(ui, ctx, &mut intents) {
            Ok(()) => {}
            Err(err) => {
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), err);
            }
        }
        EmbeddedViewerOutput {
            intents,
            app_commands: Vec::new(),
        }
    }
}

fn render_directory(
    ui: &mut egui::Ui,
    ctx: &EmbeddedViewerContext<'_>,
    intents: &mut Vec<crate::app::GraphIntent>,
) -> Result<(), String> {
    let path =
        crate::shell::desktop::workbench::tile_behavior::guarded_file_path_from_node_url(ctx.node_url, ctx.file_access_policy)?;

    let entries: Vec<(String, PathBuf, bool)> = DIR_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(&ctx.node_key) {
            if cached.url == ctx.node_url {
                return Ok::<_, String>(cached.entries.iter().map(|e| {
                    (e.display_name.clone(), e.path.clone(), e.is_dir)
                }).collect());
            }
        }

        let read_dir = std::fs::read_dir(&path)
            .map_err(|err| format!("Failed to read directory '{}': {err}", path.display()))?;

        let mut dir_entries: Vec<DirectoryEntry> = read_dir
            .filter_map(|entry| entry.ok())
            .map(|entry| {
                let entry_path = entry.path();
                let is_dir = entry_path.is_dir();
                let display_name = entry.file_name().to_string_lossy().into_owned();
                DirectoryEntry { display_name, path: entry_path, is_dir }
            })
            .collect();

        dir_entries.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));

        let result: Vec<(String, PathBuf, bool)> = dir_entries.iter().map(|e| {
            (e.display_name.clone(), e.path.clone(), e.is_dir)
        }).collect();

        cache.insert(ctx.node_key, CachedDirectoryListing {
            url: ctx.node_url.to_string(),
            entries: dir_entries,
        });

        Ok(result)
    })?;

    // Parent directory navigation
    if let Some(parent) = path.parent() {
        if ui.button("..").clicked() {
            if let Ok(parent_url) = url::Url::from_file_path(parent) {
                intents.push(crate::app::GraphIntent::SetNodeUrl {
                    key: ctx.node_key,
                    new_url: parent_url.to_string(),
                });
            }
        }
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (display_name, entry_path, is_dir) in &entries {
            let label = if *is_dir {
                format!("[dir] {display_name}")
            } else {
                display_name.clone()
            };
            if ui.button(label).clicked() {
                if let Ok(entry_url) = url::Url::from_file_path(entry_path) {
                    intents.push(crate::app::GraphIntent::SetNodeUrl {
                        key: ctx.node_key,
                        new_url: entry_url.to_string(),
                    });
                }
            }
        }
    });

    Ok(())
}
