# Workspace Architecture Proposal — decompose the root crate

**Status**: proposal, awaiting approval before extraction work begins.
**Author**: Slice 49 follow-on after the gophermap proof-of-concept.
**Audience**: anyone touching `Cargo.toml`, root-level Rust files, or the
`shell::desktop::*` tree.

---

## 1. Why this exists

The Graphshell workspace has 17 published member crates today, but the
root `graphshell` crate is itself a workspace-scale god object: 11
top-level subsystem directories (`app/`, `domain/`, `graph/`, `input/`,
`model/`, `mods/`, `platform/`, `registries/`, `render/`, `services/`,
`tests/`) plus the entire `shell/desktop/*` tree all compile as part of
one Cargo target. The §5 anti-patterns in
[`shell/2026-04-28_iced_jump_ship_plan.md`](shell/2026-04-28_iced_jump_ship_plan.md)
forbid new ≥600 LOC files and ≥6-responsibility structs, but they do not
forbid a 200K+ LOC workspace member, and the result is exactly the
file-level discipline being undone at workspace level.

The cost is concrete:

- **Build coupling.** Any change to any subsystem in the root crate
  rebuilds every other subsystem that compiles into the same target.
- **No conceptual boundary enforcement.** A `registries/` file can pull
  freely on `services/` internals, on `app/` private state, on
  `shell/desktop/runtime/` internals, etc. There is no compiler-enforced
  separation between layers.
- **Test isolation impossible.** A test that needs a registry cannot
  import only the registry — it must drag in the entire root crate.
- **Two registries directories.** `registries/` (top-level, 30 files,
  10,695 LOC) and `shell/desktop/runtime/registries/` (37 files, 32,538
  LOC) both exist. They overlap conceptually but are separated only by
  directory placement; the code review burden of reconciling them is
  invisible because they're in the same crate.

The Slice 49 gophermap-extraction proof-of-concept showed that
per-protocol crate extraction is mechanical when the protocol surface is
well-defined. Per-subsystem crate extraction at the root-crate level is
the same shape of work, scaled up.

---

## 2. Inventory — current sprawl by LOC

Captured 2026-05-01.

### 2.1 Root crate top-level directories

| Directory | LOC | Files | Conceptual content |
|---|---:|---:|---|
| `app/` | 25,852 | 35 | `GraphBrowserApp` god object + workspace routing + persistence facade + intent dispatch + agents |
| `registries/` (top-level) | 10,695 | 30 | atomic / domain / infrastructure / viewers — registry primitives + viewer-content registries |
| `render/` | 19,391 | 13 | Renderer host, surface composition, paint extraction (vendored from servoshell era) |
| `services/` | 6,664 | 11 | facts / import / persistence / query / search — app-level service implementations |
| `mods/` | 4,774 | 21 | native + wasm — mod runtime |
| `graph/` | 3,473 | 5 | frame_affinity / graphlet / physics / scene_runtime — running graph state in the binary |
| `model/` | 1,442 | 5 | archive + graph — domain model types |
| `input/` | 1,351 | 1 | Input dispatch |
| `domain/` | 31 | 1 | (essentially empty stub) |
| `platform/` | 45 | 1 | (essentially empty stub) |

### 2.2 `shell/desktop/` sub-tree

| Directory | LOC | Files | Conceptual content |
|---|---:|---:|---|
| `shell/desktop/ui/` | 53,777 | 99 | iced + egui hosts, both presents; widgets; panes; modals |
| `shell/desktop/runtime/` | 32,538 | 37 | iced-host runtime composition (registries, protocols, diagnostics, signal routing, snapshots) |
| `shell/desktop/workbench/` | 24,909 | 32 | egui workbench glue (frozen per S1) |
| `shell/desktop/host/` | 6,918 | 19 | egui-host residue (S6 deletion target) |
| `shell/desktop/lifecycle/` | 3,720 | 10 | webview lifecycle |
| `shell/desktop/tests/` | 5,406 | 20 | desktop integration tests |
| `shell/desktop/render_backend/` | 536 | 4 | wgpu renderer adapter |

