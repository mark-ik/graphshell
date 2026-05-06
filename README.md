# Graphshell

Graphshell has moved into the Mere workspace.

Active development now lives at <https://github.com/mark-ik/mere>. In that
workspace, Graphshell is the portable shell layer under `crates/graphshell`,
while Mere is the product/workspace root that owns identity, transport, engine
routing, composition, and the browser entrypoint.

This repository remains as the historical donor for the pre-Mere Graphshell
prototype and migration references.

For current work, use:

```sh
git clone https://github.com/mark-ik/mere.git
cd mere
cargo test --workspace
```
