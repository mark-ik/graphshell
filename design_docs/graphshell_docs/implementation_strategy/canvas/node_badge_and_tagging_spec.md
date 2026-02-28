# Node Badge and Tagging — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Implementation-ready

**Related**:

- `CANVAS.md`
- `graph_node_edge_interaction_spec.md`
- `2026-02-20_node_badge_and_tagging_plan.md`
- `2026-02-23_udc_semantic_tagging_plan.md`
- `../system/register/knowledge_registry_spec.md`
- `../system/register/action_registry_spec.md`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Tag data model** — reserved system tags and their behavioral effects.
2. **Badge visual system** — how tags are rendered at rest and on hover/focus.
3. **Tag assignment surface** — the floating panel and icon picker.
4. **Knowledge Registry integration** — validation, inference, and semantic search.

---

## 2. Tag Namespace Contract

Tags are `String` values stored on `Node`. The `#` prefix is the **reserved system namespace**. Tags beginning with `#` may carry behavioral effects. User-defined tags without `#` are purely organizational and carry no system behavior.

### 2.1 Reserved System Tags

| Tag | Owner system | Behavioral effect |
|-----|-------------|-------------------|
| `#pin` | Physics simulation | Node is a physics anchor; not displaced by simulation |
| `#starred` | Search / omnibar | Soft bookmark; surfaces via `@b` scope and `is:starred` predicate |
| `#archive` | Graph view | Hidden from default graph view; node rendered at reduced opacity when shown; excluded from default queries |
| `#resident` | Lifecycle / cache | Never cold-evicted regardless of workbench state |
| `#private` | Rendering / export | URL and title redacted in overlays when sharing mode is active; excluded from JSON export |
| `#nohistory` | Traversal recording | Navigation through this node does not push a traversal entry |
| `#monitor` | Background scheduler | Periodically reload and compare DOM hash; badge/toast on content change (**behavioral implementation deferred; tag is reserved**) |
| `#unread` | Badge / DOI | Auto-applied on node creation or URL change; cleared on first node activation; not user-assignable |
| `#focus` | DOI weight | Boosts degree-of-interest score; floats node toward center in layout |
| `#clip` | Node type | Node is a clipped DOM element; distinct node shape/border in graph view |

**Forward-compatibility rule**: A `#`-prefixed tag not in the reserved list is accepted as user-defined and carries no system effect. The tag assignment UI warns on unrecognized `#` prefixes but does not block. Future versions may upgrade an inert `#foo` to reserved status without breaking existing data.

**`#unread` invariant**: `#unread` is system-managed only. It must not be user-assignable through the tag panel. It is removed automatically on first node activation via the lifecycle intent path, not by user action.

**`#monitor` status**: The tag constant is reserved. Behavioral implementation (background scheduler, DOM hash comparison, notification path) requires a separate plan before any implementation begins.

---

## 3. Badge Visual Contract

### 3.1 Badge Types

```
Badge =
  | Crashed
  | WorkspaceCount(usize)
  | Pinned
  | Starred
  | Unread
  | Tag { label: String, icon: BadgeIcon }

BadgeIcon =
  | Emoji(String)
  | Lucide(&'static str)
  | None              -- label-only chip
```

### 3.2 Priority Order

When space is constrained, badges are hidden from lowest to highest priority:

1. `Crashed` — always visible; overrides slot 1
2. `WorkspaceCount` — shown when node belongs to 2+ workspaces
3. `Pinned`
4. `Starred`
5. `Unread` — rendered as a colored dot, not an icon chip
6. Other system tags (`#focus`, `#monitor`, `#private`, `#archive`, `#resident`, `#nohistory`) — ordered by tag insertion order
7. UDC semantic tags (`udc:51`) — rendered as label chips
8. User-defined tags — ordered by insertion order

### 3.3 At-Rest Display (Graph View)

- Up to **3** badges rendered as small icon-only circles (16×16 dp) at the node's top-right corner.
- If more than 3 badges exist, the third slot shows a `+N` overflow chip.
- `Crashed`: red ⚠ glyph; always occupies slot 1.
- `WorkspaceCount`: numeric chip (e.g. `2`).
- `#archive` nodes: rendered at reduced opacity (0.35–0.45) when "Show archived" is on; excluded entirely when off.

**All interactive elements must meet a 32×32 dp minimum hit target** when tapped or clicked.

### 3.4 Hover/Focus Expansion (Orbit Model)

When the cursor enters a node or the node receives keyboard focus:

- All badges expand from the at-rest corner and orbit the node periphery.
- Animation: 120 ms ease-out. Must respect `prefers-reduced-motion` — if set, jump to expanded state instantly with no transition.
- Expanded badges show both icon and label (truncated at 12 characters).
- Orbit radius scales with node size; minimum 32 dp from node center.
- Badges are non-interactive in orbit. Clicking any badge area opens the tag assignment panel (same as `T` key).

**Accessibility**: Expanded badge labels must be accessible to screen readers. Each badge's label is announced as an accessible name on the expanded chip element.

### 3.5 Tab Header Badges

In the workbench tab bar, each tab header shows a compact badge row to the right of the title:

