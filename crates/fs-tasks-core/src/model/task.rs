//! The typed JSCalendar 2.0 `Task`.
//!
//! Pattern: Template method (macro form) — `calendar_object!` of `fs-jscalendar` supplies the shared
//! members and methods; this file adds what is specific; behaviour on top lives in `ops.rs`. The
//! members mirror the JSCalendar property names one to one, so they are not documented one by one.

#![allow(missing_docs)]

use crate::now_utc;

fs_jscalendar::calendar_object!(
    /// A JSCalendar `Task`: owned by `fs-tasks` (ADR 0050).
    Task, "Task" {
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
    /// A new task: fresh `uid` (`urn:uuid:` UUIDv7), progress `needs-action`.
    pub fn new(title: impl Into<String>) -> Self {
        let now = now_utc();
        let mut t = Task {
            uid: fs_jscalendar::new_uid(),
            title: Some(title.into()),
            created: Some(now.clone()),
            updated: Some(now),
            progress: Some("needs-action".into()),
            ..Default::default()
        };
        t.normalise();
        t
    }
}
