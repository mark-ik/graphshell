<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<!-- markdownlint-disable MD024 MD030 MD032 MD036 MD004 -->

# Node Badge and Tagging Plan (2026-02-20)

**Status**: Closed / Archived 2026-04-01 — broad historical plan retained as an implementation record after the remaining follow-on work landed

**Closure note**:

- The former follow-on remainder is now closed in code: the tag panel is shell-hosted, routed through the input/action stack, reachable from command surfaces plus existing inspector/toolbar affordances, and covered by focused interaction tests.
- The keyboard route settled on `Ctrl+T`; plain `T` remains the physics toggle.
- The icon-picker scope was intentionally narrowed to searchable emoji-only. `BadgeIcon::Lucide` remains a type-level escape hatch, but Lucide assets/search are not part of the current shipped panel contract.
- Current active authority is `node_badge_and_tagging_spec.md`. This archived plan remains historical context only.

**Prerequisites**:

- Persistence hub Phase 1 tag actions and runtime tag storage. Landed.
- Current runtime reality: canonical membership lives on `Node.tags`; presentation ordering and icon overrides now also live on-node through `NodeTagPresentationState`.
- No active implementation prerequisites remain for this plan.

This plan now serves as the broad historical plan and landed-scope record for node badges and tagging. The reduced follow-on plan created on 2026-03-31 has also been closed and archived.

This plan covers the visual and interactive layers on top of that data model, plus the presentation metadata that now makes icon assignment and ordering durable.

---

## Plan

### Context

Tags are user-applied node attributes (persistence hub plan Phase 1). This plan covers:

1. **Badge visual system** — how tags are rendered on graph nodes and tab headers.
2. **Tag assignment UI** — the `T`-key floating panel for adding and removing tags.
3. **Icon system** — emoji (primary) and Lucide SVG (extended) icon sources.
4. **Tag presentation metadata** — durable icon choice and ordering semantics for user tags.

#### Tag Namespace Convention

The `#` prefix is the **reserved system namespace**. Tags beginning with `#` may carry behavioral
effects. User-defined tags without `#` (`work`, `research`, `todo`) are purely organizational and
never trigger system behavior.

**Reserved special tags:**

| Tag | Responding system | Effect |
| --- | --- | --- |
| `#pin` | Physics simulation | Node not displaced by the simulation (physics anchor). |
| `#starred` | Search / omnibar | Soft bookmark; surfaces via `@b` scope and `is:starred` predicate. |
| `#archive` | Graph view | Hidden from default graph view; demoted to archive tier. Not deleted. |
| `#resident` | Lifecycle / cache | Never cold-evicted regardless of workspace. Global complement to `tile.is_resident`. |
| `#private` | Rendering / export | URL and title redacted in overlays when screen-sharing mode is active. Excluded from JSON export. |
| `#nohistory` | Traversal recording | Navigating through this node does not push a traversal entry. |
| `#monitor` | Background scheduler | Periodically reload and compare DOM hash; badge/toast on content change. |
| `#unread` | Badge / DOI | Auto-applied when a node is added or its URL changes; cleared on first activation. |
| `#focus` | DOI weight | Boosts this node's degree-of-interest score; floats it toward center in the layout. Short-term attention marker. |
| `#clip` | Node type | Node is a clipped DOM element. Distinct node shape/border in graph view; `is:clip` omnibar predicate. Cross-ref: clipping DOM extraction plan. |

The tag assignment UI warns (but does not block) when a user types a `#`-prefixed tag that is not
in the reserved list — the tag is accepted as user-defined and carries no system effect. This
forward-compatible: future versions can upgrade a previously-inert `#foo` tag to a reserved one
without breaking existing data.

---

### Phase 1: Badge Visual System

**Status note (2026-03-31)**: Functionally landed. The badge model, shared resolver, canvas badges, orbit expansion, tab-header suffixes, archive dimming, and `#clip` dashed border treatment are all in code. Remaining work in this area is limited to test/path cleanup captured in the follow-on plan.

#### 1.1 Badge Types

```rust
#[derive(Debug, Clone)]
pub enum Badge {
    /// Node has crashed or Servo reported an error.
    Crashed,
    /// Node is a member of N workspaces (shown as a count chip).
    WorkspaceCount(usize),
    /// Node has the #pin tag (physics anchor).
    Pinned,
    /// Node has the #starred tag (soft bookmark).
    Starred,
    /// Node has the #unread tag (auto-applied; cleared on first activation).
    Unread,
    /// Node has a system or user tag with an icon and optional label.
    Tag { label: String, icon: BadgeIcon },
}

#[derive(Debug, Clone)]
pub enum BadgeIcon {
    /// Unicode emoji character (e.g. "📌", "⭐").
    Emoji(String),
    /// Lucide SVG icon identified by slug (e.g. "bookmark", "tag").
    Lucide(&'static str),
    /// No icon — label-only chip.
    None,
}
```