- At rest: icon-only, up to 2 badges (`Crashed` + one more).
- `Crashed`: red dot suffix.
- `#starred`: ⭐; `#pin`: 📌.
- User tags: first tag icon only (no overflow in narrow tab headers).

### 3.6 Default Tag Icons

System tags have fixed, non-user-reassignable icons:

| Tag | Default icon |
|-----|-------------|
| `#pin` | 📌 |
| `#starred` | ⭐ |
| `#unread` | Blue dot (not an icon chip) |
| `#archive` | 🗄 |
| `#resident` | 🏠 |
| `#private` | 🔒 |
| `#nohistory` | 🚫 |
| `#monitor` | 👁 |
| `#focus` | 🎯 |
| `#clip` | ✂️ |
| User tags | None by default; user assigns via icon picker |

---

## 4. Tag Assignment Surface Contract

### 4.1 Trigger Conditions

The tag assignment panel opens when:

- `T` key is pressed with a node selected in graph view
- Right-click context menu → "Tags…" in graph view
- A badge chip in the expanded orbit is clicked

In detail/node view, `T` targets the focused tab's node.

### 4.2 Panel Behavior Contract

The panel is non-modal and anchored near the node (below or to the right, whichever fits in the viewport).

| Interaction | Required behavior |
|-------------|------------------|
| Chip ✕ click | Emit `UntagNode { key, tag }` intent immediately |
| Text input change | Re-rank suggestions via nucleo fuzzy match against `tag_index` + `KnowledgeRegistry::search()` |
| `Enter` or suggestion click | Validate via `KnowledgeRegistry::validate(tag)`, then emit `TagNode { key, tag }` |
| `Esc` | Close panel without changes |
| Click outside / node deselected | Close panel without changes |
| `#` prefix on unknown tag | Show warning indicator; allow submission |
| Invalid UDC code | Show validation error from `KnowledgeRegistry`; block submission |

**Autocomplete contract**:
- Minimum suggestion delay: none (re-rank on every keystroke).
- Maximum suggestions shown: 5.
- Suggestion sources (combined, ranked by score): existing `tag_index` keys, `KnowledgeRegistry::search()` results, static emoji name list.
- UDC match display format: `"Calculus (udc:517)"`.

### 4.3 Icon Picker Contract

Accessible via the `[⊞]` button in the tag panel or by clicking the icon slot of an existing user tag chip.

- Two tabs: **Emoji** (default) and **Lucide**.
- Search field: nucleo against emoji names (Emoji tab) or icon slugs (Lucide tab).
- Preview: selecting an icon updates the pending tag chip in the parent panel before confirmation.
- `Cancel`: dismiss picker; no change to pending tag.
- `Select`: confirm icon selection; update pending tag's icon.

---

## 5. Icon Resource Contract

### 5.1 Emoji (Primary)

- Rendered via system font (no asset bundling).
- A static `EMOJI_NAMES: &[(&str, &str)]` list (slug → char) provides search capability (~500 curated entries).
- Search: nucleo fuzzy match over slug list.

### 5.2 Lucide Icons (Extended)

- Curated subset of ~200 icons from the Lucide MIT-licensed set.
- Embedded via `include_bytes!` in a generated `icons.rs` module.
- Rendered via `egui_extras` SVG support; rasterized at required size on first use and cached.
- Lucide tab in icon picker; slug-based search.

---

## 6. Knowledge Registry Integration Contract

- `KnowledgeRegistry::validate(tag: &str) -> ValidationResult` — called before emitting any `TagNode` intent. Must run synchronously.
- `KnowledgeRegistry::search(query: &str) -> Vec<TagSuggestion>` — called on every tag panel keystroke. Must return within one frame budget.
- `KnowledgeRegistry` may provide `color_hint: Option<Color32>` for tag chips. The tag panel uses this as the chip background color if present.
- The panel does not call `KnowledgeRegistry` directly from the reducer; it calls from the UI layer and routes mutations through intents.

---

## 7. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Reserved system tags listed in §2.1 all render their canonical badge | Test: `badges_for_node` returns correct badge type for each reserved tag |
| Badge priority order matches §3.2 | Test: nodes with multiple tags produce badges in defined order |
| At-rest capped at 3 + overflow chip | Test: node with 5 badges → 3 shown + `+2` chip |
| `Crashed` always occupies slot 1 | Test: `Crashed + Pinned` → Crashed first |
| `#unread` is not user-assignable via tag panel | Test: tag panel does not surface `#unread` in suggestions; `TagNode { tag: "#unread" }` from panel is blocked |
| `T` key opens panel for selected node | Test: `T` key with selection → `tag_panel_open == Some(key)` |
| `Esc` closes panel without change | Test: Esc → panel closes, no intents emitted |
| `Enter` emits `TagNode` | Test: type tag + Enter → `TagNode` intent in intent queue |
| Chip ✕ emits `UntagNode` | Test: click ✕ on chip → `UntagNode` intent in intent queue |
| `prefers-reduced-motion` disables animation | Test: when motion preference is set, badge expansion is instant |
| `KnowledgeRegistry::validate` blocks invalid UDC | Test: invalid UDC code → submission blocked, error shown |
