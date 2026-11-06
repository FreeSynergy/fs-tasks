//! RFC 9253 relation types between tasks.
//!
//! Pattern: Value object (enum) — the closed set of relation names JSCalendar `relatedTo` carries;
//! parsing and printing live in one place, so the wire names cannot drift.

/// How a task relates to another object (the `relation` map of a JSCalendar `Relation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Relation {
    /// The other object is the first of a series (RFC 8984).
    First,
    /// The other object is the next in a series (RFC 8984).
    Next,
    /// The other object is a child (RFC 8984).
    Child,
    /// The other object is a parent (RFC 8984).
    Parent,
    /// This task depends on the other (RFC 9253).
    DependsOn,
    /// This task can start when the other finishes (RFC 9253).
    FinishToStart,
    /// This task can finish when the other finishes (RFC 9253).
    FinishToFinish,
    /// This task can start when the other starts (RFC 9253).
    StartToStart,
    /// This task can finish when the other starts (RFC 9253).
    StartToFinish,
}

impl Relation {
    /// All relations, in declaration order.
    pub const ALL: [Relation; 9] = [
        Self::First,
        Self::Next,
        Self::Child,
        Self::Parent,
        Self::DependsOn,
        Self::FinishToStart,
        Self::FinishToFinish,
        Self::StartToStart,
        Self::StartToFinish,
    ];

    /// The JSCalendar key.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Next => "next",
            Self::Child => "child",
            Self::Parent => "parent",
            Self::DependsOn => "depends-on",
            Self::FinishToStart => "finishtostart",
            Self::FinishToFinish => "finishtofinish",
            Self::StartToStart => "starttostart",
            Self::StartToFinish => "starttofinish",
        }
    }

    /// Read a JSCalendar key.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str() == s)
    }
}
