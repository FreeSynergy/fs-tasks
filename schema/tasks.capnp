# fs-tasks contract — DRAFT 0.1 (2026-11-06)
#
# STATUS: DRAFT. Interim location: this file lives in the fs-tasks repo for now. It moves to its
# own interfaces repo (ADR 0045, `fs-interfaces`) if the owner approves that.
#
# Target rules (ADR 0045): the interface is a target, not a snapshot. It stays DRAFT until two
# independent consumers have tested it, then it becomes 1.0, and from then on only version jumps.
# Consumers so far: fs-calendar (1 of 2).
#
# Source: docs/en/programs/apps/tasks/design.md (interface sketch). The Rust form of `Items` and of
# the subscription is the trait pair `TaskItems` / `TaskFeed` in crates/fs-tasks-core (ADR 0043:
# trait first). A task travels as its JSCalendar 2.0 JSON text, which stays authoritative
# (the standards are JSON; ADR 0014, 0050), next to the AS2 envelope.
#
# Evolution rules (Cap'n Proto): never renumber or remove fields/methods; only append.
# Compiled with common.capnp by capnpc-embedded (see crates/fs-tasks-core/build.rs).

@0xc04d258971543476;

using C = import "common.capnp";
using Uuid = C.Uuid;
using Timestamp = C.Timestamp;
using Cursor = C.Cursor;
using Envelope = C.Envelope;
using CapabilityList = C.CapabilityList;
using ChangeListener = C.ChangeListener;
using Subscription = C.Subscription;

# RFC 9253 relation of a task to another object (JSCalendar `relatedTo`)
enum Relation {
  first          @0;
  next           @1;
  child          @2;
  parent         @3;
  dependsOn      @4;
  finishToStart  @5;
  finishToFinish @6;
  startToStart   @7;
  startToFinish  @8;
}

struct Task {
  envelope   @0 :Envelope;        # AS2 `Task` envelope (id, iri, name, context "tasks", times)
  jscalendar @1 :Text;            # the JSCalendar 2.0 `Task` as JSON (authoritative content)
}

# one page of tasks, ordered by due date (undated last)
struct TaskPage { items @0 :List(Task); next @1 :Cursor; }

# all members optional; unset (null / empty) = no restriction; combined with AND
struct TaskFilter {
  dueFrom  @0 :Timestamp;         # due at or after
  dueTo    @1 :Timestamp;         # due before (half-open range)
  progress @2 :Text;              # needs-action | in-process | completed | failed | cancelled
  label    @3 :Text;              # JSCalendar keyword
}

struct ImportReport {
  created @0 :List(Uuid);
  skipped @1 :List(Text);         # uids that already existed
  failed  @2 :List(Text);
  notes   @3 :List(Text);
}

# root capability (capability ids: "task.create", "task.edit", "task.assign", "relation.create", "label.assign")
interface Tasks {
  capabilities @0 () -> (capabilities :CapabilityList);
  items        @1 () -> (items :Items);
  subscribe    @2 (listener :ChangeListener) -> (subscription :Subscription);
  boards       @3 () -> (boards :Boards);      # placeholder, see below
}

interface Items {
  create         @0  (jscalendar :Text) -> (task :Task);                                   # task.create
  edit           @1  (id :Uuid, jscalendar :Text) -> (task :Task);                         # task.edit
  assign         @2  (id :Uuid, name :Text, email :Text) -> (task :Task);                  # task.assign
  createRelation @3  (id :Uuid, otherUid :Text, relation :Relation) -> (task :Task);       # relation.create
  assignLabel    @4  (id :Uuid, label :Text) -> (task :Task);                              # label.assign
  get            @5  (id :Uuid) -> (found :Bool, task :Task);
  getByUid       @6  (uid :Text) -> (found :Bool, task :Task);
  delete         @7  (id :Uuid) -> ();
  list           @8  (cursor :Cursor) -> (page :TaskPage);
  query          @9  (filter :TaskFilter, cursor :Cursor) -> (page :TaskPage);             # time range, status, label
  importIcs      @10 (text :Text) -> (report :ImportReport);                               # VTODO
  exportIcs      @11 (ids :List(Uuid)) -> (text :Text);
}

# PLACEHOLDER — declared so the root can hand it out; NOT implemented and without methods yet.
# Planned (design.md): board.open, board.create, column.create, column.limit, card.move.
# Moving a card is an `Update` on `task_link`, never on the task (projects/technical.md).
interface Boards {
}