#### 1.2 Priority Order

Badges render in this priority order (highest first). When space is constrained, lower-priority
badges are hidden first:

1. `Crashed` — always visible, red indicator.
2. `WorkspaceCount` — shown when node belongs to 2+ workspaces.
3. `Pinned` — `#pin` tag present.
4. `Starred` — `#starred` tag present.
5. `Unread` — `#unread` tag present; rendered as a colored dot (distinct from icon chips).
6. Other system tags (`#focus`, `#monitor`, `#private`, `#archive`, `#resident`, `#nohistory`) —
   rendered as `Tag` chips using their default emoji; ordered by canonical system-tag priority unless a later presentation-metadata phase introduces explicit user-facing ordering.
7. UDC Semantic tags (`udc:51`) — rendered as label chips (e.g. "51 Mathematics") or codes.
8. User-defined tags — ordered deterministically. Current runtime storage uses `HashSet<String>`, so insertion order is not available without additional presentation metadata.

`#archive` is a special case: the badge renders but the node itself is visually dimmed (reduced
opacity in graph view). `#unread` is auto-managed by the system and is the only badge the user
does not assign manually.

#### 1.3 At-Rest Display (Graph View)

On graph nodes, badges occupy a small overlay region at the node's top-right corner:

- At rest: up to **3** badges rendered as small icon-only circles (16×16 px). If more than 3
  badges exist, the third slot shows a `+N` overflow chip.
- `Crashed` uses a red ⚠ glyph and overrides any other badge in slot 1.
- `WorkspaceCount` renders as a small numeric chip (e.g. `2`).
- `Pinned` renders the 📌 emoji or a Lucide `pin` icon.
- `Starred` renders the ⭐ emoji or a Lucide `star` icon.
- User tags render the assigned `BadgeIcon`; if `None`, a small colored dot.

#### 1.4 Hover/Focus Expansion (Orbit Model)

When the cursor enters a node, or when the node is keyboard-focused:

- All badges expand from their at-rest corner position and orbit the node periphery.
- Expansion animation: 120 ms ease-out (respect `prefers-reduced-motion` → instant).
- Expanded badges show both icon and label (truncated at 12 chars).
- The orbit radius scales with node size (minimum 32 px from node center).
- Badges are non-interactive in orbit — clicking anywhere on the badge area opens the tag
  assignment panel (same as `T` key).

Implementation: store `badge_expand_t: f32` (0.0 → 1.0) per node in `GraphNodeShape` or a
parallel `HashMap<NodeKey, f32>`. Animate via `ctx.request_repaint()` each frame until 1.0.

#### 1.4.1 `#clip` Node Visual Treatment

`#clip` nodes are clipped DOM elements — a distinct node type, not just a tag with a badge. Their canvas rendering must communicate this type distinction beyond the badge orbit:

| Property | Treatment |
| --- | --- |
| Node border | Dashed stroke (distinguishes from solid-border page nodes) |
| Node shape | Same geometry as a page node (rectangle with rounded corners); no shape change |
| Badge | ✂️ emoji badge in the at-rest corner; shown in orbit with "Clip" label |
| Opacity | Full opacity (not dimmed — `#clip` is not an archived state) |
| `#archive` + `#clip` | Dimmed as normal archive treatment; clip border style preserved |

**Invariant**: `#clip` border treatment is a canvas-node-level concern (`GraphNodeShape`), not a tile-level Pass 3 concern. Clip nodes open in a viewer pane like any other node; their tile does not receive special compositor treatment.

#### 1.5 Tab Header Badges

In the detail-view tab bar, each tab header shows a compact badge row to the right of the title:

- At rest: icon-only, up to 2 badges (Crashed + one more).
- Crashed shows as a red dot suffix.
- `#starred` shows ⭐; `#pin` shows 📌.
- User tags: first tag icon only (no overflow — tab headers are narrow).

#### Tasks

- [x] Define `Badge` and `BadgeIcon` enums in `model/graph/badge.rs`.
- [x] Add `fn badges_for_node(node: &Node, workspace_count: usize, is_crashed: bool) -> Vec<Badge>` helper.
- [x] In `GraphNodeShape::ui()`: compute badges and render the at-rest overlay (top-right corner).
- [x] Add per-node `badge_expand_t` state on `GraphNodeShape` and drive repaint-based animation from the egui adapter.
- [x] Animate badge expansion on hover/selection.
- [x] Render expanded orbit layout when `badge_expand_t > 0`.
- [x] Tab bar renders compact badge suffixes per node tab in `shell/desktop/workbench/tile_behavior/tab_chrome.rs`.
- [x] In `GraphNodeShape::ui()`: nodes with `TAG_ARCHIVE` render at reduced opacity (0.35–0.45)
  when the "Show archived" graph view toggle is on. Excluded entirely when toggle is off.
