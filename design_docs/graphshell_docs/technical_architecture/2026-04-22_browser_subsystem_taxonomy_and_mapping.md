<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Browser Subsystem Taxonomy and Graphshell Mapping

**Date**: 2026-04-22
**Status**: Synthesis doc. Pulls grounding from the four adjacent technical-architecture specs and enumerates where each canonical browser subsystem lives in Graphshell today. Gap-analysis surface for "what do we not yet have, and what did we deliberately skip?"
**Audience**: Contributors familiar with Chromium / Gecko / WebKit who want a one-page "where does X live here?" map. Secondary: design-review readers who want to know what Graphshell *isn't* as a browser.

**Related docs**:

- [`GRAPHSHELL_AS_BROWSER.md`](GRAPHSHELL_AS_BROWSER.md) — user-visible browser model and high-level guarantees
- [`ARCHITECTURAL_OVERVIEW.md`](ARCHITECTURAL_OVERVIEW.md) — internal subsystem status map and dependency topology
- [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md) — portable-core / host-envelope split
- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md) — MiddleNet engine and content-tier model
- [`2026-04-16_middlenet_lane_architecture_spec.md`](2026-04-16_middlenet_lane_architecture_spec.md) — Direct Lane / HTML Lane / Servo Lane crate split
- [`../../verso_docs/technical_architecture/VERSO_AS_PEER.md`](../../verso_docs/technical_architecture/VERSO_AS_PEER.md) — routing authority and web-runtime placement
- [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) — canonical term definitions

---

## 1. Purpose and Non-Goals

### 1.1 Purpose

Chromium, Firefox, and WebKit each organize around roughly the same set of subsystems — parsers, layout, compositor, network stack, JS engine, a11y tree, devtools, extension host, and so on. A contributor walking into Graphshell expecting that topology finds it — but the pieces have different names, different authority boundaries, and different degrees of completion. Some are delegated to Servo. Some are owned by our portable MiddleNet engine. Some are consciously out of scope. Some are gaps we haven't filled yet.

This doc exists so that:

- a contributor familiar with browser engineering can locate any subsystem in our tree,
- a design-review reader can see what we have, what we don't, and why,
- a product-scoping conversation can start from a shared map instead of rediscovering boundaries.

Since it's a **synthesis**, every row cites the canonical spec that owns the detail, and this doc should not be taken as the authority on any of the subsystems themselves.

### 1.2 Non-Goals

- **Not a comparative benchmark.** No "we're X% of Chromium." Maturity notes are qualitative.
- **Not a roadmap.** Planned-but-unlanded work is flagged, not scheduled.
- **Not authoritative on any subsystem's contract.** Each row points to the spec; this doc is lossy by design.
- **Not a critique of Chromium/Gecko/WebKit.** The taxonomy borrows from them because they're the shared vocabulary, not because they're the standard to beat.

---

## 2. Organizational Axes

Traditional browser anatomy groups subsystems along roughly these axes. I'll use this grouping for the mapping in §3; skim this section if you already know what a browser is made of.

1. **Content pipeline** — given bytes over the wire, turn them into pixels. Parsing, style, layout, paint, compositing, JS runtime, Web APIs, media.
2. **Networking** — fetch those bytes. HTTP cache, TLS, cookies, CORS/CSP, DNS, WebSocket, WebRTC, service workers, protocol handlers.
3. **Process and isolation** — who gets what address space, what does the OS sandbox allow, how do processes talk.
4. **Storage** — everything that outlives a page: HTTP cache, cookies, localStorage, IndexedDB, file system, user profile.
5. **Navigation and history** — back/forward, bookmarks, autocomplete, session restore, browse history.
6. **Chrome (the UI)** — tabs, omnibox, bookmarks bar, downloads manager, find-in-page, context menus, keyboard shortcuts, full-screen, PIP.
7. **Input and accessibility** — keyboard/pointer/touch/gamepad routing, IME, focus, accessibility tree, screen readers.
8. **Developer tools** — elements inspector, JS console, network panel, performance profiler, accessibility inspector.
9. **Extensions** — WebExtension API, content scripts, background workers, native messaging.
10. **Security and privacy** — sandbox, same-origin, SafeBrowsing, permission prompts, password manager, autofill, incognito.
11. **Telemetry and diagnostics** — crash reporter, perf metrics, about:pages.
12. **Distribution** — installer, auto-update, profile migration.
13. **Sync** — account-tied cloud sync of bookmarks/history/passwords/tabs.

Graphshell adds axes that standard browsers don't have, covered in §4:

