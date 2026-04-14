// Mod system for Graphshell
//
// Manages native and WASM mods that extend the application's capabilities.
// Native mods are compiled in at build time and registered via inventory.
// WASM mods (future) are dynamically loaded and sandboxed.

pub(crate) mod native;

#[cfg(not(any(target_os = "android", target_env = "ohos", target_os = "ios")))]
pub(crate) mod wasm;

#[cfg(any(target_os = "android", target_env = "ohos", target_os = "ios"))]
pub(crate) mod wasm {
	use crate::registries::infrastructure::mod_loader::{ModManifest, WasmModSource};

	pub(crate) fn activate_mod_headless(
		manifest: &ModManifest,
		_source: &WasmModSource,
	) -> Result<(), String> {
		Err(format!(
			"WASM mod '{}' is unsupported on this platform",
			manifest.mod_id
		))
	}

	pub(crate) fn deactivate_mod_headless(_mod_id: &str) -> Result<(), String> {
		Ok(())
	}

	pub(crate) fn render_mod_headless(
		mod_id: &str,
		_render_context_json: &str,
	) -> Result<String, String> {
		Err(format!("WASM mod '{}' is unsupported on this platform", mod_id))
	}

	pub(crate) fn dispatch_event_headless(
		mod_id: &str,
		_event_json: &str,
	) -> Result<(), String> {
		Err(format!("WASM mod '{}' is unsupported on this platform", mod_id))
	}
}

pub(crate) use native::verse;

