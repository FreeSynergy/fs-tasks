//! Claims: taking a ready task so that nobody else takes it too.
//!
//! Pattern: Optimistic lock (compare and set under the write turn) — the check and the write are one step in this
//! provider process; across devices (no referee) the claim is only an announcement, and the idempotency of the work
//! itself has to make a doubled claim harmless (see the plan, N3).

use chrono::{DateTime, Duration, Utc};
use fs_objects::ObjectId;

use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::TaskEntry;
use crate::model::{MAX_CLAIM_HOURS, check_actor};
use crate::now_utc;
use crate::time::parse_utc;

impl TaskRepo {
    pub(super) async fn claim_inner(
        &self,
        id: ObjectId,
        by: &str,
        until: DateTime<Utc>,
    ) -> Result<TaskEntry> {
        let now = parse_utc(&now_utc()).map_err(Error::from)?;
        if until <= now {
            return Err(Error::Conflict(
                "a claim needs an actor IRI and an end in the future".into(),
            ));
        }
        check_actor(by, "").map_err(Error::Conflict)?;
        if until > now + Duration::hours(MAX_CLAIM_HOURS) {
            return Err(Error::Conflict(format!(
                "a claim ends within {MAX_CLAIM_HOURS} hours"
            )));
        }
        let by = by.to_string();
        self.writing(async || {
            let mut task = self.entry(id).await?.ok_or(Error::NotFound(id))?.task;
            if let Some(held) = task.claimed_at(now)
                && held.by != by
            {
                return Err(Error::Conflict(format!(
                    "the task is claimed by {} until {}",
                    held.by, held.until
                )));
            }
            task.set_claim(&by, until);
            self.edit_inner(id, task).await
        })
        .await
    }

    pub(super) async fn release_inner(&self, id: ObjectId, by: &str) -> Result<TaskEntry> {
        let now = parse_utc(&now_utc()).map_err(Error::from)?;
        self.writing(async || {
            let mut task = self.entry(id).await?.ok_or(Error::NotFound(id))?.task;
            if let Some(held) = task.claimed_at(now)
                && held.by != by
            {
                return Err(Error::Conflict(format!(
                    "the claim belongs to {}, not to {by}",
                    held.by
                )));
            }
            task.clear_claim();
            self.edit_inner(id, task).await
        })
        .await
    }
}