- **Graph truth** — nodes/edges as the durable content identity layer; URLs are a property, not an identity.
- **Workbench** — tile tree / pane arrangement / graphlet aggregation as a first-class layer above content.
- **Navigator** — cross-domain projection system (graph truth → navigable local worlds).
- **Registries** — host-neutral capability dispatch (Action, Viewer, Channel, Input, Physics, Protocol, Mod).
- **Semantic layer** — UxTree, semantic projections, distillery (speculative).

---

## 3. Subsystem-by-Subsystem Mapping

Each row has:

- **Subsystem** — browser-engineering name.
- **Graphshell home** — crate/module or external dependency.
- **Status** — ✅ Done / 🔨 Active / 📋 Planned / 🔭 Speculative / ⛔ Not-in-scope-by-design / ❓ Gap (no explicit decision yet).
- **Canonical spec** — where the contract lives. `—` if no graphshell spec exists.

Status labels follow [`ARCHITECTURAL_OVERVIEW.md`](ARCHITECTURAL_OVERVIEW.md) §3.

### 3.1 Content Pipeline

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| HTML parser | Servo's `html5ever` via `web_runtime` provider bundle; MiddleNet has its own `SemanticDocument` parser for non-HTML docs | ✅ | `../../verso_docs/technical_architecture/VERSO_AS_PEER.md` + middlenet adapters |
| CSS parser + style system | Servo (full) via `web_runtime:servo`; HTML Lane uses Blitz's style stack (planned) per lane spec | ✅ / 📋 | [`2026-04-16_middlenet_lane_architecture_spec.md`](2026-04-16_middlenet_lane_architecture_spec.md) |
| Layout engine | Servo (full web) / Blitz (HTML Lane, planned) / Direct Lane has its own document layout for Gemini/RSS/Markdown/plain text | ✅ / 📋 | `crates/middlenet-render/` + Servo |
| Paint / rasterization | Servo's WebRender (fork: `webrender-wgpu`, currently mid-SPIR-V/naga migration) / Direct Lane paints directly via Graphshell's WebRender fork | 🔨 | [`../research/2026-03-01_webrender_wgpu_renderer_research.md`](../research/2026-03-01_webrender_wgpu_renderer_research.md) |
| Compositing (surface composition, layer tree) | `shell/desktop/workbench/tile_compositor.rs` + `compositor_adapter.rs` — Graphshell's own three-pass compositor (UI chrome → content → overlay affordance) | ✅ | `../implementation_strategy/aspect_render/frame_assembly_and_compositor_spec.md` |
| JavaScript engine | SpiderMonkey via Servo's `servo-script` stack (full web only); Boa integration planned for portable core | ✅ / 📋 | `../implementation_strategy/2026-03-11_boa_scripting_engine_plan.md` |
| DOM / Web APIs | Servo's full DOM on the Servo Lane; portable core exposes only the subset MiddleNet content needs | ✅ / ⛔ | [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md) §1 |
| Image decoding | `image` 0.25 crate (PNG/JPEG/WebP/AVIF/GIF/BMP/ICO enabled) for thumbnails + favicons; Servo for in-page images | ✅ | `Cargo.toml:108` |
| Media (video/audio/WebRTC) | Inherited from Servo (full-web lane); Graphshell has no separate media pipeline | ✅ / ⛔ | Servo upstream |
| WebGL / WebGPU | Inherited from Servo; Graphshell's own render stack is wgpu-based but not exposed to content | ✅ | Servo upstream |
| Canvas 2D | Servo upstream for content; `graph-canvas` crate is Graphshell's **graph** canvas (force-directed layout), not an HTML canvas | ✅ / ⛔ | [`crates/graph-canvas/`](../../../crates/graph-canvas/) |
| Forms + input handling | Servo for full web; portable `web_runtime` is draft-capture-enabled via `GRAPHSHELL_ENABLE_FORM_DRAFT` env flag | ✅ / 🔨 | `../implementation_strategy/aspect_input/input_interaction_spec.md` |
| iframes / cross-origin frames | Servo provides; Graphshell's own `PaneId` composition is a different concern (workbench panes, not DOM frames) | ✅ | Servo upstream |

