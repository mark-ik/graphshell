use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use extism::{Manifest, Plugin, Wasm};
use wasmparser::{Parser, Payload};

use crate::registries::infrastructure::mod_loader::{
    ModManifest, ModType, WasmModSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HeadlessWasmGuestSurface {
    pub(crate) supports_update: bool,
}

#[derive(Debug)]
struct HeadlessWasmPlugin {
    #[allow(dead_code)]
    plugin: Plugin,
    #[allow(dead_code)]
    source: WasmModSource,
    surface: HeadlessWasmGuestSurface,
}

static HEADLESS_WASM_RUNTIME: OnceLock<Mutex<HashMap<String, HeadlessWasmPlugin>>> =
    OnceLock::new();

fn headless_wasm_runtime() -> &'static Mutex<HashMap<String, HeadlessWasmPlugin>> {
    HEADLESS_WASM_RUNTIME.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_headless_plugin<T>(
    mod_id: &str,
    call: impl FnOnce(&mut HeadlessWasmPlugin) -> Result<T, String>,
) -> Result<T, String> {
    let mut runtime = headless_wasm_runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let plugin = runtime
        .get_mut(mod_id)
        .ok_or_else(|| format!("headless WASM mod not active: {mod_id}"))?;
    call(plugin)
}

fn run_guest_init(plugin: &mut Plugin, mod_id: &str) -> Result<(), String> {
    plugin
        .call::<_, Vec<u8>>("init", "")
        .map(|_| ())
        .map_err(|error| format!("failed running init for WASM mod {mod_id}: {error}"))
}

fn run_guest_render(
    plugin: &mut Plugin,
    mod_id: &str,
    render_context_json: &str,
) -> Result<String, String> {
    plugin.call("render", render_context_json).map_err(|error| {
        format!("failed running render for WASM mod {mod_id}: {error}")
    })
}

fn run_guest_event(plugin: &mut Plugin, mod_id: &str, event_json: &str) -> Result<(), String> {
    plugin
        .call::<_, Vec<u8>>("on_event", event_json)
        .map(|_| ())
        .map_err(|error| format!("failed running on_event for WASM mod {mod_id}: {error}"))
}

fn ensure_supported_capabilities(manifest: &ModManifest) -> Result<(), String> {
    if let Some(capability) = manifest.capabilities.first() {
        return Err(format!(
            "headless WASM activation denies capability {:?} for {}",
            capability, manifest.mod_id
        ));
    }
    Ok(())
}

fn inspect_guest_surface(module_path: &Path) -> Result<HeadlessWasmGuestSurface, String> {
    let bytes = std::fs::read(module_path)
        .map_err(|error| format!("failed reading WASM module {}: {error}", module_path.display()))?;
    let mut has_init = false;
    let mut has_render = false;
    let mut has_on_event = false;
    let mut has_update = false;

    for payload in Parser::new(0).parse_all(&bytes) {
        let payload = payload
            .map_err(|error| format!("failed parsing WASM module {}: {error}", module_path.display()))?;
        if let Payload::ExportSection(exports) = payload {
            for export in exports {
                let export = export.map_err(|error| {
                    format!("failed parsing WASM export table {}: {error}", module_path.display())
                })?;
                if export.name == "init" {
                    has_init = true;
                }
                if export.name == "render" {
                    has_render = true;
                }
                if export.name == "on_event" {
                    has_on_event = true;
                }
                if export.name == "update" {
                    has_update = true;
                }
            }
        }
    }

    if !has_init {
        return Err(format!(
            "WASM module {} is missing required export 'init'",
            module_path.display()
        ));
    }

    if !has_render {
        return Err(format!(
            "WASM module {} is missing required export 'render'",
            module_path.display()
        ));
    }

    if !has_on_event {
        return Err(format!(
            "WASM module {} is missing required export 'on_event'",
            module_path.display()
        ));
    }

    Ok(HeadlessWasmGuestSurface {
        supports_update: has_update,
    })
}

pub(crate) fn activate_mod_headless(
    manifest: &ModManifest,
    source: &WasmModSource,
) -> Result<(), String> {
    if manifest.mod_type != ModType::Wasm {
        return Err(format!("{} is not a WASM mod", manifest.mod_id));
    }

    ensure_supported_capabilities(manifest)?;
    let surface = inspect_guest_surface(&source.module_path)?;

    let mut plugin = Plugin::new(
        Manifest::new([Wasm::file(source.module_path.clone())]),
        [],
        true,
    )
    .map_err(|error| format!("failed to instantiate WASM mod {}: {error}", manifest.mod_id))?;
    run_guest_init(&mut plugin, &manifest.mod_id)?;

    headless_wasm_runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(
            manifest.mod_id.clone(),
            HeadlessWasmPlugin {
                plugin,
                source: source.clone(),
                surface,
            },
        );

    Ok(())
}

pub(crate) fn render_mod_headless(
    mod_id: &str,
    render_context_json: &str,
) -> Result<String, String> {
    with_headless_plugin(mod_id, |plugin| {
        run_guest_render(&mut plugin.plugin, mod_id, render_context_json)
    })
}

pub(crate) fn dispatch_event_headless(mod_id: &str, event_json: &str) -> Result<(), String> {
    with_headless_plugin(mod_id, |plugin| {
        run_guest_event(&mut plugin.plugin, mod_id, event_json)
    })
}

pub(crate) fn deactivate_mod_headless(mod_id: &str) -> Result<(), String> {
    headless_wasm_runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(mod_id)
        .map(|_| ())
        .ok_or_else(|| format!("headless WASM mod not active: {mod_id}"))
}

#[cfg(test)]
pub(crate) fn headless_mod_is_active(mod_id: &str) -> bool {
    headless_wasm_runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains_key(mod_id)
}

#[cfg(test)]
pub(crate) fn headless_mod_surface(mod_id: &str) -> Option<HeadlessWasmGuestSurface> {
    headless_wasm_runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(mod_id)
        .map(|plugin| plugin.surface)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::infrastructure::mod_loader::{
        ModCapability, ModManifest, ModType, WasmModSource,
    };
    use std::fs;

    fn write_wat_test_module(
        temp_dir: &tempfile::TempDir,
        module_name: &str,
        manifest_body: &str,
        wat_source: &str,
    ) -> WasmModSource {
        let module_path = temp_dir.path().join(format!("{module_name}.wasm"));
        let manifest_path = temp_dir.path().join(format!("{module_name}.wasm.toml"));
        let wasm_bytes = wat::parse_str(wat_source).expect("test WAT should compile");
        fs::write(&module_path, wasm_bytes).expect("test module should write");
        fs::write(&manifest_path, manifest_body).expect("test manifest should write");
        WasmModSource {
            module_path,
            manifest_path,
        }
    }

    fn write_test_module(
        temp_dir: &tempfile::TempDir,
        module_name: &str,
        manifest_body: &str,
        exports: &[&str],
    ) -> WasmModSource {
        let export_block = exports
            .iter()
            .map(|export| format!("(func (export \"{export}\"))"))
            .collect::<Vec<_>>()
            .join(" ");
        write_wat_test_module(
            temp_dir,
            module_name,
            manifest_body,
            &format!("(module {export_block})"),
        )
    }

    #[test]
    fn headless_activation_instantiates_and_tracks_plugin() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let source = write_test_module(
            &temp_dir,
            "headless-ok",
            "mod_id = \"mod:headless-ok\"\ndisplay_name = \"Headless OK\"\n",
            &["init", "render", "on_event", "update"],
        );
        let manifest = ModManifest::new(
            "mod:headless-ok",
            "Headless OK",
            ModType::Wasm,
            vec!["protocol:test".to_string()],
            vec![],
            vec![],
        );

        activate_mod_headless(&manifest, &source).expect("activation should succeed");
        assert!(headless_mod_is_active("mod:headless-ok"));
        assert_eq!(
            headless_mod_surface("mod:headless-ok"),
            Some(HeadlessWasmGuestSurface {
                supports_update: true,
            })
        );

        deactivate_mod_headless("mod:headless-ok").expect("deactivation should succeed");
        assert!(!headless_mod_is_active("mod:headless-ok"));
    }

    #[test]
    fn headless_activation_runs_init_export() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let source = write_wat_test_module(
            &temp_dir,
            "headless-init-trap",
            "mod_id = \"mod:headless-init-trap\"\ndisplay_name = \"Headless Init Trap\"\n",
            "(module (func (export \"init\") unreachable) (func (export \"render\")) (func (export \"on_event\")))",
        );
        let manifest = ModManifest::new(
            "mod:headless-init-trap",
            "Headless Init Trap",
            ModType::Wasm,
            vec![],
            vec![],
            vec![],
        );

        let error = activate_mod_headless(&manifest, &source)
            .expect_err("activation should fail when init traps");
        assert!(error.contains("failed running init"));
    }

    #[test]
    fn headless_activation_requires_render_and_on_event_exports() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let source = write_test_module(
            &temp_dir,
            "headless-missing-render",
            "mod_id = \"mod:headless-missing-render\"\ndisplay_name = \"Missing Render\"\n",
            &["init", "on_event"],
        );
        let manifest = ModManifest::new(
            "mod:headless-missing-render",
            "Missing Render",
            ModType::Wasm,
            vec![],
            vec![],
            vec![],
        );

        let error = activate_mod_headless(&manifest, &source)
            .expect_err("missing guest exports should be rejected");
        assert!(error.contains("missing required export 'render'"));
    }

    #[test]
    fn headless_activation_tracks_optional_update_export() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let source = write_test_module(
            &temp_dir,
            "headless-no-update",
            "mod_id = \"mod:headless-no-update\"\ndisplay_name = \"No Update\"\n",
            &["init", "render", "on_event"],
        );
        let manifest = ModManifest::new(
            "mod:headless-no-update",
            "No Update",
            ModType::Wasm,
            vec![],
            vec![],
            vec![],
        );

        activate_mod_headless(&manifest, &source).expect("activation should succeed");
        assert_eq!(
            headless_mod_surface("mod:headless-no-update"),
            Some(HeadlessWasmGuestSurface {
                supports_update: false,
            })
        );
        deactivate_mod_headless("mod:headless-no-update")
            .expect("deactivation should succeed");
    }

    #[test]
    fn headless_render_calls_guest_export() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let source = write_test_module(
            &temp_dir,
            "headless-render-ok",
            "mod_id = \"mod:headless-render-ok\"\ndisplay_name = \"Render OK\"\n",
            &["init", "render", "on_event"],
        );
        let manifest = ModManifest::new(
            "mod:headless-render-ok",
            "Render OK",
            ModType::Wasm,
            vec![],
            vec![],
            vec![],
        );

        activate_mod_headless(&manifest, &source).expect("activation should succeed");
        let output = render_mod_headless("mod:headless-render-ok", r#"{"frame":1}"#)
            .expect("render should succeed");
        assert_eq!(output, "");
        deactivate_mod_headless("mod:headless-render-ok")
            .expect("deactivation should succeed");
    }

    #[test]
    fn headless_render_propagates_guest_failure() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let source = write_wat_test_module(
            &temp_dir,
            "headless-render-trap",
            "mod_id = \"mod:headless-render-trap\"\ndisplay_name = \"Render Trap\"\n",
            "(module (func (export \"init\")) (func (export \"render\") unreachable) (func (export \"on_event\")))",
        );
        let manifest = ModManifest::new(
            "mod:headless-render-trap",
            "Render Trap",
            ModType::Wasm,
            vec![],
            vec![],
            vec![],
        );

        activate_mod_headless(&manifest, &source).expect("activation should succeed");
        let error = render_mod_headless("mod:headless-render-trap", r#"{"frame":1}"#)
            .expect_err("render should surface guest failure");
        assert!(error.contains("failed running render"));
        deactivate_mod_headless("mod:headless-render-trap")
            .expect("deactivation should succeed");
    }

    #[test]
    fn headless_event_dispatch_calls_guest_export() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let source = write_test_module(
            &temp_dir,
            "headless-event-ok",
            "mod_id = \"mod:headless-event-ok\"\ndisplay_name = \"Event OK\"\n",
            &["init", "render", "on_event"],
        );
        let manifest = ModManifest::new(
            "mod:headless-event-ok",
            "Event OK",
            ModType::Wasm,
            vec![],
            vec![],
            vec![],
        );

        activate_mod_headless(&manifest, &source).expect("activation should succeed");
        dispatch_event_headless("mod:headless-event-ok", r#"{"type":"click"}"#)
            .expect("on_event should succeed");
        deactivate_mod_headless("mod:headless-event-ok")
            .expect("deactivation should succeed");
    }

    #[test]
    fn headless_activation_rejects_capabilities_until_host_policies_exist() {
        let temp_dir = tempfile::tempdir().expect("temp dir should create");
        let source = write_test_module(
            &temp_dir,
            "headless-cap",
            "mod_id = \"mod:headless-cap\"\ndisplay_name = \"Headless Cap\"\n",
            &["init", "render", "on_event"],
        );
        let manifest = ModManifest::new(
            "mod:headless-cap",
            "Headless Cap",
            ModType::Wasm,
            vec![],
            vec![],
            vec![ModCapability::Network],
        );

        let error =
            activate_mod_headless(&manifest, &source).expect_err("capabilities should be denied");
        assert!(error.contains("denies capability"));
    }
}
