//! Today's calendar date, in the timezone Dutch law is written in.
//!
//! Which version of a law is in force, which consolidated text BWB will serve,
//! which date a harvested file is named after — all of these are statements
//! about the Dutch calendar. The timezone of the machine doing the asking is an
//! accident of deployment: containers run UTC, a laptop runs whatever the
//! traveller's laptop runs. So neither `Local::now()` nor `Utc::now()` answers
//! the question. `Europe/Amsterdam` does, and it is the only clock this
//! workspace reads a calendar date off.
//!
//! Instants are a different matter. A `last_harvested` stamp or a job timestamp
//! records a moment, not a calendar day, and those stay `Utc::now()`.

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Europe::Amsterdam;

/// The calendar date in the Netherlands right now.
pub fn today() -> NaiveDate {
    date_at(Utc::now())
}

/// The calendar date in the Netherlands as `YYYY-MM-DD`.
pub fn today_str() -> String {
    today().format("%Y-%m-%d").to_string()
}

/// The Dutch calendar date an instant falls on. Split out from [`today`] so the
/// mapping is testable without a clock.
pub fn date_at(instant: DateTime<Utc>) -> NaiveDate {
    instant.with_timezone(&Amsterdam).date_naive()
}

#[cfg(test)]
mod tests {
    use super::date_at;
    use chrono::{NaiveDate, TimeZone, Utc};

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).single().unwrap()
    }

    #[test]
    fn winter_time_runs_an_hour_ahead_of_utc() {
        // 00:30 CET on new year's day is still 23:30 UTC on the 31st. A UTC
        // clock names the harvest file after the old year.
        assert_eq!(
            date_at(utc(2025, 12, 31, 23, 30)),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
        );
    }

    #[test]
    fn summer_time_runs_two_hours_ahead_of_utc() {
        assert_eq!(
            date_at(utc(2026, 6, 30, 22, 30)),
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        );
    }

    #[test]
    fn midday_utc_lands_on_the_same_day() {
        assert_eq!(
            date_at(utc(2026, 6, 30, 12, 0)),
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap()
        );
    }
}
