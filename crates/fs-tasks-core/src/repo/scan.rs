//! Reading the whole store of tasks by scans.
//!
//! Pattern: Repository (read side) — the scans behind the questions that need the whole graph. They continue by the
//! `published` time, not by a count of rows or an index (see `scan_each`), and refuse a store larger than the limit.

use std::collections::{BTreeMap, BTreeSet};

use fs_objects::{Cond, Op, Order, Query, StoredObject, TimeField};

use super::consts::{CONTEXT, ID_KEY, TASK_TYPE};
use super::crud::entry_of;
use super::deps::{DEPENDS_ON, PRECEDES};
use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::model::Task;
use crate::plan::edges_of;

impl TaskRepo {
    /// Every task of the store, one stored object at a time, in the order of their first storing (`published`, which an
    /// edit does not change). The scan goes on from the last `published` it saw (`>=`, the objects of that instant it has
    /// seen are skipped), not by a count of rows, so an edit made during the scan, which moves a task in the order of
    /// changes, neither hides a task nor shows it twice; a task created during the scan comes at the end of it. A store
    /// with more than `max_scanned` tasks is refused. When more tasks than one chunk share one `published` instant (a
    /// store written with given times), that instant is read on its own by task id (`scan_ties`), then the scan goes on
    /// after it. (The first version continued by the `id_key` index everywhere: an index condition costs a full pass over
    /// the rows each time, 5 s for 5000 tasks in a release build; the time column is as cheap as a plain scan.)
    pub(super) async fn scan_each(
        &self,
        mut each: impl FnMut(StoredObject) -> Result<()>,
    ) -> Result<()> {
        let mut read = 0usize;
        let mut from: Option<fs_objects::Timestamp> = None;
        let mut strict = false; // go on after `from`, not at it (its ties were read by `scan_ties`)
        let mut seen_at_from: BTreeSet<fs_objects::ObjectId> = BTreeSet::new();
        loop {
            let mut q = Query::new()
                .of_type(TASK_TYPE)
                .in_context(CONTEXT)
                .ordered(Order::PublishedAsc)
                .limit(self.scan_chunk);
            if let Some(at) = from {
                q = q.and(Cond::Time {
                    field: TimeField::Published,
                    op: if strict { Op::Gt } else { Op::Ge },
                    value: at,
                });
            }
            let chunk = self.queries.query(&q).await?;
            let full = chunk.len() >= self.scan_chunk;
            let mut progressed = false;
            for object in chunk {
                let published = object.envelope.published;
                if from == Some(published) && seen_at_from.contains(&object.envelope.id) {
                    continue;
                }
                if from != Some(published) {
                    from = Some(published);
                    strict = false;
                    seen_at_from.clear();
                }
                seen_at_from.insert(object.envelope.id);
                progressed = true;
                self.count_read(&mut read)?;
                each(object)?;
            }
            if !progressed {
                match from {
                    // a full chunk of tasks all seen: more of them share this instant than one chunk holds
                    Some(at) if full => {
                        self.scan_ties(at, &mut seen_at_from, &mut read, &mut each)
                            .await?;
                        strict = true;
                    }
                    _ => return Ok(()),
                }
            }
        }
    }

    fn count_read(&self, read: &mut usize) -> Result<()> {
        *read += 1;
        if *read > self.max_scanned {
            return Err(Error::Conflict(format!(
                "the store has more than {} tasks; the dependency graph cannot be read as a whole",
                self.max_scanned
            )));
        }
        Ok(())
    }

    /// The tasks stored at exactly `at`, which did not fit one chunk: read by task id (keyset on the `id_key` index, which
    /// costs a pass over the rows of this one instant, not over the store). Those already `seen` are skipped. A task of
    /// that instant without an id key (written before 0.7.0) cannot be reached this way: the scan then says so instead of
    /// ending short.
    async fn scan_ties(
        &self,
        at: fs_objects::Timestamp,
        seen: &mut BTreeSet<fs_objects::ObjectId>,
        read: &mut usize,
        each: &mut impl FnMut(StoredObject) -> Result<()>,
    ) -> Result<()> {
        let at_instant = |q: Query| {
            q.and(Cond::Time {
                field: TimeField::Published,
                op: Op::Ge,
                value: at,
            })
            .and(Cond::Time {
                field: TimeField::Published,
                op: Op::Le,
                value: at,
            })
        };
        let mut last: Option<String> = None;
        loop {
            let mut q = at_instant(Query::new().of_type(TASK_TYPE).in_context(CONTEXT))
                .order_by_index(ID_KEY)
                .limit(self.scan_chunk);
            if let Some(id) = &last {
                q = q.index_op(ID_KEY, Op::Gt, id);
            }
            let chunk = self.queries.query(&q).await?;
            if chunk.is_empty() {
                break;
            }
            for object in chunk {
                last = object.index.get(ID_KEY).and_then(|v| v.first()).cloned();
                if seen.insert(object.envelope.id) {
                    self.count_read(read)?;
                    each(object)?;
                }
            }
            if last.is_none() {
                break;
            }
        }
        let total = self
            .queries
            .count(&at_instant(
                Query::new().of_type(TASK_TYPE).in_context(CONTEXT),
            ))
            .await?;
        if seen.len() < total {
            return Err(Error::Stored(format!(
                "{} tasks stored at the same instant could not be read (no id key, written before 0.7.0); run upgrade_index and read its report",
                total - seen.len()
            )));
        }
        Ok(())
    }

    /// Every task of the store as the stored object (index included).
    pub(super) async fn scan_objects(&self) -> Result<Vec<StoredObject>> {
        let mut all = Vec::new();
        self.scan_each(|o| {
            all.push(o);
            Ok(())
        })
        .await?;
        Ok(all)
    }

    /// Every task that holds a dependency and every task one of them names: the whole graph of the store, read by one
    /// scan; only these tasks are parsed and kept (at most `max_loaded`).
    pub(super) async fn scan_graph(&self) -> Result<BTreeMap<String, Task>> {
        let mut holders: BTreeMap<String, Task> = BTreeMap::new();
        let mut others: BTreeMap<String, Task> = BTreeMap::new();
        self.scan_each(|object| {
            let holds =
                object.index.contains_key(PRECEDES) || object.index.contains_key(DEPENDS_ON);
            let task = entry_of(object)?.task;
            if holds {
                holders.insert(task.uid.clone(), task);
            } else {
                others.insert(task.uid.clone(), task);
            }
            Ok(())
        })
        .await?;
        let named: BTreeSet<String> = holders
            .values()
            .flat_map(|t| {
                edges_of(std::slice::from_ref(t))
                    .into_iter()
                    .flat_map(|e| [e.from, e.to])
            })
            .collect();
        for (uid, task) in others {
            if named.contains(&uid) {
                holders.insert(uid, task);
            }
        }
        if holders.len() > self.max_loaded {
            return Err(self.too_big());
        }
        Ok(holders)
    }
}
