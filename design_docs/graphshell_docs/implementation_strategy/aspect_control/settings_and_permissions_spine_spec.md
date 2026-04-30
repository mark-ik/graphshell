<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Settings & Permissions Spine Spec

**Date**: 2026-04-30
**Status**: Canonical / Active
**Scope**: The five-scope layered configuration spine that all Graphshell
settings and permission grants resolve through: **default → persona → graph
→ view/tile → pane**. Defines override resolution, persistence per scope,
the unified permission model (which follows the same scope hierarchy), and
the migration path from the current `(GraphId, display_name, avatar?)`
profile shape to the persona-as-top-scope model.

**Related**:

- [`ASPECT_CONTROL.md`](ASPECT_CONTROL.md) — Control aspect authority (settings UI surface)
- [`settings_and_control_surfaces_spec.md`](settings_and_control_surfaces_spec.md) — settings-pane UX (orthogonal: this spec is the data/scope spine; that spec is the control-surface UI)
- [`../subsystem_security/SUBSYSTEM_SECURITY.md`](../subsystem_security/SUBSYSTEM_SECURITY.md) — security subsystem authority (permissions side reads from this spine)
- [`../subsystem_storage/SUBSYSTEM_STORAGE.md`](../subsystem_storage/SUBSYSTEM_STORAGE.md) — storage subsystem (per-scope persistence backing)
- [`../shell/SHELL.md`](../shell/SHELL.md) — Shell domain (frame composition consumes profile/persona scope)
- [`../shell/iced_browser_amenities_spec.md` §8](../shell/iced_browser_amenities_spec.md) — earlier "Profiles" amenity row; this spec absorbs and replaces its scope-model implications
- [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) — canonical terms (Persona added by this spec)

---

## 1. Intent

Every Graphshell setting and every permission grant has one of five
**scopes**. The scope determines who sees the change, where it persists,
and how overrides resolve. The five scopes form a strict containment
hierarchy:

```
default   <  persona  <  graph  <  view/tile  <  pane
(broadest)                                       (narrowest)
```

A request to read setting `X` resolves by walking from the narrowest
scope to the broadest, returning the first scope that defines `X`. A
write to setting `X` must specify the target scope; writing at a
narrower scope shadows the broader scope's value for surfaces in that
narrow scope.

Permission grants follow the same hierarchy with one extension (§5):
narrower scopes can only *narrow* an inherited permission, not widen
it. (You can't grant per-pane camera access if the persona-scope
permission denies it.)

This spine replaces ad-hoc settings storage spread across egui-era
code paths and aligns the architecture with `cosmic-config`-shaped
layered configuration patterns from libcosmic.

---

## 2. The Five Scopes

### 2.1 default scope

**Owner**: ships with Graphshell as compiled-in defaults.

**Persistence**: source code (the canonical default values) plus an
optional resource bundle for default-overrides at distribution time.

**Examples**: default theme tokens, default keybindings, default
canvas layout algorithm, default web-search engine, default
permissions denial (camera/mic/location off until granted).

**Mutation**: not user-mutable. A distribution may ship overrides via
a packaged resource bundle (e.g., a libcosmic-themed Graphshell could
ship default `Theme = cosmic-dark`).

### 2.2 persona scope

**Owner**: user identity. A persona is the top-level user-scope
container; one Graphshell installation may host multiple personas.

**Persistence**: per-persona settings tree under
`{config_dir}/graphshell/personas/{persona_id}/settings/` (libcosmic-config
shape: layered key-value tree, atomic-write semantics, change events
through Subscription).

**Examples**: theme override, default keybindings, default web-search
engine, identity material (Verse/Nostr/Matrix keys), per-persona
download directory, persona-default permissions.

**Mutation**: through a Settings pane targeted at persona scope
(`verso://settings/persona`).

**Cross-graph behavior**: persona-scope settings apply to **all**
graphs owned by the persona unless overridden at graph scope.

**Note on terminology** (added 2026-04-30): "Persona" supersedes the
egui-era "Profile" concept. Per
[`../shell/iced_browser_amenities_spec.md` §8](../shell/iced_browser_amenities_spec.md),
profiles were `(GraphId, display_name, avatar?)` records — one graph
per profile. The persona model decouples user-identity from graph;
one persona may own 1..N graphs. The migration path is in §7 below.

### 2.3 graph scope

**Owner**: one `GraphId`, owned by one persona.

**Persistence**: per-graph settings keyspace co-located with the graph
data store (`{config_dir}/graphshell/personas/{persona_id}/graphs/{graph_id}/settings/`).