- [x] In `GraphNodeShape::ui()`: nodes with `TAG_CLIP` render a dashed border stroke instead of the default solid border. All other geometry is unchanged.

#### Validation Tests

- `test_badges_for_pinned_node` — node with `#pin` tag → `badges_for_node` returns `[Pinned]`.
- `test_badges_for_starred_node` — node with `#starred` → `[Starred]`.
- `test_badges_priority_order` — node with `#pin`, `#starred`, `work` tag → order is Pinned,
  Starred, Tag.
- `test_crashed_badge_first` — Crashed + Pinned → Crashed is first.
- `test_at_rest_capped_at_three` — 5 badges → 3 rendered + `+2` overflow chip.

---

### Phase 1.5: Tag Presentation Metadata

**Status note (2026-03-31)**: Landed. The presentation metadata carrier exists on `Node`, reducer flows maintain it, persistence includes it, and badge resolution consumes it.

The original version of this plan assumed that tag order and icon choice could be layered directly on top of a plain set of tag strings. That is no longer sufficient.

Current runtime storage can answer "does this node have tag X?" through `Node.tags`, but it still cannot durably answer:

- which user tag should render first,
- which icon the user assigned to a user-defined tag,
- whether a user tag should have a custom presentation color or source marker later.

Before the full icon picker and user-controlled ordering ship, Graphshell needs a small presentation metadata carrier separate from the canonical tag-membership set.

#### 1.5.1 Proposed Presentation Metadata Shape

```rust
pub struct NodeTagPresentationState {
    pub ordered_tags: Vec<String>,
    pub icon_overrides: HashMap<String, BadgeIcon>,
}
```

This is intentionally presentation-only:

- canonical membership remains the canonical tag set on `Node.tags`
- system tags keep fixed icons and do not use overrides
- user-defined tags may use `icon_overrides`
- `ordered_tags` provides stable user-visible ordering when present

If no presentation metadata exists for a node, rendering falls back to deterministic sorted order.

#### 1.5.2 Invariants

- Membership truth comes from the canonical tag set, not from presentation metadata.
- Presentation metadata may reorder or decorate tags but may not fabricate membership.
- System tags (`#pin`, `#starred`, etc.) keep their canonical icons regardless of overrides.
- Removing a tag must also prune any stale presentation metadata for that tag.

#### Tasks

- [x] Define a presentation metadata carrier for per-node tag order and icon overrides.
- [x] Sync `TagNode` / `UntagNode` flows so metadata is initialized and pruned consistently.
- [x] Make badge resolution consume presentation metadata when available and deterministic fallback ordering otherwise.
- [x] Make icon picker selection write presentation metadata instead of trying to encode icon choice into the raw tag string.

#### Validation Tests

- `test_user_tag_presentation_order_is_stable_when_metadata_present`
- `test_untag_prunes_stale_icon_override`
- `test_system_tag_icon_cannot_be_overridden`

---

### Phase 1.6: Canonical Tag Ownership Migration

Canonical tag ownership migration is now landed in code: `Node.tags` is the canonical durable tag set and the former `workspace.semantic_tags` mirror has been removed.

Canonical tag truth now lives on the node itself.

#### 1.6.1 Target Ownership

- `Node.tags` is the canonical durable tag set.
- `workspace.semantic_index` remains a derived cache.
- `workspace.suggested_semantic_tags` remains a non-canonical suggestion surface.
- `workspace.tag_panel_state` remains transient UI state.

This aligns tags with their actual meaning: tags are node-associated metadata, not workspace/session-scoped truth.

#### 1.6.2 Migration Strategy

The migration was staged so read paths could move before the old mirror was deleted. The remaining work in this section is validation/documentation follow-through rather than ownership design.

**Step 1 — Add tags to `Node`**

- Add `tags` to `Node`.
- Initialize it in node constructors and snapshot/replay paths.
- Treat it as durable semantic truth.

**Step 2 — Add helper read APIs that prefer node-owned tags**

- Add graph/model helper APIs for reading tags from nodes.
- Route badge/render/search/registry consumers through those helpers first.
- During the bridge phase, the helper may still fall back to the workspace mirror if needed.

**Step 3 — Temporary dual-write reducer bridge**

- Change `TagNode` / `UntagNode` reducers to write:
  - `node.tags`
  - `workspace.semantic_tags`
- Keep `semantic_index_dirty = true`.
- Keep existing `#pin` synchronization behavior intact.

**Step 4 — Move all readers to node-owned tags**

Update these families to read node-owned tags (preferably through helper APIs):

