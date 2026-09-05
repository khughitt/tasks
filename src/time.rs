use crate::error::{Error, Result};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn now() -> String {
    OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .expect("zero ns is valid")
        .format(&Rfc3339)
        .expect("rfc3339")
}

/// The calendar day of a validated RFC 3339 UTC timestamp, `YYYY-MM-DD`.
pub fn day(timestamp: &str) -> &str {
    &timestamp[..10]
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
    fn day_is_the_date_part() {
        assert_eq!(day("2026-08-29T14:02:11Z"), "2026-08-29");
    }
}
