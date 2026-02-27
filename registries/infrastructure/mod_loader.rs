use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::OnceLock;

// No direct DiagnosticsState import needed - this module emits via runtime diagnostics.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModType {
    Native,
    Wasm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModStatus {
    Discovered,
    Loading,
    Active,
    Failed,
    Unloaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ModCapability {
    Network,
    Filesystem,
    Identity,
    Clipboard,
    Exec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModManifest {
    pub(crate) mod_id: String,
    pub(crate) display_name: String,
    pub(crate) mod_type: ModType,
    pub(crate) provides: Vec<String>,
    pub(crate) requires: Vec<String>,
    pub(crate) capabilities: Vec<ModCapability>,
}

impl ModManifest {
    pub(crate) fn new(
        mod_id: impl Into<String>,
        display_name: impl Into<String>,
        mod_type: ModType,
        provides: Vec<String>,
        requires: Vec<String>,
        capabilities: Vec<ModCapability>,
    ) -> Self {
        Self {
            mod_id: mod_id.into(),
            display_name: display_name.into(),
            mod_type,
            provides,
            requires,
            capabilities,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModDependencyError {
    DuplicateModId(String),
    MissingRequirement { mod_id: String, requirement: String },
    DependencyCycle(Vec<String>),
}

#[derive(Debug, Clone)]
pub(crate) struct NativeModRegistration {
    pub(crate) manifest: fn() -> ModManifest,
}

inventory::collect!(NativeModRegistration);

pub(crate) fn discover_native_mods() -> Vec<ModManifest> {
    inventory::iter::<NativeModRegistration>
        .into_iter()
        .map(|registration| (registration.manifest)())
        .collect()
}

pub(crate) fn resolve_mod_load_order(
    manifests: &[ModManifest],
) -> Result<Vec<ModManifest>, ModDependencyError> {
    let mut id_to_manifest = HashMap::<String, ModManifest>::new();
    let mut provided_by = HashMap::<String, String>::new();

    for manifest in manifests {
        if id_to_manifest
            .insert(manifest.mod_id.clone(), manifest.clone())
            .is_some()
        {
            return Err(ModDependencyError::DuplicateModId(manifest.mod_id.clone()));
        }
        for provided in &manifest.provides {
            provided_by
                .entry(provided.clone())
                .or_insert_with(|| manifest.mod_id.clone());
        }
    }

    let mut indegree = HashMap::<String, usize>::new();
    let mut edges = HashMap::<String, HashSet<String>>::new();
    for id in id_to_manifest.keys() {
        indegree.insert(id.clone(), 0);
        edges.insert(id.clone(), HashSet::new());
    }

    for manifest in manifests {
        for requirement in &manifest.requires {
            let dependency_mod = provided_by.get(requirement).ok_or_else(|| {
                ModDependencyError::MissingRequirement {
                    mod_id: manifest.mod_id.clone(),
                    requirement: requirement.clone(),
                }
            })?;

            if dependency_mod == &manifest.mod_id {
                continue;
            }

            let deps = edges
                .get_mut(dependency_mod)
                .expect("dependency mod must exist");
            if deps.insert(manifest.mod_id.clone()) {
                *indegree
                    .get_mut(&manifest.mod_id)
                    .expect("mod indegree entry must exist") += 1;
            }
        }
    }

    let mut queue = VecDeque::new();
    for (id, degree) in &indegree {
        if *degree == 0 {
            queue.push_back(id.clone());
        }
    }

    let mut ordered = Vec::new();
    while let Some(id) = queue.pop_front() {
        let manifest = id_to_manifest
            .get(&id)
            .expect("mod id in queue must exist")
            .clone();
        ordered.push(manifest);

        if let Some(dependents) = edges.get(&id) {
            for dependent in dependents {
                let degree = indegree
                    .get_mut(dependent)
                    .expect("dependent indegree entry must exist");
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    queue.push_back(dependent.clone());
                }
            }
        }
    }

    if ordered.len() != manifests.len() {
        let unresolved = indegree
            .into_iter()
            .filter_map(|(id, degree)| if degree > 0 { Some(id) } else { None })
            .collect::<Vec<_>>();
        return Err(ModDependencyError::DependencyCycle(unresolved));
    }

    Ok(ordered)
}

/// Runtime registry managing mod lifecycle and status.
/// Handles discovery, dependency resolution, and activation of both native and WASM mods.
#[derive(Debug)]
pub(crate) struct ModRegistry {
    /// All discovered mods (native + future WASM)
    manifests: HashMap<String, ModManifest>,
    /// Current status of each mod
    status: HashMap<String, ModStatus>,
    /// Resolved load order (topologically sorted)
    load_order: Vec<String>,
    /// Disabled mods for this registry instance.
    disabled_mod_ids: HashSet<String>,
}

static ACTIVE_CAPABILITIES: OnceLock<HashSet<String>> = OnceLock::new();

fn parse_disabled_mod_ids_from_env() -> HashSet<String> {
    let mut disabled = HashSet::new();
    if let Ok(raw) = std::env::var("GRAPHSHELL_DISABLE_MODS") {
        for entry in raw.split([',', ';']) {
            let trimmed = entry.trim();
            if !trimmed.is_empty() {
                disabled.insert(trimmed.to_string());
            }
        }
    }
    if std::env::var("GRAPHSHELL_DISABLE_VERSO")
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "yes"
        })
        .unwrap_or(false)
    {
        disabled.insert("mod:verso".to_string());
    }
    disabled
}

fn compute_active_capabilities() -> HashSet<String> {
    let mut registry = ModRegistry::new();
    let _ = registry.resolve_dependencies();
    let _ = registry.load_all();
    registry.active_capability_ids()
}

pub(crate) fn runtime_has_capability(capability_id: &str) -> bool {
    ACTIVE_CAPABILITIES
        .get_or_init(compute_active_capabilities)
        .contains(capability_id)
}

impl ModRegistry {
    fn new_with_disabled(disabled_mod_ids: &HashSet<String>) -> Self {
        let discovered = discover_native_mods();
        let manifests = discovered
            .into_iter()
            .map(|m| (m.mod_id.clone(), m))
            .collect::<HashMap<_, _>>();

        let status = manifests
            .keys()
            .map(|id| {
                if disabled_mod_ids.contains(id) {
                    (id.clone(), ModStatus::Unloaded)
                } else {
                    (id.clone(), ModStatus::Discovered)
                }
            })
            .collect();

        Self {
            manifests,
            status,
            load_order: Vec::new(),
            disabled_mod_ids: disabled_mod_ids.clone(),
        }
    }

    /// Create a new ModRegistry and discover all native mods.
    /// Does not perform dependency resolution or loading yet.
    pub(crate) fn new() -> Self {
        let disabled_mod_ids = parse_disabled_mod_ids_from_env();
        Self::new_with_disabled(&disabled_mod_ids)
    }

    /// Resolve dependencies and compute load order.
    /// Returns error if dependencies are missing or cyclic.
    pub(crate) fn resolve_dependencies(&mut self) -> Result<(), ModDependencyError> {
        use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
        use crate::shell::desktop::runtime::registries::CHANNEL_MOD_DEPENDENCY_MISSING;

        let manifests_vec: Vec<_> = self
            .manifests
            .values()
            .filter(|manifest| !self.disabled_mod_ids.contains(&manifest.mod_id))
            .cloned()
            .collect();
        match resolve_mod_load_order(&manifests_vec) {
            Ok(ordered) => {
                self.load_order = ordered.iter().map(|m| m.mod_id.clone()).collect();
                Ok(())
            }
            Err(err) => {
                // Emit diagnostics for missing dependencies
                if let ModDependencyError::MissingRequirement {
                    mod_id,
                    requirement,
                } = &err
                {
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_MOD_DEPENDENCY_MISSING,
                        byte_len: mod_id.len() + requirement.len(),
                    });
                }
                Err(err)
            }
        }
    }

    /// Load all mods in dependency order.
    /// Emits lifecycle diagnostics for each mod.
    pub(crate) fn load_all(&mut self) -> Vec<String> {
        use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
        use crate::shell::desktop::runtime::registries::{
            CHANNEL_MOD_LOAD_FAILED, CHANNEL_MOD_LOAD_STARTED, CHANNEL_MOD_LOAD_SUCCEEDED,
        };

        let mut loaded = Vec::new();

        for mod_id in &self.load_order {
            if self.disabled_mod_ids.contains(mod_id) {
                continue;
            }
            let manifest = match self.manifests.get(mod_id) {
                Some(m) => m,
                None => continue,
            };

            // Emit load started
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_MOD_LOAD_STARTED,
                byte_len: mod_id.len() + manifest.display_name.len(),
            });

            self.status.insert(mod_id.clone(), ModStatus::Loading);

            // For Phase 2.1, native mods are already "loaded" via inventory
            // In Phase 2.2/2.3, we'll add actual protocol/viewer registration here
            let load_result = self.activate_native_mod(mod_id);

            match load_result {
                Ok(()) => {
                    self.status.insert(mod_id.clone(), ModStatus::Active);
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_MOD_LOAD_SUCCEEDED,
                        byte_len: mod_id.len()
                            + manifest.provides.iter().map(|s| s.len()).sum::<usize>(),
                    });
                    loaded.push(mod_id.clone());
                }
                Err(e) => {
                    self.status.insert(mod_id.clone(), ModStatus::Failed);
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_MOD_LOAD_FAILED,
                        byte_len: mod_id.len() + e.len(),
                    });
                }
            }
        }

        for mod_id in &self.disabled_mod_ids {
            self.status.insert(mod_id.clone(), ModStatus::Unloaded);
        }

        loaded
    }

    /// Activate a native mod by dispatching to its activation function.
    /// Phase 2.2/2.3: Calls the mod's activation hook to register capabilities.
    fn activate_native_mod(&self, mod_id: &str) -> Result<(), String> {
        let activations = super::NativeModActivations::new();
        activations.activate(mod_id)
    }

    /// Get the status of a mod
    pub(crate) fn get_status(&self, mod_id: &str) -> Option<ModStatus> {
        self.status.get(mod_id).copied()
    }

    /// Get the manifest for a mod
    pub(crate) fn get_manifest(&self, mod_id: &str) -> Option<&ModManifest> {
        self.manifests.get(mod_id)
    }

    /// List all mod IDs in load order
    pub(crate) fn list_mods(&self) -> &[String] {
        &self.load_order
    }

    /// Check if a specific capability is provided by any loaded mod
    pub(crate) fn is_capability_available(&self, capability_id: &str) -> bool {
        self.manifests.values().any(|m| {
            if self.disabled_mod_ids.contains(&m.mod_id) {
                return false;
            }
            let mod_active = self
                .status
                .get(&m.mod_id)
                .map_or(false, |s| *s == ModStatus::Active);
            mod_active && m.provides.iter().any(|p| p == capability_id)
        })
    }

    pub(crate) fn active_capability_ids(&self) -> HashSet<String> {
        self.manifests
            .values()
            .filter(|manifest| {
                self.status
                    .get(&manifest.mod_id)
                    .map(|status| *status == ModStatus::Active)
                    .unwrap_or(false)
            })
            .flat_map(|manifest| manifest.provides.iter().cloned())
            .collect()
    }
}

