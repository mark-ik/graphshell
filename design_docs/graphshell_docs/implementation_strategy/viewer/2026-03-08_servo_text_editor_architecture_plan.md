# Servo-Backed Text Editor Architecture Plan

**Date**: 2026-03-08
**Status**: Design / Planning
**Scope**: Define a concrete architecture for a `viewer:text-editor` implementation that uses Servo as the presentation surface while keeping editing semantics in a Rust editor core.

**Related docs**:

- `2026-02-24_universal_content_model_plan.md` — Viewer selection and content model alignment
- `2026-02-23_wry_integration_strategy.md` — Viewer trait integration model and backend boundaries
- `../aspect_render/2026-02-20_embedder_decomposition_plan.md` — Runtime/lifecycle decomposition baseline
- `../../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md` — Core extraction constraints and wasm-clean principles
- `../../../TERMINOLOGY.md` — Canonical terms and boundary definitions

---

## 1. Problem Statement

A text editor viewer is needed for local-first node content editing. The key architectural question is whether Servo should be the editor itself, or a rendering and input surface backed by a dedicated editor core.

This plan selects the second option:

- Servo handles rendering and input capture.
- A Rust `editor-core` crate owns text state, transactions, undo/redo, parsing, and diagnostics.

This split preserves deterministic behavior, testability, and long-term reuse across headless, desktop, and WASM targets.

---

## 2. Core Decision

**Servo is a Surface, not the Semantic Authority.**

Servo renders the editor UI via custom DOM overlays and positioned elements. It does **not** use `contenteditable` for edit semantics — Servo's `contenteditable` implementation is incomplete and should not be relied upon as a correctness boundary. All edit state lives in `editor-core`.

The surface constructs a rendered view (cursor, selection highlight, visible text lines, diagnostics underlines) from `editor-core` state. User input events (keystrokes, pointer events, IME composition) are translated into typed `CoreRequest` values and dispatched to the core. The surface never mutates document state directly.

### Why this split

1. Deterministic edit semantics independent of browser/runtime quirks.
2. Reusable engine for headless tests and collaborative/server-side operation.
3. Clean migration path between UI surfaces (Servo, native, or hybrid) without rewriting text semantics.
4. Sound integration with Graphshell intent/reducer boundaries via `GraphIntent::EditNodeText`.

---

## 3. Crate Layout

Two mandatory crates; one deferred until a second consumer exists:

```text
crates/
  editor-core/          ← text authority, WASM-clean
    src/
      lib.rs
      ids.rs
      text/{mod.rs, buffer.rs, position.rs, range.rs}
      model/{mod.rs, document.rs, cursor.rs, selection.rs, viewport.rs}
      op/{mod.rs, edit.rs, transform.rs}
      transaction/{mod.rs, txn.rs, undo.rs}
      composition.rs    ← IME composition session (see §4.3)
      syntax/{mod.rs, parse.rs, highlight.rs}
      diagnostics/
      search/
      tests/{property.rs, replay.rs}

  editor-host/          ← desktop I/O, not WASM
    src/
      lib.rs
      file_io.rs
      workspace.rs
      lsp.rs
      plugins.rs
      bridge.rs
```

`editor-servo-surface` is a module inside the graphshell desktop crate, not a standalone crate, because it has exactly one consumer (the Graphshell Servo compositor path) and shares lifecycle with the viewer registration system.

`editor-protocol` is a **deferred boundary**. The public API of `editor-core` is serde-friendly from day one. Extract `editor-protocol` as a separate crate only when a second surface implementation (e.g. native egui surface, WASM browser host) requires a shared wire format.

---

## 4. Boundaries and Responsibilities

### 4.1 `editor-core` (authoritative)

Owns:

- Document text storage and revisions
- Cursor and selection semantics
- Edit transactions and undo/redo
- IME composition session state (see §4.3)
- Incremental parse/highlight model
- Diagnostics and search index outputs

Does not know about:

- Servo, egui, wgpu
- Filesystem or LSP process I/O
- Graphshell node identity (receives opaque `DocId` assigned by host)

