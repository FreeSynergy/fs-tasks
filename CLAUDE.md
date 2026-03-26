# CLAUDE.md – fs-tasks

## What is this?

FreeSynergy Tasks — data offers, task builder and task templates.

## Rules

- Language in files: **English** (comments, code, variable names)
- Language in chat: **German**
- OOP everywhere: traits over match blocks, types carry their own behavior
- No CHANGELOG.md
- After every feature: commit directly

## Quality Gates (before every commit)

```
1. Design Pattern (Traits, Object hierarchy)
2. Structs + Traits — no impl code yet
3. cargo check
4. Impl (OOP)
5. cargo clippy --all-targets -- -D warnings
6. cargo fmt --check
7. Unit tests (min. 1 per public module)
8. cargo test
9. commit + push
```

Every lib.rs / main.rs must have:
```rust
#![deny(clippy::all, clippy::pedantic)]
#![deny(warnings)]
```

## Architecture

- `TasksApp` — root Dioxus component
- `TaskModel` / `TaskTemplate` — domain model (model.rs)
- `PipelineEditor` — visual pipeline editor (pipeline_editor.rs)

## Dependencies

- **fs-desktop** (`../fs-desktop/vendor/dioxus-desktop`) — patched Dioxus desktop
