//! The names `--sample` knows.
//!
//! Pattern: Factory — a name in, an iCalendar text out.

use super::{chain, demo};
use crate::day::Day;

/// What `--sample` names: `demo`, or `chain:N`.
///
/// # Errors
/// A message for the person when the name is not one of these.
pub fn by_name(name: &str, today: Day) -> Result<String, String> {
    if name == "demo" {
        return Ok(demo(today));
    }
    if let Some(n) = name.strip_prefix("chain:") {
        let n: usize = n
            .parse()
            .map_err(|_| format!("--sample chain:N wants a number, not {n}"))?;
        return Ok(chain(n.min(100_000), today));
    }
    Err(format!("--sample wants demo or chain:N, not {name}"))
}