### 3.2 Networking

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| HTTP/HTTPS fetch | Servo's fetch stack (full web); MiddleNet adapters use `reqwest` / `ureq` directly for simple docs | ✅ | VERSO_AS_PEER.md |
| TLS / cert validation | Delegated to Servo (web) / `rustls` via adapter crates (MiddleNet) | ✅ | — |
| HTTP cache | Servo's cache (web); MiddleNet has its own cache policy via `RuntimeCaches` | ✅ / 🔨 | `shell/desktop/runtime/caches/` |
| Cookies | Servo's cookie jar (web); non-web MiddleNet protocols are cookieless by design | ✅ / ⛔ | Servo upstream |
| CORS / CSP / mixed-content | Servo enforces for web content; not applicable to MiddleNet protocols (Gemini/Gopher/Finger have no origin model) | ✅ / ⛔ | Servo upstream |
| WebSocket | Servo upstream | ✅ | — |
| WebRTC | Servo's state of support; not used by Graphshell-owned code | ✅ / ⛔ | — |
| WebTransport / HTTP/3 | Servo's state of support | ✅ / ❓ | — |
| Service Workers | Servo's state of support; not used by Graphshell | ✅ / ⛔ | — |
| DNS / proxy | OS defaults via Servo/reqwest | ✅ | — |
| Protocol handlers | `ProtocolRegistry` (`namespace:name` keyed). `http`, `https`, `file` live today; `gemini`, `gopher`, `finger`, `nex`, `spartan`, `scroll` land per MiddleNet adapter status; `verso://` is the internal namespace | ✅ / 🔨 | `../implementation_strategy/system/2026-03-03_graphshell_address_scheme_implementation_plan.md` |
| `verso://` routing | `crates/verso` owns routing authority (`select_viewer_for_content`, `resolve_route_for_content`, `VersoResolvedRoute`) | ✅ | VERSO_AS_PEER.md |

### 3.3 Process and Isolation

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Multi-process architecture | Single-process today. Servo runs as an in-process library. | ✅ / ❓ | — |
| Site isolation | Not implemented. Servo's `WebViewId`/`PipelineNamespace` provides some per-view isolation. | ❓ | — |
| OS sandbox | Not yet wired. Deliberately deferred until the portable-core extraction settles. | ❓ | — |
| Renderer/browser/GPU process split | Chromium's model; not adopted. `CompositorAdapter` isolates GL state in-process. | ⛔ | `../implementation_strategy/aspect_render/frame_assembly_and_compositor_spec.md` |
| IPC | No process boundary yet → no IPC. Frame-inbox + mpsc channels stand in for cross-thread delivery. | ⛔ for now | `shell/desktop/ui/gui/frame_inbox.rs` |

### 3.4 Storage

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| User profile directory | Workspace-level directory per-install; `persistence_facade.rs` orchestrates reads/writes | ✅ | `../implementation_strategy/subsystem_storage/SUBSYSTEM_STORAGE.md` |
| WAL (write-ahead log) | Graphshell-owned; `fjall` for log segments + `redb` for snapshots + `rkyv` for serialization. Single-write-path invariant. | ✅ | SUBSYSTEM_STORAGE.md |
| Graph snapshot | `take_snapshot` via `fjall`/`redb`; wired to `EguiHost::drop` for shutdown persistence | ✅ | `app/persistence_facade.rs` |
| HTTP cache | See §3.2 | — | — |
| Cookie jar | Servo-owned (web); MiddleNet protocols are cookieless | ✅ / ⛔ | — |
| localStorage / sessionStorage | Servo-owned for web content | ✅ | — |
| IndexedDB | Servo-owned for web content | ✅ | — |
| File system access (Web) | Servo-owned (web standard) | ✅ | — |
| Runtime caches (thumbnails, favicons, parsed metadata) | `RuntimeCaches` in `shell/desktop/runtime/caches/`. LRU policy, size caps. | ✅ | — |
| Encryption at rest | AES-256-GCM planned per `SUBSYSTEM_STORAGE.md` | 📋 | SUBSYSTEM_STORAGE.md |
| Graph persistence layout | Per-view GraphTree serialization keyed by `GraphViewId`; workbench manifest is a separate layer | ✅ | `app/settings_persistence.rs` |