### 2.3 Two duplicate `registries/` trees

- `registries/atomic/` — diagnostics, knowledge, lens, protocol,
  protocol_provider, viewer, viewer_provider
- `registries/domain/` — layout, presentation
- `registries/infrastructure/` — mod_activation, mod_loader
- `registries/viewers/` — audio, directory, image, middlenet, pdf,
  plaintext

vs.

- `shell/desktop/runtime/registries/` — action, agent, canvas, identity,
  index, input, knowledge, layout, lens, nostr_core, physics_profile,
  protocol, renderer, signal_routing, theme, workbench_surface, workflow

`knowledge`, `lens`, `protocol`, `layout` appear in both. The boundary
between them is not principled.

---

## 3. Decisions captured during the 2026-05-01 review

The following decisions came out of the user review of the first draft
of this proposal. They take precedence over earlier wording elsewhere
in the doc; later sections were rewritten to match.

### 3.1 Default to one-crate-with-modules; extract when merited

**Rule.** When a directory of related code becomes a candidate for
extraction, the default is to extract it as **one crate with one
module per concern**, not as one crate per concern. Individual
modules graduate to their own crate when there is a real reason —
typically: an external (non-Graphshell) consumer wants only that
module, or two existing Graphshell consumers want non-overlapping
subsets, or the module's compile time has become independently
load-bearing.

**Why.** Splitting eagerly into per-concern crates produces Cargo.toml
boilerplate, hides cycles inside re-export graphs, and frequently
produces crate boundaries that don't match the conceptual ones.
Module-level discipline inside one crate is enough to make the
boundary visible (file/module names, `pub(crate)` discipline) without
the overhead of separate Cargo manifests.

**Applied.** §4 (Layer C — App services) and §5 Phase 5
(`app/` decomposition) were both rewritten to one-crate defaults.
Layer B (registers) keeps its per-registry split — see §3.3 for the
distinct reasoning that justifies per-registry crates.

### 3.2 Crate hierarchy reflects architectural hierarchy

**Rule.** When the conceptual architecture says "X is part of Y,"
the crate layout says so too — X is either a module inside Y's
crate, or a sub-crate inside a directory named after Y. X is *not* a
sibling crate of Y unless the architecture genuinely says they're
peers.

**Concrete consequence.** The signal bus is part of the *system layer*
of Graphshell. The signal bus is therefore a module *inside*
`graphshell-runtime` (the system-layer crate), not a peer crate
`graphshell-signal-bus` next to it. The signal bus does not belong as
a standalone sibling.

**Concrete layout consequence.** Crates that share a conceptual
parent live together under a directory:

```text
crates/
  graph/              # graph domain crates
    graph-canvas/
    graph-tree/
    graph-cartography/
    graph-memory/
  middlenet/          # protocol family
    middlenet-core/
    middlenet-render/
    middlenet-adapters/
    middlenet-engine/
    middlenet-gopher/    # Slice 49b
  graphshell/         # graphshell core + system + app + registers + hosts
    graphshell-core/
    graphshell-comms/
    graphshell-runtime/    # the system layer; signal bus is a module inside
    graphshell-app/        # one crate, modules per app concern
    graphshell-services/   # one crate, modules per service
    registrar/             # registers live here (see §3.3)
      register-action/
      register-canvas/
      ...
    hosts/
      iced-host/
      iced-graph-canvas-viewer/
      iced-middlenet-viewer/
      iced-wry-viewer/
      iced-widgets/
  verso/              # Servo fork
    verso/
    verso-host/
```

Cargo doesn't care about the directory nesting (members are
listed explicitly in the root `Cargo.toml`), but humans do. The
directory names are the architecture.

### 3.3 What is a registry? — definition + smell tests

The proposal originally listed ~20 candidate "registries" without
defining the term. That risk is real: `registrar/` becomes a junk
drawer for "things named with `register*.rs`" rather than an
architectural seam. To prevent that, here is the definition each
candidate is checked against before extraction.

**A *registry* is**:

