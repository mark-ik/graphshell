# WASM Mod ABI & Sandboxing Research (2026-02-24)

**Status**: Research
**Context**: Investigating the "WASM Mod" tier defined in `registry_layer_plan.md`.
**Goal**: Define how third-party extensions (Pomodoro, Stock Ticker) run safely and interact with the UI.

## 1. Runtime Strategy: Extism

We confirm the selection of **Extism** (wrapping Wasmtime).

*   **Why**: Handles the complex ABI of passing strings/bytes/JSON between Host and Guest. Raw Wasmtime requires manual memory management for these.
*   **Language Support**: Allows mods to be written in Rust, Go, TypeScript, Python, Zig.
*   **Sandboxing**: Built-in WASI support allows granular control over FS and Network access.

## 2. The UI Bridge: The "Widget" Problem

Graphshell uses `egui` (Immediate Mode). WASM cannot directly access host memory or call host closures (`&mut Ui`).

### Option A: The Webview Escape Hatch
We could say "Just write HTML/JS and we'll render it in a WebView tile."
*   **Pros**: Full UI flexibility, standard tooling.
*   **Cons**: Heavy. Spawns a Servo instance. Overkill for a simple "Pomodoro Timer" in a sidebar. Doesn't integrate visually with the graph canvas or egui theme.

### Option B: Pixel Buffer (Framebuffer)
The mod exposes `render(width, height) -> Vec<u8>`.
*   **Pros**: Simple. Good for "Stock Ticker" graphs or visualizers.
*   **Cons**: No native controls (buttons, inputs). Accessibility nightmare. High bandwidth (copying 4MB per frame for 1080p).

### Option C: Serialized UI Protocol (Recommended)
We define a lightweight, data-driven UI schema (`graphshell-mod-api`). The mod returns a "Display List" every frame (or on change).

```rust
// Shared crate: graphshell-mod-api
#[derive(Serialize, Deserialize)]
pub enum Widget {
    // Layout Primitives (Low-level)
    Label { text: String, style: Style },
    Button { id: String, label: String },
    Row { children: Vec<Widget> },
    Col { children: Vec<Widget> },
    Plot { points: Vec<Point2D> }, // For stock ticker

    // Semantic Data (High-level)
    // The Host renders these using standardized templates
    Schema { 
        #[serde(rename = "@type")]
        kind: String, // e.g., "Person", "Event", "Product" (Schema.org)
        data: serde_json::Value 
    },

    // Semantic Actions (Verbs)
    // Renders as a button/control wired to a standard action interface
    ActionTrigger {
        kind: String, // e.g., "PlayAction", "SearchAction", "SaveAction"
    },
}

#[derive(Serialize, Deserialize)]
pub struct FrameOutput {
    pub root: Widget,
    pub intents: Vec<GraphIntent>, // Mods can emit intents during render logic
}
```

*   **Host Side**: Parses `FrameOutput`. Renders `egui` widgets.
*   **Interaction**: When a button is clicked, the Host calls the mod's `on_action(action_id)` function.
*   **Pros**: Native look and feel. Accessible. Efficient (only data transfer). Secure (mod cannot draw over other UI).

## 3. The Capability Contract

`ModManifest` capabilities map to Host Functions and WASI config.

| Capability | Implementation |
| :--- | :--- |
| `network` | WASI: Allow HTTP outbound. Host: Link `host_http_request` function. |
| `filesystem` | WASI: Pre-open specific directories (e.g., mod-local storage). |
| `graph_read` | Host: Link `host_get_node(id)` function. |
| `graph_write` | Host: Link `host_emit_intent(intent)` function. |

## 4. The ABI Lifecycle

The WASM module must export:

1.  `init()`: Setup state.
2.  `update(dt: f64)`: Tick logic (optional).
3.  `render(context_json) -> ui_json`: Return the UI tree.
4.  `on_event(event_json)`: Handle clicks or inputs passed from Host.

## 5. Example: Pomodoro Timer Mod

