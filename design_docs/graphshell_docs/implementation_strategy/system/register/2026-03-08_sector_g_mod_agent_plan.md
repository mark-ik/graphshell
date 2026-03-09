<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not developed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector G — Mod, Agent & Theme Registry Development Plan

**Doc role:** Implementation plan for the mod, agent, and theme registry sector
**Status:** Active / planning
**Date:** 2026-03-08
**Parent:** [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md)
**Registries covered:** `ModRegistry`, `AgentRegistry`, `ThemeRegistry`
**Specs:** [mod_registry_spec.md](mod_registry_spec.md), [agent_registry_spec.md](agent_registry_spec.md), [theme_registry_spec.md](theme_registry_spec.md)

---

## Purpose

`ModRegistry` is the most complete registry in the system — native mod discovery, dependency
ordering, and lifecycle integration are all functional. Sector G work advances the two missing
registries (`AgentRegistry`, `ThemeRegistry`) and completes `ModRegistry`'s WASM and runtime
surface registration capabilities.

```
ModRegistry     ← complete core; needs WASM + registry surface extension
AgentRegistry   ← not started; supervised background capability declaration
ThemeRegistry   ← not started; visual token sets; required by PresentationDomainRegistry (Sector D)
```

---

## Current State

| Registry | Struct | Completeness | Key gaps |
|---|---|---|---|
| `ModRegistry` | ✅ (atomic) | Most complete | No WASM mod support; mods cannot extend registry surfaces; no unload hot-path |
| `AgentRegistry` | ❌ | Not started | No struct; background capability not declarative |
| `ThemeRegistry` | ❌ | Not started | No struct; visual tokens hardcoded in render path |

---

## Phase G1 — ModRegistry: WASM and registry surface extension

### G1.1 — WASM mod loader

The `mod_registry_spec.md`'s `manifest-gate` policy: mods must declare all capabilities in their
manifest before activation. WASM mods are isolated; they cannot access host memory directly.

```rust
pub trait WasmModHost {
    fn load_wasm(&mut self, path: &Path, manifest: &ModManifest)
        -> Result<WasmModInstance, ModLoadError>;
    fn call_export(&self, instance: &WasmModInstance, name: &str, args: &[WasmVal])
        -> Result<WasmVal, WasmCallError>;
}
```

WASM mods communicate with the host via an `IntentBridge`: they receive serialised events and
return serialised intents. They never hold references to host state.

**Done gates:**
- [ ] `WasmModHost` trait defined.
- [ ] `wasmtime` (or `wasmer`) crate added to `Cargo.toml` behind a `wasm-mods` feature flag.
- [ ] `ModRegistry::load_wasm_mod()` loads a `.wasm` file, validates manifest, activates.
- [ ] `ControlPanel` supervises the WASM host as a worker (WASM calls are async).
- [ ] Unit test: WASM mod emits a `GraphIntent::Noop` via intent bridge.

### G1.2 — Mod-provided registry surface extensions

The `mod_registry_spec.md`'s `containment` policy: mods extend registry surfaces through
declared extension points, not by replacing registry internals.

Mods can register:
- Protocol scheme handlers (`ProtocolRegistry::register_scheme_handler()` — Sector A A1.3).
- Viewer implementations (`ViewerRegistry::register_viewer()`).
- Search providers (`IndexRegistry::register_provider()` — Sector F F3.1).
- Action handlers (`ActionRegistry::register_action()` — Sector B B3.x).
- Lens descriptors (`LensRegistry::register_lens()` — Sector A A4.1).
- Canvas profiles (`CanvasRegistry::register_profile()` — Sector D D3.1).
- Workflow descriptors (`WorkflowRegistry::register_workflow()` — Sector E E2.1).

The `ModRegistry` tracks which extensions each mod has registered; `unload_mod()` removes them.

**Done gates:**
- [ ] `ModExtensionRecord` tracks all registry extensions made by a mod.
- [ ] `unload_mod()` calls the appropriate `unregister_*` on each extension type.
- [ ] Test: load mod → extensions present; unload mod → extensions removed; fallback restored.

### G1.3 — Unload hot-path

Currently `ModRegistry` supports load-all at startup. Unloading a single mod at runtime is
needed for mod developer iteration.

```rust
pub fn unload_mod(&mut self, id: &ModId) -> Result<(), ModUnloadError>
```

Unload:
1. Calls `unload()` on the mod instance (if defined in the manifest).
2. Removes all `ModExtensionRecord` entries.
3. Emits `GraphIntent::ModDeactivated { mod_id }`.
4. Emits `SignalKind::Lifecycle(ModUnloaded)` via `SignalRoutingLayer`.

