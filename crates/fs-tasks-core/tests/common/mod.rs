//! Shared helpers of the integration tests.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Arc;

use chrono::{DateTime, Utc};
use fs_store::{Store, TursoStore};
use fs_tasks_core::{TaskRepo, utc};

pub const ME: &str = "https://local.invalid/users/owner";

pub const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//x//y//EN\r\nBEGIN:VEVENT\r\nUID:ev-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261130T090000Z\r\nDURATION:PT2H\r\nSUMMARY:An event\r\nEND:VEVENT\r\nBEGIN:VTODO\r\nUID:todo-1\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261112T170000Z\r\nSUMMARY:Minutes\r\nSTATUS:NEEDS-ACTION\r\nPERCENT-COMPLETE:10\r\nEND:VTODO\r\nBEGIN:VTODO\r\nUID:todo-2\r\nDTSTAMP:20261101T000000Z\r\nDUE;TZID=Europe/Berlin:20261120T120000\r\nSUMMARY:Invoice\\, paid\r\nSTATUS:COMPLETED\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

/// A repository over a fresh in-memory store.
pub async fn repo() -> TaskRepo {
    setup().await.0
}

/// The repository and a handle to its store (for undo and count checks).
pub async fn setup() -> (TaskRepo, Arc<dyn Store>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    (TaskRepo::new(store.clone(), ME), store)
}

/// Parse a UTC timestamp.
pub fn t(s: &str) -> DateTime<Utc> {
    utc(s).unwrap()
}
