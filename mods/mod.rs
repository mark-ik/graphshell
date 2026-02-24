// Mod system for Graphshell
//
// Manages native and WASM mods that extend the application's capabilities.
// Native mods are compiled in at build time and registered via inventory.
// WASM mods (future) are dynamically loaded and sandboxed.

pub(crate) mod native;

pub(crate) use native::verse;
