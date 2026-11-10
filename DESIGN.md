# fs-tasks – Design

`fs-tasks` is the tasks program of FreeSynergy and the **owner of the JSCalendar `Task`** (ADR 0050: one standard, one
owning program, one interface). This repo holds the **program core** (`crates/fs-tasks-core`, no GUI), the server (`crates/fs-tasks-server`) and the **frontend** (`crates/fs-tasks-app`, see below). The contract
(`tasks.capnp`) lives in [`fs-interfaces`](https://github.com/FreeSynergy/fs-interfaces) (ADR 0045); this program is its owner. Other programs, e.g. `fs-calendar`, use tasks **only through the interface** (`TaskItems`, `TaskFeed`) and have
no Task model of their own. Design of the whole program: `fs-documentation/docs/en/programs/apps/tasks/design.md`.

## Interface status

**Interface status: draft, consumers: fs-calendar (1 of 2).** (ADR 0045: the interface is a target. It stays draft until two
independent consumers have tested it, then 1.0, then only version jumps.) The contract is `tasks.capnp` in `fs-interfaces` (marked DRAFT; owner: this program). The Rust form comes first (ADR 0043): the traits
`TaskItems` and `TaskFeed`.

## State

Branch `linear-import` (after 0.8.0, not tagged): the `.ics` import is **linear**. Before, each part asked the store three times whether its `uid` was taken (`stored_by_uid`, in `import_ics_inner`, `import_one` and `create_inner`), and the store answers that by looking at every task of the type, so 1 000 tasks cost 1 000 x 3 x (up to 1 000) looks. Now `known_uids` (`repo/known.rs`) reads the `uid`s of the stored tasks **once**, in pages of 500 with no condition, the import keeps the set up to date, and `store_new` stores without asking again (`create` still asks the store, once). Measured by `tests/import_linear.rs`: the work asked of the store (calls, rows, and a charge of the stored tasks for each lookup by an index field) for 60 and 120 tasks is 181 and 361 (ratio 1.99; the old flow: 5 670 and 22 140, ratio 3.90); by wall clock (debug build, a loaded machine) 500 / 1 000 / 2 000 tasks took 3.8 / 7.2 / 14.7 s. The set is held in memory for the length of one import (the `uid`s of all stored tasks). Version 0.7.0: builds on `fs-interfaces` v0.44.0 and `fs-objects` v0.4.0. `TaskFilter.term` and `open` are evaluated in the store; `Items.list` and `Items.query` cut their pages **in the store at a place in the sort key** (`TaskItems::page`), not from the whole list; `SupportReport.instance` is the store's identity; problems carry typed extensions (table below). Version 0.6.0: `TaskItems::edit_checked` and `Items.editIf` (compare-and-set), `revision` on every task, `fs-tasks-server` follows the contract of `fs-interfaces` v0.36.0. Version 0.5.3: builds on `fs-interfaces` v0.36.0 (git tag) and `fs-jscalendar` v0.1.3. Version 0.5.0: `fs-tasks-server`, the contract over a Unix socket; `TaskEntry` carries its envelope. Before, in 0.4.0: `TaskItems` follows the provider rule (ADR 0045): every method of `Items` in `tasks.capnp`, defaults answering `Unsupported`, `capabilities()` returns `fs_interfaces::Capabilities`; `fs-interfaces` v0.4.0 is a normal dependency (feature `contract` keeps the generated-code re-export). Before, in 0.3.0: the shared JSCalendar parts (time, rule, participants, upgrade, shared validation, iCalendar conversion) come from [`fs-jscalendar`](https://github.com/FreeSynergy/fs-jscalendar) v0.1.0 instead of being copied. `Task` itself stays here (ADR 0050).

| Area | State |
|---|---|
| Typed `Task` (serde), lossless unknown/vendor properties (`extra`) | done (moved from fs-calendar-core) |
| Validation (`version` "2.0", `uid`, LocalDateTime without zone suffix/fraction, IANA zones only, durations, `progress`, `percentComplete`, single `recurrenceRule`, count XOR until, override keys, participant status, obsolete 1.0 members refused) | done, all issues at once |
| `upgrade()` RFC 8984 -> 2.0 | from `fs-jscalendar`, re-exported |
| RFC 9253 relations: `Relation` enum, `Task::relate` / `relations` (`relatedTo`) | done; unknown relation keys from other writers are kept, not rejected |
| RFC 10051 status as vendor property `freesynergy.net:taskStatus` (`task_status` / `set_task_status`, index `task_status`) | done (preserved, not interpreted) |
| Interface `TaskItems` (create, edit, assign, relate, label, get, get_by_uid, delete, list, query by due range / progress / label, VTODO import/export, capabilities as `Capabilities`) | done, draft |
| Interface `TaskFeed` (Observer: `subscribe(listener)` -> `Subscription`; `Created` / `Updated` / `Deleted`) | done, draft |
| `TaskRepo` over fs-objects 0.2 (`ObjectRepository` + `ObjectQuery` only; `TaskRepo::with_backend`) | done |
| Envelope: type `Task`, `name` = `title` (ADR 0032), **`context=tasks`**, IRI `https://<instance>/tasks/<id>`, `startTime` / `endTime` (due date, else start + estimate, else start) | done |
| Index fields: `uid`, `type`, `start_utc`, `end_utc`, `due_utc`, `progress`, `task_status`, `label`, `participant_email`, `tzdb_version` (imports) | done |
| Queries (`TaskQuery` -> one store query: due range `[from, to)` with open ends, progress, label; ordered by due, undated last) | done |
| ICS VTODO import / export through `fs-jscalendar`; original kept per UID (`text/calendar`); `VEVENT`s of the same file are ignored (they belong to fs-calendar) | done |
| Contract `tasks.capnp` (in fs-interfaces): root `Tasks` (`capabilities`, `items`, `subscribe`, `boards`), `Items` (create, edit, assign, createRelation, assignLabel, get, getByUid, delete, list, query, importIcs, exportIcs), `Boards` **placeholder, no methods, not implemented** | done, DRAFT |
| Generated Rust: in `fs-interfaces` (`tasks_capnp`, `common_capnp`, generated by our `capnp-compiler`); feature `contract` pulls it in and re-exports both modules. The `schema/` copy, `build.rs` and `regenerate` feature are gone | done |
| Wire conversions (`contract::{write_filter, read_filter, relation_*, write_capabilities}`) | done |
| Server `fs-tasks-server`: every method of `Items`, `subscribe` (with `ended`), `capabilities` (what the provider offers), `supportReport`; `boards` answers `Unsupported`; errors as typed problems (`tasks/not-found` 404, `tasks/conflict` 409, `tasks/invalid` 422, `Unsupported` 501); binary with `--socket`, `--data`, `--actor`, `--instance` (must be a domain, else exit code 2) | done, draft; pages cut in the store at a place (cursor form 2, see Paging), `with_page_size` for tests |
| One command `fs-tasks` (ADR 0063, `crates/fs-tasks`): `serve` → `fs-tasks-server`, `app` or no word → `fs-tasks-app` (feature `gui`, on by default), the rest → `fs-tasks-cli` (`capabilities`, `list`, `show`, `add`, `done`, `delete`, `export`, `import` over the socket; the second user of `tasks.capnp` after the MCP tools). The three crates have no binary of their own; tests: the real program across a restart (`tests/binary.rs`), the dispatch (`tests/dispatch.rs`), and the commands against a real server in process (`fs-tasks-cli/tests`) | done, draft; `add` knows `--due`, `--zone`, `--label` only; no `query` filter yet |
| Boards, columns, `task_link`, card moves (`Update` on `task_link`, never on the task) | open, not started (design.md; `Boards` is a placeholder); the frontend's status view is something else, see Frontend |
| Type description of `Task` (`DescribesType`, all commands of requirements.md, `supported` flag), `Tasks.types` in the server, window = Admin view + status board + timeline | done, see Frontend; the Admin view cannot show the lock yet |
| Provider rule: `Error::Unsupported`; `capabilities()` and the reads (`get`, `get_by_uid`, `list`, `query`) are required, all other methods default to `Unsupported`; `TaskRepo` reports all but the boards ids (unsupported: "Boards is a placeholder; not built yet, it is possible") | done |
| Integrity: a uid that names an existing object is a `Conflict`, not an overwrite; write flows run one at a time (`writes`); `progress` is indexed as its RFC 8984 §5.2.5 effective value; `list` is by due date, undated last; range bounds round up to the index's seconds; imports report what they lost: garbage in `DTSTART`, `DUE`, `DURATION`, `COMPLETED`, `PERCENT-COMPLETE`, `RRULE`, read on the `VTODO`'s own unfolded lines | done |
| Limits: a task's JSON plus its envelope name is at most `MAX_TASK_BYTES`, so every answer fits; messages quote a value short; an import report is cut in bytes (`created` whole, the lists end with `... N more`); index times stay inside years 0 to 9999 (the task keeps its real time) | done |
| Compare-and-set: `TaskItems::edit_checked(id, Revision, task)` and `Items.editIf`; every `Task` carries `revision` (derived from the envelope's time and the task, not stored; compare for equality only). A stale revision is `tasks/conflict` (409) whose detail starts `conflict:` and names no revision; nothing is written. Atomic inside one process (the write turn) | done, draft |
| Filters `term`, `open` | `term`: trimmed, substring of the title (envelope name) or the description (index field `description`), case-folded by the store, `%` and `_` are letters. `open`: `progress` from `in-process` on in the text order of the closed list (`cancelled < completed < failed < in-process < needs-action`), a task without `progress` is indexed as `needs-action`. All filters are AND | done |
| Paging | see below | done |
| Tests | core `tests/edit_checked.rs`, `tests/integrity.rs` plus the earlier ones; server over a real socket, comparing values, exact problem types, the real binary across a restart, exit codes, data directory mode |
| ADR 0043 | every file <= 200 lines, `//! Pattern:` in every module, `#![deny(missing_docs)]`, `#![warn(clippy::pedantic)]` clean |

## Frontend (`crates/fs-tasks-app`, ADR 0065 / 0066)

The window is the standard shell (ADR 0065) with three screens: the generated **Admin** view of the type `Task` (list, detail, forms, command buttons, all made from the type description by `fs-admin-frontend`), the **status board** (`fs-components-kanban`) and the **timeline** (`fs-components-timeline`). Menus: Program (Quit), View (Tasks, Status board, Timeline), Help (Help, About: `FreeSynergy.Tasks`, `v` + `CARGO_PKG_VERSION`, licence, repository, origin). The app draws nothing and imports no `blinc_*` crate; it gives data, the trait and the description. The old workbench window (`fs-components-workbenches`; own search, sheet and form; `--import`, `--export`, `--snapshot`, `--view`) left the build; its code is in git history at tag `v0.0.3` (tags are not deleted). iCalendar import and export is the command line (`fs-tasks import|export`); the window keeps `--store`, `--size`, `--help`. The marking of sensitive tasks of the old window (`is_marked`) left with it: the core has no check of its own yet (ADR 0029, open).

**Pattern map of the new parts.**

| Module | Pattern | Why |
|---|---|---|
| core `describe/task_type.rs` | Registry entry (`DescribesType`) | `Task` reports name, program `fs-tasks`, standard "JSCalendar Task (RFC 8984 / 9253)", fields and **every** command |
| core `describe/support.rs` | Registry | the one table capability id -> command, supported, reason; the description gets its `supported` flag from it in one line (`SWITCH` in `task_type.rs`) |
| core `describe/{fields,commands,locked}.rs` | Builder | the fields, the commands the core does, the locked ones (their inputs are a first guess) |
| app `backend/*` | Adapter + Command | `TaskBackend`: `fs_admin_frontend::Backend` over `Arc<dyn TaskItems>`; `read.rs` / `write.rs` map JSCalendar JSON and values, `commands.rs` is one function per command, `issues.rs` maps errors |
| app `frontend_link.rs` | Adapter (**temporary**) | description and findings v0.0.11 -> the v0.0.2 types of the shipped frontend, and the form of `change` |
| app `views/*` | Adapter | tasks as columns and cards, as bars and arrows; moves go through the backend |
| app `window.rs` | Facade | shell, three screens, menus, About |
| server `tasks_service.rs` (`types`) | Adapter | `Tasks.types` returns the registry through `CatalogServer` |

**Type `Task`.** Fields (JSCalendar): `title` (text, required), `description` (multilineText), `start`, `due` (dateTime), `estimatedDuration` (text), `progress` (choice needs-action / in-process / completed / failed / cancelled), `percentComplete`, `priority` (text), `keywords` (keywords), `participants` (reference Person, shown), `relatedTo` (reference Task, shown), `locations` (place, multiple), `links` (link, multiple), `recurrenceRules` (text, shown), `alerts` (text, shown), `color`, `timeZone` (text). **Basic kinds `fs-types` does not have** are mapped to `text`: duration (ISO 8601, checked by the core), whole number (percent 0..100, priority 0..9, checked in the backend), colour, time zone (IANA name, checked by the core). Date-times are instants in the form and local times of the task's zone in the JSON (`start` and `due` are read in `timeZone`; a floating time counts as UTC). `participants`, `relatedTo`, `recurrenceRules` and `alerts` are shown; `change` accepts them only unchanged (people: `assign`; relations: `relate`; rules and alerts cannot be edited yet). `locations` and `links` replace the entries that have a coordinate or a `href` and keep the others. A record id is `urn:uuid:<object id>`.

**Commands the core does** (`supported = true`): `create`, `change` (an `Edit`), `find` (list: empty `term`), `delete`, `assign`, `relate`, `label`, `importIcs`, `exportIcs`, `check`, `suggest`. `change` has no input list (rule of `types.capnp`): its input is an `Edit`; the backend checks it with the rules of `fs-types`, reads the task, merges only the fields sent and writes with `edit_checked` and the revision it read (a stale write is refused). `find` is the standard find (`term`) with the filters of `TaskQuery` as further input (`dueFrom`, `dueTo`, `progress`, `label`, `open`).

**`Unsupported` (`supported = false`; in the description, locked where the frontend can show it).** Every other command of `docs/en/programs/apps/tasks/requirements.md` (`card.*` are the board-view names of the same actions; dotted ids are camel case as command names: `card.move` is `cardMove`). Ids the core does by an existing command (`card.create`, `card.edit`, `card.setDue`, `card.assign`, `label.assign`, `relation.create`, `card.priority`, `card.locate`, `task.schedule`, `card.search`, `card.list`, `task.create` ...) are in the table with that command. The row "Script and automate through our own contract" has no capability id. Not supported:

| Capability | Command | Why |
|---|---|---|
| `board.create` | `boardCreate` | boards are not built; the Boards interface is a placeholder |
| `board.edit` | `boardEdit` | boards are not built; the Boards interface is a placeholder |
| `board.open` | `boardOpen` | boards are not built; the Boards interface is a placeholder |
| `column.create` | `columnCreate` | boards are not built; the Boards interface is a placeholder |
| `column.rename` | `columnRename` | boards are not built; the Boards interface is a placeholder |
| `column.move` | `columnMove` | boards are not built; the Boards interface is a placeholder |
| `card.move` | `cardMove` | a card is in a column through task_link, which is not started |
| `card.setStatus` | `cardSetStatus` | RFC 10051 status has no interface method; progress is changed with change |
| `subtask.add` | `subtaskAdd` | progress from the children is not started; relate with child works |
| `attachment.add` | `attachmentAdd` | the file is kept by fs-files; not connected |
| `comment.add` | `commentAdd` | comments are AS2 notes; no model yet |
| `card.watch` | `cardWatch` | no subscription list yet |
| `card.notify` | `cardNotify` | delivery is a platform matter; not defined |
| `card.history` | `cardHistory` | history belongs to fs-history; not connected |
| `field.define` | `fieldDefine` | boards are not built; the Boards interface is a placeholder |
| `card.setField` | `cardSetField` | no model or no interface method in the core yet |
| `template.apply` | `templateApply` | boards are not built; the Boards interface is a placeholder |
| `member.add` | `memberAdd` | membership and rights belong to fs-groups; not connected |
| `member.role` | `memberRole` | membership and rights belong to fs-groups; not connected |
| `webhook.create` | `webhookCreate` | no model or no interface method in the core yet |
| `board.export` | `boardExport` | boards are not built; the Boards interface is a placeholder |
| `board.sync` | `boardSync` | subscribe exists in TaskFeed; live sync of a board is not built |
| `swimlane.create` | `swimlaneCreate` | boards are not built; the Boards interface is a placeholder |
| `column.limit` | `columnLimit` | boards are not built; the Boards interface is a placeholder |
| `epic.create` | `epicCreate` | no model or no interface method in the core yet |
| `card.type` | `cardType` | no model or no interface method in the core yet |
| `workflow.define` | `workflowDefine` | no model or no interface method in the core yet |
| `card.remind` | `cardRemind` | alerts are kept but cannot be edited yet |
| `card.repeat` | `cardRepeat` | no basic type for a recurrence rule; recurrence rules are kept but cannot be edited |
| `comment.mention` | `commentMention` | no model or no interface method in the core yet |
| `card.copy` | `cardCopy` | no model or no interface method in the core yet |
| `card.share` | `cardShare` | no model or no interface method in the core yet |
| `card.archive` | `cardArchive` | no model or no interface method in the core yet |
| `board.archive` | `boardArchive` | boards are not built; the Boards interface is a placeholder |
| `board.style` | `boardStyle` | boards are not built; the Boards interface is a placeholder |
| `query.save` | `querySave` | no model or no interface method in the core yet |
| `rule.create` | `ruleCreate` | no model or no interface method in the core yet |
| `form.submit` | `formSubmit` | no model or no interface method in the core yet |
| `card.linkCommit` | `cardLinkCommit` | no model or no interface method in the core yet |
| `board.import` | `boardImport` | boards are not built; the Boards interface is a placeholder |
| `task.filter` | `taskFilter` | no query language; find takes the fields of TaskQuery |
| `task.triage` | `taskTriage` | no model or no interface method in the core yet |
| `cycle.create` | `cycleCreate` | no model or no interface method in the core yet |
| `task.urgency` | `taskUrgency` | no model or no interface method in the core yet |
| `project.review` | `projectReview` | no model or no interface method in the core yet |

**The lock.** The shipped Admin frontend (`fs-admin-frontend` as of `4688cee`) does not know `supported`: it draws every command of the description as a normal button. Therefore `TaskBackend::run` refuses a command whose `supported` is false with `BackendError::Unavailable("unsupported: <name>")` (tested for each one). The visible, locked "kommt noch" control in the Admin view needs Thread A's display: a gap, reported. The status board draws its locked controls itself (add and rename column, limit; text `kanban.soon`).

**Status board and timeline.** The board is a view of the task's own `progress` (the old rule stays: **the status view is not a board**; no `Boards`, no `task_link`): one column per value; a move **edits `progress` of the task for everyone** (through `change`, with the revision check); the column operations are locked (`columnCreate`, `columnRename`, `columnLimit` are `Unsupported`). The timeline shows a bar from `start` to `due` (progress from `percentComplete`, 100 when completed), a milestone where only one of them is set, and an arrow for `depends-on` and `finishtostart`; it is read only. A task without a date has no place on the axis. Days are the days written in the task's zone.

**Pins and the temporary duplication.** `fs-admin-frontend` and every `fs-components-*` crate of the app come from `fs-components` rev `9950b51` (branch `r1-components`; its `fs-admin-frontend` is the reviewed `4688cee` of branch `thread-a-admin-frontend`, unchanged, and the same commit adds kanban and timeline). Untagged: switch to the tag once Thread A / C tag. All crates of the app must come from the same commit, otherwise there are two copies of `AppWidget`. The admin frontend is built on `fs-types` **v0.0.2**; the core and the server use **v0.0.11** (the `supported` flag, `Edit`, `change` without input). Cargo cannot unify two tags of one repository, so the app depends on both (`fs-types-frontend` and `fs-interfaces-frontend` are v0.0.2) and `frontend_link.rs` converts the description and the findings and adds the form of `change`. Delete that file when the frontend is tagged on the new types. `[patch.crates-io]` pins `blinc_app` (blinc-app-fix `5180d4e`) and `blinc_layout` (blinc-layout-fix `cb17db8`), checked on 2026-11-10 as the newest commits of both forks.

**Other limits.** The core is called in process (a tokio handle blocks the interface thread; a slow core freezes the window) until the RPC client exists. A move on the board does not refresh the Admin list until that list reloads. No sign-in. Closing the window loses an unsaved form (the Quit menu warns). Words are English in `words.rs` until the catalog (ADR 0057) serves them.

## Paging (`TaskItems::page`, `repo/page.rs`, server `cursor.rs`, `paging.rs`)

The order is the store's `order_by_index("order_key")`: smallest case-folded `order_key`, then the case-folded title, then the id as text. `order_key` is the due time as index text, or `9999-12-31T23:59:60Z` for a task without one, so undated tasks come last and every task has a place. The cursor (form byte 2) holds the three parts of the last task sent: `order_key` as indexed, the envelope name as stored, the id. The next page is at most three store queries, each with a limit, that together give everything after that place: same `order_key` and same folded title with a greater `id_key`; same `order_key` and a greater folded title; a greater `order_key`. All filters are in every query. The comparisons are consistent with the store's order: `order_key` is one fixed shape of text, so raw and folded order agree; the title goes through the store's own folding (`Cond::Name`); the id is compared as text, as the store orders it (index field `id_key`). A task added or removed between two pages moves no boundary. Cursor form 1 (a count) is refused as `invalid-request`.

Measured, not assumed (`fs-tasks-server/tests/counting.rs`): rows the store returns while all pages are read, at n and 2n tasks; a whole list per page gives ratio 4.00, the keyset gives about 2.

| Index field | Value | For |
|---|---|---|
| `order_key` | due time, or the sentinel | the order and the place |
| `id_key` | the object id as text | the last tie-breaker of a place |
| `description` | the description | `term` |

## Problem extensions (`fs-tasks-server/src/facts.rs`)

The detail text is as before; the facts are extensions (`fs_interfaces::rpc::problem_keys`). A client reads them, it does not take the text apart.

| Problem type | Cause | Extensions |
|---|---|---|
| `tasks/not-found` (404) | no task with the id | `id`, `kind` = `task`; a store `NotFound` gives `id` only |
| `tasks/conflict` (409) | `edit_if`/`edit` with a stale revision | `id`, `kind` = `task` |
| `tasks/conflict` (409) | a uid or id that is taken (`Error::Conflict`) | none: the error carries text, not the object (open) |
| `tasks/invalid` (422) | JSCalendar rules broken | `issue`, once per issue, in the order found (`path: message`) |
| `tasks/invalid` (422) | the text is no JSON / no task | `issue` = the reason |
| `tasks/invalid` (422) | a required text is empty | `issue` = the field (`label`, `email`, `otherUid`) |
| `tasks/invalid` (422) | a time, duration or zone | `kind` = `time`, `issue` = the reason |
| `tasks/invalid` (422) | iCalendar text | `kind` = `icalendar`, `issue` = the reason |
| `tasks/invalid` (422) | the store refused | `issue` = the reason |
| `too-large` (413) | a task over `MAX_TASK_BYTES`, or a text over the intake (`N bytes, at most M fit`) | `size`, `limit` (bytes) |
| `Unsupported` (501), `invalid-request` (422), unreadable stored data (no problem) | | none |

## Structure and pattern map (ADR 0043)

| Module | Pattern | Why |
|---|---|---|
| `interface/items.rs` | Repository (interface) + Null Object (`Unsupported` defaults) | `TaskItems`: the program's public contract; consumers use `dyn TaskItems` |
| `interface/feed.rs` | Observer | `TaskFeed`, `TaskListener`, `Registry` (subject), `Subscription` (drop = unsubscribe) |
| `interface/types.rs` | Value objects / Builder | `TaskEntry`, `ImportReport`, `TaskQuery` (builder methods) |
| `fs-tasks-server/{tasks_service,items_service}.rs` | Adapter + Factory | wire in, `TaskItems` call, wire out; depends on the traits only |
| `fs-tasks-server/{wire,errors,feed}.rs` | Adapter, Observer | tasks on the wire, core errors as RFC 9457 problems, `TaskFeed` to the shared forwarder |
| `repo/handle.rs` | Repository + Facade | `TaskRepo`: hides store, envelope rules, formats |
| `repo/backend.rs` | Adapter | `Arc<dyn Store>` presented as `dyn ObjectRepository` + `dyn ObjectQuery` (interface segregation) |
| `repo/{crud,edit,queries}.rs` | Repository / Template method | writes, sequence-aware edit, filtered queries |
| `repo/edit.rs` (`modify`) | Command (client) | `assign` / `relate` / `label` are in-memory commands on the `Task` applied through the edit flow |
| `repo/{build,span}.rs` | Adapter | task -> AS2 envelope + index fields; time span from `start` / `due` / `estimatedDuration` |
| `repo/transfer.rs`, `ical/*` | Template method / Adapter | fixed import flow; the conversion itself is in `fs-jscalendar` |
| `repo/known.rs` | Value object | `KnownUids`: the `uid`s already stored, read once per import so no part asks the store |
| `repo/unread.rs` | Specification | one rule per property: what an import could not read, on the component's own lines |
| `revision.rs`, `TaskItems::edit_checked` | Newtype + Repository (optimistic lock) | `Revision` has `==` and nothing else, so no code can read it as a counter or an order (the contract says: compare for equality only); the trait method is the compare-and-set, check and write one step under the write turn. Derived from the envelope's time and the task as an answer carries them, not stored; a refusal does not name the revision found |
| `interface/page.rs` | Value object | `Place`, `Placed`, `TaskPage` |
| `repo/page.rs` | Iterator (external) over a Repository | the page after a `Place` as at most three limited store queries |
| `fs-tasks-server/{cursor,facts}.rs` | none | pure functions: a `Place` as bytes; the facts of a problem |
| `fs-tasks-server/paging.rs` | Adapter | a `TaskPage` on the wire, cut by weight |
| `repo/items.rs` | Facade | the trait impls forward to the flows |
| `model/task.rs`, `model/relation.rs` | Template method (`calendar_object!` of `fs-jscalendar`) / Value object | only what is specific to `Task`; `Relation` is a closed enum |
| `model/ops.rs` | Command | small edits of the value |
| `validate/check.rs` | Specification | task rules; the shared rules and `Ctx` are in `fs-jscalendar` |
| `contract/*` | Adapter | generated Cap'n Proto code and the wire <-> Rust mapping |
| `error.rs` | none | plain error enum; one `From` for the errors of `fs-jscalendar` |

Design.md names a `Tasks` facade as the single entry point and a Command pattern with `apply` / `invert` for moves and edits
(ADR 0042): `TaskRepo` is the facade today; undo goes through the store (every write is logged and undoable). The Strategy for
column -> status mapping and ordering arrives with boards.

## Decisions

* **Independent of fs-calendar.** `Task` stands on its own (ADR 0050). What it shares with `Event` lives in `fs-jscalendar`, a plain
  library without data or program, so neither program copies it and neither depends on the other for it.
* **Context `tasks`.** Tasks written by fs-calendar 0.1 have `context=calendar`; they are not found by `TaskRepo`. No data
  existed outside tests, so there is no migration.
* **Assign** adds a participant with role `owner`; assigning the same address twice keeps one participant.
* **Relations** use the `Relation` object of JSCalendar (`relatedTo[uid].relation[key] = true`); keys: `first next child parent`
  (RFC 8984) and `depends-on finishtostart finishtofinish starttostart starttofinish` (RFC 9253). Lead/lag gaps are not modelled.
* **Index times are clamped, not refused:** `0000-01-01T00:00:00` in Auckland is valid JSCalendar, so it stays storable. Its index value is the edge of the range and its envelope keeps the real time (provisional).
* **Task envelope type** is `Task` (AS2 has no task type); content type `application/jscalendar+json`.
* **Wire form of a task**: JSCalendar JSON text plus the envelope; the JSON stays authoritative.

## Tags not to use

* **v0.5.0, v0.5.1.** Both pin `fs-interfaces` v0.22.0, whose `rpc` is superseded (see its `DESIGN.md`). In the server: a `due` of 15 MiB of control characters passes intake, its error escapes to more than a message and ends the connection; a report of an import can do the same, and the report is lost; a title of nearly 16 MiB is accepted, and then `list` answers 413 forever, as the title counts twice. In the core: `:60` is taken in any minute; a time near year 0 or 9999 falls out of every range; a `DURATION` raises a false alarm, while garbage in other properties goes unsaid; a continuation line can end a `VTODO`, and data is lost silently (`fs-jscalendar` before 0.1.3).

## Crates

* `fs-interfaces` (git tag v0.44.0), `fs-objects` (git tag v0.4.0): contract, `Capabilities`, `Unsupported`, the store.
* Frontend only: `fs-admin-frontend`, `fs-components-{app,fields,styles,widgets,kanban,timeline,calendar-grid}` (git `rev` `9950b51`, untagged), `fs-types` / `fs-interfaces` v0.0.2 renamed (temporary).
* `fs-objects` (git tag v0.2.0), `fs-jscalendar` (git tag v0.1.0; it also holds the exact pin of `calcard`).
* `chrono`, `serde`, `serde_json`, `thiserror`, `async-trait`.
* Optional (feature `contract`): `capnp` 0.27.

## Open points

* **`DURATION` of a `VTODO`:** the conversion (`calcard`) makes it `duration`, not `estimatedDuration`, so it is kept and checked but does not count for the time span. Provisional, not changed here.

* **Capability ids (provisional, question to the owner):** `tasks.capnp` names ids only for five methods. For the rest this repo uses `task.read`, `task.query`, `task.delete`, `task.import`, `task.export`, `task.subscribe` and the planned board ids (`board.open`, `board.create`, `column.create`, `column.limit`, `card.move`). They should be written into the schema or replaced.
* **`supportReport`:** the contract has it on the root; the wire answer is `Capabilities::write` (fs-interfaces). The capnp-rpc server is still not written.

* Second consumer of the interface (needed for 1.0): e.g. the project suite or the hub.
* Boards (`Boards` placeholder), `task_link`, column limits, time tracking (design.md open questions).
* Per-instance changes of recurring tasks are preserved (`recurrenceOverrides`) but not expanded: there is no task recurrence engine yet.
* `TaskChange` carries only the id; richer payloads (envelope) come with the capnp-rpc server (`common.capnp` `Change`).

## Open points (0.7.0)

* A taken uid is `Error::Exists(id)`: the problem carries `id` and `kind` of the task that holds it. `Error::is_conflict` is true for it and for `Conflict`; other conflicts (the uid of a task cannot change) name no object.
* A task stored by 0.6.0 has no `order_key`, `id_key` or `description` in its index. `TaskRepo::upgrade_index` completes them (the server binary runs it at start and prints the count and every failure to stderr); it is idempotent, adds only index fields (content, envelope and original stay; it writes one `Update` per task to the log) and one task that cannot be repaired is named in its report and does not stop the others. Until then, and for a task that cannot be repaired, the first page fails with `stored task cannot be used: N of M tasks have no place in the order ...` (`Error::Stored`) instead of giving fewer tasks; `list` and `query` still return them, keyless ones in name order after the others. A library user that does not use the binary calls `upgrade_index` itself after opening. Tested with a store written by the code of tag v0.6.0 (`tests/fixtures/v060.db`, six tasks).
* The cursor holds the title of the last task whole, with no limit of its own: it is as long as a title may be (up to `MAX_TASK_BYTES`, 16 MiB), and a message takes 64 MiB.
* The store sorts all rows that match before it applies a limit (`ORDER BY` a computed key): a page costs the database O(n log n), so reading all pages is O(n² log n) there. `counting.rs` counts the rows the store *returns*, which are linear; it does not measure the work inside the database. Only the page is read, decoded and sent. A stored sort key (a real column or an index the database can walk) would remove it; that is a change in `fs-objects`.
* `description` is indexed whole for `term`, so a large description is stored twice.
* A task without a title is found by the placeholder `(untitled task)`.
* `TaskItems::list` and `query` still return everything (the in-process frontend uses them); only `page` is cut in the store.

## After the sprint (strategy list, 2026-11-07)

* `cursor.rs` is a copy of `fs-wiki-server`'s: move it to `fs-interfaces` (`rpc`).
* `issue` texts that are sentences instead of identifiers break §2a: replace them by identifiers where a file is touched anyway.