### 4.2 Servo surface module

Owns:

- Rendering projection of core state into DOM
- Input event translation (keyboard, pointer) into `CoreRequest`
- IME windowing events forwarded as `CompositionEvent` variants (see §4.3)
- Clipboard integration
- Viewport scroll/resize requests

Does not own:

- Canonical text state
- Undo stack
- Parse diagnostics
- Composition string (owned by `editor-core`)

### 4.3 IME Composition — First-Class Concern

IME composition must be modeled in `editor-core` from the initial implementation. It is not an edge case.

```rust
// editor-core/src/composition.rs

pub struct CompositionSession {
    /// Anchor byte offset in the rope where the composition started.
    pub anchor: usize,
    /// The in-progress pre-edit string from the platform IME.
    pub preedit: String,
    /// Highlighted ranges within the preedit (platform-supplied).
    pub highlights: Vec<(usize, usize)>,
}

pub enum CompositionEvent {
    /// IME session opened; anchor is current cursor byte offset.
    Start { anchor: usize },
    /// Platform updated the preedit string and/or highlights.
    Update { preedit: String, highlights: Vec<(usize, usize)> },
    /// User confirmed; commit string replaces the preedit span.
    Commit { text: String },
    /// IME cancelled (e.g. Escape); preedit is discarded.
    Cancel,
}
```

Rules:

1. While a `CompositionSession` is active, no keystroke edit ops are applied to the rope. Only composition events mutate the provisional preedit.
2. On `Commit`, the preedit span is replaced atomically by the committed string as a single `Transaction`. This makes undo of IME input coherent.
3. On `Cancel`, the preedit span is removed and the rope returns to the anchor state.
4. The surface renders the preedit string at the anchor position with platform-appropriate underline decoration, sourced from `editor-core` state — the surface does not maintain a parallel preedit buffer.

### 4.4 `editor-host`

Owns:

- File read/write
- LSP process management
- Workspace/session persistence
- Plugin policy and security boundary

---

## 5. Request / Event Contract

All surface-to-core and core-to-surface communication is typed. These types live in `editor-core::api` (not a separate crate yet — see §3).

### 5.1 Requests (surface → core)

```rust
pub enum CoreRequest {
    Open { doc_id: DocId, text: String, syntax_hint: Option<String> },
    Apply(Transaction),
    MoveCursor { doc_id: DocId, to: Position },
    SetSelection { doc_id: DocId, ranges: Vec<TextRange> },
    Composition(CompositionEvent),
    Reparse { doc_id: DocId },
    Search { doc_id: DocId, query: String },
    Resync { doc_id: DocId },  // surface requests full state dump after desync
}
```

### 5.2 Events (core → surface)

```rust
pub enum CoreEvent {
    Applied { doc_id: DocId, revision: u64 },
    TextPatch { doc_id: DocId, from_rev: u64, to_rev: u64, hunks: Vec<PatchHunk> },
    CursorChanged { doc_id: DocId, cursors: Vec<Position> },
    SelectionChanged { doc_id: DocId, ranges: Vec<TextRange> },
    CompositionState { doc_id: DocId, session: Option<CompositionSession> },
    DiagnosticsChanged { doc_id: DocId, items: Vec<Diagnostic> },
    HighlightsChanged { doc_id: DocId, spans: Vec<HighlightSpan> },
    FullState { doc_id: DocId, revision: u64, text: String },  // resync response
}
```

Invariant: the surface never mutates document state directly. Revision IDs on every patch/event allow the surface to detect desync and issue `Resync`.

---

## 6. Required Capabilities

Minimum `editor-core` requirements for Phase A gate:

1. Rope or equivalent large-text storage
2. Revisioned transaction model with explicit from/to revision on every patch
3. Undo/redo grouping and replayability
4. Unicode-safe cursor movement and deletion (grapheme-cluster boundaries)
5. IME-safe composition session (§4.3) — not deferred
6. Incremental parse/highlight update path
7. Viewport virtualization for large files (visible line window only)
8. Diagnostics/decorations channel
9. Deterministic replay tests and property tests for transaction invariants

