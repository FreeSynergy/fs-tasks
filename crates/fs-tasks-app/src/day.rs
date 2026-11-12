//! A calendar day, and what "today" is.
//!
//! Pattern: Value object + Strategy — `Day` is plain civil-calendar arithmetic (no zone: the tasks' due times are
//! compared by their date text), and "today" is a value the program is given (`--today` for pictures and tests,
//! else the system clock in UTC), so a test never depends on the clock.

use std::time::{SystemTime, UNIX_EPOCH};

/// A day of the proleptic Gregorian calendar, as the count of days since 1970-01-01.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day(pub i64);

impl Day {
    /// The day of `year`-`month`-`day`.
    #[must_use]
    pub fn civil(year: i64, month: i64, day: i64) -> Self {
        let y = if month <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Self(era * 146_097 + doe - 719_468)
    }

    /// Read `YYYY-MM-DD`.
    ///
    /// # Errors
    /// A message for the person when the text is not such a date.
    pub fn parse(text: &str) -> Result<Self, String> {
        let bad = || format!("{text:?} is not a date like 2026-11-12");
        let mut parts = text.trim().splitn(3, '-').map(str::parse::<i64>);
        let (Some(Ok(y)), Some(Ok(m)), Some(Ok(d))) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(bad());
        };
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return Err(bad());
        }
        Ok(Self::civil(y, m, d))
    }

    /// The system's today (UTC).
    #[must_use]
    pub fn today() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self(i64::try_from(secs / 86_400).unwrap_or(0))
    }

    /// The day `days` later (earlier when negative).
    #[must_use]
    pub fn plus(self, days: i64) -> Self {
        Self(self.0 + days)
    }

    /// Year, month, day.
    #[must_use]
    pub fn ymd(self) -> (i64, i64, i64) {
        let z = self.0 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        (yoe + era * 400 + i64::from(month <= 2), month, day)
    }

    /// `YYYY-MM-DD`, the start of a due text.
    #[must_use]
    pub fn text(self) -> String {
        let (y, m, d) = self.ymd();
        format!("{y:04}-{m:02}-{d:02}")
    }

    /// The same, as iCalendar writes a date: `YYYYMMDD`.
    #[must_use]
    pub fn compact(self) -> String {
        self.text().replace('-', "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_round_trip_and_step_over_month_ends() {
        let d = Day::parse("2026-11-12").unwrap();
        assert_eq!(d.text(), "2026-11-12");
        assert_eq!(d.plus(19).text(), "2026-12-01");
        assert_eq!(
            Day::parse("2024-02-28").unwrap().plus(1).text(),
            "2024-02-29"
        );
        assert_eq!(Day::civil(1970, 1, 1), Day(0));
        for n in [-800, 0, 1, 365, 20_000] {
            let (y, m, dd) = Day(n).ymd();
            assert_eq!(Day::civil(y, m, dd), Day(n));
        }
        assert!(Day::parse("2026-13-01").is_err());
    }
}