impl Default for ModRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn verso_manifest() -> ModManifest {
    ModManifest::new(
        "mod:verso",
        "Verso",
        ModType::Native,
        vec![
            "protocol:http".to_string(),
            "protocol:https".to_string(),
            "protocol:data".to_string(),
            "viewer:webview".to_string(),
        ],
        vec!["ProtocolRegistry".to_string(), "ViewerRegistry".to_string()],
        vec![ModCapability::Network],
    )
}

fn core_protocol_manifest() -> ModManifest {
    ModManifest::new(
        "mod:core-protocol",
        "Core Protocol Registry",
        ModType::Native,
        vec!["ProtocolRegistry".to_string()],
        vec![],
        vec![],
    )
}

fn core_viewer_manifest() -> ModManifest {
    ModManifest::new(
        "mod:core-viewer",
        "Core Viewer Registry",
        ModType::Native,
        vec!["ViewerRegistry".to_string()],
        vec![],
        vec![],
    )
}

fn core_identity_manifest() -> ModManifest {
    ModManifest::new(
        "mod:core-identity",
        "Core Identity Registry",
        ModType::Native,
        vec!["IdentityRegistry".to_string()],
        vec![],
        vec![],
    )
}

fn core_action_manifest() -> ModManifest {
    ModManifest::new(
        "mod:core-action",
        "Core Action Registry",
        ModType::Native,
        vec!["ActionRegistry".to_string()],
        vec![],
        vec![],
    )
}