**Guest Code (Rust):**

```rust
static mut STATE: PomodoroState = ...;

#[plugin_fn]
pub fn render() -> FnResult<Json<FrameOutput>> {
    let time_str = format_time(STATE.remaining);
    
    let ui = Widget::Col { children: vec![
        Widget::Label { text: time_str },
        Widget::Row { children: vec![
            Widget::Button { id: "start", label: "Start" },
            Widget::Button { id: "reset", label: "Reset" }
        ]}
    ]};

    Ok(Json(FrameOutput { root: ui, intents: vec![] }))
}

#[plugin_fn]
pub fn on_action(Json(action): Json<String>) -> FnResult<()> {
    match action.as_str() {
        "start" => STATE.running = true,
        "reset" => STATE.reset(),
        _ => {}
    }
    Ok(())
}
```

## 6. Implementation Roadmap

1.  **Protocol Crate**: Create `graphshell-mod-api` (shared types).
2.  **Host Runtime**: Implement `WasmModHost` in `mods/wasm/`.
3.  **UI Interpreter**: Implement `render_widget_tree(ui: &mut Ui, widget: Widget)` in `desktop/ui/`.
4.  **Demo Mod**: Build the Pomodoro timer.
5.  **SDK**: Create `graphshell-mod-sdk` crate to hide raw ABI details.
6.  **Host Features**: Implement KV store, Config injection, and Timer scheduling in `WasmModHost`.

## 7. Semantic Extensions (Schema.org & App Classes)

To avoid every mod reinventing UI patterns, we support **Semantic Widgets** alongside layout primitives.

### 7.1 Schema.org Integration
Mods can return structured data instead of raw UI. The Host renders a consistent "Card" for that data type.
*   **Use Case**: A "Contact List" mod doesn't build rows/cols; it returns a list of `Widget::Schema { kind: "Person", ... }`.
*   **Benefit**: Graphshell ensures all "Person" cards look identical across all mods.
*   **Interoperability**: This data can be indexed by the `KnowledgeRegistry` or dragged-and-dropped as structured data.

### 7.2 Application Classes (Templates)
For specific classes of applications, we define standard high-level widgets:

| Class | Widget Variant | Renders As |
| :--- | :--- | :--- |
| **Dashboard** | `Metric { label, value, trend }` | Stat card with sparkline |
| **Feed** | `FeedItem { author, content, time }` | Social-media style post |
| **Form** | `Input { id, label, type }` | Styled text input / checkbox |

This creates a hybrid model: **Layout Primitives** for custom tools (Pomodoro), and **Semantic Widgets** for standard content (Feeds, Directories).

## 8. Mod Sophistication & Tooling (The Developer Experience)

To enable sophisticated logic without exposing raw complexity, we layer a **Rust SDK** over the raw ABI and adopt standard patterns.

### 8.1 The "Graphshell Component Model" (WIT)
Instead of loose JSON, we define the ABI using **WIT (Wasm Interface Type)**.
*   **Crate**: `wit-bindgen`.
*   **Benefit**: Generates type-safe bindings for Rust, Go, Python, JS.
*   **Contract**: Defines `Host` functions (capabilities) and `Guest` functions (lifecycle) with strong types.

### 8.2 The Mod Architecture: MVU (Model-View-Update)
We encourage (and SDK-support) the **Elm Architecture** (similar to `iced` or `yew`).
*   **Model**: The mod's internal state struct.
*   **Msg**: Enums representing user actions (`Click`, `Input`).
*   **Update**: `fn update(msg: Msg, model: &mut Model) -> Command`.
*   **View**: `fn view(model: &Model) -> Widget`.
*   **Why**: Fits perfectly with the `render()` / `on_action()` ABI cycle. Keeps mods deterministic and testable.

