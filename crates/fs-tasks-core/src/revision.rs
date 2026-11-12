//! The revision of a stored task, for compare-and-set.
//!
//! Pattern: Newtype — a number that is only ever compared for equality, so it cannot be taken for a
//! counter, a time or an order. Why this pattern: the contract (`tasks.capnp`, `Task.revision`) says
//! "compare for equality only"; a type with `==` and no `<` keeps the code to that. The trait is
//! [`TaskItems::edit_checked`](crate::TaskItems::edit_checked) (Repository with an optimistic lock): a
//! writer hands back the revision it read, and the task is replaced only if it still has it.
//!
//! The revision is not stored: it is derived from what an answer holds, the envelope's `updated`
//! (microseconds) and the task as JSON (FNV-1a), so two writes within one tick still differ and there is
//! nothing to migrate. It changes at every write that changes the state; an undo in the store may bring an
//! earlier one back, and the same state always has the same revision, so no write is lost by that.

use std::fmt;

use crate::interface::TaskEntry;

/// What a task was when it was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Revision(u64);

impl Revision {
    /// The revision of the entry as stored now.
    pub fn of(entry: &TaskEntry) -> Self {
        let step = |hash: u64, byte: u8| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in entry.envelope.updated.timestamp_micros().to_le_bytes() {
            hash = step(hash, byte);
        }
        for byte in entry.task.to_value().to_string().bytes() {
            hash = step(hash, byte);
        }
        Self(hash)
    }

    /// The revision of any stored object, from its envelope's `updated` and its content as text (a board).
    pub fn of_parts(updated: &fs_objects::Timestamp, content: &str) -> Self {
        let step = |hash: u64, byte: u8| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in updated.timestamp_micros().to_le_bytes() {
            hash = step(hash, byte);
        }
        for byte in content.bytes() {
            hash = step(hash, byte);
        }
        Self(hash)
    }

    /// The revision as it travels over a process boundary (`Task.revision`).
    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for Revision {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}
