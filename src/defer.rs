//! Deferred tasks: a one-shot calendar date that keeps an open record out of the pickers
//! until it arrives. Dueness is derived at read time from the field and the clock, as
//! periodic dueness is. See docs/specs/2026-09-15-defer-design.md.

use crate::error::{Error, Result};
use std::fmt;
use time::{Date, Month};

fn bad(value: &str, detail: &str) -> Error {
    Error::Validation(format!("bad defer date {value:?}: {detail}"))
}

/// A calendar day, `YYYY-MM-DD` (spec §2.1). A day and not an instant: the record is due
/// from the start of that day in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Defer(Date);

impl Defer {
    /// Strict: four, two, and two digits, and a day that exists.
    pub fn parse(value: &str) -> Result<Defer> {
        let parts: Vec<&str> = value.split('-').collect();
        let [year, month, day] = parts[..] else {
            return Err(bad(value, "expected YYYY-MM-DD"));
        };
        let digits = |part: &str, width: usize| {
            part.len() == width && part.bytes().all(|byte| byte.is_ascii_digit())
        };
        if !digits(year, 4) || !digits(month, 2) || !digits(day, 2) {
            return Err(bad(value, "expected YYYY-MM-DD"));
        }
        let month = Month::try_from(month.parse::<u8>().expect("two digits"))
            .map_err(|_| bad(value, "the month must be 01-12"))?;
        let date = Date::from_calendar_date(
            year.parse().expect("four digits"),
            month,
            day.parse().expect("two digits"),
        )
        .map_err(|error| bad(value, &error.to_string()))?;
        Ok(Defer(date))
    }
}

impl fmt::Display for Defer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02}",
            self.0.year(),
            self.0.month() as u8,
            self.0.day()
        )
    }
}

impl serde::Serialize for Defer {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints_a_calendar_date() {
        assert_eq!(
            Defer::parse("2026-11-10").unwrap().to_string(),
            "2026-11-10"
        );
        assert_eq!(
            Defer::parse("2026-01-05").unwrap().to_string(),
            "2026-01-05"
        );
        assert_eq!(
            serde_json::to_string(&Defer::parse("2026-11-10").unwrap()).unwrap(),
            "\"2026-11-10\""
        );
        assert!(Defer::parse("2026-01-05").unwrap() < Defer::parse("2026-11-10").unwrap());
    }

    #[test]
    fn rejects_everything_that_is_not_a_real_day() {
        for bad in [
            "",
            "2026",
            "2026-11",
            "2026-11-10T00:00:00Z",
            "26-11-10",
            "2026-1-10",
            "2026-11-1",
            "2026-13-01",
            "2026-02-30",
            "2026/11/10",
            "20261110",
            "2026-11-10-",
            "-2026-11-10",
            "2026-1a-10",
        ] {
            assert!(Defer::parse(bad).is_err(), "{bad} should be rejected");
        }
    }
}
