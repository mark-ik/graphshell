# Graphshell Developer Guide

**Last Updated:** February 20, 2026
**For:** New contributors and AI assistants
**See Also:** [QUICKSTART.md](QUICKSTART.md), [ARCHITECTURAL_OVERVIEW.md](ARCHITECTURAL_OVERVIEW.md)

---

## Quick Orientation

**Graphshell** is a spatial browser built on Servo where webpages are nodes in a force-directed graph.

- **Location:** project root (standalone crate, servo is a git dep)
- **Status:** M1 complete; M2 active (workspace routing, graph UX polish, edge traversal, settings)
- **Build:** `cargo build` / `cargo run` (no mach needed)

---

## Essential Commands

### Build & Run
```bash
# Build (release mode recommended)
cargo build --release

# Run
cargo run --release -- https://example.com

# Run with logging
RUST_LOG=debug cargo run -- https://example.com

# Clean build (if stuck)
cargo clean
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name --lib

# Count passing tests
cargo test --lib 2>&1 | grep "test result"
```

### Code Quality
```bash
cargo fmt            # Format code
cargo clippy         # Lint
cargo check          # Check compilation
```

---

## Code Conventions

### Required Practices

1. **UTF-8 Safety:** Always use `util::truncate_with_ellipsis()` for string truncation
2. **Persistence Discipline:** Every mutation must call `log_mutation()` before applying
3. **Test Coverage:** Every bug fix needs a regression test
4. **UUID Identity:** Node UUID is identity; URL is mutable metadata and can be duplicated
5. **Tests in Same File:** Use `#[cfg(test)]` modules in implementation files

### Architecture Constraints

- **No breaking Servo core:** Servo is a git dep; graphshell changes are local only
- **NodeKey stability:** petgraph StableGraph ensures NodeKey survives deletions
- **Webview mapping:** `webview_to_node` and `node_to_webview` are inverses
- **Lifecycle boundary:** reducer state mutates via intents; reconcile performs runtime side effects

---

## Module Map (Quick Reference)

### Core Data
- **`graph/mod.rs`** (~1.0k) — StableGraph wrapper, UUID identity, Node/Edge types
- **`graph/egui_adapter.rs`** — Graph -> egui_graphs projection

### UI + Runtime
-- **`desktop/gui.rs`** (~1.2k) — top-level GUI integration/orchestration
-- **`desktop/gui_frame.rs`** (~1.2k) — frame phases and apply/reconcile sequencing
-- **`desktop/toolbar_ui.rs`** (~0.4k) — toolbar orchestration + submodules (controls, settings, location panel/submit/dropdown, right controls, omnibar)
-- **`desktop/webview_controller.rs`** (~0.4k) — webview submit/close/reconcile helpers
-- **`render/mod.rs`** (~3.4k) — graph/tile rendering and interaction

### State & Persistence
-- **`app.rs`** (~6.0k) — reducer, lifecycle helpers, workspace routing, undo/redo
- **`persistence/mod.rs`** — fjall log + redb snapshots
- **`persistence/types.rs`** — LogEntry variants and snapshot schema

### Utilities
- **`util.rs`** (66 lines) — String truncation, utilities

**See [CODEBASE_MAP.md](CODEBASE_MAP.md) for detailed module breakdown.**

---

## Common Development Tasks

### Add a Graph Mutation (Full Cycle)

**1. Define LogEntry variant** (`persistence/types.rs`)
```rust
#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub enum LogEntry {
    // ... existing variants
    YourMutation { url: String, data: YourData },
}
```

**2. Add replay logic** (`persistence/mod.rs`)
```rust
ArchivedLogEntry::YourMutation { url, data } => {
    if let Some((key, _)) = graph.get_node_by_url(url.as_str()) {
        graph.apply_mutation(key, data);
    }
},
```

**3. Wire in app.rs**
```rust
pub fn your_mutation(&mut self, key: NodeKey, data: YourData) {
    if let Some(store) = &mut self.persistence {
        store.log_mutation(&LogEntry::YourMutation {
            url: self.graph.get_node(key)?.url.clone(),
            data: data.clone(),
        });
    }
    self.graph.apply_mutation(key, data);
    self.egui_state_dirty = true;
}
```

**4. Add tests**
```rust
#[test]
fn test_your_mutation_persists() {
    let (mut store, _dir) = create_test_store();
    store.log_mutation(&LogEntry::YourMutation { ... });
    let graph = store.recover().unwrap();
    // Verify mutation was applied
}
```

### Add a Keyboard Shortcut

**1. Add to `input/mod.rs`**
```rust
if !ui_has_focus(ctx) && ctx.input(|i| i.key_pressed(egui::Key::Y)) {
    app.your_action();
    return true;
}
```