### 8.3 The SDK (`graphshell-mod-sdk`)
A crate that wraps the boilerplate.
```rust
// User code looks like this:
struct Pomodoro { ... }

impl GraphshellMod for Pomodoro {
    fn update(&mut self, msg: Msg) { ... }
    fn view(&self) -> Widget {
        col![
            label(self.time_remaining()),
            button("Start").on_click(Msg::Start)
        ]
    }
}
```

### 8.4 Tooling
*   **CLI**: `graphshell mod init`, `graphshell mod build`, `graphshell mod test`.
*   **Hot Reloading**: The Host watches the `.wasm` file and reloads it instantly on change (preserving state if possible via serialization).
*   **Inspector**: A "Mod DevTools" pane in Graphshell showing the JSON tree and state size.

### 8.5 Complexity & Limits
*   **Scope**: Mods are "Applets" (Calculators, Kanban, Chat), not full Engines.
*   **Rendering**: Mods cannot issue draw calls directly; they must emit Widgets.
*   **Performance**: Wasm execution is budgeted per frame (e.g., 2ms). Heavy compute must be yielded or moved to a background Agent.

## 9. Semantic Verbs (The "Action Schema")

To complement Schema.org "Nouns" (Data), we adopt **Schema.org Actions** (and ActivityStreams) as the standard vocabulary for "Verbs".

### 9.1 Standard Action Interfaces
Instead of arbitrary command strings, mods implement standard interfaces defined in `ActionRegistry`.
*   **Manifest**: `implements: ["schema:PlayAction", "schema:SearchAction"]`.
*   **Mapping**: The Host maps global hotkeys (Media Play) or UI buttons to these standardized verbs.
*   **UI**: `Widget::ActionTrigger { kind: "PlayAction" }` renders as a standard, theme-aware Play button, automatically wired to the mod's handler.
*   **Interoperability**: A "Voice Control" mod can trigger `PlayAction` on *any* mod that implements it, without knowing the mod's internal details.

## 10. Implementation Strategy: Vocabularies vs. Capabilities

To answer "How do we implement this?" and "Are capabilities sufficient?":

### 10.1 Distinction
*   **Capabilities** (`network`, `fs`): **Security Permissions**. "Can this mod open a socket?" Enforced by the Wasm sandbox (Extism/Wasmtime).
*   **Vocabularies** (`Person`, `PlayAction`): **Semantic Contracts**. "What data is this? What does this button do?" Enforced by the Host's rendering and dispatch logic.
*   **Conclusion**: Capabilities are *not* sufficient. They handle safety, but not interoperability. We need Vocabularies to ensure a "Contact List" mod and a "Email" mod agree on what a "Person" is.

### 10.2 Implementing Nouns (Schema.org)
1.  **Shared Types**: `graphshell-mod-api` defines `Widget::Schema { kind, data }`.
2.  **Host Registry**: `desktop/schema_renderers.rs` maintains a map: `String -> fn(&Value, &mut Ui)`.
3.  **Defaults**: Graphshell ships with renderers for `schema:Person`, `schema:Event`, `schema:Article`.
4.  **Fallback**: Unknown kinds render as a collapsible JSON tree.

### 10.3 Implementing Verbs (Action Interfaces)
1.  **Manifest Declaration**: Mods declare `implements: ["schema:PlayAction"]` in `ModManifest`.
2.  **Action Registry**: The Host's `ActionRegistry` tracks which active mod handles which semantic verb.
3.  **Dispatch**:
    *   **UI Trigger**: `Widget::ActionTrigger { kind: "PlayAction" }` renders a standard Play button. Click -> Host looks up handler -> Calls mod's `on_action("PlayAction")`.
    *   **Global Trigger**: Keyboard media key -> Host finds active mod implementing `PlayAction` -> Calls mod.

### 10.4 Do we need more vocabularies?
Schema.org and ActivityStreams cover 90% of use cases.
*   **Use Schema.org** for generic data (People, Places, CreativeWorks).
*   **Use ActivityStreams** for social/event verbs (Create, Update, Like, Follow).
*   **Define `graphshell:`** only for domain-specific concepts (Node, Edge, Layout, Lens) where standard vocabularies lack precision.

