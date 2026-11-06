# fs-tasks – Design

`fs-tasks` is the tasks program of FreeSynergy and the **owner of the JSCalendar `Task`** (ADR 0050: one standard, one
owning program, one interface). This repo holds the **program core** (`crates/fs-tasks-core`, no GUI) and the contract
(`schema/`). Other programs, e.g. `fs-calendar`, use tasks **only through the interface** (`TaskItems`, `TaskFeed`) and have
no Task model of their own. Design of the whole program: `fs-documentation/docs/en/programs/apps/tasks/design.md`.

## Interface status

**Interface status: draft, consumers: fs-calendar (1 of 2).** (ADR 0045: the interface is a target. It stays draft until two
independent consumers have tested it, then 1.0, then only version jumps.) The contract is `schema/tasks.capnp` (marked DRAFT,
interim location: it moves to its own interfaces repo if the owner approves). The Rust form comes first (ADR 0043): the traits
`TaskItems` and `TaskFeed`.

## State

Version 0.1.0, local only (no remote yet; the owner creates the GitHub repo).

| Area | State |
|---|---|
| Typed `Task` (serde), lossless unknown/vendor properties (`extra`) | done (moved from fs-calendar-core) |
| Validation (`version` "2.0", `uid`, LocalDateTime without zone suffix/fraction, IANA zones only, durations, `progress`, `percentComplete`, single `recurrenceRule`, count XOR until, override keys, participant status, obsolete 1.0 members refused) | done, all issues at once |
| `upgrade()` RFC 8984 -> 2.0 (rule list, legacy members, `Z` suffix, fractions, custom zones; idempotent; Groups) | done |
| RFC 9253 relations: `Relation` enum, `Task::relate` / `relations` (`relatedTo`) | done; unknown relation keys from other writers are kept, not rejected |
| RFC 10051 status as vendor property `freesynergy.net:taskStatus` (`task_status` / `set_task_status`, index `task_status`) | done (preserved, not interpreted) |
| Interface `TaskItems` (create, edit, assign, relate, label, get, get_by_uid, delete, list, query by due range / progress / label, VTODO import/export, capabilities) | done, draft |
| Interface `TaskFeed` (Observer: `subscribe(listener)` -> `Subscription`; `Created` / `Updated` / `Deleted`) | done, draft |
| `TaskRepo` over fs-store 0.2 (`ObjectRepository` + `ObjectQuery` only; `TaskRepo::with_backend`) | done |
| Envelope: type `Task`, `name` = `title` (ADR 0032), **`context=tasks`**, IRI `https://<instance>/tasks/<id>`, `startTime` / `endTime` (due date, else start + estimate, else start) | done |
| Index fields: `uid`, `type`, `start_utc`, `end_utc`, `due_utc`, `progress`, `task_status`, `label`, `participant_email`, `tzdb_version` (imports) | done |
| Queries (`TaskQuery` -> one store query: due range `[from, to)` with open ends, progress, label; ordered by due, undated last) | done |
| ICS VTODO import / export via `calcard =0.3.14`; original kept per UID (`text/calendar`); `VEVENT`s of the same file are ignored (they belong to fs-calendar) | done |
| Contract `schema/tasks.capnp`: root `Tasks` (`capabilities`, `items`, `subscribe`, `boards`), `Items` (create, edit, assign, createRelation, assignLabel, get, getByUid, delete, list, query, importIcs, exportIcs), `Boards` **placeholder, no methods, not implemented** | done, DRAFT |
| Generated Rust (`capnpc-embedded` 0.2 + `capnpc` 0.27, no C++ toolchain) committed in `src/contract/`; feature `contract` compiles it, `regenerate` rebuilds it (`cargo build -p fs-tasks-core --features regenerate`) | done; reproduces byte for byte |
| Wire conversions (`contract::{write_filter, read_filter, relation_*, write_capabilities}`) | done; the capnp-rpc server is not written yet |
| Boards, columns, `task_link`, card moves (`Update` on `task_link`, never on the task) | not started (design.md; `Boards` is a placeholder) |
| Tests | 18 (16 without the `contract` feature) |
| ADR 0043 | every file <= 200 lines, `//! Pattern:` in every module, `#![deny(missing_docs)]`, `#![warn(clippy::pedantic)]` clean |

## Structure and pattern map (ADR 0043)

