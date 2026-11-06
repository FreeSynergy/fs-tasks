//! What a task can do to itself: vendor properties, assignment, relations, labels, status.
//!
//! Pattern: Command (in-memory, no I/O) — each method is one small edit of the value; the
//! repository applies them under `edit` and stores the result.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::consts::TASK_STATUS_KEY;
use super::participant::Participant;
use super::relation::Relation;
use super::task::Task;

impl Task {
    /// Vendor / unknown property.
    pub fn vendor(&self, key: &str) -> Option<&Value> {
        self.extra.get(key)
    }

    /// Set a vendor / unknown property.
    pub fn set_vendor(&mut self, key: &str, v: Value) {
        self.extra.insert(key.to_string(), v);
    }

    /// RFC 10051 task status (stored as the vendor property `freesynergy.net:taskStatus`).
    pub fn task_status(&self) -> Option<&str> {
        self.vendor(TASK_STATUS_KEY).and_then(Value::as_str)
    }

    /// Set the RFC 10051 task status.
    pub fn set_task_status(&mut self, status: &str) {
        self.set_vendor(TASK_STATUS_KEY, json!(status));
    }

    /// Key of the participant with this e-mail address (case-insensitive).
    pub fn participant_by_email(&self, email: &str) -> Option<&str> {
        let want = bare(email);
        self.participants.iter().find_map(|(k, p)| {
            let e = p.email.as_deref().or(p.calendar_address.as_deref())?;
            (bare(e) == want).then_some(k.as_str())
        })
    }

    /// Assign the task to a person (role `owner`); returns the participant key. Assigning the
    /// same address twice keeps the one participant.
    ///
    /// # Panics
    /// Never: among `n + 1` candidate keys at most `n` are taken.
    pub fn assign(&mut self, name: &str, email: &str) -> String {
        if let Some(k) = self.participant_by_email(email) {
            let key = k.to_string();
            if let Some(p) = self.participants.get_mut(&key) {
                p.roles.insert("owner".into(), true);
            }
            return key;
        }
        let key = (1..=self.participants.len() + 1)
            .map(|i| format!("p{i}"))
            .find(|k| !self.participants.contains_key(k))
            .expect("one of n + 1 candidate keys is free");
        self.participants.insert(
            key.clone(),
            Participant {
                type_: Some("Participant".into()),
                name: Some(name.into()),
                email: Some(email.into()),
                calendar_address: Some(format!("mailto:{email}")),
                kind: Some("individual".into()),
                roles: BTreeMap::from([("owner".to_string(), true)]),
                participation_status: Some("needs-action".into()),
                ..Default::default()
            },
        );
        key
    }

    /// Add an RFC 9253 relation to the object with `uid`; relations to the same `uid` merge.
    pub fn relate(&mut self, uid: &str, relation: Relation) {
        let entry = self
            .related_to
            .entry(uid.to_string())
            .or_insert_with(|| json!({"@type": "Relation", "relation": {}}));
        if let Some(map) = entry.get_mut("relation").and_then(Value::as_object_mut) {
            map.insert(relation.as_str().to_string(), Value::Bool(true));
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

fn bare(address: &str) -> String {
    address.trim().trim_start_matches("mailto:").to_lowercase()
}
