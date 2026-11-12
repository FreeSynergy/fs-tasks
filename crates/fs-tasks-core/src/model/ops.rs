//! What a task can do to itself: assignment, relations, labels, status.
//!
//! Pattern: Command (in-memory, no I/O) — each method is one small edit of the value; the
//! repository applies them under `edit` and stores the result.

use fs_jscalendar::model::{Participant, next_participant_key};
use serde_json::{Value, json};

use super::gap::{GAP_KEY, parse_gap};
use super::relation::Relation;
use super::task::Task;

/// RFC 10051 task status has no JSCalendar form yet.
pub const TASK_STATUS_KEY: &str = "freesynergy.net:taskStatus";

fn progress_of(p: &Participant) -> Option<&str> {
    p.extra.get("progress").and_then(Value::as_str)
}

impl Task {
    /// The progress, also when omitted: RFC 8984 §5.2.5 computes it from the participants, in this
    /// order. A task without participants has no one who completed it, so it needs action.
    pub fn effective_progress(&self) -> &str {
        if let Some(p) = self.progress.as_deref() {
            return p;
        }
        let any = |wanted: &str| {
            self.participants
                .values()
                .any(|p| progress_of(p) == Some(wanted))
        };
        if !self.participants.is_empty()
            && self
                .participants
                .values()
                .all(|p| progress_of(p) == Some("completed"))
        {
            "completed"
        } else if any("failed") {
            "failed"
        } else if any("in-process") {
            "in-process"
        } else {
            "needs-action"
        }
    }

    /// RFC 10051 task status (stored as the vendor property `freesynergy.net:taskStatus`).
    pub fn task_status(&self) -> Option<&str> {
        self.vendor(TASK_STATUS_KEY).and_then(Value::as_str)
    }

    /// Set the RFC 10051 task status.
    pub fn set_task_status(&mut self, status: &str) {
        self.set_vendor(TASK_STATUS_KEY, json!(status));
    }

    /// Assign the task to a person (role `owner`); returns the participant key. Assigning the
    /// same address twice keeps the one participant.
    pub fn assign(&mut self, name: &str, email: &str) -> String {
        if let Some(k) = self.participant_by_email(email) {
            let key = k.to_string();
            if let Some(p) = self.participants.get_mut(&key) {
                p.roles.insert("owner".into(), true);
            }
            return key;
        }
        let key = next_participant_key(&self.participants);
        self.participants
            .insert(key.clone(), Participant::person(name, email, "owner"));
        key
    }

    /// Add an RFC 9253 relation to the object with `uid`; relations to the same `uid` merge.
    pub fn relate(&mut self, uid: &str, relation: Relation) {
        let entry = self
            .related_to
            .entry(uid.to_string())
            .or_insert_with(|| json!({"@type": "Relation", "relation": {}}));
        if let Some(map) = entry.get_mut("relation").and_then(Value::as_object_mut) {
            // An imported `FINISHTOSTART` is the same relation as our `finishtostart`: no second key.
            // (a key set to `false` does not count: it is switched on below).
            if map
                .iter()
                .any(|(k, on)| on.as_bool() == Some(true) && Relation::parse(k) == Some(relation))
            {
                return;
            }
            map.insert(relation.as_str().to_string(), Value::Bool(true));
        }
    }

    /// Like [`Task::relate`], with a lag (positive) or lead (negative) between the two tasks, an ISO 8601 duration such as
    /// `P1D` or `-PT4H`, kept as the member `freesynergy.net:gap` of the relation and mirrored where the iCalendar
    /// conversion keeps the `GAP` parameter, so the export writes it. Empty = no gap (any earlier one stays).
    ///
    /// # Errors
    /// The gap is not a duration or is too large, or the relation is not one of the four temporal ones.
    pub fn relate_with_gap(
        &mut self,
        uid: &str,
        relation: Relation,
        gap: &str,
    ) -> crate::Result<()> {
        if gap.is_empty() {
            self.relate(uid, relation);
            return Ok(());
        }
        if !relation.is_temporal() {
            return Err(crate::Error::Conflict(format!(
                "a gap means something only for the temporal relations, not for {}",
                relation.as_str()
            )));
        }
        parse_gap(gap)?;
        self.relate(uid, relation);
        if let Some(entry) = self.related_to.get_mut(uid).and_then(Value::as_object_mut) {
            entry.insert(GAP_KEY.into(), json!(gap));
        }
        self.mirror_gap(uid);
        Ok(())
    }

    /// The gap of the relation to `uid` (zero if there is none or it cannot be read).
    pub fn relation_gap(&self, uid: &str) -> chrono::Duration {
        self.related_to
            .get(uid)
            .and_then(|v| v.get(GAP_KEY))
            .and_then(Value::as_str)
            .and_then(|g| parse_gap(g).ok())
            .unwrap_or_else(chrono::Duration::zero)
    }

    /// Put the gap of the relation to `uid` where the iCalendar conversion keeps parameters
    /// (`iCalendar.convertedProperties["relatedTo/<uid>"].parameters.gap`).
    pub(crate) fn mirror_gap(&mut self, uid: &str) {
        let Some(gap) = self
            .related_to
            .get(uid)
            .and_then(|v| v.get(GAP_KEY))
            .cloned()
        else {
            return;
        };
        let ical = self
            .extra
            .entry("iCalendar".to_string())
            .or_insert_with(|| json!({"name": "vtodo"}));
        if let Some(obj) = ical.as_object_mut() {
            let converted = obj
                .entry("convertedProperties")
                .or_insert_with(|| json!({}));
            if let Some(c) = converted.as_object_mut() {
                let slot = c
                    .entry(format!("relatedTo/{uid}"))
                    .or_insert_with(|| json!({}));
                if let Some(slot) = slot.as_object_mut() {
                    let parameters = slot.entry("parameters").or_insert_with(|| json!({}));
                    if let Some(p) = parameters.as_object_mut() {
                        p.insert("gap".into(), gap);
                    }
                }
            }
        }
    }

    /// Read the gaps the iCalendar conversion kept as parameters into the member `freesynergy.net:gap` of each
    /// relation (import), so the member is the one place a reader looks.
    pub(crate) fn adopt_converted_gaps(&mut self) {
        let found: Vec<(String, String)> = self
            .extra
            .get("iCalendar")
            .and_then(|i| i.get("convertedProperties"))
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| {
                let uid = k.strip_prefix("relatedTo/")?;
                let gap = v.get("parameters")?.get("gap")?.as_str()?;
                Some((uid.to_string(), gap.to_string()))
            })
            .collect();
        for (uid, gap) in found {
            if let Some(entry) = self.related_to.get_mut(&uid).and_then(Value::as_object_mut) {
                entry
                    .entry(GAP_KEY.to_string())
                    .or_insert_with(|| json!(gap));
            }
        }
    }

    /// The relations of this task as `(uid, relation)` pairs, in `uid` order.
    pub fn relations(&self) -> Vec<(String, Relation)> {
        self.related_to
            .iter()
            .flat_map(|(uid, v)| {
                let keys = v.get("relation").and_then(Value::as_object);
                keys.into_iter()
                    .flatten()
                    .filter(|(_, on)| on.as_bool() == Some(true))
                    .filter_map(|(k, _)| Relation::parse(k))
                    .map(|r| (uid.clone(), r))
            })
            .collect()
    }

    /// Attach a label (JSCalendar `keywords`).
    pub fn label(&mut self, label: &str) {
        self.keywords.insert(label.to_string(), true);
    }
}
