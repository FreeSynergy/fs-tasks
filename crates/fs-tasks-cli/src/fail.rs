//! Why a command did not work, and which exit code says so.
//!
//! Pattern: Value object — a text, plus whether the cause is a conflict (a claim somebody else holds), which a
//! script tells apart from every other error by the exit code 3.

use std::fmt;

/// A failed command.
#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    /// A claim (or another exclusive hold) of somebody else is the cause.
    pub conflict: bool,
    /// What went wrong, in words; for a conflict it names the holder.
    pub text: String,
}

impl Failure {
    /// A conflict: exit code 3.
    #[must_use]
    pub fn conflict(text: impl Into<String>) -> Self {
        Self {
            conflict: true,
            text: text.into(),
        }
    }

    /// The exit code: 3 for a conflict, 1 for everything else.
    #[must_use]
    pub fn code(&self) -> i32 {
        if self.conflict { 3 } else { 1 }
    }
}

impl From<String> for Failure {
    fn from(text: String) -> Self {
        Self {
            conflict: false,
            text,
        }
    }
}

impl From<&str> for Failure {
    fn from(text: &str) -> Self {
        text.to_string().into()
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}
