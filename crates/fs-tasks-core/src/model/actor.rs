//! Assignees that are not (only) people: an actor with an IRI and a type.
//!
//! Pattern: Value object + Command — the truth is the vendor member `freesynergy.net:actor` (an AS2 actor IRI) with
//! `freesynergy.net:actorType` on the participant (RFC 8984 section 3.3: vendor members are prefixed by a domain name
//! controlled by the vendor); the standard member `kind` is only the form JSCalendar understands (`individual`, `group`,
//! `resource`: "a non-human resource other than a location") and is written for the export.

use fs_jscalendar::model::{Participant, next_participant_key};
use serde_json::{Value, json};

use super::task::Task;

/// Vendor member of a participant: the AS2 actor IRI.
pub const ACTOR_KEY: &str = "freesynergy.net:actor";
/// Vendor member of a participant: the AS2 actor type.
pub const ACTOR_TYPE_KEY: &str = "freesynergy.net:actorType";
/// Vendor member of a participant: the actor is an AI (ADR 0054).
pub const AI_KEY: &str = "freesynergy.net:ai";

/// The kind of actor that is assigned (AS2 actor types).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorType {
    /// A person.
    Person,
    /// A program (an LLM agent, a runner, fs-store).
    Application,
    /// A machine or service (CI, a package manager, systemd).
    Service,
    /// A group of actors.
    Group,
    /// An organisation.
    Organization,
}

impl ActorType {
    /// The AS2 name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Person => "Person",
            Self::Application => "Application",
            Self::Service => "Service",
            Self::Group => "Group",
            Self::Organization => "Organization",
        }
    }

    /// Read an AS2 name.
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Person,
            Self::Application,
            Self::Service,
            Self::Group,
            Self::Organization,
        ]
        .into_iter()
        .find(|t| t.as_str() == s)
    }

    /// The JSCalendar `kind` this type is exported as: a person `individual`, a group or organisation `group`, a
    /// program or service `resource` (non-human).
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Person => "individual",
            Self::Group | Self::Organization => "group",
            Self::Application | Self::Service => "resource",
        }
    }

    /// Is a person behind it?
    pub const fn is_human(self) -> bool {
        matches!(self, Self::Person)
    }
}

/// What a participant is, as far as it says: the member `freesynergy.net:actorType` if it has one, else what `kind`
/// allows (`individual` = person, `group` = group). A `resource` without our member is non-human but of an unknown
/// type (`Application` and `Service` cannot be told apart): `None`; see [`is_non_human`].
pub fn actor_type_of(p: &Participant) -> Option<ActorType> {
    p.extra
        .get(ACTOR_TYPE_KEY)
        .and_then(Value::as_str)
        .and_then(ActorType::parse)
        .or_else(|| match p.kind.as_deref() {
            Some("individual") => Some(ActorType::Person),
            Some("group") => Some(ActorType::Group),
            _ => None,
        })
}

/// Is the participant known not to be a person: an actor type that is not `Person`, or the kind `resource`?
pub fn is_non_human(p: &Participant) -> bool {
    match actor_type_of(p) {
        Some(t) => !t.is_human(),
        None => p.kind.as_deref() == Some("resource"),
    }
}

/// The actor IRI of a participant, if it has one.
pub fn actor_of(p: &Participant) -> Option<&str> {
    p.extra.get(ACTOR_KEY).and_then(Value::as_str)
}

impl Task {
    /// Assign the task to an actor (role `owner`); returns the participant key. Assigning the same actor IRI twice keeps
    /// the one participant (its name and type are updated).
    pub fn assign_actor(&mut self, actor: &str, kind: ActorType, name: &str) -> String {
        let existing = self
            .participants
            .iter()
            .find(|(_, p)| actor_of(p) == Some(actor))
            .map(|(k, _)| k.clone());
        let key = existing.unwrap_or_else(|| next_participant_key(&self.participants));
        let p = self.participants.entry(key.clone()).or_default();
        p.type_ = Some("Participant".into());
        p.name = Some(name.to_string());
        p.kind = Some(kind.kind().to_string());
        p.roles.insert("owner".into(), true);
        p.extra.insert(ACTOR_KEY.into(), json!(actor));
        p.extra.insert(ACTOR_TYPE_KEY.into(), json!(kind.as_str()));
        key
    }

    /// The actor IRIs the task is assigned to, in participant order.
    pub fn actors(&self) -> Vec<&str> {
        self.participants.values().filter_map(actor_of).collect()
    }
}
