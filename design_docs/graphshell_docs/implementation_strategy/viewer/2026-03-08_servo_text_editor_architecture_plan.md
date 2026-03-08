# Servo-Backed Text Editor Architecture Plan

**Date**: 2026-03-08
**Status**: Design / Planning
**Scope**: Define a concrete architecture for a `viewer:text-editor` implementation that uses Servo as the presentation surface while keeping editing semantics in a Rust editor core.

**Related docs**:

- `2026-02-24_universal_content_model_plan.md` - Viewer selection and content model alignment
- `2026-02-23_wry_integration_strategy.md` - Viewer trait integration model and backend boundaries
- `../aspect_render/2026-02-20_embedder_decomposition_plan.md` - Runtime/lifecycle decomposition baseline
- `../../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md` - Core extraction constraints and wasm-clean principles
- `../../../TERMINOLOGY.md` - Canonical terms and boundary definitions

---

## 1. Problem Statement

A text editor viewer is needed for local-first node content editing. The key architectural question is whether Servo should be the editor itself, or a rendering/input surface backed by a dedicated editor core.

This plan selects the second option:

- Servo handles rendering and input capture.
- A Rust `editor-core` crate owns text state, transactions, undo/redo, parsing, and diagnostics.

This split preserves deterministic behavior, testability, and long-term reuse for headless, desktop, and wasm targets.

---

## 2. Core Decision

**Servo is a Surface, not the Semantic Authority.**

Servo can render rich editor UI quickly (`contenteditable`, custom canvas/DOM, overlays), but editor correctness must live in a platform-agnostic core.

### Why this split

1. Deterministic edit semantics independent of browser/runtime quirks.
2. Reusable engine for headless tests and potential server-side/collaborative operation.
3. Easier migration between UI surfaces (Servo, native, or hybrid) without rewriting text semantics.
4. Clean integration with Graphshell intent/reducer boundaries.

---

## 3. Crate Layout

Proposed workspace crates:

1. `editor-core`
2. `editor-protocol`
3. `editor-host`
4. `editor-servo-surface`

```text
crates/
  editor-core/
    src/
      lib.rs
      ids.rs
      text/{mod.rs, buffer.rs, position.rs, range.rs}
      model/{mod.rs, document.rs, cursor.rs, selection.rs, viewport.rs}
      op/{mod.rs, edit.rs, transform.rs}
      transaction/{mod.rs, txn.rs, undo.rs}
      syntax/{mod.rs, parse.rs, highlight.rs}
      diagnostics/{mod.rs}
      search/{mod.rs}
      tests/{property.rs, replay.rs}

  editor-protocol/
    src/
      lib.rs
      request.rs
      event.rs
      patch.rs

  editor-host/
    src/
      lib.rs
      file_io.rs
      workspace.rs
      lsp.rs
      plugins.rs
      bridge.rs

  editor-servo-surface/
    src/
      lib.rs
      input_bridge.rs
      ime.rs
      dom_patch.rs
      clipboard.rs
      viewport.rs
```

---

## 4. Boundaries and Responsibilities

### 4.1 `editor-core` (authoritative)

Owns:

- Document text storage and revisions
- Cursor and selection semantics
- Edit transactions and undo/redo
- Incremental parse/highlight model
- Diagnostics and search index outputs

Does not know about:

- Servo
- egui
- wgpu
- filesystem
- LSP process I/O

### 4.2 `editor-servo-surface`

Owns:

- Rendering projection of core state
- Input event capture (keyboard, pointer, IME composition)
- Clipboard integration hooks
- Viewport requests

Does not own:

- Canonical text state
- Undo stack
- Parse diagnostics

### 4.3 `editor-host`

Owns:

- File read/write
- LSP process management
- Workspace/session persistence
- Plugin policy and security boundary

---

## 5. Protocol Contract

All surface-core communication is typed via `editor-protocol`.

### 5.1 Requests (surface -> core)

```rust
pub enum CoreRequest {
    Open { doc_id: DocId, text: String },
    Apply(Transaction),
    MoveCursor { doc_id: DocId, to: Position },
    SetSelection { doc_id: DocId, ranges: Vec<TextRange> },
    Reparse { doc_id: DocId },
    Search { doc_id: DocId, query: String },
}
```

### 5.2 Events (core -> surface)

