//! Tijd in de cel-runtime, op een plek: het `op_moment` van een gram (een
//! moment met tijdzone, RFC 3339) en de peildatum waarop de engine een
//! regeling leest (`JJJJ-MM-DD`).
//!
//! De peildatum van een moment is de kalenderdatum in de tijdzone van dat
//! moment: `2025-03-12T00:30:00+01:00` is 12 maart, ook al is het dan in UTC
//! nog 11 maart.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, SecondsFormat};

/// Een `op_moment` gelezen. Een moment zonder tijdzone of in een andere vorm
/// is een fout, met het moment erbij.
pub fn moment(tekst: &str) -> Result<DateTime<FixedOffset>, String> {
    DateTime::parse_from_rfc3339(tekst).map_err(|e| format!("ongeldig op_moment '{tekst}': {e}"))
}

/// Een moment zoals een gram het vastlegt: op de seconde, met tijdzone.
pub fn als_op_moment(m: &DateTime<FixedOffset>) -> String {
    m.to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// De peildatum (`JJJJ-MM-DD`) van een moment, in zijn eigen tijdzone.
pub fn peildatum(m: &DateTime<FixedOffset>) -> String {
    m.date_naive().format("%Y-%m-%d").to_string()
}

/// De peildatum van een `op_moment`.
pub fn peildatum_van(op_moment: &str) -> Result<String, String> {
    moment(op_moment).map(|m| peildatum(&m))
}

/// Het jaartal van een moment.
pub fn jaar(m: &DateTime<FixedOffset>) -> i64 {
    i64::from(m.year())
}

/// Het jaartal van een datum (`JJJJ-MM-DD`) of een moment met tijdzone.
/// `None` als de tekst geen van beide is.
pub fn jaar_van(tekst: &str) -> Option<i64> {
    match NaiveDate::parse_from_str(tekst, "%Y-%m-%d") {
        Ok(d) => Some(i64::from(d.year())),
        Err(_) => moment(tekst).ok().map(|m| jaar(&m)),
    }
}

/// 1 januari van een jaar, als datum: de tegenhanger van [`jaar_van`].
pub fn eerste_dag_van_het_jaar(jaar: i64) -> String {
    format!("{jaar:04}-01-01")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn peildatum_in_de_eigen_tijdzone() {
        let m = moment("2025-03-12T00:30:00+01:00").unwrap();
        assert_eq!(peildatum(&m), "2025-03-12");
        assert_eq!(
            peildatum_van("2024-12-31T23:59:59-05:00").unwrap(),
            "2024-12-31"
        );
        assert_eq!(als_op_moment(&m), "2025-03-12T00:30:00+01:00");
    }

    #[test]
    fn een_ongeldig_moment_is_een_fout() {
        let f = moment("12 maart 2025").unwrap_err();
        assert!(f.contains("ongeldig op_moment '12 maart 2025'"), "{f}");
        // Een datum zonder tijd is geen moment.
        assert!(moment("2025-03-12").is_err());
    }

    #[test]
    fn jaar_van_een_datum_of_een_moment() {
        assert_eq!(jaar_van("2025-03-12"), Some(2025));
        assert_eq!(jaar_van("2026-01-01T00:00:00+01:00"), Some(2026));
        assert_eq!(jaar_van("geen datum"), None);
        assert_eq!(eerste_dag_van_het_jaar(2026), "2026-01-01");
    }
}
