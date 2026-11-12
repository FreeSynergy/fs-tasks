# FreeSynergy Tasks — know what to do next

**Nobody wants a Kanban board. People want to know what to do next.**

FreeSynergy Tasks keeps what you or your group have to do, with a due date, a status, labels and links between tasks. Lists, status views and, later, boards and timelines are just different ways of looking at the same tasks.

What is different here: a task follows the open JSCalendar standard (RFC 8984), and this program is its owner, so the calendar and every other program reach tasks only through one interface instead of keeping their own idea of a task. Your tasks import and export as iCalendar (VTODO), and the original text is kept. A task can start with you alone and be shared later.

## What it can do today

Version 0.0.6 (in progress). Latest tag: `v0.0.5`.

- Create, edit, assign, label, relate, list and delete tasks. Find them by due date range, progress, label, or a word in the title or description.
- Import and export iCalendar tasks (VTODO). Events in the same file are left for the calendar.
- Tell other programs when a task is created, changed or deleted.
- Refuse an overwrite when someone else changed the task meanwhile (compare-and-set).
- Tasks marked sensitive are left out of exports unless you release them by name.
- A small server that offers all of this over a local socket.
- A first, provisional window: a list and a status view of your tasks, search, create, edit, delete, import and export.
- The calendar already shows tasks through this interface.

**Not done:** the window calls the core in process, not over the socket, so it has no version handshake with the server yet; that comes with a window over RPC.

## Notes for 0.0.6

- **Faster dependency questions:** the readiness of many tasks at once, the `ready` filter and the cycle check of `relate` no longer walk the graph once per task. Measured on a chain of 5000 tasks (release build): a page of ready tasks 0.22 s (before: 117 s for a chain of 200), `relate` in front of the chain 19 ms. `readiness_many` is new in the core (not on the wire yet).
- **`ready` filter** for lists and pages in the core (`TaskQuery::only_ready`); on the wire it needs a filter member in a later `fs-interfaces`.
- **"Became ready" events** are now also sent when a task starts and a start-to-start tie is satisfied, when a relation is removed, and when a task is deleted.

## Notes for 0.0.5

- **Check your relations:** the documentation of the relation types `finishtostart`, `finishtofinish`, `starttostart` and `starttofinish` was reversed until 0.0.4. As in RFC 9253 section 4, **the task that holds the relation is the predecessor** (the other task cannot start or finish until this one does); `depends-on` is held by the dependent task. Relations created over the interface in the old, reversed sense have to be checked and swapped.
- **Readiness:** a task now says whether it can be started: `ready`, `blocked` (by which tasks), `done`, `unknown` (a dependency this store does not hold), `dependencyCancelled` (a task it waits for was cancelled or failed: a person decides, nothing frees it by itself) and `conflict` (it is part of a dependency cycle). New calls `readiness`, `blockedBy`, `dependents`.
- **Cycles:** `createRelation` refuses a relation that would close a dependency cycle (problem type `tasks/cycle`, status 409, the way round in the extension `path`). A cycle that arrives some other way (an edit, an import) is shown as `conflict`, not refused.
- **Gap:** `createRelationWithGap` sets a lag (`P1D`) or lead (`-PT4H`) on a temporal relation. It is kept as the member `freesynergy.net:gap` and written to iCalendar as the RFC 9253 `GAP` parameter; an imported `GAP` is read into it.
- **Schedule and ready filter:** the core can plan tasks with their durations and dependencies (earliest and latest start, slack), and the list can be limited to ready tasks (core only, not on the wire yet). The event "became ready" is sent as an update of the task.
- The index of older tasks is completed (new fields for the reverse lookup) when the **server or the window** opens the store (`fs-tasks serve`, `fs-tasks`); the command line talks to the server. Until then a task of 0.0.4 or older shows too few dependents and may look ready.
- `blockedBy` also returns the tasks a task waits for that were cancelled or failed (the contract text says "unfinished"); those are the ones a person must decide about.
- Limits of the dependency check (see DESIGN.md): measured on a chain of 5000 tasks (release build), a `ready` page takes 0.22 s and a `relate` in front of the chain 19 ms; a store above 100 000 tasks or a graph above 20 000 tasks is refused for these questions.

## Notes for 0.0.4

- The documentation of the relation types `finishtostart`, `finishtofinish`, `starttostart` and `starttofinish` was reversed before. As in RFC 9253 section 4, **the task that holds the relation is the predecessor** (the other task cannot start or finish until this one does); `depends-on` is held by the dependent task. If you created such relations over the interface, check their direction.
- Relations imported from iCalendar with the RFC names in capitals (`FINISHTOSTART`, `DEPENDS-ON`) are now recognised like ours in lower case. The export keeps its spelling.

## What is not there yet

- No real boards. The status view in the window is not a board: moving a card changes the task's status for everyone, and the window says so. Columns of your own, swimlanes and limits are not built.
- No repeating-task engine. Repeating tasks are kept but not expanded, and the window cannot edit one occurrence.
- The window cannot assign people, show relations, checklists, comments, attachments or reminders, and it has no undo.
- The window reads the data directly instead of talking to the server (a version over the server is on branch `gui`), and "my next tasks" shows every open task because there is no sign-in yet.
- The interface is a draft with one user (the calendar); it needs a second before it can be called stable.

## Part of FreeSynergy

Tasks is the "what next" of the project-management suite, together with history, resources, surveys and groups. FreeSynergy is a family of programs that are free for everyone, forever, and that keep your data yours. The documentation, the manifesto and the overall plan live in [fs-documentation](https://github.com/FreeSynergy/fs-documentation); the reading version is at <https://freesynergy.github.io/fs-documentation/>. Start with the [manifesto](https://freesynergy.github.io/fs-documentation/manifesto/) if you want to know why we do this.


## For developers

- One program, one command (ADR 0063): `fs-tasks` opens the window, `fs-tasks serve` serves the contract over a Unix socket, `fs-tasks list|show|add|done|delete|export|import` is the command line. Build without the window: `--no-default-features`.
- Workspace crates: `fs-tasks` (the binary), `fs-tasks-core` (core, no GUI), `fs-tasks-server`, `fs-tasks-cli` and `fs-tasks-app` (libraries behind the subcommands).
- The contract is `tasks.capnp` in [fs-interfaces](https://github.com/FreeSynergy/fs-interfaces) (draft). Tasks owns it. Shared JSCalendar parts come from [fs-jscalendar](https://github.com/FreeSynergy/fs-jscalendar).
- Build and test: `cargo build --workspace` and `cargo test --workspace`.
- Design and the full state table: [DESIGN.md](DESIGN.md).
- Licence: MIT OR Apache-2.0.