**Examples**: per-graph layout algorithm, per-graph theme override
(e.g., a "research" graph in dark mode while "social" is in light),
per-graph default `WorkbenchProfile`, per-graph permissions (e.g.,
auto-grant camera access for embedded video-call nodes only in the
"work" graph).

**Mutation**: through a Settings pane targeted at graph scope
(`verso://settings/graph`), or via specific scope-bound actions.

### 2.4 view/tile scope

**Owner**: one `GraphViewId` (canvas pane) or one tile within a tile
pane.

**Persistence**: graph-state-adjacent. View/tile-scope settings
serialize alongside the view/tile in the Frame Snapshot and the graph
WAL (where applicable for per-tile content settings).

**Examples**: per-`GraphViewId` `ViewDimension` (2D / 3D), per-`GraphViewId`
`GraphLayoutMode` (Canonical / Divergent), per-tile reader-mode
toggle, per-tile content-zoom level.

**Mutation**: through view-local chrome (e.g., a 2D/3D toggle in the
canvas pane chrome) or a Settings pane scoped to the focused view/tile.

### 2.5 pane scope

**Owner**: one Pane in a Frame's split tree.

**Persistence**: alongside the Pane state in the Frame Snapshot.

**Examples**: pane chrome density (compact / regular), pane lock
state (`PaneLock`), pane-local keymap overrides (rare).

**Mutation**: through pane chrome or a context-menu action scoped to
the Pane.

---

## 3. Override Resolution

Reading a setting `X` resolves by walking narrowest-to-broadest:

```text
fn read(scope_path: ScopePath, key: SettingKey) -> SettingValue {
    for scope in scope_path.iter_narrow_to_broad() {
        if let Some(value) = scope.get(key) {
            return value;
        }
    }
    DEFAULT_VALUE  // compiled-in default; always present
}
```

`scope_path` is the active scope path for a read site:

- A canvas-pane in a graph rendered in a Frame: `[pane, view, graph, persona, default]`.
- A tile pane: `[pane, tile, graph, persona, default]` (tile here is the per-tile scope, not view).
- A persona settings pane (no graph context): `[persona, default]`.
- A Shell-level surface (no persona context — e.g., the persona picker itself): `[default]`.

Writes specify the **target scope** explicitly:

```rust
pub enum WriteScope {
    Default,                              // distribution-only; unreachable from user UI
    Persona(PersonaId),
    Graph { persona_id: PersonaId, graph_id: GraphId },
    View { /* path with persona/graph/view */ },
    Tile { /* path with persona/graph/tile */ },
    Pane { /* path with persona/graph/frame/pane */ },
}

pub trait SettingsWrite {
    fn set(&mut self, scope: WriteScope, key: SettingKey, value: SettingValue);
    fn unset(&mut self, scope: WriteScope, key: SettingKey);  // reverts to broader-scope value
}
```

`unset` at scope S reverts the read to the next-broader scope's value.
This is how a "Reset to graph default" button in a view-scope Settings
pane works: it unsets the view-scope override.

### 3.1 Setting categories and their canonical scope

Most settings have a **canonical scope** — the narrowest scope at
which they're meaningful. Writing at a narrower scope is permitted
but typically not surfaced.

| Category | Canonical scope | Examples |
|---|---|---|
| Theme tokens (color, typography, density) | persona | dark/light mode, accent color |
| Default keybindings | persona | global shortcuts; can be overridden per-graph |
| Default web-search engine | persona | which provider answers "Search the web for X" |
| Identity material (Verse/Nostr/Matrix keys) | persona | sync/identity infrastructure |
| Layout algorithm preference | graph | which scene-level layout default applies in this graph |
| Default `WorkbenchProfile` | graph | which Workbench composition opens new Frames |
| Frame Snapshot list | graph | persisted frames within a graph |
| `ViewDimension` (2D/3D) | view | per-canvas dimension mode |
| `GraphLayoutMode` (Canonical / Divergent) | view | per-canvas physics participation |
| Reader mode toggle | tile | per-tile reader rendering |
| Content zoom | tile | per-tile content scaling |
| Pane lock state | pane | per-Pane mobility lock |
| Pane chrome density | pane | per-Pane chrome verbosity |

