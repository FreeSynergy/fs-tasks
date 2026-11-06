//! Rules for the members a task shares with every JSCalendar object.
//!
//! Pattern: Specification — one function checks start, stamps, zones, obsolete 1.0 members, the
//! recurrence rule, overrides and participants through a JSON view (so unknown members are seen too).

use serde_json::Value;

use super::ctx::Ctx;
use super::rule::rule;
use crate::model::{PARTICIPATION_STATUSES, Task};
use crate::time::{Zone, parse_local, parse_utc};

/// Shared fields of `task`, read through its JSON form.
pub(super) fn shared(c: &mut Ctx, task: &Task) {
    let v = task.to_value();
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    if let Some(st) = s("start") {
        if parse_local(st).is_err() {
            c.bad(
                "/start",
                format!("not a LocalDateTime (no zone suffix, no fraction in 2.0): {st:?}"),
            );
        }
    }
    for k in ["created", "updated"] {
        if let Some(t) = s(k) {
            if parse_utc(t).is_err() {
                c.bad(&format!("/{k}"), format!("not a UTCDateTime: {t:?}"));
            }
        }
    }
    for k in ["timeZone", "recurrenceIdTimeZone"] {
        if let Some(z) = s(k) {
            if Zone::parse(z).is_err() {
                c.bad(
                    &format!("/{k}"),
                    format!("not an IANA time zone (JSCalendar 2.0 has no custom zones): {z:?}"),
                );
            }
        }
    }
    obsolete(c, &v);
    c.one_of("/freeBusyStatus", s("freeBusyStatus"), &["free", "busy"]);
    c.one_of("/privacy", s("privacy"), &["public", "private", "secret"]);
    if let Some(r) = &task.recurrence_rule {
        rule(c, r);
        if task.start.is_none() {
            c.bad("/recurrenceRule", "needs `start`");
        }
    }
    for k in task.recurrence_overrides.keys() {
        if parse_local(k).is_err() {
            c.bad(
                &format!("/recurrenceOverrides/{k}"),
                "key must be a LocalDateTime",
            );
        }
    }
    participants(c, &v);
}

fn obsolete(c: &mut Ctx, v: &Value) {
    if v.get("recurrenceRules").is_some() {
        c.bad(
            "/recurrenceRules",
            "obsolete in 2.0, use the single `recurrenceRule`",
        );
    }
    if v.get("timeZones").is_some() || v.get("excludedRecurrenceRules").is_some() {
        c.bad(
            "/timeZones",
            "`timeZones` and `excludedRecurrenceRules` are obsolete in 2.0",
        );
    }
}

fn participants(c: &mut Ctx, v: &Value) {
    let Some(ps) = v.get("participants").and_then(Value::as_object) else {
        return;
    };
    for (k, p) in ps {
        if let Some(st) = p.get("participationStatus").and_then(Value::as_str) {
            if !PARTICIPATION_STATUSES.contains(&st) {
                c.bad(
                    &format!("/participants/{k}/participationStatus"),
                    format!("{st:?} not in {PARTICIPATION_STATUSES:?}"),
                );
            }
        }
        for obsolete in ["language", "locationId"] {
            if p.get(obsolete).is_some() {
                c.bad(&format!("/participants/{k}/{obsolete}"), "obsolete in 2.0");
            }
        }
    }
}
