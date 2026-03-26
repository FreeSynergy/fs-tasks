# fs-tasks

FreeSynergy Tasks — data offers, task builder and task templates.

Part of the [FreeSynergy](https://github.com/FreeSynergy) platform.

## Purpose

Implements the Tasks system: Data Offers/Accepts, visual pipeline editor,
and reusable task templates from the Store.

## Architecture

- `TasksApp` — root Dioxus component
- `TaskModel` / `TaskTemplate` — domain model
- `PipelineEditor` — visual pipeline editor

## Build

```bash
cargo build
```

## Dependencies

- **fs-desktop** (`../fs-desktop/vendor/dioxus-desktop`) — patched Dioxus desktop