### 3.5 Navigation and History

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Back/forward list | Traversal edges in the graph are the canonical back/forward representation. Per-node navigation history is preserved alongside. | ✅ | `../implementation_strategy/subsystem_history/edge_traversal_spec.md` |
| Global browse history | `History Manager` pane — full timeline, filtered views | ✅ | `../implementation_strategy/subsystem_history/SUBSYSTEM_HISTORY.md` |
| Session restore | `SessionWorkspace` snapshot + workbench-manifest restore at startup | ✅ | `app/startup_persistence.rs` |
| Bookmarks | Node creation is bookmarking. Import adapters exist for Firefox/Chrome bookmarks HTML and JSON. Not a separate bookmark manager. | ✅ | `../implementation_strategy/subsystem_history/2026-04-11_browser_import_normalized_carrier_sketch.md` |
| Autocomplete / suggestions | `OmnibarSearchSession` in `shell/desktop/ui/omnibar_state.rs`. Omnibar aggregates local graph + external search providers. | ✅ / 🔨 | `../implementation_strategy/aspect_command/command_surface_interaction_spec.md` |
| Temporal preview / replay | Spec exists; runtime pending | 📋 | `../implementation_strategy/subsystem_history/edge_traversal_spec.md` |
| New tab page | No persistent "new tab" in the Chromium sense — opening content creates or activates a graph node | ⛔ | GRAPHSHELL_AS_BROWSER.md §3 |
| Most-visited / top sites | Derivable from graph truth; not exposed as a distinct surface today | ❓ | — |

### 3.6 Chrome (UI)

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Tab bar / window manager | `graph-tree` crate + `shell/desktop/workbench/`. Tile Tree with Tab Group / Split / Grid containers. Tabs are a UI affordance over Tiles. | ✅ | `../implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md` + TERMINOLOGY.md |
| URL bar / omnibox | `shell/desktop/ui/toolbar/` + `omnibar_state.rs`. Per-pane drafts, scope-aware. | ✅ | `../implementation_strategy/aspect_command/command_surface_interaction_spec.md` |
| Bookmarks bar | No dedicated bar. Graph canvas + Navigator sidebar + History Manager cover the use cases. | ⛔ by design | — |
| Downloads manager | Delegated to Servo/OS for web downloads; no first-class Graphshell download UI yet. | ❓ | — |
| Find-in-page | ⛔ Not yet; would be a viewer-level concern. Graph Search (Ctrl+G) covers graph-level search. | ❓ | — |
| Print | ⛔ Not yet. | ❓ | — |
| Full-screen | Workbench layer handles pane full-screen within the workbench (`WorkbenchLayerState::WorkbenchOnly`/`Overlay`); OS-level full-screen is winit's concern. | ✅ | `shell/desktop/ui/workbench_host.rs` |
| Picture-in-picture | Servo-inherited if at all; Graphshell has no PIP UI. | ⛔ | — |
| Context menus | Context palette + Radial palette + Command palette are the three command surfaces. Right-click routes per `ContextCommandSurfacePreference`. | ✅ | command_surface_interaction_spec.md |
| Keyboard shortcuts | `InputRegistry` with chord/sequence support. User-remappable via `settings_persistence`. | ✅ | `../implementation_strategy/aspect_input/input_interaction_spec.md` |
| Toast notifications | `egui_notify::Toasts` on the egui host; iced will have its own. Surface is `HostToastPort`. | ✅ | `shell/desktop/ui/host_ports.rs` |
| Dialog / modal | Command/Context/Radial palettes, Help panel, Settings overlay, Bookmark Import, Clip Inspector, Scene overlay. All runtime-owned session state, host-rendered. | ✅ / 🔨 | — |
| Tab groups | Graphlet Anchors + Tile Groups are the structural equivalent; far richer than Chrome's tab groups. | ✅ | TERMINOLOGY.md §Tile Tree Architecture |

### 3.7 Input and Accessibility

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Keyboard input | Winit → egui/iced → `InputRegistry` → `ActionRegistry`. Chord/sequence/modal capture. | ✅ | `../implementation_strategy/aspect_input/input_interaction_spec.md` |
| Pointer input | Winit → host → canvas/pane routing via `CanvasNavigationPolicy` + `focus_authority` | ✅ | `../implementation_strategy/aspect_input/input_interaction_spec.md` |
| Touch input | Inherited from egui's touch handling; not a first-class target yet | ❓ | — |
| Gamepad | `gilrs`-backed with `AppGamepadProvider`; feature-flagged | ✅ | — |
| IME / text input | Delegated to egui (text input widget state). iced will bring its own. | ✅ | — |
| Focus management | Six-track `RuntimeFocusAuthorityState`: SemanticRegion / PaneActivation / GraphView / LocalWidget / EmbeddedContent / ReturnCapture. F6 region cycle live. | 🔨 | `../implementation_strategy/subsystem_focus/2026-03-08_unified_focus_architecture_plan.md` |
| Accessibility tree | AccessKit via `egui-winit` bridge (currently version-mismatch degraded). WebView a11y forwarding active. | 🔨 | `../implementation_strategy/subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` |
| Graph Reader (virtual a11y tree) | Planned: projects graph state into an accessibility-legible form so screen readers can navigate nodes/edges, not just the visual surface | 📋 | SUBSYSTEM_ACCESSIBILITY.md |
| Screen-reader integration | Via AccessKit on desktop. WCAG 2.2 AA normative target. | 🔨 | SUBSYSTEM_ACCESSIBILITY.md |