1. **A keyed namespace.** Entries are addressed by stable `namespace:name`
   keys (per the existing CLAUDE.md general code rule).
2. **An entry trait or value type.** Every entry implements a known
   trait or is a value of a known type. The registry's API is
   parametric over that trait/type, not over a sum type that lists
   every entry.
3. **A lookup API.** The registry exposes
   `lookup(key) -> Option<&Entry>` (or close analogues:
   `entries() -> impl Iterator<Item = &Entry>`,
   `dispatch(key, args) -> Result`, `try_resolve(key) -> Option`).
4. **Late binding.** Entries are added at registration time (startup,
   mod load, or feature gate), not hardcoded as enum variants.
   Multiple call sites — and ideally external mods or feature-gated
   modules — can register entries without modifying the registry's
   own crate.

**A registry is *not***:

- **A dispatcher** that owns a fixed set of cases and switches on a
  sum type. (`HostIntent` apply functions are dispatchers, not
  registries.)
- **A service** — a singleton with methods. (`PersistenceFacade` is a
  service, not a registry.)
- **A type module** — pure type definitions, no late-binding lookup.
- **A domain integration** — code that wraps an external system
  (`nostr_core.rs` may HOST a registry, but it is also probably the
  nostr integration crate's body. It does not automatically belong in
  `registrar/` just because it lives next to a `registries/` directory
  today).
- **A manifest or config loader** — even if it loads a list of named
  things, it isn't a registry unless other code can register more.

**Smell tests** to run on each candidate before extracting:

- Does it have an explicit `register(key, entry)` (or
  `register_*`) function that's called from multiple crates / feature
  gates / mod loaders? → registry.
- Are entries an open set (not enum-like)? → registry.
- Could a third party add an entry without modifying the candidate
  crate itself? → registry.
- If most answers are no → it's not a registry; find its real home.

**Action.** Slice 50 (the Phase 1 proof-of-concept) starts by
auditing each of the ~20 candidates in §3.4 below against this
definition and producing a short table of "registry / not-registry /
registry plus other concerns / unsure" before any extraction begins.
The table updates this proposal in place.

### 3.4 Naming for the register family

Folder name candidates considered:

| Name | Pro | Con |
|---|---|---|
| `register/` | conversational; matches user's instinct as "the place where you register things" | reads as a verb / artifact at the same time |
| `registrar/` | clear noun; names the role (the keeper of the registers); avoids singular/plural collision with "registry" | slightly bureaucratic |
| `registers/` | matches "many registries" | awkward as a directory name |
| `registry/` | conventional Rust pluralization-singular | direct collision with each subcrate also being a registry |

**Recommended: `registrar/`**. The folder is the *role* (the
maintainer of the registers); each subcrate inside is *a* registry.
The naming makes the role distinct from the artifacts and discourages
the "junk drawer of register-shaped things" failure mode by giving
the folder its own specific meaning. `register/` is acceptable if
brevity is preferred; the rest of the doc uses `registrar/`.

---

## 4. Conceptual architecture

The proposal organises Graphshell into five conceptual layers, each
with its own crate group. Numbers in parentheses are best-guess LOC for
the resulting workspace members.

### Layer A — Portable primitives (already extracted)

These crates already exist and stay as-is. They define the boundary
between graph truth and host:

- `crates/graphshell-core` — graph truth, ids, portable shell state,
  geometry, events, sanctioned writes, ux_observability, ux_probes
- `crates/graphshell-runtime` — host-neutral `runtime.tick()`,
  command dispatch, workbench/viewer/navigator services
- `crates/graph-canvas`, `crates/graph-tree`, `crates/graph-cartography`,
  `crates/graph-memory` — graph domain crates
- `crates/middlenet-core`, `crates/middlenet-render` — middlenet
  primitives + renderer
- `crates/middlenet-gopher` (Slice 49b) — protocol crates split per
  the per-protocol plan
- `crates/graphshell-comms` — comms primitives (newly added to
  workspace.members per Slice 49a)

### Layer B — Registrar layer (NEW conceptual home)

