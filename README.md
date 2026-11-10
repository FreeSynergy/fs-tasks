# FreeSynergy Tasks — know what to do next

**Nobody wants a Kanban board. People want to know what to do next.**

FreeSynergy Tasks keeps what you or your group have to do, with a due date, a status, labels and links between tasks. Lists, status views and, later, boards and timelines are just different ways of looking at the same tasks.

What is different here: a task follows the open JSCalendar standard (RFC 8984), and this program is its owner, so the calendar and every other program reach tasks only through one interface instead of keeping their own idea of a task. Your tasks import and export as iCalendar (VTODO), and the original text is kept. A task can start with you alone and be shared later.

## What it can do today

Next tag: `v0.0.4` (latest tagged: `v0.0.3`).

- Create, edit, assign, label, relate, list and delete tasks. Find them by due date range, progress, label, or a word in the title or description.
- Import and export iCalendar tasks (VTODO). Events in the same file are left for the calendar.
- Tell other programs when a task is created, changed or deleted.
- Refuse an overwrite when someone else changed the task meanwhile (compare-and-set).
- Tasks marked sensitive are left out of exports unless you release them by name.
- A small server that offers all of this over a local socket.
- A window made from the description of a task: list, detail, forms and command buttons (generated, the same way for every FreeSynergy program), plus a status board and a timeline. Import and export of iCalendar files is on the command line.
- The program tells every front end what a task is and which commands exist (`Tasks.types`); commands that are planned but not built are listed as not supported.
- The calendar already shows tasks through this interface.

## What is not there yet

- No real boards. The status board in the window is a view of the task's status: moving a card changes the status for everyone. Columns of your own, swimlanes and limits are not built (their controls are locked, "comes later").
- Not supported yet (listed, locked): boards, columns, attachments, comments, watching and notifications, history, custom fields, templates, members, webhooks, board export/import, editing of repeating rules and reminders, and the rest of the list in [DESIGN.md](DESIGN.md). The generated Admin view of this revision cannot show the lock: such a command answers "unsupported".
- No repeating-task engine. Repeating tasks are kept but not expanded, and the window cannot edit one occurrence.
- The window reads the data directly instead of talking to the server, and "my next tasks" shows every open task because there is no sign-in yet.
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