---

## 7. Crate Selections

### Mandatory

| Crate | Role |
|-------|------|
| `ropey` | Text storage — O(log n) insert/delete, byte/char/line indexing |
| `unicode-segmentation` | Grapheme cluster iteration for cursor movement and deletion |
| `tree-sitter` + `tree-sitter-highlight` | Incremental syntax parsing and highlight span generation |
| `regex-automata` + `aho-corasick` | Fast literal and pattern search |
| `slotmap` | Stable internal IDs for documents, cursors, diagnostics |
| `rkyv` + `serde_json` | Binary snapshots/journal payloads use `rkyv`; human-readable settings, sync envelopes, and interoperability payloads use `serde_json` |
| `lsp-types` | Diagnostic and position structs shared with LSP host bridge |
| `proptest` | Property-based transaction invariant testing for editor transactions |
| `insta` | Snapshot regression testing for deterministic `CoreEvent` streams |

### Optional (Phase C+)

| Crate | Role |
|-------|------|
| `yrs` | CRDT text replication for collaborative editing — preferred over `automerge` because `yrs` provides a rope-like text type (`yrs::Text`) that aligns naturally with `ropey` at transaction boundaries, and has an active Rust-first maintenance path |

### WASM portability

`editor-core` must compile to `wasm32-unknown-unknown`. All mandatory crates above satisfy this constraint. `editor-host` (file I/O, LSP process) does not target WASM and carries no WASM constraint.

---

## 8. Graphshell-Core Integration

The editor integrates with `graphshell-core` at two points:

### 8.1 Node content mutations via `GraphIntent`

When the user saves or auto-saves an edited node:

```rust
// graphshell-core/src/intent.rs (illustrative)
pub enum GraphIntent {
    // ...existing variants...
    EditNodeText {
        node_id: NodeId,
        revision: u64,
        patch: Vec<PatchHunk>,
    },
    CommitNodeText {
        node_id: NodeId,
        revision: u64,
        full_text: String,
    },
}
```

`apply_intents()` records the mutation in the WAL and updates the node content hash. The editor surface dispatches these intents; it does not write node content directly.

### 8.2 `DocId` to `NodeId` mapping

The host (desktop crate) is responsible for mapping `DocId` (editor scope) to `NodeId` (graph scope). `editor-core` never sees `NodeId`. This preserves the WASM-clean boundary of `editor-core`.

---

## 9. Viewer Registration

Add a new viewer implementation:

- `viewer:text-editor`

Selection rule (initial):

- `mime_hint` in the text family (`text/*`, selected code and document formats) routes to
  `viewer:text-editor` when the node is opened with **edit intent** (see below). `viewer:plaintext`
  remains the read-only display path for the same MIME types.
- Explicit `viewer_id_override` remains highest priority.
- Fallback: `viewer:plaintext` (read-only) then `viewer:webview` per existing policy.

**Edit intent** (canonical definition, shared with `universal_content_model_spec.md §4.2`):

- The node was created as a new local text file (`address_kind = File`, `mime_hint` in `text/*`) with no prior content — edit mode is the natural first-open state for a blank file.
- The user explicitly invoked `action:node.edit` (command palette or node context menu) on a node whose active viewer is `PlaintextViewer` or `FallbackViewer`.
- `viewer_override` is explicitly set to `viewer:text-editor`.

Edit intent is **never** inferred from MIME type alone on an existing file. Default open of a local text file via `DirectoryViewer` click, link navigation, or node activation uses read-only `PlaintextViewer`. The user must take an explicit action to enter edit mode.

**Syntax highlighting split**: `editor-core` uses `tree-sitter` + `tree-sitter-highlight` for
incremental parse/highlight (desktop-only; `tree-sitter` does not compile to
`wasm32-unknown-unknown`). `viewer:plaintext` uses `syntect` with `fancy-regex` feature for
read-only display (WASM-portable). The two stacks coexist by design — different portability
targets, different fidelity levels. See `2026-02-24_universal_content_model_plan.md` Step 3.

