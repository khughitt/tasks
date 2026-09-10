use crate::model::{Status, Task};
#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> OffsetDateTime {
        crate::time::parse(s).unwrap()
    }

    #[test]
    fn parses_days_and_weeks_and_round_trips() {
        assert_eq!(Interval::parse("30d").unwrap().to_string(), "30d");
        assert_eq!(Interval::parse("2w").unwrap().to_string(), "2w");
        assert_eq!(Interval::parse("1d").unwrap().days(), 1);
        assert_eq!(Interval::parse("2w").unwrap().days(), 14);
        assert_eq!(Interval::parse("90d").unwrap().days(), 90);
    }

    #[test]
    fn rejects_everything_else() {
        for bad in [
            "0d",
            "0w",
            "d",
            "w",
            "",
            "30",
            "30m",
            "30y",
            "-1d",
            "1.5d",
            "30D",
            "1 d",
            "030d",
            "36501d",
            "5215w",
            "99999999999999999999d",
        ] {
            assert!(Interval::parse(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn add_is_checked_against_the_end_of_the_range() {
        let every = Interval::parse("1d").unwrap();
        assert_eq!(
            add(at("2026-09-09T11:00:00Z"), every),
            Some(at("2026-09-10T11:00:00Z"))
        );
        assert!(
            add(
                OffsetDateTime::new_utc(time::Date::MAX, time::Time::MIDNIGHT),
                every
            )
            .is_none()
        );
    }

    #[test]
    fn serializes_as_its_string_form() {
        let json = serde_json::to_string(&Interval::parse("2w").unwrap()).unwrap();
        assert_eq!(json, "\"2w\"");
    }
    use crate::model::{Status, Task, TaskId};

    fn periodic(status: Status, every: Option<&str>, last_done: Option<&str>) -> Task {
        Task {
            id: TaskId::parse("sci-000001").unwrap(),
            title: "Sweep".into(),
            status,
            priority: 2,
            size: None,
            parallel: false,
            every: every.map(|e| Interval::parse(e).unwrap()),
            owner: None,
            created: "2026-01-01T00:00:00Z".into(),
            updated: "2026-01-01T00:00:00Z".into(),
            last_done: last_done.map(Into::into),
            depends: vec![],
            parent: None,
            tags: vec![],
            source: None,
            spec: None,
            plan: None,
            step: None,
            body: String::new(),
            notes: vec![],
        }
    }

    #[test]
    fn due_carries_the_status_gate() {
        let anchored = |status| periodic(status, Some("30d"), Some("2026-09-09T11:00:00Z"));
        assert_eq!(
            due(&anchored(Status::Done)),
            Some(at("2026-10-09T11:00:00Z"))
        );
        // An anchored record that is open or dropped has no pending recurrence (spec §4.1);
        // this is the None the JSON contract renders as null.
        assert_eq!(due(&anchored(Status::Todo)), None);
        assert_eq!(due(&anchored(Status::Doing)), None);
        assert_eq!(due(&anchored(Status::Dropped)), None);
        // Unanchored: no computable date even though the record is due.
        assert_eq!(due(&periodic(Status::Done, Some("30d"), None)), None);
        assert_eq!(due(&periodic(Status::Done, None, None)), None);
    }

    #[test]
    fn is_due_at_the_boundary() {
        let task = periodic(Status::Done, Some("30d"), Some("2026-09-09T11:00:00Z"));
        assert!(!is_due(&task, at("2026-10-09T10:59:59Z")));
        assert!(is_due(&task, at("2026-10-09T11:00:00Z")));
        assert!(is_due(&task, at("2026-10-09T11:00:01Z")));
    }

    #[test]
    fn unanchored_closed_recurrence_is_due_now() {
        assert!(is_due(
            &periodic(Status::Done, Some("30d"), None),
            at("2020-01-01T00:00:00Z")
        ));
    }

    #[test]
    fn open_and_dropped_and_plain_records_are_never_due() {
        let now = at("2030-01-01T00:00:00Z");
        assert!(!is_due(&periodic(Status::Todo, Some("30d"), None), now));
        assert!(!is_due(&periodic(Status::Doing, Some("30d"), None), now));
        assert!(!is_due(
            &periodic(Status::Dropped, Some("30d"), Some("2026-09-09T11:00:00Z")),
            now
        ));
        assert!(!is_due(&periodic(Status::Done, None, None), now));
    }
    #[test]
    fn days_until_counts_calendar_days() {
        // One second after a 7d cadence is stamped, the answer is still 7: truncating the
        // elapsed remainder would say 6, and disagree with the date printed beside it.
        assert_eq!(
            days_until(at("2026-10-09T11:00:00Z"), at("2026-10-02T11:00:01Z")),
            7
        );
        // A second either side of a day boundary moves it by exactly one.
        assert_eq!(
            days_until(at("2026-10-09T00:00:00Z"), at("2026-10-08T23:59:59Z")),
            1
        );
        assert_eq!(
            days_until(at("2026-10-09T23:59:59Z"), at("2026-10-09T00:00:00Z")),
            0,
            "due later today reads in 0d"
        );
    }
}
use crate::error::{Error, Result};
use std::fmt;
use time::{Duration, OffsetDateTime};

/// The largest interval accepted, in days. A product limit, not a representability
/// argument (spec §3.1): a century is a typo, and a stated number beats a platform
/// quirk. Representability is `add`'s job, and it is checked separately.
pub const MAX_DAYS: u32 = 36500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Day,
    Week,
}

impl Unit {
    fn suffix(self) -> char {
        match self {
            Unit::Day => 'd',
            Unit::Week => 'w',
        }
    }

    fn days(self) -> u32 {
        match self {
            Unit::Day => 1,
            Unit::Week => 7,
        }
    }
}

/// A recurrence interval: a positive whole number of days or weeks (spec §3.1). The unit
/// is kept so `2w` round-trips as `2w` rather than `14d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    count: u32,
    unit: Unit,
}

fn bad(value: &str, detail: &str) -> Error {
    Error::Validation(format!("bad interval {value:?}: {detail}"))
}

impl Interval {
    pub fn parse(value: &str) -> Result<Interval> {
        let (digits, unit) = match value.strip_suffix('d') {
            Some(digits) => (digits, Unit::Day),
            None => match value.strip_suffix('w') {
                Some(digits) => (digits, Unit::Week),
                None => {
                    return Err(bad(
                        value,
                        "expected a count followed by d or w, such as 30d or 2w",
                    ));
                }
            },
        };
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(bad(value, "the count must be a whole number"));
        }
        if digits.len() > 1 && digits.starts_with('0') {
            return Err(bad(value, "the count must not have a leading zero"));
        }
        let count: u32 = digits
            .parse()
            .map_err(|_| bad(value, &format!("the count must be at most {MAX_DAYS}")))?;
        if count == 0 {
            return Err(bad(value, "the count must be greater than zero"));
        }
        let days = count
            .checked_mul(unit.days())
            .ok_or_else(|| bad(value, &format!("the count must be at most {MAX_DAYS} days")))?;
        if days > MAX_DAYS {
            return Err(bad(
                value,
                &format!("{days} days is longer than the {MAX_DAYS} day limit"),
            ));
        }
        Ok(Interval { count, unit })
    }

    pub fn days(self) -> i64 {
        i64::from(self.count) * i64::from(self.unit.days())
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.count, self.unit.suffix())
    }
}