This layer is the one place where the per-crate split (rather than
one-crate-with-modules) actually earns its keep, because:

- Each registry is a load-bearing extension seam — by definition
  (§3.3), a registry's whole point is that other crates / features /
  mods can register entries from outside without modifying the
  registry's source. That outside reach is what "third party can
  depend on this registry alone" means in practice.
- A slim Graphshell build genuinely benefits from omitting registries
  it doesn't need (e.g., a build without the nostr feature should not
  pull in the nostr-related registry's deps).
- The two-trees duplication (top-level `registries/` and
  `shell/desktop/runtime/registries/`) is forced into reconciliation
  by the act of extraction.

**However**, before any per-registry crate is created, each candidate
must pass the smell tests in §3.3. Files in today's `registries/`
trees were collected by directory placement, not by passing the
definition. A real audit happens first.

#### B.1 Audit table (Slice 50, completed 2026-05-01)

Audit method: each candidate scored against the four §3.3 criteria
(keyed namespace, entry trait/value, lookup API, late binding for
extension entries). The audit caught **substantially more
non-registries than expected** — roughly half of the shell-side
"registries" are wrapper state, attachment tables, or dispatchers
that share the directory name but not the architectural role. The
canonical registries cluster in `registries/atomic/` and
`registries/domain/`; the shell-side files are mostly app/runtime
integration around them.