**Done gates:**
- [ ] `unload_mod()` implemented with full extension cleanup.
- [ ] `GraphIntent::ModDeactivated` defined and handled in reducer.
- [ ] Test: unload a mod that registered a scheme handler → scheme handler removed.

---

## Phase G2 — ThemeRegistry: Visual token sets

**Unlocks:** `PresentationDomainRegistry` (Sector D); WCAG-compliant colour resolution;
dark mode; mod-provided themes.

The `theme_registry_spec.md` adopts OSGi R8, OpenTelemetry Semantic Conventions, and
WCAG 2.2 Level AA. Theme tokens govern all visual output.

### G2.1 — Define `ThemeTokenSet` and `ThemeRegistry`

```rust
pub struct ThemeTokenSet {
    // Colours (all WCAG AA contrast-compliant)
    pub surface_background: Color,
    pub surface_foreground: Color,
    pub node_default_fill: Color,
    pub node_selected_fill: Color,
    pub node_hot_fill: Color,
    pub node_cold_fill: Color,
    pub edge_default_stroke: Color,
    pub edge_selected_stroke: Color,
    pub label_primary: Color,
    pub label_secondary: Color,
    pub focus_ring: Color,
    pub selection_highlight: Color,

    // Typography
    pub label_font_family: String,
    pub label_font_size_default: f32,
    pub label_font_size_small: f32,

    // Motion
    pub transition_duration_ms: u32,
    pub animation_easing: EasingCurve,
}

pub const THEME_ID_DARK: ThemeId = ThemeId("theme:dark");
pub const THEME_ID_LIGHT: ThemeId = ThemeId("theme:light");
pub const THEME_ID_HIGH_CONTRAST: ThemeId = ThemeId("theme:high_contrast");

pub struct ThemeRegistry {
    themes: HashMap<ThemeId, ThemeTokenSet>,
    active: ThemeId,
}
```

**Done gates:**
- [ ] `ThemeRegistry` struct in `shell/desktop/runtime/registries/theme.rs`.
- [ ] `DARK`, `LIGHT`, `HIGH_CONTRAST` built-in themes registered.
- [ ] `HIGH_CONTRAST` theme verified WCAG AA (7:1 contrast ratio minimum).
- [ ] Added to `RegistryRuntime`.
- [ ] `DIAG_THEME` channel (Info severity).

### G2.2 — Replace hardcoded colour constants in `render/panels.rs`

All `egui::Color32::*` hardcoded values in `render/panels.rs` and `render/mod.rs` are
replaced with calls to the active theme token set:

```rust
let tokens = registries.theme.active_tokens();
let node_fill = tokens.node_default_fill;
```

**Done gates:**
- [ ] No hardcoded `Color32::from_rgb(...)` literals remain in `render/`.
- [ ] `PresentationDomainRegistry::resolve_presentation_profile()` reads from `ThemeRegistry`.
- [ ] Visual regression check: dark theme produces identical colours to before.

### G2.3 — System theme detection

Detect OS light/dark preference at startup and set the active theme accordingly.

```rust
pub fn detect_system_theme() -> ThemeId {
    // use winit / platform API to detect dark/light
}
```

User preference (manual override) takes precedence over system detection. Override persists
to user preferences via `GraphIntent::SetTheme`.

**Done gates:**
- [ ] `detect_system_theme()` implemented for Windows (primary target; others as best-effort).
- [ ] `GraphIntent::SetTheme { theme_id }` defined and handled in reducer.
- [ ] Theme persists across restart.

### G2.4 — Mod-provided themes

Via `ModRegistry` extension mechanism (Phase G1.2), mods can register additional theme token
sets. Custom themes are validated for WCAG AA compliance on registration.

**Done gates:**
- [ ] `ThemeRegistry::register_theme()` validates WCAG AA contrast ratios.
- [ ] Non-compliant theme registration emits `DIAG_THEME` at `Warn` severity.
- [ ] Test: register custom theme; activate it; render uses custom tokens.

---

## Phase G3 — AgentRegistry: Supervised background capability

**Unlocks:** Declarative background agents; intelligence features (PLANNING_REGISTER §1D Sector E:
verse-intelligence, intelligence-memory); future AI-augmented graph features.

The `agent_registry_spec.md`'s `supervised-agent` policy: agents are explicit background
capabilities, not hidden threads. Agent work enters app state only through supervised intents.

The key distinction from `ControlPanel` workers: workers are platform-level infrastructure
(memory monitor, prefetch, sync). Agents are application-level capabilities declared by mods
or the built-in intelligence layer. An agent is closer to a named background task with a
declared capability surface.

### G3.1 — Define `AgentDescriptor` and `AgentRegistry`

