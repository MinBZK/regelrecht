//! Time in the cell runtime, in one place: the two times of a gram
//! (`effective_at` and `recorded_at`, each a moment with time zone, RFC 3339),
//! the reference date on which the engine reads a regulation (`YYYY-MM-DD`)
//! and the [`TimePoint`] as of which a reduction evaluates.
//!
//! The reference date of a moment is the calendar date in the time zone of
//! that moment: `2025-03-12T00:30:00+01:00` is March 12, even though in UTC
//! it is still March 11.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, SecondsFormat};

/// Parse an `effective_at`. A moment without a time zone or in another
/// format is an error, which includes the moment.
pub fn moment(text: &str) -> Result<DateTime<FixedOffset>, String> {
    moment_of("effective_at", text)
}

/// Parse a moment; an error names the field (`effective_at`,
/// `recorded_at`, `as_of`) and the moment.
pub fn moment_of(field: &str, text: &str) -> Result<DateTime<FixedOffset>, String> {
    DateTime::parse_from_rfc3339(text).map_err(|e| format!("invalid {field} '{text}': {e}"))
}

/// A point on the time axis: a date (`YYYY-MM-DD`) or a moment with time
/// zone. An as-of moment is one, and so is an `effective_at` that binds an
/// event to a submitted value (such as a date stamp).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePoint {
    /// A whole calendar day.
    Date(NaiveDate),
    Moment(DateTime<FixedOffset>),
}

impl TimePoint {
    /// Parse a date or a moment; `field` names what it is in the error.
    pub fn read(field: &str, text: &str) -> Result<Self, String> {
        match NaiveDate::parse_from_str(text, "%Y-%m-%d") {
            Ok(d) => Ok(Self::Date(d)),
            Err(_) => DateTime::parse_from_rfc3339(text)
                .map(Self::Moment)
                .map_err(|_| {
                    format!("invalid {field} '{text}': not a date (YYYY-MM-DD) and not a moment with time zone (RFC 3339)")
                }),
        }
    }

    /// Whether a moment lies at or before this time point. A date counts the
    /// whole day, in the time zone of the moment (like [`reference_date`]).
    pub fn covers(&self, m: &DateTime<FixedOffset>) -> bool {
        match self {
            Self::Date(d) => m.date_naive() <= *d,
            Self::Moment(t) => m <= t,
        }
    }

    /// The time point as a moment: a date is the start of that day, in the
    /// time zone `zone`.
    pub fn as_moment(&self, zone: FixedOffset) -> DateTime<FixedOffset> {
        match self {
            Self::Moment(m) => *m,
            Self::Date(d) => {
                let local = d.and_time(chrono::NaiveTime::MIN);
                let utc = local - chrono::Duration::seconds(i64::from(zone.local_minus_utc()));
                DateTime::from_naive_utc_and_offset(utc, zone)
            }
        }
    }
}

impl std::fmt::Display for TimePoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Date(d) => write!(f, "{}", d.format("%Y-%m-%d")),
            Self::Moment(m) => f.write_str(&as_effective_at(m)),
        }
    }
}

/// A moment as a gram records it: to the second, with time zone.
pub fn as_effective_at(m: &DateTime<FixedOffset>) -> String {
    m.to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// The reference date (`YYYY-MM-DD`) of a moment, in its own time zone.
pub fn reference_date(m: &DateTime<FixedOffset>) -> String {
    m.date_naive().format("%Y-%m-%d").to_string()
}

/// The reference date of an `effective_at`.
pub fn reference_date_of(effective_at: &str) -> Result<String, String> {
    moment(effective_at).map(|m| reference_date(&m))
}

/// The year of a moment.
pub fn year(m: &DateTime<FixedOffset>) -> i64 {
    i64::from(m.year())
}

/// The day of a date (`YYYY-MM-DD`) or of a moment with time zone (in its
/// own time zone). `None` if the text is neither.
pub fn date_of(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .or_else(|| moment(text).ok().map(|m| m.date_naive()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn reference_date_in_its_own_time_zone() {
        let m = moment("2025-03-12T00:30:00+01:00").unwrap();
        assert_eq!(reference_date(&m), "2025-03-12");
        assert_eq!(
            reference_date_of("2024-12-31T23:59:59-05:00").unwrap(),
            "2024-12-31"
        );
        assert_eq!(as_effective_at(&m), "2025-03-12T00:30:00+01:00");
    }

    #[test]
    fn an_invalid_moment_is_an_error() {
        let f = moment("12 maart 2025").unwrap_err();
        assert!(f.contains("invalid effective_at '12 maart 2025'"), "{f}");
        // A date without a time is not a moment.
        assert!(moment("2025-03-12").is_err());
    }

    #[test]
    fn a_time_point_is_a_date_or_a_moment() {
        let d = TimePoint::read("as_of", "2026-01-01").unwrap();
        assert_eq!(d.to_string(), "2026-01-01");
        // A date counts the whole day, in the time zone of the moment.
        assert!(d.covers(&moment("2026-01-01T23:59:59+01:00").unwrap()));
        assert!(!d.covers(&moment("2026-01-02T00:00:00+01:00").unwrap()));
        let m = TimePoint::read("as_of", "2026-01-01T12:00:00+01:00").unwrap();
        assert!(m.covers(&moment("2026-01-01T11:00:00Z").unwrap()));
        assert!(!m.covers(&moment("2026-01-01T11:00:01Z").unwrap()));
        let zone = FixedOffset::east_opt(3600).unwrap();
        assert_eq!(
            as_effective_at(&d.as_moment(zone)),
            "2026-01-01T00:00:00+01:00"
        );
        let f = TimePoint::read("as_of", "gisteren").unwrap_err();
        assert!(f.contains("invalid as_of 'gisteren'"), "{f}");
    }

    #[test]
    fn date_of_a_date_or_a_moment() {
        assert_eq!(date_of("2025-03-12").map(|d| d.year()), Some(2025));
        assert_eq!(
            date_of("2026-01-01T00:00:00+01:00").map(|d| d.year()),
            Some(2026)
        );
        assert_eq!(date_of("geen datum"), None);
    }
}