## 11. Prior Art & Feasibility

Has this been done? Yes. This architecture mirrors **Android Intents**, **Home Assistant**, and **VS Code**.

*   **Android**: Apps define `IntentFilters` (Capabilities) and data schemas. The OS (Host) renders standard UI (Share Sheet) and routes actions without knowing what the app does.
*   **Home Assistant**: Integrations expose "Entities" (Light, Switch). The UI (Lovelace) automatically renders the correct controls without the integration writing UI code.
*   **VS Code**: Extensions define commands in `package.json`. VS Code renders the Command Palette and menus data-driven.

## 12. The "Dictionary Problem": Automation Strategy

**Question**: Do we have to manually implement a function for every Schema.org type and action?
**Answer**: No. We use **Generic Routing** and **Progressive Enhancement**.

### 12.1 Data: The Generic Renderer
The Host implements a `DefaultSchemaRenderer`.
*   **Input**: Any JSON object.
*   **Output**: A nice-looking "Property List" card (Key-Value pairs, collapsible).
*   **Result**: *Every* Schema.org type works out of the box.
*   **Enhancement**: We only write custom templates for the "Top 10" (Person, Event, Article).

### 12.2 Actions: The Generic Router
The Host does *not* need a `fn play_action()` or `fn search_action()`.
*   **Mechanism**: The `ActionTrigger` widget holds the *string* ID (e.g., "schema:PlayAction").
*   **Execution**: When clicked, the Host looks up the mod registered for that string and passes the event.
*   **Result**: Mods can invent new actions without Host code changes. The "Dictionary" is data, not code.

## 13. Addressing Gaps: Persistence, Config, and Scheduling

To support stateful apps like Pomodoro timers without granting dangerous raw access, we add managed Host APIs.

### 13.1 Persistence (The KV Store)
Raw filesystem access (`wasi-fs`) is heavy and risky. Most mods just need to save state.
*   **Host API**: `host_store_set(key, value)`, `host_store_get(key)`.
*   **Scope**: Isolated per mod ID.
*   **Backend**: Stored in `redb` or a dedicated `mods.db`.

### 13.2 Configuration Injection
*   **Schema**: Mod provides `config_schema` (JSON Schema) in manifest.
*   **UI**: Host generates Settings UI (using `schemars` logic).
*   **Injection**: Host passes configuration JSON to `init()` and calls `on_config_changed()` when user updates settings.

### 13.3 Scheduling & Wakeup
Polling `update(dt)` every frame is wasteful for WASM.
*   **Mechanism**: Mod calls `host_request_frame()` or `host_schedule_timer(ms)` if it needs to animate or update.
*   **Default**: Mod is quiescent until an event (Input, Action, Timer) occurs.

### 13.4 Asset Bundling
*   **V1 Strategy**: Embed assets (icons, styles) directly in the WASM binary using `rust-embed`.
*   **V2 Strategy**: WASI virtual filesystem mapping a sidecar `.assets` folder.

## 14. Further Gaps: Context, Permissions, and Stability

### 14.1 Context Injection (Theme & Environment)
Mods need to adapt to the host environment to feel native.
*   **Data**: `HostContext` passed to `update()`/`render()`.
*   **Fields**: `theme_mode` (Dark/Light), `accent_color`, `locale`, `reduced_motion`.

### 14.2 Permissions UX (The "Install" Flow)
Capabilities declared in the manifest must be consented to.
*   **Flow**: On `ModRegistry::load_mod()`, if capabilities are requested:
    *   Check `user_registries.json` for existing grant.
    *   If missing, show "Mod Request" dialog: "Mod X wants to access network. Allow?"
    *   Persist decision.

### 14.3 Crash Handling (The "Sad Tile")
WASM provides isolation, so a crash shouldn't kill the app.
*   **Detection**: Extism call returns error/trap.
*   **Reaction**: Host unloads the mod instance.
*   **UI**: The tile renders a "Mod Crashed" error state with a "Restart" button and the error log.
*   **Diagnostics**: Trap info logged to `registry.mod.crash` channel.