| Module | Pattern | Why |
|---|---|---|
| `interface/items.rs` | Repository (interface) | `TaskItems`: the program's public contract; consumers use `dyn TaskItems` |
| `interface/feed.rs` | Observer | `TaskFeed`, `TaskListener`, `Registry` (subject), `Subscription` (drop = unsubscribe) |
| `interface/types.rs` | Value objects / Builder | `TaskEntry`, `ImportReport`, `TaskQuery` (builder methods) |
| `repo/handle.rs` | Repository + Facade | `TaskRepo`: hides store, envelope rules, formats |
| `repo/backend.rs` | Adapter | `Arc<dyn Store>` presented as `dyn ObjectRepository` + `dyn ObjectQuery` (interface segregation) |
| `repo/{crud,edit,queries}.rs` | Repository / Template method | writes, sequence-aware edit, filtered queries |
| `repo/edit.rs` (`modify`) | Command (client) | `assign` / `relate` / `label` are in-memory commands on the `Task` applied through the edit flow |
| `repo/{build,span}.rs` | Adapter | task -> AS2 envelope + index fields; time span from `start` / `due` / `estimatedDuration` |
| `repo/transfer.rs`, `ical/*` | Template method / Adapter | fixed import flow; iCalendar (calcard) behind it |
| `repo/items.rs` | Facade | the trait impls forward to the flows |
| `model/*` | Value objects / Template method (macro) | typed JSCalendar with lossless `extra`; `Relation` is a closed enum |
| `model/ops.rs` | Command | small edits of the value |
| `validate/*` | Specification | JSCalendar 2.0 rules reporting into one `Ctx` |
| `upgrade.rs` | Pipeline | RFC 8984 -> 2.0 rewrite steps, idempotent |
| `time/*` | Strategy + Factory + Value objects | `ZoneSource` (system / bundled tzdb), `Zone`, `Dur` |
| `contract/*` | Adapter | generated Cap'n Proto code and the wire <-> Rust mapping |
| `error.rs`, `clock.rs` | none | plain error enum, pure helper |

Design.md names a `Tasks` facade as the single entry point and a Command pattern with `apply` / `invert` for moves and edits
(ADR 0042): `TaskRepo` is the facade today; undo goes through the store (every write is logged and undoable). The Strategy for
column -> status mapping and ordering arrives with boards.

## Decisions

* **Independent of fs-calendar.** The time module (`Zone`, `Dur`, tzdb choice), the `Participant` / `RecurrenceRule` value
  types and the ICS splitting are small copies of what fs-calendar-core has. This costs duplication, but `Task` stands on its own
  (ADR 0050). Open: a shared `jscalendar-common` crate would remove it (needs the owner's decision on a new repo).
* **Context `tasks`.** Tasks written by fs-calendar 0.1 have `context=calendar`; they are not found by `TaskRepo`. No data
  existed outside tests, so there is no migration.
* **Assign** adds a participant with role `owner`; assigning the same address twice keeps one participant.
* **Relations** use the `Relation` object of JSCalendar (`relatedTo[uid].relation[key] = true`); keys: `first next child parent`
  (RFC 8984) and `depends-on finishtostart finishtofinish starttostart starttofinish` (RFC 9253). Lead/lag gaps are not modelled.
* **Task envelope type** is `Task` (AS2 has no task type); content type `application/jscalendar+json`.
* **Wire form of a task**: JSCalendar JSON text plus the envelope; the JSON stays authoritative.

## Crates

* `fs-store` (git tag v0.2.0). `calcard =0.3.14` (Apache-2.0 OR MIT), pinned exactly: later Stalwart releases are AGPL.
* `jiff` + `jiff-tzdb` (zones, system tzdb preferred, bundled fallback), `chrono`, `serde`, `serde_json`, `thiserror`, `async-trait`.
* Optional: `capnp` 0.27 (feature `contract`); build only: `capnpc` 0.27, `capnpc-embedded` 0.2 (feature `regenerate`).

## Open points

* Second consumer of the interface (needed for 1.0): e.g. the project suite or the hub.
* Where the contract lives (this repo for now; ADR 0050 asks about `fs-interfaces`).
* Boards (`Boards` placeholder), `task_link`, column limits, time tracking (design.md open questions).
* Per-instance changes of recurring tasks are preserved (`recurrenceOverrides`) but not expanded: there is no task recurrence engine yet.
* `TaskChange` carries only the id; richer payloads (envelope) come with the capnp-rpc server (`common.capnp` `Change`).
