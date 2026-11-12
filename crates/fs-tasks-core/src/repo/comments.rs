//! Comments on tasks, stored as their own objects.
//!
//! Pattern: Repository — `TaskRepo` is the provider of the `Comments` contract. A comment is an AS2 `Note` (a
//! `Tombstone` once deleted) in the same store; `context` is the IRI of the task, so a query of tasks never sees it.
//! Writes take the same turn as the task writes. The author is always the provider's actor.

use fs_objects::{Envelope, ObjectId, Order, Query, StoredObject};
use serde_json::{Value, json};

use super::handle::TaskRepo;
use crate::error::{Error, Result};
use crate::interface::{Comment, TaskChange, TaskEntry};
use crate::now_utc;

/// AS2 type of a comment.
pub(super) const NOTE: &str = "Note";
/// AS2 type of a deleted comment.
pub(super) const TOMBSTONE: &str = "Tombstone";
/// Media type of the content of a comment object.
const CONTENT_TYPE: &str = "application/vnd.freesynergy.comment+json";
/// Longest comment, in bytes.
pub const MAX_COMMENT_BYTES: usize = 64 * 1024;
/// Most comments one task has (live ones and tombstones).
pub const MAX_COMMENTS_PER_TASK: usize = 5000;

fn read(object: StoredObject) -> Result<Comment> {
    let body = object
        .content
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let tombstone = object.envelope.object_type == TOMBSTONE;
    let deleted = object
        .content
        .get("deleted")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(Comment {
        id: object.envelope.id,
        envelope: object.envelope,
        body,
        tombstone,
        deleted,
    })
}

fn checked_body(body: &str) -> Result<&str> {
    let body = body.trim();
    if body.is_empty() {
        return Err(Error::Rejected("a comment needs a text".into()));
    }
    if body.len() > MAX_COMMENT_BYTES {
        return Err(Error::Rejected(format!(
            "a comment has at most {MAX_COMMENT_BYTES} bytes, this one {}",
            body.len()
        )));
    }
    Ok(body)
}

/// The envelope name: the first line, cut (the name of an object may never be empty).
fn name_of(body: &str) -> String {
    let line = body.lines().next().unwrap_or("").trim();
    let cut: String = line.chars().take(80).collect();
    if cut.is_empty() {
        "(comment)".into()
    } else {
        cut
    }
}

/// The object id at the end of an IRI `https://<instance>/<kind>/<id>`.
fn id_of_iri(iri: &str) -> Option<ObjectId> {
    iri.rsplit('/').next()?.parse().ok()
}

impl TaskRepo {
    /// The task whose IRI is `context`.
    async fn task_of(&self, context: &str) -> Result<TaskEntry> {
        let entry = match id_of_iri(context) {
            Some(id) => self.entry(id).await?,
            None => None,
        };
        entry
            .filter(|e| e.envelope.iri.as_deref() == Some(context))
            .ok_or_else(|| Error::Conflict(format!("{context} is not a task of this store")))
    }

    async fn comment_object(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Ok(self
            .objects
            .get(id)
            .await?
            .filter(|o| matches!(o.envelope.object_type.as_str(), NOTE | TOMBSTONE)))
    }

    /// All comment objects of a context, oldest first (notes and tombstones).
    async fn comment_objects(&self, context: &str) -> Result<Vec<StoredObject>> {
        let mut all = Vec::new();
        for kind in [NOTE, TOMBSTONE] {
            let q = Query::new()
                .of_type(kind)
                .in_context(context)
                .ordered(Order::PublishedAsc);
            all.extend(self.queries.query(&q).await?);
        }
        all.sort_by_key(|o| (o.envelope.published, o.envelope.id));
        Ok(all)
    }

    pub(super) async fn comments_inner(&self, context: &str) -> Result<Vec<Comment>> {
        let comments: Vec<Comment> = self
            .comment_objects(context)
            .await?
            .into_iter()
            .map(read)
            .collect::<Result<_>>()?;
        // a reply follows the comment it answers
        let mut out = Vec::with_capacity(comments.len());
        for top in comments.iter().filter(|c| c.in_reply_to().is_none()) {
            out.push(top.clone());
            let iri = top.envelope.iri.as_deref();
            out.extend(
                comments
                    .iter()
                    .filter(|c| c.in_reply_to().is_some() && c.in_reply_to() == iri)
                    .cloned(),
            );
        }
        Ok(out)
    }