### 3.8 Developer Tools

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Element inspector | ⛔ No equivalent yet. DOM Inspector is planned as a Wry-based non-web viewer type. | 📋 | `../implementation_strategy/viewer/universal_content_model_spec.md` |
| JS console / debugger | ⛔ Not yet. Servo's own devtools protocol could be surfaced; no decision. | ❓ | — |
| Network panel | ⛔ Not yet. | ❓ | — |
| Performance profiler | Diagnostics subsystem (`ChannelRegistry` + ring buffer) covers Graphshell-internal events. No page-level performance tools. | 🔨 / ⛔ | `../implementation_strategy/subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` |
| Accessibility inspector | ⛔ Not yet. | ❓ | — |
| Diagnostics Inspector pane | Graphshell-specific: live channel feed, per-channel severity, diagnostic events. Not "page devtools" but the adjacent concern. | 🔨 | SUBSYSTEM_DIAGNOSTICS.md |
| UX probes / scenarios | `UxProbeSet` + `UxScenario` runner. Semantic test affordance. Partial active, WebDriver bridge planned. | 📋 | `../implementation_strategy/subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md` |

### 3.9 Extensions

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| WebExtension API (Manifest V2/V3) | ⛔ Not implemented. Not scope-compatible with the mod model. | ⛔ by design | — |
| Content scripts | ⛔ — | ⛔ | — |
| Background workers | ⛔ — | ⛔ | — |
| Native messaging | ⛔ — | ⛔ | — |
| **Mod model** (Graphshell-native, not a browser equivalent) | `ModRegistry` with `inventory::submit!` registration. Protocol handlers, agents, viewers register as mods. Native-compiled, not sandboxed. | ✅ | `../implementation_strategy/subsystem_mods/` (when populated) |
| WASM mod sandbox | Host-envelope-dependent. WASI Preview 2 is the portable runtime target per [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md) §1. | 📋 | portable_web_core doc |

The deliberate position is: **Graphshell uses a mod system rather than a browser-extension API.** WebExtensions assume a DOM-centered programming surface; Graphshell's programming surface is the registries + graph truth. A WebExtension shim could be a compatibility envelope later, but is not the primary extensibility story.

### 3.10 Security and Privacy

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Same-origin / CORS | Servo-enforced for web content | ✅ | — |
| Sandbox | OS sandbox: ❓ (deferred). In-process isolation: `CompositorAdapter` for GL state; per-view `PipelineNamespace` for Servo. | ❓ / ✅ | — |
| Permission prompts (camera, mic, location, notifications) | Delegated to Servo for web content; no Graphshell-native permission UI. | ✅ / ❓ | — |
| SafeBrowsing | ⛔ Not implemented. | ⛔ | — |
| Password manager / autofill | ⛔ Not implemented. Delegated to OS/host where available; no Graphshell-native password vault. | ⛔ by design (today) | — |
| Anti-phishing | ⛔ — | ⛔ | — |
| Private browsing / incognito | ⛔ Not implemented. Temporal preview mode (`history_preview_mode_active`) is adjacent but differently scoped. | ❓ | SUBSYSTEM_HISTORY.md |
| Signing boundary (Nostr) | `nip07_bridge` in `shell/desktop/runtime/runtime/`. Explicit signing boundary for Nostr event authorship. | ✅ | `../../nostr_docs/` |

### 3.11 Telemetry and Diagnostics

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Crash reporter | ⛔ Not implemented. `backtrace.rs` captures stack traces locally. | ❓ | `backtrace.rs` |
| Perf metrics / histograms | `DiagnosticsState` ring buffer; `record_span_duration` hooks in hot paths. Not wired to external telemetry sinks. | 🔨 | SUBSYSTEM_DIAGNOSTICS.md |
| about:memory / about:support / about:* | No `about:` namespace. `verso://` is the internal URL scheme; specific surfaces (diagnostics, settings, memory) are tool panes. | ✅ (tool panes) | `../implementation_strategy/system/2026-03-03_graphshell_address_scheme_implementation_plan.md` |
| Anonymized usage stats | ⛔ Local-first philosophy. No outbound telemetry. | ⛔ by design | `../../PROJECT_DESCRIPTION.md` |
| Diagnostics Inspector | Live pane. Channel severity (Error/Warn/Info). | 🔨 | SUBSYSTEM_DIAGNOSTICS.md |