```rust
pub trait Agent: Send {
    fn id(&self) -> AgentId;
    fn display_name(&self) -> &str;
    fn declared_capabilities(&self) -> Vec<AgentCapability>;

    /// Called to start the agent. Returns an async task handle.
    fn spawn(self: Box<Self>, context: AgentContext) -> AgentHandle;
}

pub struct AgentContext {
    pub intent_tx: mpsc::Sender<QueuedIntent>,
    pub signal_rx: broadcast::Receiver<SignalEnvelope>,
    pub cancel: CancellationToken,
}

pub struct AgentDescriptor {
    pub id: AgentId,
    pub display_name: String,
    pub capabilities: Vec<AgentCapability>,
    pub schedule: AgentSchedule,   // OnDemand | Periodic(Duration) | Triggered(SignalKind)
}

pub struct AgentRegistry {
    descriptors: HashMap<AgentId, AgentDescriptor>,
    handles: HashMap<AgentId, AgentHandle>,
}
```

Built-in agents (stubs for now):
- `agent:graph_summariser` — periodically generates summaries for recently visited nodes.
- `agent:tag_suggester` — suggests UDC tags for untagged nodes using heuristics.

**Done gates:**
- [ ] `AgentRegistry` struct in `shell/desktop/runtime/registries/agent.rs`.
- [ ] `Agent` trait and `AgentContext` defined.
- [ ] `GRAPH_SUMMARISER` and `TAG_SUGGESTER` stub agents registered.
- [ ] Added to `RegistryRuntime`.
- [ ] `DIAG_AGENT` channel (Info severity).

### G3.2 — Agent supervision under `ControlPanel`

The `boundary-ingress` policy: agents communicate with the app state through the intent queue,
not direct state access. `ControlPanel` supervises agent tasks as first-class workers.

```rust
impl ControlPanel {
    pub fn spawn_agent(&mut self, agent: Box<dyn Agent>, registries: &RegistryRuntime) {
        let context = AgentContext {
            intent_tx: self.intent_tx.clone(),
            signal_rx: registries.signal_routing.subscribe_all(),
            cancel: self.cancel.clone(),
        };
        let handle = agent.spawn(context);
        self.workers.spawn(handle.task);
    }
}
```

**Done gates:**
- [ ] `ControlPanel::spawn_agent()` implemented.
- [ ] Agent tasks are included in `shutdown()` JoinSet drain.
- [ ] Test: stub agent spawns, receives cancel, shuts down cleanly.

### G3.3 — `tag_suggester` agent (minimal implementation)

The `TAG_SUGGESTER` agent provides a concrete implementation of the agent model:

1. Subscribe to `SignalKind::Navigation(NodeActivated)` signals.
2. For each newly activated node, analyse the URL and title.
3. Query `KnowledgeRegistry::validate_tag()` for candidate tags.
4. Emit `GraphIntent::SuggestNodeTags { node_key, suggestions }` (new intent, non-destructive).

`SuggestNodeTags` is a display-only intent: it adds suggestions to the node's UI without
committing them to the graph model. The user must explicitly confirm to create the tags.

**Done gates:**
- [ ] `GraphIntent::SuggestNodeTags` defined and handled (display-side only).
- [ ] `TagSuggesterAgent` implementation in `app/agents/tag_suggester.rs`.
- [ ] Agent subscribes to navigation signal and emits suggestion intents.
- [ ] Test: agent suggests UDC tags for a URL with a known-category hostname.

---

## Acceptance Criteria (Sector G complete)

- [ ] WASM mod loader functional with intent bridge; isolated from host state.
- [ ] Mods can register and unregister scheme handlers, viewers, search providers, and lenses.
- [ ] `ThemeRegistry` owns all visual tokens; no hardcoded colours in `render/`.
- [ ] Dark/light/high-contrast themes built in; system theme detection works.
- [ ] Custom mod themes validated for WCAG AA on registration.
- [ ] `AgentRegistry` is a real registry; agents supervised by `ControlPanel`.
- [ ] `TAG_SUGGESTER` agent provides end-to-end agent model example.
- [ ] All three registries are in `RegistryRuntime` with diagnostics coverage.

---

## Related Documents

- [mod_registry_spec.md](mod_registry_spec.md)
- [agent_registry_spec.md](agent_registry_spec.md)
- [theme_registry_spec.md](theme_registry_spec.md)
- [2026-03-08_sector_d_canvas_surface_plan.md](2026-03-08_sector_d_canvas_surface_plan.md) — ThemeRegistry consumer
- [2026-03-08_sector_h_signal_infrastructure_plan.md](2026-03-08_sector_h_signal_infrastructure_plan.md) — Agent signal subscription
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — ControlPanel worker supervision
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