- badge resolution
- graph render
- tab-header badge suffixes
- search/index provider
- knowledge registry runtime adapter
- selected-node enrichment UI
- tag panel UI

**Step 5 — Rebuild semantic index from nodes**

- Change `reconcile_semantics()` to iterate graph nodes and read `node.tags`.
- Stop iterating `workspace.semantic_tags`.
- Remove stale-tag pruning logic that only exists because tag storage is separate from node lifetime.

**Step 6 — Remove the temporary mirror**

- Delete `workspace.semantic_tags`.
- Remove any bridge code still writing or clearing it.
- Keep `semantic_index` and `semantic_index_dirty` as derived/runtime cache state.

**Step 7 — Compatibility and tests**

- Update reducer tests.
- Update knowledge/index tests.
- Update render/badge tests.
- Review snapshot/archive compatibility because adding `tags` to `Node` is a persisted schema change.

#### 1.6.3 Invariants

- Canonical membership truth lives on nodes, not on workspace/session state.
- `semantic_index` remains derivable from node tags + `KnowledgeRegistry`.
- No read path should depend on `workspace.semantic_tags` after the migration reaches Step 4.
- The dual-write phase is temporary and must have an explicit deletion step.

#### Tasks

- [x] Add tags to `Node`.
- [x] Add helper read APIs that prefer node-owned tags.
- [x] Change `TagNode` / `UntagNode` reducers to write node tags.
- [x] Change all core readers to use node tags.
- [x] Change `reconcile_semantics()` to iterate nodes, not `workspace.semantic_tags`.
- [x] Remove the temporary mirror field `workspace.semantic_tags`.
- [x] Update tests and snapshot compatibility as needed.

#### Validation Tests

- `test_badge_and_registry_reads_prefer_node_owned_tags`
- `test_reconcile_semantics_rebuilds_from_node_tags`
- `test_workspace_semantic_tags_removed_without_behavior_regression`

---

### Phase 2: Tag Assignment UI

**Status note (2026-03-31)**: Partially landed. The non-modal tag panel exists and is reachable from the Selected Node inspector and the graph toolbar, supports add/remove intents, suggestions, and durable user-tag icon overrides, but it still lacks the original deliberate trigger/close contract and has not yet been extracted out of render-layer code.

#### 2.1 Trigger

The tag assignment panel opens when:

- `T` key is pressed with a node selected (graph view).
- Right-click context menu → "Tags…" (graph view).
- A tag chip on an expanded badge is clicked.

In detail view, `T` key targets the focused tab's node.

#### 2.2 Panel Layout

```text
┌─────────────────────────────────────────┐
│  Tags for "Servo Embedder Notes"         │
│  ┌────────────────────────────────────┐ │
│  │ 📌 #pin  ⭐ #starred  🔬 research × │ │
│  └────────────────────────────────────┘ │
│  ┌──────────────────────────────┐  [⊞] │
│  │  Add tag…                    │       │
│  └──────────────────────────────┘       │
│  ── Suggestions ──────────────────────  │
│    #work   #todo   research   todo      │
└─────────────────────────────────────────┘
```

- **Chip row**: existing tags as removable chips. Click ✕ on a chip → emit `UntagNode`.
- **Text field**: type to filter suggestions via nucleo fuzzy matching.
- **`[⊞]` icon button**: opens the full icon picker (§2.3).
- **Suggestions row**: top-5 results from nucleo against `tag_index` keys + emoji names.
- `Enter` or clicking a suggestion: add the tag (emit `TagNode`).
- `Esc`: close panel without changes.

The panel is non-modal, anchored near the node (below or to the right, whichever fits). It closes
automatically if the node is deselected or the user clicks outside.

#### 2.3 Icon Picker

Accessible via the `[⊞]` button in the tag assignment panel or by clicking the icon slot of an
existing user tag chip.

Layout: a 8×N scrollable grid of emoji, with a "Lucide" tab for SVG icons.

```text
┌─ Choose icon ──────────────────────────────┐
│  [Emoji ●]  [Lucide]     🔍 search icons…  │
│  ────────────────────────────────────────  │
│  ⭐ 📌 🔬 🗂 📎 📝 🌐 🔗                    │
│  🏷 📚 🗃 🧪 💡 🔑 🚀 📊                    │
│  …                                         │
│  [Cancel]                         [Select] │
└────────────────────────────────────────────┘
```

- Search field uses nucleo against emoji names (e.g. "bookmark" → 🔖 📑).
- Lucide tab: subset of ~200 most useful icons; search by slug name.
- Selecting an icon updates the tag chip preview in the parent panel before confirming.

#### 2.4 Nucleo Integration

