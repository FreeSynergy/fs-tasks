//! Which of the two screens is shown.
//!
//! Pattern: State — one value both screens read and either may change; the tab "List" of the board and the entry
//! "Board" of the list switch it.

use std::sync::atomic::{AtomicBool, Ordering};

/// The mode shared by the screens: `true` is the day list.
#[derive(Default)]
pub struct Mode(AtomicBool);

impl Mode {
    /// Show the day list (`true`) or the board.
    pub fn set_list(&self, list: bool) {
        self.0.store(list, Ordering::SeqCst);
    }

    /// Is the day list shown?
    pub fn is_list(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
