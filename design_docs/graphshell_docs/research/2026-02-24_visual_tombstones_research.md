# Visual Tombstones (Ghost Nodes) Research

**Status**: Research / Backlog
**Context**: Extracted from `archive_docs/checkpoint_2026-01-29/COMPREHENSIVE_SYNTHESIS.md`.
**Distinction**: Distinct from P2P "Ghost Nodes" (which are conflict artifacts). These are user-facing "deleted but remembered" placeholders.

## Concept

When a user deletes a node, the graph structure (edges) is often lost, creating a hole in the mental map. "Visual Tombstones" preserve this structure without keeping the full node content.

> "Use ghost nodes to preserve structure when removing items. When node deleted, show ghost edges (dashed/faded) to preserve knowledge of connections."

## Use Cases

1.  **Refactoring**: You delete a central hub node but want to remember what it connected.
2.  **History**: "I know I had a link here yesterday."
3.  **Pruning**: Cleaning up a graph without losing the topology.

## Implementation Sketch

### Data Model
-   **`NodeState::Tombstone`**: A new lifecycle state (alongside Active/Warm/Cold).
-   **Payload**: Retains `id`, `position`, `title` (optional), and `edges`. Drops `url`, `thumbnail`, `favicon`.
-   **Persistence**: Tombstones are persisted but can be garbage collected after N days or explicit "Clear Tombstones" command.

### Visuals
-   **Node**: Rendered as a faint, dashed outline or a small "X" marker. No fill.
-   **Edges**: Connected edges render as dashed/faded lines (`EdgeStyle::Ghost`).
-   **Interaction**: Non-interactive for navigation. Right-click to "Restore" or "Permanently Delete".

### Toggle
-   **"Show Deleted"**: A toggle in the Graph View settings (like "Show Archived").
-   Default: Off (tombstones are invisible).
-   On: Tombstones appear, revealing the "graveyard" of the graph.

## Integration

-   **Badge Plan**: Can reuse the `#archive` tag logic (dimmed rendering) but with a distinct visual style.
-   **Edge Traversal**: Tombstones preserve traversal history even if the destination content is gone.

## Recommendation
Defer until **Graph UX Polish** is complete. This is a high-value feature for long-term graph maintenance but adds visual noise if not handled carefully.