### 3.12 Distribution

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Installer / packaging | Platform packages envisioned per [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md) §3: `graphshell-windows` / `graphshell-macos` / `graphshell-linux` / `graphshell-ios` / `graphshell-android` / `graphshell-firefox` / `graphshell-chrome`. Today only native desktop is packaged. | 📋 | portable_web_core doc |
| Auto-update | ⛔ Not implemented. | ❓ | — |
| Profile migration | Workspace-layout JSON is forward-compatible via `#[serde(default)]`. No explicit migration registry. | ✅ | — |

### 3.13 Sync

| Subsystem | Graphshell home | Status | Canonical spec |
|---|---|---|---|
| Account-tied cloud sync | ⛔ No central account model. | ⛔ by design | — |
| P2P sync | `verso_docs/` bilateral sync (iroh transport, pairing, session capsule ledger). Tier 1. | 🔨 | `../../verso_docs/technical_architecture/` |
| Federated sync (Verse) | Long-horizon research. Public community layer, federated search, FLora, Proof of Access. Tier 2. | 🔭 | `../../verse_docs/` |
| Nostr integration | `mods/native/nostr_*`; relay worker + DVM support. | 🔨 | `../../nostr_docs/` |
| Matrix integration | Room protocol + room projection. | 📋 | `../../matrix_docs/` |

The sync story is deliberately the opposite of Chrome's "sign in with Google and everything just works" — sync is either strictly peer-to-peer (Verso) or strictly opt-in federated (Verse), and Nostr/Matrix are first-class protocol citizens rather than extensions.

---

## 4. What Graphshell Has That Browsers Don't

These are axes that don't exist as first-class concerns in Chromium/Firefox/WebKit. They're what makes Graphshell a *spatial* browser rather than a conventional one.

### 4.1 Graph Truth

- **NodeKey ≠ URL.** A node has a stable identity that survives URL changes, redirects, and re-captures. URL is a property of a node, not the identity of one.
- **Edges are first-class.** Traversal edges, arrangement edges, containment edges, recency edges, graphlet membership — all modeled, all reducer-owned, all replayable.
- **Graph is the persistence substrate.** WAL-logged mutations through a single write path; snapshot via `rkyv`/`redb`/`fjall`.
- **Canonical spec**: `../implementation_strategy/graph/graph_node_edge_interaction_spec.md`.

### 4.2 Workbench

- **Tile Tree** over `egui_tiles::Tree<TileKind>` (egui host). Tab Group / Split / Grid containers.
- **Panes** as the workbench-owned presentation of a node. `PaneId` is the workbench identity; `NodeKey` is the graph identity; the Projection Rule connects them.
- **Graphlets** as meaningful bounded graph subsets, produced by projections. Graphlet anchors, primary anchors, backbones, migration proposals — none of which have a browser equivalent.
- **Canonical spec**: `../implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md`.

### 4.3 Navigator and Projections

- **Navigator** = single surface with configurable scope and form factor. Per user memory: do not split into multiple instances.
- **Projections** are the umbrella pattern: pure functions across domain boundaries (graph → tree rows, graph → map, graph → timeline). Projection pipeline has five stages: Scope → Shape → Annotation → Presentation → Portal.
- **ProjectionLens** enum in `graph-tree` parameterizes Shape stage for tree-family projections.
- **Canonical spec**: `../implementation_strategy/navigator/2026-04-21_navigator_projection_pipeline_plan.md` (in flight).

### 4.4 Registries (Host-Neutral Dispatch)

All use `namespace:name` key policy. These replace the ad-hoc "hardcoded list of available X" that a conventional browser chrome would use.

- `ActionRegistry` — action invocation routing. No hardcoded command enums.
- `ViewerRegistry` — MIME → non-web viewer resolution. `crates/verso` owns the broader engine-and-viewer routing.
- `ChannelRegistry` — diagnostic channel schema + severity.
- `InputRegistry` — keybinding resolution, user-remappable.
- `PhysicsProfileRegistry` — physics presets (Liquid/Gas/Solid/Frozen).
- `LensCompositor` — resolves a Lens (topology + layout + physics + theme).
- `KnowledgeRegistry` — UDC semantic tagging (planned).
- `ModRegistry` — native mod loading via `inventory::submit!`.
- `AgentRegistry` — autonomous background agents (planned).
- `ProtocolRegistry` — protocol handlers.

