//! Deferred tasks: a one-shot calendar date that keeps an open record out of the pickers
//! until it arrives. Dueness is derived at read time from the field and the clock, as
//! periodic dueness is. See docs/specs/2026-09-15-defer-design.md.

use crate::error::{Error, Result};
use crate::model::{Status, Task};
use std::fmt;
use time::{Date, Month, OffsetDateTime};

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

    /// Resolves an absolute date or interval measured from `today`.
    pub fn resolve(value: &str, today: Date) -> Result<Defer> {
        let date = if value.ends_with('d') || value.ends_with('w') {
            let interval = crate::periodic::Interval::parse(value)?;
            today
                .checked_add(time::Duration::days(interval.days()))
                .ok_or_else(|| bad(value, "not a representable date"))?
        } else {
            Defer::parse(value)?.0
        };
        if date <= today {
            return Err(bad(
                value,
                &format!("must be after today ({})", Defer(today)),
            ));
        }
        Ok(Defer(date))
    }

    pub fn date(self) -> Date {
        self.0
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

/// An age: the `<n>d`/`<n>w` half of the deferral grammar read as a count of days. `0d`
/// is the one spelling of zero, which the deferral grammar has no use for but an age
/// bound reads as "no age check" (`sample --older-than 0d`). A clap value parser, so a
/// malformed age is a usage error naming the option.
pub fn parse_age(value: &str) -> Result<i64> {
    if value == "0d" {
        return Ok(0);
    }
    Ok(crate::periodic::Interval::parse(value)?.days())
}

/// The statuses on which a deferral is meaningful.
pub fn can_carry(status: Status) -> bool {
    matches!(status, Status::Idea | Status::Todo | Status::Blocked)
}

/// The deferral has been spent by the clock but not yet by attention (spec §4).
pub fn is_due(task: &Task, now: OffsetDateTime) -> bool {
    task.defer.is_some_and(|defer| now.date() >= defer.0)
}

/// Whole calendar days from `now` to `date`, the figure `prime` prints (spec §5.3).
pub fn days_until(date: Date, now: OffsetDateTime) -> i64 {
    (date - now.date()).whole_days()
}

/// Hidden from the pickers: the date is still ahead of the UTC calendar day (spec §4).
pub fn is_deferred(task: &Task, now: OffsetDateTime) -> bool {
    task.defer.is_some_and(|defer| now.date() < defer.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskId;

    fn day(s: &str) -> Date {
        Defer::parse(s).unwrap().0
    }

    fn at(s: &str) -> OffsetDateTime {
        crate::time::parse(s).unwrap()
    }

    fn deferred(status: Status, defer: Option<&str>) -> Task {
        Task {
            id: TaskId::parse("sci-000001").unwrap(),
            title: "Later".into(),
            status,
            priority: 2,
            size: None,
            complexity: None,
            process: None,
            parallel: false,
            every: None,
            defer: defer.map(|date| Defer::parse(date).unwrap()),
            owner: None,
            created: "2026-01-01T00:00:00Z".into(),
            updated: "2026-01-01T00:00:00Z".into(),
            started: None,
            completed: None,
            last_done: None,
            depends: vec![],
            parent: None,
            tags: vec![],
            source: None,
            model: None,
            agent: None,
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

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
    fn days_until_counts_calendar_days_not_elapsed_periods() {
        assert_eq!(days_until(day("2026-11-10"), at("2026-11-09T23:59:59Z")), 1);
        assert_eq!(days_until(day("2026-11-10"), at("2026-11-10T00:00:01Z")), 0);
        assert_eq!(
            days_until(day("2026-11-10"), at("2026-09-15T12:00:00Z")),
            56
        );
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

    #[test]
    fn resolve_accepts_a_future_date_or_an_interval_from_today() {
        let today = day("2026-09-15");
        assert_eq!(
            Defer::resolve("2026-11-10", today).unwrap().to_string(),
            "2026-11-10"
        );
        assert_eq!(
            Defer::resolve("60d", today).unwrap().to_string(),
            "2026-11-14"
        );
        assert_eq!(
            Defer::resolve("8w", today).unwrap().to_string(),
            "2026-11-10"
        );
        assert_eq!(
            Defer::resolve("1d", today).unwrap().to_string(),
            "2026-09-16"
        );
    }

    #[test]
    fn resolve_refuses_today_the_past_and_bad_intervals() {
        let today = day("2026-09-15");
        for bad in [
            "2026-09-15",
            "2026-09-14",
            "2020-01-01",
            "0d",
            "0w",
            "30m",
            "d",
        ] {
            assert!(
                Defer::resolve(bad, today).is_err(),
                "{bad} should be rejected"
            );
        }
        let error = Defer::resolve("2026-09-15", today).unwrap_err().to_string();
        assert!(
            error.contains("must be after today (2026-09-15)"),
            "{error}"
        );
    }

    #[test]
    fn only_picker_statuses_carry_a_deferral() {
        assert!(can_carry(Status::Idea));
        assert!(can_carry(Status::Todo));
        assert!(can_carry(Status::Blocked));
        for status in [
            Status::Doing,
            Status::Shelved,
            Status::Done,
            Status::Dropped,
        ] {
            assert!(!can_carry(status), "{status:?}");
        }
    }

    #[test]
    fn deferred_and_due_flip_at_the_utc_day_boundary() {
        let task = deferred(Status::Todo, Some("2026-11-10"));
        assert!(is_deferred(&task, at("2026-11-09T23:59:59Z")));
        assert!(!is_due(&task, at("2026-11-09T23:59:59Z")));
        assert!(!is_deferred(&task, at("2026-11-10T00:00:00Z")));
        assert!(is_due(&task, at("2026-11-10T00:00:00Z")));
        assert!(is_due(&task, at("2026-11-11T12:00:00Z")));
        let plain = deferred(Status::Todo, None);
        assert!(!is_deferred(&plain, at("2026-11-10T00:00:00Z")));
        assert!(!is_due(&plain, at("2026-11-10T00:00:00Z")));
    }
}
