//! The gap of a relation: lag or lead between two tasks.
//!
//! Pattern: Value object + pure function — an ISO 8601 duration with an optional sign, as the member
//! `freesynergy.net:gap` of the JSCalendar `Relation` (vendor member, RFC 8984 section 3.3) and as the RFC 9253 `GAP`
//! parameter of iCalendar. Positive is lag, negative is lead.

use chrono::Duration;
use fs_jscalendar::time::Dur;

use crate::error::{Error, Result};

/// The vendor member of a `Relation` object that holds the gap.
pub const GAP_KEY: &str = "freesynergy.net:gap";

/// Largest gap accepted, in days (about ten years). RFC 9253 (Security Considerations): "Extremely large values for
/// the GAP parameter may lead to unexpected behavior."
pub const MAX_GAP_DAYS: i64 = 3660;

/// Read a gap such as `P1D`, `-PT4H` or `+P2W`. A calendar day counts as 24 hours here.
pub fn parse_gap(text: &str) -> Result<Duration> {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let dur = Dur::parse(rest)?;
    let seconds = dur
        .days
        .checked_mul(86_400)
        .and_then(|d| d.checked_add(dur.secs))
        .ok_or_else(|| Error::Time(format!("gap too large: {text}")))?;
    if seconds > MAX_GAP_DAYS * 86_400 {
        return Err(Error::Time(format!(
            "gap too large: {text} (at most {MAX_GAP_DAYS} days)"
        )));
    }
    Ok(Duration::seconds(if negative { -seconds } else { seconds }))
}
