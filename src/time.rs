use crate::error::{Error, Result};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn now() -> String {
    format(OffsetDateTime::now_utc())
}

/// The stamp a write to a record loaded at `loaded` sets: now, or one second past `loaded`
/// when the clock has not passed it. Stamps have second precision, so a plain `now` in the
/// same second as the loaded write would give two checkouts' different copies one stamp
/// (record-home spec §3.1); strictly increasing stamps keep the written copy the newest.
pub fn after(loaded: &str) -> Result<String> {
    let floor = parse(loaded)? + time::Duration::SECOND;
    Ok(format(OffsetDateTime::now_utc().max(floor)))
}

/// The calendar day of a validated RFC 3339 UTC timestamp, `YYYY-MM-DD`.
pub fn day(timestamp: &str) -> &str {
    &timestamp[..10]
}

/// The calendar date of a `YYYY-MM-DD` day, as `day` returns it.
pub fn calendar_day(day: &str) -> Result<time::Date> {
    time::Date::parse(day, &time::format_description::well_known::Iso8601::DATE)
        .map_err(|e| Error::Validation(format!("bad date {day:?}: {e}")))
}

pub fn parse(s: &str) -> Result<OffsetDateTime> {
    if !s.ends_with('Z') {
        return Err(Error::Validation(format!(
            "timestamp {s:?} must be UTC with Z suffix"
        )));
    }
    if s.contains('.') {
        return Err(Error::Validation(format!(
            "timestamp {s:?} must have second precision"
        )));
    }
    OffsetDateTime::parse(s, &Rfc3339)
        .map_err(|e| Error::Validation(format!("bad timestamp {s:?}: {e}")))
}

/// The inverse of `parse`: the format `now` writes, for a timestamp we computed.
pub fn format(t: OffsetDateTime) -> String {
    t.replace_nanosecond(0)
        .expect("zero ns is valid")
        .format(&Rfc3339)
        .expect("rfc3339")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_whole_seconds_utc_only() {
        assert!(parse("2026-08-29T14:02:11Z").is_ok());
        assert!(parse("2026-08-29T14:02:11.5Z").is_err());
        assert!(parse("2026-08-29T14:02:11+02:00").is_err());
        assert!(parse("2026-08-29").is_err());
        assert!(now().ends_with('Z') && !now().contains('.'));
    }

    #[test]
    fn after_is_now_unless_the_loaded_stamp_has_not_passed() {
        let before = now();
        assert!(after("2020-01-01T00:00:00Z").unwrap() >= before);
        assert_eq!(
            after("2099-12-31T23:59:59Z").unwrap(),
            "2100-01-01T00:00:00Z"
        );
        assert!(after("2099-12-31T23:59:59.5Z").is_err());
    }

    #[test]
    fn day_is_the_date_part() {
        assert_eq!(day("2026-08-29T14:02:11Z"), "2026-08-29");
    }
}