    /// Add a comment on `context` as `author`. The caller holds the write turn.
    pub(super) async fn comment_add_unlocked(
        &self,
        context: &str,
        body: &str,
        in_reply_to: Option<&str>,
        author: &str,
    ) -> Result<Comment> {
        let body = checked_body(body)?;
        let task = self.task_of(context).await?;
        if let Some(parent) = in_reply_to {
            let found = match id_of_iri(parent) {
                Some(id) => self.comment_object(id).await?,
                None => None,
            };
            let ok = found.as_ref().is_some_and(|p| {
                p.envelope.iri.as_deref() == Some(parent)
                    && p.envelope.context.as_deref() == Some(context)
                    && p.envelope.in_reply_to.is_none()
            });
            if !ok {
                return Err(Error::InvalidReply(format!(
                    "{parent} is not a comment of {context} that is itself no reply"
                )));
            }
        }
        let count = self
            .queries
            .count(&Query::new().in_context(context).of_type(NOTE))
            .await?
            + self
                .queries
                .count(&Query::new().in_context(context).of_type(TOMBSTONE))
                .await?;
        if count >= MAX_COMMENTS_PER_TASK {
            return Err(Error::Conflict(format!(
                "a task has at most {MAX_COMMENTS_PER_TASK} comments"
            )));
        }
        let mut env = Envelope::new(NOTE, name_of(body));
        env.iri = Some(format!("https://{}/comments/{}", self.instance, env.id));
        env.context = Some(context.to_string());
        env.in_reply_to = in_reply_to.map(str::to_string);
        env.attributed_to = Some(author.to_string());
        env.to = vec![author.to_string()];
        // a comment of a sensitive task is sensitive too: the store refuses to keep it unencrypted
        env.sensitive.clone_from(&task.envelope.sensitive);
        let object = StoredObject::new(env, CONTENT_TYPE, json!({"body": body}));
        let id = object.envelope.id;
        self.objects.put(object, &self.actor).await?;
        let stored = self.comment_object(id).await?.ok_or(Error::NotFound(id))?;
        self.feed.notify(&TaskChange::Commented {
            task: task.id,
            comment: id,
        });
        read(stored)
    }

    pub(super) async fn comment_add_inner(
        &self,
        context: &str,
        body: &str,
        in_reply_to: Option<&str>,
    ) -> Result<Comment> {
        let _one_at_a_time = self.writes.lock().await;
        self.comment_add_unlocked(context, body, in_reply_to, &self.actor.clone())
            .await
    }

    pub(super) async fn comment_edit_inner(
        &self,
        id: ObjectId,
        revision: crate::Revision,
        body: &str,
    ) -> Result<Comment> {
        let body = checked_body(body)?;
        let _one_at_a_time = self.writes.lock().await;
        let mut object = self
            .comment_object(id)
            .await?
            .filter(|o| o.envelope.object_type == NOTE)
            .ok_or(Error::NotFound(id))?;
        if object.envelope.attributed_to.as_deref() != Some(self.actor.as_str()) {
            return Err(Error::Forbidden(
                "only the author may edit a comment".into(),
            ));
        }
        if read(object.clone())?.revision() != revision {
            return Err(Error::Changed(id));
        }
        object.envelope.name = name_of(body);
        object.content = json!({"body": body});
        self.objects.put(object, &self.actor).await?;
        let stored = self.comment_object(id).await?.ok_or(Error::NotFound(id))?;
        self.announce_comment(&stored).await;
        read(stored)
    }

    pub(super) async fn comment_delete_inner(&self, id: ObjectId) -> Result<Comment> {
        let _one_at_a_time = self.writes.lock().await;
        let mut object = self.comment_object(id).await?.ok_or(Error::NotFound(id))?;
        if object.envelope.object_type == TOMBSTONE {
            return read(object);
        }
        let context = object.envelope.context.clone().unwrap_or_default();
        let owner = self
            .task_of(&context)
            .await
            .ok()
            .and_then(|t| t.envelope.attributed_to);
        let author = object.envelope.attributed_to.clone();
        if author.as_deref() != Some(self.actor.as_str())
            && owner.as_deref() != Some(self.actor.as_str())
        {
            return Err(Error::Forbidden(
                "only the author or the owner of the task may delete a comment".into(),
            ));
        }
        object.envelope.object_type = TOMBSTONE.into();
        object.envelope.name = "(deleted comment)".into();
        object.content = json!({"body": "", "deleted": now_utc()});
        self.objects.put(object, &self.actor).await?;
        let stored = self.comment_object(id).await?.ok_or(Error::NotFound(id))?;
        self.announce_comment(&stored).await;
        read(stored)
    }

    async fn announce_comment(&self, object: &StoredObject) {
        let context = object.envelope.context.clone().unwrap_or_default();
        if let Ok(task) = self.task_of(&context).await {
            self.feed.notify(&TaskChange::Commented {
                task: task.id,
                comment: object.envelope.id,
            });
        }
    }

    /// Remove the comments of a task that is deleted (after the task itself).
    pub(super) async fn delete_comments_of(&self, context: &str) -> Result<()> {
        for object in self.comment_objects(context).await? {
            self.objects.delete(object.envelope.id, &self.actor).await?;
        }
        Ok(())
    }
}