The canonical scope is the **default write target** when a setting is
mutated through a generic Settings pane. Users may explicitly target
narrower scopes when meaningful (e.g., "set theme = sepia for *this
graph only*").

---

## 4. Persistence Model

Each scope persists through a different storage layer; the spine is
the abstraction over them.

| Scope | Storage backing | Format | Atomicity |
|---|---|---|---|
| default | source-compiled + optional resource bundle | Rust constants / TOML in distribution | n/a (immutable at runtime) |
| persona | `{config_dir}/.../personas/{id}/settings/` | layered key-value (libcosmic-config shape) | per-key atomic write + crash-safe |
| graph | per-graph keyspace in graph store (redb / fjall) | typed records keyed by SettingKey | inside graph WAL transactions |
| view / tile | Frame Snapshot + per-view records in graph WAL | rkyv-serialized typed records | covered by Frame Snapshot atomicity |
| pane | Frame Snapshot | rkyv-serialized typed records | covered by Frame Snapshot atomicity |

Cross-scope transactions (e.g., creating a new persona that
auto-creates a starter graph) are coordinated by Shell with idempotent
intents per the [TERMINOLOGY.md Intent Idempotence + Replay Contract](../../TERMINOLOGY.md).

### 4.1 Subscription channel

Settings changes emit through a single Subscription channel
(`SettingsEvent`), keyed by `(scope, key)`. iced surfaces consume the
Subscription to refresh in-flight UI state. Per the
[no-poll anti-pattern](../shell/iced_composition_skeleton_spec.md), surfaces
do not poll settings inside `view` — they read from the view-model
which is rebuilt per-tick from the latest Subscription deliveries.

### 4.2 Sync and replay

Persona-scope settings (and graph-scope settings, with explicit
opt-in) sync across the persona's devices via the Verso bilateral
sync layer. Settings mutations emit `SyncedIntent`s that satisfy the
TERMINOLOGY idempotence contract — applying a remote sync of the
same setting value is a no-op locally.

Replay (crash recovery, undo, time-travel diagnostics) follows the
same Intent contract; a replayed settings change produces the same
end state as the original.

---

## 5. Permissions Hierarchy

Permissions follow the **same five-scope hierarchy** as settings, with
one constraint: narrower scopes can only **narrow** an inherited
permission, never widen it.

### 5.1 Permission categories

| Category | Examples |
|---|---|
| Origin grants | per-origin camera / microphone / location / notifications |
| File access | per-graph filesystem read/write paths |
| Network access | outbound URL allowlists / blocklists for tools and viewers |
| Mod activation | per-graph mod enablement |
| Verse/Nostr/Matrix | per-persona identity grants for community network actions |

### 5.2 Narrowing rule

If persona-scope grants `camera = allow_per_origin_prompt` and graph
scope sets `camera = deny`, the effective permission for the graph
is `deny`. The narrower scope cannot escalate to `allow_always` if
the broader scope is more restrictive.

```text
persona:  camera = allow_per_origin_prompt
graph:    camera = deny             ✓  (narrower; allowed: deny is stricter)

persona:  camera = deny
graph:    camera = allow            ✗  (narrower; rejected: would widen)
```

The narrowing constraint is enforced at write time. A surface
attempting a widening write receives an explicit
`PermissionWriteError::WouldWiden(broader_scope_value)`; the user must
either change the broader scope first or accept the broader limit.

### 5.3 Permission Authority

Per [DOC_POLICY §11](../../DOC_POLICY.md), policy authority for
permissions lives in
[`../subsystem_security/SUBSYSTEM_SECURITY.md`](../subsystem_security/SUBSYSTEM_SECURITY.md).
This spec supplies the **scope spine** that the security subsystem
reads from; it does not redefine permission grants, prompt UX, or
revocation policy. The security subsystem owns those.

### 5.4 Servo-aligned permission boundary

For permissions concerning web content (camera / mic / location /
notifications / clipboard / etc.) the long-term preference per
the user's 2026-04-30 direction is for **Servo to own the
permission grant resolution** (since these are web-platform
permissions Servo already implements). Graphshell layers over Servo
by:

- providing the scope-spine for *which graph / persona / view*
  the grant applies under;
- routing prompt UI to a Graphshell-owned prompt surface (per
  [iced jump-ship plan §11 G18](../shell/2026-04-28_iced_jump_ship_plan.md));
- recording the resolution back into the spine at the canonical
  scope (typically per-origin, persona-scoped).

If Servo's permission resolution diverges from Graphshell's
policy needs (e.g., scope-aware revocation), Graphshell manages
the layered policy and consults Servo only for the underlying
web-platform call. The boundary remains explicit; Graphshell
does not silently override Servo's permission state.

---

## 6. Surface Read Patterns

Each surface reads settings against a known scope path:

| Surface | Scope path |
|---|---|
| App-level chrome (CommandBar, StatusBar) | `[persona, default]` |
| Persona picker | `[default]` |
| Graph canvas (main canvas) | `[view, graph, persona, default]` |
| Canvas Pane | `[pane, view, graph, persona, default]` |
| Tile Pane | `[pane, tile, graph, persona, default]` |
| Tile body (viewer) | `[tile, graph, persona, default]` |
| Settings panes (`verso://settings/<scope>`) | `[<scope>, default]` (writes target `<scope>`) |
| Tool panes (Diagnostics, Downloads, Devtools) | `[persona, default]` |
| Navigator hosts | `[host, persona, default]` (host scope is graph or workbench depending on host config) |

Each surface's spec lists its scope path; scope paths are stable
properties of surfaces and don't change at runtime.

### 6.1 Scope path computation

`graphshell-runtime` computes scope paths for active surfaces and
exposes them via `FrameViewModel`. Surfaces read settings via:

```rust
let value: SettingValue = view_model.settings(MY_SCOPE).get(SETTING_KEY);
```

The view-model's `.settings(scope)` returns a snapshot for the
walked path; settings are stable for one frame. New deliveries from
the Settings Subscription update the view-model on the next tick.

---

## 7. Migration: Profile (single-graph) → Persona (multi-graph)

Existing Graphshell installations have **profiles** with shape
`(GraphId, display_name, avatar?)` per
[`iced_browser_amenities_spec.md` §8](../shell/iced_browser_amenities_spec.md).
The persona migration:

1. **Existing profile becomes a persona** with one graph: each
   `(GraphId, display_name, avatar?)` profile maps to a persona
   with `display_name`, `avatar?`, and an initial graphs list of
   `[GraphId]`.
2. **Persona settings populate from existing per-`GraphId` settings**
   that were canonically persona-shaped (theme, keybindings,
   identity). Per-`GraphId` settings that are canonically graph-shaped
   stay graph-scoped.
3. **New personas are user-creatable** — a "New persona" action in
   the persona picker creates an empty persona with one starter
   graph.
4. **Cross-persona graph transfer** is **not** supported in the first
   bring-up; if a user wants a graph in a different persona, they
   export and re-import. (Cross-persona graph sharing is a future
   capability tracked under multi-persona collaboration.)

This is a one-shot migration script run on first launch after the
spine ships. Idempotent per the Intent contract: if the migration
has already run, re-running is a no-op.

---

## 8. Coherence Guarantees

Per the
[iced jump-ship plan §4.10](../shell/2026-04-28_iced_jump_ship_plan.md)
coherence guarantee for Settings panes:

> Settings changes never mutate graph truth. Per-graph settings are
> scoped to a `GraphId`; cross-graph settings are scoped to the
> user / profile. Theme changes apply across all surfaces atomically.

This spec preserves and extends the guarantee:

- A settings write at scope S affects only surfaces that include S
  in their scope path. Theme changes at persona scope reach all
  persona-owned graphs atomically (one Subscription event); theme
  changes at graph scope reach only that graph.
- A permission write that violates the narrowing rule (§5.2) is
  rejected; the user receives explicit feedback about the
  conflicting broader-scope value.
- Settings writes never mutate graph node identity, edges, or
  graphlet membership. Settings persist independently of graph WAL
  for persona scope and within graph WAL for graph-scope and
  narrower.

---

## 9. Open Items

- **Setting key catalog**: the canonical list of `SettingKey` values
  with their types, default values, and canonical scopes is a
  separate appendix; this spec defines the framework, not the catalog.
- **Permission UX**: prompt rendering, "remember my choice" UX,
  per-origin grant management surfaces — covered by
  `subsystem_security` specs, not here.
- **Sync conflict resolution**: when two devices write the same
  setting at the same persona scope simultaneously, last-writer-wins
  or merge logic. Tracked under the Verso sync spec.
- **Cross-persona graph transfer**: deferred (§7 #4). Future spec
  needed for export/import semantics that preserve graph truth.
- **Persona deletion**: graph-cascading-delete vs orphan-graphs
  policy. Tracked alongside profile deletion in `iced_browser_amenities_spec.md` §8.
- **Performance**: scope-path walk is O(depth), max 5 levels —
  trivially fast. Caching at the view-model layer is sufficient.

---

## 10. Bottom Line

Five scopes, narrowest-to-broadest, govern every setting and every
permission. Reads walk the path returning the first defined value;
writes target an explicit scope; permissions narrow but never widen
across scopes. The persistence layer maps each scope to its
appropriate storage (libcosmic-config-shape for persona, graph WAL
for graph and narrower). Persona supersedes the egui-era one-graph-
per-profile model with a multi-graph user-identity layer. Servo owns
web-permission resolution where applicable; Graphshell layers the
scope spine over it.

This spec is the data layer underneath
[`settings_and_control_surfaces_spec.md`](settings_and_control_surfaces_spec.md)
(which is the UI layer). The two compose: the UI spec says how
Settings panes look and route; this spec says where their reads
and writes land.