**2. Document in [QUICKSTART.md](QUICKSTART.md)**

**3. Add test**

---

## Debugging Patterns

### Physics Issues

**Nodes not moving:**
```rust
// Check if paused
if !app.physics.is_running() { warn!("Physics paused"); }

// Check for NaN
if !node.position.is_finite() { error!("NaN position"); }
```

**Enable physics panel:** Press `P` key for live config

### Persistence Issues

**Changes not persisting:**
```rust
// Verify logging
if let Some(store) = &mut self.persistence {
    store.log_mutation(&LogEntry::YourMutation { ... });
}
```

**Add debug logging in `persistence/mod.rs::replay_log()`**

### Webview Issues

**Webview not appearing:**
```rust
// Check mapping
if let Some(webview_id) = app.node_to_webview.get(&node_key) {
    log::info!("Mapped: {:?} -> {:?}", webview_id, node_key);
}

// Check view state
match app.view {
    View::Graph => log::info!("Graph view - webviews destroyed"),
    View::Detail(key) => log::info!("Detail view: {:?}", key),
}
```

### Rendering Issues

**Graph not updating:**
```rust
app.egui_state_dirty = true;  // Force rebuild
```

**Low FPS:**
- Check node count (target: 500 @ 45 FPS)
- Profile with `RUST_LOG=debug`
- Consider viewport culling (not yet implemented)

---

## Current Work Status

**Phase:** M1 complete (FT1-6); M2 active
**Active:** Workspace routing, graph UX polish, edge traversal, settings architecture

### Known Issues

1. **Large modules** remain (`app.rs`, `render/mod.rs`, `gui.rs`) — staged decomposition still planned (toolbar_ui.rs decomposed as of 2026-02-23).
2. **Lifecycle contract migration** is active (`2026-02-20_embedder_decomposition_plan.md`): reconcile/runtime model and backpressure policy are still evolving.
3. **Selection-state hardening** follow-up remains active as graph/tile behavior expands.

---

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| Nodes @ 45 FPS | 500 | Not measured (benchmarks pending) |
| Nodes @ 30 FPS | 1000 | Not measured |
| Test coverage | Rising | Run `cargo test --lib -- --list` for current totals |
| Startup time | <2s | Not measured |

---

## Git Workflow

### Before Committing
```bash
cargo fmt            # Format
cargo clippy         # Lint
cargo test           # Test
git add -A
git commit -m "Step X: Summary..."
```

### Commit Message Format (Follow Recent Pattern)
```
Step X: Short summary (50 chars max)

## Changes

### Category 1
- Bullet detail
- Another detail

### Test Status
- Test count: X -> Y (all passing)
```

---

## Troubleshooting Checklist

### Build Fails

- [ ] Run `cargo clean`
- [ ] Check Rust version: `rustc --version` (toolchain pinned in `rust-toolchain.toml`)
- [ ] Check disk space (~25GB needed for servo git dep build)
- [ ] Try debug build: `cargo build`

### Tests Fail
- [ ] Use `TempDir` for test isolation
- [ ] Use `new_for_testing()` instead of `new()`
- [ ] Run single test: `cargo test test_name -- --nocapture`

### Runtime Crash
- [ ] Check for NaN positions
- [ ] Verify char-aware string truncation
- [ ] Check persistence replay cases
- [ ] Enable `RUST_LOG=debug`

---

## Resources

### Documentation
- **[DOC_README.md](../../DOC_README.md)** — Canonical project documentation index
- **[ARCHITECTURAL_OVERVIEW.md](ARCHITECTURAL_OVERVIEW.md)** — Implementation details
- **[IMPLEMENTATION_ROADMAP.md](../implementation_strategy/IMPLEMENTATION_ROADMAP.md)** — Feature targets
- **[QUICKSTART.md](QUICKSTART.md)** — Command reference
- **[CODEBASE_MAP.md](CODEBASE_MAP.md)** — Detailed module map

### Crates
- [petgraph 0.8](https://docs.rs/petgraph/0.8/)
- [egui 0.33.3](https://docs.rs/egui/0.33.3/)
- [egui_graphs 0.29](https://docs.rs/egui_graphs/0.29/)
- [fjall 3](https://docs.rs/fjall/3/)
- [redb 3](https://docs.rs/redb/3/)
- [rkyv 0.8](https://docs.rs/rkyv/0.8/)

### Servo
- [API Documentation](https://doc.servo.org/servo/)
- [GitHub](https://github.com/servo/servo)
- [Servo Contribution Guide](https://github.com/servo/servo/blob/main/docs/HACKING_QUICKSTART.md)

