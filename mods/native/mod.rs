// Native mods (compiled in at build time)

pub(crate) mod nostrcore;
pub(crate) mod verse;
pub(crate) mod web_runtime;

// ---------------------------------------------------------------------------
// Slice 68b — NativeModRuntime trait impl (DI for register-mod-loader)
// ---------------------------------------------------------------------------

/// Host-side bridge that adapts the existing `NativeModActivations`
/// dispatch table to the
/// [`NativeModRuntime`](crate::registries::infrastructure::mod_loader::NativeModRuntime)
/// trait so `ModRegistry` can dispatch through DI rather than calling
/// `NativeModActivations::new()` inline. The activations are
/// constructed lazily on first activate (matches the pre-Slice-68b
/// behaviour of construction-per-call; can be cached in a future
/// slice if profiling shows it matters).
pub(crate) struct GraphshellNativeRuntime;

impl crate::registries::infrastructure::mod_loader::NativeModRuntime for GraphshellNativeRuntime {
    fn activate(&self, mod_id: &str) -> Result<(), String> {
        let activations = crate::registries::infrastructure::NativeModActivations::new();
        activations.activate(mod_id)
    }
}