**Adaptation pipeline exemption**: The Step 12 `SimpleDocument` adaptation pipeline in the UCM
plan short-circuits to `viewer:text-editor` for editable `text/*` + `File` nodes and never
routes them through `EngineTarget::ServoHtml`. Editing semantics are owned by `editor-core`.

This viewer renders in embedded mode (`render_embedded = true`, `is_overlay_mode = false`).

---

## 10. Implementation Phases

### Phase A: Core MVP

Deliver:

1. `DocId`, `Position`, `TextRange`, `EditOp`, `Transaction`
2. Rope-backed buffer via `ropey`
3. `apply(transaction)`, `undo()`, `redo()`
4. `CompositionSession` state machine (§4.3)
5. Deterministic replay and property tests

Gate: property/replay tests pass for transaction invariants; composition session round-trips without rope corruption.
Required wiring: `editor-core` test targets include replay and invariant coverage, aligned with Graphshell diagnostics test policy in `../subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` §6.1.

### Phase B: Servo Surface MVP

Deliver:

1. Input bridge: keyboard/pointer events → `CoreRequest`
2. IME windowing events → `CompositionEvent` dispatch
3. Patch-based DOM rendering updates
4. Viewport line-window rendering for large files

Gate: sustained typing and IME input without desync between surface revision and core revision.

### Phase C: Language Features

Deliver:

1. Incremental syntax highlighting via `tree-sitter`
2. Diagnostics overlays
3. Search results and navigation
4. LSP host bridge in `editor-host`

Gate: incremental parse updates and diagnostics remain responsive on files ≥ 100 KB.

### Phase D: Graphshell Node Integration

Deliver:

1. `viewer:text-editor` registration
2. Node-open lifecycle wiring (`DocId` ↔ `NodeId` mapping in host)
3. `GraphIntent::EditNodeText` / `CommitNodeText` dispatch on save
4. Persistence integration with session/workspace model
5. Diagnostics channels for editor latency and patch size

Gate: node editor works inside workbench tile lifecycle with no reducer boundary violations.

---

## 11. Risks and Mitigations

1. **IME/input edge cases across platforms** — Mitigation: `CompositionSession` in `editor-core` from day one; platform-specific integration tests for each IME path.

2. **Large-file performance regressions** — Mitigation: viewport virtualization (Phase A); incremental update metrics surfaced as diagnostics channels.

3. **Surface/core desynchronization** — Mitigation: revision IDs on every patch/event; explicit `Resync` request with full-state response.

4. **`contenteditable` drift** — Mitigation: do not use `contenteditable` for edit semantics; DOM overlays only.

5. **Scope creep into full IDE behavior** — Mitigation: Phase A/B gate strictly on text editing and IME; language features are Phase C with explicit gate.

---

## 12. Acceptance Criteria

1. `viewer:text-editor` can open and edit text nodes in a workbench tile.
2. All edits flow through `editor-core` transactions; no direct DOM mutation of document state.
3. Undo/redo behavior is deterministic under replay tests.
4. IME composition commits and cancels are handled correctly without rope corruption.
5. Surface can recover from revision mismatch via explicit `Resync`.
6. Large-file scrolling remains responsive via viewport virtualization.
7. Syntax highlighting and diagnostics update incrementally.
8. No Servo/egui types appear in `editor-core` public API.
9. `editor-core` compiles to `wasm32-unknown-unknown` with no feature flags.

---

## 13. Immediate Next Step

Create a small `editor-core` spike crate with:

1. `DocId`, `Position`, `TextRange`, `EditOp`, `Transaction`
2. Rope-backed buffer (`ropey`)
3. `apply(transaction)`, `undo()`, `redo()`
4. `CompositionSession` state machine
5. Property and replay tests
6. Baseline replay/invariant scaffolding (`tests/proptest_*` + `tests/snapshots/*`) aligned with Graphshell diagnostics test policy

Establish the semantic authority and composition contract before UI implementation begins.