### 4.5 Focus Architecture

Six tracks, not one. A conventional browser has "document focus" and "chrome focus" (roughly); Graphshell differentiates:

- SemanticRegion (which region of UX owns capture),
- PaneActivation (which pane is workbench-active),
- GraphView (which graph view is focused),
- LocalWidget (which in-chrome widget has input),
- EmbeddedContent (which webview is the input target),
- ReturnCapture (where focus returns when a modal closes).

Canonical spec: `../implementation_strategy/subsystem_focus/2026-03-08_unified_focus_architecture_plan.md`.

### 4.6 Semantic Layer (UX)

- `UxTree` runtime snapshot, `UxNodeId` (stable path-based), `UxProbeSet`, `UxScenario` runner. WebDriver bridge planned.
- Intent: let automated tests and assistive tooling reason about the app at a semantic layer above the rendered pixels.
- Canonical spec: `../implementation_strategy/subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md`.

### 4.7 Distillery Aspect (Speculative)

- Security-gated transform layer that turns approved graph/history/clip/`AWAL` sources into typed intelligence artifacts.
- Local-first and policy-bound.
- No browser equivalent. `🔭 Speculative` — design not yet closed.
- Canonical spec: `../implementation_strategy/aspect_distillery/ASPECT_DISTILLERY.md`.

### 4.8 Mods

- `inventory::submit!`-based native mod registration. Protocol handlers, viewers, agents register as mods.
- Portable: WASI Preview 2 target for service-host mods; wasm32-unknown-unknown for browser-host mods.
- Canonical spec: `../implementation_strategy/subsystem_mods/` (when populated).

---

## 5. What Browsers Have That Graphshell Deliberately Doesn't

For each: why not (by design / by scope / by deferral).

| Browser subsystem | Graphshell posture | Rationale |
|---|---|---|
| WebExtension API | ⛔ not implemented | Mod model is the extensibility story. Compatibility envelope possible later. |
| Password manager | ⛔ not implemented | Delegate to OS / third-party vaults. Keeps Graphshell out of the credential-storage security cone. |
| SafeBrowsing | ⛔ not implemented | Opt-in list-based filtering could land via mod. Cloud-tied SafeBrowsing APIs conflict with local-first. |
| Anonymized usage telemetry | ⛔ not implemented | Local-first; no outbound telemetry without explicit user action. |
| Auto-update | ❓ unclear | Distribution model (native package vs. portable binary vs. extension) isn't settled. |
| Account-tied cloud sync | ⛔ not implemented | P2P (Verso) + federated (Verse) + Nostr/Matrix cover the use cases. |
| New-tab page / most-visited | ⛔ not implemented | Graph canvas IS the spatial equivalent. |
| Site permissions UI | ❓ delegated | Per-webview prompts handled by Servo; no Graphshell-level aggregation yet. |
| Bookmarks bar | ⛔ by design | Navigator + graph canvas cover bookmark affordances. |
| Download manager | ❓ delegated | No Graphshell-native download UI; Servo/OS handles it. |
| DevTools (elements / console / network) | ⛔ not yet | DOM Inspector planned as Wry-based viewer. Console/network deferred. |

Three of these (WebExtension API, SafeBrowsing, account sync) are **by-design exclusions** — the architecture says "not this way." The others (dev tools, downloads, auto-update, permissions UI) are **gaps** — design space is open, no decision has been made.

---

## 6. Gap Analysis — Unmapped Subsystems

Items where the right answer for Graphshell is genuinely undetermined and a scoping conversation would be useful:

