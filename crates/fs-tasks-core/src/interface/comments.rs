//! The `Comments` contract of the tasks program: comments on a task, as the topic `comments` (draft 0.1 in fs-interfaces).
//!
//! Pattern: Repository (interface) + Null Object — like `TaskItems`: a provider that cannot do it answers
//! `Unsupported`. A comment is an AS2 `Note` that is its own object, never a member of the task: its `context` is the
//! IRI of the task, its `inReplyTo` the IRI of the comment it answers, its author an actor IRI set by the provider
//! (the caller cannot name one). Deleting leaves an AS2 `Tombstone`, so a thread does not break. Threads are one level.

use async_trait::async_trait;
use fs_objects::{Envelope, ObjectId};

use super::items::not_possible;
use crate::error::Result;
use crate::revision::Revision;

/// Capability ids of the topic (`comment.subscribe` is the feed: [`TaskChange::Commented`](super::TaskChange)).
pub const COMMENT_CAPABILITIES: [&str; 4] = [
    "comment.list",
    "comment.add",
    "comment.edit",
    "comment.delete",
];

/// A comment as stored.
#[derive(Clone, Debug, PartialEq)]
pub struct Comment {
    /// The object id.
    pub id: ObjectId,
    /// Its AS2 envelope: `context`, `in_reply_to`, `attributed_to` (the author), times, `iri`.
    pub envelope: Envelope,
    /// The text; empty once deleted.
    pub body: String,
    /// Deleted: the object stays as an AS2 `Tombstone`.
    pub tombstone: bool,
    /// When it was deleted (UTC text), if it was.
    pub deleted: Option<String>,
}

impl Comment {
    /// The author as an actor IRI.
    pub fn author(&self) -> &str {
        self.envelope.attributed_to.as_deref().unwrap_or("")
    }
    /// The IRI of the commented task.
    pub fn context(&self) -> &str {
        self.envelope.context.as_deref().unwrap_or("")
    }
    /// The IRI of the comment this one answers, if it is a reply.
    pub fn in_reply_to(&self) -> Option<&str> {
        self.envelope.in_reply_to.as_deref()
    }
    /// What the comment was when it was read, for compare-and-set (equality only).
    pub fn revision(&self) -> Revision {
        Revision::of_parts(
            &self.envelope.updated,
            &format!("{}\u{1}{}", self.body, self.tombstone),
        )
    }
}

/// Comments on tasks.
///
/// Interface status: draft (ADR 0045), the topic `comments` is a draft too. Not on the wire yet.
#[async_trait]
pub trait Comments: Send + Sync {
    /// The comments of the task with this IRI, oldest first; a reply follows the comment it answers.
    async fn comments(&self, _context: &str) -> Result<Vec<Comment>> {
        not_possible("comment.list")
    }
    /// `comment.add`: comment on the task (`in_reply_to` empty) or answer a comment of it. The author is the provider's
    /// actor. A reply to a reply or to a comment of another task is [`Error::InvalidReply`](crate::Error::InvalidReply).
    async fn comment_add(
        &self,
        _context: &str,
        _body: &str,
        _in_reply_to: Option<&str>,
    ) -> Result<Comment> {
        not_possible("comment.add")
    }
    /// `comment.edit`: only the author, and only if the comment still has `revision` ([`Error::Changed`](crate::Error::Changed) otherwise).
    async fn comment_edit(
        &self,
        _id: ObjectId,
        _revision: Revision,
        _body: &str,
    ) -> Result<Comment> {
        not_possible("comment.edit")
    }
    /// `comment.delete`: the author or the owner of the task; leaves a tombstone.
    async fn comment_delete(&self, _id: ObjectId) -> Result<Comment> {
        not_possible("comment.delete")
    }
}
