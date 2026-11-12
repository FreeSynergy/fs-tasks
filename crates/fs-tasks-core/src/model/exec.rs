//! Execution fields of a task: what a runner (or any actor that takes ready tasks) reads and writes.
//!
//! Pattern: Value object + Specification — vendor members `freesynergy.net:...` of the task (RFC 8984 section 3.3),
//! kept verbatim in `extra`. The core checks only their form and size and interprets nothing: `run` is opaque JSON for
//! the runner. Large data (logs, files) does not belong here: it goes to JSCalendar `links`.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use super::task::Task;

/// What to run: `{ call, with, timeout, retry, risk, preview, key }`, opaque to the core (an object with a `call` text).
pub const RUN_KEY: &str = "freesynergy.net:run";
/// The result of the run: a small JSON value.
pub const RESULT_KEY: &str = "freesynergy.net:result";
/// How many attempts were made (a number).
pub const ATTEMPTS_KEY: &str = "freesynergy.net:attempts";
/// The last error: a problem type URI.
pub const ERROR_KEY: &str = "freesynergy.net:error";
/// A claim against being taken twice: `{ by, until }`.
pub const CLAIM_KEY: &str = "freesynergy.net:claim";
/// Where the task came from in a template: `{ id, version, path, index }`.
pub const TEMPLATE_KEY: &str = "freesynergy.net:template";
/// The pass of a loop: `{ id, iteration, limit }`.
pub const LOOP_KEY: &str = "freesynergy.net:loop";

/// Most bytes of the JSON of `run` and of `result` (large data goes to `links`).
pub const MAX_EXEC_BYTES: usize = 64 * 1024;

/// A claim on a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    /// The actor IRI that took the task.
    pub by: String,
    /// Until when the claim holds (UTC).
    pub until: DateTime<Utc>,
}

/// The pass of a loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopPass {
    /// The loop (project id plus template path of the jump back; one loop is one id over all passes).
    pub id: String,
    /// The pass, from 1.
    pub iteration: u32,
    /// The upper limit that is valid for this run.
    pub limit: u32,
}

impl Task {
    /// The `run` member.
    pub fn run(&self) -> Option<&Value> {
        self.vendor(RUN_KEY)
    }
    /// Set the `run` member; the form is checked by `validate`.
    pub fn set_run(&mut self, run: Value) {
        self.set_vendor(RUN_KEY, run);
    }
    /// The `result` member.
    pub fn result(&self) -> Option<&Value> {
        self.vendor(RESULT_KEY)
    }
    /// Set the `result` member.
    pub fn set_result(&mut self, result: Value) {
        self.set_vendor(RESULT_KEY, result);
    }
    /// Attempts made so far (0 when nothing is written).
    pub fn attempts(&self) -> u64 {
        self.vendor(ATTEMPTS_KEY)
            .and_then(Value::as_u64)
            .unwrap_or(0)
    }
    /// Set the number of attempts.
    pub fn set_attempts(&mut self, n: u64) {
        self.set_vendor(ATTEMPTS_KEY, json!(n));
    }
    /// The last error, a problem type URI.
    pub fn error(&self) -> Option<&str> {
        self.vendor(ERROR_KEY).and_then(Value::as_str)
    }
    /// Set the last error.
    pub fn set_error(&mut self, problem_type: &str) {
        self.set_vendor(ERROR_KEY, json!(problem_type));
    }
    /// The claim on the task, if it has a readable one.
    pub fn claim(&self) -> Option<Claim> {
        let c = self.vendor(CLAIM_KEY)?;
        Some(Claim {
            by: c.get("by")?.as_str()?.to_string(),
            until: crate::time::parse_utc(c.get("until")?.as_str()?).ok()?,
        })
    }
    /// Does a claim hold at `now`?
    pub fn claimed_at(&self, now: DateTime<Utc>) -> Option<Claim> {
        self.claim().filter(|c| c.until > now)
    }
    /// Put a claim on the task.
    pub fn set_claim(&mut self, by: &str, until: DateTime<Utc>) {
        let until = until.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        self.set_vendor(CLAIM_KEY, json!({"by": by, "until": until}));
    }
    /// Take the claim away.
    pub fn clear_claim(&mut self) {
        self.extra.remove(CLAIM_KEY);
    }
    /// Where the task came from in a template, as it is (opaque).
    pub fn template(&self) -> Option<&Value> {
        self.vendor(TEMPLATE_KEY)
    }
    /// The loop pass of the task, if it has a readable one.
    pub fn loop_pass(&self) -> Option<LoopPass> {
        let l = self.vendor(LOOP_KEY)?;
        Some(LoopPass {
            id: l.get("id")?.as_str()?.to_string(),
            iteration: u32::try_from(l.get("iteration")?.as_u64()?).ok()?,
            limit: u32::try_from(l.get("limit")?.as_u64()?).ok()?,
        })
    }
    /// Set the loop pass.
    pub fn set_loop_pass(&mut self, pass: &LoopPass) {
        self.set_vendor(
            LOOP_KEY,
            json!({"id": pass.id, "iteration": pass.iteration, "limit": pass.limit}),
        );
    }
}

/// The problems with the form of the execution members: `(path, what is wrong)`. A task without them has none.
pub fn exec_issues(t: &Task) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut bad = |key: &str, what: &str| out.push((format!("/{key}"), what.to_string()));
    let size = |v: &Value| v.to_string().len();
    if let Some(run) = t.run() {
        if !run.get("call").is_some_and(Value::is_string) {
            bad(RUN_KEY, "an object with a `call` text");
        } else if size(run) > MAX_EXEC_BYTES {
            bad(RUN_KEY, "too large (at most 64 KiB)");
        }
    }
    if let Some(result) = t.result()
        && size(result) > MAX_EXEC_BYTES
    {
        bad(
            RESULT_KEY,
            "too large (at most 64 KiB; large data goes to links)",
        );
    }
    if t.vendor(ATTEMPTS_KEY).is_some_and(|v| v.as_u64().is_none()) {
        bad(ATTEMPTS_KEY, "a non-negative number");
    }
    if t.vendor(ERROR_KEY)
        .is_some_and(|v| !v.as_str().is_some_and(|s| s.contains(':')))
    {
        bad(ERROR_KEY, "a problem type URI");
    }
    if t.vendor(CLAIM_KEY).is_some() && t.claim().is_none() {
        bad(CLAIM_KEY, "{ by: actor IRI, until: UTCDateTime }");
    }
    if t.vendor(TEMPLATE_KEY).is_some_and(|v| !v.is_object()) {
        bad(TEMPLATE_KEY, "an object { id, version, path, index }");
    }
    if t.vendor(LOOP_KEY).is_some() && t.loop_pass().is_none_or(|l| l.iteration < 1 || l.limit < 1)
    {
        bad(LOOP_KEY, "{ id, iteration >= 1, limit >= 1 }");
    }
    out
}