1. **Sandbox model** — OS-level sandbox (seccomp-bpf on Linux, AppContainer on Windows, App Sandbox on macOS) hasn't been designed. Delegated to Servo's sandbox today. As Graphshell grows its own code paths — Direct Lane, Wry viewers, mods — this becomes a gap.
2. **Download manager surface** — neither UI nor policy is specified.
3. **DevTools story** — which elements (inspector, console, network) land as first-class tool panes and which don't.
4. **Permission prompts** — Servo handles per-webview; no Graphshell-level aggregation. As more mods request capabilities, a unified permission model is likely needed.
5. **Print** — spec-free today. Not urgent; flagging for completeness.
6. **Profile migration** — `#[serde(default)]` handles field addition. Schema-incompatible changes (rkyv layout bumps) don't yet have a migration policy.
7. **Crash reporting** — stack traces are captured locally; no policy on whether/how to surface them.
8. **Safe Browsing / URL reputation** — mod-based implementation possible; no list provider chosen.
9. **Multi-process architecture** — single-process is the status quo. Expected to become a real architectural question as the portable-core extraction matures.
10. **Find-in-page** — viewer-level, not yet implemented. Graph Search (Ctrl+G) is orthogonal (searches graph content, not the current page's DOM).

---

## 7. Crate/Module Quick Reference

For fast lookup. Each entry points at the canonical home for that concern.

| Concern | Crate / module |
|---|---|
| Identity + authority kernel | `crates/graphshell-core/` |
| Portable document engine (MiddleNet) | `crates/middlenet-engine/` + `middlenet-core/` + `middlenet-adapters/` + `middlenet-render/` |
| Graph canvas (force-directed layout + render) | `crates/graph-canvas/` |
| Workbench tree authority | `crates/graph-tree/` |
| Memory substrate | `crates/graph-memory/` |
| Routing authority (viewer/engine) | `crates/verso/` |
| Web runtime provider (Servo / Wry integration) | `mods/native/web_runtime/` |
| Egui host adapter | `shell/desktop/ui/gui.rs` (`EguiHost`) |
| Iced host adapter (scaffold) | `shell/desktop/ui/iced_host.rs` |
| Runtime state | `shell/desktop/ui/gui_state.rs` (`GraphshellRuntime`) |
| Host port surface | `shell/desktop/ui/host_ports.rs` |
| Compositor | `shell/desktop/workbench/tile_compositor.rs` |
| Viewer surface registry | `shell/desktop/workbench/compositor_adapter.rs` |
| App state / graph mutations | `graph_app.rs` + `app/*.rs` |
| Persistence | `app/persistence_facade.rs` + `app/settings_persistence.rs` + `app/startup_persistence.rs` |
| Control-panel workers (async) | `shell/desktop/runtime/control_panel.rs` |
| Registries | `shell/desktop/runtime/registries/` |
| Diagnostics | `shell/desktop/runtime/diagnostics/` |
| Render backend (wgpu surface / GL compat) | `shell/desktop/render_backend/` |
| Thumbnail pipeline | `shell/desktop/ui/thumbnail_pipeline.rs` |
| Focus state machine | `shell/desktop/ui/gui/focus_state.rs` |
| Input routing | `shell/desktop/ui/gui/input_routing.rs` |

---

## 8. How This Doc Stays Current

- Treat it as a **synthesis doc**: it points at canonical specs; if a canonical spec changes its subsystem's story, update this doc's row in the same session per DOC_POLICY §4.
- When a new subsystem lands (or gets explicitly deferred), add a row to §3 or §5 with a link to the spec or planning note.
- When a gap closes, move the row from §6 to §3 or §5.
- When a by-design exclusion is reconsidered, move from §5 to §3 and update the rationale.

The acceptance shape for this doc is not "everything listed" but "a contributor with browser-engineering background can find any subsystem's home in under 30 seconds, or knows definitively that Graphshell deliberately does not have one."

---

## 9. Summary

Graphshell as a browser, told in the shape that browser engineers already know:

- **Content pipeline**: Servo for full-web, MiddleNet engine for the document tier, three render lanes (Direct / HTML / Servo fallback).
- **Networking**: Servo's stack for web, adapter crates for MiddleNet protocols; `crates/verso` routes.
- **Process/isolation**: single-process today; sandbox + multi-process are open gaps.
- **Storage**: WAL + snapshots + runtime caches, all Graphshell-owned. No cloud account.
- **Navigation**: graph nodes/edges ARE navigation; History Manager + per-node + traversal edges supersede back/forward lists.
- **Chrome**: workbench tile tree + toolbar + three command surfaces (palette / radial / context). No bookmarks bar, no download manager UI (yet).
- **Input/a11y**: six-track focus authority; AccessKit bridge; Graph Reader planned.
- **DevTools**: deliberate gap. Diagnostics Inspector is the adjacent concern.
- **Extensions**: mod model (native + WASM), not WebExtensions.
- **Security**: local-first; no SafeBrowsing, no password manager, no outbound telemetry.
- **Sync**: P2P (Verso) + federated (Verse) + Nostr + Matrix. No central account.

Plus layers that no conventional browser has: **graph truth**, **workbench**, **navigator**, **registries**, **six-track focus**, **semantic UX layer**, **distillery (speculative)**, **mod system**.

If Graphshell were a conventional browser, §4 would be a feature list; because it isn't, §4 is the *architectural commitment* — the part of the shape that doesn't compress into Chromium's taxonomy.