```rust
pub enum CoreEvent {
    Applied { doc_id: DocId, revision: u64 },
    TextPatch { doc_id: DocId, from: u64, to: u64, hunks: Vec<PatchHunk> },
    CursorChanged { doc_id: DocId, cursors: Vec<Position> },
    SelectionChanged { doc_id: DocId, ranges: Vec<TextRange> },
    DiagnosticsChanged { doc_id: DocId, items: Vec<Diagnostic> },
    HighlightsChanged { doc_id: DocId, spans: Vec<HighlightSpan> },
}
```

Invariant: the surface never mutates document state directly.

---

## 6. Required Capabilities

Minimum editor-core requirements:

1. Rope or equivalent large-text storage
2. Revisioned transaction model
3. Undo/redo grouping and replayability
4. Unicode-safe cursor movement and deletion
5. IME-safe composition handling contract
6. Incremental parse/highlight update path
7. Viewport virtualization for large files
8. Diagnostics/decorations channel
9. Deterministic replay/property tests

---

## 7. Crate Recommendations

Recommended baseline:

1. `ropey` - text storage
2. `unicode-segmentation` - grapheme operations
3. `tree-sitter` and `tree-sitter-highlight` - incremental syntax parsing/highlighting
4. `lsp-types` - protocol structs
5. `regex-automata` and `aho-corasick` - fast search
6. `slotmap` - stable internal IDs
7. `serde` - state snapshots and replay fixtures

Optional collaboration:

1. `yrs` or `automerge` for CRDT replication

### Selection note

Servo is a good rendering surface for Graphshell because the compositor and viewer integration path already exists. It should not replace the editor core.

---

## 8. Integration with ViewerRegistry

Add a new viewer implementation:

- `viewer:text-editor`

Selection rule (initial):

- `mime_hint` in text family (`text/*`, selected code/document formats) routes to `viewer:text-editor`
- explicit `viewer_id_override` remains highest priority
- fallback remains `viewer:plaintext` then `viewer:webview` per existing policy

This viewer renders in embedded mode (`render_embedded = true`, `is_overlay_mode = false`).

---

## 9. Implementation Phases

### Phase A: Core MVP

Deliver:

1. single-document open/edit
2. insert/delete/replace transactions
3. undo/redo
4. cursor + selection model
5. deterministic replay tests

Gate:

- property/replay tests pass for transaction invariants

### Phase B: Servo Surface MVP

Deliver:

1. input bridge to typed requests
2. patch-based rendering updates
3. IME composition event handling
4. viewport line-window rendering

Gate:

- sustained typing/editing without desync between surface and core revisions

### Phase C: Language Features

Deliver:

1. incremental syntax highlighting
2. diagnostics overlays
3. search results and navigation
4. LSP host bridge

Gate:

- incremental parse updates and diagnostics remain responsive on large files

### Phase D: Graphshell Node Integration

Deliver:

1. `viewer:text-editor` registration
2. node-open lifecycle wiring
3. persistence integration with session/workspace model
4. diagnostics channels for editor latency and patch size

Gate:

- node editor works inside workbench tile lifecycle with no reducer boundary violations

---

## 10. Risks and Mitigations

1. IME/input edge cases across platforms
- Mitigation: strict request/event contract plus platform-specific integration tests

2. Large-file performance regressions
- Mitigation: viewport virtualization and incremental update metrics in diagnostics

3. Surface/core desynchronization
- Mitigation: revision IDs on every patch/event and explicit resync command

4. Scope creep into full IDE behavior
- Mitigation: keep initial scope to core text editing and diagnostics overlays

---

## 11. Acceptance Criteria

1. `viewer:text-editor` can open and edit text nodes in a workbench tile.
2. All edits flow through `editor-core` transactions.
3. Undo/redo behavior is deterministic under replay tests.
4. Surface can recover from revision mismatch via explicit resync.
5. Large-file scrolling remains responsive via viewport virtualization.
6. Syntax highlighting and diagnostics update incrementally.
7. No Servo/egui types appear in `editor-core` public API.

---

## 12. Immediate Next Step

Create a small `editor-core` spike crate with:

1. `DocId`, `Position`, `TextRange`, `EditOp`, `Transaction`
2. rope-backed buffer
3. `apply(transaction)` and `undo/redo`
4. replay tests

This establishes the semantic authority before UI implementation accelerates.