### 14.4 Debugging (Log Forwarding)
*   **Stdio**: Map WASM `stdout`/`stderr` to Host `tracing` logs.
*   **Visibility**: Mod logs appear in the Diagnostic Inspector under a specific target (e.g., `mod::pomodoro`).

# WASM Mod Implementation Plan (2026-02-24)

**Status**: Implementation-Ready
**Context**: Follow-up to `research/2026-02-24_wasm_mod_abi_research.md`.
**Goal**: Implement the secure, sandboxed WASM mod system with full lifecycle management, permissions, and debugging support.

## 1. Architecture Overview

The WASM mod system introduces three new components:
1.  **`graphshell-mod-api`**: A shared crate defining the ABI types (`Widget`, `HostContext`, `GraphIntent` subset).
2.  **`mods/wasm/`**: The Host Runtime using `extism`.
3.  **`graphshell-mod-sdk`**: A Rust crate for mod developers.

## 2. Addressing Research Gaps

### 2.1 Context Injection (The Environment)
Mods need to know the app's theme and locale to render native-looking UI.
*   **Mechanism**: `HostContext` struct passed to `update()`/`render()`.
*   **Source**: Derived from `ThemeRegistry` and `AppPreferences`.
*   **Consistency**: Ensures mods respect the global `PresentationDomain`.

### 2.2 Permissions Architecture (The Gatekeeper)
Capabilities must be user-consented.
*   **Store**: `ModRegistry` persists grants in `user_registries.json`.
*   **Flow**: `load_mod` -> check manifest -> if missing, emit `RequestModPermissions` -> UI Dialog -> `GrantModPermissions` -> Retry load.
*   **Consistency**: Uses the standard `GraphIntent` pipeline for state changes.

### 2.3 Stability (The "Sad Tile")
A crash in WASM must not crash the host.
*   **Detection**: `ExtismError` in `WasmModHost`.
*   **Reaction**: Unload mod instance, emit `ModCrashed` intent.
*   **UI**: Tile renders error state with "Restart" button.
*   **Consistency**: Reuses the `TileKind` state machine (Active -> Crashed).

### 2.4 Debugging (The Tracing Bridge)
Mods are black boxes without logs.
*   **Solution**: Bridge WASM stdio to Host `tracing`.
*   **Tagging**: Logs are tagged `target: "mod::{mod_id}"` for easy filtering in the Diagnostic Inspector.
*   **Consistency**: Mod logs appear alongside Servo and App logs in the existing Diagnostic pane.

## 3. Implementation Phases

### Phase 1: The Shared Protocol (`graphshell-mod-api`)
Create a standalone crate that both Host and Guest depend on.
*   **Types**: `Widget` (UI tree), `HostContext` (Env), `ModAction` (Verbs).
*   **Serialization**: `serde` support for all types.

### Phase 2: The Host Runtime (`mods/wasm`)
Implement the `extism` integration.
*   **Lifecycle**: `load()`, `unload()`, `call_update()`, `call_render()`.
*   **Tracing**: Wire `set_log_callback` to `tracing::info!`.
*   **Context**: Construct `HostContext` from `GraphBrowserApp`.

### Phase 3: Lifecycle & Permissions
Wire the mod system into the App Reducer.
*   **Intents**: `RequestModPermissions`, `GrantModPermissions`, `ModCrashed`, `ReloadMod`.
*   **UI**: Implement the Permissions Dialog and "Sad Tile" error state.
*   **Persistence**: Save/load permission grants via `ModRegistry`.

### Phase 4: The SDK & Demo
Make it usable.
*   **SDK**: `graphshell-mod-sdk` wrapping the raw ABI.
*   **Demo**: "Pomodoro Timer" mod proving state, UI, and timer scheduling.
```
