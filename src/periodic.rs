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
