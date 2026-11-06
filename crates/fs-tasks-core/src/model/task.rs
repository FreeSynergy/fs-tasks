//! The typed JSCalendar 2.0 `Task`.
//!
//! Pattern: Value object — plain serde data with lossless `extra` (unknown and vendor
//! properties survive a round trip); behaviour on top lives in `ops.rs`. The members mirror the
//! JSCalendar property names one to one, so they are not documented one by one.

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::consts::{Extra, Map, VERSION, is_empty_map};
use super::participant::Participant;
use super::rule::RecurrenceRule;
use crate::clock::now_utc;
use crate::error::Result;

typed_object!(
    /// A JSCalendar `Task`: owned by `fs-tasks` (ADR 0050).
    Task {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub version: Option<String>,
        pub uid: String,
        #[serde(rename = "prodId", skip_serializing_if = "Option::is_none")]
        pub prod_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub created: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub updated: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sequence: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub locale: Option<String>,
        /// `LocalDateTime`, interpreted in `timeZone` (floating if absent).
        #[serde(skip_serializing_if = "Option::is_none")]
        pub start: Option<String>,
        #[serde(rename = "timeZone", skip_serializing_if = "Option::is_none")]
        pub time_zone: Option<String>,
        #[serde(rename = "showWithoutTime", skip_serializing_if = "Option::is_none")]
        pub show_without_time: Option<bool>,
        #[serde(rename = "freeBusyStatus", skip_serializing_if = "Option::is_none")]
        pub free_busy_status: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub privacy: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub priority: Option<i32>,
        #[serde(rename = "recurrenceId", skip_serializing_if = "Option::is_none")]
        pub recurrence_id: Option<String>,
        #[serde(rename = "recurrenceIdTimeZone", skip_serializing_if = "Option::is_none")]
        pub recurrence_id_time_zone: Option<String>,
        #[serde(rename = "recurrenceRule", skip_serializing_if = "Option::is_none")]
        pub recurrence_rule: Option<RecurrenceRule>,
        /// Key: `LocalDateTime` of the instance; value: patch object.
        #[serde(rename = "recurrenceOverrides", skip_serializing_if = "is_empty_map")]
        pub recurrence_overrides: Map<Value>,
        #[serde(skip_serializing_if = "is_empty_map")]
        pub participants: Map<Participant>,
        /// RFC 9253 relations: key = `uid` of the other object, value = `Relation`.
        #[serde(rename = "relatedTo", skip_serializing_if = "is_empty_map")]
        pub related_to: Map<Value>,
        /// Labels.
        #[serde(skip_serializing_if = "is_empty_map")]
        pub keywords: Map<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub due: Option<String>,
        #[serde(rename = "estimatedDuration", skip_serializing_if = "Option::is_none")]
        pub estimated_duration: Option<String>,
        #[serde(rename = "percentComplete", skip_serializing_if = "Option::is_none")]
        pub percent_complete: Option<u8>,
        /// `needs-action` / `in-process` / `completed` / `failed` / `cancelled`
        #[serde(skip_serializing_if = "Option::is_none")]
        pub progress: Option<String>,
        #[serde(rename = "progressUpdated", skip_serializing_if = "Option::is_none")]
        pub progress_updated: Option<String>,
    }
);

impl Task {
    /// The JSCalendar `@type`.
    pub const TYPE: &'static str = "Task";

    /// A new task: fresh `uid` (`urn:uuid:` UUIDv7), progress `needs-action`.
    pub fn new(title: impl Into<String>) -> Self {
        let now = now_utc();
        let mut t = Task {
            uid: format!("urn:uuid:{}", fs_store::ObjectId::new()),
            title: Some(title.into()),
            created: Some(now.clone()),
            updated: Some(now),
            progress: Some("needs-action".into()),
            ..Default::default()
        };
        t.normalise();
        t
    }

    /// Set `@type` and `version` if missing.
    pub fn normalise(&mut self) {
        self.type_.get_or_insert_with(|| Self::TYPE.to_string());
        self.version.get_or_insert_with(|| VERSION.to_string());
    }

    /// `title`, or a placeholder (the envelope `name` may never be empty, ADR 0032).
    pub fn display_title(&self) -> String {
        match self.title.as_deref().map(str::trim) {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => "(untitled task)".to_string(),
        }
    }

    /// The task as a JSON value (the stored content).
    ///
    /// # Panics
    /// Never in practice: the task holds only JSON-compatible data.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("serialises")
    }

    /// Read a task from JSON.
    pub fn from_value(v: Value) -> Result<Self> {
        Ok(serde_json::from_value(v)?)
    }
}