| Candidate (today's path) | Verdict (§3.3) | Destination |
|---|---|---|
| `shell/desktop/runtime/registries/action.rs` (2825 LOC) | **REGISTRY** | `crates/graphshell/registrar/register-action/` (huge — likely needs internal decomposition during extraction) |
| `shell/desktop/runtime/registries/agent.rs` (195 LOC) | **REGISTRY** (`Agent` trait + factory + descriptor + `agent:*` keys + `register()` API) | `registrar/register-agent/` |
| `shell/desktop/runtime/registries/canvas.rs` (116 LOC) | **NOT** — "active canvas profile selector" state, delegates lookup to `registries/domain/layout/canvas.rs` | folds into `graphshell-runtime/system/` (active-state); the canonical canvas-profile registry is `registries/domain/layout/canvas.rs` |
| `shell/desktop/runtime/registries/identity.rs` (1200 LOC) | **REGISTRY** (`register_*_persona` + `resolve_user_identity_id`) | `registrar/register-identity/` |
| `shell/desktop/runtime/registries/index.rs` (751 LOC) | **REGISTRY** (`register_provider`) | `registrar/register-index/` |
| `shell/desktop/runtime/registries/input.rs` (1463 LOC) | **REGISTRY** (`register_binding` + `resolve_binding_id`) | `registrar/register-input/` |
| `shell/desktop/runtime/registries/knowledge.rs` (407 LOC) | **NOT** — re-exports `KnowledgeRegistry` from `registries/atomic/knowledge.rs` and adds `reconcile_semantics` (app logic) | the registry itself moves with `registries/atomic/knowledge.rs` to `registrar/register-knowledge/`; `reconcile_semantics` and the `SemanticReconcileReport` shape land in `graphshell-app/` |
| `shell/desktop/runtime/registries/layout.rs` (145 LOC) | **REGISTRY** (small but registry-shaped, 4 register / 2 lookup / 2 dyn) | merge with `registries/domain/layout/profile_registry.rs` → `registrar/register-layout/` |
| `shell/desktop/runtime/registries/lens.rs` (3 LOC) | **NOT** — re-exports only | delete; canonical is `registries/atomic/lens/registry.rs` → `registrar/register-lens/` |
| `shell/desktop/runtime/registries/nostr_core.rs` (large; contains `NostrCoreRegistry`) | **NOT** a pure registry — primarily nostr integration that wraps a few registry-shaped sub-tables (relays, permissions) | extract as `crates/graphshell/graphshell-nostr` integration crate; clean registry sub-surfaces (relay set, permission grants) optionally extracted as `registrar/register-nostr-relays/` etc. once the integration extract is done |
| `shell/desktop/runtime/registries/physics_profile.rs` (92 LOC) | **NOT** — "active profile selector" state delegating to `registries/atomic/lens/physics.rs` | folds into `graphshell-runtime/system/` (active state); the canonical physics-profile registry is `registries/atomic/lens/physics.rs` and travels with `register-lens/` |
| `shell/desktop/runtime/registries/protocol.rs` (378 LOC) | **NOT** — protocol resolver/dispatcher using `protocols::registry as scaffold` | folds into `graphshell-runtime/system/` or `graphshell-app/`; the canonical `ProtocolContractRegistry` is in `registries/atomic/protocol.rs` → `registrar/register-protocol/` |
| `shell/desktop/runtime/registries/renderer.rs` (162 LOC) | **NOT** — bidirectional `pane ↔ renderer` attachment table; no late-bound extension entries (just runtime relationships) | folds into `graphshell-runtime/system/` as `pane_renderer_attachments` runtime state |
| `shell/desktop/runtime/registries/signal_routing.rs` + `shell/desktop/runtime/registry_signal_router.rs` | **NOT** — signal-bus seam | inside `graphshell-runtime/system/signal_bus.rs` per §3.2 |
| `shell/desktop/runtime/registries/theme.rs` (640 LOC) | **REGISTRY** (`register_theme` + `unregister_theme` + `resolve_theme` + `themes: HashMap<String, ThemeTokenSet>` + `theme:*` keys) | `registrar/register-theme/` — **chosen as the Slice 50 proof-of-concept** |
| `shell/desktop/runtime/registries/workbench_surface*` | **REGISTRY** (`resolve_*` for layout/interaction/focus/profile, profile-keyed) | `registrar/register-workbench-surface/` |
| `shell/desktop/runtime/registries/workflow.rs` (340 LOC) | **REGISTRY** (`WorkflowRegistry` + `resolve_workflow`) | `registrar/register-workflow/` |
| `registries/atomic/diagnostics.rs` (2425 LOC) | **REGISTRY** (`channels` / `configs` / `invariants` HashMaps + descriptor-literal registration; the channel catalog accounts for most of the LOC) | `registrar/register-diagnostics/` |
| `registries/atomic/knowledge.rs` (455 LOC) | **REGISTRY** (`KnowledgeRegistry` struct; the actual registry surface) | `registrar/register-knowledge/` (the shell-side reconcile_semantics moves to graphshell-app per the row above) |
| `registries/atomic/protocol.rs` (108 LOC) | **REGISTRY** (`ProtocolContractRegistry` + `register_scheme`) | `registrar/register-protocol/` |
| `registries/atomic/protocol_provider.rs` (52 LOC) | **REGISTRY** (provider registration) | `registrar/register-protocol/` (sibling to protocol.rs in same crate) |
| `registries/atomic/lens/registry.rs` (314 LOC) + `lens/{layout,physics,theme}.rs` | **REGISTRY** (`LensRegistry` + `RegisteredLens` + entry types) | `registrar/register-lens/` (with sub-modules per lens subsystem) |
| `registries/atomic/viewer.rs` (952 LOC) + `viewer_provider.rs` (52 LOC) + `registries/viewers/*` (audio/directory/image/middlenet/pdf/plaintext) | **REGISTRY** (viewer trait + register pattern); each `viewers/*.rs` file is an *entry* IN the viewer registry, not a separate registry | `registrar/register-viewer/` (viewer entries become sub-modules: `register-viewer/src/entries/{audio,directory,image,middlenet,pdf,plaintext}.rs`) |
| `registries/domain/layout/canvas.rs` + `profile_registry.rs` + `viewer_surface.rs` + `workbench_surface.rs` | **REGISTRY** (profile registries) | the layout-profile registries fold into `registrar/register-layout/`; viewer-surface and workbench-surface profile registries pair with their respective registers |
| `registries/infrastructure/mod_activation.rs` + `mod_loader.rs` | **REGISTRY** (mod registration is the canonical extension seam) | `registrar/register-mod-loader/` |

**Summary of surprises**:

- **Five "registries" are not registries**: `canvas.rs`,
  `physics_profile.rs`, `lens.rs` (3-LOC re-export), `renderer.rs`,
  `protocol.rs`, plus the previously-audited `signal_routing.rs`.
  They're either thin "active state" selectors or runtime
  attachment tables. They fold into `graphshell-runtime/system/`,
  not `registrar/`.
- **Two "registries" are app integration**: `knowledge.rs` and
  `nostr_core.rs`. Each contains either a thin registry wrapper
  (knowledge) or an embedded registry surface inside a larger
  domain integration (nostr). The audit splits these.
- **The canonical registries cluster in `registries/atomic/` and
  `registries/domain/`**, not under `shell/desktop/runtime/`. The
  shell-side runtime registries are mostly the *runtime
  composition* on top — wrappers, state, dispatchers, and one or
  two genuine registries that grew there organically (action,
  agent, identity, input, theme, workflow, workbench_surface,
  index).
- **Final registrar count**: ~12 registries (action, agent,
  diagnostics, identity, index, input, knowledge, layout, lens,
  mod-loader, protocol, theme, viewer, workbench-surface,
  workflow), down from the ~20 candidates the first draft listed.
  About 8 candidates fold elsewhere (mostly into
  `graphshell-runtime/system/`).

### Layer C — App services (NEW; one crate, modules per service)

Per §3.1, the default for `services/` is one crate with one module
per service:

```text
crates/graphshell/graphshell-services/
  src/
    lib.rs        # re-exports + facade
    facts/        # mod.rs, types
    import/
    persistence/
    query/
    search.rs
```

A given service module graduates to its own crate
(`graphshell-services-{name}`) only when one of the §3.1 conditions
fires: an external consumer wants only that module, two consumers
want non-overlapping subsets, or its compile time becomes load-bearing.
Until then, intra-crate `pub(crate)` plus module discipline is enough.

### Layer D — Signal bus (a module inside the system layer, not a peer)

Per §3.2, the signal bus is part of the system layer of Graphshell.
It is **a module inside `graphshell-runtime`**, not a sibling crate.

Today's signal-routing files live at:

- `shell/desktop/runtime/registry_signal_router.rs`
- `shell/desktop/runtime/registries/signal_routing.rs`

They consolidate into one module:

```text
crates/graphshell/graphshell-runtime/
  src/
    system/
      signal_bus.rs    # the consolidated signal-routing surface
      ...              # other system-layer concerns (caches, snapshots, ...)
    lib.rs
    ...
```

The runtime is the system layer; the signal bus is the module
through which the system routes signals. Routing the signal bus
through a sibling crate would invert the architecture (the system
would depend on a peer-of-the-system to do its own routing). Don't
do that.

If a downstream crate ever wants only the signal-bus surface
(without the rest of the runtime), the §3.1 graduation rule kicks
in and we extract `graphshell-signal-bus` then. There is no such
consumer today.

### Layer E — Hosts and host-bundled glue

The iced-host runtime composition that lives under
`shell/desktop/runtime/` today is *not* the portable runtime — it's the
iced bring-up of the runtime. It moves into `crates/hosts/iced-host`
(with `crates/hosts/` as the new directory home for all host crates):

| Crate | Source today |
|---|---|
| `crates/hosts/iced-host` | `shell/desktop/ui/iced_*` + `shell/desktop/ui/iced_host_ports.rs` + `shell/desktop/ui/iced_host.rs` |
| `crates/hosts/iced-graph-canvas-viewer` | already exists; moves into `hosts/` |
| `crates/hosts/iced-middlenet-viewer` | already exists; moves into `hosts/` |
| `crates/hosts/iced-wry-viewer` | already exists; moves into `hosts/` |
| `crates/hosts/iced-widgets` | rename of `crates/graphshell-iced-widgets` to fit the `hosts/` family naming |
| `crates/hosts/egui-host` | (S6 deletion target — the existing `shell::desktop::ui::gui*` + `shell::desktop::workbench` + `shell::desktop::host`) |

The `shell::desktop::lifecycle/` (webview lifecycle) folds into either
`crates/hosts/iced-host` or — if it turns out to be host-neutral — into
a new `crates/graphshell-webview-lifecycle`.

### Layer F — Verso / Servo fork (already extracted)

`crates/verso` and `crates/verso-host` stay as-is. middlenet does NOT
move under verso — middlenet's whole point is portability outside
Servo, and grouping its directory under `verso/` would send the wrong
signal.

### What stays in the binary root

After all extractions, the root `graphshell` crate contains only what a
Cargo binary needs:

- `main.rs`, `build.rs`, `lib.rs` (composition root only)
- `panic_hook.rs`, `crash_handler.rs`, `backtrace.rs` (process-startup
  glue)
- `parser.rs`, `graph_app.rs`, `graph_resources.rs`, `graph_app_tests.rs`
  (composition glue — likely thinned dramatically once dependencies
  move out)

Everything else moves to one of the layers above. The binary's job
becomes: select default host, wire features, run.

---

## 5. Sequencing

Big-bang extractions are bad. The proposed order:

### Phase 1 — Audit + first-registry proof-of-concept (Slice 50)

Two tasks:

1. **Audit the candidates** in §B.1 against §3.3. Resolve every
   "TBD" verdict to **registry**, **not-registry**, or
   **registry-plus-other-concerns**. Update the §B.1 table in place.
2. **Extract one** of the rows that audited as a clean registry — the
   smallest with the fewest external callers. (Likely
   `register-theme` or `register-physics-profile`; the audit will
   confirm.) Use the middlenet-gopher extraction (Slice 49b) as the
   template.

The audit is the gating step. Without it, the registrar folder
becomes the junk drawer §3.3 warns against.

### Phase 2 — Consolidate the signal bus (Slice 51)

Per §3.2 / Layer D, the signal-routing surface lands as a module
**inside** `graphshell-runtime`'s `src/system/signal_bus.rs`, not as
a peer crate. The two existing signal-routing files
(`shell/desktop/runtime/registry_signal_router.rs` and
`shell/desktop/runtime/registries/signal_routing.rs`) consolidate
into that module. Other system-layer concerns
(`caches.rs`, `protocol_probe.rs`, `snapshots/`, `tracing.rs`)
follow as `system/` siblings in subsequent slices when they need to
move.

### Phase 3 — Service extraction (Slice 52)

Extract `crates/graphshell/graphshell-services/` as **one crate with
one module per service** (`facts`, `import`, `persistence`, `query`,
`search`). Per §3.1, individual services do not get their own
crate until a §3.1 graduation condition fires.

### Phase 4 — Registry sweep (Slices 53-60+)

Extract the remaining registries from §B.1, one per slice, using
the Phase 1 template. Order: leaves before roots in the dependency
graph (i.e., a registry that nothing else depends on first;
registries with broad dependents last). Each slice updates §B.1's
verdict + destination columns to **DONE**.

### Phase 5 — `app/` decomposition (Slices 61+)

Per §3.1, the default is **one `graphshell-app` crate with modules
per concern**, extracted as a single move. The candidate modules:

- `graph_app/` — `graph_*.rs`, `arrangement_graph_bridge`,
  `canvas_scene`
- `workspace_routing/` — `routing.rs`, `workspace_*.rs`,
  `workbench_*.rs`
- `persistence/` — `persistence_facade.rs`, `settings_persistence.rs`,
  `startup_persistence.rs`, `storage_interop/`
- `intent_system/` — `intent_phases.rs`, `intents.rs`,
  `focus_selection.rs`, `selection.rs`
- `agents/`, `history.rs`, `history_runtime.rs`, `ux_navigation.rs`,
  `action_surface.rs` — UX-side concerns; group as
  `app_ux/`
- `runtime_lifecycle.rs`, `runtime_ports.rs` — runtime composition
  glue; group as `composition/`

A module graduates to its own `graphshell-app-{name}` crate per
§3.1 if/when it earns it (likely candidates: `persistence`, since
the storage layer is an obvious extension seam; `graph_app`, since
graph-app glue is a candidate for outside reuse). Until then, intra-
crate `pub(crate)` boundaries are sufficient.

### Phase 6 — Host moves (Slices 70+)

- Move `crates/iced-*-viewer` and `crates/graphshell-iced-widgets` into
  `crates/graphshell/hosts/` per the §3.2 directory layout. Cosmetic
  but clarifies the Cargo.toml at a glance.
- Extract `shell/desktop/ui/iced_*` into
  `crates/graphshell/hosts/iced-host/`.
- The egui-host residue (`shell/desktop/ui/gui*`,
  `shell/desktop/workbench/`, `shell/desktop/host/`) is the S6 deletion
  target from the iced jump-ship plan; do not extract it as its own
  crate.

### Phase 7 — `render/` decision (defer)

The 19,391-LOC `render/` directory is mostly servoshell/Servo-renderer
glue that's slated for either retirement (S6) or significant rework as
part of the renderer-and-host-refactor plan
([`aspect_render/2026-04-30_renderer_and_host_refactor_plan.md`](aspect_render/2026-04-30_renderer_and_host_refactor_plan.md)).
Defer extraction decisions for `render/` until the refactor plan picks
its target shape.

---

## 6. Risks and tradeoffs

- **Cargo build-graph churn.** Every extraction churns
  `Cargo.toml` + `Cargo.lock`, requires `cargo build` to repopulate
  target/, and may surface latent visibility issues. Mitigation:
  one extraction per slice; verify build + tests after each.
- **IDE re-indexing.** rust-analyzer re-indexes on workspace changes.
  Annoying for the reviewer, not blocking.
- **Per-crate boilerplate.** Each new crate adds one Cargo.toml,
  one src/lib.rs (often just re-exports), and an entry in
  workspace.members. The §3.1 default (one crate with modules until
  graduation) keeps this overhead bounded; the only place it's worth
  paying eagerly is the registrar layer (§3.3), where the
  extension-seam shape directly justifies the split.
- **Public API surface accidentally widens.** Code that was
  `pub(crate)` in the root crate becomes `pub` in the extracted crate
  unless deliberately gated. Mitigation: extraction PRs explicitly call
  out which items become public; add `#[doc(hidden)]` for anything that
  shouldn't be a stable API.
- **Registrar becomes a junk drawer.** The §3.3 definition + smell
  tests + Slice-50 audit are the mitigations. Without them, anything
  named `register*.rs` or living next to a `registries/` directory
  ends up in `registrar/` regardless of whether it's actually a
  registry — the failure mode the user explicitly flagged in the
  2026-05-01 review.

---

## 7. Decision log (resolved 2026-05-01)

The first draft of this proposal listed three open decision points.
All three were resolved in the 2026-05-01 user review and are
captured in §3.1 / §3.3 / §3.4. Repeated here as a single index:

| Decision | Resolution | Captured in |
|---|---|---|
| Folder name for the registry-of-registries | `registrar/` (recommended) — names the *role* and avoids singular/plural collision; `register/` acceptable if brevity wins | §3.4 |
| One service crate or per-service crates | One crate with modules (default per §3.1); split when graduation conditions fire | §3.1, Layer C |
| `app/` cut shape | One crate with modules (default per §3.1); split when graduation conditions fire | §3.1, §5 Phase 5 |
| Where the signal bus lives | A module **inside** `graphshell-runtime`'s `system/`, not a sibling crate | §3.2, Layer D |
| What counts as a registry | §3.3 definition + smell tests; Slice 50 audits each candidate | §3.3 |

---

## 8. What this document is not

- Not a slice plan with acceptance criteria. The slice plan lives in
  the relevant per-slice plan documents per
  [DOC_POLICY §9](../DOC_POLICY.md).
- Not authoritative on the `render/` directory's future — that lives
  with the renderer-and-host-refactor plan.
- Not a refactor of the iced jump-ship plan. The `app/` decomposition
  in §5 Phase 5 is parallel to S5 / S6 of that plan, not a
  replacement.