fn core_control_panel_manifest() -> ModManifest {
    ModManifest::new(
        "mod:core-control-panel",
        "Core Control Panel",
        ModType::Native,
        vec!["ControlPanel".to_string()],
        vec![],
        vec![],
    )
}

fn core_diagnostics_manifest() -> ModManifest {
    ModManifest::new(
        "mod:core-diagnostics",
        "Core Diagnostics Registry",
        ModType::Native,
        vec!["DiagnosticsRegistry".to_string()],
        vec![],
        vec![],
    )
}

inventory::submit! {
    NativeModRegistration {
        manifest: core_protocol_manifest,
    }
}

inventory::submit! {
    NativeModRegistration {
        manifest: core_viewer_manifest,
    }
}

inventory::submit! {
    NativeModRegistration {
        manifest: core_identity_manifest,
    }
}

inventory::submit! {
    NativeModRegistration {
        manifest: core_action_manifest,
    }
}

inventory::submit! {
    NativeModRegistration {
        manifest: core_control_panel_manifest,
    }
}

inventory::submit! {
    NativeModRegistration {
        manifest: core_diagnostics_manifest,
    }
}

inventory::submit! {
    NativeModRegistration {
        manifest: verso_manifest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest(id: &str, provides: &[&str], requires: &[&str]) -> ModManifest {
        ModManifest::new(
            id,
            id,
            ModType::Native,
            provides.iter().map(|v| v.to_string()).collect(),
            requires.iter().map(|v| v.to_string()).collect(),
            vec![],
        )
    }

    fn test_registry_with_disabled(disabled: &[&str]) -> ModRegistry {
        let disabled_ids = disabled
            .iter()
            .map(|id| (*id).to_string())
            .collect::<HashSet<_>>();
        ModRegistry::new_with_disabled(&disabled_ids)
    }

    #[test]
    fn discovers_native_mods_including_verso() {
        let mods = discover_native_mods();
        assert!(mods.iter().any(|entry| entry.mod_id == "mod:core-protocol"));
        assert!(mods.iter().any(|entry| entry.mod_id == "mod:core-viewer"));
        assert!(mods.iter().any(|entry| entry.mod_id == "mod:verso"));
    }

    #[test]
    fn resolves_dependency_order() {
        let protocol = test_manifest("mod:protocol", &["ProtocolRegistry"], &[]);
        let viewer = test_manifest("mod:viewer", &["ViewerRegistry"], &[]);
        let verso = test_manifest(
            "mod:verso",
            &["viewer:webview"],
            &["ProtocolRegistry", "ViewerRegistry"],
        );

        let ordered = resolve_mod_load_order(&[verso.clone(), viewer.clone(), protocol.clone()])
            .expect("dependency order should resolve");
        let ids = ordered
            .iter()
            .map(|entry| entry.mod_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), 3);
        let protocol_idx = ids.iter().position(|id| *id == "mod:protocol").unwrap();
        let viewer_idx = ids.iter().position(|id| *id == "mod:viewer").unwrap();
        let verso_idx = ids.iter().position(|id| *id == "mod:verso").unwrap();
        assert!(protocol_idx < verso_idx);
        assert!(viewer_idx < verso_idx);
    }

    #[test]
    fn fails_on_missing_requirement() {
        let manifest = test_manifest("mod:x", &["x"], &["ProtocolRegistry"]);
        let error =
            resolve_mod_load_order(&[manifest]).expect_err("should fail missing requirement");
        assert!(matches!(
            error,
            ModDependencyError::MissingRequirement { mod_id, requirement }
                if mod_id == "mod:x" && requirement == "ProtocolRegistry"
        ));
    }

    #[test]
    fn fails_on_dependency_cycle() {
        let a = test_manifest("mod:a", &["A"], &["B"]);
        let b = test_manifest("mod:b", &["B"], &["A"]);
        let error = resolve_mod_load_order(&[a, b]).expect_err("should fail cycle");
        assert!(matches!(error, ModDependencyError::DependencyCycle(_)));
    }

    #[test]
    fn mod_registry_discovers_native_mods() {
        let registry = ModRegistry::new();
        assert!(registry.get_manifest("mod:core-protocol").is_some());
        assert!(registry.get_manifest("mod:core-viewer").is_some());
        assert!(registry.get_manifest("mod:verso").is_some());

        // All should be in Discovered state initially
        assert_eq!(
            registry.get_status("mod:core-protocol"),
            Some(ModStatus::Discovered)
        );
        assert_eq!(
            registry.get_status("mod:core-viewer"),
            Some(ModStatus::Discovered)
        );
        assert_eq!(
            registry.get_status("mod:verso"),
            Some(ModStatus::Discovered)
        );
    }

    #[test]
    fn mod_registry_resolves_dependencies() {
        let mut registry = ModRegistry::new();

        registry
            .resolve_dependencies()
            .expect("should resolve dependencies");

        // Load order should have core mods before verso
        let load_order = registry.list_mods();
        let protocol_idx = load_order.iter().position(|id| id == "mod:core-protocol");
        let viewer_idx = load_order.iter().position(|id| id == "mod:core-viewer");
        let verso_idx = load_order.iter().position(|id| id == "mod:verso");

        assert!(protocol_idx.is_some());
        assert!(viewer_idx.is_some());
        assert!(verso_idx.is_some());

        // Verso should load after its dependencies
        assert!(protocol_idx.unwrap() < verso_idx.unwrap());
        assert!(viewer_idx.unwrap() < verso_idx.unwrap());
    }

    #[test]
    fn mod_registry_loads_mods_in_order() {
        let mut registry = ModRegistry::new();

        registry.resolve_dependencies().expect("should resolve");
        let loaded = registry.load_all();

        // All mods should load successfully
        assert!(loaded.contains(&"mod:core-protocol".to_string()));
        assert!(loaded.contains(&"mod:core-viewer".to_string()));
        assert!(loaded.contains(&"mod:verso".to_string()));

        // Check status transitions to Active
        assert_eq!(
            registry.get_status("mod:core-protocol"),
            Some(ModStatus::Active)
        );
        assert_eq!(registry.get_status("mod:verso"), Some(ModStatus::Active));
    }

    #[test]
    fn mod_registry_checks_capability_availability() {
        let mut registry = ModRegistry::new();

        registry.resolve_dependencies().expect("should resolve");
        registry.load_all();

        // Verso provides these capabilities
        assert!(registry.is_capability_available("protocol:http"));
        assert!(registry.is_capability_available("protocol:https"));
        assert!(registry.is_capability_available("viewer:webview"));

        // Core provides these
        assert!(registry.is_capability_available("ProtocolRegistry"));
        assert!(registry.is_capability_available("ViewerRegistry"));

        // This doesn't exist
        assert!(!registry.is_capability_available("protocol:ipfs"));
    }

    #[test]
    fn mod_registry_without_verso_disables_webview_capability() {
        let mut registry = test_registry_with_disabled(&["mod:verso"]);
        registry
            .resolve_dependencies()
            .expect("dependencies should resolve without verso");
        registry.load_all();

        assert!(!registry.is_capability_available("viewer:webview"));
        assert!(!registry.is_capability_available("protocol:https"));
        assert!(registry.is_capability_available("ProtocolRegistry"));
        assert!(registry.is_capability_available("ViewerRegistry"));
    }
}