impl serde::Serialize for Interval {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub fn add(anchor: OffsetDateTime, every: Interval) -> Option<OffsetDateTime> {
    anchor.checked_add(Duration::days(every.days()))
}

/// The date this recurrence next falls due, or `None` for an open or dropped record, for
/// one with no cadence, and for one that has never been completed (spec §4.1). The status
/// gate lives here so eligibility, ordering, JSON, and pretty output share one value.
pub fn due(task: &Task) -> Option<OffsetDateTime> {
    if task.status != Status::Done {
        return None;
    }
    let every = task.every?;
    let anchor = task.last_done.as_deref()?;
    let anchor = crate::time::parse(anchor).expect("validate_task parsed last_done");
    Some(add(anchor, every).expect("validate_task checked this sum"))
}

/// Whether a closed recurrence has come back around (spec §4.1).
pub fn is_due(task: &Task, now: OffsetDateTime) -> bool {
    task.every.is_some()
        && task.status == Status::Done
        && due(task).map_or(task.last_done.is_none(), |date| now >= date)
}

/// Whole calendar days from `now` to `due`: the figure the `prime` line prints beside the
/// due date (spec §5.3). Calendar days, not elapsed 24-hour periods, so the number always
/// agrees with the date next to it -- a cadence stamped a second ago reads `in 7d`, where
/// truncating the remaining duration would say 6. A record due later today reads 0.
pub fn days_until(due: OffsetDateTime, now: OffsetDateTime) -> i64 {
    (due.date() - now.date()).whole_days()
}