[nucleo](https://github.com/helix-editor/nucleo) is a Rust fuzzy-matching library (MIT).

```rust
// In tag_panel.rs or omnibar.rs
use nucleo::{Matcher, Utf32Str};

fn fuzzy_rank_tags(query: &str, candidates: &[String]) -> Vec<(f32, &String)> {
    let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
    candidates.iter().filter_map(|tag| {
        let score = matcher.fuzzy_match(
            Utf32Str::new(tag, &mut Vec::new()),
            Utf32Str::new(query, &mut Vec::new()),
        );
        score.map(|s| (s as f32, tag))
    })
    .collect()
}
```

The same matcher instance is reused across keystrokes (reset on panel open). Suggestions are
re-ranked on every keystroke.

#### 2.5 Ontology Registry Integration

*   **Validation**: Before emitting `TagNode`, the UI checks `KnowledgeRegistry::validate(tag)`. Invalid tags (e.g. malformed UDC codes) show a warning or are rejected.
*   **Inference**: The suggestion list includes semantic matches from the registry via fuzzy search. Typing "calc" suggests "Calculus (udc:517)". Selecting this applies the `udc:517` tag.
*   **Visuals**: The registry can provide color hints for tags, which are reflected in the chip background.

#### Tasks

- [x] Expand `TagPanelState` beyond `{ node_key, text_input }` to cover icon-picker open state and pending icon override. A dedicated matcher/suggestion cache is still optional.
- [ ] Move panel rendering out of the temporary render-layer placement into a dedicated UI module when the broader GUI extraction settles (`shell/desktop/ui/tag_panel.rs` remains a good target).
- [ ] `T` key routing: open the panel through the input context stack or action routing. This remains a deliberate keybinding change because `T` is still bound to physics; do not silently steal the existing shortcut.
- [ ] Finish the original panel hosting contract: anchor it near the node rect and add the explicit `Esc` / outside-click close behavior from the spec.
- [x] `render_selected_node_tag_panel()`: a non-modal egui `Window` exists with chip row, text field, and suggestions.
- [x] On text field change: suggestions already re-rank against existing tags, display-only suggested tags, and `KnowledgeRegistry` search results. A reusable nucleo-backed matcher cache is not implemented yet.
- [x] On `Enter` / suggestion click: emit `GraphIntent::TagNode { key, tag }`.
- [x] On chip ✕ click: emit `GraphIntent::UntagNode { key, tag }`.
- [ ] Replace the current preset-row icon picker with the originally planned searchable emoji grid and optional Lucide tab.
- [x] On icon selection: associate icon with the pending tag in `TagPanelState`.
- [x] Persist icon choice through tag presentation metadata rather than a transient UI-only field.

#### Validation Tests

- `test_tag_panel_opens_on_t_key` — `T` key with node selected → `tag_panel_open == Some(key)`.
- `test_tag_panel_emits_tag_intent` — enter tag text, press Enter → `TagNode` intent emitted.
- `test_tag_panel_emits_untag_on_chip_remove` — click chip ✕ → `UntagNode` intent emitted.
- `test_nucleo_ranks_partial_match` — query "star" against ["#starred", "#work"] → "#starred"
  ranked higher.
- `test_tag_panel_closes_on_esc` — Esc key → `tag_panel_open == None`.

---

### Phase 3: Icon Resources

**Status note (2026-03-31)**: Still open except for the minimal preset-row emoji picker. Durable icon state is solved; the richer resource/search layer is not.

#### 3.1 Emoji (Primary)

Emoji are rendered via the system font — no asset bundling required. egui renders emoji as text
glyphs when the system font provides them (Windows 11: Segoe UI Emoji; macOS: Apple Color Emoji).

- No compile-time asset cost.
- Searchable via a static `EMOJI_NAMES: &[(&str, &str)]` list (slug → char), e.g.:
  `("star", "⭐")`, `("pin", "📌")`, `("bookmark", "🔖")`.
- The list can be generated from Unicode CLDR data or a curated subset (~500 entries covers
  common use).

#### 3.2 Lucide Icons (Extended)

[Lucide](https://lucide.dev) is an MIT-licensed icon set (~1500 SVG icons) derived from Feather.

Embedding strategy:

```rust
// In a generated icons.rs or icons/ module
pub static LUCIDE_BOOKMARK: &[u8] = include_bytes!("../assets/lucide/bookmark.svg");
pub static LUCIDE_TAG: &[u8] = include_bytes!("../assets/lucide/tag.svg");
// ... curated subset (~200 icons)
```

Rendering via egui `Image::from_bytes()` (egui 0.33 supports SVG via the `svg` feature of
`egui_extras`). Each icon is rasterized at the required size on first use and cached.

**Curated subset criteria:**
- Include: navigation, productivity, media, communication, and science categories.
- Exclude: brand/logo icons, deprecated/redundant aliases.
- Target: ~200 icons in the picker; full set available to advanced users via search.

#### 3.3 Default Tag Icons

Built-in tags have fixed default icons. These are the canonical glyphs shown in badges, the tag
assignment chip, and the icon picker's "system" section. Users cannot reassign system tag icons.

| Tag | Default icon | Badge variant | Notes |
| --- | --- | --- | --- |
| `#pin` | 📌 (Emoji) | `Pinned` | |
| `#starred` | ⭐ (Emoji) | `Starred` | |
| `#unread` | ● blue dot | `Unread` | Rendered as colored dot, not icon chip; auto-managed |
| `#archive` | 🗄 (Emoji) | `Tag` | Node also rendered at reduced opacity |
| `#resident` | 🏠 (Emoji) | `Tag` | Low-visibility; indicates lifecycle protection |
| `#private` | 🔒 (Emoji) | `Tag` | Triggers URL/title redaction in sharing mode |
| `#nohistory` | 🚫 (Emoji) | `Tag` | Behavioral only; badge shown in expanded orbit only |
| `#monitor` | 👁 (Emoji) | `Tag` | Pulses when content change detected |
| `#focus` | 🎯 (Emoji) | `Tag` | Renders with a soft highlight ring on the node |
| `#clip` | ✂️ (Emoji) | `Tag` | Distinct node shape/border in graph view |
| User tags | None (label chip) | `Tag` | User assigns icon via picker |

#### Tasks

- [ ] Create `assets/emoji_names.rs` or embed as a `const` slice in `badge.rs`:
  curated ~500 (slug, char) pairs.
- [ ] Download and add Lucide SVG curated subset to `assets/lucide/`. Add build step or
  `include_bytes!` references.
- [ ] Add `egui_extras` with `svg` feature to `Cargo.toml`.
- [ ] Implement `render_badge_icon(ui: &mut Ui, icon: &BadgeIcon, size: f32)` in `badge.rs`.
- [ ] Implement emoji search in icon picker: nucleo over `EMOJI_NAMES`.
- [ ] Implement Lucide search in icon picker: nucleo over icon slug list.

**Implementation caveat**: this phase depends on Phase 1.5. Without tag presentation metadata, user-selected icons have no durable home.

#### Validation Tests

- `test_emoji_icon_renders_without_panic` — `render_badge_icon` with `BadgeIcon::Emoji("⭐")` →
  no panic (headed test or mock `Ui`).
- `test_emoji_search_finds_star` — nucleo query "star" against emoji names → "⭐" in results.
- `test_lucide_slug_recognized` — `BadgeIcon::Lucide("bookmark")` → bytes available (non-empty).

---

## Phase 4: Rendering Context Integration (Added 2026-03-12)

Since this plan was written, the compositor architecture has advanced substantially. This section connects badge semantics to the two distinct rendering contexts where node visual state is expressed.

### 4.1 Two Rendering Contexts

Badge state (tags, lifecycle, Crashed, `#unread`) is rendered in two separate contexts that share the same semantic inputs but use different pipelines:

| Context | Where | Renderer | What it draws |
| --- | --- | --- | --- |
| **Canvas node overlay** | Graph canvas view | `GraphNodeShape::ui()` via egui | Badge orbit, at-rest badge circles, `#archive` opacity |
| **Tile-level overlay** | Workbench pane tile (Pass 3) | Compositor / `CompositorAdapter` | Lifecycle border treatment, focus/selection rings, recovery affordance badge |

These are not the same thing. A node's graph-canvas badge orbit shows user-applied tags at the canvas level. A tile's Pass 3 border shows lifecycle state at the workbench level. Both can be visible simultaneously for the same node (if the canvas and a node-pane tile are visible at once).

Chrome-scope tie-in: per `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md`,
graph-scope semantic filter chips belong in the graph-scoped Navigator host,
while pane runtime status (backend, degraded, blocked, loading) belongs in
workbench-scoped Navigator host rows/header. Node badges remain canvas/tab-level semantic output; they should
not drift into generic toolbar chrome except as explicitly derived status
signals.

### 4.2 `badges_for_node()` as Shared Semantic Resolver

`badges_for_node(node, workspace_count) -> Vec<Badge>` is the canonical semantic resolver for both contexts. The compositor's `TileSemanticOverlayInput` (see `../aspect_render/2026-03-12_compositor_expansion_plan.md` §2) draws from the same node state:

- `TileSemanticOverlayInput.lifecycle` ← same `NodeLifecycle` that drives `Badge::Crashed` / lifecycle border treatment
- `TileSemanticOverlayInput.runtime_blocked` ← same `RuntimeBlocked` condition that drives `Badge::Crashed` in slot 1
- `TileSemanticOverlayInput.has_unread_traversal_activity` ← relates to `Badge::Unread` (`#unread` tag)

**Invariant**: Semantic truth flows in one direction — graph/runtime state → both rendering contexts. Neither rendering context is authoritative over the other. A `Crashed` badge in the canvas view and a `RuntimeBlocked` recovery affordance in the tile are different visual expressions of the same underlying state, not redundant.

**Implementation note**: `badges_for_node()` should be callable from both `GraphNodeShape::ui()` and the compositor's Pass 3 setup path without duplicating the resolution logic. It belongs in a location accessible to both (e.g., `graph/badge.rs`) rather than buried inside a canvas-specific module.

**Current-state note**: the runtime currently has `badges_for_tags(...)` in `graph/badge.rs`, and both graph-canvas and tab-header code already consume it. The remaining gap is enriching that shared resolver with presentation metadata and any future tile-level semantic consumer.

### 4.3 `#focus` Tag vs. Navigation Focus

The `#focus` system tag (DOI boost, floats node toward canvas center) and the Focus subsystem's "navigation focus" (pane focus ring, `FocusDelta`, F6 region cycle) share the word "focus" but are distinct concepts:

| Concept | Tag/mechanism | Visual expression | Owner |
| --- | --- | --- | --- |
| **Semantic focus** | `#focus` tag (user-applied) | Soft highlight ring on canvas node; 🎯 badge in orbit | Graph/tag system |
| **Navigation focus** | Pane activation / keyboard focus | Focus ring on tile border (Pass 3, `FocusDelta`) | Focus subsystem / Render aspect |

These must not be conflated in implementation. A node can have the `#focus` tag (persistent user annotation) and separately receive navigation focus (transient input-routing state). Both may render simultaneously — the `#focus` canvas highlight ring and the tile border focus ring are orthogonal.

`TileSemanticOverlayInput.focus_delta` carries navigation focus transitions. The `#focus` tag is part of the node's tag set and would appear as a `Tag { icon: Emoji("🎯") }` badge in the canvas overlay — it does not feed into `FocusDelta`.

### 4.4 Compositor Cross-Reference

See `../aspect_render/2026-03-12_compositor_expansion_plan.md` for:

- O2 (lifecycle → tile border treatment) — tile-level expression of `NodeLifecycle`
- O5 (`FocusDelta`) — navigation focus ring contract
- §2 (`TileSemanticOverlayInput`) — shared semantic snapshot consumed by Pass 3
- O8 (`TileAffordanceAnnotation`) — what Pass 3 emits back to the accessibility layer

---

## Findings

### Why Emoji First

Emoji require zero asset bundling, ship with every OS graphshell targets, and are already rendered
by egui's font pipeline. The only cost is the static name list for search. Lucide extends coverage
for users who want a more uniform visual language, but it is an opt-in extension, not a
requirement.

### Orbit Animation and Reduced Motion

The orbit expansion animation adds visual continuity but must not be required for usability.
Badge identity (type + label) must be readable at rest (icon alone suffices for `#pin`, `#starred`,
`Crashed`; user tags fall back to colored dot at rest, full label on hover). The `prefers-reduced-
motion` OS setting (readable from `winit` on macOS/Windows) should skip the animation entirely and
jump to the expanded state.

### Tab-Level Pinning Distinction

The `tile.is_resident` workspace-tile property (prevents webview eviction) is deliberately NOT
represented as a badge. It is a lifecycle management setting, not a semantic annotation — it has
no meaning in the graph view, only in the tab bar. Users do not need to see "this tab is resident"
as a node property; they interact with it via the tab right-click menu.

### Badge State → AccessKit Mapping (O8)

The compositor expansion plan (O8, `TileAffordanceAnnotation`) establishes a path from Pass 3 draw output to the AccessKit bridge. Several badge states have direct AccessKit semantic equivalents that the bridge should map when `TileAffordanceAnnotation` is consumed:

| Badge / condition | AccessKit mapping | Notes |
| --- | --- | --- |
| `Crashed` / `RuntimeBlocked` | `aria-busy` or error state role | Recovery affordance badge → accessible status announcement |
| `Unread` (`#unread`) | `aria-live` region update hint | Badge clearing on activation should trigger a live region announcement |
| `#private` | Redacted label / `aria-hidden` on URL/title elements | When sharing mode is active; applies to accessible name computation |
| `Starred` (`#starred`) | `aria-label` suffix or `aria-describedby` hint | "Bookmarked" status annotation |
| `#archive` | `aria-hidden` when excluded from default view | Node effectively absent from default traversal |

This mapping is the responsibility of the canonical UX/accessibility projection layer consuming `TileAffordanceAnnotation`, not of the badge system itself and not of a direct compositor → AccessKit shortcut. The badge system's job is to resolve badge state correctly; the accessibility projection layer's job is to translate it.

### `#monitor` Requires a Dedicated Plan

`#monitor` is listed as a reserved tag but its implementation is substantially more complex than
the other system tags (which are pure read-time behaviors). It requires:

- A background scheduling mechanism (periodic wake-up independent of the webview lifecycle).
- A DOM hash comparison strategy (full-page hash vs. main-content-only heuristic).
- A throttle policy (minimum reload interval, backoff on repeated no-change).
- A notification path (badge pulse animation + toast on change detection).

This scope warrants a separate implementation plan before work begins. The tag name `#monitor` is
reserved now so data written before the plan exists is upgrade-compatible. Do not implement
`#monitor` behavioral effects as part of persistence hub Phase 1 — only reserve the constant.

### Nucleo Dependency

Nucleo is the fuzzy matcher used by the Helix editor. It is MIT-licensed, has no unsafe outside of
its SIMD hot path, and is a single-crate dependency. It is already considered for the omnibar
(UX polish plan §5.3). It is already present in the repository, so this plan does not need to add it as a new dependency.

### Current Implementation Snapshot

As of 2026-03-31, the following parts of this plan already exist in code:

- shared badge model and reserved tag constants in `model/graph/badge.rs`
- presentation metadata on the node model plus reducer and persistence plumbing
- graph-node at-rest badges, archive dimming, `#clip` dashed ring, and hover/selection orbit expansion in `model/graph/egui_adapter.rs`
- tab-header badge suffixes in `shell/desktop/workbench/tile_behavior/tab_chrome.rs`
- reducer-side reserved-tag normalization, node-owned tag truth, and metadata pruning
- a non-modal tag editor reachable from the Selected Node inspector and graph toolbar, with a minimal preset-row emoji picker and durable user-tag icon overrides

Still missing:

- deliberate keyboard and context-menu routing (`T` and graph-context `Tags…`)
- panel anchoring/extraction and the original close semantics (`Esc`, outside click)
- searchable emoji catalog and optional Lucide asset/dependency path
- explicit UI-path coverage for open/close/add/remove/icon-persistence flows

---

## Progress

### 2026-02-20 — Session 1

- Plan created from the unified tag system design discussion.
- Badge visual system (orbit model, priority slots, tab header badges) designed.
- Tag assignment UI (floating panel, nucleo autocomplete, icon picker) designed.
- Icon system: emoji (primary, zero cost) + Lucide (extended, SVG, MIT) decided.
- Implementation not started.

### 2026-03-12 — Architecture update

- Added §4 "Rendering Context Integration": two-context model (canvas node overlay vs. tile-level Pass 3), `badges_for_node()` as shared resolver invariant, cross-reference to compositor expansion plan.
- Added §4.3 semantic focus (`#focus` tag) vs. navigation focus (Focus subsystem / `FocusDelta`) naming disambiguation.
- Updated Phase 1 and Phase 2 task file references to reflect post-GUI-decomposition module paths (`gui_update_coordinator.rs`, `gui_orchestration.rs`); tag panel moves to `shell/desktop/ui/tag_panel.rs`.
- Updated `T` key task to route through `InputRegistry`/`ActionRegistry` (not hardcoded `KeyboardActions`).
- Added §1.4.1 `#clip` node visual treatment table (dashed border, no shape change, full opacity).
- Added "Badge State → AccessKit Mapping (O8)" finding with `Crashed`/`Unread`/`#private`/`#starred`/`#archive` AccessKit semantic equivalents.
- Corrected plan status and prerequisite language to match current runtime storage (`workspace.semantic_tags`, not tags embedded directly on `Node`).
- Added Phase 1.5 tag presentation metadata so durable user-tag icon choice and ordering have an explicit architectural home.
- Added Phase 1.6 canonical tag ownership migration, including the temporary dual-write reducer bridge, read-path migration, semantic-index rebuild changes, and explicit removal of `workspace.semantic_tags`.
- Added a current implementation snapshot to distinguish landed work from remaining slices.

### 2026-03-31 — Reality-check update

- Audited the plan against the current codebase and marked the broad badge model, presentation metadata, canonical tag ownership migration, graph render path, orbit expansion, and tab-header suffix work as landed.
- Updated Phase 2 to reflect the current tag panel reality: inspector and toolbar entry points exist, add/remove works, suggestions work, and icon overrides persist, but the deliberate trigger and close semantics remain open.
- Narrowed Phase 3 to the remaining resource/search work rather than durable icon persistence, which is already solved.
- Extracted the actual remainder into `2026-03-31_node_badge_and_tagging_follow_on_plan.md` so this file can remain a historical plan plus landed-scope record instead of a stale catch-all.
